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
ICONSET_WORKDIR=$(mktemp -d)
trap 'rm -rf "$ICONSET_WORKDIR"' EXIT
ICONSET="${ICONSET_WORKDIR}/app.iconset"
mkdir -p "${ICONSET}"
for size in 16 32 64 128 256 512; do
    sips -z ${size} ${size} "${MASTER}" --out "${ICONSET}/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z ${double} ${double} "${MASTER}" --out "${ICONSET}/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "${ICONSET}" -o "${APP_DIR}/Contents/Resources/app.icns"

# Only release builds configured with a signing public key carry the updater.
# Cargo/developer bundles without that key keep their existing behavior.
if [ -n "${IMMERSION_UPDATE_PUBLIC_KEY:-}" ]; then
    SPARKLE_DIR="${IMMERSION_SPARKLE_DIR:-target/sparkle-2.10.0}"
    if [ ! -d "$SPARKLE_DIR/Sparkle.framework" ]; then
        scripts/fetch-sparkle.sh "$SPARKLE_DIR"
    fi
    mkdir -p "$APP_DIR/Contents/Frameworks"
    ditto "$SPARKLE_DIR/Sparkle.framework" "$APP_DIR/Contents/Frameworks/Sparkle.framework"
    cp "$SPARKLE_DIR/LICENSE" "$APP_DIR/Contents/Resources/Sparkle-LICENSE.txt"
    python3 - "$APP_DIR/Contents/Info.plist" <<'PY'
import os, plistlib, sys
path = sys.argv[1]
with open(path, "rb") as f:
    info = plistlib.load(f)
info.update({
    "SUFeedURL": "https://github.com/404oops/immersion/releases/latest/download/appcast-macos-arm64.xml",
    "SUPublicEDKey": os.environ["IMMERSION_UPDATE_PUBLIC_KEY"],
    "SUEnableAutomaticChecks": True,
    "SUAutomaticallyUpdate": True,
    "SUVerifyUpdateBeforeExtraction": True,
    "SURequireSignedFeed": True,
    "SUSignedFeedFailureExpirationInterval": 0,
    "SUEnableSystemProfiling": False,
})
with open(path, "wb") as f:
    plistlib.dump(info, f)
PY
fi

# Ad-hoc signature so the bundle launches on Apple Silicon. Set
# IMMERSION_SIGN_IDENTITY to use a real Developer ID certificate instead.
codesign --force --deep -s "${IMMERSION_SIGN_IDENTITY:--}" "${APP_DIR}"

echo "Built ${APP_DIR}"
