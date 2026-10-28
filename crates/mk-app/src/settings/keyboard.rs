//! The drawn keyboard: per-model key geometry (pure data, unit-tested), painting with egui shapes,
//! and hit-testing so a click on a configurable key opens its control.

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use mk_core::config::ModSource;
use mk_core::model::Model;
use mk_core::Config;

use super::labels;
use crate::appconfig::AppConfig;

/// A setting reachable from a key on the drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Modifier(ModSource),
    /// Any of F1–F12 except F6: the top-row mode.
    TopRow,
    /// F6: its special action (and the top-row mode).
    F6,
    Lock,
    Eject,
    Clear,
    /// F13 + index (0..7).
    FExt(u8),
}

/// One key in key units (1 u = a letter key including its gap).
#[derive(Clone, Debug)]
pub struct Key {
    pub label: &'static str,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub target: Option<Target>,
}

impl Key {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub keys: Vec<Key>,
    pub width: f32,
    pub height: f32,
}

impl Layout {
    /// The configurable key at a position in key units.
    pub fn hit(&self, x: f32, y: f32) -> Option<&Key> {
        self.keys
            .iter()
            .find(|k| k.target.is_some() && k.contains(x, y))
    }
}

/// Width of the main (alphanumeric) block.
pub const MAIN_WIDTH: f32 = 14.5;
const TOP_ROW_H: f32 = 0.5;
const GAP: f32 = 0.5;

struct Builder {
    keys: Vec<Key>,
    x: f32,
    y: f32,
    h: f32,
}

