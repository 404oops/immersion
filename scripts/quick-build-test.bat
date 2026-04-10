@echo off
REM Quick build test script for Windows (Batch version)
REM Run: scripts\quick-build-test.bat

setlocal enabledelayedexpansion

for %%I in ("%~dp0.") do set "SCRIPT_DIR=%%~fI"
for %%I in ("%SCRIPT_DIR%\..") do set "PROJECT_ROOT=%%~fI"
set "BUILD_DIR=%PROJECT_ROOT%\build"

echo.
echo 🧪 Starting build test...
echo.

REM Step 1: Verify Qt static libraries
echo Step 1: Verifying Qt6 static libraries...
setlocal
setlocal enabledelayedexpansion
set "QT_LIB_DIR=C:\Qt\6.11.0\llvm-mingw_64\lib"

if not exist "%QT_LIB_DIR%" (
    echo ❌ Qt lib directory not found: %QT_LIB_DIR%
    exit /b 1
)

set "REQUIRED_LIBS=libQt6Core.a libQt6Gui.a libQt6Widgets.a libQt6Qml.a libQt6Quick.a libQt6QuickControls2.a"
set "MISSING_LIBS=0"

for %%L in (%REQUIRED_LIBS%) do (
    if exist "%QT_LIB_DIR%\%%L" (
        echo   ✅ Found: %%L
    ) else (
        echo   ❌ Missing: %%L
        set "MISSING_LIBS=1"
    )
)

if "%MISSING_LIBS%"=="1" (
    echo.
    echo ❌ Some Qt6 static libraries are missing!
    echo Your Qt installation appears to be shared-only (DLL-based).
    echo.
    echo To fix, install Qt6 static build:
    echo.
    echo Option 1: vcpkg
    echo   vcpkg install qt6:x64-windows-static
    echo.
    echo Option 2: Qt Official Installer
    echo   Download from qt.io and select 'Desktop (static)' variant
    echo.
    exit /b 1
)

echo ✅ All required Qt6 static libraries found!
echo.

REM Step 2: Check required tools
echo Step 2: Checking required tools...
where cmake >nul 2>nul
if errorlevel 1 (
    echo ❌ cmake not found
    exit /b 1
)
echo   ✅ cmake found

where clang++ >nul 2>nul
if errorlevel 1 (
    echo ❌ clang++ not found
    exit /b 1
)
echo   ✅ clang++ found

where ninja >nul 2>nul
if errorlevel 1 (
    echo ❌ ninja not found
    exit /b 1
)
echo   ✅ ninja found

echo.

REM Step 3: Configure
echo Step 3: Running CMake configure...
if exist "%BUILD_DIR%" (
    echo   Cleaning existing build...
    rmdir /s /q "%BUILD_DIR%" >nul 2>nul
)
mkdir "%BUILD_DIR%"

cd /d "%BUILD_DIR%"

cmake -G Ninja ^
    -DCMAKE_C_COMPILER="C:/Qt/6.11.0/llvm-mingw_64/bin/clang.exe" ^
    -DCMAKE_CXX_COMPILER="C:/Qt/6.11.0/llvm-mingw_64/bin/clang++.exe" ^
    -DQt6_DIR="C:/Qt/6.11.0/llvm-mingw_64/lib/cmake/Qt6" ^
    -DCMAKE_BUILD_TYPE=Release ^
    "%PROJECT_ROOT%"

if errorlevel 1 (
    echo.
    echo ❌ CMake configuration failed
    exit /b 1
)

echo.

REM Step 4: Build
echo Step 4: Building...
cmake --build . --config Release

if errorlevel 1 (
    echo.
    echo ❌ Build failed
    exit /b 1
)

echo.
echo ✅ Build successful!
echo Executable: %BUILD_DIR%\Immersion.exe
exit /b 0
