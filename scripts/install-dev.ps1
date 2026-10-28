#Requires -RunAsAdministrator
# Installs the test-signed mkfilter packages (development only):
#   - mkfilter.inf on the Apple keyboard collection (lower filter)
#   - mkhub.inf on a root-enumerated ROOT\MagicKbdHub devnode (virtual HID device for media keys)
#
# Steps: check Secure Boot / test signing, create a restore point, trust the cargo-wdk test
# certificate, force-install the filter INF, create the hub devnode if needed, install the hub INF,
# and print the resulting devices.
# Undo with scripts\uninstall-dev.cmd (double-click; works with the mouse alone).
param(
    [ValidateSet('debug', 'release')]
    [string]$Profile = 'debug',
    # Keyboard collections listed in mkfilter.inx; the filter is installed on each one present.
    [string[]]$HardwareIds = @(
        'HID\VID_05AC&PID_029F&MI_01&Col01',  # Magic Keyboard with Touch ID and Numeric Keypad, USB
        'HID\VID_05AC&PID_029A&MI_01&Col01',  # Magic Keyboard with Touch ID, USB
        'HID\{00001124-0000-1000-8000-00805f9b34fb}_VID&0001004c_PID&029f&Col01',  # same, Bluetooth
        'HID\{00001124-0000-1000-8000-00805f9b34fb}_VID&0001004c_PID&029a&Col01'
    ),
    [switch]$SkipRestorePoint
)
$ErrorActionPreference = 'Stop'

$HubHardwareId = 'ROOT\MagicKbdHub'
$root = Split-Path $PSScriptRoot -Parent
$target = Join-Path $root "driver\mkfilter\target\$Profile"
$inf = Join-Path $target 'mkfilter_package\mkfilter.inf'
$hubInf = Join-Path $target 'mkhub_package\mkhub.inf'
$cer = Join-Path $target 'mkfilter_package\WDRLocalTestCert.cer'

# --- Preconditions -------------------------------------------------------------------------
try { $secureBoot = Confirm-SecureBootUEFI } catch { $secureBoot = $false }
if ($secureBoot) { throw 'Secure Boot is on. Turn it off in the firmware settings first.' }

$startOptions = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control').SystemStartOptions
if ($startOptions -notmatch 'TESTSIGNING') {
    throw "Test signing is not active. Run 'bcdedit /set testsigning on' and reboot."
}
foreach ($f in $inf, $hubInf) {
    if (-not (Test-Path $f)) { throw "Package not found: $f (run scripts\build-driver.ps1 first)" }
}

$present = Get-PnpDevice -PresentOnly
$devices = @($present | Where-Object { $id = $_.InstanceId; $HardwareIds | Where-Object { $id -like "$_\*" } })
$presentIds = @($HardwareIds | Where-Object { $hw = $_; $present | Where-Object { $_.InstanceId -like "$hw\*" } })
if ($devices.Count -eq 0) { throw "No supported keyboard is connected ($($HardwareIds -join ', '))." }

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

# --- Native helpers ------------------------------------------------------------------------
Add-Type -Namespace MkDev -Name Native -MemberDefinition @'
[DllImport("newdev.dll", CharSet = CharSet.Unicode, SetLastError = true)]
public static extern bool UpdateDriverForPlugAndPlayDevicesW(
    IntPtr hwndParent, string hardwareId, string fullInfPath, uint installFlags, out bool rebootRequired);

[StructLayout(LayoutKind.Sequential)]
public struct SP_DEVINFO_DATA { public uint cbSize; public Guid ClassGuid; public uint DevInst; public IntPtr Reserved; }

[DllImport("setupapi.dll", SetLastError = true)]
public static extern IntPtr SetupDiCreateDeviceInfoList(ref Guid classGuid, IntPtr hwndParent);
[DllImport("setupapi.dll", CharSet = CharSet.Unicode, SetLastError = true)]
public static extern bool SetupDiCreateDeviceInfoW(IntPtr set, string deviceName, ref Guid classGuid,
    string description, IntPtr hwndParent, uint flags, ref SP_DEVINFO_DATA data);
