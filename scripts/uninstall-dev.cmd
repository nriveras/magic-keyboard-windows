@echo off
rem Double-click recovery: removes mkfilter even when the Apple keyboard does not work.
rem Self-elevates, then runs uninstall-dev.ps1.
net session >nul 2>&1
if errorlevel 1 (
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0uninstall-dev.ps1"
pause
