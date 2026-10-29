//! Detects a filtered keyboard whose driver stack failed to start, typically because Secure Boot
//! was turned back on: Windows then refuses the test-signed driver (problem code 52) and the
//! keyboard stops working until Secure Boot is off again or the driver is uninstalled.

use std::ptr::null_mut;

use mk_core::model::MODELS;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_DevNode_Registry_PropertyW, CM_Get_DevNode_Status, CM_Get_Device_ID_ListW,
    CM_Get_Device_ID_List_SizeW, CM_Locate_DevNodeW, CM_DRP_LOWERFILTERS,
    CM_GETIDLIST_FILTER_ENUMERATOR, CM_GETIDLIST_FILTER_PRESENT, CM_LOCATE_DEVNODE_NORMAL,
    CR_SUCCESS, DN_HAS_PROBLEM,
};
use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD};

use crate::system::wide;

/// Problem code Windows sets when a driver's signature is not accepted.
const PROBLEM_UNSIGNED_DRIVER: u32 = 52;

/// A filtered keyboard collection that is not running.
pub struct Failure {
    pub problem: u32,
}

impl Failure {
    /// Text for the tray notification and menu.
    pub fn message(&self, secure_boot: bool) -> String {
        if self.problem == PROBLEM_UNSIGNED_DRIVER || secure_boot {
            "Windows blocked the Magic Keyboard driver (Secure Boot is on). Turn Secure Boot off \
             again, or run uninstall.cmd to get the standard keyboard back."
                .into()
        } else {
            format!(
                "The Magic Keyboard driver failed to start (code {}). Run uninstall.cmd to get the \
                 standard keyboard back.",
                self.problem
            )
        }
    }
}

/// True if a device instance ID is the keyboard collection of a supported model (USB or BT).
fn is_supported_keyboard(id: &str) -> bool {
    let id = id.to_ascii_uppercase();
    MODELS.iter().any(|m| {
        id.contains(&format!("VID_05AC&PID_{:04X}&MI_01&COL01", m.pid))
            || id.contains(&format!("_VID&0001004C_PID&{:04X}&COL01", m.pid))
    })
}

fn present_hid_device_ids() -> Vec<String> {
    let filter = wide("HID");
    let flags = CM_GETIDLIST_FILTER_ENUMERATOR | CM_GETIDLIST_FILTER_PRESENT;
    let mut len = 0u32;
    if unsafe { CM_Get_Device_ID_List_SizeW(&mut len, filter.as_ptr(), flags) } != CR_SUCCESS {
        return Vec::new();
    }
    let mut buffer = vec![0u16; len as usize];
    if unsafe { CM_Get_Device_ID_ListW(filter.as_ptr(), buffer.as_mut_ptr(), len, flags) }
        != CR_SUCCESS
    {
        return Vec::new();
    }
    buffer
        .split(|c| *c == 0)
        .filter(|s| !s.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

fn has_mkfilter(devinst: u32) -> bool {
    let mut buffer = [0u16; 256];
    let mut size = std::mem::size_of_val(&buffer) as u32;
    let ok = unsafe {
        CM_Get_DevNode_Registry_PropertyW(
            devinst,
            CM_DRP_LOWERFILTERS,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
            0,
        )
    } == CR_SUCCESS;
    ok && buffer
        .split(|c| *c == 0)
        .any(|s| String::from_utf16_lossy(s).eq_ignore_ascii_case("mkfilter"))
}

/// The first supported keyboard that has mkfilter installed but is not running, if any.
pub fn failed_keyboard() -> Option<Failure> {
    present_hid_device_ids()
        .into_iter()
        .filter(|id| is_supported_keyboard(id))
        .find_map(|id| {
            let w = wide(&id);
            let mut devinst = 0u32;
            if unsafe { CM_Locate_DevNodeW(&mut devinst, w.as_ptr(), CM_LOCATE_DEVNODE_NORMAL) }
                != CR_SUCCESS
            {
                return None;
            }
            let (mut status, mut problem) = (0u32, 0u32);
            if unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, devinst, 0) } != CR_SUCCESS
            {
                return None;
            }
            (status & DN_HAS_PROBLEM != 0 && has_mkfilter(devinst)).then_some(Failure { problem })
        })
}

/// True if the firmware booted Windows with Secure Boot on.
pub fn secure_boot_on() -> bool {
    let key = wide("SYSTEM\\CurrentControlSet\\Control\\SecureBoot\\State");
    let value = wide("UEFISecureBootEnabled");
    let mut data = 0u32;
    let mut size = 4u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            (&raw mut data).cast(),
            &mut size,
        )
    };
    status == 0 && data != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_usb_and_bluetooth_keyboard_collections() {
        assert!(is_supported_keyboard(
            "HID\\VID_05AC&PID_029F&MI_01&COL01\\7&1&0&0000"
        ));
        assert!(is_supported_keyboard(
            "HID\\{00001124-0000-1000-8000-00805F9B34FB}_VID&0001004C_PID&029A&COL01\\8&1&0&0000"
        ));
        assert!(!is_supported_keyboard(
            "HID\\VID_05AC&PID_029F&MI_00&COL02\\7&1&0&0000"
        ));
        assert!(!is_supported_keyboard(
            "HID\\VID_046D&PID_C52B&MI_00\\7&1&0&0000"
        ));
    }
}
