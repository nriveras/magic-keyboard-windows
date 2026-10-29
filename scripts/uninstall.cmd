@echo off
rem Double-click uninstall: removes the app and the driver, works with the mouse alone.
rem Self-elevates once (the relaunch gets "elevated"), then runs uninstall.ps1.
rem fltmc only succeeds elevated and, unlike a "net" check, works without the Server service.
fltmc >nul 2>&1
if errorlevel 1 (
    if "%~1"=="elevated" (
        echo This needs administrator rights. Right-click %~nx0 and choose "Run as administrator".
        pause
        exit /b 1
    )
    rem Passed through the environment so no quoting can break on the path.
    set "MK_SELF=%~f0"
    powershell -NoProfile -Command "Start-Process -FilePath $env:MK_SELF -ArgumentList elevated -Verb RunAs"
    exit /b
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0uninstall.ps1"
pause
