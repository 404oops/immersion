#!/bin/bash
# Build script for Linux with CPack packaging

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_ROOT/build"

echo "🐧 Building musit for Linux..."
echo "Project root: $PROJECT_ROOT"
echo "Build directory: $BUILD_DIR"

# Create or clean build directory
if [ -d "$BUILD_DIR" ]; then
    echo "Cleaning existing build directory..."
    rm -rf "$BUILD_DIR"
fi
mkdir -p "$BUILD_DIR"

cd "$BUILD_DIR"

# Detect Qt6 installation
QT6_PATH=""
if command -v qmake6 &> /dev/null; then
    QT6_PATH="$(qmake6 -query QT_INSTALL_PREFIX)"
    echo "Found Qt6 at: $QT6_PATH"
elif command -v qmake &> /dev/null; then
    QT_VERSION=$(qmake -query QT_VERSION)
    if [[ "$QT_VERSION" == 6.* ]]; then
        QT6_PATH="$(qmake -query QT_INSTALL_PREFIX)"
        echo "Found Qt6 at: $QT6_PATH"
    fi
fi

# Configure with CMake
echo ""
echo "📋 Configuring CMake..."
if [ -n "$QT6_PATH" ]; then
    cmake -DCMAKE_BUILD_TYPE=Release \
           -DCMAKE_PREFIX_PATH="$QT6_PATH/lib/cmake" \
           -DQt6_DIR="$QT6_PATH/lib/cmake/Qt6" \
           "$PROJECT_ROOT"
else
    cmake -DCMAKE_BUILD_TYPE=Release \
           "$PROJECT_ROOT"
fi

# Build
echo ""
echo "🔨 Building musit..."
cmake --build . --config Release -j "$(nproc)"

# Package
echo ""
echo "📦 Creating Linux packages (TGZ, DEB, RPM)..."
cpack -C Release

echo ""
echo "✅ Build complete!"
echo ""
echo "Artifacts:"
ls -lah musit-*.{tar.gz,deb,rpm} 2>/dev/null || echo "  (Check build output above for available packages)"
echo ""
echo "Installation options:"
echo "  - TGZ: tar -xzf musit-*.tar.gz -C /opt/"
echo "  - DEB: sudo apt install ./musit-*.deb"
echo "  - RPM: sudo rpm -ivh musit-*.rpm"
