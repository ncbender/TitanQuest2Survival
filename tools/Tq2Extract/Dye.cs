using CUE4Parse.FileProvider;
using CUE4Parse.UE4.Assets.Exports.Texture;
using CUE4Parse_Conversion.Textures;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using SkiaSharp;

// Grim's equipment master (m_master_equipment_2) colours grey albedos per material zone. The
// zone is encoded in the ID texture: the red channel marks leather/wood, green cloth/accent,
// blue metal/bone, each at levels ~64/128/192 (255 = crest/fur/gem). Each zone's colour comes
// from a colour scheme (GrimArchetypeModificationSelection); the neutral tint is 0.5, so the
// result is albedo × tint × 2 in linear space. We bake the scheme TQ2 uses for its item icons
// (ms_item_equipment_color, entry "battlemage") so the game needs no special shader.
static class Dye
{
    const string SchemeAsset = "TQ2/Content/TQ2/Character/Item_Icon_Rendering/ms_item_equipment_color.uasset";
    const string Scheme = "battlemage";
    static readonly string[][] Zones =
    [
        ["Leather1", "Leather2", "Wood", "Crest"],
        ["Cloth1", "Cloth2", "Accent", "Fur"],
        ["Metal1", "Metal2", "Bone", "Gem"],
    ];
    static Dictionary<string, float[]>? palette;

    /// Writes textures/<albedo>_dyed.png (colours) and <albedo>_orm.png (metal zones metallic,
    /// cloth/leather matte); returns both relative paths (null if it fails).
    public static (string albedo, string orm)? Bake(DefaultFileProvider provider, UTexture albedo, UTexture id, string outDir)
    {
        var rel = $"textures/{albedo.Name}_dyed.png";
        var ormRel = $"textures/{albedo.Name}_orm.png";
        var path = Path.Combine(outDir, rel);
        if (File.Exists(path) && File.Exists(Path.Combine(outDir, ormRel))) return (rel, ormRel);
        try
        {
            palette ??= LoadPalette(provider);
            using var a = Decode(albedo);
            using var idSrc = Decode(id);
            if (a == null || idSrc == null) return null;
            using var ids = idSrc.Width == a.Width ? idSrc.Copy() : idSrc.Resize(new SKImageInfo(a.Width, a.Height), SKFilterQuality.None);
            var toLinear = Enumerable.Range(0, 256).Select(i => (float)Math.Pow(i / 255.0, 2.2)).ToArray();
            var tints = Zones.Select(ch => ch.Select(z => palette.GetValueOrDefault(z, [0.5f, 0.5f, 0.5f])).ToArray()).ToArray();
            using var outBmp = new SKBitmap(a.Width, a.Height, SKColorType.Rgba8888, SKAlphaType.Unpremul);
            // Bevy ORM: R occlusion, G roughness, B metallic.
            using var orm = new SKBitmap(a.Width, a.Height, SKColorType.Rgba8888, SKAlphaType.Unpremul);
            for (int y = 0; y < a.Height; y++)
                for (int x = 0; x < a.Width; x++)
                {
                    var c = a.GetPixel(x, y);
                    var z = ids.GetPixel(x, y);
                    byte[] channels = [z.Red, z.Green, z.Blue];
                    int ch = Array.IndexOf(channels, channels.Max());
                    int level = (int)Math.Round(channels[ch] / 64.0);
                    if (channels[ch] < 32 || level < 1)
                    {
                        outBmp.SetPixel(x, y, c);
                        orm.SetPixel(x, y, new SKColor(255, 220, 0));
                        continue;
                    }
                    bool metal = ch == 2 && level <= 2;
                    orm.SetPixel(x, y, metal ? new SKColor(255, 110, 255) : new SKColor(255, 215, 0));
                    var t = tints[ch][Math.Min(level, 4) - 1];
                    byte Tint(byte v, float k) => (byte)Math.Clamp(Math.Pow(Math.Min(toLinear[v] * k * 2f, 1f), 1 / 2.2) * 255.0, 0, 255);
                    outBmp.SetPixel(x, y, new SKColor(Tint(c.Red, t[0]), Tint(c.Green, t[1]), Tint(c.Blue, t[2]), c.Alpha));
                }
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            using var image = SKImage.FromBitmap(outBmp);
            File.WriteAllBytes(path, image.Encode(SKEncodedImageFormat.Png, 100).ToArray());
            using var ormImage = SKImage.FromBitmap(orm);
            File.WriteAllBytes(Path.Combine(outDir, ormRel), ormImage.Encode(SKEncodedImageFormat.Png, 100).ToArray());
            return (rel, ormRel);
        }
        catch (Exception ex)
        {
            Console.WriteLine($"  dye {albedo.Name}: {ex.GetType().Name} {ex.Message}");
            return null;
        }
    }

    static SKBitmap? Decode(UTexture texture) => texture.Decode(ETexturePlatform.DesktopMobile)?.ToSkBitmap();

    /// Zone name (e.g. "Metal1") -> linear RGB of the scheme.
    static Dictionary<string, float[]> LoadPalette(DefaultFileProvider provider)
    {
        var json = JArray.Parse(JsonConvert.SerializeObject(provider.LoadPackage(SchemeAsset).GetExports()));
        var props = json[0]["Properties"]!;
        var byGuid = props["m_Parameters"]!.ToDictionary(p => (string)p["m_Identifier"]!["m_Guid"]!, p => (string)p["m_Parameter"]!);
        var entry = props["m_Entries"]!.First(e => (string?)e["m_DisplayName"]?["CultureInvariantString"] == Scheme);
        var result = new Dictionary<string, float[]>();
        foreach (var m in entry["m_Modifications"]!)
        {
            var param = byGuid.GetValueOrDefault((string)m["m_AssociatedParameter"]!["m_Guid"]!, "");
            if (!param.EndsWith("_Color_Tint") || m["m_OptionalColor"]?["m_IsSet"]?.Value<bool>() != true) continue;
            var c = m["m_OptionalColor"]!["m_Color"]!;
            result[param.Replace("_Color_Tint", "")] = [(float)c["R"]!, (float)c["G"]!, (float)c["B"]!];
        }
        Console.WriteLine($"  dye scheme '{Scheme}': {string.Join(", ", result.Keys)}");
        return result;
    }
}