impl Builder {
    fn row(&mut self, x: f32, y: f32, h: f32) -> &mut Self {
        (self.x, self.y, self.h) = (x, y, h);
        self
    }
    fn key(&mut self, label: &'static str, w: f32, target: Option<Target>) -> &mut Self {
        self.keys.push(Key {
            label,
            x: self.x,
            y: self.y,
            w,
            h: self.h,
            target,
        });
        self.x += w;
        self
    }
    fn keys(&mut self, labels: &[&'static str]) -> &mut Self {
        for &l in labels {
            self.key(l, 1.0, None);
        }
        self
    }
    /// A key with explicit geometry that does not move the cursor.
    fn place(
        &mut self,
        label: &'static str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        target: Option<Target>,
    ) {
        self.keys.push(Key {
            label,
            x,
            y,
            w,
            h,
            target,
        });
    }
}

const F_LABELS: [&str; 12] = [
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
];

/// Geometry for a model (ANSI legends). Unknown models get the numeric-keypad layout.
pub fn layout_for(model: Option<&Model>) -> Layout {
    let numpad = model.is_none_or(|m| m.has_numpad);
    let eject = model.is_some_and(|m| m.has_eject);
    let mut b = Builder {
        keys: Vec::new(),
        x: 0.0,
        y: 0.0,
        h: 1.0,
    };
    use ModSource::*;

    // Top row (half height).
    b.row(0.0, 0.0, TOP_ROW_H).key("esc", 1.5, None);
    for (i, label) in F_LABELS.iter().enumerate() {
        let target = if i == 5 { Target::F6 } else { Target::TopRow };
        b.key(label, 1.0, Some(target));
    }
    if eject {
        b.key("⏏", 1.0, Some(Target::Eject));
    } else {
        b.key("lock", 1.0, Some(Target::Lock));
    }

    let r1 = TOP_ROW_H;
    b.row(0.0, r1, 1.0)
        .keys(&[
            "`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=",
        ])
        .key("delete", 1.5, None);
    b.row(0.0, r1 + 1.0, 1.0).key("tab", 1.5, None).keys(&[
        "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "[", "]", "\\",
    ]);
    b.row(0.0, r1 + 2.0, 1.0)
        .key("caps lock", 1.75, Some(Target::Modifier(CapsLock)))
        .keys(&["A", "S", "D", "F", "G", "H", "J", "K", "L", ";", "'"])
        .key("return", 1.75, None);
    b.row(0.0, r1 + 3.0, 1.0)
        .key("shift", 2.25, Some(Target::Modifier(LeftShift)))
        .keys(&["Z", "X", "C", "V", "B", "N", "M", ",", ".", "/"])
        .key("shift", 2.25, Some(Target::Modifier(RightShift)));

    let bottom = r1 + 4.0;
    let m = |s| Some(Target::Modifier(s));
    if numpad {
        b.row(0.0, bottom, 1.0)
            .key("control", 1.25, m(LeftControl))
            .key("option", 1.25, m(LeftOption))
            .key("command", 1.5, m(LeftCommand))
            .key("", 6.5, None)
            .key("command", 1.5, m(RightCommand))
            .key("option", 1.25, m(RightOption))
            .key("control", 1.25, m(RightControl));

        // Navigation cluster.
        let nav = MAIN_WIDTH + GAP;
        b.row(nav, 0.0, TOP_ROW_H);
        for (i, label) in ["F13", "F14", "F15"].into_iter().enumerate() {
            b.key(label, 1.0, Some(Target::FExt(i as u8)));
        }
        b.row(nav, r1, 1.0)
            .key("fn", 1.0, m(Fn))
            .keys(&["home", "pg up"]);
        b.row(nav, r1 + 1.0, 1.0).keys(&["del", "end", "pg dn"]);
        b.place("↑", nav + 1.0, r1 + 3.0, 1.0, 1.0, None);
        b.row(nav, bottom, 1.0).keys(&["←", "↓", "→"]);

        // Numeric keypad.
        let pad = nav + 3.0 + GAP;
        b.row(pad, 0.0, TOP_ROW_H);
        for (i, label) in ["F16", "F17", "F18", "F19"].into_iter().enumerate() {
            b.key(label, 1.0, Some(Target::FExt(3 + i as u8)));
        }
        b.row(pad, r1, 1.0)
            .key("clear", 1.0, Some(Target::Clear))
            .keys(&["=", "/", "*"]);
        b.row(pad, r1 + 1.0, 1.0).keys(&["7", "8", "9", "−"]);
        b.row(pad, r1 + 2.0, 1.0).keys(&["4", "5", "6", "+"]);
        b.row(pad, r1 + 3.0, 1.0).keys(&["1", "2", "3"]);
        b.place("enter", pad + 3.0, r1 + 3.0, 1.0, 2.0, None);
        b.row(pad, bottom, 1.0)
            .key("0", 2.0, None)
            .key(".", 1.0, None);

        Layout {
            keys: b.keys,
            width: pad + 4.0,
            height: bottom + 1.0,
        }
    } else {
        b.row(0.0, bottom, 1.0)
            .key("fn", 1.0, m(Fn))
            .key("control", 1.0, m(LeftControl))
            .key("option", 1.0, m(LeftOption))
            .key("command", 1.25, m(LeftCommand))
            .key("", 5.0, None)
            .key("command", 1.25, m(RightCommand))
            .key("option", 1.0, m(RightOption));
        // Inverted-T arrows: left/right and up/down are half height.
        let arrows = MAIN_WIDTH - 3.0;
        let half = bottom + 0.5;
        b.place("←", arrows, half, 1.0, 0.5, None);
        b.place("↑", arrows + 1.0, bottom, 1.0, 0.5, None);
        b.place("↓", arrows + 1.0, half, 1.0, 0.5, None);
        b.place("→", arrows + 2.0, half, 1.0, 0.5, None);
        Layout {
            keys: b.keys,
            width: MAIN_WIDTH,
            height: bottom + 1.0,
        }
    }
}

/// Highlight category of a key, also used for the legend.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Fn,
    Special,
    Remapped,
    Launcher,
}

impl Mark {
    pub const ALL: [Mark; 4] = [Mark::Fn, Mark::Special, Mark::Remapped, Mark::Launcher];

    pub fn color(self) -> Color32 {
        match self {
            Mark::Fn => Color32::from_rgb(0xE8, 0x8A, 0x1A),
            Mark::Special => Color32::from_rgb(0x8A, 0x6C, 0xE0),
            Mark::Remapped => Color32::from_rgb(0x2E, 0xA8, 0x4A),
            Mark::Launcher => Color32::from_rgb(0x2F, 0x86, 0xD8),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Mark::Fn => "Fn",
            Mark::Special => "special key",
            Mark::Remapped => "changed from default",
            Mark::Launcher => "launches a program",
        }
    }
}

