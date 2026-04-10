# Build script for Windows with CPack packaging (PowerShell version)
# Run with: powershell -ExecutionPolicy Bypass -File build-windows.ps1

param(
    [ValidateSet("Debug", "Release", "RelWithDebInfo")]
    [string]$BuildType = "Release",

    [string]$Generator = "Ninja"
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$BuildDir = Join-Path $ProjectRoot "build"
$autoGenerator = $false
$autoQt6Dir = $false
$autoCompiler = $false

if ([string]::IsNullOrWhiteSpace($Generator)) {
    $Generator = "Ninja"
    $autoGenerator = $true
}

function Resolve-Qt6PackageDir {
    param(
        [string]$Qt6DirEnv,
        [string]$QtDirEnv
    )

    if (-not [string]::IsNullOrWhiteSpace($Qt6DirEnv)) {
        if (Test-Path (Join-Path $Qt6DirEnv "Qt6Config.cmake")) {
            return $Qt6DirEnv
        }
        $nested = Join-Path $Qt6DirEnv "lib\cmake\Qt6"
        if (Test-Path (Join-Path $nested "Qt6Config.cmake")) {
            return $nested
        }
    }

    if (-not [string]::IsNullOrWhiteSpace($QtDirEnv)) {
        $qtDirCandidate = Join-Path $QtDirEnv "lib\cmake\Qt6"
        if (Test-Path (Join-Path $qtDirCandidate "Qt6Config.cmake")) {
            return $qtDirCandidate
        }
    }

    if (Test-Path "C:\Qt") {
        $qtVersions = Get-ChildItem -Path "C:\Qt" -Directory -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -like "6.*" } |
            Sort-Object Name -Descending

        foreach ($versionDir in $qtVersions) {
            $kits = @("llvm-mingw_64", "mingw_64", "msvc2022_64", "msvc2019_64", "msvc2022_arm64", "clang_64")
            foreach ($kit in $kits) {
                $candidate = Join-Path $versionDir.FullName "$kit\lib\cmake\Qt6"
                if (Test-Path (Join-Path $candidate "Qt6Config.cmake")) {
                    return $candidate
                }
            }
        }
    }

    return $null
}

function Resolve-ClangCompilers {
    param(
        [string]$Qt6PackageDir
    )

    $resolvedC = $env:CMAKE_C_COMPILER
    $resolvedCxx = $env:CMAKE_CXX_COMPILER
    $fromQtKit = $false

    if (($resolvedC -and $resolvedCxx) -and (Test-Path $resolvedC) -and (Test-Path $resolvedCxx)) {
        return @($resolvedC, $resolvedCxx, $fromQtKit)
    }

    if (-not [string]::IsNullOrWhiteSpace($Qt6PackageDir)) {
        $qtKitRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $Qt6PackageDir))
        $qtClang = Join-Path $qtKitRoot "bin\clang.exe"
        $qtClangxx = Join-Path $qtKitRoot "bin\clang++.exe"
        if ((Test-Path $qtClang) -and (Test-Path $qtClangxx)) {
            $resolvedC = $qtClang
            $resolvedCxx = $qtClangxx
            $fromQtKit = $true
            return @($resolvedC, $resolvedCxx, $fromQtKit)
        }
    }

    if (-not $resolvedC) {
        $clang = Get-Command clang -ErrorAction SilentlyContinue
        if ($clang) {
            $resolvedC = $clang.Source
        }
    }

    if (-not $resolvedCxx) {
        $clangxx = Get-Command clang++ -ErrorAction SilentlyContinue
        if ($clangxx) {
            $resolvedCxx = $clangxx.Source
        }
    }

    return @($resolvedC, $resolvedCxx, $fromQtKit)
}

$qt6PackageDir = Resolve-Qt6PackageDir -Qt6DirEnv $env:Qt6_DIR -QtDirEnv $env:QTDIR
if ($qt6PackageDir -and [string]::IsNullOrWhiteSpace($env:Qt6_DIR)) {
    $autoQt6Dir = $true
}

