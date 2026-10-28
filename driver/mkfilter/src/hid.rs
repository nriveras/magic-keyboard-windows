//! HID parser glue: decodes an input report of the keyboard collection into an mk-core
//! [`KeyState`] and rebuilds the report from the engine output, using the collection's preparsed
//! data (hidparse.sys). Callable at IRQL <= DISPATCH_LEVEL.

use mk_core::state::{KbdReport, KeyState, MAX_KEYS};
use mk_core::usage::{consumer, kbd, page};
use wdk_sys::{
    _HIDP_REPORT_TYPE::HidP_Input,
    _USAGE_AND_PAGE, NTSTATUS, PCHAR, PHIDP_PREPARSED_DATA, ULONG, USAGE,
    hid::{HidP_GetUsagesEx, HidP_InitializeReportForID, HidP_SetUsages},
};

pub const HIDP_STATUS_SUCCESS: NTSTATUS = 0x0011_0000;
pub const HIDP_STATUS_BUFFER_TOO_SMALL: NTSTATUS = 0xC011_0007_u32 as NTSTATUS;

/// Upper bound for usages in one report: 8 modifiers + 6 array slots + 4 Apple bits, with headroom.
const MAX_USAGES: usize = 32;

/// Usages reported by `HidP_GetUsagesEx`, kept for the debug log (`page:usage`).
pub struct Usages {
    list: [_USAGE_AND_PAGE; MAX_USAGES],
    len: usize,
}

impl Usages {
    fn as_slice(&self) -> &[_USAGE_AND_PAGE] {
        &self.list[..self.len]
    }
}

impl core::fmt::Debug for Usages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("[")?;
        for (i, u) in self.as_slice().iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{:02X}:{:02X}", u.UsagePage, u.Usage)?;
        }
        f.write_str("]")
    }
}

/// Decodes one input report. Returns `None` if the parser rejects it (e.g. an unknown report ID).
/// `lock_reports_menu` treats consumer Menu as the Lock key (see `Model::lock_reports_menu`).
///
/// # Safety
/// `preparsed` must be the preparsed data of the collection that produced `report`.
pub unsafe fn decode(
    preparsed: PHIDP_PREPARSED_DATA,
    report: &[u8],
    lock_reports_menu: bool,
) -> Option<(KeyState, Usages)> {
    let mut usages = Usages {
        list: [_USAGE_AND_PAGE::default(); MAX_USAGES],
        len: 0,
    };
    let mut count = MAX_USAGES as ULONG;
    let status = unsafe {
        HidP_GetUsagesEx(
            HidP_Input,
            0,
            usages.list.as_mut_ptr(),
            &mut count,
            preparsed,
            report.as_ptr() as PCHAR,
            report.len() as ULONG,
        )
    };
    if status != HIDP_STATUS_SUCCESS {
        return None;
    }
    usages.len = (count as usize).min(MAX_USAGES);

    let mut state = KeyState::new();
    for u in usages.as_slice() {
        match (u.UsagePage, u.Usage) {
            (page::KEYBOARD, k @ 0x01..=0xFF) => {
                state.press(k as u8);
            }
            (page::APPLE_TOP_CASE, page::APPLE_FN_USAGE) => state.fn_key = true,
            (page::CONSUMER, consumer::EJECT) => state.eject = true,
            (page::CONSUMER, consumer::MENU) if lock_reports_menu => state.lock = true,
            (page::CONSUMER, consumer::MENU) => state.menu = true,
            (page::CONSUMER, consumer::AL_TERMINAL_LOCK) => state.lock = true,
            _ => {}
        }
    }
    Some((state, usages))
}

/// Rebuilds `report` (same report ID) from scratch with keyboard-page usages only, so the Apple
/// bits (Fn, Eject, Menu, Lock) never reach kbdhid. More keys than the report's array slots
/// become an ErrorRollOver report instead of being silently truncated.
///
/// # Safety
/// `preparsed` must be the preparsed data of the collection that produced `report`.
pub unsafe fn encode(
    preparsed: PHIDP_PREPARSED_DATA,
    report: &mut [u8],
    out: &KbdReport,
) -> NTSTATUS {
    let mut usages = [0 as USAGE; 8 + MAX_KEYS];
    let mut n = 0;
    for bit in 0..8u8 {
        if out.mods & (1 << bit) != 0 {
            usages[n] = (kbd::LEFT_CTRL + bit) as USAGE;
            n += 1;
        }
    }
    let mods = n;
    for &k in out.keys.as_slice() {
        usages[n] = k as USAGE;
        n += 1;
    }

    let status = unsafe { set_usages(preparsed, report, &mut usages[..n]) };
    if status != HIDP_STATUS_BUFFER_TOO_SMALL {
        return status;
    }
    usages[mods] = kbd::ERROR_ROLLOVER as USAGE;
    unsafe { set_usages(preparsed, report, &mut usages[..=mods]) }
}

unsafe fn set_usages(
    preparsed: PHIDP_PREPARSED_DATA,
    report: &mut [u8],
    usages: &mut [USAGE],
) -> NTSTATUS {
    let id = report[0];
    let buffer = report.as_mut_ptr() as PCHAR;
    let length = report.len() as ULONG;
    let status = unsafe { HidP_InitializeReportForID(HidP_Input, id, preparsed, buffer, length) };
    if status != HIDP_STATUS_SUCCESS || usages.is_empty() {
        return status;
    }
    let mut count = usages.len() as ULONG;
    unsafe {
        HidP_SetUsages(
            HidP_Input,
            page::KEYBOARD,
            0,
            usages.as_mut_ptr(),
            &mut count,
            preparsed,
            buffer,
            length,
        )
    }
}
