// External, read-only .usmap dumper for Titan Quest II.
//
// Reads the running game's reflection data with ReadProcessMemory: nothing is
// injected and nothing in the game or its install is written. Layout facts that
// differ from stock UE 5.7 were verified from crash dumps (see docs/ASSET_PIPELINE.md);
// everything else is auto-detected and self-checked at startup.
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

const string ProcessName = "TQ2-Win64-Shipping";
const long GUObjectArrayRva = 0xa5c2ed0; // from UE4SS pattern scan, build 24869641

var root = FindRoot();
var outPath = args.Length > 0 && !args[0].StartsWith("--") ? args[0] : Path.Combine(root, "cache", "Mappings.usmap");

var game = Process.GetProcessesByName(ProcessName).FirstOrDefault()
           ?? throw new Exception($"{ProcessName}.exe is not running");
var mem = new Mem(game);
var module = game.MainModule!;
long exeBase = module.BaseAddress, exeSize = module.ModuleMemorySize;
Console.WriteLine($"pid {game.Id}, exe base {exeBase:x}, size {exeSize:x}");

var names = new Names(mem, FindNamePool(mem, exeBase, exeSize));
var objects = ReadObjectArray(mem, exeBase + GUObjectArrayRva, names);
Console.WriteLine($"{objects.Count} live objects");

// --- classify objects -------------------------------------------------------
var chainCache = new Dictionary<long, string[]>();
string[] ClassChain(long cls)
{
    if (chainCache.TryGetValue(cls, out var c)) return c;
    var list = new List<string>();
    for (long s = cls; s != 0 && list.Count < 64; s = mem.Q(s + 0x40)) list.Add(names.Obj(s));
    return chainCache[cls] = list.ToArray();
}

var structs = new List<long>();
var enums = new List<long>();
foreach (var o in objects)
{
    var chain = ClassChain(mem.Q(o + 0x10));
    if (chain.Contains("Enum")) enums.Add(o);
    // PropertyBag_* are transient runtime structs that never appear in cooked packages.
    else if (chain.Contains("Struct") && !chain.Contains("Function") && !names.Obj(o).StartsWith("PropertyBag_")) structs.Add(o);
}
Console.WriteLine($"{structs.Count} structs/classes, {enums.Count} enums");

// --- detect FProperty subclass member offset (stock: 0x78) -----------------
var structProps = new List<long>();
foreach (var s in structs)
{
    foreach (var p in Props(s))
        if (names.FieldClass(p) == "StructProperty") { structProps.Add(p); if (structProps.Count > 200) break; }
    if (structProps.Count > 200) break;
}
int extraOff = new[] { 0x70, 0x78, 0x80, 0x88 }
    .OrderByDescending(off => structProps.Count(p => ClassChain(mem.Q(mem.Q(p + off) + 0x10)).Contains("ScriptStruct")))
    .First();
var hit = structProps.Count(p => ClassChain(mem.Q(mem.Q(p + extraOff) + 0x10)).Contains("ScriptStruct"));
Console.WriteLine($"FProperty subclass members at +{extraOff:x} ({hit}/{structProps.Count} StructProperty samples valid)");

// --- UEnum entries -----------------------------------------------------------
// TQ2 / UE 5.7 stores names and values in two separate arrays, both pointers
// tagged in bit 0: +0x40 FName[], +0x48 int64[], +0x50 int32 count.
// (Stock 5.x: TArray<TPair<FName, int64>> Names.)
(long Names, long Values, int Count) EnumEntries(long e) => (mem.Q(e + 0x40) & ~1L, mem.Q(e + 0x48) & ~1L, mem.I(e + 0x50));
var badEnums = enums.Count(e => { var (n, _, c) = EnumEntries(e); return c < 0 || c > 65535 || (c > 0 && names.Get(mem.Q(n)).Length == 0); });
Console.WriteLine($"UEnum entries: {enums.Count - badEnums}/{enums.Count} plausible");

// --- ArrayProperty::Inner (UE 5.x: ArrayFlags precede Inner) ---------------
var arrayProps = structs.SelectMany(Props).Where(p => names.FieldClass(p) == "ArrayProperty").Take(300).ToList();
int arrayInnerOff = new[] { extraOff, extraOff + 8 }
    .OrderByDescending(off => arrayProps.Count(p => names.FieldClass(mem.Q(p + off)).EndsWith("Property"))).First();
Console.WriteLine($"ArrayProperty::Inner at +{arrayInnerOff:x} ({arrayProps.Count(p => names.FieldClass(mem.Q(p + arrayInnerOff)).EndsWith("Property"))}/{arrayProps.Count} valid)");

