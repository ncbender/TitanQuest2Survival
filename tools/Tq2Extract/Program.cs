// Reads Titan Quest II assets straight from the local Steam install (read-only)
// and exports them into the git-ignored cache/ folder.
//
//   list                         write cache/filelist.txt
//   dump <package>...            print exports as JSON
//   export <name> <path|dir>...  export meshes/skeletons/anims/textures to cache/export/<name>
//   reflect <TypeName|Ns.>...    print CUE4Parse-Conversion API (development aid)
//   ctor <FullTypeName>          print constructor parameters (development aid)
using System.Reflection;
using CUE4Parse.Compression;
using CUE4Parse.FileProvider;
using CUE4Parse.MappingsProvider.Usmap;
using CUE4Parse.UE4.Assets.Exports.Animation;
using CUE4Parse.UE4.Objects.Core.Math;
using CUE4Parse.UE4.Versions;
using CUE4Parse_Conversion;
using CUE4Parse_Conversion.Animations;
using CUE4Parse_Conversion.Options;
using Newtonsoft.Json;

const string GameDir = @"C:\Program Files (x86)\Steam\steamapps\common\Titan Quest II\TQ2\Content\Paks";
var root = FindRoot();
var cache = Path.Combine(root, "cache");
Directory.CreateDirectory(cache);

var command = args.FirstOrDefault();
if (command is "reflect" or "ctor")
{
    Reflect(command, args.Skip(1).ToArray());
    return 0;
}

var oodle = Path.Combine(cache, OodleHelper.OodleFileName);
if (!File.Exists(oodle)) OodleHelper.DownloadOodleDll(ref oodle);
OodleHelper.Initialize(oodle);

var provider = new DefaultFileProvider(GameDir, SearchOption.TopDirectoryOnly, new VersionContainer(EGame.GAME_TitanQuest2));
var usmap = Path.Combine(cache, "Mappings.usmap");
if (File.Exists(usmap)) provider.MappingsContainer = new FileUsmapTypeMappingsProvider(usmap);
else Console.WriteLine("warning: cache/Mappings.usmap missing, packages cannot be parsed (see tools/UsmapDumper)");
provider.Initialize();
provider.Mount();
Console.WriteLine($"mounted {provider.MountedVfs.Count} containers, {provider.Files.Count} files");

