//! mkfilter: KMDF lower filter for the keyboard collection of Apple Magic Keyboards
//! (`HID\VID_05AC&PID_xxxx&MI_01&Col01`).
//!
//! It sits between kbdhid and the hidclass PDO, so every input report kbdhid reads passes through
//! our IRP_MJ_READ completion. M3 is pass-through only: requests are forwarded unchanged and each
//! input report is logged with `DbgPrint` (view with DebugView, "Capture Kernel" + verbose output).

#![no_std]

use core::mem::size_of;
use core::ptr::null_mut;

use wdk::println;
use wdk_sys::{
    _WDF_EXECUTION_LEVEL::WdfExecutionLevelInheritFromParent,
    _WDF_IO_QUEUE_DISPATCH_TYPE::WdfIoQueueDispatchParallel,
    _WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeInheritFromParent,
    _WDF_TRI_STATE::WdfUseDefault,
    call_unsafe_wdf_function_binding, NTSTATUS, PCUNICODE_STRING, PDRIVER_OBJECT, PVOID,
    PWDFDEVICE_INIT, PWDF_REQUEST_COMPLETION_PARAMS, STATUS_SUCCESS, ULONG, WDFCONTEXT, WDFDEVICE,
    WDFDRIVER, WDFIOTARGET, WDFOBJECT, WDFQUEUE, WDFREQUEST, WDF_DRIVER_CONFIG, WDF_IO_QUEUE_CONFIG,
    WDF_NO_HANDLE, WDF_NO_OBJECT_ATTRIBUTES, WDF_OBJECT_ATTRIBUTES, WDF_OBJECT_CONTEXT_TYPE_INFO,
};

#[cfg(not(test))]
extern crate wdk_panic;

#[cfg(not(test))]
use wdk_alloc::WdkAllocator;

#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WdkAllocator = WdkAllocator;

/// Per-device state (WDF device context).
#[repr(C)]
struct DeviceContext {
    target: WDFIOTARGET,
}

/// `WDF_DECLARE_CONTEXT_TYPE` equivalent. The wrapper only exists to make the static `Sync`.
#[repr(transparent)]
struct ContextTypeInfo(WDF_OBJECT_CONTEXT_TYPE_INFO);

// SAFETY: the type info is immutable and only read by the framework.
unsafe impl Sync for ContextTypeInfo {}

static DEVICE_CONTEXT_TYPE: ContextTypeInfo = ContextTypeInfo(WDF_OBJECT_CONTEXT_TYPE_INFO {
    Size: size_of::<WDF_OBJECT_CONTEXT_TYPE_INFO>() as ULONG,
    ContextName: c"DeviceContext".as_ptr(),
    ContextSize: size_of::<DeviceContext>(),
    UniqueType: &DEVICE_CONTEXT_TYPE.0,
    EvtDriverGetUniqueContextType: None,
});

fn nt_success(status: NTSTATUS) -> bool {
    status >= 0
}

/// # Safety
/// `device` must be a WDFDEVICE created with `DEVICE_CONTEXT_TYPE`.
unsafe fn device_context(device: WDFDEVICE) -> *mut DeviceContext {
    unsafe {
        call_unsafe_wdf_function_binding!(
            WdfObjectGetTypedContextWorker,
            device as WDFOBJECT,
            DEVICE_CONTEXT_TYPE.0.UniqueType
        )
    }
    .cast()
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
    let status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfDriverCreate,
            driver,
            registry_path,
            WDF_NO_OBJECT_ATTRIBUTES,
            &mut config,
            WDF_NO_HANDLE.cast::<WDFDRIVER>()
        )
    };
    println!("mkfilter: DriverEntry {status:#010x}");
    status
}