// --- self test --------------------------------------------------------------
var vector = structs.FirstOrDefault(s => names.Obj(s) == "Vector" && names.Obj(mem.Q(s + 0x20)) == "/Script/CoreUObject");
Console.WriteLine("self test Vector: " + string.Join(", ", Props(vector).Select(p => $"{names.FieldClass(p)} {names.Field(p)}")));

// --- write usmap ------------------------------------------------------------
var nameIdx = new Dictionary<string, int>();
var nameList = new List<string>();
int N(string s) { if (!nameIdx.TryGetValue(s, out var i)) { i = nameList.Count; nameList.Add(s); nameIdx[s] = i; } return i; }

var unknownTypes = new Dictionary<string, int>();
string currentStruct = "", currentProp = "";
var typeStack = new Stack<string>();
var unknownSamples = new Dictionary<string, string>();
var body = new MemoryStream();
var w = new BinaryWriter(body);

w.Write((uint)enums.Count);
foreach (var e in enums)
{
    var (na, va, num) = EnumEntries(e);
    num = Math.Clamp(num, 0, 65535);
    w.Write(N(names.Obj(e)));
    w.Write((ushort)num);
    for (int i = 0; i < num; i++)
    {
        var n = names.Get(mem.Q(na + i * 8));
        var sep = n.IndexOf("::", StringComparison.Ordinal);
        w.Write(mem.Q(va + i * 8));
        w.Write(N(sep >= 0 ? n[(sep + 2)..] : n));
    }
}

w.Write((uint)structs.Count);
foreach (var s in structs)
{
    var props = Props(s).ToList();
    var super = mem.Q(s + 0x40);
    currentStruct = names.Obj(s);
    w.Write(N(names.Obj(s)));
    w.Write(super != 0 ? N(names.Obj(super)) : -1);
    w.Write((ushort)props.Sum(p => Math.Max(1, mem.I(p + 0x30))));
    w.Write((ushort)props.Count);
    int index = 0;
    foreach (var p in props)
    {
        int dim = Math.Max(1, mem.I(p + 0x30));
        w.Write((ushort)index);
        w.Write((byte)dim);
        currentProp = names.Field(p);
        w.Write(N(names.Field(p)));
        WriteType(p);
        index += dim;
    }
}

void WriteType(long p)
{
    var type = names.FieldClass(p);
    typeStack.Push(type);
    try { WriteTypeInner(p, type); } finally { typeStack.Pop(); }
}
void WriteTypeInner(long p, string type)
{
    switch (type)
    {
        case "ByteProperty":
            var byteEnum = mem.Q(p + extraOff);
            if (byteEnum != 0) { w.Write((byte)PropType.EnumProperty); w.Write((byte)PropType.ByteProperty); w.Write(N(names.Obj(byteEnum))); }
            else w.Write((byte)PropType.ByteProperty);
            return;
        case "EnumProperty":
            w.Write((byte)PropType.EnumProperty);
            WriteType(mem.Q(p + extraOff));
            var en = mem.Q(p + extraOff + 8);
            w.Write(en != 0 ? N(names.Obj(en)) : -1);
            return;
        case "StructProperty":
            w.Write((byte)PropType.StructProperty);
            w.Write(N(names.Obj(mem.Q(p + extraOff))));
            return;
        case "ArrayProperty":
            w.Write((byte)PropType.ArrayProperty);
            WriteType(mem.Q(p + arrayInnerOff));
            return;
        case "MulticastSparseDelegateProperty": // never serialized; closest usmap type
            w.Write((byte)PropType.MulticastDelegateProperty);
            return;
        case "SetProperty" or "OptionalProperty":
            w.Write((byte)Enum.Parse<PropType>(type));
            WriteType(mem.Q(p + extraOff));
            return;
        case "MapProperty":
            w.Write((byte)PropType.MapProperty);
            WriteType(mem.Q(p + extraOff));
            WriteType(mem.Q(p + extraOff + 8));
            return;
    }
    if (Enum.TryParse<PropType>(type, out var t)) w.Write((byte)t);
    else
    {
        unknownTypes[type] = unknownTypes.GetValueOrDefault(type) + 1;
        var key = string.Join(" > ", typeStack.Reverse().Select(t => t.Length > 30 ? "<garbage>" : t));
        unknownSamples.TryAdd(key, $"{currentStruct}.{currentProp} p={p:x}");
        w.Write((byte)PropType.Unknown);
    }
}

