//! Engine input (decoded keyboard report) and output (rewritten report + virtual device state).

use crate::usage::kbd;

/// Maximum number of simultaneously pressed non-modifier keys tracked per report.
pub const MAX_KEYS: usize = 16;
/// Maximum simultaneously held consumer usages on the virtual consumer device.
pub const MAX_CONSUMER: usize = 4;

/// Fixed-capacity, de-duplicated list of keyboard usages.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct KeyList {
    keys: [u8; MAX_KEYS],
    len: u8,
    overflow: bool,
}

impl KeyList {
    pub const fn new() -> Self {
        KeyList { keys: [0; MAX_KEYS], len: 0, overflow: false }
    }

    /// Adds `usage` unless it is already present. Sets the overflow flag when full.
    pub fn push(&mut self, usage: u8) {
        if usage == 0 || self.contains(usage) {
            return;
        }
        if (self.len as usize) < MAX_KEYS {
            self.keys[self.len as usize] = usage;
            self.len += 1;
        } else {
            self.overflow = true;
        }
    }

    pub fn contains(&self, usage: u8) -> bool {
        self.as_slice().contains(&usage)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.keys[..self.len as usize]
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// True if more keys were pushed than fit.
    pub fn overflowed(&self) -> bool {
        self.overflow
    }

    pub fn clear(&mut self) {
        *self = KeyList::new();
    }
}

impl Default for KeyList {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for KeyList {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.as_slice().iter().map(|k| Hex(*k))).finish()
    }
}

struct Hex(u8);
impl core::fmt::Debug for Hex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#04x}", self.0)
    }
}

/// One decoded input report from the physical keyboard collection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyState {
    /// Keyboard-page modifier bits (bit n = usage 0xE0 + n).
    pub mods: u8,
    /// Pressed non-modifier keyboard usages.
    pub keys: KeyList,
    /// Apple vendor usage 0x00FF:0x0003.
    pub fn_key: bool,
    /// Consumer 0xB8.
    pub eject: bool,
    /// Consumer 0x40.
    pub menu: bool,
    /// Consumer 0x19E (Lock / Touch ID key).
    pub lock: bool,
}

impl KeyState {
    pub const fn new() -> Self {
        KeyState { mods: 0, keys: KeyList::new(), fn_key: false, eject: false, menu: false, lock: false }
    }

    /// Marks a keyboard-page usage as pressed (modifier usages set the matching bit).
    pub fn press(&mut self, usage: u8) -> &mut Self {
        if kbd::is_modifier(usage) {
            self.mods |= kbd::modifier_bit(usage);
        } else {
            self.keys.push(usage);
        }
        self
    }

    pub fn with_fn(mut self, down: bool) -> Self {
        self.fn_key = down;
        self
    }

    /// True when the keyboard reported phantom/rollover state (usage 0x01 in the key array).
    pub fn is_rollover(&self) -> bool {
        self.keys.contains(kbd::ERROR_ROLLOVER)
    }
}

/// A keyboard report (modifier byte + key list), used for both the real and the virtual keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KbdReport {
    pub mods: u8,
    pub keys: KeyList,
}

/// Engine notifications for the control channel / app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notification {
    FnLockChanged(crate::config::FnMode),
}

/// Everything the driver must emit after processing one input report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outputs {
    /// Rewritten report for the physical keyboard collection. All Apple-specific usages
    /// (Fn, Eject, Menu, Lock) are consumed; only keyboard-page usages remain.
    pub keyboard: KbdReport,
    /// State of the virtual keyboard (synthesized shortcuts such as Win+Tab).
    pub virt_keyboard: KbdReport,
    /// Held consumer usages on the virtual consumer device.
    pub consumer: [u16; MAX_CONSUMER],
    /// Held system-control usage on the virtual system-control device (0 = none).
    pub system: u8,
    /// Set when `virt_keyboard` differs from the previous call and must be submitted.
    pub virt_keyboard_changed: bool,
    /// Set when `consumer` differs from the previous call and must be submitted.
    pub consumer_changed: bool,
    /// Set when `system` differs from the previous call and must be submitted.
    pub system_changed: bool,
    /// A shortcut to tap once on the virtual keyboard (press then release), e.g. from a Fn double-tap.
    pub pulse: Option<KbdReport>,
    pub notification: Option<Notification>,
}

impl Outputs {
    pub fn consumer_usages(&self) -> impl Iterator<Item = u16> + '_ {
        self.consumer.iter().copied().filter(|u| *u != 0)
    }

    /// True if any virtual device report must be sent.
    pub fn any_virtual_change(&self) -> bool {
        self.virt_keyboard_changed || self.consumer_changed || self.system_changed || self.pulse.is_some()
    }
}
