# Progress

## Done (2026-10-21)

- Environment: .NET 10, Rust 1.99, Python 3.14 (`py`), Windows SDK 10.0.26100 installed for MSVC linking.
- TQ2 archives open with CUE4Parse (`GAME_TitanQuest2`), 114,292 files listed.
- Own `.usmap` generated from live game memory (UE4SS route crashed twice; see ASSET_PIPELINE).
- Boarman Brute exported: skeletal mesh, textures, 35 original animations baked into one .glb.
- **Milestone 1 reached:** Bevy viewer renders the textured Boarman Brute playing
  `a_boarman_none_idle_01` (verified by screenshot).

- **Milestone 2 (partial):** `Tq2Extract scene` exports a map as unique meshes + `scene.json`
  placements (static, instanced, foliage, level instances); `MaterialLibrary` resolves Grim
  material chains into `materials.json` + shared textures; Bevy applies one shared material per
  TQ2 material. `l_main_menu_scene_02` (2,905 instances, 187 meshes, 102/104 materials textured)
  renders with the animated Boarman (screenshot verified). Landscape not exported yet.

## Next

1. ~~Engine decision~~ Bevy confirmed.
2. Fix Boarman mouth interior; attach mane/armhair meshes.
3. Milestone 2: landscape (heightmap + layer weights) so the ground exists; then a land arena
   (Greece world partition cells use FastGeoContainer, not yet supported).
4. Understand Satyr assembly (static mesh parts + humanoid rig + archetypes).
5. Survivors vertical slice: player movement, enemy spawning/chase, auto-attack, XP, level-up choices.
6. Performance: measure 50 / 100 / 250 animated Boarmen.

## Blockers / risks

- ACL-compressed animations need `CUE4Parse-Natives.dll` (3/42 Boarman clips).
- Usmap RVA is build-specific: a TQ2 patch requires re-running the dumper with the game open.
