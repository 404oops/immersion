# Post-build Windows packaging: builds immersion.exe and compiles the NSIS
# installer into dist\ImmersionSetup-<version>-windows-<architecture>.exe.
#
# Usage: powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1 [-Production]
#   -Production  package the `production` cargo profile build (fat LTO,
#                stripped) — use this for distribution.
#
# Requires NSIS (makensis on PATH): https://nsis.sourceforge.io or
# `winget install NSIS.NSIS`.
param(
    [switch]$Production,
    [ValidateSet("x64", "arm64")]
    [string]$Architecture = $(if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" })
)

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $root

$buildProfile = if ($Production) { "production" } else { "release" }
$rustTarget = if ($Architecture -eq "arm64") { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }

$version = (Select-String -Path "Cargo.toml" -Pattern '^version = "(.*)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value

Write-Host "Building Immersion $version ($buildProfile)..."
cargo build --profile $buildProfile --target $rustTarget --locked -p immersion
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (-not (Get-Command makensis -ErrorAction SilentlyContinue)) {
    Write-Error "makensis not found on PATH. Install NSIS: winget install NSIS.NSIS"
}

New-Item -ItemType Directory -Force -Path "dist" | Out-Null
$exeSource = Join-Path $root "target\$rustTarget\$buildProfile\immersion.exe"
$outFile = Join-Path $root "dist\ImmersionSetup-$version-windows-$Architecture.exe"

makensis `
    "/DVERSION=$version" `
    "/DEXE_SOURCE=$exeSource" `
    "/DLICENSE_SOURCE=$(Join-Path $root 'LICENSE')" `
    "/DOUTFILE=$outFile" `
    (Join-Path $root "packaging\nsis\Immersion.nsi")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Packaged $outFile"
