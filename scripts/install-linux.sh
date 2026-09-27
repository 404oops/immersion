#!/usr/bin/env bash
# Build and install the native Linux app under ~/.local, or a supplied prefix.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
prefix="${1:-$HOME/.local}"
if [[ "$prefix" != /* ]]; then
    prefix="$PWD/$prefix"
fi

cd "$repo_root"
cargo build --release --locked -p immersion

app_id=io.github._404oops.immersion
install -Dm755 target/release/immersion "$prefix/bin/immersion"
install -Dm644 "packaging/linux/$app_id.desktop" "$prefix/share/applications/$app_id.desktop"
install -Dm644 "packaging/linux/$app_id.metainfo.xml" "$prefix/share/metainfo/$app_id.metainfo.xml"
for size in 256 512; do
    install -Dm644 "packaging/linux/icon-$size.png" \
        "$prefix/share/icons/hicolor/${size}x${size}/apps/$app_id.png"
done
install -Dm644 LICENSE "$prefix/share/licenses/$app_id/LICENSE"

echo "Installed Immersion in $prefix"