switch (command)
{
    case "list":
        var listFile = Path.Combine(cache, "filelist.txt");
        File.WriteAllLines(listFile, provider.Files.Keys.OrderBy(k => k));
        Console.WriteLine($"wrote {listFile}");
        break;

    case "dump":
        foreach (var path in args.Skip(1))
            foreach (var e in provider.LoadPackage(path).GetExports())
            {
                var json = JsonConvert.SerializeObject(e, Formatting.Indented);
                Console.WriteLine(json.Length > 6000 ? json[..6000] + "\n  ..." : json);
            }
        break;

    case "export":
    {
        var outDir = Path.Combine(cache, "export", args[1]);
        var options = new ExportOptions(EMeshFormat.Gltf2);
        var session = new ExportSession(null!);
        foreach (var arg in args.Skip(2))
        {
            var prefix = arg.TrimEnd('/') + "/";
            var paths = provider.Files.Keys.Where(k => k.EndsWith(".uasset") &&
                (k.Equals(arg, StringComparison.OrdinalIgnoreCase) || k.StartsWith(prefix, StringComparison.OrdinalIgnoreCase)));
            foreach (var path in paths)
            {
                try
                {
                    // Animations are not exported here: CUE4Parse has no glTF animation
                    // writer, so the "animate" command bakes them into the mesh .glb.
                    foreach (var e in provider.LoadPackage(path).GetExports())
                        if (e.ExportType is "SkeletalMesh" or "Texture2D" or "StaticMesh")
                            session.Add(e);
                }
                catch (Exception ex) { Console.WriteLine($"  FAILED {path}: {ex.GetType().Name}: {ex.Message}"); }
            }
        }
        var results = await session.RunAsync(outDir, options, null!, CancellationToken.None);
        foreach (var r in results)
            Console.WriteLine(r.Success
                ? $"ok   {r.ObjectPath} -> {string.Join(", ", (r.DiskFilePaths ?? []).Select(Path.GetFileName))}"
                : $"FAIL {r.ObjectPath}: {r.Error?.GetType().Name} {r.Error?.Message}");
        break;
    }

    case "animate":
    {
        // animate <mesh.glb relative to cache/export> <out.glb> <skeleton pkg> <anim pkg or dir>...
        var model = SharpGLTF.Schema2.ModelRoot.Load(Path.Combine(cache, "export", args[1]));
        var nodes = model.LogicalNodes.GroupBy(n => n.Name ?? "").ToDictionary(g => g.Key, g => g.First());
        var skeleton = provider.LoadPackage(args[3]).GetExports().OfType<USkeleton>().First();
        var boneNames = skeleton.ReferenceSkeleton.FinalRefBoneInfo.Select(b => b.Name.Text).ToArray();
        Console.WriteLine($"skeleton {skeleton.Name}: {boneNames.Length} bones, {boneNames.Count(nodes.ContainsKey)} matched in mesh");

        var animPaths = args.Skip(4).SelectMany(arg => provider.Files.Keys.Where(k => k.EndsWith(".uasset") &&
            (k.Equals(arg, StringComparison.OrdinalIgnoreCase) || k.StartsWith(arg.TrimEnd('/') + "/", StringComparison.OrdinalIgnoreCase))));
        foreach (var path in animPaths)
        {
            UAnimSequence? seq;
            try { seq = provider.LoadPackage(path).GetExports().OfType<UAnimSequence>().FirstOrDefault(); }
            catch (Exception ex) { Console.WriteLine($"  FAILED {path}: {ex.Message}"); continue; }
            if (seq == null) continue;
            try
            {
                var anim = skeleton.ConvertAnims(seq).Sequences[0];
                if (anim.IsAdditive) { Console.WriteLine($"  skip additive {anim.Name}"); continue; }
                var gltfAnim = model.CreateAnimation(seq.Name);
                int channels = 0;
                for (int bone = 0; bone < anim.Tracks.Count && bone < boneNames.Length; bone++)
                {
                    var track = anim.Tracks[bone];
                    if (!track.HasKeys() || !nodes.TryGetValue(boneNames[bone], out var node)) continue;
                    var rot = new Dictionary<float, System.Numerics.Quaternion>();
                    var pos = new Dictionary<float, System.Numerics.Vector3>();
                    var scl = new Dictionary<float, System.Numerics.Vector3>();
                    for (int frame = 0; frame < Math.Max(1, anim.NumFrames); frame++)
                    {
                        FQuat q = FQuat.Identity; FVector p = FVector.ZeroVector, s = FVector.OneVector;
                        track.GetBoneTransform(frame, anim.NumFrames, ref q, ref p, ref s);
                        float t = frame / anim.FramesPerSecond;
                        // Same UE (Z-up, LH, cm) -> glTF (Y-up, RH, m) mapping as CUE4Parse's Gltf writer.
                        rot[t] = System.Numerics.Quaternion.Normalize(new(q.X, q.Z, q.Y, -q.W));
                        pos[t] = new(p.X * 0.01f, p.Z * 0.01f, p.Y * 0.01f);
                        scl[t] = new(s.X, s.Z, s.Y);
                    }
                    gltfAnim.CreateRotationChannel(node, rot);
                    gltfAnim.CreateTranslationChannel(node, pos);
                    gltfAnim.CreateScaleChannel(node, scl);
                    channels++;
                }
                Console.WriteLine($"  {seq.Name}: {anim.NumFrames} frames @ {anim.FramesPerSecond} fps, {channels} bones");
            }
            catch (Exception ex) { Console.WriteLine($"  FAILED {seq.Name}: {ex.GetType().Name}: {ex.Message}"); }
        }
        BindMaterialTextures(model, Path.Combine(cache, "export", args[1].Split('/')[0]));
        var outGlb = Path.Combine(cache, "export", args[2]);
        model.SaveGLB(outGlb);
        Console.WriteLine($"wrote {outGlb} with {model.LogicalAnimations.Count} animations");
        break;
    }

    default:
        Console.WriteLine("commands: list | dump <pkg>... | export <name> <path>... | animate <mesh.glb> <out.glb> <skeleton> <anims>... | reflect <type>... | ctor <type>");
        return 1;
}
return 0;

