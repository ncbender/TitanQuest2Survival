using CUE4Parse.FileProvider;
using CUE4Parse.UE4.Assets.Exports;
using CUE4Parse.UE4.Assets.Exports.Material;
using CUE4Parse.UE4.Assets.Exports.Texture;
using CUE4Parse.UE4.Objects.Core.Math;
using CUE4Parse.UE4.Objects.UObject;
using CUE4Parse.UE4.Assets.Objects;
using CUE4Parse_Conversion.Options;
using CUE4Parse_Conversion.Textures;
using Newtonsoft.Json;
using SharpGLTF.Schema2;

// Resolves TQ2 material instance chains (child parameters override parents) into a
// flat description the game turns into one shared StandardMaterial per material:
//   <outDir>/materials.json   name -> { albedo, normal, orm, color, uv_scale, alpha, cutoff, two_sided, hidden }
//   <outDir>/textures/*.png   every referenced texture exported once
// Grim master materials are procedural (masks, gradients, detail textures); this is an
// approximation built from the original texture maps, not a shader port.
// material_overrides.json supplies looks TQ2 assigns at runtime (archetype skin/hair, proxies).
class MaterialLibrary(DefaultFileProvider provider, string outDir)
{
    public record Def(string? albedo, string? normal, string? orm, float[] color, float uv_scale,
                      string alpha, float cutoff, bool two_sided, string chain, bool hidden = false);

    record Override(string? albedo, string? normal, string? orm, float[]? color, bool hidden, int slice);
    static readonly Dictionary<string, Override> Overrides = LoadOverrides();

    static Dictionary<string, Override> LoadOverrides()
    {
        var path = Path.Combine(AppContext.BaseDirectory, "material_overrides.json");
        if (!File.Exists(path)) return new();
        return JsonConvert.DeserializeObject<Dictionary<string, Newtonsoft.Json.Linq.JToken>>(File.ReadAllText(path))!
            .Where(kv => !kv.Key.StartsWith('_'))
            .ToDictionary(kv => kv.Key, kv => kv.Value.ToObject<Override>()!, StringComparer.OrdinalIgnoreCase);
    }

    readonly Dictionary<string, Def?> defs = new(StringComparer.OrdinalIgnoreCase);
    readonly Dictionary<string, string?> textures = new(StringComparer.OrdinalIgnoreCase);
    readonly Lookup<string, string> packagesByName = (Lookup<string, string>)provider.Files.Keys
        .Where(k => k.EndsWith(".uasset")).ToLookup(k => Path.GetFileNameWithoutExtension(k), StringComparer.OrdinalIgnoreCase);

    // Parameters are visited most-derived first; a parameter matches by exact name or substring.
    // Texture suffix is the fallback. ("orm" is exact-only: as a substring it would hit "normals".)
    static readonly (string[] exact, string[] contains, string suffix) Albedo = (["albedo", "basecolor"], ["albedo", "diffuse", "basecolor", "base color"], "_a");
    static readonly (string[] exact, string[] contains, string suffix) Normal = (["normal"], ["normal"], "_n");
    static readonly (string[] exact, string[] contains, string suffix) Orm = (["orm"], ["specularmasks", " orm"], "_orm");

    /// Adds every material used by the glTF meshes in <paramref name="glbs"/>.
    public void AddFromMeshes(IEnumerable<string> glbs)
    {
        foreach (var glb in glbs)
            foreach (var mat in ModelRoot.Load(glb).LogicalMaterials)
                if (mat.Name != null) Resolve(mat.Name);
    }

