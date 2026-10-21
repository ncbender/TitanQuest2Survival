using Newtonsoft.Json.Linq;
using System.Numerics;
using CUE4Parse.FileProvider;
using CUE4Parse.UE4.Assets.Exports;
using CUE4Parse.UE4.Assets.Exports.Component;
using CUE4Parse.UE4.Assets.Exports.Component.StaticMesh;
using CUE4Parse.UE4.Assets.Exports.StaticMesh;
using CUE4Parse.UE4.Objects.Core.Math;
using CUE4Parse.UE4.Objects.UObject;
using CUE4Parse.UE4.Assets.Objects;
using CUE4Parse.UE4.Objects.PhysicsEngine;
using CUE4Parse_Conversion;
using CUE4Parse_Conversion.Options;
using Newtonsoft.Json;
using SharpGLTF.Schema2;

// Exports a map as one .glb per unique static mesh plus scene.json listing every
// placement (static mesh components, instanced/foliage instances, level instances)
// already converted to Bevy space (Y-up, right-handed, metres), plus colliders.json with
// each mesh's original UE simple collision (boxes, spheres, capsules, convex hulls).
static class SceneExport
{
    internal record Placement(string Mesh, Matrix4x4 Ue, bool Collides = true, bool Visible = true);

    public static async Task Run(DefaultFileProvider provider, string cache, string name, string mapPath)
    {
        var placements = new List<Placement>();
        var meshes = new Dictionary<string, UObject>(); // static meshes and landscape actors
        CollectLevel(provider, mapPath, Matrix4x4.Identity, placements, meshes, 0);
        await Build(provider, cache, name, mapPath, placements, meshes);
    }

    /// Size of a Greece landscape streaming proxy (m) and how far the ground reaches past a region.
    const float ProxySize = 63;
    const float TerrainMargin = 40;

    /// A square of the Greece world partition around (x, y) (UE metres) with half size `radius`:
    /// every generated cell overlapping it (see cache/cells.json from `cells`), cut to the square.
    public static async Task RunRegion(DefaultFileProvider provider, string cache, string name, float x, float y, float radius)
    {
        var cells = JArray.Parse(File.ReadAllText(Path.Combine(cache, "cells.json")));
        var placements = new List<Placement>();
        var meshes = new Dictionary<string, UObject>();
        var dir = "TQ2/Content/TQ2/Maps/Greece/l_gr_p/_Generated_/";
        int used = 0;
        foreach (var c in cells)
        {
            float minX = (float)c["minX"]!, maxX = (float)c["maxX"]!, minY = (float)c["minY"]!, maxY = (float)c["maxY"]!;
            // Landscape proxies are placed by their corner; allow for their size and the margin.
            float pad = (bool)c["landscape"]! ? ProxySize + TerrainMargin : 0;
            if (maxX + pad < x - radius || minX > x + radius + pad || maxY + pad < y - radius || minY > y + radius + pad) continue;
            int before = placements.Count;
            CollectLevel(provider, dir + (string)c["map"]!, Matrix4x4.Identity, placements, meshes, 0);
            used++;
            if (Environment.GetEnvironmentVariable("TQ2_DEBUG_REGION") != null)
                foreach (var p in placements.Skip(before).Where(p => meshes[p.Mesh] is not UStaticMesh))
                    Console.WriteLine($"  {c["map"]}: {p.Mesh.Split('.').Last()} at {p.Ue.M41 / 100:0} {p.Ue.M42 / 100:0}");
        }
        bool Inside(Placement p, float margin) =>
            Math.Abs(p.Ue.M41 / 100 - x) <= radius + margin && Math.Abs(p.Ue.M42 / 100 - y) <= radius + margin;
        // A proxy (corner + ProxySize) is kept when it overlaps the square plus the margin, so the
        // ground reaches a little past the props without the holes of a partial ring further out.
        bool ProxyNear(Placement p)
        {
            float px = p.Ue.M41 / 100, py = p.Ue.M42 / 100, m = radius + TerrainMargin;
            return px <= x + m && px + ProxySize >= x - m && py <= y + m && py + ProxySize >= y - m;
        }
        // Props by their bounds: big rocks and cliffs whose origin lies outside still fill the edge
        // (TQ2 leaves the landscape out under them).
        bool PropNear(Placement p)
        {
            var bounds = (meshes[p.Mesh] as UStaticMesh)?.GetOrDefault<FBoxSphereBounds>("ExtendedBounds");
            if (bounds == null) return Inside(p, 0);
            var c = Vector3.Transform(new Vector3((float)bounds.Origin.X, (float)bounds.Origin.Y, (float)bounds.Origin.Z), p.Ue) / 100;
            var scale = MathF.Max(new Vector3(p.Ue.M11, p.Ue.M12, p.Ue.M13).Length(), MathF.Max(new Vector3(p.Ue.M21, p.Ue.M22, p.Ue.M23).Length(), new Vector3(p.Ue.M31, p.Ue.M32, p.Ue.M33).Length()));
            float r = (float)bounds.SphereRadius / 100 * scale;
            // Origin inside, or a large mesh reaching into the square plus the terrain margin.
            if (Inside(p, 0)) return true;
            return r > 8 && Math.Abs(c.X - x) <= radius + TerrainMargin + r && Math.Abs(c.Y - y) <= radius + TerrainMargin + r;
        }
        placements = placements.Where(p => meshes[p.Mesh] is not UStaticMesh ? ProxyNear(p) : PropNear(p)).ToList();
        var keep = placements.Select(p => p.Mesh).ToHashSet();
        foreach (var k in meshes.Keys.Where(k => !keep.Contains(k)).ToList()) meshes.Remove(k);
        Console.WriteLine($"region ({x}, {y}) ±{radius} m: {used} cells");
        if (Environment.GetEnvironmentVariable("TQ2_DEBUG_REGION") != null) return;
        await Build(provider, cache, name, $"region {x} {y} {radius}", placements, meshes);
    }

