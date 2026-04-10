@echo off
REM Build script for Windows with CPack packaging (Batch version)

setlocal enabledelayedexpansion

REM Get script directory
for %%I in ("%~dp0.") do set "SCRIPT_DIR=%%~fI"
for %%I in ("%SCRIPT_DIR%..") do set "PROJECT_ROOT=%%~fI"
set "BUILD_DIR=%PROJECT_ROOT%\build"

echo 🪟 Building musit for Windows...
echo Project root: %PROJECT_ROOT%
echo Build directory: %BUILD_DIR%
echo.

REM Check for required tools
where cmake >nul 2>nul
if errorlevel 1 (
    echo ❌ CMake not found. Please install CMake and add it to PATH
    exit /b 1
)

where msbuild >nul 2>nul
if errorlevel 1 (
    echo ❌ MSBuild not found. Please install Visual Studio Build Tools or Visual Studio
    exit /b 1
)

REM Create or clean build directory
if exist "%BUILD_DIR%" (
    echo Cleaning existing build directory...
    rmdir /s /q "%BUILD_DIR%"
)
mkdir "%BUILD_DIR%"

cd /d "%BUILD_DIR%"

REM Configure with CMake
echo.
echo 📋 Configuring CMake...
cmake -G "Visual Studio 17 2022" ^
       -DCMAKE_PREFIX_PATH="%Qt6_DIR%" ^
       "%PROJECT_ROOT%"

if errorlevel 1 (
    echo ❌ CMake configuration failed
    exit /b 1
)

REM Build
echo.
echo 🔨 Building musit...
cmake --build . --config Release

if errorlevel 1 (
    echo ❌ Build failed
    exit /b 1
)

REM Package
echo.
echo 📦 Creating Windows installer...
cpack -C Release

if errorlevel 1 (
    echo ❌ Packaging failed
    exit /b 1
)

echo.
echo ✅ Build complete!
echo.
echo Artifacts:
for %%F in (musit-*.exe musit-*.msi musit-*.zip) do (
    if exist "%%F" echo   %%F
)
echo.
echo Installation:
echo   - Run the .exe or .msi installer
echo   - Administrator privileges may be required
pause
