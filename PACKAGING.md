# Immersion Packaging

This project is configured with CMake + CPack for desktop artifacts.

## macOS

Build app bundle and DMG:

```bash
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j
cpack --config build/CPackConfig.cmake
```

Expected outputs:
- `build/musit.app`
- `musit-<version>-Darwin.dmg`

## Windows

Configure with a Qt-enabled toolchain (MSVC or MinGW), then build and package:

```powershell
cmake -S . -B build -G "Ninja" -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release
cpack --config build/CPackConfig.cmake
```

Expected output:
- `musit-<version>-Windows.exe` (NSIS installer)

Notes:
- NSIS must be installed and available on PATH for installer generation.
- Qt runtime deployment is required for release distribution.

## Linux

Not configured yet by design. Add a Linux CPack generator later (for example `DEB` or `TGZ`).
