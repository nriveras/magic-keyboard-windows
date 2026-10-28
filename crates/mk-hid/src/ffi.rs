//! Thin wrappers over hid.dll / cfgmgr32 calls.

use std::io;
use std::mem::{size_of, zeroed};

use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_Device_Interface_ListW, CM_Get_Device_Interface_List_SizeW,
    CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CR_BUFFER_SMALL, CR_SUCCESS,
};
use windows_sys::Win32::Devices::HumanInterfaceDevice as hid;
use windows_sys::Win32::Foundation::HANDLE;

use crate::Collection;

const HIDP_STATUS_SUCCESS: i32 = 0x0011_0000;

/// Device interface paths of all present HID collections.
pub(crate) fn hid_interface_paths() -> io::Result<Vec<String>> {
    let mut guid: GUID = unsafe { zeroed() };
    unsafe { hid::HidD_GetHidGuid(&mut guid) };
    interface_paths(&guid)
}

/// Paths of all present device interfaces of class `guid`.
pub(crate) fn interface_paths(guid: &GUID) -> io::Result<Vec<String>> {
    loop {
        let mut len = 0u32;
        let cr = unsafe {
            CM_Get_Device_Interface_List_SizeW(
                &mut len,
                guid,
                std::ptr::null(),
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        if cr != CR_SUCCESS {
            return Err(io::Error::other(format!(
                "CM_Get_Device_Interface_List_Size failed: {cr}"
            )));
        }
        let mut buf = vec![0u16; len as usize];
        let cr = unsafe {
            CM_Get_Device_Interface_ListW(
                guid,
                std::ptr::null(),
                buf.as_mut_ptr(),
                len,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        if cr == CR_BUFFER_SMALL {
            continue; // a device arrived between the two calls
        }
        if cr != CR_SUCCESS {
            return Err(io::Error::other(format!(
                "CM_Get_Device_Interface_List failed: {cr}"
            )));
        }
        return Ok(buf
            .split(|c| *c == 0)
            .filter(|s| !s.is_empty())
            .map(String::from_utf16_lossy)
            .collect());
    }
}

pub(crate) fn describe(h: HANDLE, path: &str) -> Option<Collection> {
    let mut attrs: hid::HIDD_ATTRIBUTES = unsafe { zeroed() };
    attrs.Size = size_of::<hid::HIDD_ATTRIBUTES>() as u32;
    if !unsafe { hid::HidD_GetAttributes(h, &mut attrs) } {
        return None;
    }
    let mut pp: hid::PHIDP_PREPARSED_DATA = 0;
    if !unsafe { hid::HidD_GetPreparsedData(h, &mut pp) } {
        return None;
    }
    let mut caps: hid::HIDP_CAPS = unsafe { zeroed() };
    let st = unsafe { hid::HidP_GetCaps(pp, &mut caps) };
    unsafe { hid::HidD_FreePreparsedData(pp) };
    if st != HIDP_STATUS_SUCCESS {
        return None;
    }
    let mut name = [0u16; 128];
    let product = if unsafe {
        hid::HidD_GetProductString(h, name.as_mut_ptr().cast(), size_of::<[u16; 128]>() as u32)
    } {
        let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
        Some(String::from_utf16_lossy(&name[..end]))
    } else {
        None
    };
    Some(Collection {
        path: path.to_string(),
        vid: attrs.VendorID,
        pid: attrs.ProductID,
        version: attrs.VersionNumber,
        usage_page: caps.UsagePage,
        usage: caps.Usage,
        input_len: caps.InputReportByteLength,
        output_len: caps.OutputReportByteLength,
        feature_len: caps.FeatureReportByteLength,
        product,
    })
}

/// A button capability (single usage or usage range; arrays show up as ranges).
#[derive(Clone, Debug)]
pub struct ButtonCap {
    pub report_id: u8,
    pub usage_page: u16,
    pub usage_min: u16,
    pub usage_max: u16,
    pub is_range: bool,
    pub report_count: u16,
    pub link_collection: u16,
    pub data_index_min: u16,
    pub data_index_max: u16,
}

/// A value capability.
#[derive(Clone, Debug)]
pub struct ValueCap {
    pub report_id: u8,
    pub usage_page: u16,
    pub usage_min: u16,
    pub usage_max: u16,
    pub is_range: bool,
    pub bit_size: u16,
    pub report_count: u16,
    pub logical_min: i32,
    pub logical_max: i32,
    pub link_collection: u16,
}

fn caps_counts(pp: hid::PHIDP_PREPARSED_DATA, kind: hid::HIDP_REPORT_TYPE) -> (u16, u16) {
    let mut caps: hid::HIDP_CAPS = unsafe { zeroed() };
    unsafe { hid::HidP_GetCaps(pp, &mut caps) };
    match kind {
        hid::HidP_Input => (caps.NumberInputButtonCaps, caps.NumberInputValueCaps),
        hid::HidP_Output => (caps.NumberOutputButtonCaps, caps.NumberOutputValueCaps),
        _ => (caps.NumberFeatureButtonCaps, caps.NumberFeatureValueCaps),
    }
}

pub(crate) fn button_caps(
    pp: hid::PHIDP_PREPARSED_DATA,
    kind: hid::HIDP_REPORT_TYPE,
) -> Vec<ButtonCap> {
    let (mut n, _) = caps_counts(pp, kind);
    if n == 0 {
        return Vec::new();
    }
    let mut raw: Vec<hid::HIDP_BUTTON_CAPS> = (0..n).map(|_| unsafe { zeroed() }).collect();
    if unsafe { hid::HidP_GetButtonCaps(kind, raw.as_mut_ptr(), &mut n, pp) } != HIDP_STATUS_SUCCESS
    {
        return Vec::new();
    }
    raw.truncate(n as usize);
    raw.iter()
        .map(|c| unsafe {
            let is_range = c.IsRange;
            let (min, max, dmin, dmax) = if is_range {
                let r = c.Anonymous.Range;
                (r.UsageMin, r.UsageMax, r.DataIndexMin, r.DataIndexMax)
            } else {
                let r = c.Anonymous.NotRange;
                (r.Usage, r.Usage, r.DataIndex, r.DataIndex)
            };
            ButtonCap {
                report_id: c.ReportID,
                usage_page: c.UsagePage,
                usage_min: min,
                usage_max: max,
                is_range,
                report_count: c.ReportCount,
                link_collection: c.LinkCollection,
                data_index_min: dmin,
                data_index_max: dmax,
            }
        })
        .collect()
}

pub(crate) fn value_caps(
    pp: hid::PHIDP_PREPARSED_DATA,
    kind: hid::HIDP_REPORT_TYPE,
) -> Vec<ValueCap> {
    let (_, mut n) = caps_counts(pp, kind);
    if n == 0 {
        return Vec::new();
    }
    let mut raw: Vec<hid::HIDP_VALUE_CAPS> = (0..n).map(|_| unsafe { zeroed() }).collect();
    if unsafe { hid::HidP_GetValueCaps(kind, raw.as_mut_ptr(), &mut n, pp) } != HIDP_STATUS_SUCCESS
    {
        return Vec::new();
    }
    raw.truncate(n as usize);
    raw.iter()
        .map(|c| unsafe {
            let is_range = c.IsRange;
            let (min, max) = if is_range {
                (c.Anonymous.Range.UsageMin, c.Anonymous.Range.UsageMax)
            } else {
                (c.Anonymous.NotRange.Usage, c.Anonymous.NotRange.Usage)
            };
            ValueCap {
                report_id: c.ReportID,
                usage_page: c.UsagePage,
                usage_min: min,
                usage_max: max,
                is_range,
                bit_size: c.BitSize,
                report_count: c.ReportCount,
                logical_min: c.LogicalMin,
                logical_max: c.LogicalMax,
                link_collection: c.LinkCollection,
            }
        })
        .collect()
}

pub(crate) fn usage_value(
    pp: hid::PHIDP_PREPARSED_DATA,
    kind: hid::HIDP_REPORT_TYPE,
    page: u16,
    usage: u16,
    report: &[u8],
) -> Option<u32> {
    let mut v = 0u32;
    let st = unsafe {
        hid::HidP_GetUsageValue(
            kind,
            page,
            0,
            usage,
            &mut v,
            pp,
            report.as_ptr().cast(),
            report.len() as u32,
        )
    };
    (st == HIDP_STATUS_SUCCESS).then_some(v)
}

pub(crate) fn usages(
    pp: hid::PHIDP_PREPARSED_DATA,
    kind: hid::HIDP_REPORT_TYPE,
    page: u16,
    report: &[u8],
) -> Vec<u16> {
    let mut list = [0u16; 64];
    let mut n = list.len() as u32;
    let st = unsafe {
        hid::HidP_GetUsages(
            kind,
            page,
            0,
            list.as_mut_ptr(),
            &mut n,
            pp,
            report.as_ptr() as _,
            report.len() as u32,
        )
    };
    if st != HIDP_STATUS_SUCCESS {
        return Vec::new();
    }
    list[..n as usize].to_vec()
}

/// Maximum number of buttons of `page` one report can carry (e.g. the key-array slot count).
pub(crate) fn max_usages(
    pp: hid::PHIDP_PREPARSED_DATA,
    kind: hid::HIDP_REPORT_TYPE,
    page: u16,
) -> u32 {
    unsafe { hid::HidP_MaxUsageListLength(kind, page, pp) }
}
