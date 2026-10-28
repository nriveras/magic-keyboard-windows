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
- Until M5 the top row is forced to F-keys first; media/shortcut/Lock outputs are logged and dropped.

## M5 — Hub + VHF
- [ ] Root hub device + VHF descriptor
- [ ] Media / volume / brightness / sleep / combos
- [ ] Control interface IOCTLs

## M6 — mk-app
- [ ] Tray + battery + Fn lock toggle
- [ ] Settings window, config push, F16–F19 launchers, toasts, autostart, cheat sheet

## M7 — Bluetooth
- [ ] BT hardware IDs in INF, battery over BT, full test matrix

## M8 — Packaging & docs
- [ ] mk-install, README install/uninstall, driverless fallback
