#Requires -RunAsAdministrator
# Builds and installs magic-keyboard-windows for everyday use:
#   - release driver (no keystroke logging) on every connected supported keyboard + the hub device
#   - mk-app.exe copied to %LOCALAPPDATA%\Programs\MagicKeyboardWindows, started with Windows
# Run from an elevated PowerShell in the repository. Undo with scripts\uninstall.ps1 (or uninstall.cmd).
param(
    [switch]$SkipBuild,
    [switch]$SkipRestorePoint
)
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot -Parent
$appDir = Join-Path $env:LOCALAPPDATA 'Programs\MagicKeyboardWindows'
$appExe = Join-Path $appDir 'mk-app.exe'

if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot 'build-driver.ps1') -Profile release
    Push-Location $root
    try {
        cargo build --release -p mk-app
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }
    } finally {
        Pop-Location
    }
}

# --- Driver --------------------------------------------------------------------------------
$driverArgs = @{ Profile = 'release' }
if ($SkipRestorePoint) { $driverArgs.SkipRestorePoint = $true }
& (Join-Path $PSScriptRoot 'install-dev.ps1') @driverArgs

# --- App -----------------------------------------------------------------------------------
Get-Process mk-app -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
New-Item -ItemType Directory -Force $appDir | Out-Null
Copy-Item (Join-Path $root 'target\release\mk-app.exe') $appExe -Force

$run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
Set-ItemProperty -Path $run -Name 'MagicKeyboardWindows' -Value "`"$appExe`""

# Explorer starts the app without administrator rights, like a normal login would.
Start-Process explorer.exe $appExe
Write-Host "Installed. App: $appExe (starts with Windows)."
