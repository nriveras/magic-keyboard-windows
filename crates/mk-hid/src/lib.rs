//! User-mode HID access for Apple keyboards: enumeration of top-level collections, capability
//! dumps, report I/O and battery decoding. Windows only.

#![cfg(windows)]

pub mod battery;
pub mod control;
mod ffi;

use std::fmt;
use std::io;

use windows_sys::Win32::Devices::HumanInterfaceDevice as hid;
use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

pub use ffi::{ButtonCap, ValueCap};

/// One HID top-level collection (one Windows device interface).
#[derive(Clone, Debug)]
pub struct Collection {
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub version: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub input_len: u16,
    pub output_len: u16,
    pub feature_len: u16,
}

impl Collection {
    pub fn model(&self) -> Option<&'static mk_core::model::Model> {
        mk_core::model::find(self.vid, self.pid)
    }

    pub fn is_bluetooth(&self) -> bool {
        self.vid == mk_core::model::APPLE_BT_VID
            || self.path.to_ascii_lowercase().contains("00001124")
    }

    /// Short devnode-style tag, e.g. `MI_01&Col02`, for display and filtering.
    pub fn tag(&self) -> String {
        let up = self.path.to_ascii_uppercase();
        let mut parts = Vec::new();
        for key in ["MI_", "COL"] {
            if let Some(i) = up.find(&format!("&{key}")) {
                let rest = &self.path[i + 1..];
                let end = rest.find(['&', '#']).unwrap_or(rest.len());
                parts.push(rest[..end].to_string());
            }
        }
        if parts.is_empty() {
            "-".into()
        } else {
            parts.join("&")
        }
    }

    pub fn is_battery(&self) -> bool {
        self.usage_page == 0xFF00 && self.usage == 0x0014
    }

    pub fn is_keyboard(&self) -> bool {
        self.usage_page == 0x01 && self.usage == 0x06
    }
}

impl fmt::Display for Collection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04X}:{:04X} {:<12} UP={:04X} U={:04X} in={:<3} out={:<3} feat={:<3} {}",
            self.vid,
            self.pid,
            self.tag(),
            self.usage_page,
            self.usage,
            self.input_len,
            self.output_len,
            self.feature_len,
            if self.is_bluetooth() { "BT" } else { "USB" }
        )
    }
}

/// An open handle to a collection.
pub struct Device {
    handle: HANDLE,
    preparsed: hid::PHIDP_PREPARSED_DATA,
    pub info: Collection,
}

