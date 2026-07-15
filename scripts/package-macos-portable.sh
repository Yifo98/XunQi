#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(node -e 'const fs=require("fs"); const config=JSON.parse(fs.readFileSync(process.argv[1], "utf8")); process.stdout.write(config.version)' "$ROOT_DIR/src-tauri/tauri.conf.json")"
PACKAGE_VERSION="$(node -e 'const fs=require("fs"); const config=JSON.parse(fs.readFileSync(process.argv[1], "utf8")); process.stdout.write(config.version)' "$ROOT_DIR/package.json")"
CARGO_VERSION="$(awk -F ' *= *' '/^version = / {gsub(/"/, "", $2); print $2; exit}' "$ROOT_DIR/src-tauri/Cargo.toml")"
MACHINE_ARCH="$(uname -m)"

if [[ "$VERSION" != "$PACKAGE_VERSION" || "$VERSION" != "$CARGO_VERSION" ]]; then
  echo "版本号不一致：tauri=$VERSION package=$PACKAGE_VERSION cargo=$CARGO_VERSION" >&2
  exit 1
fi

if [[ "$VERSION" != *-* ]]; then
  echo "公开包只允许预发布版本，当前版本：$VERSION" >&2
  exit 1
fi

if ! git -C "$ROOT_DIR" diff --quiet || ! git -C "$ROOT_DIR" diff --cached --quiet; then
  echo "请先提交当前修改；源码包必须与 Git 提交完全一致。" >&2
  exit 1
fi

case "$MACHINE_ARCH" in
  arm64 | aarch64)
    PACKAGE_ARCH="arm64"
    TARGET_TRIPLE="aarch64-apple-darwin"
    ;;
  *)
    echo "当前预发布包只支持 macOS Apple Silicon，当前架构：$MACHINE_ARCH" >&2
    exit 1
    ;;
esac

export PATH="$HOME/.cargo/bin:$PATH"

cd "$ROOT_DIR"
# tauri.conf.json 的 beforeBuildCommand 会先运行 build:native，因此干净
# checkout 也会在构建主程序前生成两个未纳入 Git 的本地 helper。
pnpm tauri build --no-bundle

MAIN_BINARY="$ROOT_DIR/src-tauri/target/release/xunqi"
PDF_HELPER="$ROOT_DIR/src-tauri/bin/xunqi-pdf-renderer-$TARGET_TRIPLE"
SNIFFER_HELPER="$ROOT_DIR/src-tauri/bin/xunqi-authorized-sniffer-$TARGET_TRIPLE"
OUTPUT_DIR="$ROOT_DIR/release"
PACKAGE_NAME="XunQi-$VERSION-macOS-$PACKAGE_ARCH-source-preview"
ZIP_PATH="$OUTPUT_DIR/$PACKAGE_NAME.zip"
CHECKSUM_PATH="$ZIP_PATH.sha256"
STAGING_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/xunqi-source-preview.XXXXXX")"

cleanup() {
  rm -rf "$STAGING_ROOT"
}
trap cleanup EXIT

PACKAGE_DIR="$STAGING_ROOT/$PACKAGE_NAME"
RUNTIME_DIR="$PACKAGE_DIR/runtime"
SOURCE_DIR="$PACKAGE_DIR/source"
mkdir -p "$RUNTIME_DIR" "$SOURCE_DIR" "$OUTPUT_DIR"
rm -f "$ZIP_PATH" "$CHECKSUM_PATH"

EXPECTED_SNIFFER_SHA256="ee777e9f07a4784163d16c6dc0288bb9d8c1ea9db3ae4a7275f098d1a1a56aca"
ACTUAL_SNIFFER_SHA256="$(shasum -a 256 "$SNIFFER_HELPER" | awk '{print $1}')"
if [[ "$ACTUAL_SNIFFER_SHA256" != "$EXPECTED_SNIFFER_SHA256" ]]; then
  echo "授权嗅探组件校验失败：$ACTUAL_SNIFFER_SHA256" >&2
  exit 1
fi