fn mark(target: Target, config: &Config, app: &AppConfig) -> Option<Mark> {
    let defaults = Config::default();
    match target {
        Target::Modifier(ModSource::Fn)
            if config.modifier(ModSource::Fn) == ModSource::Fn.native_target() =>
        {
            Some(Mark::Fn)
        }
        Target::Modifier(src) => {
            (config.modifier(src) != src.native_target()).then_some(Mark::Remapped)
        }
        Target::Lock | Target::Eject | Target::Clear => Some(Mark::Special),
        Target::FExt(i) if i >= 3 => app.launcher(usize::from(i - 3)).map(|_| Mark::Launcher),
        Target::FExt(i) => (config.f_ext[usize::from(i)] != defaults.f_ext[usize::from(i)])
            .then_some(Mark::Remapped),
        Target::F6 => (config.f6 != defaults.f6).then_some(Mark::Remapped),
        Target::TopRow => None,
    }
}

struct Palette {
    case: Color32,
    key: Color32,
    key_hover: Color32,
    edge: Color32,
    text: Color32,
}

fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            case: Color32::from_gray(0x2C),
            key: Color32::from_gray(0x44),
            key_hover: Color32::from_gray(0x58),
            edge: Color32::from_gray(0x1C),
            text: Color32::from_gray(0xE8),
        }
    } else {
        Palette {
            case: Color32::from_gray(0xC9),
            key: Color32::from_gray(0xFA),
            key_hover: Color32::from_gray(0xE6),
            edge: Color32::from_gray(0x9A),
            text: Color32::from_gray(0x30),
        }
    }
}

/// Paints the keyboard at the available width. Returns the clicked configurable key and its
/// screen rectangle.
pub fn show(
    ui: &mut egui::Ui,
    layout: &Layout,
    config: &Config,
    app: &AppConfig,
    selected: Option<Target>,
) -> Option<(Target, Rect)> {
    let border = 0.35;
    let unit = (ui.available_width() / (layout.width + 2.0 * border)).min(46.0);
    let size = Vec2::new(layout.width + 2.0 * border, layout.height + 2.0 * border) * unit;
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let origin = rect.min + Vec2::splat(border * unit);
    let key_rect = |k: &Key| {
        Rect::from_min_size(
            origin + Vec2::new(k.x, k.y) * unit,
            Vec2::new(k.w, k.h) * unit,
        )
        .shrink(unit * 0.06)
    };
    let to_units = |p: Pos2| ((p.x - origin.x) / unit, (p.y - origin.y) / unit);
    let hovered = response
        .hover_pos()
        .and_then(|p| {
            let (x, y) = to_units(p);
            layout.hit(x, y)
        })
        .cloned();

    let colors = palette(ui.visuals().dark_mode);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same((unit * 0.35) as u8), colors.case);
    let radius = CornerRadius::same((unit * 0.12).max(2.0) as u8);
    for k in &layout.keys {
        let r = key_rect(k);
        let is_hovered = hovered.as_ref().is_some_and(|h| h.x == k.x && h.y == k.y);
        let fill = if is_hovered {
            colors.key_hover
        } else {
            colors.key
        };
        painter.rect_filled(r, radius, fill);
        let mark = k.target.and_then(|t| mark(t, config, app));
        let (stroke_color, width) = match mark {
            Some(m) => (m.color(), (unit * 0.07).max(2.0)),
            None => (colors.edge, 1.0),
        };
        painter.rect_stroke(
            r,
            radius,
            Stroke::new(width, stroke_color),
            StrokeKind::Inside,
        );
        if k.target.is_some() && k.target == selected {
            painter.rect_stroke(
                r.expand(unit * 0.04),
                radius,
                Stroke::new((unit * 0.09).max(2.5), ui.visuals().selection.stroke.color),
                StrokeKind::Outside,
            );
        }
        if !k.label.is_empty() {
            let font =
                FontId::proportional((unit * if k.label.len() > 3 { 0.24 } else { 0.32 }).max(7.0));
            painter.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                k.label,
                font,
                colors.text,
            );
        }
    }

    if let Some(key) = &hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        if let Some(target) = key.target {
            let text = labels::summary(target, config, app);
            response.clone().on_hover_ui_at_pointer(|ui| {
                ui.label(text);
            });
        }
    }
    if response.clicked() {
        if let Some(key) = hovered {
            return key.target.map(|t| (t, key_rect(&key)));
        }
    }
    None
}

