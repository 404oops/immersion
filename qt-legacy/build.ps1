#!/usr/bin/env pwsh
# Simple build script for Immersion project

$projectDir = $PSScriptRoot
$buildDir = "$projectDir\build"
$cmake = "C:\Program Files\Microsoft Visual Studio\18\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
$clangcxx = "C:\Qt\Tools\llvm-mingw1706_64\bin\clang++.exe"
$clangc = "C:\Qt\Tools\llvm-mingw1706_64\bin\clang.exe"
$Qt6Dir = "C:\Qt\6.11.0\llvm-mingw_64\lib\cmake\Qt6"

Write-Host "Building Immersion with explicit compiler paths..."

# Clean cache if exists
if (Test-Path "$buildDir\CMakeCache.txt") {
    Remove-Item "$buildDir\CMakeCache.txt" -Force
}
if (Test-Path "$buildDir\CMakeFiles") {
    Remove-Item "$buildDir\CMakeFiles" -Recurse -Force
}

# Configure with explicit C and CXX compilers.
# NOTE: must be a MinGW-style generator; the project rejects the MSVC ABI,
# so Visual Studio generators cannot be used.
New-Item -ItemType Directory -Force -Path $buildDir | Out-Null
Set-Location $buildDir

Write-Host "Running cmake configure..."
& $cmake `
    -G "MinGW Makefiles" `
    -DCMAKE_BUILD_TYPE=Release `
    -DCMAKE_C_COMPILER="$clangc" `
    -DCMAKE_CXX_COMPILER="$clangcxx" `
    -DQt6_DIR="$Qt6Dir" `
    -DCMAKE_MAKE_PROGRAM="mingw32-make" `
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
Write-Host "Creating package (runs windeployqt via the install step)..."
& $cmake --build . --config Release --target dist-release

if ($LASTEXITCODE -ne 0) {
    Write-Host "Packaging failed"
    exit 1
}

Write-Host "Build completed successfully"
