//! Connected Apple keyboards with their battery state, shared by the tray and the settings window.

use mk_core::model::{self, Model};
use mk_hid::battery::{self, Battery};

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
pub fn connected() -> Vec<Keyboard> {
    let mut batteries: Vec<_> = battery::collections().unwrap_or_default();
    mk_hid::apple_collections()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.is_keyboard() && model::find(c.vid, c.pid).is_some())
        .map(|c| {
            // Pair with the first unused battery collection of the same product and transport.
            let battery = batteries
                .iter()
                .position(|b| b.pid == c.pid && b.is_bluetooth() == c.is_bluetooth())
                .map(|i| batteries.remove(i))
                .and_then(|b| battery::read(&b).ok());
            Keyboard {
                model: c.model(),
                pid: c.pid,
                bluetooth: c.is_bluetooth(),
                battery,
            }
        })
        .collect()
}