// CUE4Parse writes glTF materials by name only and the parameters to <material>.json.
// Embed the original TQ2 textures: Albedo -> base color, Normal -> normal,
// ORM (R=AO, G=roughness, B=metal, same packing as glTF) -> metallicRoughness + occlusion.
static void BindMaterialTextures(SharpGLTF.Schema2.ModelRoot model, string exportDir)
{
    var files = Directory.EnumerateFiles(exportDir, "*.*", SearchOption.AllDirectories)
        .GroupBy(f => Path.GetFileName(f).ToLowerInvariant()).ToDictionary(g => g.Key, g => g.First());
    foreach (var mat in model.LogicalMaterials)
    {
        if (mat.Name == null || !files.TryGetValue(mat.Name.ToLowerInvariant() + ".json", out var jsonPath)) continue;
        var json = Newtonsoft.Json.Linq.JObject.Parse(File.ReadAllText(jsonPath));
        string? Tex(params string[] keys)
        {
            foreach (var k in keys)
            {
                var name = (string?)json["Textures"]?[k]?["ObjectName"];
                if (name == null) continue;
                var file = name.Split('\'')[1].ToLowerInvariant() + ".png";
                if (files.TryGetValue(file, out var path)) return path;
            }
            return null;
        }
        var albedo = Tex("Albedo", "PM_Diffuse");
        var normal = Tex("Normal", "PM_Normals");
        var orm = Tex("ORM", "PM_SpecularMasks");
        mat.InitializePBRMetallicRoughness();
        if (albedo != null) mat.FindChannel("BaseColor")!.Value.SetTexture(0, model.UseImage(new SharpGLTF.Memory.MemoryImage(albedo)));
        if (normal != null) mat.FindChannel("Normal")!.Value.SetTexture(0, model.UseImage(new SharpGLTF.Memory.MemoryImage(normal)));
        if (orm != null)
        {
            var img = model.UseImage(new SharpGLTF.Memory.MemoryImage(orm));
            mat.FindChannel("MetallicRoughness")!.Value.SetTexture(0, img);
            mat.FindChannel("MetallicRoughness")!.Value.SetFactor("MetallicFactor", 1);
            mat.FindChannel("MetallicRoughness")!.Value.SetFactor("RoughnessFactor", 1);
            mat.FindChannel("Occlusion")!.Value.SetTexture(0, img);
        }
        if ((string?)json["Properties"]?["BasePropertyOverrides"]?["BlendMode"] == "EBlendMode::BLEND_Masked")
        {
            mat.Alpha = SharpGLTF.Schema2.AlphaMode.MASK;
            mat.AlphaCutoff = (float?)json["Properties"]?["BasePropertyOverrides"]?["OpacityMaskClipValue"] ?? 0.333f;
        }
        Console.WriteLine($"  material {mat.Name}: albedo={albedo != null} normal={normal != null} orm={orm != null} alpha={mat.Alpha}");
    }
}

static void Reflect(string command, string[] names)
{
    var asm = Assembly.Load("CUE4Parse-Conversion");
    if (command == "ctor")
    {
        foreach (var c in asm.GetType(names[0])!.GetConstructors())
        {
            Console.WriteLine("ctor:");
            foreach (var p in c.GetParameters())
                Console.WriteLine($"  {p.ParameterType.Name} {p.Name} = {(p.HasDefaultValue ? p.DefaultValue : "<required>")}");
        }
        return;
    }
    foreach (var t in asm.GetTypes().Where(t => t.IsPublic && names.Any(a => t.Name == a || (a.EndsWith('.') && t.FullName!.StartsWith("CUE4Parse_Conversion." + a)))))
    {
        Console.WriteLine($"== {t.FullName}");
        if (t.IsEnum) { Console.WriteLine("  " + string.Join(", ", Enum.GetNames(t))); continue; }
        foreach (var p in t.GetProperties()) Console.WriteLine($"  prop {p.PropertyType.Name} {p.Name} {(p.CanWrite ? "rw" : "r")}");
        foreach (var f in t.GetFields(BindingFlags.Public | BindingFlags.Instance)) Console.WriteLine($"  field {f.FieldType.Name} {f.Name}");
        foreach (var m in t.GetMethods(BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly).Where(m => !m.IsSpecialName))
            Console.WriteLine("  " + m);
    }
}

static string FindRoot()
{
    var d = new DirectoryInfo(AppContext.BaseDirectory);
    while (d != null && !Directory.Exists(Path.Combine(d.FullName, ".git"))) d = d.Parent;
    return d?.FullName ?? Directory.GetCurrentDirectory();
}
