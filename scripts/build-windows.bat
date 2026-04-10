@echo off
REM Build script for Windows with CPack packaging (Batch version)
REM Usage:
REM   scripts\build-windows.bat [generator]
REM Examples:
REM   scripts\build-windows.bat
REM   scripts\build-windows.bat "Ninja"

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
set "AUTO_GENERATOR=0"

if not "%~1"=="" set "GENERATOR=%~1"

if not defined GENERATOR (
    set "GENERATOR=Ninja"
    set "AUTO_GENERATOR=1"
)

if defined GENERATOR (
    echo Generator: %GENERATOR%
    if "%AUTO_GENERATOR%"=="1" echo Generator selected automatically (clang workflow default)
) else (
    echo Generator: auto-detect
)
echo.

REM Check for required tools
where cmake >nul 2>nul
if errorlevel 1 (
    echo ❌ CMake not found. Please install CMake and add it to PATH
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
            if exist "C:\Qt\%%D\llvm-mingw_64\lib\cmake\Qt6\Qt6Config.cmake" set "Qt6_DIR=C:\Qt\%%D\llvm-mingw_64\lib\cmake\Qt6"
            if not defined Qt6_DIR if exist "C:\Qt\%%D\mingw_64\lib\cmake\Qt6\Qt6Config.cmake" set "Qt6_DIR=C:\Qt\%%D\mingw_64\lib\cmake\Qt6"
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

REM Resolve clang/clang++ compilers (Qt kit first, PATH fallback)
set "AUTO_COMPILER=0"
set "C_COMPILER=%CMAKE_C_COMPILER%"
set "CXX_COMPILER=%CMAKE_CXX_COMPILER%"

if defined Qt6_DIR (
    for %%I in ("%Qt6_DIR%\..\..\..") do set "QT_KIT_ROOT=%%~fI"
)

if not defined C_COMPILER (
    if defined QT_KIT_ROOT if exist "%QT_KIT_ROOT%\bin\clang.exe" set "C_COMPILER=%QT_KIT_ROOT%\bin\clang.exe"
)
if not defined CXX_COMPILER (
    if defined QT_KIT_ROOT if exist "%QT_KIT_ROOT%\bin\clang++.exe" set "CXX_COMPILER=%QT_KIT_ROOT%\bin\clang++.exe"
)

if not defined C_COMPILER (
    for /f "usebackq delims=" %%I in (`where clang 2^>nul`) do if not defined C_COMPILER set "C_COMPILER=%%I"
)
if not defined CXX_COMPILER (
    for /f "usebackq delims=" %%I in (`where clang++ 2^>nul`) do if not defined CXX_COMPILER set "CXX_COMPILER=%%I"
)

if not defined C_COMPILER (
    echo ❌ clang not found. Install LLVM/LLVM-MinGW or add clang to PATH.
    exit /b 1
)
if not defined CXX_COMPILER (
    echo ❌ clang++ not found. Install LLVM/LLVM-MinGW or add clang++ to PATH.
    exit /b 1
)

if defined QT_KIT_ROOT (
    if "%C_COMPILER%"=="%QT_KIT_ROOT%\bin\clang.exe" set "AUTO_COMPILER=1"
)

echo C compiler: %C_COMPILER%
echo CXX compiler: %CXX_COMPILER%
if "%AUTO_COMPILER%"=="1" echo Compilers selected automatically from Qt kit
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
    if defined Qt6_DIR (
        cmake -G "%GENERATOR%" -DCMAKE_C_COMPILER="%C_COMPILER%" -DCMAKE_CXX_COMPILER="%CXX_COMPILER%" -DQt6_DIR="%Qt6_DIR%" -DBUILD_SHARED_LIBS=OFF "%PROJECT_ROOT%"
    ) else (
        cmake -G "%GENERATOR%" -DCMAKE_C_COMPILER="%C_COMPILER%" -DCMAKE_CXX_COMPILER="%CXX_COMPILER%" -DBUILD_SHARED_LIBS=OFF "%PROJECT_ROOT%"
    )
) else (
    if defined Qt6_DIR (
        cmake -DCMAKE_C_COMPILER="%C_COMPILER%" -DCMAKE_CXX_COMPILER="%CXX_COMPILER%" -DQt6_DIR="%Qt6_DIR%" -DBUILD_SHARED_LIBS=OFF "%PROJECT_ROOT%"
    ) else (
        cmake -DCMAKE_C_COMPILER="%C_COMPILER%" -DCMAKE_CXX_COMPILER="%CXX_COMPILER%" -DBUILD_SHARED_LIBS=OFF "%PROJECT_ROOT%"
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
