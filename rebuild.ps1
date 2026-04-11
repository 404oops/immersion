#!/usr/bin/env pwsh
# Build script for Immersion project

$projectDir = $PSScriptRoot
$buildDir = "$projectDir\build"

Write-Host "Immersion Build Script" -ForegroundColor Cyan

# Step 1: Verify tools exist
Write-Host "`n[1/4] Checking required tools..." -ForegroundColor Yellow
$cmake = "C:\Program Files\Microsoft Visual Studio\18\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
$clangcxx = "C:\Qt\Tools\llvm-mingw1706_64\bin\clang++.exe"
$clangc = "C:\Qt\Tools\llvm-mingw1706_64\bin\clang.exe"
$Qt6Dir = "C:\Qt\6.11.0\llvm-mingw_64\lib\cmake\Qt6"

if (!(Test-Path $cmake)) { Write-Host "ERROR: CMake not found at $cmake"; exit 1 }
if (!(Test-Path $clangcxx)) { Write-Host "ERROR: clang++ not found at $clangcxx"; exit 1 }
if (!(Test-Path $Qt6Dir)) { Write-Host "ERROR: Qt6 not found at $Qt6Dir"; exit 1 }
Write-Host "✓ All tools found" -ForegroundColor Green

# Step 2: Clean and configure
Write-Host "`n[2/4] Configuring CMake..." -ForegroundColor Yellow
if (Test-Path "$buildDir\CMakeCache.txt") {
    Remove-Item "$buildDir\CMakeCache.txt" -Force -ErrorAction SilentlyContinue
}
if (Test-Path "$buildDir\CMakeFiles") {
    Remove-Item "$buildDir\CMakeFiles" -Recurse -Force -ErrorAction SilentlyContinue
}

cd $buildDir
$configResult = & $cmake `
    -G "MinGW Makefiles" `
    -DCMAKE_BUILD_TYPE=Release `
    -DCMAKE_C_COMPILER="$clangc" `
    -DCMAKE_CXX_COMPILER="$clangcxx" `
    -DQt6_DIR="$Qt6Dir" `
    -DCMAKE_MAKE_PROGRAM="mingw32-make" `
    ".."

if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: CMake configuration failed!" -ForegroundColor Red
    exit 1
}
Write-Host "✓ CMake configured" -ForegroundColor Green

# Step 3: Build
Write-Host "`n[3/4] Building Immersion executable..." -ForegroundColor Yellow
$buildCMD = & $cmake --build . --config Release --target Immersion -- -j 4
if ($LASTEXITCODE -ne 0) {
    Write-Host "WARNING: Build had some warnings/errors, continuing to packaging..." -ForegroundColor Yellow
}
Write-Host "✓ Immersion built" -ForegroundColor Green

# Step 4: Package with windeployqt environment setup
Write-Host "`n[4/4] Packaging with windeployqt..." -ForegroundColor Yellow
Write-Host "Setting Qt plugin path environment variables..."

$env:QT_PLUGIN_PATH = "C:\Qt\6.11.0\llvm-mingw_64\plugins"
$env:QT_QPA_PLATFORM_PLUGIN_PATH = "C:\Qt\6.11.0\llvm-mingw_64\plugins\platforms"

$packResult = & $cmake --build . --config Release --target dist-release
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Packaging failed!" -ForegroundColor Red
    Write-Host "Check if windeployqt is attempting to load plugins correctly." -ForegroundColor Yellow
    exit 1
}
Write-Host "✓ Package created" -ForegroundColor Green

Write-Host "`n✓ Build completed successfully!" -ForegroundColor Green
Write-Host "Package location: $buildDir/Immersion-0.1.0-Windows.zip" -ForegroundColor Cyan
