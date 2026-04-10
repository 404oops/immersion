# Fixing "QtCore.dll Not Found" Error on Windows

## Problem
The executable `Immersion.exe` fails at startup with:
```
The code execution cannot proceed because QtCore.dll was not found.
Reinstalling the program may fix this problem.
```

This error occurs when the executable is **dynamically linked** to shared Qt libraries (`.dll` files) instead of having Qt **statically linked** (`.a` files compiled into the executable).

## Root Cause
The build system attempted to use static Qt libraries (`.a` files), but:
1. CMake found and used shared library files (`.dll`/`.lib`) instead of static files (`.a`)
2. The Qt installation kit has both static and shared libraries, and CMake chose the wrong ones
3. Or, your Qt kit doesn't actually have static libraries compiled

## Solution: Step-by-Step

### Step 1: Verify Your Qt Kit Has Static Libraries
Run the diagnostic script to check if static `.a` files exist:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/diagnose-qt-static.ps1
```

**Expected output:**
- ✓ libQt6Core.a
- ✓ libQt6Gui.a  
- ✓ libQt6Widgets.a
- etc.

**If you see ✗ (missing files):**
Your Qt kit doesn't have static libraries. **Solution:** Install a static Qt build:
- Download Qt from qt.io and select **Custom Installation**
- Choose a kit like `llvm-mingw_64` with "Static Release" option
- Install to `C:/Qt/6.x.x/llvm-mingw_64/` where x.x.x is your Qt version

### Step 2: Clean and Rebuild
After verifying static libraries exist:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/quick-build-test.ps1
```

The script now includes automatic diagnostics that will verify static linking was used.

### Step 3: Check Build Diagnostics
When the build completes, the script automatically runs `diagnose-qt-static.ps1` which will report:
- ✓ Static libraries are present
- Whether the build actually used static linking
- If both static and shared libraries were found (and which was used)

### Step 4: Verify the Executable Works
If diagnostics pass:
```powershell
./build/Immersion.exe
```

The executable should launch without any DLL errors.

## Understanding Static Linking

**Static Linking (what we want):**
- Qt libraries (.a files) are compiled directly into the executable
- Executable is fully self-contained
- No external Qt DLLs needed at runtime
- Larger executable file, but runs anywhere

**Dynamic Linking (the error you got):**
- Executable contains only references to Qt
- Requires Qt .DLL files in PATH or same directory at runtime
- Smaller executable, but needs dependencies deployed

## Common Issues & Fixes

### Issue 1: "libQt6Core.a not found"
Your Qt kit has shared libraries only.
- **Fix:** Download and install a Qt kit with static libraries (llvm-mingw_64 static)

### Issue 2: Both .a and .dll files found
Qt kit has both static and shared libraries.
- **Fix:** This is normal. The CMakeLists.txt now prioritizes `.a` (static) files.
- If this still caused DLL linking, you may need to:
  1. Find and move/delete the .dll files from the Qt bin/ directory
  2. Or specify the exact Qt cmake path containing only static libraries
  3. Or rebuild Qt as static-only

### Issue 3: Build succeeds but still says Qt6Core.dll missing
The executable was built with shared linking despite our changes.
- **Fix:** 
  1. Check the build directory: `build/CMakeFiles/Immersion.dir/link.txt`
  2. Look for whether it contains `.a` (static) or `.lib`/`.dll` (shared) files
  3. Run: `objdump -p build/Immersion.exe | grep "DLL Name"`
  4. If you see Qt6Core.dll in the output, it's dynamically linked
  5. Possible cause: CMAKE_FIND_LIBRARY_SUFFIXES not working as expected
  6. **Workaround:** Manually specify static libraries in CMakeLists.txt

## Advanced: Force Static Linking (if above doesn't work)

Edit `CMakeLists.txt` and replace the `target_link_libraries` section with explicit static library paths:

```cmake
# Find the actual .a file paths
find_library(Qt6Core_STATIC NAMES libQt6Core.a 
    PATHS "${Qt6_DIR}/../" NO_DEFAULT_PATH)
    
if(Qt6Core_STATIC)
    message(STATUS "Found Qt6Core static: ${Qt6Core_STATIC}")
    target_link_libraries(Immersion ${Qt6Core_STATIC})
else()
    message(FATAL_ERROR "Could not find Qt6Core static library")
endif()

# Repeat for each Qt component...
```

Then rebuild.

## Command Reference

```bash
# Clean build from scratch
rm -r build/
powershell -ExecutionPolicy Bypass -File scripts/quick-build-test.ps1

# Test diagnostic without rebuilding
powershell -ExecutionPolicy Bypass -File scripts/diagnose-qt-static.ps1

# Check what libraries the executable links to
objdump -p build/Immersion.exe | grep -E "DLL Name|Dynamic|IMPORT"

# Check for -static flag in build
type build\CMakeFiles\Immersion.dir\link.txt | findstr /I static
```

## Still Having Issues?

If none of the above work:
1. Check you're using the correct Qt kit path (should contain `llvm-mingw_64` or `mingw_64`)
2. Verify you're building with clang/clang++ (not MSVC): ` clang --version`
3. Run the diagnostic script and share its output
4. Check that CMakeLists.txt has the static linking configuration (lines with CMAKE_FIND_LIBRARY_SUFFIXES)
5. Verify you cleaned the build directory completely before rebuilding
