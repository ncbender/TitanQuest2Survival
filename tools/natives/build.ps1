# Builds CUE4Parse-Natives.dll (ACL animation decompression) into cache/natives/, where
# tools/Tq2Extract picks it up. Without it, ACL-compressed TQ2 animations are skipped.
# Requires Visual Studio (C++ workload, Windows SDK) and git. Run from PowerShell.
$ErrorActionPreference = "Stop"
$tag = "1.2.2.202610"   # must match the CUE4Parse NuGet version in Tq2Extract.csproj
$root = Resolve-Path "$PSScriptRoot\..\.."
$work = Join-Path $env:TEMP "cue4parse-natives-$tag"
$out = Join-Path $root "cache\natives"

if (-not (Test-Path $work)) {
    git clone --depth 1 --branch $tag https://github.com/FabianFG/CUE4Parse.git $work
    # Only ACL and its rtm dependency are needed (the nested test submodules are not).
    git -C $work submodule update --init --depth 1 CUE4Parse-Natives/ACL/external/acl
    git -C "$work\CUE4Parse-Natives\ACL\external\acl" submodule update --init --depth 1 external/rtm
}

$vs = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -property installationPath
$n = "$work\CUE4Parse-Natives"
New-Item -ItemType Directory -Force $out | Out-Null
# Same defines/includes as the project's CMakeLists; static CRT so the DLL runs without the VC++ redistributable.
$cmd = @"
call "$vs\VC\Auxiliary\Build\vcvars64.bat" >nul && cd /d "$out" && cl /nologo /LD /O2 /MT /EHsc /std:c++14 /DNDEBUG /DWIN_EXPORT /DWITH_ACL /I"$n\ACL\external\acl\includes" /I"$n\ACL\external\acl\external\rtm\includes" /I"$n\common" /I"$n\ACL" "$n\ACL\ACL.cpp" "$n\Features.cpp" /Fe:CUE4Parse-Natives.dll
"@
cmd /c $cmd
if (-not (Test-Path "$out\CUE4Parse-Natives.dll")) { throw "build failed" }
Remove-Item "$out\*.obj", "$out\*.exp", "$out\*.lib" -ErrorAction SilentlyContinue
Write-Host "built $out\CUE4Parse-Natives.dll"
