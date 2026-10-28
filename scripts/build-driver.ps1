# Builds, packages and test-signs driver\mkfilter with cargo-wdk.
# Output: driver\mkfilter\target\<profile>\mkfilter_package\
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
Write-Host "Package: $(Join-Path $driver "target\$Profile\mkfilter_package")"
