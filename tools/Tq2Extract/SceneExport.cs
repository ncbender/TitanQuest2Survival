using System.Numerics;
using CUE4Parse.FileProvider;
using CUE4Parse.UE4.Assets.Exports;
using CUE4Parse.UE4.Assets.Exports.Component;
using CUE4Parse.UE4.Assets.Exports.Component.StaticMesh;
using CUE4Parse.UE4.Assets.Exports.StaticMesh;
using CUE4Parse.UE4.Objects.Core.Math;
using CUE4Parse.UE4.Objects.UObject;
using CUE4Parse_Conversion;
using CUE4Parse_Conversion.Options;
using Newtonsoft.Json;
using SharpGLTF.Schema2;

// Exports a map as one .glb per unique static mesh plus scene.json listing every
// placement (static mesh components, instanced/foliage instances, level instances)
// already converted to Bevy space (Y-up, right-handed, metres).
static class SceneExport
{
    record Placement(string Mesh, Matrix4x4 Ue);

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
        var results = await session.RunAsync(rawDir, new ExportOptions(EMeshFormat.Gltf2, exportMaterials: false), null!, CancellationToken.None);
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
            });
        }
        File.WriteAllText(Path.Combine(outDir, "scene.json"), JsonConvert.SerializeObject(new { map = mapPath, instances }));
        Console.WriteLine($"wrote {outDir}/scene.json: {instances.Count} instances, {meshFiles.Count} meshes");
    }

    static void CollectLevel(DefaultFileProvider provider, string mapPath, Matrix4x4 parent,
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
                        foreach (var inst in ism.GetInstances() ?? [])
                            placements.Add(new(mesh, ToMatrix(inst.TransformData) * comp));
                        break;
                    }
                    case UStaticMeshComponent smc:
                    {
                        var mesh = Mesh(smc, meshes);
                        if (mesh != null) placements.Add(new(mesh, ToMatrix(smc.GetAbsoluteTransform()) * parent));
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
