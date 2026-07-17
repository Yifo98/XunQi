#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_PATH="$SCRIPT_DIR/XunQi-launch.log"
APP_BINARY=""
SOURCE_DIR=""

for candidate in \
  "$SCRIPT_DIR/runtime/xunqi" \
  "$SCRIPT_DIR/讯栖项目文件/src-tauri/target/release/xunqi" \
  "$SCRIPT_DIR/src-tauri/target/release/xunqi" \
  "$SCRIPT_DIR/../../src-tauri/target/release/xunqi"
do
  if [[ -f "$candidate" ]]; then
    APP_BINARY="$candidate"
    break
  fi
done

if [[ -n "$APP_BINARY" ]]; then
  RUNTIME_DIR="$(dirname "$APP_BINARY")"
  for executable in "$APP_BINARY" "$RUNTIME_DIR/xunqi-pdf-renderer" "$RUNTIME_DIR/xunqi-authorized-sniffer"
  do
    if [[ -f "$executable" ]]; then
      chmod u+x "$executable"
    fi
  done
  nohup "$APP_BINARY" >>"$LOG_PATH" 2>&1 &
  exit 0
fi

for candidate in \
  "$SCRIPT_DIR/讯栖项目文件" \
  "$SCRIPT_DIR" \
  "$SCRIPT_DIR/../.."
do
  if [[ -f "$candidate/package.json" && -d "$candidate/src-tauri" ]]; then
    SOURCE_DIR="$(cd "$candidate" && pwd)"
    break
  fi
done

if [[ -z "$SOURCE_DIR" ]]; then
  /usr/bin/osascript -e 'display alert "没有找到讯栖运行文件" message "请确认压缩包已完整解压；如果这是源码目录，请保留完整的项目文件夹。" as critical'
  exit 1
fi

PNPM_BIN="$(command -v pnpm || true)"
if [[ -z "$PNPM_BIN" ]]; then
  PNPM_BIN="$(/bin/zsh -lic 'command -v pnpm' 2>/dev/null || true)"
fi
if [[ -z "$PNPM_BIN" ]]; then
  /usr/bin/osascript -e 'display alert "缺少本地开发环境" message "源码启动需要 Node.js 与 pnpm。GitHub 下载的 macOS 压缩包无需安装这些工具，请改用压缩包内的启动器。" as critical'
  exit 1
fi

cd "$SOURCE_DIR"
exec /usr/bin/env PATH="$HOME/.cargo/bin:$PATH" "$PNPM_BIN" tauri dev
