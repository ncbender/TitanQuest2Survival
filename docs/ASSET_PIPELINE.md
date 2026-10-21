# Asset pipeline

All Titan Quest II data is read from the local Steam install at runtime of the
tools and written only to the git-ignored `cache/` folder. Nothing extracted is
committed or distributed; each player runs the pipeline on their own install.

```
Steam install (.utoc/.ucas, read-only)
   │  tools/UsmapDumper   (reads running game's memory → cache/Mappings.usmap)
   │  tools/Tq2Extract    (CUE4Parse + mappings)
   ▼
cache/export/<name>/…   .glb meshes, .png textures, material .json
   │  Tq2Extract animate  (bakes AnimSequences + textures into one .glb)
   ▼
cache/export/<name>/<asset>_animated.glb  →  Bevy (game/)
```

## Installed game (verified 2026-10-21)

- Steam app 1154030, build 24869641 (files dated 2026-08-22), UE 5.7.4 (Grim fork).
- `TQ2/Content/Paks/TQ2-Windows.utoc/.ucas` (27.5 GB) + `global.utoc/.ucas` + `TQ2-Windows.pak` (299 MB).
- IoStore TOC version 8, container flags `Compressed | Indexed`, **not encrypted**
  (EncryptionKeyGuid zero). Compression: Oodle. No AES key needed.
- 114,292 files, 101,939 under `TQ2/Content/TQ2`.
- Packages use **unversioned properties** → a `.usmap` is mandatory.

## Mappings (.usmap)

No public TQ2 usmap exists (Nexus mod 62 was removed). UE4SS (experimental
v3.0.1-1181) injected into the game crashed twice:

1. UClass layout differs from stock 5.7: Grim inserts **0x60 bytes**, so
   `ClassDefaultObject` is at **+0x170** (stock +0x110). Verified from a crash
   dump: 6/8 UClasses point to their own CDO at +0x170.
2. After that fix, UE4SS's built-in usmap generator crashed on a null deref
   (no symbols available).

So `tools/UsmapDumper` reads the running game's reflection data **externally**
(`ReadProcessMemory`, nothing injected, nothing written to the game). Layout
facts it relies on, all verified against live memory by built-in checks:

| Item | TQ2 value | Note |
|---|---|---|
| GUObjectArray | exe RVA `0xa5c2ed0` | from UE4SS pattern scan; build-specific |
| FNamePool blocks | found by scan (RVA `0xa4f4990` this build) | first entries "None", "ByteProperty" |
| FUObjectItem | 0x18 bytes, **UObject\* at +0x08** | stock: at +0x00 |
| UObject | Class +0x10, Name +0x18, Outer +0x20 | stock |
| UStruct | Super +0x40, Children +0x48, ChildProperties +0x50 | stock |
| FField | Class +0x08, Owner +0x10, Next +0x18, Name +0x20 | stock |
| FProperty | ArrayDim +0x30, subclass members +0x78 | 201/201 StructProperty samples |
| FArrayProperty | Inner **+0x80** (ArrayFlags at +0x78) | 300/300 samples |
| UEnum | **names FName[] +0x40, values int64[] +0x48 (both ptrs tagged bit0), count +0x50** | stock: TArray<TPair> |

Result: 17,260 structs/classes, 2,331 enums, 60,778 names, no unknown property
types. Re-run after every game update (the RVA may move):

```
# start TQ2 to the main menu, then:
dotnet run --project tools/UsmapDumper -c Release
```

## Compatibility results

| # | Question | Status | Evidence |
|---|---|---|---|
| 1 | Open archives | **Confirmed** | CUE4Parse 1.2.2.202610, `EGame.GAME_TitanQuest2`, Oodle via OodleHelper download |
| 2 | Package index / names | **Confirmed** | 114,292 paths (`cache/filelist.txt`) |
| 3 | Static meshes | **Confirmed** (parse) | `sk_satyr_brute_01_body_01` is a StaticMesh; parsed, not yet exported/rendered |
| 4 | Skeletal meshes | **Confirmed** | Boarman Brute body/mane/armhair → .glb |
| 5 | Textures | **Confirmed** | `_a` albedo, `_n` normal, `_orm`, `_id`, `_mask` → .png |
| 6 | Skeletons | **Confirmed** | 64-joint skin in .glb; USkeleton has 92 bones (extra ones unused by this mesh) |
| 7 | Animations | **Confirmed** (35/42 Boarman) | baked to glTF by `animate`; 4 additive skipped; 3 need ACL natives (`CUE4Parse-Natives` DLL missing) |
| 8 | Materials / texture assignment | **Partial** | `MaterialLibrary` walks instance chains (+ root material referenced textures) → albedo/normal/ORM/alpha/two-sided; procedural Grim masters (rock gradients, ID tints) approximated |
| 9 | Terrain / environment | **Partial** | static/instanced/foliage meshes + level instances placed via `scene`; landscape exported as glb (CUE4Parse LandscapeMeshExporter) and placed by actor root (scale 100 = 1 m/quad), single-layer material (layer blend TODO); WP cells use FastGeoContainer (untested) |
| 10 | Load in standalone game | **Confirmed** | Bevy 0.20 renders the textured, animated Boarman Brute |

### Known gaps

- **Satyr:** its body/head parts are *StaticMeshes* (`sk_satyr_*`), apparently
  attached to the shared humanoid rig at runtime (archetype system,
  `/Game/TQ2/Core/Archetypes/Satyr`). Not yet understood → Boarman Brute used for milestone 1.
- Mouth interior of the Boarman shows background (two-sided or mask issue).
- Mane / arm hair are separate skeletal meshes, not yet attached.
- ACL-compressed animations need `CUE4Parse-Natives.dll` (build from source).

