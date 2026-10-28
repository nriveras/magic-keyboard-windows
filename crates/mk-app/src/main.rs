//! mk-app: tray companion for the mkfilter driver.
//!
//! - Tray icon with the keyboard's battery level; low-battery notification.
//! - Menu: top-row mode (media keys / F1–F12), start with Windows, quit.
//! - F16–F19 launch programs (Explorer, Calculator, Notepad, Task Manager).
//!
//! It talks to the driver only through the control interface (config and status). Key events
//! never reach user mode.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("mk-app only runs on Windows");
}

#[cfg(windows)]
mod icon;
#[cfg(windows)]
mod system;

#[cfg(windows)]
fn main() {
    app::run();
}

#[cfg(windows)]
mod app {
    use std::cell::RefCell;
    use std::ptr::{null, null_mut};
    use std::time::Duration;

    use mk_core::config::FnMode;
    use mk_core::proto::Status;
    use mk_hid::battery::{self, Battery};
    use mk_hid::control::Control;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, MOD_NOREPEAT, VK_F16, VK_F17, VK_F18, VK_F19,
    };
    use windows_sys::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_WARNING, NIM_ADD,
        NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
        DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, MessageBoxW, PostMessageW,
        PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow, SetTimer,
        TrackPopupMenu, TranslateMessage, HICON, MB_ICONERROR, MB_OK, MF_CHECKED, MF_GRAYED,
        MF_SEPARATOR, MF_STRING, MSG, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP,
        WM_DESTROY, WM_HOTKEY, WM_LBUTTONUP, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_TIMER, WNDCLASSW,
        WS_OVERLAPPED,
    };

    use crate::icon::{self, Level};
    use crate::system::{self, wide};

    const WM_TRAY: u32 = WM_APP + 1;
    /// Posted by the watcher thread when the driver status changed (or the hub came/went).
    const WM_DRIVER: u32 = WM_APP + 2;

    const TRAY_ID: u32 = 1;
    const BATTERY_TIMER: usize = 1;
    const BATTERY_POLL: u32 = 60_000;
    /// Warn once when the charge drops to this level; re-armed above `LOW_BATTERY + 5` or charging.
    const LOW_BATTERY: u8 = 20;

    const CMD_MEDIA_FIRST: usize = 10;
    const CMD_FKEYS_FIRST: usize = 11;
    const CMD_AUTOSTART: usize = 12;
    const CMD_QUIT: usize = 13;

    /// F16–F19 launchers (hotkey id = index + 1).
    const LAUNCHERS: [(u16, &str); 4] = [
        (VK_F16, "explorer.exe"),
        (VK_F17, "calc.exe"),
        (VK_F18, "notepad.exe"),
        (VK_F19, "taskmgr.exe"),
    ];

    struct App {
        hwnd: HWND,
        icon: HICON,
        status: Option<Status>,
        battery: Option<Battery>,
        low_warned: bool,
        taskbar_created: u32,
    }

    thread_local! {
        static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    }

    fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
        APP.with(|a| a.borrow_mut().as_mut().map(f))
    }

    pub fn run() {
        let Some(_instance) = system::single_instance() else {
            return; // already running
        };
        let class = wide("MagicKeyboardWindowsTray");
        let hwnd = unsafe {
            let hinstance = GetModuleHandleW(null());
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance,
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassW(&wc);
            // A hidden top-level window: message-only windows can't own a tray menu properly.
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                hinstance,
                null(),
            )
        };
        if hwnd.is_null() {
            return;
        }
        let taskbar_created = unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) };
        APP.with(|a| {
            *a.borrow_mut() = Some(App {
                hwnd,
                icon: null_mut(),
                status: None,
                battery: None,
                low_warned: false,
                taskbar_created,
            })
        });

        refresh_driver();
        refresh_battery();
        add_tray_icon();
        unsafe { SetTimer(hwnd, BATTERY_TIMER, BATTERY_POLL, None) };
        for (i, (vk, _)) in LAUNCHERS.iter().enumerate() {
            // Fails silently if another program owns the key; the rest still work.
            unsafe { RegisterHotKey(hwnd, i as i32 + 1, MOD_NOREPEAT, u32::from(*vk)) };
        }
        spawn_watcher(hwnd as usize);

        let mut msg: MSG = unsafe { std::mem::zeroed() };
        while unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) } > 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    /// Waits for driver status changes on its own handle and pokes the UI thread.
    fn spawn_watcher(hwnd: usize) {
        std::thread::spawn(move || loop {
            match Control::open() {
                Ok(control) => {
                    let mut generation = control.status().map(|s| s.generation).unwrap_or(0);
                    unsafe { PostMessageW(hwnd as HWND, WM_DRIVER, 0, 0) };
                    while let Ok(status) = control.wait_status(generation) {
                        generation = status.generation;
                        unsafe { PostMessageW(hwnd as HWND, WM_DRIVER, 0, 0) };
                    }
                    // Hub removed or driver reinstalled.
                    unsafe { PostMessageW(hwnd as HWND, WM_DRIVER, 0, 0) };
                }
                Err(_) => std::thread::sleep(Duration::from_secs(5)),
            }
        });
    }

    fn refresh_driver() {
        let status = Control::open().and_then(|c| c.status()).ok();
        with_app(|a| a.status = status);
    }

    fn refresh_battery() {
        let reading = battery::collections()
            .ok()
            .and_then(|cols| cols.into_iter().find_map(|c| battery::read(&c).ok()));
        let warn = with_app(|a| {
            a.battery = reading;
            match reading {
                Some(b) if b.charging || b.percent > LOW_BATTERY + 5 => {
                    a.low_warned = false;
                    false
                }
                Some(b) if b.percent <= LOW_BATTERY && !a.low_warned => {
                    a.low_warned = true;
                    true
                }
                _ => false,
            }
        })
        .unwrap_or(false);
        update_tray_icon();
        if let (true, Some(b)) = (warn, reading) {
            notify(
                "Magic Keyboard battery low",
                &format!("{}% left. Connect the keyboard to charge it.", b.percent),
            );
        }
    }

    fn battery_text(a: &App) -> String {
        match a.battery {
            Some(b) if b.fully_charged => format!("{}% (charged)", b.percent),
            Some(b) if b.charging => format!("{}% (charging)", b.percent),
            Some(b) => format!("{}%", b.percent),
            None => "battery unknown".into(),
        }
    }

    fn driver_text(a: &App) -> &'static str {
        match &a.status {
            None => "driver not installed",
            Some(s) if s.keyboards == 0 => "no keyboard",
            Some(s) if !s.virtual_device_ready => "media keys unavailable",
            Some(_) => "active",
        }
    }

    fn tooltip(a: &App) -> String {
        format!("Magic Keyboard: {}, {}", battery_text(a), driver_text(a))
    }

    fn tray_data(a: &App) -> NOTIFYICONDATAW {
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = a.hwnd;
        nid.uID = TRAY_ID;
        nid
    }

    fn copy_into(dst: &mut [u16], s: &str) {
        let src: Vec<u16> = s.encode_utf16().take(dst.len() - 1).collect();
        dst[..src.len()].copy_from_slice(&src);
        dst[src.len()] = 0;
    }

    fn current_level(a: &App) -> Level {
        match a.battery {
            Some(b) => Level::Percent {
                percent: b.percent,
                charging: b.charging && !b.fully_charged,
            },
            None => Level::Unknown,
        }
    }

    fn add_tray_icon() {
        with_app(|a| {
            let new_icon = icon::render(current_level(a), system::light_taskbar());
            if !a.icon.is_null() {
                unsafe { DestroyIcon(a.icon) };
            }
            a.icon = new_icon;
            let mut nid = tray_data(a);
            nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            nid.uCallbackMessage = WM_TRAY;
            nid.hIcon = a.icon;
            copy_into(&mut nid.szTip, &tooltip(a));
            unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
        });
    }

    fn update_tray_icon() {
        with_app(|a| {
            let new_icon = icon::render(current_level(a), system::light_taskbar());
            let mut nid = tray_data(a);
            nid.uFlags = NIF_ICON | NIF_TIP;
            nid.hIcon = new_icon;
            copy_into(&mut nid.szTip, &tooltip(a));
            unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
            if !a.icon.is_null() {
                unsafe { DestroyIcon(a.icon) };
            }
            a.icon = new_icon;
        });
    }

    fn notify(title: &str, text: &str) {
        with_app(|a| {
            let mut nid = tray_data(a);
            nid.uFlags = NIF_INFO;
            nid.dwInfoFlags = NIIF_WARNING;
            copy_into(&mut nid.szInfoTitle, title);
            copy_into(&mut nid.szInfo, text);
            unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
        });
    }

    fn error_box(hwnd: HWND, text: &str) {
        let (text, title) = (wide(text), wide("Magic Keyboard"));
        unsafe { MessageBoxW(hwnd, text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR) };
    }

    fn set_fn_mode(hwnd: HWND, mode: FnMode) {
        let result = Control::open().and_then(|c| {
            let mut config = c.config()?;
            config.fn_mode = mode;
            c.set_config(&config)
        });
        if let Err(e) = result {
            error_box(hwnd, &format!("Could not change the top-row mode:\n{e}"));
        }
        refresh_driver();
    }

    fn show_menu(hwnd: HWND) {
        let Some((status_line, battery_line, fn_mode, driver_ok)) = with_app(|a| {
            let fn_mode = a.status.as_ref().and_then(|s| FnMode::from_u8(s.fn_mode));
            (
                format!("Driver: {}", driver_text(a)),
                format!("Battery: {}", battery_text(a)),
                fn_mode,
                a.status.is_some(),
            )
        }) else {
            return;
        };

        unsafe {
            let menu = CreatePopupMenu();
            let item = |flags: u32, id: usize, text: &str| {
                let w = wide(text);
                AppendMenuW(menu, flags, id, w.as_ptr());
            };
            item(MF_STRING | MF_GRAYED, 0, &battery_line);
            item(MF_STRING | MF_GRAYED, 0, &status_line);
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            let enabled = if driver_ok { 0 } else { MF_GRAYED };
            let checked = |m: FnMode| if fn_mode == Some(m) { MF_CHECKED } else { 0 };
            item(
                MF_STRING | enabled | checked(FnMode::MediaFirst),
                CMD_MEDIA_FIRST,
                "Top row: media keys (Fn for F1–F12)",
            );
            item(
                MF_STRING | enabled | checked(FnMode::FKeysFirst),
                CMD_FKEYS_FIRST,
                "Top row: F1–F12 (Fn for media keys)",
            );
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            let autostart = if system::autostart_enabled() {
                MF_CHECKED
            } else {
                0
            };
            item(MF_STRING | autostart, CMD_AUTOSTART, "Start with Windows");
            item(MF_STRING, CMD_QUIT, "Quit");

            let mut pt = POINT { x: 0, y: 0 };
            GetCursorPos(&mut pt);
            // Required so the menu closes when the user clicks elsewhere.
            SetForegroundWindow(hwnd);
            let cmd = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                pt.x,
                pt.y,
                0,
                hwnd,
                null(),
            ) as usize;
            DestroyMenu(menu);

            match cmd {
                CMD_MEDIA_FIRST => set_fn_mode(hwnd, FnMode::MediaFirst),
                CMD_FKEYS_FIRST => set_fn_mode(hwnd, FnMode::FKeysFirst),
                CMD_AUTOSTART => {
                    if let Err(e) = system::set_autostart(!system::autostart_enabled()) {
                        error_box(hwnd, &format!("Could not change autostart:\n{e}"));
                    }
                }
                CMD_QUIT => {
                    DestroyWindow(hwnd);
                }
                _ => {}
            }
        }
    }

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_TRAY => {
                let event = lparam as u32;
                if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
                    show_menu(hwnd);
                }
                0
            }
            WM_DRIVER => {
                refresh_driver();
                update_tray_icon();
                0
            }
            WM_TIMER if wparam == BATTERY_TIMER => {
                refresh_battery();
                0
            }
            WM_HOTKEY => {
                if let Some((_, target)) = LAUNCHERS.get(wparam.wrapping_sub(1)) {
                    system::launch(target);
                }
                0
            }
            WM_SETTINGCHANGE => {
                // Taskbar theme may have changed.
                update_tray_icon();
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_DESTROY => {
                with_app(|a| {
                    let nid = tray_data(a);
                    unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
                });
                unsafe { PostQuitMessage(0) };
                0
            }
            _ if with_app(|a| a.taskbar_created == msg).unwrap_or(false) => {
                // Explorer restarted: the icon is gone and must be added again.
                add_tray_icon();
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
