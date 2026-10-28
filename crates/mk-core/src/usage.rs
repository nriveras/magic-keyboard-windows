//! HID usage constants used by Apple keyboards and by the engine's outputs.

/// HID usage pages.
pub mod page {
    pub const GENERIC_DESKTOP: u16 = 0x01;
    pub const KEYBOARD: u16 = 0x07;
    pub const CONSUMER: u16 = 0x0C;
    /// Apple "top case" vendor page; usage 0x03 is the Fn key.
    pub const APPLE_TOP_CASE: u16 = 0x00FF;
    pub const APPLE_FN_USAGE: u16 = 0x0003;
}

/// Keyboard/Keypad page (0x07) usages.
pub mod kbd {
    pub const ERROR_ROLLOVER: u8 = 0x01;
    pub const A: u8 = 0x04;
    pub const H: u8 = 0x0B;
    pub const L: u8 = 0x0F;
    pub const N: u8 = 0x11;
    pub const P: u8 = 0x13;
    pub const S: u8 = 0x16;
    pub const ENTER: u8 = 0x28;
    pub const ESCAPE: u8 = 0x29;
    pub const BACKSPACE: u8 = 0x2A;
    pub const TAB: u8 = 0x2B;
    pub const SPACE: u8 = 0x2C;
    pub const CAPS_LOCK: u8 = 0x39;
    pub const F1: u8 = 0x3A;
    pub const F11: u8 = 0x44;
    pub const F12: u8 = 0x45;
    pub const PRINT_SCREEN: u8 = 0x46;
    pub const SCROLL_LOCK: u8 = 0x47;
    pub const PAUSE: u8 = 0x48;
    pub const INSERT: u8 = 0x49;
    pub const HOME: u8 = 0x4A;
    pub const PAGE_UP: u8 = 0x4B;
    pub const DELETE: u8 = 0x4C;
    pub const END: u8 = 0x4D;
    pub const PAGE_DOWN: u8 = 0x4E;
    pub const RIGHT: u8 = 0x4F;
    pub const LEFT: u8 = 0x50;
    pub const DOWN: u8 = 0x51;
    pub const UP: u8 = 0x52;
    /// "Keypad Num Lock and Clear" — the Clear key on Apple numeric keypads.
    pub const NUM_LOCK_CLEAR: u8 = 0x53;
    pub const APPLICATION: u8 = 0x65;
    pub const F13: u8 = 0x68;
    pub const F16: u8 = 0x6B;
    pub const F17: u8 = 0x6C;
    pub const F18: u8 = 0x6D;
    pub const F19: u8 = 0x6E;

    pub const LEFT_CTRL: u8 = 0xE0;
    pub const LEFT_SHIFT: u8 = 0xE1;
    pub const LEFT_ALT: u8 = 0xE2;
    pub const LEFT_GUI: u8 = 0xE3;
    pub const RIGHT_CTRL: u8 = 0xE4;
    pub const RIGHT_SHIFT: u8 = 0xE5;
    pub const RIGHT_ALT: u8 = 0xE6;
    pub const RIGHT_GUI: u8 = 0xE7;

    /// Fn row index (0 = F1 … 11 = F12) if `usage` is F1–F12.
    pub const fn f_row_index(usage: u8) -> Option<u8> {
        if usage >= F1 && usage <= F12 {
            Some(usage - F1)
        } else {
            None
        }
    }

    /// Extended F-key index (0 = F13 … 6 = F19) if `usage` is F13–F19.
    pub const fn f_ext_index(usage: u8) -> Option<u8> {
        if usage >= F13 && usage <= F19 {
            Some(usage - F13)
        } else {
            None
        }
    }

    pub const fn is_modifier(usage: u8) -> bool {
        usage >= LEFT_CTRL && usage <= RIGHT_GUI
    }

    /// Bit in the modifier byte for a modifier usage (0xE0..=0xE7).
    pub const fn modifier_bit(usage: u8) -> u8 {
        1 << (usage - LEFT_CTRL)
    }
}

/// Modifier byte bit masks.
pub mod modbits {
    pub const LCTRL: u8 = 0x01;
    pub const LSHIFT: u8 = 0x02;
    pub const LALT: u8 = 0x04;
    pub const LGUI: u8 = 0x08;
    pub const RCTRL: u8 = 0x10;
    pub const RSHIFT: u8 = 0x20;
    pub const RALT: u8 = 0x40;
    pub const RGUI: u8 = 0x80;
    pub const ANY_SHIFT: u8 = LSHIFT | RSHIFT;
}

/// Consumer page (0x0C) usages.
pub mod consumer {
    pub const MENU: u16 = 0x0040;
    pub const BRIGHTNESS_UP: u16 = 0x006F;
    pub const BRIGHTNESS_DOWN: u16 = 0x0070;
    pub const FAST_FORWARD: u16 = 0x00B3;
    pub const REWIND: u16 = 0x00B4;
    pub const NEXT_TRACK: u16 = 0x00B5;
    pub const PREV_TRACK: u16 = 0x00B6;
    pub const STOP: u16 = 0x00B7;
    pub const EJECT: u16 = 0x00B8;
    pub const PLAY_PAUSE: u16 = 0x00CD;
    pub const MUTE: u16 = 0x00E2;
    pub const VOLUME_UP: u16 = 0x00E9;
    pub const VOLUME_DOWN: u16 = 0x00EA;
    pub const AL_TERMINAL_LOCK: u16 = 0x019E;
}

/// Generic Desktop system-control usages.
pub mod system {
    pub const POWER_DOWN: u8 = 0x81;
    pub const SLEEP: u8 = 0x82;
    pub const WAKE_UP: u8 = 0x83;
}
