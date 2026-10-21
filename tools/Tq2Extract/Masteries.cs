using CUE4Parse.FileProvider;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

// Exports TQ2's masteries for the skill tree: every mastery's tiers with their active skills,
// passives (with feats) and modifiers, each with its original name, description and icon.
//
// Structure comes from the mastery data assets (TQ2/Saved/JsonDataCache/.../da_mastery_*),
// the texts from the English string tables the game ships as loose CSV files
// (TQ2/Content/TQ2/Data/Text/<table>.csv, read-only), icons are exported as PNG.
// Output: cache/export/masteries/masteries.json + textures/*.png
static class Masteries
{
    const string TextDir = @"C:\Program Files (x86)\Steam\steamapps\common\Titan Quest II\TQ2\Content\TQ2\Data\Text";
    static readonly string[] Ids = ["storm", "earth", "warfare", "rogue", "spirit", "forge"];

    record Ability(string id, string group, string name, string description, string? icon, bool wip);
    record Feat(string name, string description);
    record Passive(string id, string name, string description, string? icon, int max_level, List<Feat> feats);
    record Tier(List<Ability> actives, List<Passive> passives, List<Ability> modifiers);
    record Mastery(string id, string name, string slogan, string description, string color, string? icon, string? background, List<Tier> tiers);

    public static void Run(DefaultFileProvider provider, string cache)
    {
        var dir = Path.Combine(cache, "export", "masteries");
        // Clear the contents (not the folder itself, which a shell may have open).
        if (Directory.Exists(dir))
            foreach (var f in Directory.EnumerateFiles(dir, "*", SearchOption.AllDirectories)) File.Delete(f);
        var textures = new MaterialLibrary(provider, dir);
        var reader = new Reader(provider);
        var masteries = new List<Mastery>();
        foreach (var id in Ids)
        {
            var exports = reader.Exports($"TQ2/Saved/JsonDataCache/Game/TQ2/masteries/da_mastery_{id}.uasset");
            var main = exports.First(e => (string?)e["Type"] == "TQ2MasteryDescription")["Properties"]!;
            JToken Inner(JToken reference) => exports.First(e => reference["ObjectName"]!.ToString().EndsWith(":" + Outer(e) + (string)e["Name"]! + "'"));

            var tiers = new List<Tier>();
            foreach (var tierRef in main["m_Tiers"]!)
            {
                var tier = Inner(tierRef)["Properties"]!;
                var actives = new List<Ability>();
                foreach (var a in tier["m_ActiveAbilities"] ?? new JArray())
                {
                    var p = Inner(a)["Properties"]!;
                    var wip = p["m_RequiredEntitlements"]?.Any() == true;
                    var ability = reader.Describe(reader.Resolve((string)p["pAbility"]!["ObjectPath"]!), textures);
                    if (ability != null) actives.Add(ability with { wip = wip });
                }
                var passives = new List<Passive>();
                foreach (var a in tier["m_PassiveAbilities"] ?? new JArray())
                {
                    var p = Inner(a)["Properties"]!;
                    var info = reader.Describe((string)p["m_pPassiveAbility"]!["ObjectPath"]!, textures);
                    if (info == null) continue;
                    var feats = (p["m_SelectableFeatsNew"] ?? new JArray())
                        .Select(f => reader.Describe((string)f["m_pMainEffect"]!["ObjectPath"]!, null))
                        .Where(f => f != null)
                        .Select(f => new Feat(f!.name, f.description))
                        .ToList();
                    passives.Add(new Passive(info.id, info.name, info.description, info.icon, (int?)p["m_MaxLevel"] ?? 1, feats));
                }
                var modifiers = (tier["m_SelectableModifiers"] ?? new JArray())
                    .Select(m => reader.Describe((string)m["ObjectPath"]!, textures))
                    .Where(m => m != null)
                    .Select(m => m!)
                    .ToList();
                tiers.Add(new Tier(actives, passives, modifiers));
            }
            var icon = AssetName(main["m_pIcon"]?["AssetPathName"]);
            var background = AssetName(main["m_pBackground"]?["AssetPathName"]);
            foreach (var t in new[] { icon, background }.Where(t => t != null)) textures.ExportByName(t!);
            masteries.Add(new Mastery(id, reader.Text(main["m_Title"]), reader.Text(main["m_Slogan"]), reader.Text(main["m_Description"]),
                (string?)main["m_Color"]?["Hex"] ?? "808080", icon, background, tiers));
            Console.WriteLine($"{id}: {tiers.Sum(t => t.actives.Count)} actives, {tiers.Sum(t => t.passives.Count)} passives, {tiers.Sum(t => t.modifiers.Count)} modifiers");
        }
        File.WriteAllText(Path.Combine(dir, "masteries.json"), JsonConvert.SerializeObject(masteries, Formatting.Indented));
        Console.WriteLine($"wrote {dir}");
    }