/// Legend of the highlight colors.
pub fn legend(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for m in Mark::ALL {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(2),
                Stroke::new(2.5_f32, m.color()),
                StrokeKind::Inside,
            );
            ui.weak(m.label());
            ui.add_space(8.0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::model;

    fn model(pid: u16) -> &'static Model {
        model::find(model::APPLE_USB_VID, pid).unwrap()
    }

    fn targets(layout: &Layout) -> Vec<Target> {
        layout.keys.iter().filter_map(|k| k.target).collect()
    }

    #[test]
    fn main_block_rows_span_the_full_width() {
        for layout in [
            layout_for(Some(model(0x029A))),
            layout_for(Some(model(0x029F))),
        ] {
            let mut rows: Vec<f32> = layout
                .keys
                .iter()
                .filter(|k| k.x < MAIN_WIDTH && (k.y == 0.0 || k.h >= 1.0))
                .map(|k| k.y)
                .collect();
            rows.dedup();
            for y in rows {
                let row_h = if y == 0.0 { TOP_ROW_H } else { 1.0 };
                // Keys whose vertical span intersects the row (half-height arrows count).
                let right = layout
                    .keys
                    .iter()
                    .filter(|k| k.x < MAIN_WIDTH && k.y < y + row_h && k.y + k.h > y)
                    .map(|k| k.x + k.w)
                    .fold(0.0, f32::max);
                assert!(
                    (right - MAIN_WIDTH).abs() < 1e-4,
                    "row at y={y} ends at {right}"
                );
            }
        }
    }

    #[test]
    fn keys_do_not_overlap() {
        for layout in [
            layout_for(Some(model(0x029A))),
            layout_for(Some(model(0x029F))),
        ] {
            for (i, a) in layout.keys.iter().enumerate() {
                for b in &layout.keys[i + 1..] {
                    let overlap = a.x < b.x + b.w - 1e-4
                        && b.x < a.x + a.w - 1e-4
                        && a.y < b.y + b.h - 1e-4
                        && b.y < a.y + a.h - 1e-4;
                    assert!(!overlap, "{} overlaps {}", a.label, b.label);
                }
            }
        }
    }

    #[test]
    fn modifiers_appear_once_per_model() {
        let compact = targets(&layout_for(Some(model(0x029A))));
        let keypad = targets(&layout_for(Some(model(0x029F))));
        for src in ModSource::ALL {
            let count = |t: &[Target]| t.iter().filter(|&&x| x == Target::Modifier(src)).count();
            assert_eq!(count(&keypad), 1, "{src:?} on keypad model");
            let expected = usize::from(src != ModSource::RightControl);
            assert_eq!(count(&compact), expected, "{src:?} on compact model");
        }
    }

    #[test]
    fn keypad_only_targets() {
        let compact = targets(&layout_for(Some(model(0x029A))));
        let keypad = targets(&layout_for(Some(model(0x029F))));
        assert!(!compact.contains(&Target::Clear));
        assert!(!compact.iter().any(|t| matches!(t, Target::FExt(_))));
        assert!(keypad.contains(&Target::Clear));
        for i in 0..7 {
            assert!(keypad.contains(&Target::FExt(i)));
        }
        // Lock on Touch ID models, Eject on older ones.
        assert!(compact.contains(&Target::Lock));
        assert!(targets(&layout_for(Some(model(0x0267)))).contains(&Target::Eject));
    }

    #[test]
    fn hit_test_finds_each_configurable_key() {
        for layout in [
            layout_for(Some(model(0x029A))),
            layout_for(Some(model(0x029F))),
        ] {
            for k in layout.keys.iter().filter(|k| k.target.is_some()) {
                let hit = layout
                    .hit(k.x + k.w / 2.0, k.y + k.h / 2.0)
                    .map(|h| h.target);
                assert_eq!(hit, Some(k.target), "{}", k.label);
            }
        }
    }
}
