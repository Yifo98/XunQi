#!/usr/bin/env bash

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LAUNCHER="$PROJECT_ROOT/assets/portable/启动讯栖.command"
TEMP_ROOT="$(mktemp -d)"

cleanup() {
  rm -rf "$TEMP_ROOT"
}
trap cleanup EXIT

make_stub() {
  local target="$1"
  mkdir -p "$(dirname "$target")"
  printf '%s\n' \
    '#!/usr/bin/env bash' \
    'printf launched > "$XUNQI_LAUNCHER_TEST_MARKER"' >"$target"
  chmod u+x "$target"
}

wait_for_marker() {
  local marker="$1"
  for _ in {1..20}; do
    [[ -f "$marker" ]] && return 0
    sleep 0.05
  done
  echo "启动器没有运行预期的讯栖文件：$marker" >&2
  return 1
}

PACKAGE_ROOT="$TEMP_ROOT/package"
PACKAGE_MARKER="$TEMP_ROOT/package-launched"
mkdir -p "$PACKAGE_ROOT"
cp "$LAUNCHER" "$PACKAGE_ROOT/启动讯栖.command"
make_stub "$PACKAGE_ROOT/runtime/xunqi"
XUNQI_LAUNCHER_TEST_MARKER="$PACKAGE_MARKER" bash "$PACKAGE_ROOT/启动讯栖.command"
wait_for_marker "$PACKAGE_MARKER"

RELEASE_ROOT="$TEMP_ROOT/release-source"
RELEASE_MARKER="$TEMP_ROOT/release-source-launched"
mkdir -p "$RELEASE_ROOT"
cp "$LAUNCHER" "$RELEASE_ROOT/启动讯栖.command"
make_stub "$RELEASE_ROOT/讯栖项目文件/src-tauri/target/release/xunqi"
XUNQI_LAUNCHER_TEST_MARKER="$RELEASE_MARKER" bash "$RELEASE_ROOT/启动讯栖.command"
wait_for_marker "$RELEASE_MARKER"

DEV_ROOT="$TEMP_ROOT/development-source"
DEV_MARKER="$TEMP_ROOT/development-source-launched"
mkdir -p "$DEV_ROOT/讯栖项目文件/src-tauri" "$DEV_ROOT/bin"
cp "$LAUNCHER" "$DEV_ROOT/启动讯栖.command"
printf '{"name":"launcher-test"}\n' >"$DEV_ROOT/讯栖项目文件/package.json"
make_stub "$DEV_ROOT/bin/pnpm"
PATH="$DEV_ROOT/bin:$PATH" XUNQI_LAUNCHER_TEST_MARKER="$DEV_MARKER" bash "$DEV_ROOT/启动讯栖.command"
wait_for_marker "$DEV_MARKER"

echo "portable launcher smoke test passed"
