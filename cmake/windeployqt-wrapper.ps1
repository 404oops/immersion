#!/usr/bin/env pwsh
# This script wraps windeployqt to properly set environment variables before execution
# It's called by CMake during the install phase

param(
    [Parameter(Mandatory=$true)]
    [string]$ExePath,
    
    [Parameter(Mandatory=$true)]
    [string]$QtRoot,
    
    [string]$ConfigFlag = "--release",
    [string]$QmlDir = "",
    [switch]$EnableVerbose = $false
)

Write-Host "windeployqt wrapper script starting..."
Write-Host "  ExePath: $ExePath"
Write-Host "  QtRoot: $QtRoot"
Write-Host "  Config: $ConfigFlag"

# Verify exe exists
if (!(Test-Path $ExePath)) {
    Write-Error "Executable not found: $ExePath"
    exit 1
}

# Set up Qt environment
$env:QT_PLUGIN_PATH = "$QtRoot/plugins"
$env:QT_QPA_PLATFORM_PLUGIN_PATH = "$QtRoot/plugins/platforms"
$env:QT_DEBUG_PLUGINS = "1"

Write-Host "Environment variables set:"
Write-Host "  QT_PLUGIN_PATH: $($env:QT_PLUGIN_PATH)"
Write-Host "  QT_QPA_PLATFORM_PLUGIN_PATH: $($env:QT_QPA_PLATFORM_PLUGIN_PATH)"

# Build windeployqt command
$windeployqt = "$QtRoot/bin/windeployqt.exe"
if (!(Test-Path $windeployqt)) {
    Write-Error "windeployqt not found at: $windeployqt"
    exit 1
}

$args = @(
    $ConfigFlag,
    "--no-translations",
    "--no-system-d3d-compiler",
    "--compiler-runtime"
)

if ($QmlDir) {
    $args += "--qmldir"
    $args += $QmlDir
}

if ($EnableVerbose) {
    $args += "--verbose"
    $args += "1"
}

$args += $ExePath

Write-Host "Running: $windeployqt $($args -join ' ')"
Write-Host ""

# Run windeployqt
& $windeployqt @args

if ($LASTEXITCODE -ne 0) {
    Write-Error "windeployqt failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

Write-Host ""
Write-Host "windeployqt completed successfully"
exit 0
