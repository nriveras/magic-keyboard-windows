# Contributing

Thanks for helping. Bug reports, tests on keyboards we don't have and pull requests are all welcome.

## Reporting problems

- **Bugs**: open an issue with the *Bug report* template. Include the keyboard model, USB or
  Bluetooth, your Windows build (`winver`) and what the tray icon says about the driver.
- **A keyboard that isn't supported yet**: use the *New keyboard* template and attach the output of
  `mk-probe list` and `mk-probe caps` (see below).
- **Security problems**: don't open a public issue — see [SECURITY.md](SECURITY.md).

## Building

Set up the toolchain with [docs/DEV-SETUP.md](docs/DEV-SETUP.md) (Rust, Visual Studio Build Tools,
WDK, cargo-wdk). Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) first: almost all key handling
lives in `crates/mk-core`, which is plain Rust and fully testable on the host. The kernel driver in
`driver/mkfilter` is a thin wrapper around it.

`mk-probe` is a read-only diagnostics tool:

```powershell
cargo run -p mk-probe -- list
cargo run -p mk-probe -- caps
```

## Before opening a pull request

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
powershell -ExecutionPolicy Bypass -File scripts\tests\run-tests.ps1   # if you changed scripts or INFs
```

CI runs the first three. It can't build or load the driver, so for driver changes say in the pull
request which keyboards and transports (USB / Bluetooth) you tested on, and whether you ran Driver
Verifier.

- Keep pull requests focused on one change.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/):
  `fix(driver): …`, `feat(app): …`, `docs(readme): …`.
- New engine behaviour needs a unit test in `crates/mk-core`.

## Adding a keyboard model

Supported models are a table in `crates/mk-core/src/model.rs`. Add the USB and Bluetooth product IDs
there, add the hardware IDs to the INFs in `driver/mkfilter`, and update the table in the README.
Note any report-layout differences in [docs/HID-NOTES.md](docs/HID-NOTES.md).

## License

By contributing you agree that your contributions are licensed under the [MIT License](LICENSE).
