# Static Linking Fix - April 11, 2026

## Issue
The built `Immersion.exe` failed at runtime with error:
```
The code execution cannot proceed because QtCore.dll was not found.
```

This indicated the executable was dynamically linked to Qt instead of having Qt statically compiled in.

## Root Cause
While the CMakeLists.txt and build scripts were configured for static linking, `find_package(Qt6)` could not distinguish between static `.a` libraries and shared library import `.lib` files on Windows.

When both static and shared libraries exist in a Qt kit (which is common), CMake's default library search order was finding `.lib` (import libraries for DLLs) before `.a` (static libraries), causing dynamic linking.

## Solution Implemented

### 1. CMakeLists.txt - Library Suffix Prioritization
**Lines 117-120:**
```cmake
if(WIN32 AND NOT MSVC)
    set(CMAKE_FIND_LIBRARY_SUFFIXES ".a" ".lib" ".dll")
    message(STATUS "Configured CMAKE_FIND_LIBRARY_SUFFIXES for static linking...")
endif()
```
Forces CMake to search for `.a` (static) files before `.lib` (import) files.

### 2. CMakeLists.txt - Qt6 Static Configuration
**Lines 63-70:**
```cmake
set(Qt6_SHARED_LIBS OFF)
set(Qt6_USE_STATIC_LIBS ON)
set(Qt6_NO_UNINSTALLED_BUILD_SUPPORT ON)
```
Explicitly tells Qt6 CMake modules to prefer static libraries.

### 3. CMakeLists.txt - Static Library Verification
**Lines 122-136:**
Checks if Qt6Core.a (static) exists and validates the build is using static libraries, not shared DLLs.

### 4. CMakeLists.txt - Target-Level Static Linking
**Lines 193-196:**
```cmake
if(WIN32 AND NOT MSVC)
    target_link_options(Immersion PRIVATE -static -static-libgcc -static-libstdc++)
endif()
```
Applies `-static` flag directly to the Immersion target to ensure full static linkage.

### 5. Build Scripts Update
- `quick-build-test.ps1`: Added Step 5 to run diagnostic script after successful build
- Diagnostic output helps verify whether static or shared libraries were actually linked

### 6. New Diagnostic Tools
- **scripts/diagnose-qt-static.ps1** (PowerShell): Checks if Qt static libraries exist and validates build configuration
- **scripts/diagnose-qt-static.bat** (Batch): Batch version of diagnostic script
- Both run automatically after successful build completion

### 7. Documentation
- **QTDLL_ERROR_FIX.md**: Comprehensive troubleshooting guide including:
  - How to verify Qt has static libraries
  - How to install Qt with static builds
  - Step-by-step recovery process
  - Advanced troubleshooting (manual linking, objdump analysis)
  - Understanding static vs dynamic linking
  
- **README.md**: Added Troubleshooting section pointing to QTDLL_ERROR_FIX.md

## How This Fixes the Issue

The combination of:
1. **Library suffix ordering** ensures `.a` files are found first
2. **Qt6 configuration variables** direct the find_package system toward static libraries
3. **Static library verification** fails early with clear error if static libs don't exist
4. **Target-level linker flags** enforce static linking at the final linking stage
5. **Diagnostic scripts** help users verify the build actually used static linking

When the user rebuilds:
1. CMake will prioritize `.a` files when searching for Qt libraries
2. find_package(Qt6) will bind to static `.a` libraries instead of shared libs
3. The linker will enforce `-static` flag to disable dynamic linking
4. Result: Executable is fully self-contained with no external Qt DLL dependencies

## Testing

To verify the fix works:

```powershell
# Clean build from scratch
rm -r build/
powershell -ExecutionPolicy Bypass -File scripts/quick-build-test.ps1

# Should now:
# 1. Configure with CMAKE_FIND_LIBRARY_SUFFIXES = .a .lib .dll
# 2. Find Qt6Core.a (static) instead of Qt6Core.lib (import)
# 3. Apply -static -static-libgcc -static-libstdc++ flags
# 4. Complete build successfully
# 5. Run diagnostic confirming static libraries used
# 6. Executable runs without QtCore.dll errors
```

## Backward Compatibility

All changes are Windows-specific (guarded with `if(WIN32 AND NOT MSVC)`), so:
- macOS/Linux builds unaffected
- MSVC builds unaffected (already rejected with clear error message)
- Non-Windows CMake branches unaffected

## Additional Notes

- The fix is defensive with multiple layers (suffix ordering, configuration variables, verification, linker flags)
- Multiple approaches ensure static linking even if one mechanism fails
- Clear error messages guide users to correct Qt kit with static libraries if needed
- Diagnostic scripts help troubleshoot if static linking still doesn't work
