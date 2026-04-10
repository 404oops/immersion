# Windows Build Quick Start Guide

This guide will help you build Immersion with dynamic Qt linking and bundled runtime files.

## Prerequisites

1. **Qt 6.11.0 with Deployment Tools**
   - Install Qt from [qt.io](https://www.qt.io/download)
   - Ensure `windeployqt.exe` is available in your Qt kit's `bin` directory

2. **Build Tools**
   - CMake (download from cmake.org or `winget install cmake`)
   - Ninja (download or `winget install ninja`)
   - LLVM/Clang (included in Qt llvm-mingw_64 kit or from LLVM project)

## Step 1: Run the Build Test

This will configure and build the project from scratch.

**PowerShell:**
```powershell
cd C:\Users\404oops\Documents\musit
powershell -ExecutionPolicy Bypass -File scripts/quick-build-test.ps1
```

**Batch:**
```cmd
cd C:\Users\404oops\Documents\musit
scripts\quick-build-test.bat
```

The build test now does all of this automatically:
1. Checks toolchain availability (`cmake`, `clang++`, `ninja`)
2. Configures and builds with CMake
3. Runs `windeployqt` to copy Qt DLLs/plugins/QML runtime

## Step 2: Use the Built Executable

After a successful build:
- Executable location: `build\Immersion.exe`
- Qt runtime files are deployed next to the executable
- It should launch without requiring global Qt installation

## Full Build (Release)

For a production build with optimizations:

**PowerShell:**
```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-windows.ps1 -BuildType Release
```

**Batch:**
```cmd
scripts\build-windows.bat
```

## VS Code Integration

If using VS Code with CMake extension:
1. Open the project folder
2. CMake Tools will automatically use `.vscode/settings.json`
3. Click "Configure" or run `CMake: Configure`
4. Click "Build" or press Ctrl+Shift+B

## Troubleshooting

### QtCore.dll Not Found
```
The code execution cannot proceed because Qt6Core.dll was not found.
```
**Solution:**
- Re-run `scripts/quick-build-test.ps1` to force deployment
- Check `windeployqt` exists in your Qt kit (`...\bin\windeployqt.exe`)
- Confirm Qt DLLs are present next to `build\Immersion.exe`

### cmake/clang++/ninja Not Found
```
❌ cmake not found
```
**Solution:** 
- CMake: Download from cmake.org or `winget install cmake`
- Clang: Included in Qt kit or install LLVM
- Ninja: Download or `winget install ninja`

Ensure each tool is in your PATH or provide full path in scripts.

### Qt6_DIR Points to Wrong Location
Edit `.vscode/settings.json` or environment variable:
```powershell
$env:Qt6_DIR = "C:\Qt\6.11.0\llvm-mingw_64\lib\cmake\Qt6"
```

### Build Fails with Linker Errors
- Verify compiler and Qt kit ABI match
- Clean and rebuild: `rmdir /s /q build` then run build script again
- Check CMake configuration output for ABI mismatches

## Next Steps

For more build options and details, see `scripts/BUILD.md`
