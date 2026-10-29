//! mk-app: tray companion for the mkfilter driver.
//!
//! - Tray icon with the battery level of every connected keyboard; low-battery notification.
//! - Menu: top-row mode (media keys / F1–F12), settings, start with Windows, quit.
//! - F16–F19 launch the programs configured in app.toml.
//! - `mk-app settings` opens the settings window (separate process, see `settings`).
//!
//! It talks to the driver only through the control interface (config and status). Key events
//! never reach user mode.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("mk-app only runs on Windows");
}

mod appconfig;
#[cfg(windows)]
mod devices;
#[cfg(windows)]
mod health;
#[cfg(windows)]
mod icon;
#[cfg_attr(not(windows), allow(dead_code))]
mod lowbattery;
#[cfg(windows)]
mod settings;
#[cfg_attr(not(windows), allow(dead_code))]
mod slot;
#[cfg(windows)]
mod system;
#[cfg_attr(not(windows), allow(dead_code))]
mod watch;

#[cfg(windows)]
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("settings") => settings::run(),
        _ => app::run(),
    }
}

#[cfg(windows)]
mod app {
    use std::cell::RefCell;
    use std::io;
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    use mk_core::config::FnMode;
    use mk_core::proto::Status;
    use mk_hid::battery::Battery;
    use mk_hid::control::Control;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT, VK_F16, VK_F17, VK_F18, VK_F19,
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

    use crate::appconfig::{self, AppConfig};
    use crate::devices;
    use crate::health;
    use crate::icon::{self, Level};
    use crate::lowbattery::LowBattery;
    use crate::slot::with_slot;
    use crate::system::{self, wide, TRAY_CLASS, WM_RELOAD};
    use crate::watch::{self, StatusSource};

    const WM_TRAY: u32 = WM_APP + 1;
    /// Posted by the watcher thread when the driver status changed (or the hub came/went).
    const WM_DRIVER: u32 = WM_APP + 2;
    /// Posted by the battery poll thread; `lparam` owns a `Box<Readings>`.
    const WM_BATTERY: u32 = WM_APP + 4;

    /// One reading per connected keyboard (short model name, battery).
    type Readings = Vec<(&'static str, Battery)>;

    /// A battery poll thread is running.
    static POLLING: AtomicBool = AtomicBool::new(false);
    /// Registered "TaskbarCreated" message; outside `App` so the window procedure can match it
    /// even while the state is borrowed.
    static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

    const TRAY_ID: u32 = 1;
    const BATTERY_TIMER: usize = 1;
    const BATTERY_POLL: u32 = 60_000;

    const CMD_MEDIA_FIRST: usize = 10;
    const CMD_FKEYS_FIRST: usize = 11;
    const CMD_AUTOSTART: usize = 12;
    const CMD_QUIT: usize = 13;
    const CMD_SETTINGS: usize = 14;
    const CMD_DRIVER_HELP: usize = 15;

    /// F16–F19 launcher keys (hotkey id = index + 1); targets come from app.toml.
    const LAUNCHER_KEYS: [u16; appconfig::LAUNCHER_COUNT] = [VK_F16, VK_F17, VK_F18, VK_F19];

    struct App {
        hwnd: HWND,
        icon: HICON,
        status: Option<Status>,
        batteries: Readings,
        config: AppConfig,
        low_battery: LowBattery,
        /// Why a filtered keyboard is not running (driver blocked or failed), if one is not.
        failure: Option<String>,
        failure_notified: bool,
    }

    thread_local! {
        static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    }

    /// Runs `f` on the tray state. `None` when re-entered from a message sent while the state is
    /// in use; keep Win32 calls that can dispatch sent messages (Shell_NotifyIconW, MessageBoxW,
    /// TrackPopupMenu) outside `f`.
    fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
        APP.with(|a| with_slot(a, f))
    }

    fn shell_notify(message: u32, nid: Option<NOTIFYICONDATAW>) {
        if let Some(nid) = nid {
            unsafe { Shell_NotifyIconW(message, &nid) };
        }
    }