var payload = new MemoryStream();
var pw = new BinaryWriter(payload);
pw.Write((uint)nameList.Count);
foreach (var n in nameList) { var b = Encoding.UTF8.GetBytes(n); pw.Write((ushort)b.Length); pw.Write(b); }
pw.Write(body.ToArray());

Directory.CreateDirectory(Path.GetDirectoryName(outPath)!);
using (var f = new BinaryWriter(File.Create(outPath)))
{
    f.Write((ushort)0x30C4);
    f.Write((byte)4); // EUsmapVersion.ExplicitEnumValues
    f.Write(0);       // HasVersioning = false (read as int32)
    f.Write((byte)0); // no compression
    f.Write((uint)payload.Length);
    f.Write((uint)payload.Length);
    f.Write(payload.ToArray());
}
Console.WriteLine($"wrote {outPath}: {nameList.Count} names, {enums.Count} enums, {structs.Count} structs");
foreach (var (k, v) in unknownTypes) Console.WriteLine($"  unknown property type {(k.Length > 30 ? "<garbage>" : k)}: {v}");
foreach (var (k, v) in unknownSamples) Console.WriteLine($"  path {k}  e.g. {v}");
return 0;

IEnumerable<long> Props(long ustruct)
{
    if (ustruct == 0) yield break;
    int guard = 0;
    for (long f = mem.Q(ustruct + 0x50); f != 0 && guard++ < 10000; f = mem.Q(f + 0x18)) yield return f;
}

static long FindNamePool(Mem mem, long exeBase, long exeSize)
{
    // FNameEntryAllocator { FRWLock; uint32 CurrentBlock; uint32 CurrentByteCursor; uint8* Blocks[] }
    // Blocks[0] starts with the "None" entry followed by "ByteProperty".
    var pe = mem.Bytes(exeBase, 0x1000);
    int lfanew = BitConverter.ToInt32(pe, 0x3c), nsec = BitConverter.ToUInt16(pe, lfanew + 6);
    int optSize = BitConverter.ToUInt16(pe, lfanew + 20), secOff = lfanew + 24 + optSize;
    for (int i = 0; i < nsec; i++)
    {
        int o = secOff + i * 40;
        uint vsize = BitConverter.ToUInt32(pe, o + 8), va = BitConverter.ToUInt32(pe, o + 12), ch = BitConverter.ToUInt32(pe, o + 36);
        if ((ch & 0x80000000) == 0) continue; // writable sections only
        for (long chunk = 0; chunk < vsize; chunk += 0x100000)
        {
            var buf = mem.Bytes(exeBase + va + chunk, (int)Math.Min(0x100000 + 0x20, vsize - chunk));
            for (int k = 16; k + 16 <= buf.Length; k += 8)
            {
                uint curBlock = BitConverter.ToUInt32(buf, k - 8), cursor = BitConverter.ToUInt32(buf, k - 4);
                long b0 = BitConverter.ToInt64(buf, k), b1 = BitConverter.ToInt64(buf, k + 8);
                if (curBlock == 0 || curBlock > 8192 || cursor > 0x40000 || b0 < 0x10000 || b1 < 0x10000 || b0 > 0x7fffffffffff) continue;
                var head = mem.TryBytes(b0, 32);
                if (head == null) continue;
                if (Encoding.ASCII.GetString(head, 2, 4) == "None" && Encoding.ASCII.GetString(head, 8, 12) == "ByteProperty")
                {
                    Console.WriteLine($"name pool blocks at {exeBase + va + chunk + k:x} (RVA {va + chunk + k:x}), {curBlock + 1} blocks");
                    return exeBase + va + chunk + k;
                }
            }
        }
    }
    throw new Exception("FNamePool not found");
}

static List<long> ReadObjectArray(Mem mem, long guobjectArray, Names names)
{
    // FUObjectArray: 4 ints, then FChunkedFixedUObjectArray { Objects**, PreAllocated*, Max, Num, MaxChunks, NumChunks }
    long chunks = mem.Q(guobjectArray + 0x10);
    int num = mem.I(guobjectArray + 0x24), numChunks = mem.I(guobjectArray + 0x2c);
    Console.WriteLine($"GUObjectArray: {num} elements in {numChunks} chunks");
    if (num <= 0 || num > 10_000_000) throw new Exception("GUObjectArray layout mismatch");
    const int PerChunk = 64 * 1024;
    // Stock UE: { UObject*, flags, ... }; UE 5.7 / TQ2: { FlagsAndRefCount, UObject*, serial }.
    long c0 = mem.Q(chunks);
    var (itemSize, objOff) = (from size in new[] { 0x18, 0x20, 0x10 }
                              from off in new[] { 0, 8 }
                              orderby Enumerable.Range(0, 64).Count(i => { long o = mem.Q(c0 + i * size + off); return o > 0x10000 && names.Obj(mem.Q(o + 0x10)).Length > 0; }) descending
                              select (size, off)).First();
    Console.WriteLine($"FUObjectItem size {itemSize:x}, object at +{objOff:x}");
    var result = new List<long>(num);
    for (int c = 0; c < numChunks; c++)
    {
        long chunk = mem.Q(chunks + c * 8);
        int count = Math.Min(PerChunk, num - c * PerChunk);
        if (chunk == 0 || count <= 0) continue;
        var buf = mem.Bytes(chunk, count * itemSize);
        for (int i = 0; i < count; i++)
        {
            long o = BitConverter.ToInt64(buf, i * itemSize + objOff);
            if (o != 0) result.Add(o);
        }
    }
    return result;
}

