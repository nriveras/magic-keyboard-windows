#Requires -RunAsAdministrator
# Installs the test-signed mkfilter package on the Apple keyboard collection (development only).
#
# Steps: check Secure Boot / test signing, create a restore point, trust the cargo-wdk test
# certificate, force-install the INF on the keyboard collection, and print the resulting stack.
# Undo with scripts\uninstall-dev.cmd (double-click; works with the mouse alone).
param(
    [ValidateSet('debug', 'release')]
    [string]$Profile = 'debug',
    [string]$HardwareId = 'HID\VID_05AC&PID_029F&MI_01&Col01',
    [switch]$SkipRestorePoint
)
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot -Parent
$pkg = Join-Path $root "driver\mkfilter\target\$Profile\mkfilter_package"
$inf = Join-Path $pkg 'mkfilter.inf'
$cer = Join-Path $pkg 'WDRLocalTestCert.cer'

# --- Preconditions -------------------------------------------------------------------------
try { $secureBoot = Confirm-SecureBootUEFI } catch { $secureBoot = $false }
if ($secureBoot) { throw 'Secure Boot is on. Turn it off in the firmware settings first.' }

$startOptions = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control').SystemStartOptions
if ($startOptions -notmatch 'TESTSIGNING') {
    throw "Test signing is not active. Run 'bcdedit /set testsigning on' and reboot."
}
if (-not (Test-Path $inf)) { throw "Package not found: $pkg (run scripts\build-driver.ps1 first)" }

$devices = @(Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like "$HardwareId\*" })
if ($devices.Count -eq 0) { throw "No present device matches $HardwareId. Is the keyboard connected?" }

# --- Restore point -------------------------------------------------------------------------
if (-not $SkipRestorePoint) {
    try {
        Enable-ComputerRestore -Drive "$env:SystemDrive\"
        Checkpoint-Computer -Description 'before mkfilter' -RestorePointType MODIFY_SETTINGS
        Write-Host 'Restore point requested (Windows skips it if one was created in the last 24 h).'
    } catch {
        Write-Warning "Could not create a restore point: $_"
    }
}

# --- Trust the test certificate (avoids the 'unknown publisher' prompt) --------------------
foreach ($store in 'Root', 'TrustedPublisher') {
    Import-Certificate -FilePath $cer -CertStoreLocation "Cert:\LocalMachine\$store" | Out-Null
}

# --- Install -------------------------------------------------------------------------------
# The inbox keyboard.inf outranks a test-signed package, so a plain 'pnputil /install' would not
# switch the device over. UpdateDriverForPlugAndPlayDevices with INSTALLFLAG_FORCE does.
Add-Type -Namespace MkDev -Name NewDev -MemberDefinition @'
[DllImport("newdev.dll", CharSet = CharSet.Unicode, SetLastError = true)]
public static extern bool UpdateDriverForPlugAndPlayDevicesW(
    IntPtr hwndParent, string hardwareId, string fullInfPath, uint installFlags, out bool rebootRequired);
'@
$reboot = $false
if (-not [MkDev.NewDev]::UpdateDriverForPlugAndPlayDevicesW([IntPtr]::Zero, $HardwareId, $inf, 1, [ref]$reboot)) {
    $err = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
    throw ("UpdateDriverForPlugAndPlayDevices failed: 0x{0:X8}" -f $err)
}

# --- Report --------------------------------------------------------------------------------
foreach ($d in $devices) {
    $props = Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName `
        DEVPKEY_Device_Service, DEVPKEY_Device_LowerFilters, DEVPKEY_Device_DriverInfPath
    $now = Get-PnpDevice -InstanceId $d.InstanceId
    Write-Host "$($d.InstanceId)"
    Write-Host "  status: $($now.Status)  problem: $($now.Problem)"
    $props | ForEach-Object { Write-Host "  $($_.KeyName): $($_.Data)" }
}
if ($reboot) { Write-Warning 'Windows asks for a reboot to finish the installation.' }
