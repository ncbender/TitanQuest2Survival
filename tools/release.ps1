# Packs the prebuilt game for players without Rust / Visual Studio Build Tools:
# release/tq2-survivors-<version>.zip with game/target/release/tq2_survivors.exe and
# cache/natives/CUE4Parse-Natives.dll (the C++ animation decoder). Both are compiled from this
# repository and CUE4Parse; no Titan Quest II data is included.
#   powershell -ExecutionPolicy Bypass -File tools/release.ps1 -Version 0.3.0
param([Parameter(Mandatory)][string]$Version)
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\.."
Push-Location (Join-Path $root "game")
cargo build --release
if ($LASTEXITCODE -ne 0) { Pop-Location; throw "build failed" }
Pop-Location

$stage = Join-Path $root "release\stage"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force "$stage\game\target\release", "$stage\cache\natives" | Out-Null
Copy-Item (Join-Path $root "game\target\release\tq2_survivors.exe") "$stage\game\target\release\"
Copy-Item (Join-Path $root "cache\natives\CUE4Parse-Natives.dll") "$stage\cache\natives\"
$zip = Join-Path $root "release\tq2-survivors-$Version.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path "$stage\*" -DestinationPath $zip
Remove-Item -Recurse -Force $stage
Write-Host "wrote $zip"
