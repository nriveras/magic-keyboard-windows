# magic-keyboard-windows

Open-source Rust driver + tray app that makes Apple Magic Keyboards fully usable on Windows:
Fn layer, media/volume/brightness keys, Fn lock, missing PC keys (Delete, Home/End, PgUp/PgDn, Insert,
PrtSc …), F13–F19 actions, modifier remapping and battery monitoring.

> **Status:** working over USB on the models below; Bluetooth support is in progress — see
> [PROGRESS.md](PROGRESS.md).

| Keyboard | USB ID | Status |
|---|---|---|
| Magic Keyboard with Touch ID and Numeric Keypad (2021) | 05AC:029F | tested (USB) |
| Magic Keyboard with Touch ID (2021) | 05AC:029A | tested (USB) |

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

The kernel driver is self-built and **test-signed**. Loading it requires:

1. **Secure Boot off** (firmware settings).
2. **Test-signing mode on**: in an administrator terminal run `bcdedit /set testsigning on`, then reboot.
   Windows shows a "Test Mode" watermark; some anti-cheat software refuses to run in this mode.
3. The build tools from [docs/DEV-SETUP.md](docs/DEV-SETUP.md) (Rust, Visual Studio Build Tools,
   WDK, cargo-wdk).

If BitLocker is on, have your recovery key at hand before changing Secure Boot.

## Install

Plug in the keyboard(s) over USB, then in an **administrator PowerShell** in this folder:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\install.ps1
```

This builds the release driver and the app, creates a System Restore point, installs the driver on
every connected supported keyboard, installs the app to `%LOCALAPPDATA%\Programs\MagicKeyboardWindows`
and starts it with Windows. Run it again after updating the sources, or after plugging a supported
keyboard into a new USB port for the first time.

## Use

- **Tray icon** (battery outline): hover for battery and driver state; click for the menu.
- **Settings…** opens one tab per connected keyboard. Coloured keys on the drawing are configurable:
  click one to change it, or use the panels on the right. **Apply** saves; settings survive reboots.
- **Show all key mappings** lists every Fn combination.

## Uninstall

Double-click `scripts\uninstall.cmd` (it asks for administrator rights; works with the mouse alone),
or run `scripts\uninstall.ps1` in an administrator PowerShell. Add `-RemoveSettings` to also delete
the app settings. Test-signing mode stays on: turn it off with `bcdedit /set testsigning off` and
reboot.

## If something goes wrong

- **Keyboard stops working**: double-click `scripts\uninstall.cmd` with the mouse — the stock Windows
  driver comes back.
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
