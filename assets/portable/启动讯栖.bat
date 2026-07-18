@echo off
setlocal EnableExtensions DisableDelayedExpansion

cd /d "%~dp0"
set "SOURCE=%~dp0source"
set "PNPM_RUN="
set "COREPACK_ENABLE_DOWNLOAD_PROMPT=0"

echo.
echo XunQi Windows source launcher
echo ----------------------------------------
echo This package contains source code and does not include a prebuilt XunQi.exe.
echo The launcher uses pnpm, Corepack, or npx to install dependencies and start XunQi.
echo If Windows blocks the launcher, read README-Windows.txt before changing security settings.
echo.

if /I "%XUNQI_LAUNCHER_SELF_TEST%"=="1" goto self_test

if not exist "%SOURCE%\package.json" goto missing_source
if not exist "%SOURCE%\src-tauri\Cargo.toml" goto missing_source

where node >nul 2>nul || goto missing_node
where cargo >nul 2>nul || goto missing_rust

call :select_pnpm
if errorlevel 1 goto missing_pnpm

cd /d "%SOURCE%"

if exist "node_modules" goto dependencies_ready
echo [1/2] Installing frontend dependencies for the first run...
call %PNPM_RUN% install --frozen-lockfile
if errorlevel 1 goto failed
goto start_app

:dependencies_ready
echo [1/2] Frontend dependencies are already available.

:start_app
echo [2/2] Building and starting XunQi from source...
call %PNPM_RUN% tauri dev
if errorlevel 1 goto failed
goto done

:select_pnpm
where corepack >nul 2>nul
if errorlevel 1 goto try_global_pnpm
echo Preparing pnpm through Corepack...
call corepack pnpm --version >nul 2>nul
if errorlevel 1 goto try_global_pnpm
set "PNPM_RUN=corepack pnpm"
exit /b 0

:try_global_pnpm
where pnpm >nul 2>nul
if errorlevel 1 goto try_npx_pnpm
call pnpm --version >nul 2>nul
if errorlevel 1 goto try_npx_pnpm
set "PNPM_RUN=pnpm"
exit /b 0

:try_npx_pnpm
where npx >nul 2>nul
if errorlevel 1 exit /b 1
echo Corepack or global pnpm is unavailable. Using npx with pnpm 11.7.0...
call npx --yes pnpm@11.7.0 --version >nul 2>nul
if errorlevel 1 exit /b 1
set "PNPM_RUN=npx --yes pnpm@11.7.0"
exit /b 0

:missing_source
echo Launch failed: the complete source directory was not found.
echo Fully extract the ZIP before running Launch-XunQi.bat.
goto failed_pause

:missing_node
echo Launch failed: Node.js 22 or newer was not found.
echo Install Node.js LTS from https://nodejs.org/
goto failed_pause

:missing_pnpm
echo Launch failed: pnpm could not be started through Corepack, pnpm, or npx.
echo Check the network connection, then run one of these commands in Command Prompt:
echo   corepack pnpm --version
echo   npx --yes pnpm@11.7.0 --version
goto failed_pause

:missing_rust
echo Launch failed: Rust and Cargo were not found.
echo Install Rust stable from https://rustup.rs/ and install the MSVC build tools.
goto failed_pause

:failed
echo.
echo The build or launch did not finish. Keep this window open and save the error text.

:failed_pause
echo.
pause
exit /b 1

:self_test
echo XUNQI_LAUNCHER_SELF_TEST_OK
exit /b 0

:done
endlocal
