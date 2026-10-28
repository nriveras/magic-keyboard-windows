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

Driver: see `driver/mkfilter/README.md` (added in M3).

## Loading a test-signed driver

1. Disable Secure Boot in the PC's UEFI firmware settings.
2. Elevated prompt: `bcdedit /set testsigning on`, then reboot ("Test Mode" watermark appears).
3. Create a System Restore point.
4. Run `scripts/install-dev.ps1` (elevated).

To revert: `scripts/uninstall-dev.ps1`, `bcdedit /set testsigning off`, reboot, re-enable Secure Boot.
If the machine fails to boot after installing the driver: boot Safe Mode and run `uninstall-dev.ps1`,
or restore the restore point.

## Installed versions
| Tool | Version |
|---|---|
| rustc / cargo | 1.99.1 |
| VS 2022 Build Tools | 17.14.42 |
| Windows SDK / WDK | 10.0.26100 (WDK 10.1.26100.6584) |
| LLVM / clang | 23.1.4 |
