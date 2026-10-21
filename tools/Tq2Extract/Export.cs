using CUE4Parse.FileProvider;
using CUE4Parse_Conversion;
using CUE4Parse_Conversion.Options;

// Exports meshes and textures of packages (or whole folders) with CUE4Parse into
// cache/export/<name>. Animations are not exported here: CUE4Parse has no glTF animation
// writer, so Animate bakes them into the mesh .glb files afterwards.
static class Export
{
    public static async Task Run(DefaultFileProvider provider, string cache, string name, IEnumerable<string> paths)
    {
        var outDir = Path.Combine(cache, "export", name);
        // Morph targets are skipped: Bevy's glTF loader rejects their data-less accessors.
        var options = new ExportOptions(EMeshFormat.Gltf2, exportMorphTargets: false);
        var session = new ExportSession(null!);
        foreach (var path in Resolve(provider, paths, ".uasset", ".umap"))
        {
            try
            {
                foreach (var e in provider.LoadPackage(path).GetExports())
                    if (e.ExportType is "SkeletalMesh" or "Texture2D" or "StaticMesh" or "World" or "Landscape" or "LandscapeStreamingProxy" or "LandscapeProxy")
                        session.Add(e);
            }
            catch (Exception ex) { Console.WriteLine($"  FAILED {path}: {ex.GetType().Name}: {ex.Message}"); }
        }
        var results = await session.RunAsync(outDir, options, null!, CancellationToken.None);
        foreach (var r in results)
            Console.WriteLine(r.Success
                ? $"ok   {r.ObjectPath} -> {string.Join(", ", (r.DiskFilePaths ?? []).Select(Path.GetFileName))}"
                : $"FAIL {r.ObjectPath}: {r.Error?.GetType().Name} {r.Error?.Message}");
    }

    /// Package paths matching each argument exactly or as a folder prefix.
    public static IEnumerable<string> Resolve(DefaultFileProvider provider, IEnumerable<string> args, params string[] extensions) =>
        args.SelectMany(arg => provider.Files.Keys.Where(k => extensions.Any(e => k.EndsWith(e)) &&
            (k.Equals(arg, StringComparison.OrdinalIgnoreCase) || k.StartsWith(arg.TrimEnd('/') + "/", StringComparison.OrdinalIgnoreCase))));
}