    public Def? Resolve(string name)
    {
        if (defs.TryGetValue(name, out var cached)) return cached;
        defs[name] = null;
        var pkg = packagesByName[name].FirstOrDefault();
        if (pkg == null) return null;
        UObject? obj;
        try { obj = provider.LoadPackage(pkg).GetExports().FirstOrDefault(e => e is UMaterialInterface); }
        catch (Exception ex) { Console.WriteLine($"  material {name}: {ex.Message}"); return null; }

        // Walk to the root material; the first (most derived) value of each parameter wins.
        var tex = new List<(string key, UTexture texture)>();
        var vec = new Dictionary<string, FLinearColor>(StringComparer.OrdinalIgnoreCase);
        var scl = new Dictionary<string, float>(StringComparer.OrdinalIgnoreCase);
        string? blend = null; float? cutoff = null; bool? twoSided = null;
        var chain = new List<string>();
        for (var cur = obj; cur != null && chain.Count < 16; )
        {
            chain.Add(cur.Name);
            var overrides = cur.GetOrDefault<FStructFallback>("BasePropertyOverrides");
            if (overrides != null)
            {
                if (overrides.GetOrDefault<bool>("bOverride_BlendMode")) blend ??= overrides.GetOrDefault<FName>("BlendMode").Text;
                if (overrides.GetOrDefault<bool>("bOverride_OpacityMaskClipValue")) cutoff ??= overrides.GetOrDefault<float>("OpacityMaskClipValue");
                if (overrides.GetOrDefault<bool>("bOverride_TwoSided")) twoSided ??= overrides.GetOrDefault<bool>("TwoSided");
            }
            if (cur is UMaterialInstanceConstant mic)
            {
                foreach (var p in mic.TextureParameterValues ?? [])
                    if (p.ParameterValue.TryLoad(out UTexture? t) && t != null) tex.Add((p.ParameterInfo.Name.Text.ToLowerInvariant(), t));
                foreach (var p in mic.VectorParameterValues ?? [])
                    if (p.ParameterValue is { } v) vec.TryAdd(p.ParameterInfo.Name.Text, v);
                foreach (var p in mic.ScalarParameterValues ?? [])
                    scl.TryAdd(p.ParameterInfo.Name.Text, p.ParameterValue);
                cur = mic.Parent?.Load<UObject>();
            }
            else
            {
                // Root UMaterial: textures wired directly into the graph are not parameters;
                // add them by name (key = texture name, matched by suffix below).
                if (cur is UMaterial m)
                    foreach (var idx in m.ReferencedTextures ?? [])
                        if (idx.TryLoad(out UTexture? t) && t != null) tex.Add((t.Name.ToLowerInvariant(), t));
                // Its own blend settings apply when no instance overrode them.
                var b = cur.GetOrDefault<FName>("BlendMode");
                if (!b.IsNone) blend ??= b.Text;
                cutoff ??= cur.GetOrDefault("OpacityMaskClipValue", 0.3333f);
                twoSided ??= cur.GetOrDefault<bool>("TwoSided");
                cur = null;
            }
        }

        // Engine placeholders (DefaultDiffuse, T_Default_Material_Grid_*) are never the real look.
        var real = tex.Where(t => !t.texture.Name.StartsWith("Default", StringComparison.OrdinalIgnoreCase)
                                && !t.texture.Name.StartsWith("T_Default", StringComparison.OrdinalIgnoreCase)).ToList();
        string? Pick((string[] exact, string[] contains, string suffix) slot) =>
            Export(real.FirstOrDefault(t => slot.exact.Contains(t.key) || slot.contains.Any(k => t.key.Contains(k))).texture
                   ?? real.FirstOrDefault(t => t.texture.Name.EndsWith(slot.suffix, StringComparison.OrdinalIgnoreCase)).texture);

        var albedo = Pick(Albedo);
        // Equipment albedos are grey: Grim tints them per material zone (ID map) with a colour
        // scheme at runtime. Bake TQ2's item-icon scheme into a dyed copy.
        var idMap = real.FirstOrDefault(t => t.key == "id").texture;
        var albedoTex = real.FirstOrDefault(t => Albedo.exact.Contains(t.key)).texture;
        string? dyedOrm = null;
        if (idMap != null && albedoTex != null && chain.Any(c => c.StartsWith("m_master_equipment", StringComparison.OrdinalIgnoreCase))
            && Dye.Bake(provider, albedoTex, idMap, outDir) is { } dyed)
            (albedo, dyedOrm) = dyed;
        // Tiling master textures are tinted by the family's Color/Tint parameters (e.g. rocks).
        var color = new float[] { 1, 1, 1, 1 };
        if (vec.TryGetValue("Color", out var c) && !tex.Any(t => t.key is "albedo" && t.texture.Name.EndsWith("_a")))
        {
            var tint = vec.TryGetValue("Tint", out var ti) ? ti : new FLinearColor(1, 1, 1, 1);
            // Color is a dark multiplier in linear space; scale so the detail texture stays visible.
            color = [Math.Clamp(c.R * tint.R * 3, 0, 1), Math.Clamp(c.G * tint.G * 3, 0, 1), Math.Clamp(c.B * tint.B * 3, 0, 1), 1];
        }
        var alpha = blend switch
        {
            "EBlendMode::BLEND_Masked" => "mask",
            "EBlendMode::BLEND_Translucent" or "EBlendMode::BLEND_Additive" => "blend",
            _ => "opaque",
        };
        var def = new Def(albedo, Pick(Normal), dyedOrm ?? Pick(Orm), color,
            scl.GetValueOrDefault("UV Scale", 1f), alpha, cutoff ?? 0.3333f, twoSided ?? false, string.Join(" > ", chain));
        // Hair colour comes from the archetype at runtime; the master's own texture is just
        // a layered noise. Render Grim hair as a plain dark colour instead.
        if (chain[^1].Equals("m_human_hair_master", StringComparison.OrdinalIgnoreCase) && def.albedo?.Contains("t_3noise") == true && !Overrides.ContainsKey(name))
            def = def with { albedo = null, color = [0.07f, 0.05f, 0.035f, 1f] };
        if (Overrides.TryGetValue(name, out var o))
        {
            string? Tex(string? asset) => asset is null or "none" ? null
                : Export(packagesByName[asset].Select(p => provider.LoadPackage(p).GetExports().OfType<UTexture>().FirstOrDefault()).FirstOrDefault(t => t != null), o.slice);
            def = def with
            {
                albedo = o.albedo != null ? Tex(o.albedo) : def.albedo,
                normal = o.normal != null ? Tex(o.normal) : def.normal,
                orm = o.orm != null ? Tex(o.orm) : def.orm,
                color = o.color ?? def.color,
                hidden = o.hidden,
                chain = def.chain + " (override)",
            };
        }
        return defs[name] = def;
    }