[DllImport("setupapi.dll", SetLastError = true)]
public static extern bool SetupDiSetDeviceRegistryPropertyW(IntPtr set, ref SP_DEVINFO_DATA data,
    uint property, byte[] buffer, uint size);
[DllImport("setupapi.dll", SetLastError = true)]
public static extern bool SetupDiCallClassInstaller(uint function, IntPtr set, ref SP_DEVINFO_DATA data);
[DllImport("setupapi.dll", SetLastError = true)]
public static extern bool SetupDiDestroyDeviceInfoList(IntPtr set);

// Creates a root-enumerated devnode (ROOT\SYSTEM\nnnn) with the given hardware ID, like devcon install.
public static void CreateRootDevice(string hardwareId) {
    Guid system = new Guid("4d36e97d-e325-11ce-bfc1-08002be10318");
    IntPtr set = SetupDiCreateDeviceInfoList(ref system, IntPtr.Zero);
    if (set == new IntPtr(-1)) throw new System.ComponentModel.Win32Exception();
    try {
        SP_DEVINFO_DATA data = new SP_DEVINFO_DATA();
        data.cbSize = (uint)Marshal.SizeOf(typeof(SP_DEVINFO_DATA));
        if (!SetupDiCreateDeviceInfoW(set, "System", ref system, null, IntPtr.Zero, 1 /* DICD_GENERATE_ID */, ref data))
            throw new System.ComponentModel.Win32Exception();
        byte[] ids = System.Text.Encoding.Unicode.GetBytes(hardwareId + "\0\0");
        if (!SetupDiSetDeviceRegistryPropertyW(set, ref data, 1 /* SPDRP_HARDWAREID */, ids, (uint)ids.Length))
            throw new System.ComponentModel.Win32Exception();
        if (!SetupDiCallClassInstaller(0x19 /* DIF_REGISTERDEVICE */, set, ref data))
            throw new System.ComponentModel.Win32Exception();
    } finally {
        SetupDiDestroyDeviceInfoList(set);
    }
}
'@

# The inbox keyboard.inf outranks a test-signed package, so a plain 'pnputil /install' would not
# switch the device over. UpdateDriverForPlugAndPlayDevices with INSTALLFLAG_FORCE does.
function Install-Inf([string]$Id, [string]$Path) {
    $reboot = $false
    if (-not [MkDev.Native]::UpdateDriverForPlugAndPlayDevicesW([IntPtr]::Zero, $Id, $Path, 1, [ref]$reboot)) {
        $err = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        throw ("UpdateDriverForPlugAndPlayDevices($Id) failed: 0x{0:X8}" -f $err)
    }
    if ($reboot) { Write-Warning "Windows asks for a reboot to finish installing $Id." }
}

function Get-HubDevices {
    @(Get-PnpDevice | Where-Object { $_.HardwareID -contains $HubHardwareId })
}

# --- Install -------------------------------------------------------------------------------
foreach ($id in $presentIds) { Install-Inf $id $inf }

if ((Get-HubDevices).Count -eq 0) {
    [MkDev.Native]::CreateRootDevice($HubHardwareId)
    Write-Host "Created $HubHardwareId devnode."
}
Install-Inf $HubHardwareId $hubInf

# --- Report --------------------------------------------------------------------------------
foreach ($d in @($devices) + (Get-HubDevices)) {
    $props = Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName `
        DEVPKEY_Device_Service, DEVPKEY_Device_LowerFilters, DEVPKEY_Device_DriverInfPath
    $now = Get-PnpDevice -InstanceId $d.InstanceId
    Write-Host "$($d.InstanceId)"
    Write-Host "  status: $($now.Status)  problem: $($now.Problem)"
    $props | ForEach-Object { Write-Host "  $($_.KeyName): $($_.Data)" }
}