    /// "pkg:Outer.Name" prefix of an export inside the mastery package ("" for top-level ones).
    static string Outer(JToken e)
    {
        var outer = e["Outer"]?["ObjectName"]?.ToString();
        if (outer == null) return "";
        var inner = outer.Split('\'')[1];
        var colon = inner.IndexOf(':');
        return colon < 0 ? "" : inner[(colon + 1)..] + ".";
    }

    static string? AssetName(JToken? path) => path?.ToString() is { Length: > 0 } s ? s.Split('.').Last() : null;

    sealed class Reader(DefaultFileProvider provider)
    {
        readonly Dictionary<string, JArray> packages = [];
        // Object paths and file names differ in case ("Masteries" vs "masteries").
        readonly Dictionary<string, string> files = provider.Files.Keys.GroupBy(k => k.ToLowerInvariant()).ToDictionary(g => g.Key, g => g.First());
        readonly Dictionary<string, Dictionary<string, string>> tables = [];

        public JArray Exports(string path)
        {
            if (!packages.TryGetValue(path, out var exports))
            {
                try { exports = JArray.Parse(JsonConvert.SerializeObject(provider.LoadPackage(files.GetValueOrDefault(path.ToLowerInvariant(), path)).GetExports())); }
                catch (Exception ex) { Console.WriteLine($"  {path}: {ex.GetType().Name}"); exports = []; }
                packages[path] = exports;
            }
            return exports;
        }

        /// "/Game/TQ2/Data/X/bp_y.0" -> "TQ2/Content/TQ2/Data/X/bp_y.uasset"
        static string Package(string objectPath)
        {
            var p = objectPath[..objectPath.LastIndexOf('.')];
            return "TQ2/Content" + p["/Game".Length..] + ".uasset";
        }

        static JToken? Cdo(JArray exports) => exports.FirstOrDefault(e => ((string?)e["Name"])?.StartsWith("Default__") == true)?["Properties"];

        /// Follows ability selectors (GrimAbilityDescriptionSelection) to the first real ability.
        public string Resolve(string objectPath)
        {
            var cdo = Cdo(Exports(Package(objectPath)));
            var variants = cdo?["m_Descriptions"]?.ToList() ?? [];
            if (variants.Count == 0) return objectPath;
            // The base variant needs no activation tags; modifier variants are named "_m_<modifier>".
            bool IsBase(JToken v) => v["m_ActivationRequiredTags"]?.Any() != true
                && !((string?)v["m_Description"]?["ObjectPath"] ?? "").Contains("_m_");
            var chosen = variants.FirstOrDefault(IsBase)
                ?? variants.FirstOrDefault(v => v["m_ActivationRequiredTags"]?.Any() != true)
                ?? variants[0];
            return (string?)chosen["m_Description"]?["ObjectPath"] ?? objectPath;
        }

