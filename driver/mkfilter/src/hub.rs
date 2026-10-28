//! Hub role: a root-enumerated device (`ROOT\MagicKbdHub`, installed by mkhub.inf with vhf.sys as
//! its lower filter) that owns one Virtual HID Framework device with keyboard, consumer-control
//! and system-control collections. Filter instances submit the engine's virtual outputs here, so
//! Windows handles media keys, volume, sleep and synthesized shortcuts natively.
//!
//! The VHF handle is driver-global because the hub and the keyboard filters are different devnodes
//! served by the same driver image.

use core::ffi::c_void;
use core::mem::{size_of, zeroed};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicPtr, Ordering};

use mk_core::Outputs;
use mk_core::state::{KbdReport, MAX_CONSUMER};
use wdk::println;
use wdk_sys::{
    _HID_XFER_PACKET, _VHF_CONFIG,
    _WDF_EXECUTION_LEVEL::WdfExecutionLevelInheritFromParent,
    _WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeInheritFromParent,
    NTSTATUS, PUCHAR, PWDFDEVICE_INIT, STATUS_DEVICE_ALREADY_ATTACHED, STATUS_SUCCESS, ULONG,
    VHFHANDLE, WDF_OBJECT_ATTRIBUTES, WDFDEVICE, WDFOBJECT, WDFSPINLOCK,
    call_unsafe_wdf_function_binding,
    hid::{VhfCreate, VhfDelete, VhfReadReportSubmit, VhfStart},
};

use crate::{nt_success, utf16};

/// Hardware ID of the hub devnode (created by scripts\install-dev.ps1).
pub const HARDWARE_ID: [u16; 16] = utf16(b"ROOT\\MagicKbdHub");

const REPORT_KEYBOARD: u8 = 1;
const REPORT_CONSUMER: u8 = 2;
const REPORT_SYSTEM: u8 = 3;
/// System-control usages are sent as `usage - SYSTEM_BASE` (logical 1..3 = 0x81..0x83).
const SYSTEM_BASE: u8 = 0x80;

/// Report descriptor of the virtual device: three top-level collections.
static REPORT_DESCRIPTOR: &[u8] = &[
    // Keyboard (ID 1): modifier byte, reserved byte, 6-key array. No LED output.
    0x05,
    0x01,
    0x09,
    0x06,
    0xA1,
    0x01,
    0x85,
    REPORT_KEYBOARD, //
    0x05,
    0x07,
    0x19,
    0xE0,
    0x29,
    0xE7,
    0x15,
    0x00,
    0x25,
    0x01,
    0x75,
    0x01,
    0x95,
    0x08,
    0x81,
    0x02,
    0x75,
    0x08,
    0x95,
    0x01,
    0x81,
    0x01, //
    0x19,
    0x00,
    0x2A,
    0xFF,
    0x00,
    0x15,
    0x00,
    0x26,
    0xFF,
    0x00,
    0x75,
    0x08,
    0x95,
    0x06,
    0x81,
    0x00,
    0xC0, //
    // Consumer control (ID 2): 4 x 16-bit usage array.
    0x05,
    0x0C,
    0x09,
    0x01,
    0xA1,
    0x01,
    0x85,
    REPORT_CONSUMER, //
    0x19,
    0x00,
    0x2A,
    0xFF,
    0x03,
    0x15,
    0x00,
    0x26,
    0xFF,
    0x03,
    0x75,
    0x10,
    0x95,
    0x04,
    0x81,
    0x00,
    0xC0, //
    // System control (ID 3): one 8-bit array entry, 1..3 = Power Down / Sleep / Wake Up.
    0x05,
    0x01,
    0x09,
    0x80,
    0xA1,
    0x01,
    0x85,
    REPORT_SYSTEM, //
    0x19,
    0x81,
    0x29,
    0x83,
    0x15,
    0x01,
    0x25,
    0x03,
    0x75,
    0x08,
    0x95,
    0x01,
    0x81,
    0x00, //
    0xC0,
];

/// The started VHF device, or null.
static VHF: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
/// Driver-wide spinlock serializing submissions against hub teardown (created in DriverEntry).
static LOCK: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

/// Stores the driver-wide lock. Called once from DriverEntry.
pub fn set_lock(lock: WDFSPINLOCK) {
    LOCK.store(lock.cast(), Ordering::Release);
}

/// True once the virtual device is up.
pub fn is_ready() -> bool {
    !VHF.load(Ordering::Acquire).is_null()
}

/// Runs `f` with the VHF handle under the driver-wide lock; does nothing if the hub is absent.
fn with_vhf(f: impl FnOnce(VHFHANDLE)) {
    let lock: WDFSPINLOCK = LOCK.load(Ordering::Acquire).cast();
    if lock.is_null() {
        return;
    }
    unsafe { call_unsafe_wdf_function_binding!(WdfSpinLockAcquire, lock) };
    let handle = VHF.load(Ordering::Acquire);
    if !handle.is_null() {
        f(handle);
    }
    unsafe { call_unsafe_wdf_function_binding!(WdfSpinLockRelease, lock) };
}

