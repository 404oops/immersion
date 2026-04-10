# Immersion - Music Project Versioning

A Qt6-based music project management application with dynamic Qt runtime deployment.

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
- Qt6 (with deployment tools: `macdeployqt` and `windeployqt`)
- Clang/Clang++ (LLVM)
- Ninja build system

## Features

- Dynamic Qt linking with automated runtime deployment
- Qt6 GUI with QML support
- Cross-platform (Windows, macOS, Linux)
- C++20 standard

## Troubleshooting

### "QtCore.dll not found" Error
If you get this error when running the executable, Qt runtime files were not deployed next to the executable.

Quick fix:
1. Rebuild using: `powershell -ExecutionPolicy Bypass -File scripts/quick-build-test.ps1`
2. Ensure `windeployqt` exists in your Qt kit or PATH
3. Confirm Qt DLLs are in the same folder as `Immersion.exe`

### macOS Runtime Bundling
The macOS build script runs `macdeployqt` and places Qt runtime files in the app bundle:
- `Immersion.app/Contents/Frameworks`
- `Immersion.app/Contents/Resources`

Copyright, 404oops (c)