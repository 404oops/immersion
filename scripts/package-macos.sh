#!/usr/bin/env bash
# Post-build macOS packaging: builds Immersion.app (via bundle-macos.sh) and
# wraps it in a drag-to-Applications DMG under dist/.
#
# Usage: scripts/package-macos.sh [--production]
#   --production  package the `production` cargo profile build (fat LTO,
#                 stripped) — use this for distribution.
set -euo pipefail

cd "$(dirname "$0")/.."

PROFILE="release"
BUNDLE_ARGS=()
for arg in "$@"; do
    case "$arg" in
        --production) PROFILE="production"; BUNDLE_ARGS+=("--production") ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

scripts/bundle-macos.sh ${BUNDLE_ARGS[@]+"${BUNDLE_ARGS[@]}"}

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
APP_DIR="target/${PROFILE}/Immersion.app"
DMG="dist/Immersion-${VERSION}.dmg"

mkdir -p dist
STAGING=$(mktemp -d)/Immersion
mkdir -p "${STAGING}"
cp -R "${APP_DIR}" "${STAGING}/"
cp LICENSE "${STAGING}/LICENSE.txt"
ln -s /Applications "${STAGING}/Applications"

rm -f "${DMG}"
hdiutil create -volname "Immersion ${VERSION}" -srcfolder "${STAGING}" \
    -ov -format UDZO "${DMG}" >/dev/null
python3 scripts/add-dmg-license.py "${DMG}"

echo "Packaged ${DMG}"