static string FindRoot()
{
    var d = new DirectoryInfo(AppContext.BaseDirectory);
    while (d != null && !Directory.Exists(Path.Combine(d.FullName, ".git"))) d = d.Parent;
    return d?.FullName ?? Directory.GetCurrentDirectory();
}

class Names(Mem mem, long blocks)
{
    readonly Dictionary<uint, string> cache = new();
    readonly long[] blockPtrs = Enumerable.Range(0, 8192).Select(i => mem.Q(blocks + i * 8)).ToArray();

    // FName = { uint32 ComparisonIndex, uint32 Number }
    public string Get(long fname)
    {
        uint idx = (uint)fname, number = (uint)(fname >> 32);
        var s = Entry(idx);
        return number == 0 ? s : $"{s}_{number - 1}";
    }

    string Entry(uint idx)
    {
        if (cache.TryGetValue(idx, out var s)) return s;
        s = "";
        long block = (idx >> 16) < blockPtrs.Length ? blockPtrs[idx >> 16] : 0;
        if (block != 0)
        {
            var hdr = mem.TryBytes(block + (idx & 0xFFFF) * 2L, 2);
            if (hdr != null)
            {
                int h = BitConverter.ToUInt16(hdr), len = h >> 6;
                bool wide = (h & 1) != 0;
                var raw = mem.TryBytes(block + (idx & 0xFFFF) * 2L + 2, wide ? len * 2 : len);
                if (raw != null) s = wide ? Encoding.Unicode.GetString(raw) : Encoding.Latin1.GetString(raw);
            }
        }
        return cache[idx] = s;
    }

    public string Obj(long uobject) => uobject == 0 ? "" : Get(mem.Q(uobject + 0x18));
    public string Field(long ffield) => Get(mem.Q(ffield + 0x20));
    public string FieldClass(long ffield) => Get(mem.Q(mem.Q(ffield + 0x08)));
}

class Mem(Process p)
{
    readonly IntPtr handle = OpenProcess(0x0010 | 0x0400, false, p.Id); // VM_READ | QUERY_INFORMATION

    public byte[]? TryBytes(long addr, int n)
    {
        if (addr <= 0) return null;
        var buf = new byte[n];
        return ReadProcessMemory(handle, (IntPtr)addr, buf, n, out var read) && read == n ? buf : null;
    }
    public byte[] Bytes(long addr, int n) => TryBytes(addr, n) ?? new byte[n];
    public long Q(long addr) => BitConverter.ToInt64(Bytes(addr, 8));
    public int I(long addr) => BitConverter.ToInt32(Bytes(addr, 4));

    [DllImport("kernel32", SetLastError = true)] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32", SetLastError = true)] static extern bool ReadProcessMemory(IntPtr h, IntPtr addr, byte[] buf, int size, out int read);
}

enum PropType : byte
{
    ByteProperty, BoolProperty, IntProperty, FloatProperty, ObjectProperty, NameProperty, DelegateProperty,
    DoubleProperty, ArrayProperty, StructProperty, StrProperty, TextProperty, InterfaceProperty,
    MulticastDelegateProperty, WeakObjectProperty, LazyObjectProperty, AssetObjectProperty, SoftObjectProperty,
    UInt64Property, UInt32Property, UInt16Property, Int64Property, Int16Property, Int8Property, MapProperty,
    SetProperty, EnumProperty, FieldPathProperty, OptionalProperty, Utf8StrProperty, AnsiStrProperty,
    ClassProperty, MulticastInlineDelegateProperty, SoftClassProperty, VerseStringProperty,
    VerseDynamicProperty, VerseFunctionProperty, Unknown = 0xFF
}
