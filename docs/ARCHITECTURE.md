# Architecture

```
 Apple keyboard (USB: hidusb | BT: hidbth) → hidclass → keyboard TLC PDO  HID\...&MI_01&Col01
                                                          ▲ IRP_MJ_READ completions (raw HID reports)
                              mkfilter.sys (Rust KMDF) ───┘ device LowerFilter, i.e. *below kbdhid*
                                │ HidP_GetUsages → KeyState → mk-core Engine → HidP_SetUsages
                                │ extra outputs: media / volume / brightness / sleep / key combos
                                ▼
                 ROOT\MagicKbdHub (same .sys, second role) ── VHF virtual HID device
                                ▲ device interface + IOCTLs (config / status only)
 mk-app.exe — tray icon, settings window, battery (hidapi), F16–F19 launchers, toasts, autostart
```

## Why a kernel filter

The Fn key is reported as usage `0x00FF:0x0003` *inside the keyboard collection*. Windows' `kbdhid`
opens keyboard collections exclusively and ignores vendor usages, so no user-mode API (Raw Input,
hidapi, hooks) can observe Fn. A filter between `kbdhid` and the hidclass collection PDO sees every
raw report regardless of transport (USB or Bluetooth), so a single code path covers both.

## Components

| Crate / dir | Kind | Responsibility |
|---|---|---|
| `crates/mk-core` | `no_std` lib | Models/PIDs, HID usage constants, `KeyState`, the remapping `Engine`, `DriverConfig` wire format, IOCTL codes. Pure and host-testable. Feature `std` adds serde/TOML user config. |
| `crates/mk-probe` | CLI | Diagnostics: list devices, dump capabilities, watch vendor reports, read battery, monitor what Windows receives. |
| `crates/mk-app` | GUI | Tray + settings window (egui), battery, F16–F19 launchers, notifications. |
| `crates/mk-install` | CLI | Dev installer: test certificate, signing, `pnputil`, root hub device creation. |
| `driver/mkfilter` | KMDF driver | Filter role (report rewrite) + hub role (VHF virtual device, control interface). |

## Engine contract

`Engine::process(&mut self, input: &KeyState, out: &mut Outputs)`

- `KeyState`: set of pressed usages from one input report (keyboard page, Fn, Eject, Menu, Lock …).
- `Outputs`: keyboard usages to put back in the report + a fixed-capacity list of virtual events
  (consumer / system-control / synthesized keyboard combo, each press or release) + notifications.
- A latch table remembers, per physical key, what it produced when pressed, so releases are always
  consistent even if modifiers change mid-press.

## Security

- The control interface carries configuration and status only. It never returns key events.
- Driver config is validated (version, size, enum ranges) before being applied.
