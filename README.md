# Titan Quest II Survivors

A private Survivors-style game built from the original Titan Quest II assets of
your own Steam installation. No game data is included in this repository; every
player extracts it from their own copy.

## Requirements

- Titan Quest II (Steam) at the default install path
- .NET 10 SDK, Rust (stable, MSVC toolchain), git
- Build Tools for Visual Studio 2022 (free) with "Desktop development with C++" (MSVC linker +
  Windows SDK, which Rust uses on Windows to link the .exe; also compiles the C++ animation
  decoder). The full Visual Studio IDE is not needed; `rustup` offers to install them.

## Quick start (install and updates)

```
git clone <repository url> "Titan quest Survival"
cd "Titan quest Survival"
powershell -ExecutionPolicy Bypass -File update.ps1
```

`update.ps1` does everything below: it pulls the newest version, asks you once to start TQ2 for
the mappings, extracts the assets from your own install (again only when the asset tools
changed), builds and starts the game. Run it again for every update. `-NoStart` only updates,
`-Assets` forces a fresh extraction. Your save (`save/`) and the extracted data (`cache/`) stay
local and are never uploaded.

## Without Rust (prebuilt game)

Download the latest `tq2-survivors-<version>.zip` from the repository's Releases page and unpack
it into the repository folder (it holds `game/target/release/tq2_survivors.exe` and
`cache/natives/CUE4Parse-Natives.dll`). Then only Git and the .NET 10 SDK are needed:
`update.ps1` skips building and starts the prebuilt game. Unpack each new release to update the
game; the asset extraction still comes from your own TQ2 install.

## One-time setup (manual)

All commands from PowerShell in the repository root.

1. **Mappings** – start Titan Quest II, wait in the main menu, then run
   `dotnet run --project tools/UsmapDumper -c Release` and close the game.
   (Only reads the game's memory; nothing is installed or changed. Repeat after TQ2 updates.)
2. **Animation decoder** – `powershell -File tools/natives/build.ps1`
3. **Assets** – `cd tools/Tq2Extract; dotnet run -- assets` (a few minutes; writes `cache/export`)

## Play

```
cd game
cargo run --release
```

WASD move · Q/E rotate camera · mouse wheel zoom · 1–3 / click pick an upgrade · Esc pause · R restart

Create a hero (starting weapon and a TQ2 mastery) in the main menu. Items from chests go to your
stash when a run ends; equip them in the **Armoury** and spend active/passive points on TQ2's
skills in **Masteries** between runs. Your progress is saved in `save/profile.json`. After
pulling a new version, run the assets step again (new weapons, animations, masteries, UI).

See `docs/` for the asset pipeline, architecture decisions and progress.
