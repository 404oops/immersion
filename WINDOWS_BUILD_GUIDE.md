# Windows Build Quick Start Guide

This guide will help you build Immersion with static linking enabled.

## Prerequisites

1. **Qt 6.11.0 with Static Libraries**
   - Download from [qt.io](https://www.qt.io/download) using the offline installer
   - Or install via vcpkg: `vcpkg install qt6:x64-windows-static`

2. **Build Tools**
   - CMake (download from cmake.org or `winget install cmake`)
   - Ninja (download or `winget install ninja`)
   - LLVM/Clang (included in Qt llvm-mingw_64 kit or from LLVM project)

## Step 1: Verify Qt Static Libraries

Run the verification script to confirm your Qt installation has static libraries:

**PowerShell:**
```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify-qt-static.ps1
```

**Batch:**
```cmd
scripts\verify-qt-static.ps1
```

Expected output:
```
✅ All required Qt6 static libraries found!
Ready to build with static linking.
```

If you see missing libraries, install the Qt static build variant.

## Step 2: Run the Build Test

This will configure and build the project from scratch.

**PowerShell:**
```powershell
cd C:\Users\404oops\Documents\immersion
powershell -ExecutionPolicy Bypass -File scripts/quick-build-test.ps1
```

**Batch:**
```cmd
cd C:\Users\404oops\Documents\immersion
scripts\quick-build-test.bat
```

## Step 3: Use the Built Executable

After a successful build:
- Executable location: `build\Immersion.exe`
- This is a fully static binary with no Qt DLLs required
- Should work on any Windows machine, even without Qt installed

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

### Qt Static Libraries Not Found
```
❌ Some Qt6 static libraries are missing!
```
**Solution:** Install Qt6 static build from qt.io or via `vcpkg install qt6:x64-windows-static`

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
- Verify Qt static libraries exist: `dir C:\Qt\6.11.0\llvm-mingw_64\lib` (should show `.a` files)
- Clean and rebuild: `rmdir /s /q build` then run build script again
- Check CMake configuration output for ABI mismatches

## Next Steps

For more build options and details, see `scripts/BUILD.md`
