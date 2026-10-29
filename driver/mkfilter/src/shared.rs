//! Driver-wide state shared by the hub and filter devnodes (one driver image serves both), guarded
//! by a single spinlock created in DriverEntry.
//!
//! Lock order: a filter's device lock may be held while taking this lock, never the reverse.

use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicPtr, Ordering};

use mk_core::proto::{PROTOCOL_VERSION, Status};
use mk_core::sync::SharedConfig;
use mk_core::{Config, model};
use wdk_sys::{VHFHANDLE, WDFOBJECT, WDFQUEUE, WDFSPINLOCK, call_unsafe_wdf_function_binding};

use crate::control;

pub struct Shared {
    /// Started VHF device, or null.
    pub vhf: VHFHANDLE,
    /// The hub WDFDEVICE that owns `vhf`, or null.
    pub hub: WDFOBJECT,
    /// Manual queue holding pending `IOCTL_WAIT_STATUS` requests, or null.
    pub wait_queue: WDFQUEUE,
    /// User configuration (registry or `IOCTL_SET_CONFIG`, `None` = model defaults) and its
    /// generation; filters re-apply it before their next report when the generation moved.
    pub config: SharedConfig,
    /// Keyboards currently filtered with remapping on.
    pub keyboards: u8,
    pub last_pid: u16,
    pub status_gen: u32,
}

impl Shared {
    /// Defaults for the most recently attached model.
    pub fn model_defaults(&self) -> Config {
        model::find(model::APPLE_USB_VID, self.last_pid)
            .map(Config::default_for)
            .unwrap_or_default()
    }

    /// The configuration the keyboards run with.
    pub fn effective_config(&self) -> Config {
        self.config.config.unwrap_or_else(|| self.model_defaults())
    }

    pub fn status(&self) -> Status {
        Status {
            protocol_version: PROTOCOL_VERSION,
            keyboards: self.keyboards,
            fn_mode: self.effective_config().fn_mode as u8,
            virtual_device_ready: !self.vhf.is_null(),
            last_pid: self.last_pid,
            generation: self.status_gen,
        }
    }

    /// Records a status change and completes every pending `IOCTL_WAIT_STATUS` request.
    pub fn status_changed(&mut self) {
        self.status_gen = self.status_gen.wrapping_add(1);
        if !self.wait_queue.is_null() {
            control::complete_waiters(self.wait_queue, &self.status());
        }
    }
}

struct StateCell(UnsafeCell<Shared>);

// SAFETY: every access goes through `with`, which holds the driver-wide spinlock.
unsafe impl Sync for StateCell {}

static STATE: StateCell = StateCell(UnsafeCell::new(Shared {
    vhf: null_mut(),
    hub: null_mut(),
    wait_queue: null_mut(),
    config: SharedConfig::new(),
    keyboards: 0,
    last_pid: 0,
    status_gen: 0,
}));

static LOCK: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

/// Stores the driver-wide lock. Called once from DriverEntry.
pub fn set_lock(lock: WDFSPINLOCK) {
    LOCK.store(lock.cast(), Ordering::Release);
}

/// Runs `f` on the shared state under the driver-wide lock (raises to DISPATCH_LEVEL).
/// Returns `None` if DriverEntry could not create the lock.
pub fn with<R>(f: impl FnOnce(&mut Shared) -> R) -> Option<R> {
    let lock: WDFSPINLOCK = LOCK.load(Ordering::Acquire).cast();
    if lock.is_null() {
        return None;
    }
    unsafe { call_unsafe_wdf_function_binding!(WdfSpinLockAcquire, lock) };
    // SAFETY: the lock serializes all access to STATE.
    let result = f(unsafe { &mut *STATE.0.get() });
    unsafe { call_unsafe_wdf_function_binding!(WdfSpinLockRelease, lock) };
    Some(result)
}
