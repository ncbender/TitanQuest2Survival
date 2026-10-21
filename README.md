# Titan Quest II Survivors

A private Survivors-style game built from the original Titan Quest II assets of
your own Steam installation. No game data is included in this repository.

## Requirements

- Titan Quest II (Steam) installed at the default path
- .NET 10 SDK, Rust (stable, MSVC), Visual Studio Build Tools with Windows SDK

## Setup

1. Start Titan Quest II and stay in the main menu, then run
   `dotnet run --project tools/UsmapDumper -c Release` (creates `cache/Mappings.usmap`). Close the game.
2. Export assets: see `docs/ASSET_PIPELINE.md` (commands for `tools/Tq2Extract`).
3. Run the game from PowerShell: `cd game; cargo run`.
