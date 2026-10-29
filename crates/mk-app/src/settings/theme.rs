//! Windows 11 look for the settings window: Segoe UI fonts, Fluent greys and radii, the system
//! accent color, and the window icon.

use std::sync::{Arc, OnceLock};

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Theme, Vec2, Visuals,
};

use crate::system;

/// Colors not covered by egui's `Visuals`.
pub struct Palette {
    pub card: Color32,
    pub card_stroke: Color32,
    pub subtle: Color32,
    pub accent: Color32,
    /// Text drawn on an accent fill.
    pub on_accent: Color32,
}

pub fn palette(dark: bool) -> Palette {
    let accent = accent(dark);
    if dark {
        Palette {
            card: Color32::from_gray(0x2B),
            card_stroke: Color32::from_gray(0x1D),
            subtle: Color32::from_gray(0xA0),
            accent,
            on_accent: Color32::BLACK,
        }
    } else {
        Palette {
            card: Color32::from_gray(0xFB),
            card_stroke: Color32::from_gray(0xE5),
            subtle: Color32::from_gray(0x60),
            accent,
            on_accent: Color32::WHITE,
        }
    }
}

/// The system accent color, lightened for dark mode the way Windows does for text and fills.
fn accent(dark: bool) -> Color32 {
    static SYSTEM: OnceLock<Option<(u8, u8, u8)>> = OnceLock::new();
    match (*SYSTEM.get_or_init(system::accent_color), dark) {
        (Some((r, g, b)), false) => Color32::from_rgb(r, g, b),
        (Some((r, g, b)), true) => {
            let lift = |c: u8| c + ((255 - c) as f32 * 0.45) as u8;
            Color32::from_rgb(lift(r), lift(g), lift(b))
        }
        (None, false) => Color32::from_rgb(0x00, 0x78, 0xD4),
        (None, true) => Color32::from_rgb(0x60, 0xCD, 0xFF),
    }
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

/// A Segoe Fluent Icons glyph.
pub fn icon(glyph: char, size: f32) -> egui::RichText {
    egui::RichText::new(glyph.to_string()).font(FontId::new(size, FontFamily::Name("icons".into())))
}

pub mod glyph {
    pub const KEYBOARD: char = '\u{E765}';
    pub const BLUETOOTH: char = '\u{E702}';
    pub const USB: char = '\u{E88E}';
    pub const BOLT: char = '\u{E945}';
    pub const RESET: char = '\u{E7A7}';
    pub const CLOSE: char = '\u{E711}';
    pub const INFO: char = '\u{E946}';
    pub const WARNING: char = '\u{E7BA}';
    pub const ERROR: char = '\u{E783}';
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    for theme in [Theme::Dark, Theme::Light] {
        ctx.set_visuals_of(theme, visuals(theme == Theme::Dark));
        ctx.style_mut_of(theme, |style| {
            style.text_styles = [
                (TextStyle::Heading, semibold(20.0)),
                (TextStyle::Body, FontId::proportional(14.0)),
                (TextStyle::Button, FontId::proportional(14.0)),
                (TextStyle::Small, FontId::proportional(12.0)),
                (TextStyle::Monospace, FontId::monospace(13.0)),
            ]
            .into();
            let spacing = &mut style.spacing;
            spacing.item_spacing = Vec2::new(8.0, 6.0);
            spacing.button_padding = Vec2::new(12.0, 5.0);
            spacing.interact_size.y = 30.0;
            spacing.combo_height = 320.0;
        });
    }
}

fn visuals(dark: bool) -> Visuals {
    let p = palette(dark);
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let (window, control, hover, active, text) = if dark {
        (0x20, 0x37, 0x3D, 0x32, 0xFF)
    } else {
        (0xF3, 0xFE, 0xF6, 0xF0, 0x1A)
    };
    v.panel_fill = Color32::from_gray(window);
    v.window_fill = if dark {
        Color32::from_gray(0x2C)
    } else {
        Color32::from_gray(0xF9)
    };
    v.extreme_bg_color = if dark {
        Color32::from_gray(0x1C)
    } else {
        Color32::WHITE
    };
    v.faint_bg_color = p.card;
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_stroke = Stroke::new(1.0_f32, p.card_stroke);
    v.selection.bg_fill = p.accent.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0_f32, p.accent);
    v.hyperlink_color = p.accent;
    v.override_text_color = Some(Color32::from_gray(text));
    let border = if dark {
        Color32::from_gray(0x45)
    } else {
        Color32::from_gray(0xD6)
    };
    for (w, fill) in [
        (&mut v.widgets.inactive, control),
        (&mut v.widgets.hovered, hover),
        (&mut v.widgets.active, active),
        (&mut v.widgets.open, active),
    ] {
        w.bg_fill = Color32::from_gray(fill);
        w.weak_bg_fill = Color32::from_gray(fill);
        w.bg_stroke = Stroke::new(1.0_f32, border);
        w.corner_radius = CornerRadius::same(4);
        w.expansion = 0.0;
    }
    v.widgets.hovered.fg_stroke.color = Color32::from_gray(text);
    v.widgets.noninteractive.corner_radius = CornerRadius::same(4);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.card_stroke);
    v.widgets.noninteractive.fg_stroke.color = Color32::from_gray(text);
    v
}

