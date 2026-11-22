#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

ARCH="${ARCH:-x86_64}"
TOOL="appimagetool-$ARCH.AppImage"

cargo build --release

if [ ! -f "$TOOL" ]; then
  curl -fsSL -o "$TOOL" "https://github.com/AppImage/appimagetool/releases/download/continuous/$TOOL"
  chmod +x "$TOOL"
fi

rm -rf AppDir
mkdir -p AppDir/usr/bin
cp target/release/zimnlauncher AppDir/usr/bin/
cp packaging/zimnlauncher.desktop AppDir/
cp packaging/zimnlauncher.png AppDir/
ln -s usr/bin/zimnlauncher AppDir/AppRun

export APPIMAGE_EXTRACT_AND_RUN=1
ARCH="$ARCH" "./$TOOL" AppDir "zimnlauncher-$ARCH.AppImage"
echo "Built zimnlauncher-$ARCH.AppImage"
