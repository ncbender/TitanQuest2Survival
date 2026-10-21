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

## Next

1. Performance: animation LOD / update-rate reduction, drop mane/armhair parts at distance,
   profile where the 250+ cost goes (pose evaluation vs. skinning vs. draw calls).
2. ~~More enemy types~~ Spider, Giant Boar, Skeleton (rises from the ground), Harpy, Cyclops boss
   added via `tools/assets.json` + `bestiary.rs`; still no real death animation.
3. Weapon on the hero (staff), cast animation, TQ2 VFX instead of the temporary Nova torus.
4. Terrain layer blending (weight maps) and a larger land arena (Greece world partition, FastGeo).
5. Satyr assembly (static-mesh parts on the humanoid rig).
6. Main menu, settings/save, then two-player co-op.

## Blockers / risks

- Usmap RVA is build-specific: a TQ2 patch requires re-running the dumper with the game open.
- `a_boarman_mage_death_01` decodes to only 2 frames (unverified why).
