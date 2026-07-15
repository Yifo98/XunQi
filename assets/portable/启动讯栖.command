#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME_DIR="$SCRIPT_DIR/runtime"
APP_BINARY="$RUNTIME_DIR/xunqi"
LOG_PATH="$SCRIPT_DIR/XunQi-launch.log"

if [[ ! -f "$APP_BINARY" ]]; then
  osascript -e 'display alert "没有找到讯栖运行文件" message "请确认压缩包已经完整解压，且 runtime 文件夹与启动器位于同一目录。" as critical'
  exit 1
fi

chmod u+x "$APP_BINARY" "$RUNTIME_DIR/xunqi-pdf-renderer" "$RUNTIME_DIR/xunqi-authorized-sniffer"
nohup "$APP_BINARY" >>"$LOG_PATH" 2>&1 &
exit 0
