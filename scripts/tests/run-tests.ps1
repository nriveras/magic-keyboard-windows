# Tests for the install/uninstall scripts and the driver INFs. They never touch the installed
# driver or the running app: scripts run as copies in a temporary folder with stubbed commands.
#   powershell -ExecutionPolicy Bypass -File scripts\tests\run-tests.ps1
$ErrorActionPreference = 'Stop'

$root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$scripts = Join-Path $root 'scripts'
$driver = Join-Path $root 'driver\mkfilter'
$script:failed = 0

function Test([string]$Name, [scriptblock]$Body) {
    try {
        & $Body
        Write-Host "ok    $Name"
    } catch {
        $script:failed++
        Write-Host "FAIL  $Name" -ForegroundColor Red
        Write-Host "      $($_.Exception.Message)"
    }
}

function Assert([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function New-TempDir([string]$Name) {
    $dir = Join-Path ([IO.Path]::GetTempPath()) "mk-tests-$([guid]::NewGuid().ToString('N').Substring(0, 8))\$Name"
    New-Item -ItemType Directory $dir | Out-Null
    $dir
}

function Test-Admin {
    ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
}

# --- uninstall.ps1 -------------------------------------------------------------------------------

Test 'uninstall.ps1 removes the driver even when the app folder cannot be deleted' {
    $dir = New-TempDir 'uninstall'
    $copy = Join-Path $dir 'scripts'
    New-Item -ItemType Directory $copy | Out-Null
    # The copy runs unelevated: drop the #Requires line.
    Get-Content (Join-Path $scripts 'uninstall.ps1') | Where-Object { $_ -notmatch '^#Requires' } |
        Set-Content (Join-Path $copy 'uninstall.ps1')
    $marker = Join-Path $dir 'driver-removed'
    Set-Content (Join-Path $copy 'uninstall-dev.ps1') "Set-Content '$marker' 'yes'"

    $local = Join-Path $dir 'local'
    $appDir = Join-Path $local 'Programs\MagicKeyboardWindows'
    New-Item -ItemType Directory $appDir | Out-Null
    $exe = Join-Path $appDir 'mk-app.exe'
    Set-Content $exe 'stub'
    # Like an mk-app.exe that has not exited yet: the delete fails.
    $lock = [IO.File]::Open($exe, 'Open', 'Read', 'None')
    try {
        $command = "function Get-Process {}; function Stop-Process {}; function Remove-ItemProperty {}; " +
            "`$env:LOCALAPPDATA = '$local'; `$env:APPDATA = '$dir\roaming'; " +
            "& '$(Join-Path $copy 'uninstall.ps1')'"
        $ErrorActionPreference = 'Continue'  # stderr of native commands must not throw
        $output = powershell -NoProfile -ExecutionPolicy Bypass -Command $command 2>&1 | Out-String
    } finally {
        $lock.Close()
    }
    Assert (Test-Path $marker) "uninstall-dev.ps1 did not run. Output:`n$output"
}

# --- uninstall.cmd / uninstall-dev.cmd -----------------------------------------------------------

foreach ($pair in @(@('uninstall.cmd', 'uninstall.ps1'), @('uninstall-dev.cmd', 'uninstall-dev.ps1'))) {
    $cmdName, $ps1Name = $pair

    Test "$cmdName does not rely on the Server service to detect elevation" {
        $text = Get-Content -Raw (Join-Path $scripts $cmdName)
        Assert ($text -notmatch 'net\s+session') "$cmdName still uses 'net session'"
        Assert ($text -notmatch "'%~f0'") "$cmdName quotes its own path with apostrophes"
    }

    Test "$cmdName stops instead of relaunching again once elevation was attempted" {
        # The folder name contains an apostrophe and a space on purpose.
        $dir = New-TempDir "it's here"
        Copy-Item (Join-Path $scripts $cmdName) $dir
        $marker = Join-Path $dir 'ran'
        Set-Content (Join-Path $dir $ps1Name) "Set-Content '$($marker -replace "'", "''")' 'yes'"
        $ErrorActionPreference = 'Continue'
        $output = cmd /c "`"$(Join-Path $dir $cmdName)`" elevated < nul" 2>&1 | Out-String
        if (Test-Admin) {
            Assert (Test-Path $marker) "$ps1Name did not run elevated. Output:`n$output"
        } else {
            Assert (-not (Test-Path $marker)) "$ps1Name ran without administrator rights"
            Assert ($output -match 'administrator') "no explanation shown. Output:`n$output"
        }
    }
}

# --- Driver packages ------------------------------------------------------------------------------

Test 'INFs only install on Windows versions that ship the KMDF version the driver needs' {
    $toml = Get-Content -Raw (Join-Path $driver 'Cargo.toml')
    $minor = [int]([regex]::Match($toml, 'target-kmdf-version-minor\s*=\s*(\d+)').Groups[1].Value)
    # First Windows build that ships each KMDF 1.x runtime.
    $firstBuild = @{ 31 = 19041; 33 = 22000 }
    Assert $firstBuild.ContainsKey($minor) "unknown KMDF minor version $minor"
    foreach ($inx in 'mkfilter.inx', 'mkhub.inx') {
        $text = Get-Content -Raw (Join-Path $driver $inx)
        $builds = [regex]::Matches($text, 'NT\$ARCH\$\.10\.0\.\.\.(\d+)') | ForEach-Object { [int]$_.Groups[1].Value }
        Assert ($builds.Count -gt 0) "$inx has no OS-decorated models section"
        foreach ($b in $builds) {
            Assert ($b -ge $firstBuild[$minor]) "$inx allows build $b, KMDF 1.$minor needs $($firstBuild[$minor])"
        }
    }
}

Test 'install-dev.ps1 refuses Windows builds older than the driver supports' {
    $text = Get-Content -Raw (Join-Path $scripts 'install-dev.ps1')
    Assert ($text -match 'CurrentBuild') 'install-dev.ps1 does not check the Windows build'
    Assert ($text -match '22000') 'install-dev.ps1 does not require build 22000'
}

Test 'README states the Windows 11 requirement' {
    $text = Get-Content -Raw (Join-Path $root 'README.md')
    Assert ($text -match 'Windows 11') 'README does not mention Windows 11'
}

Write-Host ''
if ($script:failed -gt 0) {
    Write-Host "$script:failed test(s) failed" -ForegroundColor Red
    exit 1
}
Write-Host 'All script tests passed.'
