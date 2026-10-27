# Software Specification

Reference: [Magic Utilities – Magic Keyboard features](https://magicutilities.net/magic-keyboard/features)
and [key mappings](https://magicutilities.net/magic-keyboard/help/key-mappings). This project
re-implements the keyboard feature set as open-source Rust; it does not copy any of their code or assets.

Priorities: **P0** = required for 1.0, **P1** = nice to have, **OOS** = out of scope.

## 1. Supported devices

| Model | USB PID (VID 05AC) | BT (VID 004C) | Priority |
|---|---|---|---|
| Magic Keyboard with Touch ID and Numeric Keypad (2021, A2520) | 029F | 029F | **P0 (primary test device)** |
| Magic Keyboard with Touch ID (2021, A2449) | 029A | 029A | P1 |
| Magic Keyboard (2021, A2450) | 029C | 029C | P1 |
| Magic Keyboard (2015, A1644) | 0267 | 0267 | P1 |
| Magic Keyboard with Numeric Keypad (2017, A1843) | 026C | 026C | P1 |
| Magic Keyboard USB‑C (2024) family | 0320–0322 | same | P1 |
| Apple Wireless Keyboard (aluminium, AA batteries) | — | various | OOS |

Connectivity: **USB and Bluetooth (P0)**. Charging over USB works natively; no feature needed.

## 2. Fn key and function row

Physical keys F1–F12 produce *either* the standard F-key *or* the special function below.
Which one is the default is controlled by **Fn lock**; holding Fn selects the other one.

| Key | Special function | Shift + key (special mode) |
|---|---|---|
| F1 | Brightness down (consumer 0x70) | |
| F2 | Brightness up (consumer 0x6F) | |
| F3 | Task View (Win+Tab) | |
| F4 | Search (Win+S) | |
| F5 | Dictation (Win+H) | |
| F6 | Configurable: Sleep (default) / Do Not Disturb (Win+N) / none | |
| F7 | Previous track | Rewind |
| F8 | Play / Pause | Stop |
| F9 | Next track | Fast forward |
| F10 | Mute | |
| F11 | Volume down | |
| F12 | Volume up | |

- **FN-01 (P0)** Fn key state is detected from HID usage `0x00FF:0x0003`.
- **FN-02 (P0)** Fn lock setting: `media-first` (default, like macOS) or `fkeys-first`.
- **FN-03 (P0)** Fn lock can be toggled from the tray menu.
- **FN-04 (P1)** Fn lock toggle by double‑tapping Fn (configurable: off / Fn lock / Dictation).
- **FN-05 (P0)** A key's output is latched at press time; its release always releases what was
  pressed, even if Fn/Shift changed in between (no stuck keys).
- **FN-06 (P1)** Sleep shortcut Fn+Fn+Fn+Space (triple-tap Fn, then Space) — can be disabled.

## 3. Missing Windows keys (always available while holding Fn)

| Windows key | Apple combination |
|---|---|
| Delete | Fn + ⌫ (delete) |
| Insert | Fn + Return |
| Home / End | Fn + ← / Fn + → |
| Page Up / Page Down | Fn + ↑ / Fn + ↓ |
| Pause/Break | Fn + Esc |
| Print Screen | Fn + P (keeps other modifiers, e.g. Alt+PrtSc) |
| Print Screen | Fn + Left Shift + F11 (Left Alt/Win held are passed through) |
| Scroll Lock | Fn + Left Shift + F12 |
| Menu (▤) | Shift+F10 works natively; Menu consumer usage 0x40 → Apps key |
| Ctrl+Alt+Del | Fn + Ctrl + Option + ⌫ (Fn turns ⌫ into Delete) |

- **MK-01 (P0)** all rows above.

## 4. Extended function keys (numeric keypad models)

| Key | Default | Configurable |
|---|---|---|
| F13 | Print Screen | P1 |
| F14 | Scroll Lock | P1 |
| F15 | Pause/Break | P1 |
| F16 | Launch File Explorer | **P0** (app-handled) |
| F17 | Launch Calculator | **P0** |
| F18 | Launch Notepad | **P0** |
| F19 | Launch Task Manager | **P0** |

F16–F19 actions are executed by the tray app (any program path + arguments, or "pass through as F-key").

## 5. Modifier remapping

- **MOD-01 (P0)** Each physical modifier — Left/Right Control, Option, Command, Shift, Caps Lock, Fn —
  can be mapped to: Ctrl, Alt, Win, Shift, Caps Lock, Fn, AltGr (Right Alt), Menu (Apps), Insert, Disabled.
- **MOD-02 (P0)** Presets:
  - *Windows positions* (default): Command → Win, Option → Alt, Control → Ctrl.
  - *PC order* (swap Option/Command): Command → Alt, Option → Win — matches the physical
    Ctrl‑Win‑Alt order of a PC keyboard.
  - *Mac-like shortcuts*: Command → Ctrl, Control → Win, Option → Alt — so ⌘C/⌘V copy/paste.
- **MOD-03 (P0)** Right Command → AltGr for third characters (requires a matching keyboard layout).
- **MOD-04 (P1)** Swap Fn and Left Control (like WinAppleKey) — falls out of MOD-01.

## 6. Other keys

- **KEY-01 (P0)** Keypad Clear (usage 0x53): Num Lock (default) / Delete / Backspace / Disabled.
- **KEY-02 (P0)** Lock / Touch ID key (consumer 0x19E): Lock workstation (Win+L, default) / Disabled /
  Insert (MU option: "fn key can be assigned to Insert").
- **KEY-03 (P1)** Eject (consumer 0xB8, older models): Delete (default), Fn+Eject = Insert; on numeric
  keypad models Eject acts as Fn.

## 7. Battery

- **BAT-01 (P0)** Battery percentage and charging state shown in the tray icon tooltip / menu.
- **BAT-02 (P0)** Low battery notification (Windows toast) at a configurable threshold (default 15 %), repeated at 5 %.
- **BAT-03 (P0)** Works on USB and Bluetooth.

## 8. Application

- **APP-01 (P0)** Tray icon with: battery, Fn lock toggle, Settings…, Show key mappings…, Start with Windows, Quit.
- **APP-02 (P0)** Settings window covering every configurable item in sections 2–7; changes apply live.
- **APP-03 (P0)** Settings are stored as TOML in `%APPDATA%\MagicKeyboard\config.toml` and pushed to the driver,
  which persists its copy in the registry so it works at the login screen without the app.
- **APP-04 (P0)** "Show key mappings" window: a cheat sheet of every Fn combination.
- **APP-05 (P0)** Single instance; autostart via `HKCU\...\Run`.
- **APP-06 (P1)** Driver status (installed / test-signing / device connected, USB vs BT) shown in Settings.
- **APP-07 (P1)** Driverless fallback (low-level keyboard hook) when the driver is not installed:
  modifier swaps + F-keys-as-media only (Fn cannot be detected without the driver).

## 9. Driver

- **DRV-01 (P0)** KMDF filter written in Rust; installs below `kbdhid` on Apple keyboard collections; works for USB and BT.
- **DRV-02 (P0)** Media/volume/brightness/sleep/combos are emitted through a virtual HID device (VHF) so Windows
  handles them natively (works with no app running and at the login screen).
- **DRV-03 (P0)** Never exposes keystroke data to user mode. IOCTLs carry configuration and status only.
- **DRV-04 (P0)** Clean unload/uninstall; survives unplug with keys held, sleep/resume, BT reconnect; passes Driver Verifier.

## 10. Out of scope

Touch ID / biometrics, Magic Mouse & Magic Trackpad, custom keyboard layouts (link to existing Apple
ISO/ANSI layouts instead), backlight (not present), production (attestation/EV) signing.
