#!/usr/bin/env bash
# Builds Immersion.app from a cargo build. The .app bundle (with a real
# bundle identifier) is required for macOS user notifications and
# launch-at-login (SMAppService); the bare cargo binary skips both.
#
# Usage: scripts/bundle-macos.sh [--production]
#   --production  build with the `production` cargo profile (fat LTO,
#                 stripped) instead of `release`.
set -euo pipefail

cd "$(dirname "$0")/.."

PROFILE="release"
for arg in "$@"; do
    case "$arg" in
        --production) PROFILE="production" ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
APP_DIR="target/${PROFILE}/Immersion.app"
ICONS="crates/immersion/assets/icons"

echo "Building Immersion ${VERSION} (${PROFILE})..."
cargo build --profile "${PROFILE}" -p immersion

rm -rf "${APP_DIR}"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"

cp "target/${PROFILE}/immersion" "${APP_DIR}/Contents/MacOS/Immersion"
sed "s/__VERSION__/${VERSION}/g" crates/immersion/resources/Info.plist \
    > "${APP_DIR}/Contents/Info.plist"

# Menu bar template icon, looked up via NSBundle pathForResource.
cp "${ICONS}/menubar.png" "${APP_DIR}/Contents/Resources/menubar.png"
cp LICENSE "${APP_DIR}/Contents/Resources/LICENSE.txt"

# App icon: build an .icns from the 1024px master.
MASTER="${ICONS}/iconcomposer-macOS-Default-1024x1024@1x.png"
ICONSET=$(mktemp -d)/app.iconset
mkdir -p "${ICONSET}"
for size in 16 32 64 128 256 512; do
    sips -z ${size} ${size} "${MASTER}" --out "${ICONSET}/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z ${double} ${double} "${MASTER}" --out "${ICONSET}/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "${ICONSET}" -o "${APP_DIR}/Contents/Resources/app.icns"

# Ad-hoc signature so the bundle launches on Apple Silicon. Set
# IMMERSION_SIGN_IDENTITY to use a real Developer ID certificate instead.
codesign --force --deep -s "${IMMERSION_SIGN_IDENTITY:--}" "${APP_DIR}"

echo "Built ${APP_DIR}"