    static async Task Build(DefaultFileProvider provider, string cache, string name, string mapPath, List<Placement> placements, Dictionary<string, UObject> meshes)
    {
        var outDir = Path.Combine(cache, "export", name);
        Console.WriteLine($"{placements.Count} placements of {meshes.Count} unique meshes ({meshes.Values.Count(m => m is not UStaticMesh)} landscapes)");

        // Export each unique mesh (+ its materials/textures) with CUE4Parse.
        var rawDir = Path.Combine(outDir, "raw");
        var session = new ExportSession(null!);
        foreach (var m in meshes.Values) session.Add(m);
        // Materials are resolved separately (MaterialLibrary), so skip CUE4Parse's per-mesh texture export.
        var results = await session.RunAsync(rawDir, new ExportOptions(EMeshFormat.Gltf2, exportMaterials: false, exportMorphTargets: false), null!, CancellationToken.None);
        Console.WriteLine($"exported {results.Count(r => r.Success)}/{results.Count} objects ({results.Count(r => !r.Success)} failed)");
        foreach (var r in results.Where(r => !r.Success).Take(5)) Console.WriteLine($"  FAIL {r.ObjectPath}: {r.Error?.Message}");

        // Copy every mesh glb to meshes/<name>.glb and resolve the materials it uses.
        var files = Directory.EnumerateFiles(rawDir, "*.glb", SearchOption.AllDirectories)
            .GroupBy(f => Path.GetFileName(f).ToLowerInvariant()).ToDictionary(g => g.Key, g => g.First());
        Directory.CreateDirectory(Path.Combine(outDir, "meshes"));
        var meshFiles = new Dictionary<string, string>();
        foreach (var (path, mesh) in meshes)
        {
            if (!files.TryGetValue(mesh.Name.ToLowerInvariant() + ".glb", out var glb)) continue;
            var rel = $"meshes/{mesh.Name}.glb";
            File.Copy(glb, Path.Combine(outDir, rel), true);
            meshFiles[path] = rel;
        }
        var materials = new MaterialLibrary(provider, outDir);
        materials.AddFromMeshes(meshFiles.Values.Select(f => Path.Combine(outDir, f)));
        materials.Save();
        Directory.Delete(rawDir, true);

        var instances = new List<object>();
        foreach (var p in placements)
        {
            if (!meshFiles.TryGetValue(p.Mesh, out var file)) continue;
            if (!Matrix4x4.Decompose(p.Ue, out var s, out var q, out var t)) continue;
            // UE (Z-up, left-handed, cm) -> Bevy/glTF (Y-up, right-handed, m); matches the mesh writer.
            instances.Add(new
            {
                mesh = file,
                t = new[] { t.X * 0.01f, t.Z * 0.01f, t.Y * 0.01f },
                r = new[] { q.X, q.Z, q.Y, -q.W },
                s = new[] { s.X, s.Z, s.Y },
                c = p.Collides,
                v = p.Visible,
            });
        }
        File.WriteAllText(Path.Combine(outDir, "scene.json"), JsonConvert.SerializeObject(new { map = mapPath, instances }));
        Console.WriteLine($"wrote {outDir}/scene.json: {instances.Count} instances, {meshFiles.Count} meshes");

        var colliders = meshes.Where(m => m.Value is UStaticMesh && meshFiles.ContainsKey(m.Key))
            .Select(m => (file: meshFiles[m.Key], shapes: Colliders((UStaticMesh)m.Value)))
            .Where(c => c.shapes.Count > 0).GroupBy(c => c.file).ToDictionary(g => g.Key, g => g.First().shapes);
        File.WriteAllText(Path.Combine(outDir, "colliders.json"), JsonConvert.SerializeObject(colliders));
        Console.WriteLine($"wrote colliders.json: {colliders.Count}/{meshFiles.Count} meshes have collision, {placements.Count(p => !p.Collides)} placements opt out");

        var landscapes = placements.Where(p => meshes[p.Mesh] is not UStaticMesh && meshFiles.ContainsKey(p.Mesh))
            .Select(p => (Path.Combine(outDir, meshFiles[p.Mesh]), ToBevy(p.Ue))).ToList();
        if (landscapes.Count > 0) WriteGround(Path.Combine(outDir, "ground.json"), landscapes);

        var landscapeParts = placements.Where(p => meshes[p.Mesh] is not UStaticMesh && meshFiles.ContainsKey(p.Mesh))
            .Select(p => (actor: meshes[p.Mesh], glb: Path.Combine(outDir, meshFiles[p.Mesh]), transform: ToBevy(p.Ue))).ToList();
        if (landscapeParts.Count > 0) await WriteTerrain(provider, materials, outDir, landscapeParts);
    }

