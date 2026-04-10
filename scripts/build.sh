#!/bin/bash
# Cross-platform build script - automatically detects OS and runs appropriate build

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🔨 Immersion Cross-Platform Build Script"
echo "========================================"
echo ""

# Detect OS
if [[ "$OSTYPE" == "darwin"* ]]; then
    OS="macOS"
    BUILD_SCRIPT="$SCRIPT_DIR/build-macos.sh"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    OS="Linux"
    BUILD_SCRIPT="$SCRIPT_DIR/build-linux.sh"
elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" ]]; then
    OS="Windows"
    BUILD_SCRIPT="$SCRIPT_DIR/build-windows.bat"
else
    echo "❌ Unsupported operating system: $OSTYPE"
    exit 1
fi

echo "Detected OS: $OS"
echo "Build script: $BUILD_SCRIPT"
echo ""

# Make build scripts executable on Unix-like systems
if [[ "$OS" != "Windows" ]]; then
    chmod +x "$SCRIPT_DIR/build-macos.sh"
    chmod +x "$SCRIPT_DIR/build-linux.sh"
fi

# Run appropriate build script
if [[ "$OS" == "Windows" ]]; then
    echo "Running: cmd /c \"$BUILD_SCRIPT\""
    cmd /c "$BUILD_SCRIPT"
else
    echo "Running: $BUILD_SCRIPT"
    "$BUILD_SCRIPT"
fi
