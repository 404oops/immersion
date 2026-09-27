#!/usr/bin/env bash
# Run on x86_64 Linux after cargo build --release --locked -p immersion.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

[[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]] || {
    echo "Linux x86_64 is required" >&2
    exit 1
}
[[ -x target/release/immersion ]] || {
    echo "Build target/release/immersion first" >&2
    exit 1
}

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
export IMMERSION_VERSION="$version"
app_id=io.github._404oops.immersion
mkdir -p dist

if command -v nfpm >/dev/null 2>&1; then
    for format in deb rpm archlinux; do
        case "$format" in
            deb) suffix=deb ;;
            rpm) suffix=rpm ;;
            archlinux) suffix=pkg.tar.zst ;;
        esac
        nfpm pkg --config packaging/linux/nfpm.yml --packager "$format" \
            --target "dist/Immersion-${version}-linux-x86_64.${suffix}"
    done
else
    echo "nfpm is required for deb, rpm, and Arch packages" >&2
    exit 1
fi

bundle="dist/Immersion-${version}-linux-x86_64"
mkdir -p "$bundle/bin" "$bundle/share/applications" "$bundle/share/metainfo"
mkdir -p "$bundle/share/icons/hicolor/256x256/apps" "$bundle/share/icons/hicolor/512x512/apps"
install -m755 target/release/immersion "$bundle/bin/immersion"
install -m644 "packaging/linux/$app_id.desktop" "$bundle/share/applications/$app_id.desktop"
install -m644 "packaging/linux/$app_id.metainfo.xml" "$bundle/share/metainfo/$app_id.metainfo.xml"
for size in 256 512; do
    install -m644 "packaging/linux/icon-$size.png" \
        "$bundle/share/icons/hicolor/${size}x${size}/apps/$app_id.png"
done
install -m644 LICENSE "$bundle/LICENSE"
tar -C dist -czf "${bundle}.tar.gz" "$(basename "$bundle")"

if command -v linuxdeploy >/dev/null 2>&1; then
    appdir="dist/Immersion.AppDir"
    rm -rf "$appdir"
    install -m644 packaging/linux/icon-512.png "dist/$app_id.png"
    touch dist/appimage-start
    ARCH=x86_64 linuxdeploy --appdir "$appdir" \
        --executable target/release/immersion \
        --desktop-file "packaging/linux/$app_id.desktop" \
        --icon-file "dist/$app_id.png" \
        --output appimage
    appimage="$(find . -maxdepth 1 -name '*.AppImage' -newer dist/appimage-start -print -quit)"
    [[ -n "$appimage" ]] || { echo "linuxdeploy produced no AppImage" >&2; exit 1; }
    mv "$appimage" "dist/Immersion-${version}-linux-x86_64.AppImage"
    rm "dist/$app_id.png" dist/appimage-start
else
    echo "linuxdeploy is required for the AppImage" >&2
    exit 1
fi
