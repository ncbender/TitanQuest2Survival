# Titan Quest II Survivors

A private Survivors-style game built from the original Titan Quest II assets of
your own Steam installation. No game data is included in this repository; every
player extracts it from their own copy.

## Requirements

- Titan Quest II (Steam) at the default install path
- .NET 10 SDK, Rust (stable, MSVC toolchain), git
- Visual Studio 2022+ with the C++ workload and a Windows SDK

## One-time setup

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

Create a hero (starting weapon) in the main menu. Items from chests go to your stash when a run
ends; equip them in the **Armoury** and spend favour in the **Skill Tree** between runs. Your
progress is saved in `save/profile.json`. After pulling a new version, run the assets step again
(new weapons and animations).

See `docs/` for the asset pipeline, architecture decisions and progress.
