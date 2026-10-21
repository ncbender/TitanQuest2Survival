# Progress

## Done (2026-10-21)

- Environment: .NET 10, Rust 1.99, Python 3.14 (`py`), Windows SDK 10.0.26100 installed for MSVC linking.
- TQ2 archives open with CUE4Parse (`GAME_TitanQuest2`); own `.usmap` from live game memory.
- **Milestone 1:** Boarman Brute (mesh, textures, skeleton, 39 original clips) animated in Bevy.
- **Milestone 2:** `l_main_menu_scene_02` beach cove (2,906 placements, landscape, 105 materials)
  rendered with animated enemies.
- **Vertical slice (playable):** hero (TQ2 male body + Greek adventurer armour + hair, staff
  animations) on the original terrain; Boarman waves with elites and bosses (5:00, 9:00);
  auto-aimed forge bolts, Nova, crits, pierce; XP crystals (TQ2 mystic shards); level-up with
  3 random upgrades; HUD; 10-minute run, death/victory screen, restart.
- ACL animations decode (`CUE4Parse-Natives.dll` built locally, see ASSET_PIPELINE).

## Performance (measured, release build, Ryzen 7 9800X3D + Radeon RX 9070 XT, VSync 120 Hz)

`TQ2_AUTOPLAY=1 TQ2_BENCH=<n> cargo run --release` (each Boarman = 3 skinned parts, all animated):

| enemies | avg FPS | avg ms | p95 ms |
|---|---|---|---|
| 50 | 120 (VSync cap) | 8.33 | 8.63 |
| 100 | 112 | 8.91 | 9.73 |
| 250 | 60 | 16.73 | 20.94 |
| 500 | 45 | 22.27 | 30.59 |

After merging part skeletons into the body at runtime (`character.rs`, same day):

| enemies | avg FPS | avg ms | p95 ms |
|---|---|---|---|
| 250 | 115 | 8.69 | 10.15 |
| 500 | 84 | 11.95 | 14.70 |
| 800 | 50 | 19.94 | 25.39 |

Profiling note: with only the body part (`TQ2_BODY_ONLY=1`) 500 Boarmen hit the 120 FPS cap, so
the extra skeletons of mane/arm-hair parts were the main cost.

- Playtest fixes (2026-10-21): multishot fans out visibly (±11° per extra bolt), floating damage
  numbers, Nova fire flash (verified: 11–15 enemies hit per pulse with 30 enemies in range),
  flow-field pathing so enemies walk around rocks/water (unit test + 500 enemies at 79 FPS).

## Next

1. Performance beyond 500: animation LOD / reduced update rate for distant enemies.
2. ~~More enemy types~~ Spider, Giant Boar, Skeleton (rises from the ground), Harpy, Cyclops boss
   added via `tools/assets.json` + `bestiary.rs`; still no real death animation.
3. ~~Weapon on the hero~~ done; cast animation still open; TQ2 flipbook VFX for fireballs/Nova done.
4. ~~Terrain layer blending~~ done (11 original ground layers); larger land arena (Greece world
   partition, FastGeo) still open.
5. Satyr assembly (static-mesh parts on the humanoid rig).
6. ~~Main menu~~ + character creation done; settings/save, then two-player co-op.

## Gear (first version, 2026-10-21)

- `equipment.rs`: weapons (Staff of Apollo – fire bolts, Bow of Iris – piercing arrows,
  Battlemage blade – melee sweep) with their own stance clips, attach bone, sounds and attack;
  armour sets (Adventurer, Scholar, Ranger) swap the visible torso/legs/shoulders and absorb
  damage. Elites and bosses drop a TQ2 chest; walking into it offers three pieces of gear.
- Verified: screenshots of bow+ranger and sword+scholar; chest -> sword swap in a test run.
- Not yet: attack animations while firing/swinging, more weapon/armour sets, item rarity/stats.

## Character creation, fireballs, English UI (2026-10-21)

- Main menu → "New Hero" → character creation: pick the starting weapon (1–3 or click);
  every run (including restarts) starts with it (`StartingWeapon` in `equipment.rs`).
- Staff fires fireballs: TQ2 flipbook `t_explosion_01_05x05_c` (frames 0–4 looping) with an
  orange point light; explodes on impact (frames 0–11, the fiery part before the smoke).
  `vfx.rs` holds the camera-facing flipbook sprites. Fire Nova spawns a ring of 10 explosions.
- First elite is guaranteed after 30 s (verified: elite at 31 s, chest dropped at 40 s in an
  autoplay run), then every 15th spawn after a minute as before.
- All in-game text is English (menu, pause, HUD, level-up, upgrades, gear, loot, end screen,
  enemy names).
- Test hook `TQ2_MENU=hero` opens the character creation page (for screenshots).

## Planned features (requested by the user)

- **Visible gear:** equipment found or chosen during a run is shown on the hero (armour parts,
  weapon). The staff on `HandAttach_R` (2026-10-21) is the first test of this.
- **Weapon-driven auto-attacks:** the equipped weapon type decides the hero's auto-attack
  (e.g. staff = bolts, bow = arrows, sword = melee sweep); changing the weapon changes both the
  mesh in the hand and the attack. Build on `Attachment` + a weapon table like `bestiary.rs`.

## Blockers / risks

- Usmap RVA is build-specific: a TQ2 patch requires re-running the dumper with the game open.
- `a_boarman_mage_death_01` decodes to only 2 frames (unverified why).
