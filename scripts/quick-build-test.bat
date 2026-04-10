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

REM Step 1: Check required tools
echo Step 1: Checking required tools...
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

REM Step 2: Configure
echo Step 2: Running CMake configure...
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
    -DBUILD_SHARED_LIBS=ON ^
    -DCMAKE_BUILD_TYPE=Release ^
    "%PROJECT_ROOT%"

if errorlevel 1 (
    echo.
    echo ❌ CMake configuration failed
    exit /b 1
)

echo.

REM Step 3: Build
echo Step 3: Building...
cmake --build . --config Release

if errorlevel 1 (
    echo.
    echo ❌ Build failed
    exit /b 1
)

REM Step 4: Deploy Qt runtime
echo.
echo Step 4: Deploying Qt runtime (windeployqt)...
set "WINDEPLOYQT=C:/Qt/6.11.0/llvm-mingw_64/bin/windeployqt.exe"
if not exist "%WINDEPLOYQT%" (
    set "WINDEPLOYQT="
    for /f "usebackq delims=" %%I in (`where windeployqt 2^>nul`) do if not defined WINDEPLOYQT set "WINDEPLOYQT=%%I"
)

if not defined WINDEPLOYQT (
    echo ❌ windeployqt not found
    exit /b 1
)

"%WINDEPLOYQT%" --release --qmldir "%PROJECT_ROOT%\src\qml" "%BUILD_DIR%\Immersion.exe"
if errorlevel 1 (
    echo ❌ windeployqt failed
    exit /b 1
)

echo.
echo ✅ Build successful!
echo Executable: %BUILD_DIR%\Immersion.exe
echo Qt runtime deployed next to executable.
exit /b 0
