//! Client for the driver's control interface (the hub device, `mk_core::proto`). Carries
//! configuration and status only.

use std::io;

use mk_core::config::WIRE_SIZE;
use mk_core::proto::{self, Status, STATUS_SIZE};
use mk_core::Config;
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows_sys::Win32::System::IO::DeviceIoControl;

use crate::{ffi, open_raw};

/// An open handle to the hub's control interface. Synchronous: a pending [`Control::wait_status`]
/// blocks other calls on the same handle, so use a separate `Control` for waiting.
pub struct Control {
    handle: HANDLE,
}

// SAFETY: the handle may be used from any thread.
unsafe impl Send for Control {}

impl Drop for Control {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
    }
}

fn interface_guid() -> GUID {
    let (data1, data2, data3, data4) = proto::INTERFACE_GUID;
    GUID {
        data1,
        data2,
        data3,
        data4,
    }
}

impl Control {
    /// Opens the first present hub interface. `NotFound` if the driver's hub isn't installed.
    pub fn open() -> io::Result<Control> {
        let path = ffi::interface_paths(&interface_guid())?
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "mkfilter hub not present"))?;
        Ok(Control {
            handle: open_raw(&path, GENERIC_READ | GENERIC_WRITE)?,
        })
    }

    fn ioctl(&self, code: u32, input: &[u8], output: &mut [u8]) -> io::Result<usize> {
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                self.handle,
                code,
                input.as_ptr().cast(),
                input.len() as u32,
                output.as_mut_ptr().cast(),
                output.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(returned as usize)
        }
    }

    fn read_status(&self, code: u32, input: &[u8]) -> io::Result<Status> {
        let mut out = [0u8; STATUS_SIZE];
        let n = self.ioctl(code, input, &mut out)?;
        Status::from_bytes(&out[..n]).ok_or_else(|| io::Error::other("short status reply"))
    }

    pub fn status(&self) -> io::Result<Status> {
        self.read_status(proto::IOCTL_GET_STATUS, &[])
    }

    /// Blocks until the driver's status generation differs from `last_seen`, then returns it.
    pub fn wait_status(&self, last_seen: u32) -> io::Result<Status> {
        self.read_status(proto::IOCTL_WAIT_STATUS, &last_seen.to_le_bytes())
    }

    /// The configuration the keyboards currently run with.
    pub fn config(&self) -> io::Result<Config> {
        let mut out = [0u8; WIRE_SIZE];
        let n = self.ioctl(proto::IOCTL_GET_CONFIG, &[], &mut out)?;
        Config::from_wire(&out[..n])
            .map_err(|e| io::Error::other(format!("bad config reply: {e:?}")))
    }

    /// Applies `config` live and persists it in the driver's registry key.
    pub fn set_config(&self, config: &Config) -> io::Result<()> {
        self.ioctl(proto::IOCTL_SET_CONFIG, &config.to_wire(), &mut [])?;
        Ok(())
    }
}
