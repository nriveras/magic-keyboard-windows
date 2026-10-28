#Requires -RunAsAdministrator
# Removes every installed mkfilter package; the keyboard falls back to the inbox keyboard.inf.
$ErrorActionPreference = 'Stop'

$packages = @(Get-WindowsDriver -Online | Where-Object { $_.OriginalFileName -like '*\mkfilter.inf' })
if ($packages.Count -eq 0) { Write-Host 'No mkfilter driver package installed.' }
foreach ($p in $packages) {
    Write-Host "Removing $($p.Driver) ($($p.OriginalFileName))"
    pnputil /delete-driver $p.Driver /uninstall /force
}

# Re-enumerate so the keyboard collection gets the inbox driver again.
pnputil /scan-devices | Out-Null

if (Get-Service mkfilter -ErrorAction SilentlyContinue) {
    sc.exe delete mkfilter | Out-Null
}

$left = Get-PnpDevice -PresentOnly | Where-Object { $_.Class -eq 'Keyboard' } | ForEach-Object {
    $f = (Get-PnpDeviceProperty -InstanceId $_.InstanceId -KeyName DEVPKEY_Device_LowerFilters).Data
    if ($f -contains 'mkfilter') { $_.InstanceId }
}
if ($left) {
    Write-Warning "Still filtered (unplug/replug or reboot): $($left -join ', ')"
} else {
    Write-Host 'mkfilter removed.'
}