    pub fn run() {
        let Some(_instance) = system::named_mutex("Local\\MagicKeyboardWindowsTray") else {
            return; // already running
        };
        let class = wide(TRAY_CLASS);
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
        TASKBAR_CREATED.store(taskbar_created, Ordering::Relaxed);
        APP.with(|a| {
            *a.borrow_mut() = Some(App {
                hwnd,
                icon: null_mut(),
                status: None,
                batteries: Vec::new(),
                config: appconfig::load(),
                low_battery: LowBattery::default(),
                failure: None,
                failure_notified: false,
            })
        });

        refresh_driver();
        add_tray_icon();
        poll_battery(hwnd);
        unsafe { SetTimer(hwnd, BATTERY_TIMER, BATTERY_POLL, None) };
        register_launchers(hwnd);
        spawn_watcher(hwnd as usize);

        let mut msg: MSG = unsafe { std::mem::zeroed() };
        while unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) } > 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    impl StatusSource for Control {
        fn status(&self) -> io::Result<Status> {
            Control::status(self)
        }

        fn wait_status(&self, generation: u32) -> io::Result<Status> {
            Control::wait_status(self, generation)
        }
    }

    /// Waits for driver status changes on its own handle and pokes the UI thread.
    fn spawn_watcher(hwnd: usize) {
        std::thread::spawn(move || {
            watch::run(
                Control::open,
                || unsafe {
                    PostMessageW(hwnd as HWND, WM_DRIVER, 0, 0);
                },
                |delay| {
                    std::thread::sleep(delay);
                    true
                },
            )
        });
    }

    fn refresh_driver() {
        let status = Control::open().and_then(|c| c.status()).ok();
        let failure = health::failed_keyboard().map(|f| f.message(health::secure_boot_on()));
        let notify_now = with_app(|a| {
            a.status = status;
            let first = failure.is_some() && !a.failure_notified;
            a.failure_notified = failure.is_some();
            a.failure = failure.clone();
            first
        })
        .unwrap_or(false);
        if notify_now {
            if let Some(text) = &failure {
                notify("Magic Keyboard driver not running", text);
            }
        }
    }

    /// Claims F16–F19 for the enabled launchers only, so a disabled one types a plain F-key.
    fn register_launchers(hwnd: HWND) {
        let config = with_app(|a| a.config.clone()).unwrap_or_default();
        for (i, vk) in LAUNCHER_KEYS.iter().enumerate() {
            let id = i as i32 + 1;
            unsafe { UnregisterHotKey(hwnd, id) };
            if config.launcher(i).is_some() {
                // Fails silently if another program owns the key; the rest still work.
                unsafe { RegisterHotKey(hwnd, id, MOD_NOREPEAT, u32::from(*vk)) };
            }
        }
    }

    /// Reads the batteries on a worker thread (a Bluetooth GET_REPORT can block) and posts the
    /// result as `WM_BATTERY`. Does nothing while a poll is still running.
    fn poll_battery(hwnd: HWND) {
        if POLLING.swap(true, Ordering::AcqRel) {
            return;
        }
        let hwnd = hwnd as usize;
        std::thread::spawn(move || {
            let readings: Readings = devices::connected()
                .into_iter()
                .filter_map(|k| k.battery.map(|b| (k.short_name(), b)))
                .collect();
            let boxed = Box::into_raw(Box::new(readings));
            if unsafe { PostMessageW(hwnd as HWND, WM_BATTERY, 0, boxed as LPARAM) } == 0 {
                drop(unsafe { Box::from_raw(boxed) });
                POLLING.store(false, Ordering::Release);
            }
        });
    }

    fn apply_battery(readings: Readings) {
        let warn = with_app(|a| {
            a.batteries = readings;
            // The weakest keyboard that is running on battery.
            let lowest = a
                .batteries
                .iter()
                .filter(|(_, b)| !b.charging)
                .min_by_key(|(_, b)| b.percent);
            let name = lowest.map(|(name, _)| *name);
            a.low_battery
                .update(lowest.map(|(_, b)| b.percent), a.config.low_battery)
                .zip(name)
        })
        .flatten();
        update_tray_icon();
        if let Some((name, percent)) = warn {
            notify(
                "Magic Keyboard battery low",
                &format!("{percent}% left ({name}). Connect it to charge."),
            );
        }
    }

    fn battery_text(b: &Battery) -> String {
        if b.fully_charged {
            format!("{}% (charged)", b.percent)
        } else if b.charging {
            format!("{}% (charging)", b.percent)
        } else {
            format!("{}%", b.percent)
        }
    }

    fn driver_text(a: &App) -> &'static str {
        if a.failure.is_some() {
            return "driver failed to load";
        }
        match &a.status {
            None => "driver not installed",
            Some(s) if s.keyboards == 0 => "no keyboard",
            Some(s) if !s.virtual_device_ready => "media keys unavailable",
            Some(_) => "active",
        }
    }

    fn tooltip(a: &App) -> String {
        let battery = match a.batteries.as_slice() {
            [] => "battery unknown".to_string(),
            [(_, b)] => battery_text(b),
            many => many
                .iter()
                .map(|(name, b)| format!("{name} {}", battery_text(b)))
                .collect::<Vec<_>>()
                .join(", "),
        };
        format!("Magic Keyboard: {battery}, {}", driver_text(a))
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

    /// The icon shows the weakest keyboard.
    fn current_level(a: &App) -> Level {
        match a.batteries.iter().map(|(_, b)| b).min_by_key(|b| b.percent) {
            Some(b) => Level::Percent {
                percent: b.percent,
                charging: b.charging && !b.fully_charged,
            },
            None => Level::Unknown,
        }
    }

    /// Renders the current icon and tooltip; returns the data and the icon it replaces, which the
    /// caller destroys after the shell has switched over.
    fn icon_data(a: &mut App) -> (NOTIFYICONDATAW, HICON) {
        let new_icon = icon::render(current_level(a), system::light_taskbar());
        let old_icon = std::mem::replace(&mut a.icon, new_icon);
        let mut nid = tray_data(a);
        nid.uFlags = NIF_ICON | NIF_TIP;
        nid.hIcon = new_icon;
        copy_into(&mut nid.szTip, &tooltip(a));
        (nid, old_icon)
    }

    fn set_tray_icon(message: u32) {
        let Some((mut nid, old_icon)) = with_app(icon_data) else {
            return;
        };
        if message == NIM_ADD {
            nid.uFlags |= NIF_MESSAGE;
            nid.uCallbackMessage = WM_TRAY;
        }
        shell_notify(message, Some(nid));
        if !old_icon.is_null() {
            unsafe { DestroyIcon(old_icon) };
        }
    }

    fn add_tray_icon() {
        set_tray_icon(NIM_ADD);
    }

    fn update_tray_icon() {
        set_tray_icon(NIM_MODIFY);
    }

    fn notify(title: &str, text: &str) {
        let nid = with_app(|a| {
            let mut nid = tray_data(a);
            nid.uFlags = NIF_INFO;
            nid.dwInfoFlags = NIIF_WARNING;
            copy_into(&mut nid.szInfoTitle, title);
            copy_into(&mut nid.szInfo, text);
            nid
        });
        shell_notify(NIM_MODIFY, nid);
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

    fn open_settings(hwnd: HWND) {
        let result = std::env::current_exe()
            .and_then(|exe| std::process::Command::new(exe).arg("settings").spawn());
        if let Err(e) = result {
            error_box(hwnd, &format!("Could not open the settings window:\n{e}"));
        }
    }

    fn show_menu(hwnd: HWND) {
        let Some((battery_lines, driver_line, fn_mode, driver_ok, failure)) = with_app(|a| {
            let batteries = if a.batteries.is_empty() {
                vec!["Battery: unknown".to_string()]
            } else {
                a.batteries
                    .iter()
                    .map(|(name, b)| format!("Battery ({name}): {}", battery_text(b)))
                    .collect()
            };
            (
                batteries,
                format!("Driver: {}", driver_text(a)),
                a.status.as_ref().and_then(|s| FnMode::from_u8(s.fn_mode)),
                a.status.is_some(),
                a.failure.clone(),
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
            for line in &battery_lines {
                item(MF_STRING | MF_GRAYED, 0, line);
            }
            if failure.is_some() {
                item(
                    MF_STRING,
                    CMD_DRIVER_HELP,
                    &format!("⚠ {driver_line} — what to do…"),
                );
            } else {
                item(MF_STRING | MF_GRAYED, 0, &driver_line);
            }
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
            item(MF_STRING, CMD_SETTINGS, "Settings…");
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
                CMD_SETTINGS => open_settings(hwnd),
                CMD_DRIVER_HELP => {
                    if let Some(text) = &failure {
                        error_box(hwnd, text);
                    }
                }
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
            WM_BATTERY => {
                // SAFETY: posted by poll_battery with a leaked Box<Readings>; taken back once.
                let readings = *unsafe { Box::from_raw(lparam as *mut Readings) };
                POLLING.store(false, Ordering::Release);
                apply_battery(readings);
                0
            }
            WM_RELOAD => {
                with_app(|a| a.config = appconfig::load());
                register_launchers(hwnd);
                poll_battery(hwnd);
                0
            }
            WM_TIMER if wparam == BATTERY_TIMER => {
                refresh_driver();
                poll_battery(hwnd);
                0
            }
            WM_HOTKEY => {
                let target = with_app(|a| {
                    a.config
                        .launcher(wparam.wrapping_sub(1))
                        .map(str::to_string)
                })
                .flatten();
                if let Some(target) = target {
                    system::launch(&target);
                }
                0
            }
            WM_SETTINGCHANGE => {
                // Taskbar theme may have changed.
                update_tray_icon();
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_DESTROY => {
                shell_notify(NIM_DELETE, with_app(|a| tray_data(a)));
                unsafe { PostQuitMessage(0) };
                0
            }
            _ if msg != 0 && msg == TASKBAR_CREATED.load(Ordering::Relaxed) => {
                // Explorer restarted: the icon is gone and must be added again.
                add_tray_icon();
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
