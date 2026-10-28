# This project vs. Brigadier + Apple Boot Camp drivers

*Written 2026-10-28. Facts about Brigadier come from its [README](https://github.com/timsutton/brigadier).
Claims about how Boot Camp behaves on a non-Mac PC with the A2520 come from user reports and have **not**
been verified on our hardware. Those rows are marked ⚠️.*

## What each one is

| | **Brigadier + Boot Camp** | **magic-keyboard-windows (this project)** |
|---|---|---|
| What it is | Brigadier is a Python script (also shipped as a prebuilt `.exe`) that **downloads and unpacks Apple's Boot Camp driver packages (ESDs)** from Apple's software-update catalog. It writes no driver code of its own; the keyboard support comes from Apple's Boot Camp drivers. | A purpose-built Rust kernel filter driver plus a tray/settings app, written for Apple keyboards on any Windows PC. |
| How you get it | `brigadier.exe -m <MacModel> [-i] [-o dir] [-k]`. It picks the package for a given **Mac model ID** (default: the current machine's) and can run the 64-bit MSI with `-i`. It uses 7-Zip and installs it temporarily if it's missing. | Build from source; install with dev scripts or `mk-install` (planned for M8). |
| Intended target | **Macs running Windows** (Boot Camp). Running it on a regular PC is unofficial: you have to pass a Mac model ID to `-m`. | Regular Windows 10/11 PCs, and Macs too. |
| License | MIT for Brigadier. The Boot Camp drivers are proprietary Apple software. | MIT for everything. |
| Source code | Brigadier's is open; Apple's drivers are closed binaries. | Fully open. |

## Feature comparison

Legend: ✅ supported · ❌ not supported · ⚠️ unverified or inconsistent · 🛠️ planned or in progress here

| Feature (from [SPEC](SPEC.md)) | Boot Camp (via Brigadier) | This project |
|---|---|---|
| Driver signing / Secure Boot | ✅ **Apple-signed**; works with Secure Boot on. No test mode. | ❌ Self-signed. Needs **Secure Boot off + test-signing mode**, which shows a watermark and trips some anti-cheat. |
| Works on non-Mac PCs | ⚠️ Unofficial. Apple only supports its drivers on Macs. | ✅ Designed for it. |
| A2520 (Touch ID + numpad, PID 029F) support | ❌ **Not in the KeyMagic 6.1.6700 INF** (see spike result below). The driver doesn't bind, which fits the user reports of volume keys not working and the keyboard tab missing. | 🛠️ Primary test device. HID layout already mapped ([HID-NOTES](HID-NOTES.md)). |
| Fn key detection | ✅ On supported models, inside Apple's driver. | 🛠️ Kernel filter reads usage `0x00FF:0x0003`. |
| Media keys (F7–F9) | ⚠️ Needs the Boot Camp control-panel/tray process; reported broken on A2520. | 🛠️ Virtual HID device (VHF): handled natively by Windows, no app process required. |
| Volume keys (F10–F12) | ⚠️ Reported not working on A2520. | 🛠️ Virtual consumer device; the keyboard itself has no volume usages. |
| Brightness, Task View, Search, Dictation, Sleep/DND (F1–F6) | ⚠️ Partial. Brightness only targets Mac displays. | 🛠️ Full spec table. |
| Fn lock (media-first vs F-keys-first) | ✅ "Use F1, F2… as standard function keys" checkbox, when the control panel shows the keyboard tab. | ✅ Implemented in the engine; tray toggle and Fn double-tap. |
| Missing PC keys (Del, Home/End, PgUp/PgDn, Insert, PrtSc, Pause, ScrLk) | ⚠️ Fn+⌫ = Delete and a few others. Coverage varies by version. | ✅ Implemented in the engine (the full Magic Utilities set). |
| Ctrl+Alt+Del from the Apple keyboard | ⚠️ | ✅ Fn+Ctrl+Opt+⌫ (engine). |
| F13–F19 (numpad models) | ⚠️ Passed through as F13–F19 at best. | ✅ F13–F15 → PrtSc/ScrLk/Pause; 🛠️ F16–F19 app launchers. |
| Modifier remapping (Cmd/Opt/Ctrl/Caps/Fn) | ❌ No remapping UI. You need registry Scancode Map or third-party tools. | ✅ Any modifier → any target, plus presets (engine). 🛠️ settings UI. |
| Right ⌘ → AltGr (third characters) | ❌ | ✅ |
| Clear key / Lock (Touch ID) key behaviour | ❌ / ⚠️ | ✅ Configurable. Lock defaults to Win+L. |
| Touch ID fingerprint (Windows Hello) | ❌ | ❌ Out of scope; no one has this on Windows. |
| Battery level / low-battery alerts | ⚠️ Battery shown on supported Macs/models. | 🛠️ Read from report 0x90 in user mode; tray display and toasts. |
| USB **and** Bluetooth | ⚠️ Mainly Bluetooth. One report of A2520 crashing Windows 11 over BT. | 🛠️ One filter path for both transports (M7 validates BT). |
| Works at the login screen | ✅ (driver level) | 🛠️ Yes. The driver stores its config in the registry, so the app isn't needed there. |
| Keyboard layouts (Apple ISO/ANSI) | ✅ Boot Camp ships Apple layouts. | ❌ Out of scope. Use existing layouts such as those in supermarsx/magickeyboard. |
| Magic Mouse / Trackpad | ✅ Some drivers included. | ❌ Out of scope. |

## Operations and risk

| | Boot Camp (via Brigadier) | This project |
|---|---|---|
| Install effort | Low. One command, but you have to pick a Mac model whose package contains the keyboard driver you need. | Medium. Toggle Secure Boot in firmware, `bcdedit`, then run the install script. |
| System impact | Installs the whole Boot Camp bundle (many Mac-only drivers, the control panel, a startup service) unless you hand-pick INFs from the unpacked folder (`-k`). | Two small components: one filter driver (keyboard collection only) and an optional tray app. |
| Stability risk | Mature, signed code, but running on hardware it was never tested on. There are crash reports with A2520 over BT. | New kernel code. A bug means a BSOD until fixed. Mitigated by Driver Verifier, restore points and Safe Mode uninstall. |
| Maintenance | Brigadier's author says their use case is shrinking. Some silent installs fail with Boot Camp 5/6. Apple only ships Boot Camp for Intel Macs, so no new keyboard models are coming. | Active (this repo); new models are a table entry in `mk-core::model`. |
| Fixability | None. Closed binaries. | Full: the engine is unit- and property-tested on the host. |

## Verdict

- **Pick Boot Camp (KeyMagic)** if you need Secure Boot or anti-cheat compatibility, only want the basics, **and own an older model** (0267, 026C or earlier). **It does not support the A2520** or any other 2021+ model: their PIDs are missing from the INF (see the spike result).
- **Pick this project** for the full Magic Utilities feature set (media and volume keys that work, remapping, F13–F19, battery alerts) on a non-Mac PC, if you're willing to run in test-signing mode.
- **Don't install both on the same keyboard.** Apple's keyboard filter and `mkfilter.sys` would both sit on the keyboard stack and double-translate Fn keys. Uninstall the Boot Camp keyboard driver before installing this project's driver.

## Spike result (M1, 2026-10-28, read-only)

Checked package: `magickeyboard2_AppleKeyboardInstaller64.exe` from supermarsx/magickeyboard. It is a RAR SFX and
was unpacked without being run. It contains Apple's **KeyMagic** driver: `Keymagic64.inf` / `Keymagic.sys`,
**DriverVer 10/07/2016, 6.1.6700.0**, KMDF 1.11, Apple-signed catalog.

- **Architecture: the same idea as this project.** KeyMagic is a KMDF `LowerFilters` driver on the keyboard
  stack. It binds at the USB interface (`USB\VID_05AC&PID_xxxx&MI_0x`) or the BT HID devnode
  (`BTHENUM\{00001124-…}_VID&0001004c_PID&xxxx`) and reads a per-model `KeyboardTable` from the registry.
- **Supported PIDs** (71 total, not contiguous): 0205–025B, 0262–0264, **0267** (Magic Keyboard 2015),
  **026C** (Magic Keyboard with numpad 2017), 0273, 0274, 0290, 0291 (internal MacBook keyboards).
- **Not listed:** 029A, 029C, **029F (our A2520)**, 0320–0322. This driver won't bind to the 2021 or 2024
  Magic Keyboards at all. That fits the user reports that media and volume keys don't work.
- Not checked: the very latest Boot Camp ESD (6.1.x for 2020 Intel Macs) through Brigadier. Doing so needs
  7-Zip and a download of several hundred MB. Since the repackaged "Magic Keyboard 2 and 3" driver above is
  the one people use on PCs, the verdict above doesn't depend on it.

**Conclusion:** for the A2520, Boot Camp is **not a working fallback**. The Secure-Boot-friendly fallback is
the driverless mode in M8 (low-level hook, no Fn).

## Sources

- [Brigadier README](https://github.com/timsutton/brigadier)
- [supermarsx/magickeyboard: repackaged Boot Camp keyboard drivers](https://github.com/supermarsx/magickeyboard)
- [Apple Discussions: Magic Keyboard on Boot Camp, volume keys / missing keyboard tab reports](https://discussions.apple.com/thread/8317183?page=2)
- [MacRumors: Magic Keyboard with Touch ID crashing Windows 11](https://forums.macrumors.com/threads/magic-keyboard-with-touch-id-is-crashing-windows-11.2446988)
- [MacRumors: best way to use the Magic Keyboard with Windows](https://forums.macrumors.com/threads/best-way-to-use-apple-magic-keyboard-with-windows-10-machine-best-generation-of-magic-keyboard-to-use.2445536)
- [Switching to Mac: using the Magic Keyboard on Windows (Brigadier usage)](https://www.switchingtomac.com/how-to-set-up-and-use-the-apple-magic-keyboard-on-windows/)
- [Magic Utilities features (feature reference)](https://magicutilities.net/magic-keyboard/features)
