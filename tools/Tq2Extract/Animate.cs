using System.Numerics;
using CUE4Parse.FileProvider;
using CUE4Parse.UE4.Assets.Exports.Animation;
using CUE4Parse.UE4.Objects.Core.Math;
using CUE4Parse_Conversion.Animations;
using SharpGLTF.Schema2;

// CUE4Parse exports skeletal meshes to glTF but animations only to psa/ueanim/usd.
// Bake: decode every AnimSequence once, sample it per frame, and write the tracks as glTF
// animations into each skeletal mesh of an export (body and modular parts share the clips,
// so the game can play them in lockstep).
static class Animate
{
    record Track(string Bone, Dictionary<float, Quaternion> Rot, Dictionary<float, Vector3> Pos, Dictionary<float, Vector3> Scale);
    record Clip(string Name, List<Track> Tracks);

    public static void Run(DefaultFileProvider provider, string cache, string name, string skeletonPath, IEnumerable<string> animArgs)
    {
        var exportDir = Path.Combine(cache, "export", name);
        var skeleton = provider.LoadPackage(skeletonPath).GetExports().OfType<USkeleton>().First();
        var boneNames = skeleton.ReferenceSkeleton.FinalRefBoneInfo.Select(b => b.Name.Text).ToArray();

        var clips = new List<Clip>();
        var animPaths = animArgs.SelectMany(arg => provider.Files.Keys.Where(k => k.EndsWith(".uasset") &&
            (k.Equals(arg, StringComparison.OrdinalIgnoreCase) || k.StartsWith(arg.TrimEnd('/') + "/", StringComparison.OrdinalIgnoreCase))));
        foreach (var path in animPaths)
        {
            try
            {
                var seq = provider.LoadPackage(path).GetExports().OfType<UAnimSequence>().FirstOrDefault();
                if (seq == null) continue;
                var anim = skeleton.ConvertAnims(seq).Sequences[0];
                if (anim.IsAdditive) { Console.WriteLine($"  skip additive {anim.Name}"); continue; }
                var tracks = new List<Track>();
                for (int bone = 0; bone < anim.Tracks.Count && bone < boneNames.Length; bone++)
                {
                    var track = anim.Tracks[bone];
                    if (!track.HasKeys()) continue;
                    var t = new Track(boneNames[bone], new(), new(), new());
                    for (int frame = 0; frame < Math.Max(1, anim.NumFrames); frame++)
                    {
                        FQuat q = FQuat.Identity; FVector p = FVector.ZeroVector, s = FVector.OneVector;
                        track.GetBoneTransform(frame, anim.NumFrames, ref q, ref p, ref s);
                        float time = frame / anim.FramesPerSecond;
                        // Same UE (Z-up, LH, cm) -> glTF (Y-up, RH, m) mapping as CUE4Parse's Gltf writer.
                        t.Rot[time] = Quaternion.Normalize(new(q.X, q.Z, q.Y, -q.W));
                        t.Pos[time] = new(p.X * 0.01f, p.Z * 0.01f, p.Y * 0.01f);
                        t.Scale[time] = new(s.X, s.Z, s.Y);
                    }
                    tracks.Add(t);
                }
                clips.Add(new(seq.Name, tracks));
                Console.WriteLine($"  {seq.Name}: {anim.NumFrames} frames @ {anim.FramesPerSecond:0.#} fps, {tracks.Count} bones");
            }
            catch (Exception ex) { Console.WriteLine($"  FAILED {Path.GetFileNameWithoutExtension(path)}: {ex.GetType().Name}: {ex.Message}"); }
        }
        Console.WriteLine($"skeleton {skeleton.Name}: {boneNames.Length} bones, {clips.Count} clips");

        // Every exported skeletal mesh (has a skin) gets the clips; static meshes are left alone.
        var meshes = Directory.EnumerateFiles(Path.Combine(exportDir, "TQ2"), "*.glb", SearchOption.AllDirectories).ToList();
        foreach (var glb in meshes)
        {
            var model = ModelRoot.Load(glb);
            if (model.LogicalSkins.Count == 0) continue;
            var nodes = model.LogicalNodes.Where(n => n.Name != null).GroupBy(n => n.Name).ToDictionary(g => g.Key, g => g.First());
            foreach (var clip in clips)
            {
                var gltfAnim = model.CreateAnimation(clip.Name);
                foreach (var t in clip.Tracks)
                {
                    if (!nodes.TryGetValue(t.Bone, out var node)) continue;
                    gltfAnim.CreateRotationChannel(node, t.Rot);
                    gltfAnim.CreateTranslationChannel(node, t.Pos);
                    gltfAnim.CreateScaleChannel(node, t.Scale);
                }
            }
            var outGlb = Path.Combine(exportDir, Path.GetFileNameWithoutExtension(glb) + "_animated.glb");
            model.SaveGLB(outGlb);
            Console.WriteLine($"wrote {outGlb}: {nodes.Count(n => clips.Any(c => c.Tracks.Any(t => t.Bone == n.Key)))} animated bones");
        }

        var library = new MaterialLibrary(provider, exportDir);
        library.AddFromMeshes(meshes);
        library.Save();
    }
}
