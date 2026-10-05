@echo off
setlocal
cd /d "%~dp0"

echo.
echo === DevCast Windows build ===
echo Working directory: %CD%
echo.

PowerShell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-windows.ps1" -NoPause
set "BUILD_EXIT_CODE=%ERRORLEVEL%"

echo.
if not "%BUILD_EXIT_CODE%"=="0" (
  echo Build did not complete. Exit code: %BUILD_EXIT_CODE%
) else (
  echo Build script completed successfully.
)
echo.
pause
exit /b %BUILD_EXIT_CODE%
