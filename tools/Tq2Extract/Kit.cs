using System.Numerics;
using CUE4Parse.FileProvider;
using CUE4Parse.UE4.Assets.Exports;
using CUE4Parse.UE4.Assets.Exports.StaticMesh;
using Newtonsoft.Json.Linq;

// The building kits of generated arenas (game/src/mapgen.rs): every TQ2 mesh and ground texture
// named in tools/kits.json is exported once into cache/export/kit (meshes with materials and
// collision, textures/<set>_a/_n.png) together with a copy of kits.json for the game.
static class Kit
{
    public static async Task Run(DefaultFileProvider provider, string root, string cache)
    {
        var source = Path.Combine(root, "tools", "kits.json");
        var kits = JObject.Parse(File.ReadAllText(source));
        var names = new SortedSet<string>(StringComparer.OrdinalIgnoreCase);
        var textures = new SortedSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var (_, kit) in kits)
        {
            foreach (var role in new[] { "border", "cover", "clutter" })
                foreach (var piece in (JArray)kit![role]!)
                    names.Add(piece is JArray a ? (string)a[0]! : (string)piece!);
            foreach (var set in (JArray)kit!["ground"]!)
                textures.Add((string)set!);
        }

        // Package by mesh name; environment art first (the same name can exist in test maps).
        var byName = provider.Files.Keys
            .Where(k => k.EndsWith(".uasset") && Path.GetFileName(k).StartsWith("sm_", StringComparison.OrdinalIgnoreCase))
            .GroupBy(k => Path.GetFileNameWithoutExtension(k), StringComparer.OrdinalIgnoreCase)
            .ToDictionary(g => g.Key, g => g.OrderBy(k => k.Contains("/Environment/", StringComparison.OrdinalIgnoreCase) ? 0 : 1).First(), StringComparer.OrdinalIgnoreCase);
        var meshes = new Dictionary<string, UObject>();
        var placements = new List<SceneExport.Placement>();
        foreach (var name in names)
        {
            if (!byName.TryGetValue(name, out var path))
            {
                Console.WriteLine($"  kit: mesh {name} not found");
                continue;
            }
            try
            {
                var mesh = provider.LoadPackage(path).GetExports().OfType<UStaticMesh>().FirstOrDefault();
                if (mesh == null) { Console.WriteLine($"  kit: {path} has no static mesh"); continue; }
                meshes[mesh.GetPathName()] = mesh;
                placements.Add(new(mesh.GetPathName(), Matrix4x4.Identity));
            }
            catch (Exception ex) { Console.WriteLine($"  kit: {path}: {ex.Message}"); }
        }
        var outDir = Path.Combine(cache, "export", "kit");
        if (Directory.Exists(outDir)) Directory.Delete(outDir, true);
        await SceneExport.BuildKit(provider, cache, "kit", placements, meshes);

        var library = new MaterialLibrary(provider, outDir);
        foreach (var set in textures)
            foreach (var suffix in new[] { "_a", "_n" })
            {
                string? file = null;
                try { file = library.ExportByName(set + suffix); } catch (KeyNotFoundException) { }
                Console.WriteLine($"  ground {set}{suffix} -> {file ?? "NOT FOUND"}");
            }
        File.Copy(source, Path.Combine(outDir, "kits.json"), true);
        Console.WriteLine($"kit: {meshes.Count}/{names.Count} meshes, {textures.Count} ground sets");
    }
}
