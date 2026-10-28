//! Remapping configuration and its fixed-size binary wire format (app → driver).

use crate::usage::kbd;

macro_rules! wire_enum {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $var:ident = $val:expr),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "std", serde(rename_all = "kebab-case"))]
        #[repr(u8)]
        pub enum $name { $($(#[$vm])* $var = $val),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];

            pub fn from_u8(v: u8) -> Option<Self> {
                match v { $($val => Some($name::$var),)+ _ => None }
            }
        }
    };
}

wire_enum! {
    /// What the top row does without Fn held.
    FnMode {
        /// F1–F12 are brightness/media/volume; Fn + key gives the F-key (macOS default).
        MediaFirst = 0,
        /// F1–F12 are F-keys; Fn + key gives the special function.
        FKeysFirst = 1,
    }
}

wire_enum! {
    /// Action for the F6 special function.
    F6Action { Sleep = 0, DoNotDisturb = 1, None = 2 }
}

wire_enum! {
    /// Numeric keypad Clear key.
    ClearAction { NumLock = 0, Delete = 1, Backspace = 2, Disabled = 3 }
}

wire_enum! {
    /// Lock / Touch ID key (consumer AL Terminal Lock).
    LockAction { LockWorkstation = 0, Insert = 1, Disabled = 2 }
}

wire_enum! {
    /// Eject key on older models.
    EjectAction {
        /// Eject = Delete, Fn + Eject = Insert.
        Delete = 0,
        /// Eject acts as an extra Fn key.
        Fn = 1,
        Disabled = 2,
    }
}

wire_enum! {
    /// Action when Fn is tapped twice quickly.
    FnDoubleTap { Off = 0, ToggleFnLock = 1, Dictation = 2 }
}

wire_enum! {
    /// What a physical modifier key produces.
    ModTarget {
        Ctrl = 0,
        Shift = 1,
        Alt = 2,
        Win = 3,
        /// Right Alt (AltGr) regardless of the physical side.
        AltGr = 4,
        CapsLock = 5,
        Fn = 6,
        Menu = 7,
        Insert = 8,
        Disabled = 9,
    }
}

/// Physical modifier keys that can be remapped, in wire order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ModSource {
    LeftControl = 0,
    LeftShift = 1,
    LeftOption = 2,
    LeftCommand = 3,
    RightControl = 4,
    RightShift = 5,
    RightOption = 6,
    RightCommand = 7,
    CapsLock = 8,
    Fn = 9,
}

impl ModSource {
    pub const COUNT: usize = 10;
    pub const ALL: [ModSource; Self::COUNT] = [
        ModSource::LeftControl,
        ModSource::LeftShift,
        ModSource::LeftOption,
        ModSource::LeftCommand,
        ModSource::RightControl,
        ModSource::RightShift,
        ModSource::RightOption,
        ModSource::RightCommand,
        ModSource::CapsLock,
        ModSource::Fn,
    ];

    pub fn is_right(self) -> bool {
        matches!(
            self,
            ModSource::RightControl | ModSource::RightShift | ModSource::RightOption | ModSource::RightCommand
        )
    }

    /// Identity mapping: what the key produces with no remapping.
    pub fn native_target(self) -> ModTarget {
        match self {
            ModSource::LeftControl | ModSource::RightControl => ModTarget::Ctrl,
            ModSource::LeftShift | ModSource::RightShift => ModTarget::Shift,
            ModSource::LeftOption | ModSource::RightOption => ModTarget::Alt,
            ModSource::LeftCommand | ModSource::RightCommand => ModTarget::Win,
            ModSource::CapsLock => ModTarget::CapsLock,
            ModSource::Fn => ModTarget::Fn,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ModSource::LeftControl => "Left Control",
            ModSource::LeftShift => "Left Shift",
            ModSource::LeftOption => "Left Option",
            ModSource::LeftCommand => "Left Command",
            ModSource::RightControl => "Right Control",
            ModSource::RightShift => "Right Shift",
            ModSource::RightOption => "Right Option",
            ModSource::RightCommand => "Right Command",
            ModSource::CapsLock => "Caps Lock",
            ModSource::Fn => "Fn",
        }
    }
}

