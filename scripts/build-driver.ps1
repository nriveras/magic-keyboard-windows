# Builds, packages and test-signs driver\mkfilter with cargo-wdk, then assembles the hub package
# (mkhub.inf + the same mkfilter.sys) with the WDK tools, signed with the same test certificate.
# Output: driver\mkfilter\target\<profile>\mkfilter_package\ and ...\mkhub_package\
param(
    [ValidateSet('debug', 'release')]
    [string]$Profile = 'debug'
)
$ErrorActionPreference = 'Stop'

$driver = Join-Path (Split-Path $PSScriptRoot -Parent) 'driver\mkfilter'
Push-Location $driver
try {
    if ($Profile -eq 'release') { cargo wdk build --profile release } else { cargo wdk build }
    if ($LASTEXITCODE -ne 0) { throw "cargo wdk build failed ($LASTEXITCODE)" }
} finally {
    Pop-Location
}
$filterPkg = Join-Path $driver "target\$Profile\mkfilter_package"
Write-Host "Package: $filterPkg"

# --- Hub package ---------------------------------------------------------------------------
$kitBin = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$sdk = Get-ChildItem $kitBin -Directory | Where-Object { Test-Path (Join-Path $_.FullName 'x64\stampinf.exe') } |
    Sort-Object { [version]$_.Name } | Select-Object -Last 1
if (-not $sdk) { throw "WDK tools not found under $kitBin" }
$stampinf = Join-Path $sdk.FullName 'x64\stampinf.exe'
$signtool = Join-Path $sdk.FullName 'x64\signtool.exe'
$inf2cat = Join-Path $sdk.FullName 'x86\Inf2Cat.exe'

$hubPkg = Join-Path $driver "target\$Profile\mkhub_package"
if (Test-Path $hubPkg) { Remove-Item -Recurse -Force $hubPkg }
New-Item -ItemType Directory $hubPkg | Out-Null
Copy-Item (Join-Path $driver 'mkhub.inx') (Join-Path $hubPkg 'mkhub.inf')
Copy-Item (Join-Path $filterPkg 'mkfilter.sys'), (Join-Path $filterPkg 'WDRLocalTestCert.cer') $hubPkg

# Same DriverVer as the filter package so both INFs describe one build.
$driverVer = (Select-String -Path (Join-Path $filterPkg 'mkfilter.inf') -Pattern '^DriverVer\s*=\s*(.+)$').Matches[0].Groups[1].Value.Trim()
$date, $version = $driverVer -split ','

Push-Location $hubPkg
try {
    & $stampinf -f mkhub.inf -a amd64 -c mkhub.cat -k 1.33 -d $date -v $version
    if ($LASTEXITCODE -ne 0) { throw "stampinf failed ($LASTEXITCODE)" }
    & $inf2cat "/driver:$hubPkg" /os:10_x64 /uselocaltime | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "inf2cat failed ($LASTEXITCODE)" }
    & $signtool sign /s WDRTestCertStore /n WDRLocalTestCert /fd SHA256 mkhub.cat | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "signtool failed ($LASTEXITCODE)" }
} finally {
    Pop-Location
}
Write-Host "Package: $hubPkg"
