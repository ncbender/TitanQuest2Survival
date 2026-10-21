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