    /// Exports the texture asset with this name (e.g. "t_sand_beach_02a_a"), if it exists.
    public string? ExportByName(string assetName) =>
        Export(packagesByName[assetName].Select(p => provider.LoadPackage(p).GetExports().OfType<UTexture>().FirstOrDefault()).FirstOrDefault(t => t != null));

    /// Exports a texture once. Grim packs variants (e.g. 9 skin tones) side by side into one
    /// virtual texture whose UVs cover a single block; only block `slice` is kept.
    string? Export(UTexture? texture, int slice = 0)
    {
        if (texture == null) return null;
        if (textures.TryGetValue(texture.Name, out var done)) return done;
        string? rel = null;
        try
        {
            var decoded = texture.Decode(ETexturePlatform.DesktopMobile);
            if (decoded != null)
            {
                byte[] png;
                int blocks = decoded.Height > 0 ? decoded.Width / decoded.Height : 1;
                if (texture.GetOrDefault<bool>("VirtualTextureStreaming") && blocks > 1 && decoded.Width == blocks * decoded.Height)
                {
                    using var full = decoded.ToSkBitmap();
                    using var block = new SkiaSharp.SKBitmap();
                    var x = Math.Clamp(slice, 0, blocks - 1) * decoded.Height;
                    full.ExtractSubset(block, new SkiaSharp.SKRectI(x, 0, x + decoded.Height, decoded.Height));
                    using var image = SkiaSharp.SKImage.FromBitmap(block);
                    png = image.Encode(SkiaSharp.SKEncodedImageFormat.Png, 100).ToArray();
                }
                else png = decoded.Encode(ETextureFormat.Png, false, out _);
                if (texture.Name.EndsWith("_nm", StringComparison.OrdinalIgnoreCase)) png = RebuildNormalZ(png);
                rel = $"textures/{texture.Name}.png";
                Directory.CreateDirectory(Path.Combine(outDir, "textures"));
                File.WriteAllBytes(Path.Combine(outDir, rel), png);
            }
        }
        catch (Exception ex) { Console.WriteLine($"  texture {texture.Name}: {ex.GetType().Name} {ex.Message}"); }
        return textures[texture.Name] = rel;
    }

    /// Grim's skin "_nm" maps keep only X/Y in R/G (blue holds another mask); rebuild Z so
    /// standard tangent-space shading works.
    static byte[] RebuildNormalZ(byte[] png)
    {
        using var bmp = SkiaSharp.SKBitmap.Decode(png);
        for (int y = 0; y < bmp.Height; y++)
            for (int x = 0; x < bmp.Width; x++)
            {
                var c = bmp.GetPixel(x, y);
                float nx = c.Red / 127.5f - 1, ny = c.Green / 127.5f - 1;
                float nz = MathF.Sqrt(MathF.Max(0, 1 - nx * nx - ny * ny));
                bmp.SetPixel(x, y, new SkiaSharp.SKColor(c.Red, c.Green, (byte)Math.Round((nz + 1) * 127.5f), 255));
            }
        using var image = SkiaSharp.SKImage.FromBitmap(bmp);
        return image.Encode(SkiaSharp.SKEncodedImageFormat.Png, 100).ToArray();
    }

    public void Save()
    {
        var resolved = defs.Where(d => d.Value != null).ToDictionary(d => d.Key, d => d.Value);
        File.WriteAllText(Path.Combine(outDir, "materials.json"), JsonConvert.SerializeObject(resolved, Formatting.Indented));
        Console.WriteLine($"materials.json: {resolved.Count} materials ({resolved.Count(d => d.Value!.albedo != null)} with albedo), {textures.Count(t => t.Value != null)} textures");
    }
}
