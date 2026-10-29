//! Hub control interface for mk-app: device interface `proto::INTERFACE_GUID` with the config and
//! status IOCTLs from `mk_core::proto`. It carries configuration and status only, never key events.
//! Access is limited by the hub's SDDL in mkhub.inf (SYSTEM/Administrators full, interactive users
//! read/write).

use core::mem::size_of;
use core::ptr::null_mut;

use mk_core::Config;
use mk_core::config::WIRE_SIZE;
use mk_core::proto::{self, STATUS_SIZE, Status};
use wdk::println;
use wdk_sys::{
    _WDF_EXECUTION_LEVEL::{WdfExecutionLevelInheritFromParent, WdfExecutionLevelPassive},
    _WDF_IO_QUEUE_DISPATCH_TYPE::{WdfIoQueueDispatchManual, WdfIoQueueDispatchParallel},
    _WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeInheritFromParent,
    _WDF_TRI_STATE::WdfFalse,
    GUID, NTSTATUS, PVOID, STATUS_DEVICE_NOT_READY, STATUS_INVALID_DEVICE_REQUEST,
    STATUS_INVALID_PARAMETER, STATUS_SUCCESS, ULONG, WDF_EXECUTION_LEVEL, WDF_IO_QUEUE_CONFIG,
    WDF_OBJECT_ATTRIBUTES, WDFDEVICE, WDFOBJECT, WDFQUEUE, WDFREQUEST,
    call_unsafe_wdf_function_binding,
};

use crate::{nt_success, registry, shared};

fn queue_attributes(level: WDF_EXECUTION_LEVEL) -> WDF_OBJECT_ATTRIBUTES {
    WDF_OBJECT_ATTRIBUTES {
        Size: size_of::<WDF_OBJECT_ATTRIBUTES>() as ULONG,
        ExecutionLevel: level,
        SynchronizationScope: WdfSynchronizationScopeInheritFromParent,
        ..Default::default()
    }
}

/// Creates the device interface, the IOCTL queue and the wait queue on the hub device.
///
/// # Safety
/// `device` is the hub WDFDEVICE, before it starts; PASSIVE_LEVEL.
pub unsafe fn create(device: WDFDEVICE) -> NTSTATUS {
    let (data1, data2, data3, data4) = proto::INTERFACE_GUID;
    let guid = GUID {
        Data1: data1,
        Data2: data2,
        Data3: data3,
        Data4: data4,
    };
    let status = unsafe {
        call_unsafe_wdf_function_binding!(WdfDeviceCreateDeviceInterface, device, &guid, null_mut())
    };
    if !nt_success(status) {
        return status;
    }

    // IOCTLs run at PASSIVE_LEVEL because SET_CONFIG writes the registry.
    let mut config = WDF_IO_QUEUE_CONFIG {
        Size: size_of::<WDF_IO_QUEUE_CONFIG>() as ULONG,
        DispatchType: WdfIoQueueDispatchParallel,
        PowerManaged: WdfFalse,
        DefaultQueue: 1,
        EvtIoDeviceControl: Some(evt_io_device_control),
        ..Default::default()
    };
    config.Settings.Parallel.NumberOfPresentedRequests = ULONG::MAX;
    let mut attributes = queue_attributes(WdfExecutionLevelPassive);
    let status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfIoQueueCreate,
            device,
            &mut config,
            &mut attributes,
            null_mut()
        )
    };
    if !nt_success(status) {
        return status;
    }

    let mut wait_config = WDF_IO_QUEUE_CONFIG {
        Size: size_of::<WDF_IO_QUEUE_CONFIG>() as ULONG,
        DispatchType: WdfIoQueueDispatchManual,
        PowerManaged: WdfFalse,
        ..Default::default()
    };
    let mut wait_attributes = WDF_OBJECT_ATTRIBUTES {
        EvtCleanupCallback: Some(evt_wait_queue_cleanup),
        ..queue_attributes(WdfExecutionLevelInheritFromParent)
    };
    let mut wait_queue: WDFQUEUE = null_mut();
    let status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfIoQueueCreate,
            device,
            &mut wait_config,
            &mut wait_attributes,
            &mut wait_queue
        )
    };
    if nt_success(status) {
        shared::with(|s| s.wait_queue = wait_queue);
    }
    status
}

extern "C" fn evt_wait_queue_cleanup(object: WDFOBJECT) {
    shared::with(|s| {
        if s.wait_queue == object as WDFQUEUE {
            s.wait_queue = null_mut();
        }
    });
}

/// Completes every request in `queue` with `status`. IRQL <= DISPATCH_LEVEL.
pub fn complete_waiters(queue: WDFQUEUE, status: &Status) {
    let bytes = status.to_bytes();
    loop {
        let mut request: WDFREQUEST = null_mut();
        let st = unsafe {
            call_unsafe_wdf_function_binding!(WdfIoQueueRetrieveNextRequest, queue, &mut request)
        };
        if !nt_success(st) {
            break;
        }
        complete_with(request, &bytes);
    }
}

