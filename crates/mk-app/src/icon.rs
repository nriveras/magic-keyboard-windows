//! Tray icon drawn at runtime: a battery outline filled to the charge level.

use std::ptr::null_mut;

use windows_sys::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
};

/// What the icon shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// No battery reading (keyboard absent or not readable).
    Unknown,
    Percent {
        percent: u8,
        charging: bool,
    },
}

/// ARGB colors.
const GREEN: u32 = 0xFF3C_C85A;
const AMBER: u32 = 0xFFF0_B428;
const RED: u32 = 0xFFE6_3C3C;
const BLUE: u32 = 0xFF3C_A0F0;

/// Renders the icon at the system small-icon size. `light_taskbar` picks a dark outline.
pub fn render(level: Level, light_taskbar: bool) -> HICON {
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.clamp(16, 64);
    let pixels = draw(size as usize, level, light_taskbar);
    unsafe { to_hicon(size, &pixels) }
}

/// Draws a horizontal battery: body with a 1/16-size stroke, a terminal nub on the right.
fn draw(size: usize, level: Level, light_taskbar: bool) -> Vec<u32> {
    let mut px = vec![0u32; size * size];
    let outline = if light_taskbar {
        0xFF20_2020
    } else {
        0xFFF0_F0F0
    };
    let stroke = (size / 16).max(1);
    let (left, right) = (stroke, size - 3 * stroke);
    let (top, bottom) = (size * 5 / 16, size - size * 5 / 16);
    let mut set = |x: usize, y: usize, c: u32| px[y * size + x] = c;

    for y in top..bottom {
        for x in left..right {
            let edge = x < left + stroke
                || x >= right - stroke
                || y < top + stroke
                || y >= bottom - stroke;
            if edge {
                set(x, y, outline);
            }
        }
    }
    // Terminal nub.
    for y in (size * 7 / 16)..(size - size * 7 / 16) {
        for x in right..(right + 2 * stroke) {
            set(x, y, outline);
        }
    }

    let (fill_color, fraction) = match level {
        Level::Unknown => (outline & 0x80FF_FFFF, 1.0),
        Level::Percent { percent, charging } => {
            let color = if charging {
                BLUE
            } else if percent <= 20 {
                RED
            } else if percent <= 40 {
                AMBER
            } else {
                GREEN
            };
            (color, f32::from(percent.min(100)) / 100.0)
        }
    };
    let inner_left = left + 2 * stroke;
    let inner_right = right - 2 * stroke;
    let width = inner_right.saturating_sub(inner_left);
    // At least one column so a nearly empty battery is still visible.
    let filled = ((width as f32 * fraction).round() as usize).clamp(1, width);
    for y in (top + 2 * stroke)..(bottom - 2 * stroke) {
        for x in inner_left..inner_left + filled {
            set(x, y, fill_color);
        }
    }
    px
}

/// Builds an alpha-blended HICON from top-down ARGB pixels.
unsafe fn to_hicon(size: i32, pixels: &[u32]) -> HICON {
    let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: size,
        biHeight: -size, // top-down
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        ..unsafe { std::mem::zeroed() }
    };
    let mut bits: *mut core::ffi::c_void = null_mut();
    unsafe {
        let color = CreateDIBSection(null_mut(), &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if color.is_null() || bits.is_null() {
            return null_mut();
        }
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u32>(), pixels.len());
        // The mask is ignored for 32-bit icons with alpha, but must exist.
        let mask_bytes = vec![0u8; (size as usize).div_ceil(16) * 2 * size as usize];
        let mask = CreateBitmap(size, size, 1, 1, mask_bytes.as_ptr().cast());
        let icon_info = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&icon_info);
        DeleteObject(color);
        DeleteObject(mask);
        icon
    }
}
