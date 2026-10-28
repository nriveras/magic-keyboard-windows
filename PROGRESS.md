# Progress

Milestones from the project plan. Each completed item is committed to git.

## M0 — Repo & toolchain
- [x] `git init`, `.gitignore`
- [x] docs: SPEC, ARCHITECTURE, HID-NOTES, DEV-SETUP
- [x] Rust toolchain (rustup, stable MSVC)
- [x] VS 2022 Build Tools + Windows SDK, WDK 10.0.26100, LLVM
- [x] cargo workspace skeleton

## M1 — mk-probe & spikes
- [x] `list`, `caps`, `watch`, `battery`, `feature`, `input-monitor`
- [x] Keyboard report logical range / layout recorded in HID-NOTES (0x00–0xFF array, 6KRO)
- [x] Battery report decoded
- [x] `input-monitor` baseline captured (F-row = plain F-keys, Lock dropped, Fn-held loses key-ups)
- [x] Boot Camp driver evaluation written up (KeyMagic 6.1.6700 has no 029F → not a fallback)

## M2 — mk-core engine
- [x] Usage constants, KeyState, config model (64-byte wire format)
- [x] Fn layer + Fn lock + latch + Fn double/triple tap
- [x] Missing keys, F13–F19, modifiers, Clear/Lock/Eject
- [x] Unit (51) + property tests (no stuck keys, stable idle)

## M3 — Driver pass-through
- [x] Rust KMDF filter builds and signs (test cert), pass-through + debug report log
- [x] Dev scripts: build-driver, install-dev (forced install), uninstall-dev(.cmd)
- [x] INF installs as LowerFilter on keyboard TLC (USB, keyboard keeps working)
- [x] Fn transitions visible in DebugView (byte 9 bit 1; report layout in HID-NOTES)
- [x] Lock key location (report 01, byte 9 bit 2)

## M4 — Driver remapping
- [x] mk-core wired into read completion (HidP decode → engine → report rebuilt with page 07 only, spinlock)
- [x] Registry config load (`Services\mkfilter\Parameters\Config`, REG_BINARY wire format; model defaults otherwise)
- [x] ErrorRollOver instead of truncation when more keys than array slots
- [x] Hardware test (USB): Fn+⌫/arrows/Return/Esc/P, F13–F15, F1–F12, modifiers; no lost key-ups with Fn
- [x] Lock arrives as consumer `0x40` → per-model override (`Model::lock_reports_menu`)
- [x] Re-test: Lock no longer produces Apps/Menu; emits Win+L press/release (dropped until M5)
- (M4 only: the top row was forced to F-keys first; removed in M5.)

## M5 — Hub + VHF
- [x] Same image, two roles (picked by hardware ID); `mkhub.inf` (System class, vhf lower filter) packaged by build-driver
- [x] Root hub device + VHF descriptor (keyboard ID 1, consumer ID 2, system control ID 3)
- [x] Filter submits virtual outputs under the device lock; engine timer (Fn double-tap); release-all on D0 exit
- [x] install-dev creates `ROOT\MagicKbdHub`; uninstall-dev removes it
- [x] Hardware test (USB): F8/F9 media, F10–F12 mute/volume, F3/F4/F5 Task View/Search/Dictation, Lock → Win+L, Fn+F-key = plain F-key
- [x] Hardware test: F7, Shift+F7–F9, F6 / Fn×3+Space sleep
- [x] Control interface on the hub (GET/SET_CONFIG, GET/WAIT_STATUS; SDDL: admins full, interactive users RW)
- [x] Driver-wide shared state (config generation, keyboard count); config persisted to `Parameters\Config`
- [x] Fn-lock double-tap published to status (not persisted)
- [x] `mk-hid::control` client + `mk-probe driver status|config|fnlock|watch`
- [x] Hardware test: control interface (status, fnlock switch live, watch, persistence across replug; non-admin)

## M6 — mk-app
- [x] Tray (Win32 Shell_NotifyIcon): battery icon + tooltip, low-battery balloon (≤20%), top-row mode toggle via control interface, autostart (HKCU Run), single instance, F16–F19 launchers
- [x] Hardware test: tray app
- [x] Second model: 029A (Magic Keyboard with Touch ID) added to mkfilter.inx; install-dev installs on every present supported keyboard
- [ ] Hardware test: 029A remapping, media keys, Lock
- [ ] Settings window, config push, F16–F19 launchers, toasts, autostart, cheat sheet

## M7 — Bluetooth
- [ ] BT hardware IDs in INF, battery over BT, full test matrix

## M8 — Packaging & docs
- [ ] mk-install, README install/uninstall, driverless fallback
