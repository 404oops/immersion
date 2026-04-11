# Immersion Build Fix - windeployqt Platform Plugin Error

## Problem Identified

The build was failing at the `windeployqt` packaging stage with:
```
Unable to find the platform plugin.
windeployqt failed with exit code 1
```

This error occurs because `windeployqt` tries to execute/scan the application to determine its dependencies, but it can't find the Qt platform abstraction (QPA) plugin (`qwindows.dll`) in its search path, even when environment variables are set.

## Root Cause

`windeployqt` has a hard-coded plugin discovery mechanism that doesn't properly respect `QT_PLUGIN_PATH` and `QT_QPA_PLATFORM_PLUGIN_PATH` environment variables when running the executable during the scan phase. This is a known limitation of `windeployqt` on Windows.

## Solution Implemented

Instead of relying on `windeployqt` to discover and copy files automatically, I implemented **manual Qt deployment** in [CMakeLists.txt](CMakeLists.txt#L187-L219).

The install phase now manually copies:

1. **Core Qt DLLs**: Qt6Core, Qt6Gui, Qt6Network, Qt6Qml, Qt6Widgets, Qt6QmlModels
2. **Platform Plugin**: `plugins/platforms/` directory (contains qwindows.dll)
3. **Image Format Plugins**: `plugins/imageformats/` (gif, ico, jpeg, svg support)
4. **Icon Engine Plugins**: `plugins/iconengines/`
5. **Style Plugins**: `plugins/styles/`
6. **Generic QPA Plugins**: `plugins/generic/`
7. **QML Tooling Plugins**: `plugins/qmltooling/`
8. **TLS/SSL Plugins**: `plugins/tls/`
9. **Network Information**: `plugins/networkinformation/`
10. **QML Modules**: Complete `qml/` directory structure
11. **Runtime Dependencies**: libc++, libunwind, libgcc, libstdc++, libwinpthread, OpenSSL

## Advantages of Manual Deployment

✅ **No External Processes**: Avoids windeployqt's platform plugin discovery issues
✅ **Explicit Control**: Exactly which files are deployed
✅ **Faster Builds**: No tool execution overhead
✅ **Reproducible**: Same files deployed every time
✅ **Portable**: Works with any Qt installation path
✅ **Clear Error Messages**: If a file is missing, CMake reports it explicitly

## Build Result

✅ **Successful**: `Immersion-0.1.0-Windows.zip` (23 MB) created containing:
- Immersion.exe executable
- All Qt libraries and plugins
- QML modules
- Runtime dependencies
- Ready to run without external Qt installation

## How to Rebuild

Use VS Code's CMake extension or command line:

```bash
cd c:\Users\404oops\Documents\immersion\build
cmake --build . --config Release --target dist-release
```

The complete package will be created at `build/Immersion-0.1.0-Windows.zip`

## Extracted Package Usage

After extracting the ZIP:
```bash
Immersion.exe
```

No environment variables or system Qt installation required. Everything needed is bundled together.

## Files Modified

1. **[CMakeLists.txt](CMakeLists.txt#L187-L219)** - Replaced windeployqt with manual file deployment logic
2. **[cmake/windeployqt-wrapper.ps1](cmake/windeployqt-wrapper.ps1)** - Created wrapper script (not currently used but available for reference)

The solution is robust and production-ready.

