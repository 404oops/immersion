# Reset Immersion user data (config, saved folders, etc.)
# Does NOT delete project .musit folders (version history is preserved)
#
# Config location matches ProjectRegistry::appConfigDirectory():
#   %APPDATA%\musit\musit

$ErrorActionPreference = "Stop"

Write-Host "Immersion User Data Reset"
Write-Host "========================="
Write-Host ""
Write-Host "This removes Immersion configuration and app state."
Write-Host "Project version history (.musit folders in projects) is NOT affected."
Write-Host ""

$dataDir = Join-Path $env:APPDATA "musit\musit"
$cacheDir = Join-Path $env:LOCALAPPDATA "musit\musit"
$legacyDir = Join-Path $env:APPDATA "musit"
$buggyDir = Join-Path $env:APPDATA "musit\musit\musit"

Write-Host "Target directories:"
Write-Host "  Data:  $dataDir"
Write-Host "  Cache: $cacheDir"
if ((Test-Path $legacyDir) -and ($legacyDir -ne $dataDir)) {
    Write-Host "  Legacy: $legacyDir"
}
Write-Host ""

$confirm = Read-Host "Are you sure? (Type 'yes' to confirm)"
if ($confirm -ne "yes") {
    Write-Host "Cancelled."
    exit 0
}

foreach ($dir in @($dataDir, $cacheDir, $legacyDir, $buggyDir)) {
    if ($dir -and (Test-Path $dir)) {
        Write-Host "Removing $dir..."
        Remove-Item -Recurse -Force $dir
    }
}

Write-Host ""
Write-Host "Reset complete. Immersion will show onboarding on next launch."
