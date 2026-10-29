#Requires -RunAsAdministrator
# Removes magic-keyboard-windows: the driver packages and hub device, the tray app and its
# autostart entry. App settings (%APPDATA%\magic-keyboard-windows) are kept unless -RemoveSettings.
param(
    [switch]$RemoveSettings
)
$ErrorActionPreference = 'Stop'

$app = @(Get-Process mk-app -ErrorAction SilentlyContinue)
$app | Stop-Process -Force -ErrorAction SilentlyContinue

$run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
Remove-ItemProperty -Path $run -Name 'MagicKeyboardWindows' -ErrorAction SilentlyContinue

# Driver packages, hub device and service (the driver's own settings go with the service). First,
# so a problem removing the app files below can never leave the driver in the keyboard stack.
& (Join-Path $PSScriptRoot 'uninstall-dev.ps1')

function Remove-Folder([string]$Path) {
    if (-not (Test-Path $Path)) { return }
    try {
        Remove-Item -Recurse -Force $Path
    } catch {
        Write-Warning "Could not remove $Path ($($_.Exception.Message)). Delete it after a restart."
    }
}

# Stop-Process does not wait for the exit; the app's files stay locked until then.
if ($app.Count -gt 0) { $app | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue }
Remove-Folder (Join-Path $env:LOCALAPPDATA 'Programs\MagicKeyboardWindows')
if ($RemoveSettings) {
    Remove-Folder (Join-Path $env:APPDATA 'magic-keyboard-windows')
}

Write-Host 'magic-keyboard-windows removed.'