/// Sends the virtual-device part of `out` (only what changed). IRQL <= DISPATCH_LEVEL.
pub fn submit(out: &Outputs) {
    if !out.any_virtual_change() {
        return;
    }
    with_vhf(|handle| {
        if let Some(pulse) = &out.pulse {
            send_keyboard(handle, pulse);
            send_keyboard(handle, &out.virt_keyboard);
        } else if out.virt_keyboard_changed {
            send_keyboard(handle, &out.virt_keyboard);
        }
        if out.consumer_changed {
            let mut report = [0u8; 1 + 2 * MAX_CONSUMER];
            report[0] = REPORT_CONSUMER;
            for (i, usage) in out.consumer.iter().enumerate() {
                report[1 + 2 * i..3 + 2 * i].copy_from_slice(&usage.to_le_bytes());
            }
            send(handle, &mut report);
        }
        if out.system_changed {
            let value = match out.system {
                u @ 0x81..=0x83 => u - SYSTEM_BASE,
                _ => 0,
            };
            send(handle, &mut [REPORT_SYSTEM, value]);
        }
    });
}

fn send_keyboard(handle: VHFHANDLE, keyboard: &KbdReport) {
    let mut report = [0u8; 9];
    report[0] = REPORT_KEYBOARD;
    report[1] = keyboard.mods;
    for (slot, key) in report[3..].iter_mut().zip(keyboard.keys.as_slice()) {
        *slot = *key;
    }
    send(handle, &mut report);
}

fn send(handle: VHFHANDLE, report: &mut [u8]) {
    let mut packet = _HID_XFER_PACKET {
        reportBuffer: report.as_mut_ptr(),
        reportBufferLen: report.len() as ULONG,
        reportId: report[0],
    };
    // VHF copies the report (no EvtVhfReadyForNextReadReport is registered), so a stack buffer is fine.
    let status = unsafe { VhfReadReportSubmit(handle, &mut packet) };
    if !nt_success(status) {
        println!("mkfilter: VhfReadReportSubmit failed {status:#010x}");
    }
}

/// EvtDriverDeviceAdd for the hub devnode.
///
/// # Safety
/// Called from EvtDriverDeviceAdd with the framework's `init`; PASSIVE_LEVEL.
pub unsafe fn device_add(mut init: PWDFDEVICE_INIT) -> NTSTATUS {
    let mut attributes = WDF_OBJECT_ATTRIBUTES {
        Size: size_of::<WDF_OBJECT_ATTRIBUTES>() as ULONG,
        ExecutionLevel: WdfExecutionLevelInheritFromParent,
        SynchronizationScope: WdfSynchronizationScopeInheritFromParent,
        EvtCleanupCallback: Some(evt_cleanup),
        ..Default::default()
    };
    let mut device: WDFDEVICE = null_mut();
    let status = unsafe {
        call_unsafe_wdf_function_binding!(WdfDeviceCreate, &mut init, &mut attributes, &mut device)
    };
    if !nt_success(status) {
        println!("mkfilter: hub WdfDeviceCreate failed {status:#010x}");
        return status;
    }

    let mut config: _VHF_CONFIG = unsafe { zeroed() };
    config.Size = size_of::<_VHF_CONFIG>() as ULONG;
    config.DeviceObject =
        unsafe { call_unsafe_wdf_function_binding!(WdfDeviceWdmGetDeviceObject, device) };
    config.ReportDescriptorLength = REPORT_DESCRIPTOR.len() as u16;
    config.ReportDescriptor = REPORT_DESCRIPTOR.as_ptr() as PUCHAR;

    let mut handle: VHFHANDLE = null_mut();
    let status = unsafe { VhfCreate(&mut config, &mut handle) };
    if !nt_success(status) {
        println!("mkfilter: VhfCreate failed {status:#010x}");
        return status;
    }
    let status = unsafe { VhfStart(handle) };
    if !nt_success(status) {
        println!("mkfilter: VhfStart failed {status:#010x}");
        unsafe { VhfDelete(handle, 1) };
        return status;
    }
    if VHF
        .compare_exchange(null_mut(), handle, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        println!("mkfilter: a second hub device is not supported");
        unsafe { VhfDelete(handle, 1) };
        return STATUS_DEVICE_ALREADY_ATTACHED;
    }
    println!("mkfilter: hub ready, virtual HID device started");
    STATUS_SUCCESS
}

/// Hub device cleanup (PASSIVE_LEVEL): detach the handle from submitters, then delete it.
extern "C" fn evt_cleanup(_object: WDFOBJECT) {
    let lock: WDFSPINLOCK = LOCK.load(Ordering::Acquire).cast();
    if lock.is_null() {
        return;
    }
    unsafe { call_unsafe_wdf_function_binding!(WdfSpinLockAcquire, lock) };
    let handle = VHF.swap(null_mut(), Ordering::AcqRel);
    unsafe { call_unsafe_wdf_function_binding!(WdfSpinLockRelease, lock) };
    if !handle.is_null() {
        unsafe { VhfDelete(handle, 1) };
        println!("mkfilter: hub removed");
    }
}
