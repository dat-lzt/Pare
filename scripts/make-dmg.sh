#!/bin/bash
# 用 hdiutil 做磁盘映像。不调用访达，因此不需要「自动化」权限。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONF="$ROOT/src-tauri/tauri.conf.json"
APP_DIR="$ROOT/src-tauri/target/release/bundle/macos"
OUT_DIR="$ROOT/src-tauri/target/release/bundle/dmg"

NAME="$(python3 -c "import json; print(json.load(open('$CONF'))['productName'])")"
VERSION="$(python3 -c "import json; print(json.load(open('$CONF'))['version'])")"
case "$(uname -m)" in
  arm64) ARCH=aarch64 ;;
  *) ARCH="$(uname -m)" ;;
esac

APP="$APP_DIR/$NAME.app"
DMG="$OUT_DIR/${NAME}_${VERSION}_${ARCH}.dmg"

if [[ ! -d "$APP" ]]; then
  echo "找不到 $APP" >&2
  exit 1
fi

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/pare-dmg.XXXXXX")"
cleanup() { rm -rf "$STAGE"; }
trap cleanup EXIT

mkdir -p "$OUT_DIR" "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -volname "$NAME" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
echo "磁盘映像：$DMG"
