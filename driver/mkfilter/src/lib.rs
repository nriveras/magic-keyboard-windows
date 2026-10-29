//! mkfilter: KMDF driver for Apple Magic Keyboards. One image, two roles:
//!
//! - [`filter`]: lower filter on each keyboard collection devnode; rewrites input reports through
//!   the mk-core engine.
//! - [`hub`]: function driver of the root-enumerated `ROOT\MagicKbdHub` devnode; owns the virtual
//!   HID device (VHF) that carries media keys, volume, sleep and synthesized shortcuts.
//!
//! The role is picked in EvtDriverDeviceAdd from the devnode's first hardware ID.
//!
//! Debug builds log every report with `DbgPrint` (view with DebugView, "Capture Kernel" + verbose
//! output); release builds never log report contents.

#![no_std]

mod control;
mod filter;
mod hid;
mod hub;
mod registry;
mod shared;

use core::mem::size_of;

use wdk::println;
use wdk_sys::{
    _WDF_EXECUTION_LEVEL::WdfExecutionLevelInheritFromParent,
    _WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeInheritFromParent,
    DEVICE_REGISTRY_PROPERTY::DevicePropertyHardwareID, NTSTATUS, PCUNICODE_STRING, PDRIVER_OBJECT,
    PWDFDEVICE_INIT, ULONG, WDF_DRIVER_CONFIG, WDF_NO_OBJECT_ATTRIBUTES, WDF_OBJECT_ATTRIBUTES,
    WDFDEVICE, WDFDRIVER, WDFOBJECT, WDFSPINLOCK, call_unsafe_wdf_function_binding,
};

#[cfg(not(test))]
extern crate wdk_panic;

#[cfg(not(test))]
use wdk_alloc::WdkAllocator;

#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WdkAllocator = WdkAllocator;

const fn utf16<const N: usize>(s: &[u8; N]) -> [u16; N] {
    let mut out = [0u16; N];
    let mut i = 0;
    while i < N {
        out[i] = s[i] as u16;
        i += 1;
    }
    out
}

fn nt_success(status: NTSTATUS) -> bool {
    status >= 0
}

fn check(status: NTSTATUS) -> Result<(), NTSTATUS> {
    if nt_success(status) {
        Ok(())
    } else {
        Err(status)
    }
}

/// Object attributes for a child of `parent` without a context.
fn child_attributes(parent: WDFDEVICE) -> WDF_OBJECT_ATTRIBUTES {
    WDF_OBJECT_ATTRIBUTES {
        Size: size_of::<WDF_OBJECT_ATTRIBUTES>() as ULONG,
        ExecutionLevel: WdfExecutionLevelInheritFromParent,
        SynchronizationScope: WdfSynchronizationScopeInheritFromParent,
        ParentObject: parent as WDFOBJECT,
        ..Default::default()
    }
}

/// Driver entry point.
///
/// # Safety
/// Called once by the I/O manager with a valid driver object and registry path.
// SAFETY: "DriverEntry" is the required symbol name for Windows driver entry points.
// No other function in this compilation unit exports this name, preventing symbol conflicts.
#[unsafe(export_name = "DriverEntry")]
pub unsafe extern "system" fn driver_entry(
    driver: PDRIVER_OBJECT,
    registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    let mut config = WDF_DRIVER_CONFIG {
        Size: size_of::<WDF_DRIVER_CONFIG>() as ULONG,
        EvtDriverDeviceAdd: Some(evt_device_add),
        ..Default::default()
    };
    let mut driver_handle: WDFDRIVER = core::ptr::null_mut();
    let status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfDriverCreate,
            driver,
            registry_path,
            WDF_NO_OBJECT_ATTRIBUTES,
            &mut config,
            &mut driver_handle
        )
    };
    if nt_success(status) {
        // Driver-wide lock for the shared state; parented to the driver object.
        let mut lock: WDFSPINLOCK = core::ptr::null_mut();
        let st = unsafe {
            call_unsafe_wdf_function_binding!(
                WdfSpinLockCreate,
                WDF_NO_OBJECT_ATTRIBUTES,
                &mut lock
            )
        };
        if nt_success(st) {
            shared::set_lock(lock);
            let config = unsafe { registry::read_config(driver_handle) };
            shared::with(|s| s.config.set(config));
        } else {
            println!("mkfilter: driver lock creation failed {st:#010x}");
        }
    }
    println!("mkfilter: DriverEntry {status:#010x}");
    status
}

extern "C" fn evt_device_add(_driver: WDFDRIVER, init: PWDFDEVICE_INIT) -> NTSTATUS {
    if unsafe { is_hub(init) } {
        unsafe { hub::device_add(init) }
    } else {
        unsafe { filter::device_add(init) }
    }
}

/// True if the devnode's first hardware ID is the hub's (ASCII case-insensitive).
///
/// # Safety
/// `init` must be the framework's device init; PASSIVE_LEVEL.
unsafe fn is_hub(init: PWDFDEVICE_INIT) -> bool {
    let mut ids = [0u16; 512];
    let mut length: ULONG = 0;
    let status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfFdoInitQueryProperty,
            init,
            DevicePropertyHardwareID,
            size_of::<[u16; 512]>() as ULONG,
            ids.as_mut_ptr().cast(),
            &mut length
        )
    };
    if !nt_success(status) {
        return false;
    }
    let first = ids.split(|c| *c == 0).next().unwrap_or(&[]);
    first.len() == hub::HARDWARE_ID.len()
        && first
            .iter()
            .zip(hub::HARDWARE_ID.iter())
            .all(|(a, b)| ascii_lower(*a) == ascii_lower(*b))
}

fn ascii_lower(c: u16) -> u16 {
    if (b'A' as u16..=b'Z' as u16).contains(&c) {
        c + 32
    } else {
        c
    }
}
