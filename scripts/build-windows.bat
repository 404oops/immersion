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

REM Resolve Qt6 package directory (for Qt6Config.cmake)
set "AUTO_QT6_DIR=0"
if defined Qt6_DIR (
    if exist "%Qt6_DIR%\Qt6Config.cmake" (
        set "Qt6_DIR=%Qt6_DIR%"
    ) else (
        if exist "%Qt6_DIR%\lib\cmake\Qt6\Qt6Config.cmake" set "Qt6_DIR=%Qt6_DIR%\lib\cmake\Qt6"
    )
)

if not defined Qt6_DIR (
    if defined QTDIR (
        if exist "%QTDIR%\lib\cmake\Qt6\Qt6Config.cmake" (
            set "Qt6_DIR=%QTDIR%\lib\cmake\Qt6"
            set "AUTO_QT6_DIR=1"
        )
    )
)

if not defined Qt6_DIR (
    for /f "delims=" %%D in ('dir /b /ad /o-n "C:\Qt\6.*" 2^>nul') do (
        if not defined Qt6_DIR (
            if exist "C:\Qt\%%D\msvc2022_64\lib\cmake\Qt6\Qt6Config.cmake" set "Qt6_DIR=C:\Qt\%%D\msvc2022_64\lib\cmake\Qt6"
            if not defined Qt6_DIR if exist "C:\Qt\%%D\msvc2019_64\lib\cmake\Qt6\Qt6Config.cmake" set "Qt6_DIR=C:\Qt\%%D\msvc2019_64\lib\cmake\Qt6"
            if not defined Qt6_DIR if exist "C:\Qt\%%D\msvc2022_arm64\lib\cmake\Qt6\Qt6Config.cmake" set "Qt6_DIR=C:\Qt\%%D\msvc2022_arm64\lib\cmake\Qt6"
            if not defined Qt6_DIR if exist "C:\Qt\%%D\clang_64\lib\cmake\Qt6\Qt6Config.cmake" set "Qt6_DIR=C:\Qt\%%D\clang_64\lib\cmake\Qt6"
        )
    )
    if defined Qt6_DIR set "AUTO_QT6_DIR=1"
)

if defined Qt6_DIR (
    echo Qt6_DIR: %Qt6_DIR%
    if "%AUTO_QT6_DIR%"=="1" echo Qt6_DIR selected automatically
) else (
    echo Qt6_DIR: not found automatically
    echo Hint: set Qt6_DIR to a folder containing Qt6Config.cmake
)
echo.

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
            cmake -G "%GENERATOR%" -T "%TOOLSET%" -DQt6_DIR="%Qt6_DIR%" "%PROJECT_ROOT%"
        ) else (
            cmake -G "%GENERATOR%" -T "%TOOLSET%" "%PROJECT_ROOT%"
        )
    ) else (
        if defined Qt6_DIR (
            cmake -G "%GENERATOR%" -DQt6_DIR="%Qt6_DIR%" "%PROJECT_ROOT%"
        ) else (
            cmake -G "%GENERATOR%" "%PROJECT_ROOT%"
        )
    )
) else (
    if defined Qt6_DIR (
        cmake -DQt6_DIR="%Qt6_DIR%" "%PROJECT_ROOT%"
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
