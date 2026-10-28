//! `input-monitor`: shows what Windows actually receives, via Raw Input on a message-only window.
//! Keyboards arrive as RAWKEYBOARD (VK + scan code); consumer-control collections arrive as raw HID
//! reports, which are decoded with the collection's preparsed data.

use std::collections::HashMap;
use std::io;
use std::mem::{size_of, zeroed};
use std::sync::Mutex;

use windows_sys::Win32::Devices::HumanInterfaceDevice as hid;
use windows_sys::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, RegisterRawInputDevices, HRAWINPUT, RAWINPUT,
    RAWINPUTDEVICE, RAWINPUTHEADER, RIDEV_INPUTSINK, RIDI_DEVICENAME, RIDI_PREPARSEDDATA,
    RID_INPUT, RIM_TYPEHID, RIM_TYPEKEYBOARD,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
    TranslateMessage, HWND_MESSAGE, MSG, WM_INPUT, WNDCLASSW,
};

const HIDP_STATUS_SUCCESS: i32 = 0x0011_0000;
const RI_KEY_BREAK: u16 = 1;
const RI_KEY_E0: u16 = 2;
const RI_KEY_E1: u16 = 4;

struct DevInfo {
    name: String,
    preparsed: Vec<u8>,
}

/// Device info cached by raw-input device handle.
static DEVICES: Mutex<Option<HashMap<usize, DevInfo>>> = Mutex::new(None);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Shortens a device path to its interesting part, e.g. `VID_05AC&PID_029F&MI_01&Col01`.
fn short_name(path: &str) -> String {
    let p = path.trim_start_matches(r"\\?\");
    let p = p.split('#').nth(1).unwrap_or(p);
    p.to_string()
}

fn device_info(h: HANDLE) -> (String, Vec<u8>) {
    unsafe {
        let mut n = 0u32;
        GetRawInputDeviceInfoW(h, RIDI_DEVICENAME, std::ptr::null_mut(), &mut n);
        let mut name = vec![0u16; n as usize];
        GetRawInputDeviceInfoW(h, RIDI_DEVICENAME, name.as_mut_ptr().cast(), &mut n);
        let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
        let name = String::from_utf16_lossy(&name[..end]);

        let mut n = 0u32;
        GetRawInputDeviceInfoW(h, RIDI_PREPARSEDDATA, std::ptr::null_mut(), &mut n);
        let mut pp = vec![0u8; n as usize];
        if n > 0 {
            GetRawInputDeviceInfoW(h, RIDI_PREPARSEDDATA, pp.as_mut_ptr().cast(), &mut n);
        }
        (short_name(&name), pp)
    }
}

fn with_device<R>(h: HANDLE, f: impl FnOnce(&DevInfo) -> R) -> R {
    let mut guard = DEVICES.lock().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    let info = map.entry(h as usize).or_insert_with(|| {
        let (name, preparsed) = device_info(h);
        DevInfo { name, preparsed }
    });
    f(info)
}

fn vk_name(vk: u16) -> &'static str {
    match vk {
        0x08 => "BACK",
        0x09 => "TAB",
        0x0D => "RETURN",
        0x10 => "SHIFT",
        0x11 => "CONTROL",
        0x12 => "MENU(Alt)",
        0x13 => "PAUSE",
        0x14 => "CAPITAL",
        0x1B => "ESCAPE",
        0x20 => "SPACE",
        0x21 => "PRIOR(PgUp)",
        0x22 => "NEXT(PgDn)",
        0x23 => "END",
        0x24 => "HOME",
        0x25 => "LEFT",
        0x26 => "UP",
        0x27 => "RIGHT",
        0x28 => "DOWN",
        0x2C => "SNAPSHOT(PrtSc)",
        0x2D => "INSERT",
        0x2E => "DELETE",
        0x5B => "LWIN",
        0x5C => "RWIN",
        0x5D => "APPS",
        0x5F => "SLEEP",
        0x70..=0x87 => "F-key",
        0x90 => "NUMLOCK",
        0x91 => "SCROLL",
        0xA0 => "LSHIFT",
        0xA1 => "RSHIFT",
        0xA2 => "LCONTROL",
        0xA3 => "RCONTROL",
        0xA4 => "LMENU",
        0xA5 => "RMENU",
        0xAD => "VOLUME_MUTE",
        0xAE => "VOLUME_DOWN",
        0xAF => "VOLUME_UP",
        0xB0 => "MEDIA_NEXT",
        0xB1 => "MEDIA_PREV",
        0xB2 => "MEDIA_STOP",
        0xB3 => "MEDIA_PLAY_PAUSE",
        0xFF => "(none: Fn/vendor)",
        _ => "",
    }
}

