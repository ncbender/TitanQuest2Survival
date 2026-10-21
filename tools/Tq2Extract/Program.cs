// Reads Titan Quest II assets straight from the local Steam install (read-only)
// and exports them into the git-ignored cache/ folder.
//
//   list                         write cache/filelist.txt
//   dump <package>...            print exports as JSON
//   export <name> <path|dir>...  export meshes/textures to cache/export/<name>
//   animate <name> <skeleton> <anims>...  bake animations into every skeletal mesh of an export
//   materials <name>             write materials.json + textures for an export
//   scene <name> <map.umap>      export a map (meshes, placements, materials, ground)
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
// Detex decodes BC6H/BC7 textures; CUE4Parse-Conversion ships it as an embedded DLL.
var detex = Path.Combine(cache, "Detex.dll");
if (!File.Exists(detex)) await CUE4Parse_Conversion.Textures.BC.DetexHelper.LoadDllAsync(detex);
CUE4Parse_Conversion.Textures.BC.DetexHelper.Initialize(detex);

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
        var options = new ExportOptions(EMeshFormat.Gltf2, exportMorphTargets: false);
        var session = new ExportSession(null!);
        foreach (var arg in args.Skip(2))
        {
            var prefix = arg.TrimEnd('/') + "/";
            var paths = provider.Files.Keys.Where(k => (k.EndsWith(".uasset") || k.EndsWith(".umap")) &&
                (k.Equals(arg, StringComparison.OrdinalIgnoreCase) || k.StartsWith(prefix, StringComparison.OrdinalIgnoreCase)));
            foreach (var path in paths)
            {
                try
                {
                    // Animations are not exported here: CUE4Parse has no glTF animation
                    // writer, so the "animate" command bakes them into the mesh .glb.
                    foreach (var e in provider.LoadPackage(path).GetExports())
                        if (e.ExportType is "SkeletalMesh" or "Texture2D" or "StaticMesh" or "World" or "Landscape" or "LandscapeStreamingProxy" or "LandscapeProxy")
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

    case "scene":
        // scene <name> <map.umap>
        await SceneExport.Run(provider, cache, args[1], args[2]);
        break;

    case "level":
    {
        // level <umap or dir> [max]: export-type histogram per map package
        int max = args.Length > 2 ? int.Parse(args[2]) : 20;
        var maps = provider.Files.Keys.Where(k => k.EndsWith(".umap") && k.StartsWith(args[1], StringComparison.OrdinalIgnoreCase)).Take(max);
        foreach (var path in maps)
        {
            try
            {
                var exports = provider.LoadPackage(path).GetExports().ToList();
                var hist = exports.GroupBy(e => e.ExportType).OrderByDescending(g => g.Count()).Take(8).Select(g => $"{g.Key}×{g.Count()}");
                Console.WriteLine($"{Path.GetFileName(path)}: {exports.Count} exports | {string.Join(", ", hist)}");
            }
            catch (Exception ex) { Console.WriteLine($"{Path.GetFileName(path)}: FAILED {ex.GetType().Name}: {ex.Message}"); }
        }
        break;
    }

    case "materials":
    {
        // materials <export name>: write materials.json for every mesh of an export
        var dir = Path.Combine(cache, "export", args[1]);
        var library = new MaterialLibrary(provider, dir);
        library.AddFromMeshes(Directory.EnumerateFiles(dir, "*.glb", SearchOption.AllDirectories));
        library.Save();
        break;
    }

    case "animate":
        // animate <export name> <skeleton pkg> <anim pkg or dir>...
        Animate.Run(provider, cache, args[1], args[2], args.Skip(3));
        break;

    default:
        Console.WriteLine("commands: list | dump <pkg>... | export <name> <path>... | animate <name> <skeleton> <anims>... | reflect <type>... | ctor <type>");
        return 1;
}
return 0;

static void Reflect(string command, string[] names)
{
    var asm = names.Any(n => n.StartsWith("core:")) ? typeof(DefaultFileProvider).Assembly : Assembly.Load("CUE4Parse-Conversion");
    names = names.Select(n => n.Replace("core:", "")).ToArray();
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
    foreach (var t in asm.GetTypes().Where(t => t.IsPublic && names.Any(a => t.Name == a || (a.EndsWith('.') && t.FullName!.Contains(a)))))
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
