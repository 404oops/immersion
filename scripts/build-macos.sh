#!/bin/bash
# Build script for macOS with CPack packaging

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_ROOT/build"

echo "🍎 Building Immersion for macOS..."
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
echo "🔨 Building Immersion..."
cmake --build . --config MinSizeRel -j "$(sysctl -n hw.logicalcpu)"

# Deploy Qt frameworks and resources into the app bundle
APP_BUNDLE="$BUILD_DIR/Immersion.app"
if [ -d "$APP_BUNDLE" ]; then
    echo ""
    echo "🚚 Deploying Qt runtime into app bundle..."
    if command -v macdeployqt >/dev/null 2>&1; then
        macdeployqt "$APP_BUNDLE" -qmldir="$PROJECT_ROOT/src/qml"
    elif [ -x "$(brew --prefix qt6)/bin/macdeployqt" ]; then
        "$(brew --prefix qt6)/bin/macdeployqt" "$APP_BUNDLE" -qmldir="$PROJECT_ROOT/src/qml"
    else
        echo "❌ macdeployqt not found. Install Qt tools and ensure macdeployqt is in PATH."
        exit 1
    fi

    echo ""
    echo "📁 Bundle runtime locations:"
    echo "  Frameworks: $APP_BUNDLE/Contents/Frameworks"
    echo "  Resources:  $APP_BUNDLE/Contents/Resources"
else
    echo "❌ Expected app bundle not found: $APP_BUNDLE"
    exit 1
fi

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
