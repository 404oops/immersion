#!/bin/bash
# Build script for macOS with CPack packaging

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_ROOT/build"

echo "🍎 Building musit for macOS..."
echo "Project root: $PROJECT_ROOT"
echo "Build directory: $BUILD_DIR"

# Create or clean build directory
if [ -d "$BUILD_DIR" ]; then
    echo "Cleaning existing build directory..."
    rm -rf "$BUILD_DIR"
fi
mkdir -p "$BUILD_DIR"

cd "$BUILD_DIR"

# Configure with CMake
echo ""
echo "📋 Configuring CMake..."
cmake -DCMAKE_BUILD_TYPE=MinSizeRel \
       -DCMAKE_PREFIX_PATH="$(brew --prefix qt6)" \
       -DQt6_DIR="$(brew --prefix qt6)/lib/cmake/Qt6" \
       "$PROJECT_ROOT"

# Build
echo ""
echo "🔨 Building musit..."
cmake --build . --config MinSizeRel -j "$(sysctl -n hw.logicalcpu)"

# Package
echo ""
echo "📦 Creating macOS DMG package..."
cpack -C MinSizeRel

echo ""
echo "✅ Build complete!"
echo ""
echo "Artifacts:"
ls -lah musit-*.dmg 2>/dev/null || echo "  (No DMG found - check build output above)"
echo ""
echo "To install: open musit-*.dmg and drag musit.app to Applications"
