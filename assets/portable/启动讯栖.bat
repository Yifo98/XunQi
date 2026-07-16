@echo off
chcp 65001 >nul
setlocal EnableExtensions

cd /d "%~dp0"
set "SOURCE=%~dp0source"

echo.
echo 讯栖 Windows 源码启动器
echo ----------------------------------------
echo 此包不包含预编译的 XunQi.exe、安装器或其他 Windows 运行文件。
echo 本 BAT 只负责检查开发环境，并从 source 目录本地编译后启动讯栖。
echo 当前预览版没有 Windows 代码签名；签名不是功能依赖，未被系统拦截时不影响使用。
echo 注意：BAT 不能绕过 Windows 智能应用控制；本地生成的程序仍可能被系统检查。
echo 如遇拦截，请先阅读同目录 README-Windows.txt，不要直接关闭安全防护。
echo.

if not exist "%SOURCE%\package.json" goto missing_source
if not exist "%SOURCE%\src-tauri\Cargo.toml" goto missing_source

where node >nul 2>nul || goto missing_node
where pnpm >nul 2>nul || goto missing_pnpm
where cargo >nul 2>nul || goto missing_rust

cd /d "%SOURCE%"

if not exist "node_modules" (
  echo [1/2] 首次运行，正在安装前端依赖……
  call pnpm install --frozen-lockfile
  if errorlevel 1 goto failed
) else (
  echo [1/2] 已找到前端依赖。
)

echo [2/2] 正在从源码编译并启动讯栖……
call pnpm tauri dev
if errorlevel 1 goto failed
goto done

:missing_source
echo 启动失败：没有找到完整 source 目录，请先完整解压 ZIP。
goto failed_pause

:missing_node
echo 启动失败：没有找到 Node.js 22 或更高版本。
echo 请先从 https://nodejs.org/ 安装 Node.js LTS。
goto failed_pause

:missing_pnpm
echo 启动失败：没有找到 pnpm 11。
echo 安装 Node.js 后，请在终端运行：corepack enable
goto failed_pause

:missing_rust
echo 启动失败：没有找到 Rust / Cargo。
echo 请先从 https://rustup.rs/ 安装 Rust stable 和 Windows MSVC 构建工具。
goto failed_pause

:failed
echo.
echo 编译或启动没有完成。请保留本窗口中的错误信息用于排查。

:failed_pause
echo.
pause
exit /b 1

:done
endlocal