$compilerResolution = Resolve-ClangCompilers -Qt6PackageDir $qt6PackageDir
$cCompiler = $compilerResolution[0]
$cxxCompiler = $compilerResolution[1]
$autoCompiler = [bool]$compilerResolution[2]

if ([string]::IsNullOrWhiteSpace($cCompiler)) {
    Write-Host "❌ clang not found. Install LLVM/LLVM-MinGW or add clang to PATH" -ForegroundColor Red
    exit 1
}

if ([string]::IsNullOrWhiteSpace($cxxCompiler)) {
    Write-Host "❌ clang++ not found. Install LLVM/LLVM-MinGW or add clang++ to PATH" -ForegroundColor Red
    exit 1
}

Write-Host "🪟 Building Immersion for Windows (PowerShell)" -ForegroundColor Cyan
Write-Host "Project root: $ProjectRoot"
Write-Host "Build directory: $BuildDir"
Write-Host "Build type: $BuildType"
if ([string]::IsNullOrWhiteSpace($Generator)) {
    Write-Host "Generator: auto-detect"
} else {
    Write-Host "Generator: $Generator"
    if ($autoGenerator) {
        Write-Host "Generator selected automatically (clang workflow default)"
    }
}
if (-not [string]::IsNullOrWhiteSpace($qt6PackageDir)) {
    Write-Host "Qt6_DIR: $qt6PackageDir"
    if ($autoQt6Dir) {
        Write-Host "Qt6_DIR selected automatically"
    }
} else {
    Write-Host "Qt6_DIR: not found automatically"
    Write-Host "Hint: set Qt6_DIR to a folder containing Qt6Config.cmake"
}
Write-Host "C compiler: $cCompiler"
Write-Host "CXX compiler: $cxxCompiler"
if ($autoCompiler) {
    Write-Host "Compilers selected automatically from Qt kit"
}
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
        "-DCMAKE_BUILD_TYPE=$BuildType",
        "-DCMAKE_C_COMPILER=$cCompiler",
        "-DCMAKE_CXX_COMPILER=$cxxCompiler",
        "-DBUILD_SHARED_LIBS=OFF"
    )

    if (-not [string]::IsNullOrWhiteSpace($Generator)) {
        $cmakeArgs += @("-G", $Generator)
    }
    
    if ($qt6PackageDir) {
        $cmakeArgs += "-DQt6_DIR=$qt6PackageDir"
    }
    
    $cmakeArgs += $ProjectRoot
    
    & cmake @cmakeArgs
    if ($LASTEXITCODE -ne 0) {
        throw "CMake configuration failed"
    }

    # Build
    Write-Host ""
    Write-Host "🔨 Building Immersion..." -ForegroundColor Yellow
    & cmake --build . --config $BuildType -- /M:$env:NUMBER_OF_PROCESSORS
    if ($LASTEXITCODE -ne 0) {
        throw "Build failed"
    }

    # Deploy Qt runtime for dynamic linking
    Write-Host ""
    Write-Host "🚚 Deploying Qt runtime (windeployqt)..." -ForegroundColor Yellow
    $exePath = Join-Path $BuildDir "Immersion.exe"
    if (-not (Test-Path $exePath)) {
        throw "Built executable not found: $exePath"
    }

    $qtKitRoot = $null
    if ($qt6PackageDir) {
        $qtKitRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $qt6PackageDir))
    }

    $windeployqt = $null
    if ($qtKitRoot) {
        $candidate = Join-Path $qtKitRoot "bin\windeployqt.exe"
        if (Test-Path $candidate) {
            $windeployqt = $candidate
        }
    }
    if (-not $windeployqt) {
        $cmd = Get-Command windeployqt -ErrorAction SilentlyContinue
        if ($cmd) {
            $windeployqt = $cmd.Source
        }
    }

    if (-not $windeployqt) {
        throw "windeployqt not found. Install Qt tools or add windeployqt to PATH."
    }

    & $windeployqt --$($BuildType.ToLower()) --qmldir "$ProjectRoot\src\qml" "$exePath"
    if ($LASTEXITCODE -ne 0) {
        throw "windeployqt failed"
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
