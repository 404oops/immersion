# Building musit

This directory contains platform-specific build scripts for musit that handle CMake configuration, compilation, and CPack packaging.

## Quick Start

### macOS
```bash
bash scripts/build-macos.sh
```
Creates: `musit-0.1.0-Darwin.dmg`

### Linux
```bash
bash scripts/build-linux.sh
```
Creates: `musit-0.1.0-Linux.tar.gz`, `.deb`, `.rpm` packages

### Windows (PowerShell - Recommended)
```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-windows.ps1
```
Creates: `musit-0.1.0-Windows.exe`, `.msi`

### Windows (Batch)
```cmd
scripts\build-windows.bat
```
Creates: `musit-0.1.0-Windows.exe`, `.msi`

### Auto-Detect Build
```bash
bash scripts/build.sh
```
Automatically detects your OS and runs the appropriate build script.

## Prerequisites

### macOS
- Xcode Command Line Tools: `xcode-select --install`
- CMake: `brew install cmake`
- Qt6: `brew install qt6`

### Linux (Ubuntu/Debian)
```bash
sudo apt install cmake qt6-base-dev qt6-qml-dev qt6-tools-dev libgl1-mesa-dev
```

### Linux (Fedora/RHEL)
```bash
sudo dnf install cmake qt6-qtbase-devel qt6-qtdeclarative-devel libglvnd-devel
```

### Windows
- Visual Studio 2022 (or 2019) with C++ workload
- CMake (download from cmake.org or `winget install cmake`)
- Qt6 (download from qt.io or `vcpkg install qt6`)

## Build Output

All platforms clean and rebuild from scratch. Output artifacts are in the `build/` directory:

| Platform | Artifacts | Installation |
|----------|-----------|--------------|
| **macOS** | `.dmg` | Drag `.app` to `/Applications` |
| **Linux** | `.tar.gz`, `.deb`, `.rpm` | See script output for platform-specific instructions |
| **Windows** | `.exe`, `.msi` | Run installer (admin required for MSI) |

## Advanced Options

### Windows (PowerShell)
```powershell
# Specify build type
.\scripts\build-windows.ps1 -BuildType Debug

# Specify generator
.\scripts\build-windows.ps1 -Generator "Ninja"
```

### Custom Qt Installation
Set environment variables before running the build script:

**macOS:**
```bash
export Qt6_DIR=/path/to/qt6
bash scripts/build-macos.sh
```

**Linux:**
```bash
export Qt6_DIR=/path/to/qt6
bash scripts/build-linux.sh
```

**Windows (PowerShell):**
```powershell
$env:Qt6_DIR = "C:\path\to\qt6"
.\scripts\build-windows.ps1
```

## Manual Build (without scripts)

If scripts don't work for your setup:

```bash
mkdir build && cd build
cmake -DCMAKE_BUILD_TYPE=Release ..
cmake --build . -j$(nproc)
cpack
```

## Troubleshooting

### CMake Configuration Fails
- Ensure Qt6 is installed and in PATH
- Check that Qt6 is version 6.2 or later
- On Linux, verify `qmake6` or `qmake` is available

### Build Fails
- Clean build: `rm -rf build && bash scripts/build-[platform].sh`
- Check compiler compatibility (C++20 required)
- Ensure all dependencies are installed

### CPack Fails
- macOS: Requires `bzip2` (`brew install bzip2`)
- Linux: Additional packages may be needed for DEB/RPM creation
- Windows: Ensure NSIS is installed for `.exe` creation

## CI/CD Integration

Use the universal `build.sh` script in CI/CD pipelines:

```bash
bash scripts/build.sh
```

This works across all platforms and handles OS detection automatically.

## Build Configuration

Edit `CMakeLists.txt` to modify:
- Build type (MinSizeRel, Release, Debug)
- Packaging generators (DragNDrop, NSIS, TGZ, DEB, RPM)
- Installation paths
- Compiler optimizations
