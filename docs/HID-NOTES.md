# HID notes

Findings from real hardware. Update this file whenever a probe reveals something new.

## Magic Keyboard with Touch ID and Numeric Keypad — USB `05AC:029F`, rev 0420

Captured 2026-10-28 with `HidP_GetCaps/GetButtonCaps/GetValueCaps` on Windows 11 26200.

| Devnode | TLC (page:usage) | Reports | Notes |
|---|---|---|---|
| `MI_00&Col01` | FF00:000B | in `E0` (4 B), in `9A` (1 B) | vendor, unknown |
| `MI_00&Col02` | FF00:0014 | in `90`: page 0x85 usages 0x44 Charging, 0x46 FullyCharged, 0x61 ?, value 0x65 AbsoluteStateOfCharge (8 bit) | **battery** — user-mode readable |
| `MI_01&Col01` | 0001:0006 Keyboard | in `01` (10 B): kbd page mods+array, consumer 0xB8 Eject, **0x00FF:0x0003 Fn**, consumer 0x40 Menu, consumer 0x19E AL Terminal Lock; out `01` LEDs | **opened exclusively by kbdhid → kernel filter needed for Fn** |
| `MI_01&Col02` | 000C:0001 Consumer | in `52`: 0xCD Play/Pause, 0xB3 FF, 0xB4 RW, 0xB5 Next, 0xB6 Prev; feature `09` FF01:000B (8 bit) | no volume usages; feature 09 unknown |
| `MI_01&Col03` | FF00:0006 | in `3F` (64 B) | vendor, unknown |
| `MI_02` | FF00:004B | in `20`/`21`/`22` (large) | likely Touch ID / secure channel — ignore |

Hardware ID of the keyboard collection: `HID\VID_05AC&PID_029F&REV_0420&MI_01&Col01`.

### mk-probe results (2026-10-28, USB)

Reproduce with `mk-probe caps`, `mk-probe battery`, `mk-probe feature MI_01&Col02 0x09`.

**Keyboard report `01` (`MI_01&Col01`)**
- Modifiers: page 07 `E0..E7`, data indexes 0–7 (one bit each).
- Key array: page 07, usage range **`0x00..0xFF`** (data indexes 8–263). The array can carry every
  keyboard-page usage, including F13–F24 (`0x68–0x73`), Delete/Insert/Home/End/PgUp/PgDn and
  PrtSc/ScrLk/Pause. The driver can therefore rewrite all in-report outputs without needing the VHF keyboard.
- `HidP_MaxUsageListLength(page 07) = 14`, which is 8 modifiers plus **6 array slots (6KRO)**. The engine's
  key list holds 16 keys. In M4 the driver must turn `HidP_SetUsages` → `HIDP_STATUS_BUFFER_TOO_SMALL` into
  an ErrorRollOver report and never truncate silently.
- Single bits: consumer `0xB8` Eject (idx 264), `FF:0003` **Fn** (idx 265), consumer `0x40` Menu (266),
  consumer `0x19E` Lock (267).
- Output report `01`: LED page 08 usages 1–5.
- Exact byte offsets are unknown, because user mode can't read the report: kbdhid holds it exclusively
  (`watch` → Access denied). They don't matter, because the driver decodes and encodes with HidP. M3 will
  log raw reports anyway. The expected layout is: id, mods, reserved, 6 keys, a bits byte (10 B).

**Battery report `90` (`MI_00&Col02`)**
- Bit order: `0x46` FullyCharged (idx 0), `0x44` Charging (idx 1), `0x61` (idx 2, unknown; maybe
  external power).
- Value `0x65` AbsoluteStateOfCharge: 8 bit, logical 0..255. The decoder clamps it to 100.
- `HidD_GetInputReport(0x90)` works from user mode while the keyboard is in use. While charging on USB it
  reads **100%, FullyCharged**.

**Consumer collection `MI_01&Col02`**
- Input `52`: 5 single bits (CD, B3, B4, B5, B6). It can be opened read/write from user mode, so `watch` works.
- Feature `09` (FF01:000B, 8 bit, logical **0..1**): current value **`01`**. It's a vendor flag of unknown
  meaning. A boolean on the media collection hints at a firmware Fn/media mode, but **it is never written** by
  this project unless a later, deliberate spike documents it.

**Vendor collections**: `MI_02` has inputs 20/21/22 (107/649/62 bytes; Touch ID channel). `MI_00&Col01` has E0 (4 B) and 9A (1 B).
`MI_01&Col03` has 3F (64 B). All are ignored.

### `input-monitor` baseline (stock Windows drivers, USB, 2026-10-28)

- F1–F12 arrive as plain `VK_F1..VK_F12` (scan codes 3B–44, 57, 58) from `MI_01&Col01`. No consumer
  events at all, so media and volume keys are dead today. The consumer collection `MI_01&Col02` stayed
  silent even though it declares Play/Pause/FF/RW/Next/Prev. The firmware evidently sends the top row as
  F-keys, which possibly relates to feature `09` = 1 (unconfirmed, still never written).
- Fn produces no event. kbdhid drops vendor usage `FF:0003`.
- A second run (Fn presumably held) showed key-downs for `1`, F1–F12 and Ctrl **with no key-ups**. To be
  confirmed: is this kbdhid mishandling releases while the Fn bit is set, or did the capture stop early?
- Lock key (`0x19E`): no event captured yet.

### Open questions
- [ ] Byte offsets of report 01 (log it from the driver in M3).
- [ ] Battery values while discharging over Bluetooth; meaning of usage 0x61.
- [ ] Bluetooth hardware IDs and descriptor for the same keyboard (M7).
- [ ] `input-monitor` follow-up: Lock key, Fn+⌫, key-ups while Fn is held
