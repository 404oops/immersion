@echo off
REM Build script for Windows with CPack packaging (Batch version)
REM Usage:
REM   scripts\build-windows.bat [generator] [toolset]
REM Examples:
REM   scripts\build-windows.bat
REM   scripts\build-windows.bat "Visual Studio 18 2026" v180

setlocal enabledelayedexpansion

REM Get script directory
for %%I in ("%~dp0.") do set "SCRIPT_DIR=%%~fI"
for %%I in ("%SCRIPT_DIR%\..") do set "PROJECT_ROOT=%%~fI"
set "BUILD_DIR=%PROJECT_ROOT%\build"

echo 🪟 Building Immersion for Windows...
echo Project root: %PROJECT_ROOT%
echo Build directory: %BUILD_DIR%
echo.

set "GENERATOR=%CMAKE_GENERATOR%"
set "TOOLSET=%CMAKE_GENERATOR_TOOLSET%"
set "AUTO_TOOLSET=0"
set "AUTO_GENERATOR=0"

if not "%~1"=="" set "GENERATOR=%~1"
if not "%~2"=="" set "TOOLSET=%~2"

if not defined GENERATOR (
    set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
    if exist "%VSWHERE%" (
        for /f "usebackq delims=" %%V in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion`) do set "VS_VERSION=%%V"
        if defined VS_VERSION (
            for /f "tokens=1 delims=." %%M in ("%VS_VERSION%") do set "VS_MAJOR=%%M"
            if "%VS_MAJOR%"=="18" set "GENERATOR=Visual Studio 18 2026"
            if "%VS_MAJOR%"=="17" set "GENERATOR=Visual Studio 17 2022"
            if "%VS_MAJOR%"=="16" set "GENERATOR=Visual Studio 16 2019"
            if defined GENERATOR set "AUTO_GENERATOR=1"
        )
    )
)

if not defined TOOLSET (
    where clang-cl >nul 2>nul
    if not errorlevel 1 (
        if defined GENERATOR (
            echo %GENERATOR% | findstr /B /C:"Visual Studio" >nul
            if not errorlevel 1 (
                set "TOOLSET=ClangCL"
                set "AUTO_TOOLSET=1"
            )
        )
    )
)

if defined GENERATOR (
    echo Generator: %GENERATOR%
    if "%AUTO_GENERATOR%"=="1" echo Generator selected automatically (latest Visual Studio detected)
) else (
    echo Generator: auto-detect
)

if defined TOOLSET (
    echo Toolset: %TOOLSET%
    if "%AUTO_TOOLSET%"=="1" echo Toolset selected automatically (clang-cl detected)
)
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
if defined GENERATOR (
    if defined TOOLSET (
        if defined Qt6_DIR (
            cmake -G "%GENERATOR%" -T "%TOOLSET%" -DCMAKE_PREFIX_PATH="%Qt6_DIR%" "%PROJECT_ROOT%"
        ) else (
            cmake -G "%GENERATOR%" -T "%TOOLSET%" "%PROJECT_ROOT%"
        )
    ) else (
        if defined Qt6_DIR (
            cmake -G "%GENERATOR%" -DCMAKE_PREFIX_PATH="%Qt6_DIR%" "%PROJECT_ROOT%"
        ) else (
            cmake -G "%GENERATOR%" "%PROJECT_ROOT%"
        )
    )
) else (
    if defined Qt6_DIR (
        cmake -DCMAKE_PREFIX_PATH="%Qt6_DIR%" "%PROJECT_ROOT%"
    ) else (
        cmake "%PROJECT_ROOT%"
    )
)

if errorlevel 1 (
    echo ❌ CMake configuration failed
    exit /b 1
)

REM Build
echo.
echo 🔨 Building Immersion...
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