install -m 755 "$MAIN_BINARY" "$RUNTIME_DIR/xunqi"
install -m 755 "$PDF_HELPER" "$RUNTIME_DIR/xunqi-pdf-renderer"
install -m 755 "$SNIFFER_HELPER" "$RUNTIME_DIR/xunqi-authorized-sniffer"
install -m 755 "$ROOT_DIR/assets/portable/启动讯栖.command" "$PACKAGE_DIR/Launch-XunQi.command"
install -m 644 "$ROOT_DIR/assets/portable/使用说明.txt" "$PACKAGE_DIR/README-macOS.txt"
install -m 644 "$ROOT_DIR/assets/portable/第三方许可-wx_channels_download.txt" "$PACKAGE_DIR/THIRD-PARTY-wx_channels_download.txt"

git -C "$ROOT_DIR" archive --format=tar HEAD | tar -xf - -C "$SOURCE_DIR"

for binary in "$RUNTIME_DIR/xunqi" "$RUNTIME_DIR/xunqi-pdf-renderer" "$RUNTIME_DIR/xunqi-authorized-sniffer"; do
  codesign --force --sign - "$binary"
  codesign --verify --strict "$binary"
done

EXPECTED_MACH_ARCH="arm64"
verify_macos_binary() {
  local binary="$1"
  local maximum_minos="$2"
  local architectures minos
  architectures="$(/usr/bin/lipo -archs "$binary")"
  if [[ " $architectures " != *" $EXPECTED_MACH_ARCH "* ]]; then
    echo "程序架构错误：$binary ($architectures)" >&2
    exit 1
  fi
  minos="$(/usr/bin/vtool -show-build "$binary" | awk '/^[[:space:]]+minos / {print $2; exit}')"
  if [[ -z "$minos" ]]; then
    echo "无法读取最低系统版本：$binary" >&2
    exit 1
  fi
  awk -v actual="$minos" -v maximum="$maximum_minos" '
    BEGIN {
      split(actual, a, "."); split(maximum, m, ".");
      if ((a[1] + 0) > (m[1] + 0) || ((a[1] + 0) == (m[1] + 0) && (a[2] + 0) > (m[2] + 0))) exit 1;
    }
  ' || {
    echo "最低系统版本过高：$binary 需要 macOS $minos，门禁上限为 $maximum_minos" >&2
    exit 1
  }
}

for binary in "$RUNTIME_DIR/xunqi" "$RUNTIME_DIR/xunqi-pdf-renderer" "$RUNTIME_DIR/xunqi-authorized-sniffer"; do
  verify_macos_binary "$binary" "11.0"
done

(
  cd "$STAGING_ROOT"
  COPYFILE_DISABLE=true zip -qryX "$ZIP_PATH" "$PACKAGE_NAME"
)

CONTENTS="$(bsdtar -tf "$ZIP_PATH")"
if printf '%s\n' "$CONTENTS" | grep -Eq '(^|/)(__MACOSX|\.DS_Store|\._|[^/]+\.app/)'; then
  echo "压缩包含有禁止的 macOS App 或垃圾文件" >&2
  exit 1
fi

for required_path in \
  "$PACKAGE_NAME/Launch-XunQi.command" \
  "$PACKAGE_NAME/README-macOS.txt" \
  "$PACKAGE_NAME/runtime/xunqi" \
  "$PACKAGE_NAME/runtime/xunqi-pdf-renderer" \
  "$PACKAGE_NAME/runtime/xunqi-authorized-sniffer" \
  "$PACKAGE_NAME/source/package.json" \
  "$PACKAGE_NAME/source/src/App.tsx" \
  "$PACKAGE_NAME/source/src-tauri/src/main.rs"
do
  if ! printf '%s\n' "$CONTENTS" | grep -Fxq "$required_path"; then
    echo "压缩包缺少：$required_path" >&2
    exit 1
  fi
done

CHECKSUM="$(shasum -a 256 "$ZIP_PATH" | awk '{print $1}')"
printf '%s  %s\n' "$CHECKSUM" "$(basename "$ZIP_PATH")" > "$CHECKSUM_PATH"

echo "macOS 源码预发布包：$ZIP_PATH"
echo "SHA-256：$CHECKSUM"
