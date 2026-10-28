//! Battery state from the Apple battery collection (vendor TLC FF00:0014, report 0x90,
//! Battery System page 0x85).

use std::io;

use crate::{apple_collections, Collection, Device, ReportKind};

pub const REPORT_ID: u8 = 0x90;
const PAGE_BATTERY_SYSTEM: u16 = 0x85;
const USAGE_CHARGING: u16 = 0x44;
const USAGE_FULLY_CHARGED: u16 = 0x46;
const USAGE_ABSOLUTE_STATE_OF_CHARGE: u16 = 0x65;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
    pub fully_charged: bool,
}

/// Decodes a battery report with the collection's own descriptor (no hard-coded offsets).
pub fn decode(dev: &Device, report: &[u8]) -> Option<Battery> {
    if report.first() != Some(&REPORT_ID) {
        return None;
    }
    let percent = dev.usage_value(
        ReportKind::Input,
        PAGE_BATTERY_SYSTEM,
        USAGE_ABSOLUTE_STATE_OF_CHARGE,
        report,
    )?;
    let flags = dev.usages(ReportKind::Input, PAGE_BATTERY_SYSTEM, report);
    Some(Battery {
        percent: percent.min(100) as u8,
        charging: flags.contains(&USAGE_CHARGING),
        fully_charged: flags.contains(&USAGE_FULLY_CHARGED),
    })
}

/// Battery collections of all connected Apple keyboards.
pub fn collections() -> io::Result<Vec<Collection>> {
    Ok(apple_collections()?
        .into_iter()
        .filter(Collection::is_battery)
        .collect())
}

/// Polls the current battery state of one keyboard.
pub fn read(c: &Collection) -> io::Result<Battery> {
    let dev = Device::open(c, true)?;
    let report = dev.get_input_report(REPORT_ID)?;
    decode(&dev, &report).ok_or_else(|| io::Error::other("unexpected battery report"))
}
