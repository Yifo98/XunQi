@echo off
setlocal EnableExtensions DisableDelayedExpansion

cd /d "%~dp0"
set "APP=%~dp0runtime\xunqi.exe"
set "LOG_ROOT=%LOCALAPPDATA%\XunQi\logs"
if not defined LOCALAPPDATA set "LOG_ROOT=%TEMP%\XunQi\logs"
if not exist "%LOG_ROOT%" mkdir "%LOG_ROOT%" >nul 2>nul
set "LOG_FILE=%LOG_ROOT%\launcher.log"

call :log launcher_start

if /I "%XUNQI_RUNTIME_LAUNCHER_SELF_TEST%"=="1" goto self_test
if not exist "%APP%" goto missing_runtime

echo Starting XunQi...
echo Startup log: %LOG_FILE%
start "" /wait "%APP%"
set "APP_EXIT=%ERRORLEVEL%"
if not "%APP_EXIT%"=="0" goto failed
call :log launcher_exit_ok
exit /b 0

:missing_runtime
call :log error_runtime_missing
echo.
echo Launch failed: runtime\xunqi.exe was not found.
echo Fully extract the ZIP before running Launch-XunQi.bat.
goto failed_pause

:failed
call :log error_runtime_exit
echo.
echo XunQi stopped with exit code %APP_EXIT%.
echo Open the log folder with Open-XunQi-Logs.bat and send the log for diagnosis.

:failed_pause
echo.
pause
exit /b 1

:self_test
if not exist "%APP%" exit /b 2
if not exist "%LOG_ROOT%" exit /b 3
echo XUNQI_RUNTIME_LAUNCHER_SELF_TEST_OK
exit /b 0

:log
>>"%LOG_FILE%" echo [%DATE% %TIME%] event=%~1 version=0.5.1
exit /b 0
