# Titan Quest II Survivors

Standalone Survivors-style game (Vampire Survivors / Halls of Torment) that renders
**original Titan Quest II assets read from the player's own Steam install**.
Private use for the owner and one friend; both own TQ2. Never commit, upload or
distribute extracted game data (`cache/` is git-ignored). Never modify the TQ2
install; only read it.

- TQ2 install: `C:\Program Files (x86)\Steam\steamapps\common\Titan Quest II`
  (build 24869641, UE 5.7.4 Grim fork, IoStore, unencrypted, Oodle, unversioned properties)
- Long-term: two-player co-op. Keep gameplay state separable from rendering.

## Layout

- `tools/UsmapDumper` – C# external reader of the running game's reflection data → `cache/Mappings.usmap`
- `tools/Tq2Extract` – C# CUE4Parse CLI: `list`, `dump`, `export`, `animate` (bakes anims + textures into .glb)
- `tools/re` – Python minidump helpers used to verify engine layouts
- `game/` – Rust + Bevy 0.20 game; loads `../cache/export/*.glb`
- `docs/` – ASSET_PIPELINE (formats, verified layouts, results), PROGRESS, DECISIONS, ARCHITECTURE

## Build / run

- Windows SDK 10.0.26100 + MSVC are required for Rust linking. Build Rust from
  **PowerShell**; Git Bash puts coreutils `link` before MSVC `link.exe`.
- `cd game; cargo run -- boarman/boarman_brute_animated.glb none_idle`
- `TQ2_SCREENSHOT=<png>` makes the viewer save a screenshot after 4 s and exit (use it to verify rendering).
- Asset pipeline commands: see `docs/ASSET_PIPELINE.md`.

## Current state

See `docs/PROGRESS.md` for the current milestone, blockers and next tasks.
Engine choice (Bevy vs UE5) is pending the user's confirmation; see `docs/DECISIONS.md`.

## Working rules

- Verify before claiming: render a screenshot, run the tool, measure FPS. Never invent numbers.
- Record verified engine-layout facts in `docs/ASSET_PIPELINE.md` (they differ from stock UE).
- The user launches TQ2 themselves when a live-memory step (usmap dump) is needed.
