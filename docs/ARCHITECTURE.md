# Architecture

Two parts: offline tools turn the player's own TQ2 install into ordinary files under the
git-ignored `cache/`, and the game (Rust + Bevy) loads only those files.

```
TQ2 install (read only)
   │  tools/UsmapDumper   reads the running game's reflection data -> cache/Mappings.usmap
   │  tools/Tq2Extract    CUE4Parse: meshes, animations, textures, maps, masteries, affixes
   ▼                      (everything listed in tools/assets.json; `assets` builds it all)
cache/export/<name>/…    glTF (.glb) + PNG + JSON (scene.json, ground.json, terrain.json,
   │                      colliders.json, materials.json, masteries.json, affixes.json)
   ▼
game/ (Bevy)
```

`update.ps1` in the repository root runs the whole chain for a player: pull, mappings, natives,
assets (only when the tools changed), build, start.

## Game modules (`game/src`)

Engine-side, independent of the game rules:

| module | job |
|---|---|
| `map.rs` | the loaded map: scene props, ground heights, obstacles, blended terrain; swapped when another arena is selected (`SelectedMap` → `LoadedMap`, `MapLoaded` event) |
| `tq2_scene.rs` | spawns `scene.json` placements, reads colliders, prop extent |
| `ground.rs` | height lookup from `ground.json`, water, playable bounds |
| `obstacles.rs` | 2D obstacles from UE simple collision |
| `terrain.rs` + `terrain.wesl` | landscape layer blending (world-aligned weight maps) |
| `tq2_materials.rs` | shared PBR materials for exported TQ2 materials |
| `character.rs` | modular characters (body + parts), animation graphs, upper-body attacks |
| `audio.rs` | TQ2's FMOD banks through the FMOD runtime |
| `fx.rs`, `vfx.rs` | skill effects and flipbook sprites from TQ2 VFX textures |

The game (`survivors/`): the simulation lives in plain components on the ground plane
(`Pos`, `Vel`, `Health`, `Radius`) and runs in `FixedUpdate`; rendering follows it. This keeps a
later two-player host/client split possible.

| module | job |
|---|---|
| `mod.rs` | states, run start/reset, ground following |
| `arenas.rs` | arena list: map, enemy roster, boss, strength, loot, unlocks |
| `bestiary.rs` | enemy types (TQ2 creatures + stats) and roster picking |
| `enemies.rs`, `flowfield.rs` | waves, chasing around obstacles, melee |
| `combat.rs` | weapon attacks, projectiles, deaths, chests, XP |
| `abilities.rs` | TQ2 skills as survivor behaviours, ailments, buffs |
| `mechanics.rs` | Barrier, Rage, glancing hits, execution, shades (passive-driven) |
| `player.rs`, `camera.rs` | hero movement and stats, camera |
| `progression.rs`, `hud.rs` | XP, level-ups, HUD, end screen |
| `equipment.rs`, `items.rs` | base items, slots, stances, rolled items (rarity, affixes) |
| `profile.rs` | save: shared stash, characters, arena unlocks |
| `skills.rs`, `mastery_data.rs`, `skilltree.rs` | TQ2 masteries, passives, mastery screen |
| `menu.rs`, `settings.rs`, `ui.rs` | menus in TQ2's look, settings |
| `bench.rs` | benchmark and gallery test modes |

Test hooks (environment variables) are listed in the header of `main.rs`.
