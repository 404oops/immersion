# Immersion - Music Project Versioning

A Qt6-based music project management application with automatic version
snapshots for DAW project files (Bitwig, Ableton, FL Studio, Reaper, ...).

## Quick Start (Windows)

```powershell
powershell -ExecutionPolicy Bypass -File rebuild.ps1
```

Both `build.ps1` and `rebuild.ps1` expect Qt (llvm-mingw kit) under `C:\Qt`;
adjust the paths at the top of the scripts for your machine.

## Quick Start (macOS / Linux)

```bash
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j
cmake --build build --target dist   # optional: create a package
```

## Build Requirements

- CMake 3.21+
- Qt6 (with deployment tools: `macdeployqt` and `windeployqt`)
- Clang/Clang++ (LLVM). On Windows, the MinGW ABI is required; MSVC is
  rejected by the build.
- Ninja or Make

## Features

- Automatic snapshot versioning of DAW project files, with a branching
  version graph and per-version notes
- Dynamic Qt linking with automated runtime deployment
- Qt6 GUI with QML support
- Cross-platform (Windows, macOS, Linux)
- C++20 standard

## Utilities

- `scripts/reset-musit-data.sh` — removes Immersion's app config/state
  (project `.musit` version history is untouched).

## Troubleshooting

### "QtCore.dll not found" / platform plugin errors (Windows)
Qt runtime files were not deployed next to the executable. Re-run the
packaging step (`cmake --build build --target dist-release`), which invokes
`windeployqt`, and confirm the Qt DLLs plus the `platforms/` folder sit next
to `Immersion.exe`.

### macOS Runtime Bundling
The install step runs `macdeployqt` and places Qt runtime files in the app
bundle under `Immersion.app/Contents/Frameworks` and
`Immersion.app/Contents/Resources`.

Copyright, 404oops (c)