/// Modifier presets offered by the settings UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModPreset {
    /// Command → Win, Option → Alt (Apple key positions, no remapping).
    WindowsPositions,
    /// Command → Alt, Option → Win, matching a PC keyboard's Ctrl-Win-Alt physical order.
    PcOrder,
    /// Command → Ctrl, Control → Win, so ⌘C / ⌘V copy and paste.
    MacLikeShortcuts,
}

/// Number of configurable extended F-keys (F13–F19).
pub const F_EXT_COUNT: usize = 7;

/// Complete remapping configuration understood by the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub fn_mode: FnMode,
    pub f6: F6Action,
    pub clear: ClearAction,
    pub lock: LockAction,
    pub eject: EjectAction,
    pub fn_double_tap: FnDoubleTap,
    /// Fn, Fn, Fn, Space puts the computer to sleep.
    pub sleep_combo: bool,
    /// Indexed by [`ModSource`].
    pub modifiers: [ModTarget; ModSource::COUNT],
    /// Keyboard usage emitted by F13…F19; 0 disables the key. Using the key's own usage passes it through.
    pub f_ext: [u8; F_EXT_COUNT],
}

impl Default for Config {
    fn default() -> Self {
        let mut modifiers = [ModTarget::Disabled; ModSource::COUNT];
        for src in ModSource::ALL {
            modifiers[src as usize] = src.native_target();
        }
        Config {
            fn_mode: FnMode::MediaFirst,
            f6: F6Action::Sleep,
            clear: ClearAction::NumLock,
            lock: LockAction::LockWorkstation,
            eject: EjectAction::Delete,
            fn_double_tap: FnDoubleTap::Off,
            sleep_combo: true,
            modifiers,
            f_ext: [
                kbd::PRINT_SCREEN,
                kbd::SCROLL_LOCK,
                kbd::PAUSE,
                kbd::F16,
                kbd::F17,
                kbd::F18,
                kbd::F19,
            ],
        }
    }
}

impl Config {
    pub fn modifier(&self, src: ModSource) -> ModTarget {
        self.modifiers[src as usize]
    }

    pub fn set_modifier(&mut self, src: ModSource, target: ModTarget) {
        self.modifiers[src as usize] = target;
    }

    /// Applies a preset to the Option/Command/Control keys; Shift, Caps Lock and Fn are untouched.
    pub fn apply_preset(&mut self, preset: ModPreset) {
        use ModSource::*;
        let (ctrl, opt, cmd) = match preset {
            ModPreset::WindowsPositions => (ModTarget::Ctrl, ModTarget::Alt, ModTarget::Win),
            ModPreset::PcOrder => (ModTarget::Ctrl, ModTarget::Win, ModTarget::Alt),
            ModPreset::MacLikeShortcuts => (ModTarget::Win, ModTarget::Alt, ModTarget::Ctrl),
        };
        for (src, target) in [
            (LeftControl, ctrl),
            (RightControl, ctrl),
            (LeftOption, opt),
            (RightOption, opt),
            (LeftCommand, cmd),
            (RightCommand, cmd),
        ] {
            self.set_modifier(src, target);
        }
    }

    /// Default configuration adjusted for a keyboard model (Eject behaves as Fn on numpad models).
    pub fn default_for(model: &crate::model::Model) -> Self {
        let mut cfg = Config::default();
        if model.has_numpad && model.has_eject {
            cfg.eject = EjectAction::Fn;
        }
        cfg
    }
}

// ---------------------------------------------------------------------------------------------
// Wire format
// ---------------------------------------------------------------------------------------------

/// "MKC1" little endian.
pub const WIRE_MAGIC: u32 = u32::from_le_bytes(*b"MKC1");
pub const WIRE_VERSION: u16 = 1;
pub const WIRE_SIZE: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    TooShort,
    BadMagic,
    UnsupportedVersion(u16),
    BadSize(u16),
    /// Byte offset of the invalid field.
    InvalidField(usize),
}

