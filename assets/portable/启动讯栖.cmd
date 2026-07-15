@echo off
chcp 65001 >nul
setlocal
cd /d "%~dp0"
start "" "%~dp0runtime\XunQi.exe"
if errorlevel 1 (
  echo 讯栖启动失败，请确认 ZIP 已完整解压，且 runtime\XunQi.exe 存在。
  pause
)
endlocal
