#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARCH="$(uname -m)"
VERSION="v260706"
ZIP_NAME="wx_video_download_v260706_darwin_arm64.zip"
DOWNLOAD_URL="https://github.com/ltaoo/wx_channels_download/releases/download/v260706/$ZIP_NAME"
ZIP_SHA256="c912c73f802366ade747e83da63a6a3b03a6e180508d05d90f25563c99604b3c"
BINARY_SHA256="ee777e9f07a4784163d16c6dc0288bb9d8c1ea9db3ae4a7275f098d1a1a56aca"
TARGET="$ROOT_DIR/src-tauri/bin/xunqi-authorized-sniffer-aarch64-apple-darwin"
LICENSE_TARGET="$ROOT_DIR/assets/portable/第三方许可-wx_channels_download.txt"

if [[ "$ARCH" != "arm64" ]]; then
  echo "授权嗅探助手首版仅提供 macOS arm64 组件，当前架构：$ARCH" >&2
  exit 1
fi

if [[ -f "$TARGET" ]] && [[ "$(shasum -a 256 "$TARGET" | awk '{print $1}')" == "$BINARY_SHA256" ]] && [[ -f "$LICENSE_TARGET" ]]; then
  exit 0
fi

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/xunqi-sniffer-helper.XXXXXX")"
cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

curl --fail --location --silent --show-error "$DOWNLOAD_URL" --output "$WORK_DIR/$ZIP_NAME"
ACTUAL_ZIP_SHA256="$(shasum -a 256 "$WORK_DIR/$ZIP_NAME" | awk '{print $1}')"
if [[ "$ACTUAL_ZIP_SHA256" != "$ZIP_SHA256" ]]; then
  echo "授权嗅探助手压缩包校验失败：$ACTUAL_ZIP_SHA256" >&2
  exit 1
fi

ditto -x -k "$WORK_DIR/$ZIP_NAME" "$WORK_DIR/unpacked"
SOURCE_BINARY="$WORK_DIR/unpacked/wx_video_download"
SOURCE_LICENSE="$WORK_DIR/unpacked/LICENSE"
ACTUAL_BINARY_SHA256="$(shasum -a 256 "$SOURCE_BINARY" | awk '{print $1}')"
if [[ "$ACTUAL_BINARY_SHA256" != "$BINARY_SHA256" ]]; then
  echo "授权嗅探助手程序校验失败：$ACTUAL_BINARY_SHA256" >&2
  exit 1
fi

mkdir -p "$(dirname "$TARGET")" "$(dirname "$LICENSE_TARGET")"
install -m 755 "$SOURCE_BINARY" "$TARGET"
{
  printf '讯栖授权嗅探助手第三方许可\n'
  printf '来源：https://github.com/ltaoo/wx_channels_download\n'
  printf '固定版本：%s\n' "$VERSION"
  printf '固定提交：5d4b36740c9f035747ba7b1693202c7feb473eb3\n'
  printf '发布包 SHA-256：%s\n' "$ZIP_SHA256"
  printf '程序 SHA-256：%s\n\n' "$BINARY_SHA256"
  cat "$SOURCE_LICENSE"
} > "$LICENSE_TARGET"