/// Copies `bytes` to the request's output buffer and completes it.
fn complete_with(request: WDFREQUEST, bytes: &[u8]) {
    let mut buffer: PVOID = null_mut();
    let mut length = 0usize;
    let status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfRequestRetrieveOutputBuffer,
            request,
            bytes.len(),
            &mut buffer,
            &mut length
        )
    };
    if !nt_success(status) {
        unsafe { call_unsafe_wdf_function_binding!(WdfRequestComplete, request, status) };
        return;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast::<u8>(), bytes.len());
        call_unsafe_wdf_function_binding!(
            WdfRequestCompleteWithInformation,
            request,
            STATUS_SUCCESS,
            bytes.len() as u64
        );
    }
}

fn complete(request: WDFREQUEST, status: NTSTATUS) {
    unsafe { call_unsafe_wdf_function_binding!(WdfRequestComplete, request, status) };
}

/// Returns the request's input buffer if it holds at least `min` bytes.
fn input(request: WDFREQUEST, min: usize) -> Result<&'static [u8], NTSTATUS> {
    let mut buffer: PVOID = null_mut();
    let mut length = 0usize;
    let status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfRequestRetrieveInputBuffer,
            request,
            min,
            &mut buffer,
            &mut length
        )
    };
    if !nt_success(status) {
        return Err(status);
    }
    // SAFETY: the framework keeps the buffer valid until the request completes; callers finish
    // reading before completing it.
    Ok(unsafe { core::slice::from_raw_parts(buffer.cast::<u8>(), length) })
}

/// IOCTL dispatch (PASSIVE_LEVEL, see the queue's execution level).
extern "C" fn evt_io_device_control(
    queue: WDFQUEUE,
    request: WDFREQUEST,
    _output_length: usize,
    _input_length: usize,
    code: ULONG,
) {
    match code {
        proto::IOCTL_GET_STATUS => {
            let status = shared::with(|s| s.status()).unwrap_or_default();
            complete_with(request, &status.to_bytes());
        }
        proto::IOCTL_GET_CONFIG => match shared::with(|s| s.effective_config()) {
            Some(config) => complete_with(request, &config.to_wire()),
            None => complete(request, STATUS_INVALID_DEVICE_REQUEST),
        },
        proto::IOCTL_SET_CONFIG => {
            let status = set_config(queue, request);
            complete(request, status);
        }
        proto::IOCTL_WAIT_STATUS => wait_status(request),
        _ => complete(request, STATUS_INVALID_DEVICE_REQUEST),
    }
}

fn set_config(queue: WDFQUEUE, request: WDFREQUEST) -> NTSTATUS {
    let config = match input(request, WIRE_SIZE).map(Config::from_wire) {
        Ok(Ok(config)) => config,
        Ok(Err(_)) => return STATUS_INVALID_PARAMETER,
        Err(status) => return status,
    };
    unsafe {
        let device = call_unsafe_wdf_function_binding!(WdfIoQueueGetDevice, queue);
        let driver = call_unsafe_wdf_function_binding!(WdfDeviceGetDriver, device);
        let st = registry::write_config(driver, &config);
        if !nt_success(st) {
            // Still applied live; it just won't survive a reboot.
            println!("mkfilter: saving config failed {st:#010x}");
        }
    }
    shared::with(|s| {
        s.config.set(Some(config));
        s.status_changed();
    });
    STATUS_SUCCESS
}

/// Completes at once if the caller's generation is stale, otherwise parks the request until the
/// next status change. Parked requests are cancelled by the framework when the handle closes.
fn wait_status(request: WDFREQUEST) {
    let last_seen = match input(request, 4) {
        Ok(b) => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        Err(status) => return complete(request, status),
    };
    enum Outcome {
        Now([u8; STATUS_SIZE]),
        Parked,
        Failed(NTSTATUS),
    }
    // Check and park under one lock hold so a concurrent change cannot slip in between.
    let outcome = shared::with(|s| {
        if s.status_gen != last_seen {
            return Outcome::Now(s.status().to_bytes());
        }
        if s.wait_queue.is_null() {
            // Hub going away: an unchanged status now would make the caller spin.
            return Outcome::Failed(STATUS_DEVICE_NOT_READY);
        }
        let st = unsafe {
            call_unsafe_wdf_function_binding!(WdfRequestForwardToIoQueue, request, s.wait_queue)
        };
        if nt_success(st) {
            Outcome::Parked
        } else {
            Outcome::Failed(st)
        }
    });
    match outcome {
        Some(Outcome::Now(bytes)) => complete_with(request, &bytes),
        Some(Outcome::Parked) => {}
        Some(Outcome::Failed(status)) => complete(request, status),
        None => complete(request, STATUS_INVALID_DEVICE_REQUEST),
    }
}
