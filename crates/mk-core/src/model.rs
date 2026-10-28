//! Known Apple keyboard models.

/// USB vendor ID used by Apple when wired.
pub const APPLE_USB_VID: u16 = 0x05AC;
/// Bluetooth vendor ID (Apple's Bluetooth SIG company ID) reported when paired.
pub const APPLE_BT_VID: u16 = 0x004C;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub pid: u16,
    pub name: &'static str,
    pub has_numpad: bool,
    pub has_touch_id: bool,
    /// Older models have an Eject key (consumer 0xB8) instead of a Lock/Touch ID key.
    pub has_eject: bool,
}

pub const MODELS: &[Model] = &[
    Model { pid: 0x0267, name: "Magic Keyboard (2015)", has_numpad: false, has_touch_id: false, has_eject: true },
    Model { pid: 0x026C, name: "Magic Keyboard with Numeric Keypad (2017)", has_numpad: true, has_touch_id: false, has_eject: true },
    Model { pid: 0x029A, name: "Magic Keyboard with Touch ID (2021)", has_numpad: false, has_touch_id: true, has_eject: false },
    Model { pid: 0x029C, name: "Magic Keyboard (2021)", has_numpad: false, has_touch_id: false, has_eject: false },
    Model { pid: 0x029F, name: "Magic Keyboard with Touch ID and Numeric Keypad (2021)", has_numpad: true, has_touch_id: true, has_eject: false },
    Model { pid: 0x0320, name: "Magic Keyboard (USB-C, 2024)", has_numpad: false, has_touch_id: false, has_eject: false },
    Model { pid: 0x0321, name: "Magic Keyboard with Touch ID (USB-C, 2024)", has_numpad: false, has_touch_id: true, has_eject: false },
    Model { pid: 0x0322, name: "Magic Keyboard with Touch ID and Numeric Keypad (USB-C, 2024)", has_numpad: true, has_touch_id: true, has_eject: false },
];

/// Looks up a model by vendor/product ID (USB or Bluetooth vendor ID).
pub fn find(vid: u16, pid: u16) -> Option<&'static Model> {
    if vid != APPLE_USB_VID && vid != APPLE_BT_VID {
        return None;
    }
    MODELS.iter().find(|m| m.pid == pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_primary_device_on_both_transports() {
        assert!(find(APPLE_USB_VID, 0x029F).unwrap().has_numpad);
        assert_eq!(find(APPLE_BT_VID, 0x029F), find(APPLE_USB_VID, 0x029F));
        assert!(find(0x046D, 0x029F).is_none());
    }
}