// SAFETY: HID handles and preparsed data may be used from any thread.
unsafe impl Send for Device {}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            if self.preparsed != 0 {
                hid::HidD_FreePreparsedData(self.preparsed);
            }
            CloseHandle(self.handle);
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn open_raw(path: &str, access: u32) -> io::Result<HANDLE> {
    let w = wide(path);
    let h = unsafe {
        CreateFileW(
            w.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(h)
    }
}

/// Lists all HID collections whose vendor is Apple (USB or Bluetooth vendor ID).
pub fn apple_collections() -> io::Result<Vec<Collection>> {
    collections(|vid| vid == mk_core::model::APPLE_USB_VID || vid == mk_core::model::APPLE_BT_VID)
}

/// Lists every present HID collection that can be opened for attribute queries.
pub fn all_collections() -> io::Result<Vec<Collection>> {
    collections(|_| true)
}

/// Collections whose vendor ID passes `keep_vendor`, checked before the costlier queries.
fn collections(keep_vendor: fn(u16) -> bool) -> io::Result<Vec<Collection>> {
    let mut out = Vec::new();
    for path in ffi::hid_interface_paths()? {
        // Access 0 = query only; works even for collections opened exclusively by Windows.
        let Ok(h) = open_raw(&path, 0) else { continue };
        if let Some(c) = ffi::describe(h, &path, keep_vendor) {
            out.push(c);
        }
        unsafe { CloseHandle(h) };
    }
    Ok(out)
}

impl Device {
    /// Opens a collection. `rw` requests read/write access (needed for reports); without it only
    /// capabilities can be queried.
    pub fn open(info: &Collection, rw: bool) -> io::Result<Device> {
        let access = if rw { GENERIC_READ | GENERIC_WRITE } else { 0 };
        let handle = open_raw(&info.path, access)?;
        let mut preparsed: hid::PHIDP_PREPARSED_DATA = 0;
        if !unsafe { hid::HidD_GetPreparsedData(handle, &mut preparsed) } {
            let e = io::Error::last_os_error();
            unsafe { CloseHandle(handle) };
            return Err(e);
        }
        Ok(Device {
            handle,
            preparsed,
            info: info.clone(),
        })
    }

    pub fn button_caps(&self, kind: ReportKind) -> Vec<ButtonCap> {
        ffi::button_caps(self.preparsed, kind.raw())
    }

    pub fn value_caps(&self, kind: ReportKind) -> Vec<ValueCap> {
        ffi::value_caps(self.preparsed, kind.raw())
    }

    fn buffer(&self, kind: ReportKind, id: u8) -> Vec<u8> {
        let len = match kind {
            ReportKind::Input => self.info.input_len,
            ReportKind::Output => self.info.output_len,
            ReportKind::Feature => self.info.feature_len,
        } as usize;
        let mut b = vec![0u8; len.max(1)];
        b[0] = id;
        b
    }

    /// Requests an input report by ID through the control pipe (`HidD_GetInputReport`).
    pub fn get_input_report(&self, id: u8) -> io::Result<Vec<u8>> {
        let mut b = self.buffer(ReportKind::Input, id);
        if !unsafe { hid::HidD_GetInputReport(self.handle, b.as_mut_ptr().cast(), b.len() as u32) }
        {
            return Err(io::Error::last_os_error());
        }
        Ok(b)
    }

    /// Reads a feature report by ID (`HidD_GetFeature`). Read-only; never sets features.
    pub fn get_feature(&self, id: u8) -> io::Result<Vec<u8>> {
        let mut b = self.buffer(ReportKind::Feature, id);
        if !unsafe { hid::HidD_GetFeature(self.handle, b.as_mut_ptr().cast(), b.len() as u32) } {
            return Err(io::Error::last_os_error());
        }
        Ok(b)
    }

    /// Blocks until the next input report arrives on the interrupt pipe.
    pub fn read_report(&self) -> io::Result<Vec<u8>> {
        let mut b = vec![0u8; self.info.input_len.max(1) as usize];
        let mut n = 0u32;
        let ok = unsafe {
            ReadFile(
                self.handle,
                b.as_mut_ptr(),
                b.len() as u32,
                &mut n,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        b.truncate(n as usize);
        Ok(b)
    }

    /// Reads a scaled-less usage value from a report buffer.
    pub fn usage_value(
        &self,
        kind: ReportKind,
        page: u16,
        usage: u16,
        report: &[u8],
    ) -> Option<u32> {
        ffi::usage_value(self.preparsed, kind.raw(), page, usage, report)
    }

    /// Maximum number of simultaneously reported buttons of `page`.
    pub fn max_usages(&self, kind: ReportKind, page: u16) -> u32 {
        ffi::max_usages(self.preparsed, kind.raw(), page)
    }

    /// Buttons of `page` set in a report buffer.
    pub fn usages(&self, kind: ReportKind, page: u16, report: &[u8]) -> Vec<u16> {
        ffi::usages(self.preparsed, kind.raw(), page, report)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportKind {
    Input,
    Output,
    Feature,
}

impl ReportKind {
    fn raw(self) -> hid::HIDP_REPORT_TYPE {
        match self {
            ReportKind::Input => hid::HidP_Input,
            ReportKind::Output => hid::HidP_Output,
            ReportKind::Feature => hid::HidP_Feature,
        }
    }

    pub const ALL: [ReportKind; 3] = [ReportKind::Input, ReportKind::Output, ReportKind::Feature];
}

/// Formats bytes as space-separated hex.
pub fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
