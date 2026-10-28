//! Persistent configuration: `HKLM\SYSTEM\CurrentControlSet\Services\mkfilter\Parameters\Config`
//! (REG_BINARY, 64-byte config wire format).

use core::ptr::null_mut;

use mk_core::Config;
use mk_core::config::WIRE_SIZE;
use wdk::println;
use wdk_sys::{
    KEY_READ, KEY_WRITE, NTSTATUS, PWCH, REG_BINARY, ULONG, UNICODE_STRING,
    WDF_NO_OBJECT_ATTRIBUTES, WDFDRIVER, WDFKEY, call_unsafe_wdf_function_binding,
};

use crate::{nt_success, utf16};

const CONFIG_VALUE_NAME: [u16; 6] = utf16(b"Config");

/// Opens the driver's Parameters key.
///
/// # Safety
/// PASSIVE_LEVEL.
unsafe fn open(driver: WDFDRIVER, access: ULONG) -> Result<WDFKEY, NTSTATUS> {
    let mut key: WDFKEY = null_mut();
    let status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfDriverOpenParametersRegistryKey,
            driver,
            access,
            WDF_NO_OBJECT_ATTRIBUTES,
            &mut key
        )
    };
    if nt_success(status) {
        Ok(key)
    } else {
        Err(status)
    }
}

/// Runs `f` with the value name as a UNICODE_STRING.
fn with_name<R>(f: impl FnOnce(&UNICODE_STRING) -> R) -> R {
    let mut buffer = CONFIG_VALUE_NAME;
    let name = UNICODE_STRING {
        Length: (buffer.len() * 2) as u16,
        MaximumLength: (buffer.len() * 2) as u16,
        Buffer: buffer.as_mut_ptr() as PWCH,
    };
    f(&name)
}

/// Reads the stored configuration, if present and valid.
///
/// # Safety
/// PASSIVE_LEVEL.
pub unsafe fn read_config(driver: WDFDRIVER) -> Option<Config> {
    let key = unsafe { open(driver, KEY_READ) }.ok()?;
    let mut value = [0u8; WIRE_SIZE];
    let mut length: ULONG = 0;
    let mut value_type: ULONG = 0;
    let status = with_name(|name| unsafe {
        call_unsafe_wdf_function_binding!(
            WdfRegistryQueryValue,
            key,
            name,
            WIRE_SIZE as ULONG,
            value.as_mut_ptr().cast(),
            &mut length,
            &mut value_type
        )
    });
    unsafe { call_unsafe_wdf_function_binding!(WdfRegistryClose, key) };
    if !nt_success(status) || value_type != REG_BINARY {
        return None;
    }
    match Config::from_wire(&value[..(length as usize).min(WIRE_SIZE)]) {
        Ok(config) => Some(config),
        Err(e) => {
            println!("mkfilter: ignoring invalid registry config {e:?}");
            None
        }
    }
}

/// Stores `config`.
///
/// # Safety
/// PASSIVE_LEVEL.
pub unsafe fn write_config(driver: WDFDRIVER, config: &Config) -> NTSTATUS {
    let key = match unsafe { open(driver, KEY_READ | KEY_WRITE) } {
        Ok(key) => key,
        Err(status) => return status,
    };
    let mut wire = config.to_wire();
    let status = with_name(|name| unsafe {
        call_unsafe_wdf_function_binding!(
            WdfRegistryAssignValue,
            key,
            name,
            REG_BINARY,
            WIRE_SIZE as ULONG,
            wire.as_mut_ptr().cast()
        )
    });
    unsafe { call_unsafe_wdf_function_binding!(WdfRegistryClose, key) };
    status
}
