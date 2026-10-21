using CUE4Parse.FileProvider;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

// Exports TQ2's item affixes: which prefixes/suffixes can roll on weapons, armour and off-hands
// (from the affix drop tables, with TQ2's drop weights), their names per tier and their effect
// text. Output: cache/export/items/affixes.json.
//
// The numeric ranges live in GrimDynamicsTables (da_tq2_item_affix_values_*), whose data is
// custom-serialised and not readable with CUE4Parse; the game supplies its own ranges.
static class Affixes
{
    const string TextDir = @"C:\Program Files (x86)\Steam\steamapps\common\Titan Quest II\TQ2\Content\TQ2\Data\Text";
    const string Tables = "TQ2/Content/TQ2/Items/Affixes/AffixDropTables/";

    /// Top-level drop tables per item slot.
    static readonly (string slot, string[] tables)[] Slots =
    [
        ("weapon", ["Prefix/weapons/da_at_01_Prefix_Generic_AllWeapons", "Suffix/weapons/da_at_01_Suffix_Generic_AllWeapon"]),
        ("armor", ["Prefix/Armors/da_at_01_Prefix_Torso", "Suffix/Armors/da_at_01_Suffix_Torso"]),
        ("offhand", ["Prefix/Offhand/", "Suffix/Offhand/"]),
    ];

    record Affix(string id, string kind, List<string> names, string text, Dictionary<string, float> weights);

    public static void Run(DefaultFileProvider provider, string cache)
    {
        var files = provider.Files.Keys.GroupBy(k => k.ToLowerInvariant()).ToDictionary(g => g.Key, g => g.First());
        var strings = new Dictionary<string, Dictionary<string, string>>();
        JArray Load(string objectPath)
        {
            var p = objectPath.Contains('.') ? objectPath[..objectPath.LastIndexOf('.')] : objectPath;
            var pkg = ("TQ2/Content" + p["/Game".Length..] + ".uasset").ToLowerInvariant();
            if (!files.TryGetValue(pkg, out var real)) return [];
            try { return JArray.Parse(JsonConvert.SerializeObject(provider.LoadPackage(real).GetExports())); }
            catch { return []; }
        }
        string Text(JToken? t)
        {
            var table = (string?)t?["TableId"];
            var key = (string?)t?["Key"];
            if (table == null || key == null) return "";
            if (!strings.TryGetValue(table, out var entries))
            {
                entries = [];
                var file = Path.Combine(TextDir, table.Replace('/', Path.DirectorySeparatorChar) + ".csv");
                if (File.Exists(file))
                    foreach (var line in File.ReadAllLines(file).Skip(1))
                    {
                        // "key","text","comment" (single-line entries are enough for affixes)
                        var parts = line.Split("\",\"");
                        if (parts.Length >= 2) entries[parts[0].Trim('"', '\uFEFF')] = parts[1].Replace("\\'", "'");
                    }
                strings[table] = entries;
            }
            return entries.GetValueOrDefault(key, "");
        }

        var affixes = new Dictionary<string, Affix>();
        void Walk(string objectPath, string slot, float weight, int depth)
        {
            if (depth > 6) return;
            foreach (var e in Load(objectPath))
            {
                var table = e["Properties"]?["m_Table"];
                if (table == null) continue;
                float total = 0;
                var entries = (table["Entries"] ?? new JArray()).Concat(table["SubTables"] ?? new JArray()).ToList();
                foreach (var x in entries) total += (float?)x["Weight"]?["Value"]?["Value"] ?? 1;
                foreach (var x in entries)
                {
                    var w = weight * ((float?)x["Weight"]?["Value"]?["Value"] ?? 1) / Math.Max(total, 1);
                    if (x["pTable"]?["ObjectPath"] is { } sub) Walk((string)sub!, slot, w, depth + 1);
                    else if (x["pAffix"]?["ObjectPath"] is { } a) AddAffix((string)a!, slot, w);
                }
            }
        }
        void AddAffix(string objectPath, string slot, float weight)
        {
            var id = objectPath.Split('/').Last().Split('.')[0];
            if (!affixes.TryGetValue(id, out var affix))
            {
                var exports = Load(objectPath);
                var props = exports.FirstOrDefault()?["Properties"];
                var levels = props?["m_Levels"]?["Levels"] as JArray;
                if (props == null || levels == null || levels.Count == 0) return;
                var names = new List<string>();
                foreach (var level in levels)
                {
                    var name = Text(level["TitleText"]);
                    names.Add(name.Length > 0 ? name : names.LastOrDefault() ?? "");
                }
                var text = Text(levels[0]["AffixEffects"]?.FirstOrDefault()?["ToolTipText"]);
                var kind = ((string?)props["m_Type"] ?? "").EndsWith("Prefix") ? "prefix" : "suffix";
                if (names.All(n => n.Length == 0) || text.Length == 0) return;
                affix = affixes[id] = new Affix(id, kind, names, text, []);
            }
            affix.weights[slot] = affix.weights.GetValueOrDefault(slot) + weight;
        }

        foreach (var (slot, tables) in Slots)
            foreach (var t in tables)
            {
                var paths = t.EndsWith('/')
                    ? provider.Files.Keys.Where(k => k.StartsWith(Tables + t, StringComparison.OrdinalIgnoreCase) && k.EndsWith(".uasset"))
                        .Select(k => "/Game" + k["TQ2/Content".Length..^".uasset".Length] + ".0")
                    : ["/Game/TQ2/Items/Affixes/AffixDropTables/" + t + ".0"];
                foreach (var p in paths) Walk(p, slot, 1000, 0);
            }

        var dir = Path.Combine(cache, "export", "items");
        Directory.CreateDirectory(dir);
        File.WriteAllText(Path.Combine(dir, "affixes.json"), JsonConvert.SerializeObject(affixes.Values.OrderBy(a => a.id), Formatting.Indented));
        Console.WriteLine($"affixes.json: {affixes.Count} affixes ({affixes.Values.Count(a => a.kind == "prefix")} prefixes)");
    }
}