extern "C" fn evt_device_add(_driver: WDFDRIVER, mut init: PWDFDEVICE_INIT) -> NTSTATUS {
    unsafe { call_unsafe_wdf_function_binding!(WdfFdoInitSetFilter, init) };

    let mut attributes = WDF_OBJECT_ATTRIBUTES {
        Size: size_of::<WDF_OBJECT_ATTRIBUTES>() as ULONG,
        ExecutionLevel: WdfExecutionLevelInheritFromParent,
        SynchronizationScope: WdfSynchronizationScopeInheritFromParent,
        ContextTypeInfo: &DEVICE_CONTEXT_TYPE.0,
        ..Default::default()
    };
    let mut device: WDFDEVICE = null_mut();
    let status = unsafe {
        call_unsafe_wdf_function_binding!(WdfDeviceCreate, &mut init, &mut attributes, &mut device)
    };
    if !nt_success(status) {
        println!("mkfilter: WdfDeviceCreate failed {status:#010x}");
        return status;
    }

    unsafe {
        (*device_context(device)).target =
            call_unsafe_wdf_function_binding!(WdfDeviceGetIoTarget, device);
    }

    // Default parallel queue with only a read handler: as a filter, the framework forwards every
    // other request type to the lower driver untouched.
    let mut queue_config = WDF_IO_QUEUE_CONFIG {
        Size: size_of::<WDF_IO_QUEUE_CONFIG>() as ULONG,
        DispatchType: WdfIoQueueDispatchParallel,
        PowerManaged: WdfUseDefault,
        DefaultQueue: 1,
        EvtIoRead: Some(evt_io_read),
        ..Default::default()
    };
    queue_config.Settings.Parallel.NumberOfPresentedRequests = ULONG::MAX;
    let status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfIoQueueCreate,
            device,
            &mut queue_config,
            WDF_NO_OBJECT_ATTRIBUTES,
            null_mut()
        )
    };
    println!("mkfilter: device added {status:#010x}");
    status
}

extern "C" fn evt_io_read(queue: WDFQUEUE, request: WDFREQUEST, _length: usize) {
    unsafe {
        let device = call_unsafe_wdf_function_binding!(WdfIoQueueGetDevice, queue);
        let ctx = device_context(device);
        call_unsafe_wdf_function_binding!(WdfRequestFormatRequestUsingCurrentType, request);
        call_unsafe_wdf_function_binding!(
            WdfRequestSetCompletionRoutine,
            request,
            Some(read_complete),
            ctx as WDFCONTEXT
        );
        if call_unsafe_wdf_function_binding!(WdfRequestSend, request, (*ctx).target, null_mut())
            == 0
        {
            let status = call_unsafe_wdf_function_binding!(WdfRequestGetStatus, request);
            println!("mkfilter: WdfRequestSend failed {status:#010x}");
            call_unsafe_wdf_function_binding!(WdfRequestComplete, request, status);
        }
    }
}

/// Read completion, at IRQL <= DISPATCH_LEVEL.
extern "C" fn read_complete(
    request: WDFREQUEST,
    _target: WDFIOTARGET,
    params: PWDF_REQUEST_COMPLETION_PARAMS,
    _context: WDFCONTEXT,
) {
    unsafe {
        let status = (*params).IoStatus.__bindgen_anon_1.Status;
        let information = (*params).IoStatus.Information;
        if nt_success(status) && information > 0 {
            let mut buffer: PVOID = null_mut();
            let mut length = 0usize;
            let st = call_unsafe_wdf_function_binding!(
                WdfRequestRetrieveOutputBuffer,
                request,
                1,
                &mut buffer,
                &mut length
            );
            if st == STATUS_SUCCESS {
                let report =
                    core::slice::from_raw_parts(buffer.cast::<u8>(), length.min(information as usize));
                // Debug builds only: logging reports exposes keystrokes to anyone capturing kernel
                // debug output. Release builds never log report contents.
                #[cfg(debug_assertions)]
                println!("mkfilter: report {report:02X?}");
                #[cfg(not(debug_assertions))]
                let _ = report;
            }
        }
        call_unsafe_wdf_function_binding!(
            WdfRequestCompleteWithInformation,
            request,
            status,
            information
        );
    }
}
