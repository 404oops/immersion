# Verify Qt6 static libraries are installed
# Run: powershell -ExecutionPolicy Bypass -File scripts/verify-qt-static.ps1

param(
    [string]$QtDir = "C:\Qt\6.11.0\llvm-mingw_64"
)

Write-Host "🔍 Verifying Qt6 static libraries at: $QtDir" -ForegroundColor Cyan
Write-Host ""

$libDir = Join-Path $QtDir "lib"

if (-not (Test-Path $libDir)) {
    Write-Host "❌ Qt lib directory not found: $libDir" -ForegroundColor Red
    exit 1
}

$requiredLibs = @(
    "libQt6Core.a",
    "libQt6Gui.a",
    "libQt6Widgets.a",
    "libQt6Qml.a",
    "libQt6Quick.a",
    "libQt6QuickControls2.a"
)

$foundAll = $true

foreach ($lib in $requiredLibs) {
    $libPath = Join-Path $libDir $lib
    if (Test-Path $libPath) {
        Write-Host "✅ Found: $lib" -ForegroundColor Green
    } else {
        Write-Host "❌ Missing: $lib" -ForegroundColor Red
        $foundAll = $false
    }
}

Write-Host ""
if ($foundAll) {
    Write-Host "✅ All required Qt6 static libraries found!" -ForegroundColor Green
    Write-Host "Ready to build with static linking." -ForegroundColor Green
    exit 0
} else {
    Write-Host "❌ Some Qt6 static libraries are missing!" -ForegroundColor Red
    Write-Host ""
    Write-Host "Your Qt installation appears to be shared-only (DLL-based)." -ForegroundColor Yellow
    Write-Host "To fix, install Qt6 static build:" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Option 1: vcpkg (recommended)" -ForegroundColor Cyan
    Write-Host "  vcpkg install qt6:x64-windows-static" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Option 2: Qt Official Installer" -ForegroundColor Cyan
    Write-Host "  Download from qt.io and select 'Desktop (static)' variant" -ForegroundColor Gray
    Write-Host ""
    exit 1
}
