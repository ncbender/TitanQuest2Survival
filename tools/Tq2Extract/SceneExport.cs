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
    internal record Placement(string Mesh, Matrix4x4 Ue, bool Collides = true);

    public static async Task Run(DefaultFileProvider provider, string cache, string name, string mapPath)
    {
        var outDir = Path.Combine(cache, "export", name);
        var placements = new List<Placement>();
        var meshes = new Dictionary<string, UObject>(); // static meshes and landscape actors
        CollectLevel(provider, mapPath, Matrix4x4.Identity, placements, meshes, 0);
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
            });
        }
        File.WriteAllText(Path.Combine(outDir, "scene.json"), JsonConvert.SerializeObject(new { map = mapPath, instances }));
        Console.WriteLine($"wrote {outDir}/scene.json: {instances.Count} instances, {meshFiles.Count} meshes");

        var colliders = meshes.Where(m => m.Value is UStaticMesh && meshFiles.ContainsKey(m.Key))
            .Select(m => (file: meshFiles[m.Key], shapes: Colliders((UStaticMesh)m.Value)))
            .Where(c => c.shapes.Count > 0).ToDictionary(c => c.file, c => c.shapes);
        File.WriteAllText(Path.Combine(outDir, "colliders.json"), JsonConvert.SerializeObject(colliders));
        Console.WriteLine($"wrote colliders.json: {colliders.Count}/{meshFiles.Count} meshes have collision, {placements.Count(p => !p.Collides)} placements opt out");

        var landscapes = placements.Where(p => meshes[p.Mesh] is not UStaticMesh && meshFiles.ContainsKey(p.Mesh))
            .Select(p => (Path.Combine(outDir, meshFiles[p.Mesh]), ToBevy(p.Ue))).ToList();
        if (landscapes.Count > 0) WriteGround(Path.Combine(outDir, "ground.json"), landscapes);

        var landscapeActors = meshes.Values.Where(m => m is not UStaticMesh).ToList();
        if (landscapeActors.Count > 0) await WriteTerrain(provider, materials, outDir, landscapeActors, meshFiles.Values.First(f => f.Contains("Landscape")));
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
    static async Task WriteTerrain(DefaultFileProvider provider, MaterialLibrary materials, string outDir, List<UObject> landscapes, string landscapeGlb)
    {
        var raw = Path.Combine(outDir, "raw_terrain");
        var session = new ExportSession(null!);
        foreach (var l in landscapes) session.Add(l);
        await session.RunAsync(raw, new ExportOptions(EMeshFormat.Gltf2, exportMorphTargets: false), null!, CancellationToken.None);

        var mapping = JsonConvert.DeserializeObject<Dictionary<string, string?>>(
            File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "terrain_layers.json")))!;
        Directory.CreateDirectory(Path.Combine(outDir, "terrain"));
        var layers = new List<object>();
        foreach (var weight in Directory.EnumerateFiles(raw, "ll_*.png", SearchOption.AllDirectories).OrderBy(f => f))
        {
            var layer = Path.GetFileNameWithoutExtension(weight)[3..];
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
            var rel = $"terrain/ll_{layer}.png";
            File.Copy(weight, Path.Combine(outDir, rel), true);
            layers.Add(new { name = layer, weight = rel, albedo, normal });
        }
        Directory.Delete(raw, true);
        var material = ModelRoot.Load(Path.Combine(outDir, landscapeGlb)).LogicalMaterials.FirstOrDefault()?.Name;
        // UE landscape materials tile ground textures every few metres; 1 repeat per 4 m.
        File.WriteAllText(Path.Combine(outDir, "terrain.json"), JsonConvert.SerializeObject(new { material, tiling = 0.25f, layers }, Formatting.Indented));
        Console.WriteLine($"wrote terrain.json: {layers.Count} textured layers for material {material}");
    }

    // UE-space matrix -> the same transform in Bevy space (what the game applies to the glb).
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
        // Fill cells no vertex landed in from their neighbours.
        for (int pass = 0; pass < 8 && heights.Any(v => v == null); pass++)
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
            heights = heights.Select(v => MathF.Round(v ?? 0, 2)),
        }));
        Console.WriteLine($"wrote {path}: {w}x{h} cells from {points.Count} landscape vertices");
    }

    internal static void CollectLevel(DefaultFileProvider provider, string mapPath, Matrix4x4 parent,
        List<Placement> placements, Dictionary<string, UObject> meshes, int depth)
    {
        if (depth > 8) return;
        IEnumerable<UObject> exports;
        try { exports = provider.LoadPackage(mapPath).GetExports().ToList(); }
        catch (Exception ex) { Console.WriteLine($"  FAILED {mapPath}: {ex.Message}"); return; }

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
                        foreach (var inst in ism.GetInstances() ?? [])
                            placements.Add(new(mesh, ToMatrix(inst.TransformData) * comp, collides));
                        break;
                    }
                    case UStaticMeshComponent smc:
                    {
                        var mesh = Mesh(smc, meshes);
                        if (mesh != null) placements.Add(new(mesh, ToMatrix(smc.GetAbsoluteTransform()) * parent, CollisionOn(smc)));
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
