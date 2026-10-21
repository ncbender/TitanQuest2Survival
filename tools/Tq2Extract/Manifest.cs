using CUE4Parse.FileProvider;
using Newtonsoft.Json;

// Builds every asset the game uses from tools/assets.json, so a fresh checkout (or a
// friend's PC) needs one command: `dotnet run -- assets` (optionally only some names).
static class Manifest
{
    record Character(string[] parts, string skeleton, string[] anims, string[]? clips);
    record File(Dictionary<string, Character> characters, Dictionary<string, string[]> props, Dictionary<string, string> scenes,
                Dictionary<string, string>? fonts, Dictionary<string, string[]>? textures);

    public static async Task Run(DefaultFileProvider provider, string root, string cache, string[] only)
    {
        var manifest = JsonConvert.DeserializeObject<File>(System.IO.File.ReadAllText(Path.Combine(root, "tools", "assets.json")))!;
        bool Wanted(string name) => only.Length == 0 || only.Contains(name);

        foreach (var (name, c) in manifest.characters.Where(c => Wanted(c.Key)))
        {
            Console.WriteLine($"== character {name}");
            var dir = Path.Combine(cache, "export", name);
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
            await Export.Run(provider, cache, name, c.parts);
            Animate.Run(provider, cache, name, c.skeleton, c.anims, c.clips);
        }
        foreach (var (name, paths) in manifest.props.Where(p => Wanted(p.Key)))
        {
            Console.WriteLine($"== props {name}");
            var dir = Path.Combine(cache, "export", name);
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
            await Export.Run(provider, cache, name, paths);
            var library = new MaterialLibrary(provider, dir);
            library.AddFromMeshes(Directory.EnumerateFiles(dir, "*.glb", SearchOption.AllDirectories));
            library.Save();
        }
        // TQ2's UI fonts -> cache/export/fonts/<name>.ttf. A .ufont is the TrueType file behind an
        // int32 length prefix.
        foreach (var (name, path) in (manifest.fonts ?? []).Where(f => Wanted("fonts") || Wanted(f.Key)))
        {
            var dir = Path.Combine(cache, "export", "fonts");
            Directory.CreateDirectory(dir);
            var raw = provider.Files[path].Read();
            var length = BitConverter.ToInt32(raw, 0);
            System.IO.File.WriteAllBytes(Path.Combine(dir, name + ".ttf"), raw.AsSpan(4, Math.Min(length, raw.Length - 4)).ToArray());
            Console.WriteLine($"font {name} <- {path}");
        }
        // Loose textures (e.g. TQ2 VFX flipbooks) -> cache/export/<name>/textures/<texture>.png
        foreach (var (name, names) in (manifest.textures ?? []).Where(t => Wanted(t.Key)))
        {
            var library = new MaterialLibrary(provider, Path.Combine(cache, "export", name));
            foreach (var texture in names)
                Console.WriteLine($"texture {texture} -> {library.ExportByName(texture) ?? "NOT FOUND"}");
        }
        foreach (var (name, map) in manifest.scenes.Where(s => Wanted(s.Key)))
        {
            Console.WriteLine($"== scene {name}");
            await SceneExport.Run(provider, cache, name, map);
        }
    }
}
