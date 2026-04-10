# Immersion - Music Project Versioning

A Qt6-based music project management application with static linking support.

## Quick Start (Windows)

```powershell
powershell -ExecutionPolicy Bypass -File scripts/quick-build-test.ps1
```

Or with Batch:

```cmd
scripts\quick-build-test.bat
```

## Full Documentation

- **Windows:** See [WINDOWS_BUILD_GUIDE.md](WINDOWS_BUILD_GUIDE.md)
- **All platforms:** See [scripts/BUILD.md](scripts/BUILD.md)
- **In Progress:** The app is still under development

## Build Requirements

- CMake 3.21+
- Qt6 with **static libraries**
- Clang/Clang++ (LLVM)
- Ninja build system

## Features

- Fully static linking (no runtime dependencies)
- Qt6 GUI with QML support
- Cross-platform (Windows, macOS, Linux)
- C++20 standard

## Troubleshooting

### "QtCore.dll not found" Error
If you get this error when running the executable, it means the build used dynamic linking instead of static. See **[QTDLL_ERROR_FIX.md](QTDLL_ERROR_FIX.md)** for complete diagnostic steps and solutions.

Quick fix:
1. Run: `powershell -ExecutionPolicy Bypass -File scripts/diagnose-qt-static.ps1`
2. Verify static `.a` libraries exist in your Qt kit
3. Clean and rebuild: Delete the `build/` folder first, then rebuild

Copyright, 404oops (c)