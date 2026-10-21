# Updates and starts Titan Quest II Survivors: pulls the latest version, re-extracts the game
# assets from your own TQ2 install when the asset tools changed, builds and runs the game.
#   powershell -ExecutionPolicy Bypass -File update.ps1            update + play
#   powershell -ExecutionPolicy Bypass -File update.ps1 -NoStart   update only
#   powershell -ExecutionPolicy Bypass -File update.ps1 -Assets    force asset re-extraction
# Reads the TQ2 install only; nothing outside this folder is changed.
param([switch]$NoStart, [switch]$Assets)
$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
Set-Location $root

function Need($cmd, $hint) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) { throw "$cmd not found. $hint" }
}
Need git "Install Git for Windows: https://git-scm.com/download/win"
Need dotnet "Install the .NET 10 SDK: https://dotnet.microsoft.com/download"
Need cargo "Install Rust: https://rustup.rs (default MSVC toolchain)"

Write-Host "== Pulling the latest version" -ForegroundColor Yellow
git pull --ff-only
if ($LASTEXITCODE -ne 0) { throw "git pull failed (local changes?). Run 'git status' to see them." }

$usmap = Join-Path $root "cache\Mappings.usmap"
if (-not (Test-Path $usmap)) {
    Write-Host "== Mappings missing. Start Titan Quest II, wait in its main menu, then press Enter." -ForegroundColor Yellow
    Read-Host | Out-Null
    dotnet run --project tools/UsmapDumper -c Release
    if (-not (Test-Path $usmap)) { throw "UsmapDumper did not write cache\Mappings.usmap" }
    Write-Host "You can close Titan Quest II now."
}

if (-not (Test-Path (Join-Path $root "cache\natives\CUE4Parse-Natives.dll"))) {
    Write-Host "== Building the animation decoder" -ForegroundColor Yellow
    powershell -ExecutionPolicy Bypass -File tools/natives/build.ps1
}

# Assets only need re-extracting when the extractor or its asset list changed.
$stamp = Join-Path $root "cache\assets.stamp"
$toolsRev = (git log -1 --format=%H -- tools/Tq2Extract tools/assets.json tools/kits.json)
$done = if (Test-Path $stamp) { (Get-Content $stamp -Raw).Trim() } else { "" }
if ($Assets -or $toolsRev -ne $done) {
    Write-Host "== Extracting assets from your TQ2 install (takes a while)" -ForegroundColor Yellow
    Push-Location tools/Tq2Extract
    dotnet run -c Release -- assets
    $ok = $LASTEXITCODE -eq 0
    Pop-Location
    if (-not $ok) { throw "asset extraction failed" }
    Set-Content -Path $stamp -Value $toolsRev -Encoding ascii
} else {
    Write-Host "== Assets are up to date"
}

Write-Host "== Building the game" -ForegroundColor Yellow
Push-Location game
cargo build --release
$ok = $LASTEXITCODE -eq 0
if ($ok -and -not $NoStart) {
    Write-Host "== Starting" -ForegroundColor Green
    cargo run --release
}
Pop-Location
if (-not $ok) { throw "build failed" }
