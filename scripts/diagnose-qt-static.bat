@echo off
REM Diagnose Qt static library configuration on Windows (Batch version)

setlocal enabledelayedexpansion

set QtPath=C:\Qt\6.11.0\llvm-mingw_64
if not "%1"=="" set QtPath=%1

echo.
echo === Qt Static Library Diagnostic ===
echo Qt Path: %QtPath%
echo.

REM Check for static .a files
echo Checking for static libraries (.a files)...
set foundStatic=0
set foundShared=0

if exist "%QtPath%\lib\libQt6Core.a" (
    echo   ✓ libQt6Core.a
    set foundStatic=1
) else (
    echo   ✗ libQt6Core.a not found
)

if exist "%QtPath%\lib\libQt6Gui.a" (
    echo   ✓ libQt6Gui.a
    set foundStatic=1
) else (
    echo   ✗ libQt6Gui.a not found
)

if exist "%QtPath%\lib\libQt6Widgets.a" (
    echo   ✓ libQt6Widgets.a
    set foundStatic=1
) else (
    echo   ✗ libQt6Widgets.a not found
)

echo.
echo Checking for shared libraries (.dll files)...

if exist "%QtPath%\bin\Qt6Core.dll" (
    echo   !! Qt6Core.dll found in bin/
    set foundShared=1
)

if exist "%QtPath%\bin\Qt6Gui.dll" (
    echo   !! Qt6Gui.dll found in bin/
    set foundShared=1
)

if exist "%QtPath%\bin\Qt6Widgets.dll" (
    echo   !! Qt6Widgets.dll found in bin/
    set foundShared=1
)

echo.
echo Diagnostic Summary:
if %foundStatic% equ 1 (
    echo ✓ Static libraries (.a files) are present
) else (
    echo ✗ Static libraries (.a files) NOT FOUND
    echo   You need a Qt kit with static build (llvm-mingw_64 or mingw_64)
)

if %foundShared% equ 1 (
    if %foundStatic% equ 1 (
        echo ⚠ WARNING: Both static (.a) and shared (.dll) libraries found^^!
        echo   CMake should prefer .a files due to CMAKE_FIND_LIBRARY_SUFFIXES
    )
)

REM Check build artifacts
echo.
echo Checking build artifacts for static link verification...
set BuildDir=%cd%\build
if exist "%BuildDir%\CMakeFiles\Immersion.dir\link.txt" (
    echo Build artifacts found - checking linker flags...
    setlocal enabledelayedexpansion
    for /f "tokens=*" %%A in (%BuildDir%\CMakeFiles\Immersion.dir\link.txt) do (
        set LinkLine=%%A
        if "!LinkLine!"=="-static" echo ✓ Contains -static flag
    )
) else (
    echo Build directory not found - run build first
)

echo.
echo For more information, see: QTDLL_ERROR_FIX.md