## Terrain layers

CUE4Parse's landscape export writes one full-size weight map per layer (`ll_<layer>.png`,
same grid as the height field, addressed by the landscape-wide UV_1). The ground textures are
wired inside `m_terrain_weight_blend_01a`'s shader graph (not parameters), so
`tools/Tq2Extract/terrain_layers.json` maps layer names to TQ2 texture sets by name; `puddle`
and `wetness` are modifiers without textures. `scene` writes `terrain/ll_*.png` + `terrain.json`.
The game (`terrain.rs`, `terrain.wesl`) packs weights into an RGBA array, builds albedo/normal
arrays with CPU-generated mipmaps and blends them per pixel, tiling every 4 m in world space.
Bevy 0.20 shaders are WESL (`import bevy_pbr::…`, `@group(constants::MATERIAL_BIND_GROUP)`).

## Collision

`scene` also writes `colliders.json`: every mesh's original UE simple collision from its
`BodySetup.AggGeom` (boxes, spheres, capsules, convex hulls) in Bevy mesh space. Meshes whose
profile is `NoCollision`/overlap-only get none (grass, flowers), and placements whose component
disables collision are marked `c: false` in `scene.json` (705 of 2,906 in menu2). The game
flattens each shape to a convex footprint and only blocks between knee and head height.

## Orientation

Exported TQ2 characters face +Z in Bevy space (toe bones of humans and Boarmen lie at +Z in the
bind pose); yaw for a ground velocity (x, z) is `atan2(x, z)`.

## Materials

Grim materials are chains, e.g. `mi_rock_greek_boulder_01a > mi_rock_greek_00a >
mi_master_rock_01a > m_master_rock_01a`. Rocks have no own albedo: the master's tiling
`BaseColor` is tinted by `Color`×`Tint`. Rules (`tools/Tq2Extract/Materials.cs`): most-derived
parameter wins; slot by exact name/substring, then texture suffix (`_a`, `_n`, `_orm`); engine
`Default*` textures ignored; root `UMaterial.ReferencedTextures` used when no parameters exist.
In Bevy: vertex colours stripped (UE uses them as masks), normal maps flipped Y (DirectX),
ORM matches Bevy's channel packing, samplers repeat.

## VFX flipbooks

TQ2's Niagara effects are not converted; the game uses their flipbook textures directly.
The `textures` section of `tools/assets.json` exports loose textures by name (e.g. `vfx` →
`cache/export/vfx/textures/`). `t_explosion_01_05x05_c` is a 5×5 sheet: frames 0–11 are fire,
the rest smoke. `game/src/vfx.rs` shows cells as camera-facing unlit quads (`uv_transform`
selects the cell).

## Masteries (skill tree)

`dotnet run -- masteries` (also part of `assets`) writes `cache/export/masteries/masteries.json`
and the icons/artwork as PNG:
- structure from `TQ2/Saved/JsonDataCache/Game/TQ2/masteries/da_mastery_<id>.uasset`
  (`TQ2MasteryDescription` → `m_Tiers` → `TQ2MasteryTierDescription` with `m_ActiveAbilities`,
  `m_PassiveAbilities` (`m_MaxLevel`, `m_SelectableFeatsNew`), `m_SelectableModifiers`);
  actives behind `m_RequiredEntitlements` are TQ2 work in progress and stay hidden;
- active skills are often `GrimAbilityDescriptionSelection` blueprints: the base variant is the
  `m_Descriptions` entry without activation tags and without `_m_` in its name;
- each ability's class default object has `m_Name`, `m_DynamicDescription`/`m_Description`
  (string-table references) and `m_pIconTexture`; parents are followed for missing fields;
- texts come from the English string tables TQ2 ships as loose CSV files in
  `TQ2/Content/TQ2/Data/Text/**.csv` (read only); the packaged `TQ2.locres` holds only engine
  texts;
- `{DescriptionComponent[X]}` building blocks are expanded with
  `TQ2/Saved/JsonDataCache/Game/TQ2/Text/da_global_text_components` (identifier → string table
  line + value variable, `X2` = next level);
- object paths and file names differ in case (`Masteries` vs `masteries`): look packages up
  case-insensitively.
`dumpfile <pkg|dir>` writes complete JSON of packages to `cache/dump/` for such analysis.
UI frames/backgrounds (`ui`), item icons (`items`) and skill VFX textures (`vfx`) are loose
texture lists in `tools/assets.json`.

## Building all game assets

`tools/assets.json` lists every character (parts, skeleton, animation folders, optional clip
prefixes), prop set and scene the game uses. One command rebuilds everything:

```
cd tools/Tq2Extract
dotnet run -- assets            # all
dotnet run -- assets spider     # only one entry
```

## Commands

```
cd tools/Tq2Extract
dotnet run -- list
dotnet run -- export boarman TQ2/Content/TQ2/Character/Boarman/Boarman_Brute
dotnet run -- scene menu2 TQ2/Content/TQ2/Maps/menu/l_main_menu_scene_02.umap
dotnet run -- animate boarman/TQ2/Content/TQ2/Character/Boarman/Boarman_Brute/sk_boarman_brute_01.glb \
    boarman/boarman_brute_animated.glb \
    TQ2/Content/TQ2/Character/Boarman/Boarman_Brute/skel_boarman_brute_01.uasset \
    TQ2/Content/TQ2/Character/Boarman/Boarman_Brute/Animation
```

Coordinate conversion (must match CUE4Parse's glTF writer): position
`(x, z, y) * 0.01`, rotation `(x, z, y, -w)`, scale `(x, z, y)`.
