@echo off
rem Double-click uninstall: removes the app and the driver, works with the mouse alone.
rem Self-elevates, then runs uninstall.ps1.
net session >nul 2>&1
if errorlevel 1 (
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0uninstall.ps1"
pause
