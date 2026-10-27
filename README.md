# magic-keyboard-windows

Open-source Rust driver + tray app that makes Apple Magic Keyboards fully usable on Windows:
Fn layer, media/volume/brightness keys, Fn lock, missing PC keys (Delete, Home/End, PgUp/PgDn, Insert,
PrtSc …), F13–F19 actions, modifier remapping and battery monitoring — over USB and Bluetooth.

> **Status:** early development — see [PROGRESS.md](PROGRESS.md).

- [Software specification](docs/SPEC.md)
- [Architecture](docs/ARCHITECTURE.md)
- [HID notes](docs/HID-NOTES.md)
- [Developer setup](docs/DEV-SETUP.md)

## Important: driver signing

The kernel driver is self-built and test-signed. Loading it requires **Secure Boot off** and
**Windows test-signing mode on** (`bcdedit /set testsigning on`). This shows a "Test Mode" watermark
and some anti-cheat software refuses to run in this mode.