    /// TQ2's ground texture sets (Ground/Terrain/Textures/t_<name>_a), for layers not listed in
    /// terrain_layers.json: the set whose name contains all words of the layer name wins.
    static string? GuessTerrainSet(DefaultFileProvider provider, string layer)
    {
        var sets = provider.Files.Keys
            .Where(k => k.Contains("/Ground/Terrain/Textures/", StringComparison.OrdinalIgnoreCase) && k.EndsWith("_a.uasset"))
            .Select(k => Path.GetFileNameWithoutExtension(k)[..^2])
            .Distinct()
            .ToList();
        var words = layer.ToLowerInvariant().Split('_', StringSplitOptions.RemoveEmptyEntries).Where(w => w.Length > 2).ToArray();
        var best = sets.Where(s => words.All(w => s.Contains(w))).OrderBy(s => s.Length).FirstOrDefault()
            ?? sets.Where(s => words.Any(w => s.Contains(w))).OrderByDescending(s => words.Count(w => s.Contains(w))).ThenBy(s => s.Length).FirstOrDefault();
        if (best != null) Console.WriteLine($"  terrain layer {layer}: guessed {best}");
        return best;
    }

    /// Terrain layer blending: CUE4Parse writes one full-size weight map per landscape layer
    /// (ll_<layer>.png); terrain_layers.json names the TQ2 ground textures for each layer.
    /// Writes terrain/*.png and terrain.json { material, tiling, layers[{name, weight, albedo, normal}] }.
    /// Terrain layer blending. CUE4Parse writes one weight map per layer for every landscape
    /// actor (ll_<layer>.png, addressed by the actor's UV_1). A map can consist of many streaming
    /// proxies, so every proxy is exported on its own, the UV_1 -> world mapping is fitted from
    /// its vertices, and all proxies are resampled into one world-aligned map per layer
    /// (1 px per metre). terrain.json { material, tiling, bounds[minX, minZ, maxX, maxZ], layers }.
    static async Task WriteTerrain(DefaultFileProvider provider, MaterialLibrary materials, string outDir,
        List<(UObject actor, string glb, Matrix4x4 transform)> parts)
    {
        var raw = Path.Combine(outDir, "raw_terrain");
        var mapping = JsonConvert.DeserializeObject<Dictionary<string, string?>>(
            File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "terrain_layers.json")))!;

        // Per proxy: its weight maps and the affine UV_1 -> world XZ fit.
        var proxies = new List<(Dictionary<string, SkiaSharp.SKBitmap> weights, float[] fit, float minX, float minZ, float maxX, float maxZ)>();
        for (int i = 0; i < parts.Count; i++)
        {
            var (actor, glb, m) = parts[i];
            var dir = Path.Combine(raw, i.ToString());
            var session = new ExportSession(null!);
            session.Add(actor);
            await session.RunAsync(dir, new ExportOptions(EMeshFormat.Gltf2, exportMorphTargets: false), null!, CancellationToken.None);
            var weights = Directory.EnumerateFiles(dir, "ll_*.png", SearchOption.AllDirectories)
                .ToDictionary(f => Path.GetFileNameWithoutExtension(f)[3..], f => SkiaSharp.SKBitmap.Decode(f));
            var pos = new List<Vector3>();
            var uv = new List<Vector2>();
            foreach (var prim in ModelRoot.Load(glb).LogicalMeshes.SelectMany(me => me.Primitives))
            {
                var uv1 = prim.GetVertexAccessor("TEXCOORD_1");
                if (uv1 == null) continue;
                pos.AddRange(prim.GetVertexAccessor("POSITION").AsVector3Array().Select(v => Vector3.Transform(v, m)));
                uv.AddRange(uv1.AsVector2Array());
            }
            if (pos.Count < 3 || weights.Count == 0) continue;
            proxies.Add((weights, FitUvToWorld(pos, uv), pos.Min(p => p.X), pos.Min(p => p.Z), pos.Max(p => p.X), pos.Max(p => p.Z)));
        }
        if (proxies.Count == 0) return;
        float bx0 = proxies.Min(p => p.minX), bz0 = proxies.Min(p => p.minZ), bx1 = proxies.Max(p => p.maxX), bz1 = proxies.Max(p => p.maxZ);
        int w = (int)Math.Ceiling(bx1 - bx0) + 1, h = (int)Math.Ceiling(bz1 - bz0) + 1;

        Directory.CreateDirectory(Path.Combine(outDir, "terrain"));
        var layers = new List<object>();
        var names = proxies.SelectMany(p => p.weights.Keys).Distinct().OrderBy(n => n).ToList();
        foreach (var layer in names)
        {
            if (!mapping.TryGetValue(layer, out var set) && (set = GuessTerrainSet(provider, layer)) == null)
            {
                Console.WriteLine($"  terrain layer {layer}: not in terrain_layers.json and no matching texture set, skipped");
                continue;
            }
            if (set == null) continue; // blend modifier without textures
            var albedo = materials.ExportByName(set + "_a") ?? materials.ExportByName(set + "_albedo");
            var normal = materials.ExportByName(set + "_n") ?? materials.ExportByName(set + "b_n");
            if (albedo == null)
            {
                Console.WriteLine($"  terrain layer {layer}: no albedo for {set}, skipped");
                continue;
            }
            using var merged = new SkiaSharp.SKBitmap(w, h, SkiaSharp.SKColorType.Gray8, SkiaSharp.SKAlphaType.Opaque);
            merged.Erase(SkiaSharp.SKColors.Black);
            foreach (var (weights, fit, minX, minZ, maxX, maxZ) in proxies)
            {
                if (!weights.TryGetValue(layer, out var map)) continue;
                for (int z = (int)Math.Floor(minZ - bz0); z <= (int)Math.Ceiling(maxZ - bz0) && z < h; z++)
                    for (int x = (int)Math.Floor(minX - bx0); x <= (int)Math.Ceiling(maxX - bx0) && x < w; x++)
                    {
                        if (x < 0 || z < 0) continue;
                        var (u, v) = WorldToUv(fit, bx0 + x, bz0 + z);
                        if (u < 0 || v < 0 || u > 1 || v > 1) continue;
                        var c = map.GetPixel(Math.Clamp((int)(u * (map.Width - 1) + 0.5f), 0, map.Width - 1), Math.Clamp((int)(v * (map.Height - 1) + 0.5f), 0, map.Height - 1));
                        var old = merged.GetPixel(x, z).Red;
                        if (c.Red > old) merged.SetPixel(x, z, new SkiaSharp.SKColor(c.Red, c.Red, c.Red));
                    }
            }
            var rel = $"terrain/ll_{layer}.png";
            using (var image = SkiaSharp.SKImage.FromBitmap(merged))
                File.WriteAllBytes(Path.Combine(outDir, rel), image.Encode(SkiaSharp.SKEncodedImageFormat.Png, 100).ToArray());
            layers.Add(new { name = layer, weight = rel, albedo, normal });
        }
        foreach (var p in proxies) foreach (var b in p.weights.Values) b.Dispose();
        Directory.Delete(raw, true);
        // Proxies of one map can use different landscape material instances; all get blended.
        var landscapeMaterials = parts.SelectMany(p => ModelRoot.Load(p.glb).LogicalMaterials.Select(m => m.Name)).Distinct().ToList();
        var material = landscapeMaterials.FirstOrDefault();
        // UE landscape materials tile ground textures every few metres; 1 repeat per 4 m.
        File.WriteAllText(Path.Combine(outDir, "terrain.json"),
            JsonConvert.SerializeObject(new { material, materials = landscapeMaterials, tiling = 0.25f, bounds = new[] { bx0, bz0, bx0 + w - 1, bz0 + h - 1 }, layers }, Formatting.Indented));
        Console.WriteLine($"wrote terrain.json: {layers.Count} layers from {proxies.Count} landscape parts, {w}x{h} m");
    }

    /// Least-squares affine fit world (x, z) = A·(u, v, 1); returns its inverse as 6 floats so
    /// (u, v) = inv·(x, z, 1).
    static float[] FitUvToWorld(List<Vector3> pos, List<Vector2> uv)
    {
        // Normal equations for x and z separately on the basis (u, v, 1).
        double[,] m = new double[3, 3];
        double[] bx = new double[3], bz = new double[3];
        for (int i = 0; i < pos.Count; i++)
        {
            double[] f = [uv[i].X, uv[i].Y, 1];
            for (int r = 0; r < 3; r++)
            {
                for (int c = 0; c < 3; c++) m[r, c] += f[r] * f[c];
                bx[r] += f[r] * pos[i].X;
                bz[r] += f[r] * pos[i].Z;
            }
        }
        var ax = Solve3(m, bx);
        var az = Solve3(m, bz);
        // x = ax0 u + ax1 v + ax2 ; z = az0 u + az1 v + az2 -> invert the 2x2 part.
        double det = ax[0] * az[1] - ax[1] * az[0];
        double i00 = az[1] / det, i01 = -ax[1] / det, i10 = -az[0] / det, i11 = ax[0] / det;
        return [(float)i00, (float)i01, (float)(-(i00 * ax[2] + i01 * az[2])), (float)i10, (float)i11, (float)(-(i10 * ax[2] + i11 * az[2]))];
    }

    static (float u, float v) WorldToUv(float[] inv, float x, float z) =>
        (inv[0] * x + inv[1] * z + inv[2], inv[3] * x + inv[4] * z + inv[5]);

    static double[] Solve3(double[,] a, double[] b)
    {
        var m = (double[,])a.Clone();
        var r = (double[])b.Clone();
        for (int c = 0; c < 3; c++)
        {
            int p = c;
            for (int k = c + 1; k < 3; k++) if (Math.Abs(m[k, c]) > Math.Abs(m[p, c])) p = k;
            for (int k = 0; k < 3; k++) (m[c, k], m[p, k]) = (m[p, k], m[c, k]);
            (r[c], r[p]) = (r[p], r[c]);
            for (int k = c + 1; k < 3; k++)
            {
                var f = m[k, c] / m[c, c];
                for (int j = c; j < 3; j++) m[k, j] -= f * m[c, j];
                r[k] -= f * r[c];
            }
        }
        var x = new double[3];
        for (int c = 2; c >= 0; c--)
        {
            double sum = r[c];
            for (int k = c + 1; k < 3; k++) sum -= m[c, k] * x[k];
            x[c] = sum / m[c, c];
        }
        return x;
    }

    static Matrix4x4 ToBevy(Matrix4x4 ue)
    {
        Matrix4x4.Decompose(ue, out var s, out var q, out var t);
        return Matrix4x4.CreateScale(s.X, s.Z, s.Y)
             * Matrix4x4.CreateFromQuaternion(new Quaternion(q.X, q.Z, q.Y, -q.W))
             * Matrix4x4.CreateTranslation(t.X * 0.01f, t.Z * 0.01f, t.Y * 0.01f);
    }

    // Height grid (1 m cells) rasterised from the landscape vertices, so gameplay can
    // place units on the terrain without physics: ground.json { origin, cell, w, h, heights }.
    static void WriteGround(string path, List<(string glb, Matrix4x4 transform)> landscapes)
    {
        const float Cell = 1f;
        var points = new List<Vector3>();
        foreach (var (glb, m) in landscapes)
            foreach (var mesh in ModelRoot.Load(glb).LogicalMeshes)
                foreach (var prim in mesh.Primitives)
                    points.AddRange(prim.GetVertexAccessor("POSITION").AsVector3Array().Select(v => Vector3.Transform(v, m)));
        float minX = points.Min(p => p.X), minZ = points.Min(p => p.Z);
        int w = (int)Math.Ceiling((points.Max(p => p.X) - minX) / Cell) + 1;
        int h = (int)Math.Ceiling((points.Max(p => p.Z) - minZ) / Cell) + 1;
        var heights = new float?[w * h];
        foreach (var p in points)
        {
            int x = (int)Math.Round((p.X - minX) / Cell), z = (int)Math.Round((p.Z - minZ) / Cell);
            var i = z * w + x;
            heights[i] = heights[i] is { } old ? Math.Max(old, p.Y) : p.Y;
        }
        // Fill cells no vertex landed in from their neighbours. Gaps still open after two passes
        // are landscape holes (cellars, caves): they stay null so the game can block them.
        for (int pass = 0; pass < 2 && heights.Any(v => v == null); pass++)
            for (int z = 0; z < h; z++)
                for (int x = 0; x < w; x++)
                {
                    if (heights[z * w + x] != null) continue;
                    var n = new[] { (x - 1, z), (x + 1, z), (x, z - 1), (x, z + 1) }
                        .Where(c => c.Item1 >= 0 && c.Item1 < w && c.Item2 >= 0 && c.Item2 < h)
                        .Select(c => heights[c.Item2 * w + c.Item1]).Where(v => v != null).ToList();
                    if (n.Count > 0) heights[z * w + x] = n.Average();
                }
        File.WriteAllText(path, JsonConvert.SerializeObject(new
        {
            origin = new[] { minX, minZ },
            cell = Cell,
            w,
            h,
            heights = heights.Select(v => v is { } f ? (float?)MathF.Round(f, 2) : null),
        }));
        Console.WriteLine($"wrote {path}: {w}x{h} cells from {points.Count} landscape vertices");
    }

    /// Adds the placements of a level (and its level instances). False when the package
    /// itself could not be loaded.
    internal static bool CollectLevel(DefaultFileProvider provider, string mapPath, Matrix4x4 parent,
        List<Placement> placements, Dictionary<string, UObject> meshes, int depth)
    {
        if (depth > 8) return true;
        IEnumerable<UObject> exports;
        try { exports = provider.LoadPackage(mapPath).GetExports().ToList(); }
        catch (Exception ex) { Console.WriteLine($"  FAILED {mapPath}: {ex.Message}"); return false; }

        foreach (var e in exports)
        {
            try
            {
                switch (e)
                {
                    case UInstancedStaticMeshComponent ism:
                    {
                        var mesh = Mesh(ism, meshes);
                        if (mesh == null) break;
                        var comp = ToMatrix(ism.GetAbsoluteTransform()) * parent;
                        var collides = CollisionOn(ism);
                        var visible = Visible(ism, mesh);
                        if (visible == null || (visible == false && !collides)) break;
                        foreach (var inst in ism.GetInstances() ?? [])
                            placements.Add(new(mesh, ToMatrix(inst.TransformData) * comp, collides, visible.Value));
                        break;
                    }
                    case UStaticMeshComponent smc:
                    {
                        var mesh = Mesh(smc, meshes);
                        if (mesh == null) break;
                        var collides = CollisionOn(smc);
                        var visible = Visible(smc, mesh);
                        if (visible == null || (visible == false && !collides)) break;
                        placements.Add(new(mesh, ToMatrix(smc.GetAbsoluteTransform()) * parent, collides, visible.Value));
                        break;
                    }
                    case USceneComponent sc when e.ExportType == "LevelInstanceComponent":
                        break; // handled through the owning actor below
                    default:
                        if (e.ExportType is "Landscape" or "LandscapeProxy" or "LandscapeStreamingProxy")
                        {
                            // Landscape glb is in actor-local space (1 unit per quad); place by the actor root.
                            var root = e.GetOrDefault<FPackageIndex>("RootComponent")?.Load<USceneComponent>();
                            var path = e.GetPathName();
                            meshes.TryAdd(path, e);
                            placements.Add(new(path, (root != null ? ToMatrix(root.GetAbsoluteTransform()) : Matrix4x4.Identity) * parent));
                        }
                        else if (e.ExportType.StartsWith("LevelInstance") && e.TryGetValue(out FSoftObjectPath world, "WorldAsset"))
                        {
                            var root = e.GetOrDefault<FPackageIndex>("RootComponent")?.Load<USceneComponent>();
                            var m = (root != null ? ToMatrix(root.GetAbsoluteTransform()) : Matrix4x4.Identity) * parent;
                            var pkg = world.AssetPathName.Text.Split('.')[0];
                            var file = provider.FixPath(pkg + ".umap");
                            CollectLevel(provider, file, m, placements, meshes, depth + 1);
                        }
                        break;
                }
            }
            catch (Exception ex) { Console.WriteLine($"  skip {e.ExportType} {e.Name}: {ex.GetType().Name} {ex.Message}"); }
        }
        return true;
    }

    /// Editor and lighting helpers that never show up in game (shadow casters, light shafts,
    /// gizmo arrows, light actor proxies).
    static readonly string[] HelperMeshes = ["shadowcaster", "axis_arrows", "lightshaft", "lightactor", "foggate", "/vfx/", "unit_box"];
    /// Invisible blockers (engine shapes, dev meshes): kept for collision only.
    static readonly string[] BlockerMeshes = ["invisiblewall", "blockmesh", "engine/", "/dev/"];

    /// true = rendered, false = collision only, null = dropped.
    static bool? Visible(UObject component, string mesh)
    {
        var lower = mesh.ToLowerInvariant();
        if (HelperMeshes.Any(lower.Contains)) return null;
        if (component.GetOrDefault("bHiddenInGame", false) || !component.GetOrDefault("bVisible", true)) return false;
        return !BlockerMeshes.Any(lower.Contains);
    }

    static readonly string[] NoCollisionProfiles = ["NoCollision", "OverlapAll", "OverlapAllDynamic", "Trigger", "UI"];

    /// A component can switch its mesh's collision off (common for decorative foliage).
    static bool CollisionOn(UObject component)
    {
        var body = component.GetOrDefault<FStructFallback>("BodyInstance");
        if (body == null) return true;
        if (body.GetOrDefault<FName>("CollisionEnabled").Text == "ECollisionEnabled::NoCollision") return false;
        return !NoCollisionProfiles.Contains(body.GetOrDefault<FName>("CollisionProfileName").Text);
    }

    /// The mesh's simple collision as point sets in Bevy-space mesh coordinates; spheres and
    /// capsules carry a radius (the shape is the hull of the points inflated by it).
    static List<object> Colliders(UStaticMesh mesh)
    {
        var shapes = new List<object>();
        var body = mesh.BodySetup?.Load<UBodySetup>();
        if (body?.AggGeom == null) return shapes;
        var profile = body.GetOrDefault<FStructFallback>("DefaultInstance")?.GetOrDefault<FName>("CollisionProfileName").Text;
        if (profile != null && NoCollisionProfiles.Contains(profile)) return shapes;
        var agg = body.AggGeom;

        static float[] B(FVector v) => [(float)v.X * 0.01f, (float)v.Z * 0.01f, (float)v.Y * 0.01f];
        void Add(IEnumerable<FVector> points, float radius) =>
            shapes.Add(new { points = points.Select(B).ToArray(), radius = radius * 0.01f });

        foreach (var box in agg.BoxElems ?? [])
        {
            var q = box.Rotation.Quaternion();
            var h = new FVector(box.X, box.Y, box.Z) * 0.5f;
            Add(from sx in new[] { -1, 1 } from sy in new[] { -1, 1 } from sz in new[] { -1, 1 }
                select box.Center + q.RotateVector(new FVector(h.X * sx, h.Y * sy, h.Z * sz)), 0);
        }
        foreach (var sphere in agg.SphereElems ?? [])
            Add([sphere.Center], sphere.Radius);
        foreach (var capsule in agg.SphylElems ?? [])
        {
            // UE capsules run along their local Z axis.
            var axis = capsule.Rotation.Quaternion().RotateVector(new FVector(0, 0, 1)) * (capsule.Length * 0.5f);
            Add([capsule.Center + axis, capsule.Center - axis], capsule.Radius);
        }
        foreach (var convex in agg.ConvexElems ?? [])
            Add((convex.VertexData ?? []).Select(v => convex.Transform.TransformPosition(v)), 0);
        return shapes;
    }

    static string? Mesh(UStaticMeshComponent c, Dictionary<string, UObject> meshes)
    {
        var mesh = c.GetLoadedStaticMesh();
        if (mesh == null) return null;
        var path = mesh.GetPathName();
        meshes.TryAdd(path, mesh);
        return path;
    }

    // UE FTransform -> row-vector matrix (scale, then rotate, then translate), still in UE space.
    static Matrix4x4 ToMatrix(FTransform t) =>
        Matrix4x4.CreateScale((float)t.Scale3D.X, (float)t.Scale3D.Y, (float)t.Scale3D.Z)
        * Matrix4x4.CreateFromQuaternion(new Quaternion((float)t.Rotation.X, (float)t.Rotation.Y, (float)t.Rotation.Z, (float)t.Rotation.W))
        * Matrix4x4.CreateTranslation((float)t.Translation.X, (float)t.Translation.Y, (float)t.Translation.Z);
}
