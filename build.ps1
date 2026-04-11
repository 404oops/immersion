#!/usr/bin/env pwsh
# Simple build script for Immersion project

$projectDir = $PSScriptRoot
$buildDir = "$projectDir\build"
$cmake = "C:\Program Files\Microsoft Visual Studio\18\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
$clangcxx = "C:\Qt\Tools\llvm-mingw1706_64\bin\clang++.exe"
$clangc = "C:\Qt\Tools\llvm-mingw1706_64\bin\clang.exe"
$Qt6Dir = "C:\Qt\6.11.0\llvm-mingw_64\lib\cmake\Qt6"

Write-Host "Building Immersion with explicit compiler paths..."

# Set environment for Qt plugins
$env:QT_PLUGIN_PATH = "C:\Qt\6.11.0\llvm-mingw_64\plugins"
$env:QT_QPA_PLATFORM_PLUGIN_PATH = "C:\Qt\6.11.0\llvm-mingw_64\plugins\platforms"

# Clean cache if exists
if (Test-Path "$buildDir\CMakeCache.txt") {
    Remove-Item "$buildDir\CMakeCache.txt" -Force
}
if (Test-Path "$buildDir\CMakeFiles") {
    Remove-Item "$buildDir\CMakeFiles" -Recurse -Force
}

# Configire with explicit C and CXX compilers
cd $buildDir

Write-Host "Running cmake configure..."
& $cmake `
    -G "Visual Studio 18 2026" `
    -A x64 `
    -DCMAKE_C_COMPILER="$clangc" `
    -DCMAKE_CXX_COMPILER="$clangcxx" `
    -DQt6_DIR="$Qt6Dir" `
    ".."

if ($LASTEXITCODE -ne 0) {
    Write-Host "Configuration failed"
    exit 1
}

# Build executable
Write-Host "Building executable..."
& $cmake --build . --config Release

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed"
    exit 1
}

# Create package
Write-Host "Creating package with windeployqt..."
& $cmake --build . --config Release --target dist-release

if ($LASTEXITCODE -ne 0) {
    Write-Host "Packaging failed"
    exit 1
}

Write-Host "Build completed successfully"
