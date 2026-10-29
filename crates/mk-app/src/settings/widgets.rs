//! Fluent-style building blocks: setting cards, a toggle switch, an info bar and header pills.

use eframe::egui::{
    self, Color32, CornerRadius, Margin, Response, RichText, Sense, Stroke, StrokeKind, Vec2,
};
use mk_hid::battery::Battery;

use super::theme::{self, glyph};

/// Width of the control column in cards, so dropdowns line up.
pub const CONTROL_WIDTH: f32 = 220.0;

/// Section title above a group of cards.
pub fn section_header(ui: &mut egui::Ui, text: &str) {
    ui.add_space(14.0);
    ui.label(RichText::new(text).font(theme::semibold(15.0)));
    ui.add_space(2.0);
}

fn card_frame(ui: &egui::Ui) -> egui::Frame {
    let p = theme::palette(ui.visuals().dark_mode);
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0_f32, p.card_stroke))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(16, 10))
}

/// A settings row: title and optional description on the left, the control on the right.
pub fn card(
    ui: &mut egui::Ui,
    title: &str,
    description: Option<&str>,
    control: impl FnOnce(&mut egui::Ui),
) {
    card_with(ui, |ui| title_block(ui, title, description), control);
}

/// A card whose left side is drawn by `left`.
pub fn card_with(
    ui: &mut egui::Ui,
    left: impl FnOnce(&mut egui::Ui),
    control: impl FnOnce(&mut egui::Ui),
) {
    card_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::Sides::new()
            .height(38.0)
            .shrink_left()
            .wrap()
            .show(ui, left, control);
    });
    ui.add_space(3.0);
}

/// A card holding arbitrary content.
pub fn card_body(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    card_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
    ui.add_space(3.0);
}

pub fn title_block(ui: &mut egui::Ui, title: &str, description: Option<&str>) {
    if description.is_none() {
        ui.label(title);
        return;
    }
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.label(title);
        if let Some(d) = description {
            ui.label(
                RichText::new(d)
                    .small()
                    .color(theme::palette(ui.visuals().dark_mode).subtle),
            );
        }
    });
}

/// A small dot marking a setting changed from its default.
pub fn changed_dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, color);
    response.on_hover_text("Changed from default");
}

/// On/off switch with its state written next to it, as in Windows Settings.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool) -> Response {
    let p = theme::palette(ui.visuals().dark_mode);
    let size = Vec2::new(40.0, 20.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, "")
    });
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_responsive(response.id, *on);
        let visuals = ui.style().interact(&response);
        let radius = rect.height() / 2.0;
        let (fill, stroke, knob) = if *on {
            (p.accent, Stroke::new(1.0_f32, p.accent), p.on_accent)
        } else {
            (
                Color32::TRANSPARENT,
                Stroke::new(1.0_f32, visuals.fg_stroke.color),
                visuals.fg_stroke.color,
            )
        };
        let fill = if ui.is_enabled() {
            fill
        } else {
            fill.gamma_multiply(0.5)
        };
        ui.painter()
            .rect(rect, radius, fill, stroke, StrokeKind::Inside);
        let knob_r = if response.hovered() { 7.0 } else { 6.0 };
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        ui.painter()
            .circle_filled(egui::pos2(x, rect.center().y), knob_r, knob);
    }
    response
}

/// Toggle with "On"/"Off" written before it (the card's control column is right-to-left).
pub fn labeled_toggle(ui: &mut egui::Ui, on: &mut bool) -> Response {
    let response = toggle(ui, on);
    ui.label(if *on { "On" } else { "Off" });
    response
}

#[derive(Clone, Copy)]
pub enum Severity {
    Warning,
    Error,
}

/// A colored banner for state the user should know about.
pub fn info_bar(ui: &mut egui::Ui, severity: Severity, title: &str, body: &str) {
    let dark = ui.visuals().dark_mode;
    let rgb = |c: u32| Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
    let (icon, color, fill) = match (severity, dark) {
        (Severity::Warning, true) => (glyph::WARNING, 0xFC_E100, 0x43_3519),
        (Severity::Warning, false) => (glyph::WARNING, 0x9D_5D00, 0xFF_F4CE),
        (Severity::Error, true) => (glyph::ERROR, 0xFF_99A4, 0x44_2726),
        (Severity::Error, false) => (glyph::ERROR, 0xC4_2B1C, 0xFD_E7E9),
    };
    let (color, fill) = (rgb(color), rgb(fill));
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, theme::palette(dark).card_stroke))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.label(theme::icon(icon, 16.0).color(color));
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).font(theme::semibold(14.0)));
                    if !body.is_empty() {
                        ui.label(body);
                    }
                });
            });
        });
    ui.add_space(6.0);
}

/// A rounded, selectable header button with an icon.
pub fn pill(ui: &mut egui::Ui, icon: char, text: &str, detail: &str, selected: bool) -> Response {
    let p = theme::palette(ui.visuals().dark_mode);
    let fill = if selected {
        p.accent
            .linear_multiply(if ui.visuals().dark_mode { 0.22 } else { 0.14 })
    } else {
        Color32::TRANSPARENT
    };
    let stroke = if selected {
        Stroke::new(1.0_f32, p.accent)
    } else {
        Stroke::new(1.0_f32, p.card_stroke)
    };
    let response = egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::symmetric(12, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::icon(glyph::KEYBOARD, 15.0));
                ui.label(text);
                ui.label(theme::icon(icon, 12.0).color(p.subtle));
                ui.label(RichText::new(detail).small().color(p.subtle));
            });
        })
        .response
        .interact(Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// Battery level drawn as a small battery, with the percentage and charging state. Added from
/// right to left: place it in a right-to-left layout.
pub fn battery(ui: &mut egui::Ui, battery: &Battery) {
    let p = theme::palette(ui.visuals().dark_mode);
    if battery.fully_charged {
        ui.label(RichText::new("Charged").small().color(p.subtle));
    } else if battery.charging {
        ui.label(theme::icon(glyph::BOLT, 13.0).color(p.subtle))
            .on_hover_text("Charging");
    }
    ui.label(format!("{} %", battery.percent));
    let (rect, _) = ui.allocate_exact_size(Vec2::new(26.0, 13.0), Sense::hover());
    let body = egui::Rect::from_min_max(rect.min, rect.max - Vec2::new(3.0, 0.0));
    let fg = ui.visuals().text_color();
    ui.painter()
        .rect_stroke(body, 2.5, Stroke::new(1.2_f32, fg), StrokeKind::Inside);
    ui.painter().rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(body.right(), body.center().y - 2.5),
            egui::pos2(rect.right(), body.center().y + 2.5),
        ),
        1.0,
        fg,
    );
    let fraction = f32::from(battery.percent.min(100)) / 100.0;
    let inner = body.shrink(2.5);
    let color = if battery.charging || battery.fully_charged {
        Color32::from_rgb(0x3C, 0xC8, 0x5A)
    } else if battery.percent <= 20 {
        Color32::from_rgb(0xE6, 0x3C, 0x3C)
    } else {
        fg
    };
    ui.painter().rect_filled(
        egui::Rect::from_min_size(
            inner.min,
            Vec2::new((inner.width() * fraction).max(1.5), inner.height()),
        ),
        1.0,
        color,
    );
}
