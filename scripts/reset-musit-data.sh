#!/bin/bash
# Reset musit user data (config, saved folders, etc.)
# Does NOT delete project .musit folders (version history is preserved)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "musit User Data Reset"
echo "====================="
echo ""
echo "This script removes musit configuration and app state."
echo "Project version history (.musit folders in projects) is NOT affected."
echo ""

# Detect platform and resolve app data directory
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS: ~/Library/Application Support/musit
    MUSIT_DATA="$HOME/Library/Application Support/musit"
    MUSIT_CACHE="$HOME/Library/Caches/musit"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Linux: ~/.local/share/musit
    MUSIT_DATA="$HOME/.local/share/musit"
    MUSIT_CACHE="$HOME/.cache/musit"
else
    # Windows (Git Bash, MSYS, etc.): %APPDATA%\musit
    MUSIT_DATA="$APPDATA/musit"
    MUSIT_CACHE="$LOCALAPPDATA/musit/cache"
fi

echo "Target directories:"
echo "  Data: $MUSIT_DATA"
echo "  Cache: $MUSIT_CACHE"
echo ""

read -p "Are you sure? (Type 'yes' to confirm): " -r
echo ""

if [[ $REPLY != "yes" ]]; then
    echo "Cancelled."
    exit 0
fi

# Delete user data directories
if [ -d "$MUSIT_DATA" ]; then
    echo "Removing $MUSIT_DATA..."
    rm -rf "$MUSIT_DATA"
fi

if [ -d "$MUSIT_CACHE" ]; then
    echo "Removing $MUSIT_CACHE..."
    rm -rf "$MUSIT_CACHE"
fi

echo ""
echo "✓ Reset complete. musit will show onboarding on next launch."
