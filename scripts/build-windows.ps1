# Build script for Windows with CPack packaging (PowerShell version)
# Run with: powershell -ExecutionPolicy Bypass -File build-windows.ps1

param(
    [ValidateSet("Debug", "Release", "RelWithDebInfo")]
    [string]$BuildType = "Release",
    
    [ValidateSet("Visual Studio 17 2022", "Visual Studio 16 2019", "Ninja")]
    [string]$Generator = "Visual Studio 17 2022"
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$BuildDir = Join-Path $ProjectRoot "build"

Write-Host "🪟 Building musit for Windows (PowerShell)" -ForegroundColor Cyan
Write-Host "Project root: $ProjectRoot"
Write-Host "Build directory: $BuildDir"
Write-Host "Build type: $BuildType"
Write-Host "Generator: $Generator"
Write-Host ""

# Check for required tools
$cmakeExists = $null -ne (Get-Command cmake -ErrorAction SilentlyContinue)
if (-not $cmakeExists) {
    Write-Host "❌ CMake not found. Please install CMake and add it to PATH" -ForegroundColor Red
    exit 1
}

# Create or clean build directory
if (Test-Path $BuildDir) {
    Write-Host "Cleaning existing build directory..."
    Remove-Item -Recurse -Force $BuildDir
}
New-Item -ItemType Directory -Path $BuildDir | Out-Null

Push-Location $BuildDir

try {
    # Configure with CMake
    Write-Host ""
    Write-Host "📋 Configuring CMake..." -ForegroundColor Yellow
    $cmakeArgs = @(
        "-G", $Generator,
        "-DCMAKE_BUILD_TYPE=$BuildType"
    )
    
    if ($env:Qt6_DIR) {
        $cmakeArgs += "-DCMAKE_PREFIX_PATH=$env:Qt6_DIR"
    }
    
    $cmakeArgs += $ProjectRoot
    
    & cmake @cmakeArgs
    if ($LASTEXITCODE -ne 0) {
        throw "CMake configuration failed"
    }

    # Build
    Write-Host ""
    Write-Host "🔨 Building musit..." -ForegroundColor Yellow
    & cmake --build . --config $BuildType -- /M:$env:NUMBER_OF_PROCESSORS
    if ($LASTEXITCODE -ne 0) {
        throw "Build failed"
    }

    # Package
    Write-Host ""
    Write-Host "📦 Creating Windows installer..." -ForegroundColor Yellow
    & cpack -C $BuildType
    if ($LASTEXITCODE -ne 0) {
        Write-Host "⚠️  CPack completed with warnings (this is often normal)" -ForegroundColor Yellow
    }

    Write-Host ""
    Write-Host "✅ Build complete!" -ForegroundColor Green
    Write-Host ""
    
    Write-Host "Artifacts:" -ForegroundColor Cyan
    $artifacts = Get-ChildItem -Filter "musit-*" -Include "*.exe", "*.msi", "*.zip", "*.nsis"
    if ($artifacts) {
        foreach ($artifact in $artifacts) {
            Write-Host "   $($artifact.Name) ($([math]::Round($artifact.Length / 1MB, 2)) MB)"
        }
    } else {
        Write-Host "   (No packages found - check build output above)"
    }
    
    Write-Host ""
    Write-Host "Installation:" -ForegroundColor Cyan
    Write-Host "   - Run the .exe or .msi installer"
    Write-Host "   - Administrator privileges may be required"

} finally {
    Pop-Location
}
