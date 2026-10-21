using System.Numerics;
using CUE4Parse.FileProvider;
using CUE4Parse.UE4.Assets.Exports;
using Newtonsoft.Json;

// Survey of the Greece world partition: for every generated cell its centre and extent in
// metres (UE x/y), object count, whether it has landscape, and the asset folders it uses
// most (forest, ruins, beach, ...). Output: cache/cells.json. Used to pick arena regions.
static class Cells
{
    record Cell(string map, float x, float y, float minX, float minY, float maxX, float maxY, int objects, bool landscape, Dictionary<string, int> tags);

    public static void Run(DefaultFileProvider provider, string cache)
    {
        var maps = provider.Files.Keys.Where(k => k.Contains("/l_gr_p/_Generated_/") && k.EndsWith(".umap")).OrderBy(k => k).ToList();
        var cells = new List<Cell>();
        int done = 0;
        Parallel.ForEach(maps, new ParallelOptions { MaxDegreeOfParallelism = 6 }, map =>
        {
            var placements = new List<SceneExport.Placement>();
            var meshes = new Dictionary<string, UObject>();
            try { SceneExport.CollectLevel(provider, map, Matrix4x4.Identity, placements, meshes, 0); }
            catch { }
            if (Interlocked.Increment(ref done) % 500 == 0) Console.WriteLine($"  {done}/{maps.Count}");
            var objects = placements.Where(p => !meshes.TryGetValue(p.Mesh, out var m) || m is not null && !m.ExportType.StartsWith("Landscape")).ToList();
            if (objects.Count == 0 && placements.Count == 0) return;
            var pts = placements.Select(p => new Vector2(p.Ue.M41, p.Ue.M42) / 100f).ToList();
            var tags = new Dictionary<string, int>();
            foreach (var p in objects)
            {
                // ".../Environment/<Culture>/<Group>/<Sub>/..." -> "culture/group/sub"
                var parts = p.Mesh.Split('/');
                var i = Array.FindIndex(parts, s => s.Equals("Environment", StringComparison.OrdinalIgnoreCase));
                if (i < 0 || i + 3 >= parts.Length) continue;
                var tag = string.Join('/', parts[(i + 1)..(i + 4)]).ToLowerInvariant();
                tags[tag] = tags.GetValueOrDefault(tag) + 1;
            }
            var cell = new Cell(Path.GetFileName(map), pts.Average(p => p.X), pts.Average(p => p.Y), pts.Min(p => p.X), pts.Min(p => p.Y), pts.Max(p => p.X), pts.Max(p => p.Y),
                objects.Count, placements.Count > objects.Count, tags.OrderByDescending(t => t.Value).Take(6).ToDictionary());
            lock (cells) cells.Add(cell);
        });
        File.WriteAllText(Path.Combine(cache, "cells.json"), JsonConvert.SerializeObject(cells.OrderBy(c => c.x).ThenBy(c => c.y), Formatting.Indented));
        Console.WriteLine($"cells.json: {cells.Count} cells");
    }
}
