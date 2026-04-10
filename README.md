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

Copyright, 404oops (c)