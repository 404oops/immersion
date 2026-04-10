# Diagnose Qt static library configuration and check if it's static or shared
param(
    [string]$QtPath = "C:/Qt/6.11.0/llvm-mingw_64"
)

Write-Host "=== Qt Static Library Diagnostic ===" -ForegroundColor Cyan
Write-Host "Qt Path: $QtPath`n" -ForegroundColor Gray

# Check for static .a files
$staticLibs = @("libQt6Core.a", "libQt6Gui.a", "libQt6Widgets.a")
$foundStatic = $false
$foundShared = $false

Write-Host "Checking for static libraries (.a files)..." -ForegroundColor Yellow
foreach ($lib in $staticLibs) {
    $libPath = Join-Path $QtPath "lib" $lib
    if (Test-Path $libPath) {
        Write-Host "  ✓ $lib" -ForegroundColor Green
        $foundStatic = $true
    } else {
        Write-Host "  ✗ $lib not found at $libPath" -ForegroundColor Red
    }
}

Write-Host "`nChecking for shared libraries (.dll files)..." -ForegroundColor Yellow
$sharedLibs = @("Qt6Core.dll", "Qt6Gui.dll", "Qt6Widgets.dll")
foreach ($lib in $sharedLibs) {
    $dllPath = Join-Path $QtPath "bin" $lib
    if (Test-Path $dllPath) {
        Write-Host "  !! $lib found at bin/" -ForegroundColor Yellow
        $foundShared = $true
    }
}

Write-Host "`nDiagnostic Summary:" -ForegroundColor Cyan
if ($foundStatic) {
    Write-Host "✓ Static libraries (.a files) are present" -ForegroundColor Green
} else {
    Write-Host "✗ Static libraries (.a files) NOT FOUND" -ForegroundColor Red
    Write-Host "  You need a Qt kit with static build (llvm-mingw_64 or mingw_64)" -ForegroundColor Yellow
}

if ($foundShared -and $foundStatic) {
    Write-Host "⚠ WARNING: Both static (.a) and shared (.dll) libraries found!" -ForegroundColor Yellow
    Write-Host "  CMake should prefer .a files due to CMAKE_FIND_LIBRARY_SUFFIXES" -ForegroundColor Yellow
    Write-Host "  If build still uses DLLs, you may need to manually remove shared libraries" -ForegroundColor Yellow
}

# Check build artifacts
Write-Host "`nChecking build artifacts for static link verification..." -ForegroundColor Yellow
$buildDir = Split-Path -Parent $PSScriptRoot | Join-Path -ChildPath "build"
if (Test-Path $buildDir) {
    $linkCmd = Get-Content "$buildDir/CMakeFiles/Immersion.dir/link.txt" -ErrorAction SilentlyContinue
    if ($linkCmd) {
        Write-Host "Link command found in build directory" -ForegroundColor Green
        Write-Host "Link flags:" -ForegroundColor Gray
        if ($linkCmd -match "-static") {
            Write-Host "  ✓ Contains -static flag" -ForegroundColor Green
        } else {
            Write-Host "  ✗ Missing -static flag" -ForegroundColor Red
        }
        if ($linkCmd -match "\.a") {
            Write-Host "  ✓ Links .a (static) libraries" -ForegroundColor Green
        } elseif ($linkCmd -match "\.dll\.a" -or $linkCmd -match "Qt6.*\.a") {
            Write-Host "  ✓ Uses import libraries" -ForegroundColor Yellow
        }
    } else {
        Write-Host "Build artifacts not found - run build first" -ForegroundColor Gray
    }
} else {
    Write-Host "Build directory not found - run build first" -ForegroundColor Gray
}
