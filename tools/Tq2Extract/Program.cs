// Reads Titan Quest II assets straight from the local Steam install (read-only)
// and exports them into the git-ignored cache/ folder.
//
//   assets [name...]             build all game assets listed in tools/assets.json
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
using CUE4Parse.UE4.Versions;
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

    case "affixes":
        // affixes: TQ2 item prefixes/suffixes (names, texts, drop weights per slot)
        Affixes.Run(provider, cache);
        break;

    case "region":
        // region <name> <x> <y> <radius>: a square of the Greece world (UE metres) as a scene
        await SceneExport.RunRegion(provider, cache, args[1], float.Parse(args[2], System.Globalization.CultureInfo.InvariantCulture),
            float.Parse(args[3], System.Globalization.CultureInfo.InvariantCulture), float.Parse(args[4], System.Globalization.CultureInfo.InvariantCulture));
        break;

    case "cells":
        // cells: survey of the Greece world partition cells (position, content) -> cache/cells.json
        Cells.Run(provider, cache);
        break;

    case "masteries":
        // masteries: skill-tree data (tiers, skills, passives, modifiers, texts, icons)
        Masteries.Run(provider, cache);
        break;

    case "dumpfile":
    {
        // dumpfile <pkg|dir>...: full JSON of every export into cache/dump/<package>.json, plus
        // cache/dump/locres_en.json (English texts by namespace and key) to resolve FTexts
        var outDir = Path.Combine(cache, "dump");
        Directory.CreateDirectory(outDir);
        var locres = new CUE4Parse.UE4.Localization.FTextLocalizationResource(provider.CreateReader("TQ2/Content/Localization/TQ2/en/TQ2.locres"));
        var texts = locres.Entries.ToDictionary(ns => ns.Key.Str, ns => ns.Value.ToDictionary(k => k.Key.Str, k => k.Value.LocalizedString));
        File.WriteAllText(Path.Combine(outDir, "locres_en.json"), JsonConvert.SerializeObject(texts, Formatting.Indented));
        var packages = args.Skip(1).SelectMany(a => a.EndsWith(".uasset")
            ? [a]
            : provider.Files.Keys.Where(k => k.EndsWith(".uasset") && k.StartsWith(a, StringComparison.OrdinalIgnoreCase)));
        foreach (var path in packages)
        {
            try
            {
                var json = JsonConvert.SerializeObject(provider.LoadPackage(path).GetExports(), Formatting.Indented);
                File.WriteAllText(Path.Combine(outDir, Path.GetFileNameWithoutExtension(path) + ".json"), json);
            }
            catch (Exception ex) { Console.WriteLine($"{path}: FAILED {ex.GetType().Name}: {ex.Message}"); }
        }
        Console.WriteLine($"wrote {outDir}");
        break;
    }

    case "export":
        // export <name> <path|dir>...
        await Export.Run(provider, cache, args[1], args.Skip(2));
        break;

    case "assets":
        // assets [name...]: build everything in tools/assets.json (or only the named entries)
        await Manifest.Run(provider, root, cache, args.Skip(1).ToArray());
        break;

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

    case "reftex":
        // reftex <material package>: textures a root material references directly
        foreach (var m in provider.LoadPackage(args[1]).GetExports().OfType<CUE4Parse.UE4.Assets.Exports.Material.UMaterial>())
            foreach (var idx in m.ReferencedTextures ?? [])
                Console.WriteLine($"  {idx.Name}");
        break;

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
        Animate.Run(provider, cache, args[1], args[2], args.Skip(3), null);
        break;

    default:
        Console.WriteLine("commands: assets [name...] | list | dump <pkg>... | export <name> <path>... | animate <name> <skeleton> <anims>... | materials <name> | scene <name> <map> | level <dir> | reflect <type>... | ctor <type>");
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
