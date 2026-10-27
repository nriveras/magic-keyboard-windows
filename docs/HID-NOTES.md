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

### Open questions (M1)
- [ ] Logical min/max of the keyboard-page array in report 01 (can it carry F13–F24 / 0x68–0x73?).
- [ ] Byte layout of report 01 (expected: id, mods, reserved, 6 keys, vendor/consumer bits).
- [ ] Battery report 0x90 values over time; usage 0x61 meaning.
- [ ] Feature report 0x09 current value (read only).
- [ ] Bluetooth hardware IDs and descriptor for the same keyboard.