impl Config {
    /// Encodes the configuration into the fixed 64-byte wire format.
    pub fn to_wire(&self) -> [u8; WIRE_SIZE] {
        let mut b = [0u8; WIRE_SIZE];
        b[0..4].copy_from_slice(&WIRE_MAGIC.to_le_bytes());
        b[4..6].copy_from_slice(&WIRE_VERSION.to_le_bytes());
        b[6..8].copy_from_slice(&(WIRE_SIZE as u16).to_le_bytes());
        b[8] = self.fn_mode as u8;
        b[9] = self.f6 as u8;
        b[10] = self.clear as u8;
        b[11] = self.lock as u8;
        b[12] = self.eject as u8;
        b[13] = self.fn_double_tap as u8;
        b[14] = self.sleep_combo as u8;
        for (i, m) in self.modifiers.iter().enumerate() {
            b[16 + i] = *m as u8;
        }
        b[26..26 + F_EXT_COUNT].copy_from_slice(&self.f_ext);
        b
    }

    /// Decodes and validates a wire buffer. Never panics on malformed input.
    pub fn from_wire(b: &[u8]) -> Result<Self, WireError> {
        if b.len() < WIRE_SIZE {
            return Err(WireError::TooShort);
        }
        if u32::from_le_bytes([b[0], b[1], b[2], b[3]]) != WIRE_MAGIC {
            return Err(WireError::BadMagic);
        }
        let version = u16::from_le_bytes([b[4], b[5]]);
        if version != WIRE_VERSION {
            return Err(WireError::UnsupportedVersion(version));
        }
        let size = u16::from_le_bytes([b[6], b[7]]);
        if size as usize != WIRE_SIZE {
            return Err(WireError::BadSize(size));
        }
        fn field<T>(b: &[u8], at: usize, f: fn(u8) -> Option<T>) -> Result<T, WireError> {
            f(b[at]).ok_or(WireError::InvalidField(at))
        }
        let mut modifiers = [ModTarget::Disabled; ModSource::COUNT];
        for (i, m) in modifiers.iter_mut().enumerate() {
            *m = field(b, 16 + i, ModTarget::from_u8)?;
        }
        let mut f_ext = [0u8; F_EXT_COUNT];
        f_ext.copy_from_slice(&b[26..26 + F_EXT_COUNT]);
        Ok(Config {
            fn_mode: field(b, 8, FnMode::from_u8)?,
            f6: field(b, 9, F6Action::from_u8)?,
            clear: field(b, 10, ClearAction::from_u8)?,
            lock: field(b, 11, LockAction::from_u8)?,
            eject: field(b, 12, EjectAction::from_u8)?,
            fn_double_tap: field(b, 13, FnDoubleTap::from_u8)?,
            sleep_combo: match b[14] {
                0 => false,
                1 => true,
                _ => return Err(WireError::InvalidField(14)),
            },
            modifiers,
            f_ext,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_roundtrip() {
        let mut cfg = Config::default();
        cfg.fn_mode = FnMode::FKeysFirst;
        cfg.apply_preset(ModPreset::MacLikeShortcuts);
        cfg.f_ext[3] = 0;
        assert_eq!(Config::from_wire(&cfg.to_wire()), Ok(cfg));
    }

    #[test]
    fn wire_rejects_garbage() {
        let good = Config::default().to_wire();
        assert_eq!(Config::from_wire(&good[..10]), Err(WireError::TooShort));
        let mut b = good;
        b[0] = 0;
        assert_eq!(Config::from_wire(&b), Err(WireError::BadMagic));
        let mut b = good;
        b[4] = 9;
        assert_eq!(Config::from_wire(&b), Err(WireError::UnsupportedVersion(9)));
        let mut b = good;
        b[8] = 7;
        assert_eq!(Config::from_wire(&b), Err(WireError::InvalidField(8)));
        let mut b = good;
        b[16 + 3] = 200;
        assert_eq!(Config::from_wire(&b), Err(WireError::InvalidField(19)));
        let mut b = good;
        b[14] = 2;
        assert_eq!(Config::from_wire(&b), Err(WireError::InvalidField(14)));
    }

    #[test]
    fn presets() {
        let mut cfg = Config::default();
        cfg.apply_preset(ModPreset::PcOrder);
        assert_eq!(cfg.modifier(ModSource::LeftCommand), ModTarget::Alt);
        assert_eq!(cfg.modifier(ModSource::RightOption), ModTarget::Win);
        cfg.apply_preset(ModPreset::WindowsPositions);
        assert_eq!(cfg, Config::default());
    }
}
