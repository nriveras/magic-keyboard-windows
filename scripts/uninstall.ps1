#Requires -RunAsAdministrator
# Removes magic-keyboard-windows: the tray app, its autostart entry, the driver packages and the
# hub device. App settings (%APPDATA%\magic-keyboard-windows) are kept unless -RemoveSettings.
param(
    [switch]$RemoveSettings
)
$ErrorActionPreference = 'Stop'

Get-Process mk-app -ErrorAction SilentlyContinue | Stop-Process -Force

$run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
Remove-ItemProperty -Path $run -Name 'MagicKeyboardWindows' -ErrorAction SilentlyContinue

$appDir = Join-Path $env:LOCALAPPDATA 'Programs\MagicKeyboardWindows'
if (Test-Path $appDir) { Remove-Item -Recurse -Force $appDir }

if ($RemoveSettings) {
    $settings = Join-Path $env:APPDATA 'magic-keyboard-windows'
    if (Test-Path $settings) { Remove-Item -Recurse -Force $settings }
}

# Driver packages, hub device and service (the driver's own settings go with the service).
& (Join-Path $PSScriptRoot 'uninstall-dev.ps1')
Write-Host 'magic-keyboard-windows removed.'
