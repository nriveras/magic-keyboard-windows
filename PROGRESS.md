# Progress

Milestones from the project plan. Each completed item is committed to git.

## M0 — Repo & toolchain
- [x] `git init`, `.gitignore`
- [x] docs: SPEC, ARCHITECTURE, HID-NOTES, DEV-SETUP
- [ ] Rust toolchain (rustup, stable MSVC)
- [ ] VS 2022 Build Tools + Windows SDK, WDK 10.0.26100, LLVM
- [ ] cargo workspace skeleton

## M1 — mk-probe & spikes
- [ ] `list`, `caps`, `watch`, `battery`, `feature`, `input-monitor`
- [ ] Keyboard report logical range / layout recorded in HID-NOTES
- [ ] Battery report decoded
- [ ] Boot Camp driver evaluation written up

## M2 — mk-core engine
- [ ] Usage constants, KeyState, config model
- [ ] Fn layer + Fn lock + latch
- [ ] Missing keys, F13–F19, modifiers, Clear/Lock/Eject
- [ ] Unit + property tests

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