fn consumer_usages(pp: &[u8], report: &[u8]) -> Option<Vec<u16>> {
    if pp.is_empty() {
        return None;
    }
    let mut list = [0u16; 32];
    let mut n = list.len() as u32;
    let st = unsafe {
        hid::HidP_GetUsages(
            hid::HidP_Input,
            0x0C,
            0,
            list.as_mut_ptr(),
            &mut n,
            pp.as_ptr() as hid::PHIDP_PREPARSED_DATA,
            report.as_ptr() as _,
            report.len() as u32,
        )
    };
    (st == HIDP_STATUS_SUCCESS).then(|| list[..n as usize].to_vec())
}

unsafe fn on_input(lparam: LPARAM) {
    let mut size = 0u32;
    let hdr = size_of::<RAWINPUTHEADER>() as u32;
    GetRawInputData(
        lparam as HRAWINPUT,
        RID_INPUT,
        std::ptr::null_mut(),
        &mut size,
        hdr,
    );
    // u64 storage keeps RAWINPUT suitably aligned.
    let mut buf = vec![0u64; (size as usize).div_ceil(8).max(1)];
    if GetRawInputData(
        lparam as HRAWINPUT,
        RID_INPUT,
        buf.as_mut_ptr().cast(),
        &mut size,
        hdr,
    ) != size
    {
        return;
    }
    let raw = &*(buf.as_ptr() as *const RAWINPUT);
    let h = raw.header.hDevice;
    match raw.header.dwType {
        RIM_TYPEKEYBOARD => {
            let k = raw.data.keyboard;
            let prefix = if k.Flags & RI_KEY_E1 != 0 {
                "E1 "
            } else if k.Flags & RI_KEY_E0 != 0 {
                "E0 "
            } else {
                ""
            };
            let dir = if k.Flags & RI_KEY_BREAK != 0 {
                "up  "
            } else {
                "down"
            };
            let name = with_device(h, |d| d.name.clone());
            println!(
                "kbd  {dir} VK={:02X} {:<17} sc={prefix}{:02X}  {name}",
                k.VKey,
                vk_name(k.VKey),
                k.MakeCode
            );
        }
        RIM_TYPEHID => {
            let hd = &raw.data.hid;
            let total = (hd.dwSizeHid * hd.dwCount) as usize;
            let data = std::slice::from_raw_parts(hd.bRawData.as_ptr(), total);
            with_device(h, |d| {
                for r in data.chunks(hd.dwSizeHid.max(1) as usize) {
                    let decoded = match consumer_usages(&d.preparsed, r) {
                        Some(u) if u.is_empty() => "  [released]".to_string(),
                        Some(u) => {
                            let s: Vec<String> = u.iter().map(|x| format!("{x:03X}")).collect();
                            format!("  [0C: {}]", s.join(","))
                        }
                        None => String::new(),
                    };
                    println!("hid  {}{decoded}  {}", mk_hid::hex(r), d.name);
                }
            });
        }
        _ => {}
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_INPUT {
        on_input(lparam);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

pub fn run() -> io::Result<()> {
    unsafe {
        let hinst = GetModuleHandleW(std::ptr::null());
        let class = wide("mk-probe-rawinput");
        let mut wc: WNDCLASSW = zeroed();
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = hinst;
        wc.lpszClassName = class.as_ptr();
        if RegisterClassW(&wc) == 0 {
            return Err(io::Error::last_os_error());
        }
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err(io::Error::last_os_error());
        }
        // Generic Desktop/Keyboard and Consumer/Consumer Control, delivered even without focus.
        let devs = [(0x01u16, 0x06u16), (0x0C, 0x01)].map(|(page, usage)| RAWINPUTDEVICE {
            usUsagePage: page,
            usUsage: usage,
            dwFlags: RIDEV_INPUTSINK,
            hwndTarget: hwnd,
        });
        if RegisterRawInputDevices(
            devs.as_ptr(),
            devs.len() as u32,
            size_of::<RAWINPUTDEVICE>() as u32,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        println!("monitoring keyboard + consumer input (Ctrl+C stops)");
        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
