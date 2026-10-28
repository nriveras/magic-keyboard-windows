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
- [ ] `input-monitor` baseline captured (needs interactive key presses)
- [ ] Boot Camp driver evaluation written up

## M2 — mk-core engine
- [x] Usage constants, KeyState, config model (64-byte wire format)
- [x] Fn layer + Fn lock + latch + Fn double/triple tap
- [x] Missing keys, F13–F19, modifiers, Clear/Lock/Eject
- [x] Unit (51) + property tests (no stuck keys, stable idle)

## M3 — Driver pass-through
- [ ] Rust KMDF filter builds and signs (test cert)
- [ ] INF installs as LowerFilter on keyboard TLC
- [ ] Fn transitions visible in DebugView

## M4 — Driver remapping
- [ ] mk-core wired into read completion
- [ ] Registry config load

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
