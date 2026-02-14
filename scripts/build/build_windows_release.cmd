@echo off
setlocal
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build_windows_release.ps1"
if errorlevel 1 exit /b %errorlevel%
endlocal
