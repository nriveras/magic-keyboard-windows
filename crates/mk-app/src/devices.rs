//! Connected Apple keyboards with their battery state, shared by the tray and the settings window.

use mk_core::model::{self, Model};
use mk_hid::battery::{self, Battery};
use mk_hid::Collection;

pub struct Keyboard {
    pub model: Option<&'static Model>,
    pub pid: u16,
    pub bluetooth: bool,
    pub battery: Option<Battery>,
}

impl Keyboard {
    pub fn name(&self) -> String {
        match self.model {
            Some(m) => m.name.to_string(),
            None => format!("Apple keyboard {:04X}", self.pid),
        }
    }

    /// Short name for the tray tooltip, which is limited to 128 characters.
    pub fn short_name(&self) -> &'static str {
        match self.model {
            Some(m) if m.has_numpad => "keypad model",
            Some(_) => "compact model",
            None => "keyboard",
        }
    }

    pub fn transport(&self) -> &'static str {
        if self.bluetooth {
            "Bluetooth"
        } else {
            "USB"
        }
    }
}

/// Every connected Apple keyboard, each paired with the battery collection of the same device.
///
/// Blocks on a battery GET_REPORT per keyboard (slow over Bluetooth): call it off the UI thread.
pub fn connected() -> Vec<Keyboard> {
    pair(mk_hid::apple_collections().unwrap_or_default())
        .into_iter()
        .map(|(c, battery)| Keyboard {
            model: c.model(),
            pid: c.pid,
            bluetooth: c.is_bluetooth(),
            battery: battery.and_then(|b| battery::read(&b).ok()),
        })
        .collect()
}

/// Pairs every supported keyboard collection with the first unused battery collection of the same
/// product and transport, from one enumeration of the Apple collections.
fn pair(collections: Vec<Collection>) -> Vec<(Collection, Option<Collection>)> {
    let (mut batteries, keyboards): (Vec<_>, Vec<_>) =
        collections.into_iter().partition(Collection::is_battery);
    keyboards
        .into_iter()
        .filter(|c| c.is_keyboard() && model::find(c.vid, c.pid).is_some())
        .map(|c| {
            let battery = batteries
                .iter()
                .position(|b| b.pid == c.pid && b.is_bluetooth() == c.is_bluetooth())
                .map(|i| batteries.remove(i));
            (c, battery)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::model::{APPLE_BT_VID, APPLE_USB_VID};

    fn col(vid: u16, pid: u16, usage_page: u16, usage: u16, path: &str) -> Collection {
        Collection {
            path: path.into(),
            vid,
            pid,
            version: 0,
            usage_page,
            usage,
            input_len: 0,
            output_len: 0,
            feature_len: 0,
        }
    }

    fn keyboard(vid: u16, pid: u16, path: &str) -> Collection {
        col(vid, pid, 0x01, 0x06, path)
    }

    fn battery(vid: u16, pid: u16, path: &str) -> Collection {
        col(vid, pid, 0xFF00, 0x0014, path)
    }

    #[test]
    fn pairs_keyboards_with_battery_of_same_product_and_transport() {
        let paired = pair(vec![
            battery(APPLE_USB_VID, 0x029F, "usb-bat"),
            battery(APPLE_BT_VID, 0x029F, "bt-bat"),
            keyboard(APPLE_BT_VID, 0x029F, "bt-kbd"),
            keyboard(APPLE_USB_VID, 0x029F, "usb-kbd"),
            keyboard(APPLE_USB_VID, 0x029A, "usb-kbd-2"),
        ]);
        let names: Vec<_> = paired
            .iter()
            .map(|(k, b)| (k.path.as_str(), b.as_ref().map(|b| b.path.as_str())))
            .collect();
        assert_eq!(
            names,
            vec![
                ("bt-kbd", Some("bt-bat")),
                ("usb-kbd", Some("usb-bat")),
                ("usb-kbd-2", None),
            ]
        );
    }

    #[test]
    fn skips_unsupported_and_non_keyboard_collections() {
        let paired = pair(vec![
            keyboard(APPLE_USB_VID, 0x0250, "old-model"),
            col(APPLE_USB_VID, 0x029F, 0x0C, 0x01, "consumer"),
            keyboard(0x046D, 0x029F, "logitech"),
        ]);
        assert!(paired.is_empty());
    }
}