/// Segoe UI (plus Semibold) for text, Segoe UI Symbol as fallback for arrows and other symbols,
/// Segoe Fluent Icons for icons. Keeps the bundled fonts for any file that is missing.
fn install_fonts(ctx: &egui::Context) {
    let fonts_dir = std::env::var_os("WINDIR")
        .map(|w| std::path::PathBuf::from(w).join("Fonts"))
        .unwrap_or_else(|| "C:\\Windows\\Fonts".into());
    let mut fonts = egui::FontDefinitions::default();
    let mut load = |name: &str, file: &str| {
        let bytes = std::fs::read(fonts_dir.join(file)).ok()?;
        fonts
            .font_data
            .insert(name.into(), Arc::new(egui::FontData::from_owned(bytes)));
        Some(name.to_string())
    };
    let regular = load("segoe-ui", "segoeui.ttf");
    let semibold = load("segoe-ui-semibold", "seguisb.ttf");
    let symbol = load("segoe-ui-symbol", "seguisym.ttf");
    let icons = load("segoe-fluent-icons", "SegoeIcons.ttf");

    let bundled = fonts.families[&FontFamily::Proportional].clone();
    let family = |first: &[Option<String>]| {
        let mut list: Vec<String> = first.iter().flatten().cloned().collect();
        list.extend(symbol.clone());
        list.extend(bundled.iter().cloned());
        list
    };
    fonts.families.insert(
        FontFamily::Proportional,
        family(std::slice::from_ref(&regular)),
    );
    fonts.families.insert(
        FontFamily::Name("semibold".into()),
        family(&[semibold, regular.clone()]),
    );
    fonts
        .families
        .insert(FontFamily::Name("icons".into()), family(&[icons, regular]));
    ctx.set_fonts(fonts);
}

/// A small keyboard drawn at 32 × 32 for the title bar and taskbar.
pub fn window_icon() -> egui::IconData {
    const S: usize = 32;
    let mut rgba = vec![0u8; S * S * 4];
    let mut set = |x: usize, y: usize, c: [u8; 4]| rgba[(y * S + x) * 4..][..4].copy_from_slice(&c);
    let body = [0x3A, 0x3A, 0x3C, 0xFF];
    let key = [0xF2, 0xF2, 0xF2, 0xFF];
    let (top, bottom) = (8, 25);
    for y in top..bottom {
        for x in 1..S - 1 {
            let corner = (y == top || y == bottom - 1) && (x == 1 || x == S - 2);
            if !corner {
                set(x, y, body);
            }
        }
    }
    // Three rows of 3 × 3 keys and a space bar.
    for row in 0..3 {
        let y0 = top + 2 + row * 4;
        for col in 0..7 {
            let x0 = 3 + col * 4;
            for y in y0..y0 + 3 {
                for x in x0..x0 + 3 {
                    set(x, y, key);
                }
            }
        }
    }
    for y in bottom - 3..bottom - 1 {
        for x in 9..S - 9 {
            set(x, y, key);
        }
    }
    egui::IconData {
        rgba,
        width: S as u32,
        height: S as u32,
    }
}
