@echo off
setlocal EnableExtensions DisableDelayedExpansion

set "LOG_ROOT=%LOCALAPPDATA%\XunQi\logs"
if not defined LOCALAPPDATA set "LOG_ROOT=%TEMP%\XunQi\logs"
if not exist "%LOG_ROOT%" mkdir "%LOG_ROOT%" >nul 2>nul
start "" explorer.exe "%LOG_ROOT%"
exit /b 0
