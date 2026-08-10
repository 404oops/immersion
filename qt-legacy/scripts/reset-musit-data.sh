#!/bin/bash
# Reset Immersion user data (config, saved folders, etc.)
# Does NOT delete project .musit folders (version history is preserved)
#
# Config location matches ProjectRegistry::appConfigDirectory(), which uses
# Qt's AppDataLocation (org/app name "musit" set in main.cpp):
#   macOS:   ~/Library/Application Support/musit/musit
#   Linux:   ~/.local/share/musit/musit
#   Windows: %APPDATA%/musit/musit

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "Immersion User Data Reset"
echo "========================="
echo ""
echo "This script removes Immersion configuration and app state."
echo "Project version history (.musit folders in projects) is NOT affected."
echo ""

# Detect platform and resolve app data directory (must match QStandardPaths::AppDataLocation)
if [[ "$OSTYPE" == "darwin"* ]]; then
    MUSIT_DATA="$HOME/Library/Application Support/musit/musit"
    MUSIT_CACHE="$HOME/Library/Caches/musit/musit"
    MUSIT_LEGACY="$HOME/Library/Application Support/musit"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    MUSIT_DATA="${XDG_DATA_HOME:-$HOME/.local/share}/musit/musit"
    MUSIT_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/musit/musit"
    MUSIT_LEGACY="$HOME/.config/musit"
else
    # Windows (Git Bash, MSYS, etc.)
    MUSIT_DATA="$APPDATA/musit/musit"
    MUSIT_CACHE="$LOCALAPPDATA/musit/musit"
    MUSIT_LEGACY="$APPDATA/musit"
fi

echo "Target directories:"
echo "  Data:  $MUSIT_DATA"
echo "  Cache: $MUSIT_CACHE"
if [ -d "$MUSIT_LEGACY" ] && [ "$MUSIT_LEGACY" != "$MUSIT_DATA" ]; then
    echo "  Legacy: $MUSIT_LEGACY (pre-unification config, also removed)"
fi
echo ""

read -p "Are you sure? (Type 'yes' to confirm): " -r
echo ""

if [[ $REPLY != "yes" ]]; then
    echo "Cancelled."
    exit 0
fi

for dir in "$MUSIT_DATA" "$MUSIT_CACHE" "$MUSIT_LEGACY"; do
    if [ -n "$dir" ] && [ -d "$dir" ]; then
        echo "Removing $dir..."
        rm -rf "$dir"
    fi
done

# Also remove the buggy nested path some Windows builds created.
if [[ "$OSTYPE" == "msys"* || "$OSTYPE" == "win32" || "$OSTYPE" == "cygwin" ]]; then
    BUGGY="$APPDATA/musit/musit/musit"
    if [ -d "$BUGGY" ]; then
        echo "Removing $BUGGY..."
        rm -rf "$BUGGY"
    fi
fi

echo ""
echo "Reset complete. Immersion will show onboarding on next launch."
