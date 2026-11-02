# magic-keyboard-windows

[![CI](https://github.com/nriveras/magic-keyboard-windows/actions/workflows/ci.yml/badge.svg)](https://github.com/nriveras/magic-keyboard-windows/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Open-source Rust driver + tray app that makes Apple Magic Keyboards fully usable on Windows:
Fn layer, media/volume/brightness keys, Fn lock, missing PC keys (Delete, Home/End, PgUp/PgDn, Insert,
PrtSc …), F13–F19 actions, modifier remapping and battery monitoring.

> **Status:** working over USB and Bluetooth on the models below — see [PROGRESS.md](PROGRESS.md).

| Keyboard | USB ID | Bluetooth ID | Status |
|---|---|---|---|
| Magic Keyboard with Touch ID and Numeric Keypad (2021) | 05AC:029F | 004C:029F | tested (USB, Bluetooth) |
| Magic Keyboard with Touch ID (2021) | 05AC:029A | 004C:029A | tested (USB, Bluetooth) |

Over Bluetooth the vendor ID is Apple's Bluetooth company ID (`004C`) instead of its USB vendor ID
(`05AC`); the hardware IDs look like
`HID\{00001124-0000-1000-8000-00805f9b34fb}_VID&0001004c_PID&029F`.

## What you get

- **Top row**: brightness, Task View, Search, Dictation, sleep / Do Not Disturb, media and volume keys —
  or plain F1–F12, switchable from the tray (hold Fn for the other function).
- **Missing keys**: Fn+⌫ Delete, Fn+arrows Home/End/PgUp/PgDn, Fn+Return Insert, Fn+Esc Pause,
  Fn+P Print Screen, Fn+Ctrl+Option+⌫ Ctrl+Alt+Del.
- **Lock / Touch ID key** locks the computer (Win+L). Touch ID itself is not supported.
- **F13–F19** (keypad model): Print Screen / Scroll Lock / Pause, and four program launchers.
- **Modifier remapping** with presets (Apple positions, PC order, Mac-like shortcuts).
- **Tray app**: battery level of each keyboard, low-battery warning, top-row mode, settings window
  with a drawn keyboard — click a key to change it.

Everything except the tray app runs in the driver, so it also works at the login screen.

## Requirements

**Windows 11** (64-bit, build 22000 or later): the driver uses KMDF 1.33, which Windows 10 does not
have, and the installer refuses older versions.

The kernel driver is self-built and **test-signed**. Loading it requires:

1. **Secure Boot off** (firmware settings).
2. **Test-signing mode on**: in an administrator terminal run `bcdedit /set testsigning on`, then reboot.
   Windows shows a "Test Mode" watermark; some anti-cheat software refuses to run in this mode.
3. The build tools from [docs/DEV-SETUP.md](docs/DEV-SETUP.md) (Rust, Visual Studio Build Tools,
   WDK, cargo-wdk).

If BitLocker is on, have your recovery key at hand before changing Secure Boot.

> **Secure Boot must stay off while the driver is installed.** Windows checks driver signatures at
> every boot: with Secure Boot back on, test mode is disabled, the driver is rejected (Device Manager
> code 52) and the Magic Keyboards **stop working entirely** — the driver sits in their driver chain.
> Other keyboards and the mouse keep working. The tray app warns when this happens. Fix it by turning
> Secure Boot off again, or by running `scripts\uninstall.cmd` before (or after) turning it on.
> Running with Secure Boot on requires a Microsoft-signed driver (EV certificate + attestation
> signing), which this project does not have.

## Install

Plug in the keyboard(s) over USB, then in an **administrator PowerShell** in this folder:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\install.ps1
```

This builds the release driver and the app, creates a System Restore point, installs the driver on
every connected supported keyboard, installs the app to `%LOCALAPPDATA%\Programs\MagicKeyboardWindows`
and starts it with Windows. Run it again after updating the sources, after plugging a supported
keyboard into a new USB port for the first time, or after pairing a keyboard over Bluetooth.

When updating, the script usually ends with **"Restart Windows to load the new driver version"**:
the previous driver keeps running until you restart.

## Use

- **Tray icon** (battery outline): hover for battery and driver state; click for the menu.
- **Settings…** opens one tab per connected keyboard. Coloured keys on the drawing are configurable:
  click one to change it, or use the panels on the right. **Apply** saves; settings survive reboots.
- **Show all key mappings** lists every Fn combination.

## Uninstall

Double-click `scripts\uninstall.cmd` (it asks for administrator rights; works with the mouse alone),
or run `scripts\uninstall.ps1` in an administrator PowerShell. Add `-RemoveSettings` to also delete
the app settings. Test-signing mode stays on: once the driver is removed, turn it off with
`bcdedit /set testsigning off` and reboot.

## If something goes wrong

- **Keyboard stops working**: double-click `scripts\uninstall.cmd` with the mouse — the stock Windows
  driver comes back.
- **Keyboard stopped working after turning Secure Boot on** (tray: "driver failed to load"): turn
  Secure Boot off again, or run `scripts\uninstall.cmd` — see the note under Requirements.
- **Keyboard stopped working after `bcdedit /set testsigning off`** (the tray says test-signing mode
  is off): turn it back on with `bcdedit /set testsigning on` and restart, or run
  `scripts\uninstall.cmd`.
- **Windows does not start**: boot into Safe Mode and run `scripts\uninstall.cmd`, or use System
  Restore → "before mkfilter".

## Privacy

Key events never leave the kernel driver: the app only exchanges configuration and status with it.
Release builds never log keystrokes. **Debug builds** (`scripts\install-dev.ps1`, for development)
log every input report to the kernel debugger output — do not use them for daily typing.

## Documentation

- [Software specification](docs/SPEC.md)
- [Architecture](docs/ARCHITECTURE.md)
- [HID notes](docs/HID-NOTES.md)
- [Developer setup](docs/DEV-SETUP.md)
- [Comparison with Brigadier + Boot Camp](docs/COMPARISON-BRIGADIER.md)

## Contributing

Bug reports, tests on other Apple keyboards and pull requests are welcome — see
[CONTRIBUTING.md](CONTRIBUTING.md). Report security problems privately as described in
[SECURITY.md](SECURITY.md).

## License and disclaimer

[MIT](LICENSE).

This is an independent project, not affiliated with or endorsed by Apple Inc. Apple, Magic Keyboard
and Touch ID are trademarks of Apple Inc., used here only to say which hardware the project works with.

The software installs a test-signed kernel driver and is provided as is, without warranty of any kind.
Read the Requirements section before installing; you use it at your own risk.
