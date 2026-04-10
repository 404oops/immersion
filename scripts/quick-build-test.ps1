# Quick build test script for Windows
# Run: powershell -ExecutionPolicy Bypass -File scripts/quick-build-test.ps1

Write-Host "🧪 Starting build test..." -ForegroundColor Cyan
Write-Host ""

# Step 1: Verify Qt static libraries
Write-Host "Step 1️⃣ : Verifying Qt6 static libraries..." -ForegroundColor Yellow
& powershell -ExecutionPolicy Bypass -File "scripts/verify-qt-static.ps1"
if ($LASTEXITCODE -ne 0) {
    Write-Host ""
    Write-Host "❌ Qt6 static libraries not found. Cannot proceed." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Step 2️⃣ : Checking required tools..." -ForegroundColor Yellow
$tools = @("cmake", "clang++", "ninja")
foreach ($tool in $tools) {
    $found = $null -ne (Get-Command $tool -ErrorAction SilentlyContinue)
    if ($found) {
        Write-Host "  ✅ $tool found" -ForegroundColor Green
    } else {
        Write-Host "  ❌ $tool not found" -ForegroundColor Red
        exit 1
    }
}

Write-Host ""
Write-Host "Step 3️⃣ : Running CMake configure..." -ForegroundColor Yellow
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$BuildDir = Join-Path $ProjectRoot "build"

if (Test-Path $BuildDir) {
    Write-Host "  Cleaning existing build..." -ForegroundColor Gray
    Remove-Item -Recurse -Force $BuildDir
}

New-Item -ItemType Directory -Path $BuildDir | Out-Null
Push-Location $BuildDir

try {
    cmake -G Ninja `
        -DCMAKE_C_COMPILER="C:/Qt/6.11.0/llvm-mingw_64/bin/clang.exe" `
        -DCMAKE_CXX_COMPILER="C:/Qt/6.11.0/llvm-mingw_64/bin/clang++.exe" `
        -DQt6_DIR="C:/Qt/6.11.0/llvm-mingw_64/lib/cmake/Qt6" `
        -DCMAKE_BUILD_TYPE=Release `
        "$ProjectRoot"

    if ($LASTEXITCODE -ne 0) {
        Write-Host ""
        Write-Host "❌ CMake configuration failed" -ForegroundColor Red
        exit 1
    }

    Write-Host ""
    Write-Host "Step 4️⃣ : Building..." -ForegroundColor Yellow
    cmake --build . --config Release

    if ($LASTEXITCODE -ne 0) {
        Write-Host ""
        Write-Host "❌ Build failed" -ForegroundColor Red
        exit 1
    }

    Write-Host ""
    Write-Host "✅ Build successful!" -ForegroundColor Green
    Write-Host "Executable: $(Join-Path $BuildDir "Immersion.exe")" -ForegroundColor Green
    exit 0
} finally {
    Pop-Location
}