        /// Name, description and icon from a class default object, walking up parent blueprints
        /// for fields the child does not override.
        public Ability? Describe(string objectPath, MaterialLibrary? textures)
        {
            JToken? name = null, description = null, icon = null;
            var path = objectPath;
            for (int depth = 0; depth < 5 && path != null && path.StartsWith("/Game/"); depth++)
            {
                var exports = Exports(Package(path));
                var cdo = Cdo(exports);
                name ??= cdo?["m_Name"];
                description ??= cdo?["m_DynamicDescription"] ?? cdo?["m_Description"];
                icon ??= cdo?["m_pIconTexture"];
                if (name != null && description != null && icon != null) break;
                var cls = exports.FirstOrDefault(e => (string?)e["Type"] == "BlueprintGeneratedClass");
                path = (string?)cls?["SuperStruct"]?["ObjectPath"];
            }
            if (name == null) return null;
            var id = Path.GetFileNameWithoutExtension(Package(objectPath));
            // The ability's folder below masteries/ groups modifiers with their skill.
            var package = Package(objectPath);
            var group = package.Contains("/Modifiers/") ? package[..package.IndexOf("/Modifiers/")].Split('/').Last() : package.Split('/')[^2];
            string? iconName = icon?["ObjectName"]?.ToString().Split('\'')[1];
            if (iconName != null && textures != null && textures.ExportByName(iconName) == null) iconName = null;
            return new Ability(id, group, Text(name), Components(Text(description)), iconName, false);
        }

        Dictionary<string, JToken>? components;

        /// Expands TQ2's text building blocks: "{DescriptionComponent[IncreasedColdDamage2]}" becomes
        /// the stat line from its string table with its value variable, e.g.
        /// "<b>{IncreasedColdDamage2}|fmt(…)</> increased Cold Damage" (da_global_text_components).
        public string Components(string text)
        {
            components ??= Exports("TQ2/Saved/JsonDataCache/Game/TQ2/Text/da_global_text_components.uasset")
                .SelectMany(e => e["Properties"]?["TextComponents"] ?? new JArray())
                .GroupBy(c => (string)c["Identifier"]!)
                .ToDictionary(g => g.Key, g => g.First());
            return System.Text.RegularExpressions.Regex.Replace(text, @"\{DescriptionComponent\[(\w+)\]\}", m =>
            {
                if (!components.TryGetValue(m.Groups[1].Value, out var c)) return "";
                var line = Text(c["Default"]);
                foreach (var arg in c["ArgReplacements"] ?? new JArray())
                    line = line.Replace("{" + (string)arg["Key"]! + "}", "{" + (string)arg["Value"]! + "}");
                return line;
            });
        }

        public string Text(JToken? text)
        {
            if (text == null) return "";
            var table = (string?)text["TableId"];
            var key = (string?)text["Key"];
            if (table == null || key == null) return (string?)text["SourceString"] ?? (string?)text["LocalizedString"] ?? "";
            if (!tables.TryGetValue(table, out var entries))
            {
                entries = [];
                var file = Path.Combine(TextDir, table.Replace('/', Path.DirectorySeparatorChar) + ".csv");
                if (File.Exists(file))
                    foreach (var row in Csv(File.ReadAllText(file)).Skip(1).Where(r => r.Count >= 2))
                        entries[row[0]] = row[1];
                else Console.WriteLine($"  missing string table {file}");
                tables[table] = entries;
            }
            return entries.TryGetValue(key, out var value) ? value : "";
        }

        /// Minimal RFC 4180 reader: quoted fields may contain commas, newlines and "".
        static IEnumerable<List<string>> Csv(string text)
        {
            var row = new List<string>();
            var field = new System.Text.StringBuilder();
            bool quoted = false;
            for (int i = 0; i < text.Length; i++)
            {
                char c = text[i];
                if (quoted)
                {
                    if (c == '"' && i + 1 < text.Length && text[i + 1] == '"') { field.Append('"'); i++; }
                    else if (c == '"') quoted = false;
                    else field.Append(c);
                }
                else if (c == '"') quoted = true;
                else if (c == ',') { row.Add(field.ToString()); field.Clear(); }
                else if (c == '\n')
                {
                    row.Add(field.ToString().TrimEnd('\r')); field.Clear();
                    yield return row;
                    row = [];
                }
                else if (c != '\uFEFF') field.Append(c);
            }
            if (field.Length > 0 || row.Count > 0) { row.Add(field.ToString()); yield return row; }
        }
    }
}
