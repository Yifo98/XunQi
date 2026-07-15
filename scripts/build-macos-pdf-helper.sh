#!/bin/bash

set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_TRIPLE="${XUNQI_TARGET_TRIPLE:-$(rustc -vV | awk '/^host:/ {print $2}')}"
OUTPUT_DIR="$ROOT_DIR/src-tauri/bin"
OUTPUT_PATH="$OUTPUT_DIR/xunqi-pdf-renderer-$TARGET_TRIPLE"

if [[ "$TARGET_TRIPLE" != *-apple-darwin ]]; then
  echo "PDF helper only supports macOS targets: $TARGET_TRIPLE" >&2
  exit 1
fi

case "$TARGET_TRIPLE" in
  aarch64-apple-darwin) SWIFT_TARGET="arm64-apple-macos11.0" ;;
  x86_64-apple-darwin) SWIFT_TARGET="x86_64-apple-macos11.0" ;;
esac

mkdir -p "$OUTPUT_DIR"
xcrun swiftc -swift-version 5 -O -warnings-as-errors \
  -target "$SWIFT_TARGET" \
  -framework AppKit \
  -framework WebKit \
  "$ROOT_DIR/src-tauri/helpers/xunqi-pdf-renderer.swift" \
  -o "$OUTPUT_PATH"
chmod 755 "$OUTPUT_PATH"
echo "$OUTPUT_PATH"
