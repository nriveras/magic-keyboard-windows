# Security policy

This project installs a kernel-mode driver that sees every key typed on supported keyboards. A bug in
it can crash Windows or expose keystrokes, so please report security problems privately.

## Reporting a vulnerability

Use GitHub's private reporting: **Security → Report a vulnerability** on this repository. Please
include the affected version or commit, the steps to reproduce and the impact you see.

Don't open a public issue for security problems. You'll get an answer within a week; a fix is
published once it's ready, crediting you unless you'd rather not be named.

## Supported versions

Only the latest commit on `main` gets fixes.

## Scope

In scope: the driver (`driver/mkfilter`), the tray app (`crates/mk-app`), the install and uninstall
scripts, and anything that could leak keystrokes (see Privacy in the README).

Out of scope: the need to turn off Secure Boot and turn on test-signing mode. That is a known,
documented trade-off of using a self-signed driver.
