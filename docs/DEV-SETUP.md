# Developer setup

## Toolchain

| Tool | Install | Used for |
|---|---|---|
| rustup + stable `x86_64-pc-windows-msvc` | `winget install Rustlang.Rustup` | everything |
| VS 2022 Build Tools (VC tools, Windows 11 SDK 26100) | `winget install Microsoft.VisualStudio.2022.BuildTools` (+ VCTools workload) | MSVC linker, SDK headers |
| WDK 10.0.26100 | `winget install Microsoft.WindowsWDK.10.0.26100` | driver headers/libs, signtool, inf2cat |
| LLVM / clang | `winget install LLVM.LLVM` | bindgen for `wdk-sys` |
| cargo-wdk | `cargo install cargo-wdk` | building/packaging the driver |

Installed versions are recorded at the bottom of this file.

## Building

```powershell
cargo build --workspace           # host crates (mk-core, mk-probe, mk-app, mk-install)
cargo test  -p mk-core            # engine tests, no hardware needed
```

Driver (needs the WDK; first build takes about 2 min for bindgen):

```powershell
scripts\build-driver.ps1             # cargo wdk build -> driver\mkfilter\target\debug\mkfilter_package\
```

cargo-wdk test-signs the package with `CN=WDRLocalTestCert` (CurrentUser\WDRTestCertStore, valid to 2040)
and exports it as `WDRLocalTestCert.cer` into the package.

## Loading a test-signed driver

1. Disable Secure Boot in the PC's UEFI firmware settings.
2. Elevated prompt: `bcdedit /set testsigning on`, then reboot ("Test Mode" watermark appears).
3. Elevated PowerShell: `scripts\install-dev.ps1`. It checks 1–2, requests a restore point, trusts the test
   certificate (LocalMachine Root + TrustedPublisher), and force-installs `mkfilter.inf` on
   `HID\VID_05AC&PID_029F&MI_01&Col01`. The force is needed because the inbox `keyboard.inf` outranks a
   test-signed package. It then prints the device status and LowerFilters.

Only the Apple keyboard collection is affected; other keyboards and the mouse keep working.

### Watching the driver (debug builds)

Run [DebugView](https://learn.microsoft.com/sysinternals/downloads/debugview) as administrator, then enable
**Capture → Capture Kernel** and **Capture → Enable Verbose Kernel Output**. You should see
`mkfilter: DriverEntry`, `mkfilter: device added`, and one `mkfilter: report [01, …]` line per key event.
Debug builds log every report, which **includes keystrokes**, so stop the capture before typing passwords.
Release builds never log reports.

### Removing it

- Double-click `scripts\uninstall-dev.cmd`. It self-elevates and works with the mouse alone if the keyboard is
  dead. It deletes the mkfilter package(s), rescans devices so `keyboard.inf` takes over again, and deletes
  the service.
- If it doesn't work: Device Manager → Keyboards → the Apple keyboard → Uninstall device, tick
  "Attempt to remove the driver", then unplug and replug the keyboard.
- If the PC fails to boot: Safe Mode → `uninstall-dev.cmd`, or System Restore to "before mkfilter".
- Full revert: also run `bcdedit /set testsigning off`, reboot, and re-enable Secure Boot.

## Installed versions
| Tool | Version |
|---|---|
| rustc / cargo | 1.99.1 |
| VS 2022 Build Tools | 17.14.42 |
| Windows SDK / WDK | 10.0.26100 (WDK 10.1.26100.6584) |
| LLVM / clang | 23.1.4 |
