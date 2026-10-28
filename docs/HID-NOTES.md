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
- User mode can't read the report: kbdhid holds it exclusively (`watch` → Access denied). The M3 driver
  log confirmed the layout below.

**Report `01` raw layout (confirmed by the M3 driver log, USB, 2026-10-28)**

| Byte | Content |
|---|---|
| 0 | report ID `01` |
| 1 | modifier bits `E0..E7` (e.g. `08` = Left GUI / Left ⌘) |
| 2 | reserved `00` |
| 3–8 | 6-slot key array (page 07 usages) |
| 9 | bit 0 Eject `0xB8`, **bit 1 Fn `FF:0003` (`02`)**, bit 2 Menu `0x40`, bit 3 Lock `0x19E` (assumed from descriptor index order) |

Captured sequences:
- Fn alone: `…00 02` → `…00 00`.
- Fn+F1: `02`, then `3A` in byte 3 with `02` still set, then the key array clears while Fn stays `02`, then all zero.
- Fn+⌫: same as Fn+F1 with `2A`.
- Left ⌘+A: byte 1 = `08`, then byte 3 = `04`.
- **The firmware sends correct key releases while Fn is held** (the array clears and Fn stays set). The lost
  key-ups under stock drivers therefore come from kbdhid's handling of the vendor bit, not from the keyboard.
- **Lock produced no report on this collection.** Either it wasn't pressed in this capture, or the A2520 sends
  Lock elsewhere (candidates: `MI_02` Touch ID channel, `MI_00` vendor collections). To be rechecked.

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
- **Fn held → key-ups are lost.** With Fn held, ⌫, F1, F2 and `1` each produced only a key-down (`BACK`, `F1`,
  `F2`, `1`), with no key-up and no typematic repeat. Ctrl+C right afterwards had normal down/up pairs. Two runs
  reproduced this. It suggests kbdhid mis-translates releases when the vendor Fn bit (`FF:0003`) is set in the report.
- **Lock key (`0x19E`): no event at all.** kbdhid drops it, so the driver must emit Win+L through VHF.
- **Driver requirement (M4):** rebuild every report passed to kbdhid from scratch with `HidP_SetUsages`, using
  only keyboard-page usages. Never forward `FF:0003` Fn, consumer `0x40` Menu, `0x19E` Lock or `0xB8` Eject.

### Open questions
- [x] Byte offsets of report 01 (logged by the M3 driver; see above).
- [ ] Where the Lock / Touch ID key is reported on the A2520 (not seen in report 01).
- [ ] Battery values while discharging over Bluetooth; meaning of usage 0x61.
- [ ] Bluetooth hardware IDs and descriptor for the same keyboard (M7).
