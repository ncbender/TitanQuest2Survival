# Decisions

## D1 – Read assets from each player's install, never ship them (2026-10-21)
Tools export into git-ignored `cache/`. A friend runs the same pipeline on their own copy.

## D2 – Generate the usmap by external memory reading (2026-10-21)
No public TQ2 usmap exists. UE4SS injected into the game crashed twice (Grim UClass
layout, then its usmap generator). A read-only external reader cannot crash the game,
needs no files in the install, and its layout assumptions are self-checked.

## D3 – Animations baked into glTF by our own tool (2026-10-21)
CUE4Parse exports skeletal meshes to glTF but animations only to psa/ueanim/usd.
`Tq2Extract animate` samples each AnimSequence per frame and writes glTF channels,
using the same axis conversion as CUE4Parse's mesh writer.

## D4 – Engine: Rust + Bevy (confirmed by user 2026-10-21)
Evidence: textured, animated TQ2 enemy renders in Bevy 0.20 via plain glTF.
UE5 would not avoid conversion: cooked TQ2 packages cannot be opened in the UE5
editor, so the same export → reimport would be needed, plus a ~100 GB engine install
and an Epic account. UE5's advantage is closer material fidelity (Grim shaders would
still need re-authoring). Bevy gives a small standalone exe, full control over many
enemies, and matches the azeroth-survivors-re reference architecture.

## D5 – Sound through TQ2's own FMOD runtime (2026-10-21)
TQ2 ships `fmod.dll`/`fmodstudio.dll` (FMOD 2.02.22) and unencrypted `.bank` files (FSB5/Vorbis,
1,862 events). The game loads both straight from the player's install at runtime (`audio.rs`,
`libloading`) and plays the original events by path, so nothing is extracted or shipped and
TQ2's own variations/mixing are kept. Without the DLLs the game runs silently.
`tq2_survivors --list-sounds` prints every event path.

## D6 – Meta progression: gear between runs, skill tree, local save (2026-10-21, user's design)
The user asked that gear found in a run can only be equipped outside the run, alongside a skill
tree that unlocks passives and new skills. Chests offer three items; the picked one goes into
the run's loot and is added to the stash when the run ends (death, victory or leaving via the
pause menu; nothing is lost). Favour = levels gained + full minutes + 5 for surviving. The
profile is JSON in `save/profile.json` inside the project folder (git-ignored, nothing outside
the project is written); automated test runs use `save/test_profile.json`. Unreadable profiles
are kept as `*.bad` instead of being overwritten.

## D7 – Weapon slots and attack animations (2026-10-21, user's rules)
Two-handed weapon alone, or a one-handed weapon plus a second one-handed weapon, a shield or a
talisman. The combination picks TQ2's own stance set (`sword+shield`, `sword+sword`,
`sword+talisman`, `staff2h`, `bow`, `axe2h`). Attack clips play on the upper body only (an
animation mask from `Spine1_M` up), so the legs keep running. Only the hero gets the masked
graph (`UpperBodyRig`): giving it to all characters dropped 500 enemies from 79 to 53 FPS.
