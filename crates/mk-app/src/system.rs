//! Small Windows helpers: single instance, autostart (HKCU Run), taskbar theme, launching programs.

use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_DWORD,
    RRF_RT_REG_SZ,
};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, PostMessageW, SW_SHOWNORMAL, WM_APP,
};

use crate::command;

/// Window class of the tray's hidden window.
pub const TRAY_CLASS: &str = "MagicKeyboardWindowsTray";
/// Posted to the tray window when app.toml changed.
pub const WM_RELOAD: u32 = WM_APP + 3;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Creates a named mutex held for the process lifetime. `None` if another process holds it.
pub fn named_mutex(name: &str) -> Option<HANDLE> {
    let name = wide(name);
    let handle = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
    if handle.is_null() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        None
    } else {
        Some(handle)
    }
}

/// Tells a running tray to reload app.toml.
pub fn notify_tray_reload() {
    let class = wide(TRAY_CLASS);
    let hwnd = unsafe { FindWindowW(class.as_ptr(), null()) };
    if !hwnd.is_null() {
        unsafe { PostMessageW(hwnd, WM_RELOAD, 0, 0) };
    }
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "MagicKeyboardWindows";

pub fn autostart_enabled() -> bool {
    let (key, value) = (wide(RUN_KEY), wide(RUN_VALUE));
    let mut size = 0u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            null_mut(),
            &mut size,
        )
    };
    status == 0
}

pub fn set_autostart(enable: bool) -> std::io::Result<()> {
    let (key, value) = (wide(RUN_KEY), wide(RUN_VALUE));
    let status = if enable {
        let exe = std::env::current_exe()?;
        let command = wide(&format!("\"{}\"", exe.display()));
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * 2) as u32,
            )
        }
    } else {
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) }
    };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(status as i32))
    }
}

/// True if the taskbar uses the light theme (then the icon needs a dark outline).
pub fn light_taskbar() -> bool {
    let key = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let value = wide("SystemUsesLightTheme");
    let mut data = 0u32;
    let mut size = 4u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
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

/// The Windows accent color as (r, g, b), if set.
pub fn accent_color() -> Option<(u8, u8, u8)> {
    let key = wide("Software\\Microsoft\\Windows\\DWM");
    let value = wide("AccentColor");
    let mut data = 0u32;
    let mut size = 4u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            (&raw mut data).cast(),
            &mut size,
        )
    };
    // Stored as 0xAABBGGRR.
    (status == 0).then_some((data as u8, (data >> 8) as u8, (data >> 16) as u8))
}

/// Opens a program (with optional arguments, see [`command::split`]) or document the way
/// Explorer would. The calling thread must have initialized COM.
pub fn launch(target: &str) -> std::io::Result<()> {
    let (file, args) = command::split(target);
    let (verb, file) = (wide("open"), wide(file));
    let args = args.map(wide);
    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            args.as_ref().map_or(null(), |a| a.as_ptr()),
            null(),
            SW_SHOWNORMAL,
        )
    };
    // Values above 32 mean success.
    if result as isize > 32 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Shows the Windows "Open" dialog for a program or document. `None` if cancelled.
pub fn browse_file() -> Option<String> {
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OPENFILENAMEW,
    };
    let filter: Vec<u16> = "Programs\0*.exe;*.lnk;*.bat;*.cmd\0All files\0*.*\0\0"
        .encode_utf16()
        .collect();
    let mut file = vec![0u16; 1024];
    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = file.as_mut_ptr();
    ofn.nMaxFile = file.len() as u32;
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR;
    if unsafe { GetOpenFileNameW(&mut ofn) } == 0 {
        return None;
    }
    let len = file.iter().position(|c| *c == 0).unwrap_or(file.len());
    Some(String::from_utf16_lossy(&file[..len]))
}
