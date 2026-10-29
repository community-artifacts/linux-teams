#!/usr/bin/env bash
# Builds the tray with Tauri, then injects firefoxpwa + a pre-patched Firefox runtime
# into the resulting AppImage and repacks it.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
SRC_RUNTIME=${SRC_RUNTIME:-$HOME/.local/share/firefoxpwa/runtime}
APPIMAGETOOL=${APPIMAGETOOL:-${XDG_CACHE_HOME:-$HOME/.cache}/teams-for-linux/appimagetool}
if [ ! -x "$APPIMAGETOOL" ]; then
  mkdir -p "$(dirname "$APPIMAGETOOL")"
  curl -fsSL -o "$APPIMAGETOOL" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$APPIMAGETOOL"
fi
OUT=$ROOT/dist
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

(cd src-tauri && NO_STRIP=true cargo tauri build --bundles appimage)
BASE=$(ls src-tauri/target/release/bundle/appimage/*.AppImage | head -1)

cp "$BASE" "$WORK/base.AppImage" && chmod +x "$WORK/base.AppImage"
(cd "$WORK" && ./base.AppImage --appimage-extract >/dev/null)
APPDIR=$WORK/squashfs-root
LIB=$APPDIR/usr/lib/teams-for-linux

mkdir -p "$LIB/sysdata"
cp /usr/bin/firefoxpwa /usr/libexec/firefoxpwa-connector "$LIB/"
cp -r /usr/share/firefoxpwa/userchrome "$LIB/sysdata/"
cp -a "$SRC_RUNTIME" "$LIB/runtime"
curl -fsSL -o "$LIB/teams.png" https://teams.public.onecdn.static.microsoft/evergreen-assets/icons/windows/teams-icon-pwa-v2025-256.png

mkdir -p "$OUT"
ARCH=x86_64 "$APPIMAGETOOL" "$APPDIR" "$OUT/Teams_for_Linux-x86_64.AppImage"
echo "built: $OUT/Teams_for_Linux-x86_64.AppImage"
