# Progress

## Done (2026-10-21)

- Environment: .NET 10, Rust 1.99, Python 3.14 (`py`), Windows SDK 10.0.26100 installed for MSVC linking.
- TQ2 archives open with CUE4Parse (`GAME_TitanQuest2`), 114,292 files listed.
- Own `.usmap` generated from live game memory (UE4SS route crashed twice; see ASSET_PIPELINE).
- Boarman Brute exported: skeletal mesh, textures, 35 original animations baked into one .glb.
- **Milestone 1 reached:** Bevy viewer renders the textured Boarman Brute playing
  `a_boarman_none_idle_01` (verified by screenshot).

## Next

1. Engine decision confirmation (Bevy recommended, see DECISIONS).
2. Fix Boarman mouth interior; attach mane/armhair meshes.
3. Milestone 2: export a TQ2 environment (static meshes / level) and show it with the animated enemy.
4. Understand Satyr assembly (static mesh parts + humanoid rig + archetypes).
5. Survivors vertical slice: player movement, enemy spawning/chase, auto-attack, XP, level-up choices.
6. Performance: measure 50 / 100 / 250 animated Boarmen.

## Blockers / risks

- ACL-compressed animations need `CUE4Parse-Natives.dll` (3/42 Boarman clips).
- Usmap RVA is build-specific: a TQ2 patch requires re-running the dumper with the game open.
