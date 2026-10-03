#!/usr/bin/env bash
# Pinned upstream binary, with checksum verification before extraction.
set -euo pipefail
DEST="${1:?usage: fetch-sparkle.sh destination}"
mkdir -p "$DEST"
curl -fLsS --retry 3 https://github.com/sparkle-project/Sparkle/releases/download/2.10.0/Sparkle-2.10.0.tar.xz -o "$DEST/archive.tar.xz"
echo "c2bf58aa8387266ac179357b1415d6f2635f044da8be41042af32425dae6da0c  $DEST/archive.tar.xz" | shasum -a 256 -c -
tar -xf "$DEST/archive.tar.xz" -C "$DEST"
