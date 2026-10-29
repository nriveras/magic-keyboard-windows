//! The setting cards below the drawing, and the per-key controls they share with the inspector.

use eframe::egui::{self, RichText};
use mk_core::config::{
    ClearAction, EjectAction, F6Action, FnDoubleTap, FnMode, LockAction, ModPreset, ModSource,
    ModTarget,
};
use mk_core::model::Model;
use mk_core::usage::kbd;
use mk_core::Config;

use super::keyboard::{self, Target};
use super::labels::{self, choice};
use super::theme;
use super::widgets::{self, card, CONTROL_WIDTH};
use crate::appconfig::{AppConfig, LAUNCHER_COUNT};
use crate::system;

/// Dropdown width for modifier cards, which sit two per row.
const MODIFIER_WIDTH: f32 = 130.0;

/// The control for one key, as shown in the inspector.
pub fn target_control(
    ui: &mut egui::Ui,
    target: Target,
    config: &mut Config,
    app: &mut AppConfig,
    width: f32,
) {
    match target {
        Target::Modifier(src) => modifier_choice(ui, src, config, width),
        Target::TopRow => top_row(ui, config, width),
        Target::F6 => {
            ui.label("Top row");
            top_row(ui, config, width);
            ui.add_space(4.0);
            ui.label("F6 in media mode");
            f6(ui, config, width);
        }
        Target::Lock => lock(ui, config, width),
        Target::Eject => eject(ui, config, width),
        Target::Clear => clear(ui, config, width),
        Target::FExt(i) if usize::from(i) >= 3 => {
            let i = usize::from(i) - 3;
            ui.horizontal(|ui| {
                launcher_toggle(ui, i, config, app);
            });
            ui.horizontal(|ui| {
                let browse_w = 70.0;
                launcher_path(ui, i, app, width - browse_w - ui.spacing().item_spacing.x);
                launcher_browse(ui, i, app);
            });
        }
        Target::FExt(i) => f_ext(ui, usize::from(i), config, width),
    }
}

/// Puts a key back to its default.
pub fn reset(target: Target, config: &mut Config, app: &mut AppConfig, defaults: &Config) {
    let app_defaults = AppConfig::default();
    match target {
        Target::Modifier(src) => config.set_modifier(src, defaults.modifier(src)),
        Target::TopRow => config.fn_mode = defaults.fn_mode,
        Target::F6 => config.f6 = defaults.f6,
        Target::Lock => config.lock = defaults.lock,
        Target::Eject => config.eject = defaults.eject,
        Target::Clear => config.clear = defaults.clear,
        Target::FExt(i) => {
            let i = usize::from(i);
            config.f_ext[i] = defaults.f_ext[i];
            if i >= 3 {
                app.launchers[i - 3] = app_defaults.launchers[i - 3].clone();
                app.launchers_enabled[i - 3] = app_defaults.launchers_enabled[i - 3];
            }
        }
    }
}

fn top_row(ui: &mut egui::Ui, config: &mut Config, width: f32) {
    choice(
        ui,
        "fn-mode",
        &mut config.fn_mode,
        FnMode::ALL,
        labels::fn_mode_label,
        width,
    );
}

fn f6(ui: &mut egui::Ui, config: &mut Config, width: f32) {
    choice(
        ui,
        "f6",
        &mut config.f6,
        F6Action::ALL,
        labels::f6_label,
        width,
    );
}

fn lock(ui: &mut egui::Ui, config: &mut Config, width: f32) {
    choice(
        ui,
        "lock",
        &mut config.lock,
        LockAction::ALL,
        labels::lock_label,
        width,
    );
}

fn eject(ui: &mut egui::Ui, config: &mut Config, width: f32) {
    choice(
        ui,
        "eject",
        &mut config.eject,
        EjectAction::ALL,
        labels::eject_label,
        width,
    );
}

fn clear(ui: &mut egui::Ui, config: &mut Config, width: f32) {
    choice(
        ui,
        "clear",
        &mut config.clear,
        ClearAction::ALL,
        labels::clear_label,
        width,
    );
}

fn modifier_choice(ui: &mut egui::Ui, src: ModSource, config: &mut Config, width: f32) {
    let mut target = config.modifier(src);
    choice(
        ui,
        ("mod", src as u8),
        &mut target,
        ModTarget::ALL,
        labels::target_label,
        width,
    );
    config.set_modifier(src, target);
}

fn f_ext(ui: &mut egui::Ui, i: usize, config: &mut Config, width: f32) {
    egui::ComboBox::from_id_salt(("fext", i))
        .selected_text(labels::f_ext_label(config.f_ext[i], i))
        .width(width)
        .show_ui(ui, |ui| {
            for option in labels::f_ext_options(i) {
                ui.selectable_value(&mut config.f_ext[i], option, labels::f_ext_label(option, i));
            }
        });
}

/// An enabled launcher needs its key to reach Windows as itself, so enabling it also resets the
/// driver mapping of that key.
fn launcher_toggle(ui: &mut egui::Ui, i: usize, config: &mut Config, app: &mut AppConfig) {
    let key = 3 + i;
    if widgets::toggle(ui, &mut app.launchers_enabled[i]).changed() && app.launchers_enabled[i] {
        config.f_ext[key] = kbd::F13 + key as u8;
    }
}

fn launcher_path(ui: &mut egui::Ui, i: usize, app: &mut AppConfig, width: f32) {
    ui.add_enabled(
        app.launchers_enabled[i],
        egui::TextEdit::singleline(&mut app.launchers[i])
            .hint_text("Program, file or web address")
            .desired_width(width)
            .margin(egui::vec2(8.0, 6.0)),
    );
}

fn launcher_browse(ui: &mut egui::Ui, i: usize, app: &mut AppConfig) {
    if ui
        .add_enabled(app.launchers_enabled[i], egui::Button::new("Browse…"))
        .on_hover_text("Choose a program or file")
        .clicked()
    {
        if let Some(path) = system::browse_file() {
            app.launchers[i] = path;
        }
    }
}

pub fn keys_section(ui: &mut egui::Ui, config: &mut Config, model: Option<&Model>) {
    widgets::section_header(ui, "Keys");
    card(
        ui,
        "Top row (F1–F12)",
        Some("What the keys do without Fn. Hold Fn for the other function."),
        |ui| top_row(ui, config, CONTROL_WIDTH),
    );
    card(ui, "F6 key", Some("In media-keys mode"), |ui| {
        f6(ui, config, CONTROL_WIDTH)
    });
    if model.is_none_or(|m| !m.has_eject) {
        card(ui, "Lock key", Some("Top-right key (Touch ID)"), |ui| {
            lock(ui, config, CONTROL_WIDTH)
        });
    }
    if model.is_some_and(|m| m.has_eject) {
        card(
            ui,
            "Eject key",
            Some("Top-right key. Fn + Eject = Insert."),
            |ui| eject(ui, config, CONTROL_WIDTH),
        );
    }
    if model.is_none_or(|m| m.has_numpad) {
        card(
            ui,
            "Clear key",
            Some("Top-left key of the numeric keypad"),
            |ui| clear(ui, config, CONTROL_WIDTH),
        );
    }
    card(ui, "Fn double tap", Some("Press Fn twice quickly"), |ui| {
        choice(
            ui,
            "fn2",
            &mut config.fn_double_tap,
            FnDoubleTap::ALL,
            labels::double_tap_label,
            CONTROL_WIDTH,
        );
    });
    card(
        ui,
        "Sleep shortcut",
        Some("Fn, Fn, Fn, Space puts the computer to sleep"),
        |ui| {
            widgets::labeled_toggle(ui, &mut config.sleep_combo);
        },
    );
    widgets::card_body(ui, |ui| {
        egui::CollapsingHeader::new("Built-in Fn shortcuts")
            .id_salt("cheat-sheet")
            .show(ui, |ui| {
                ui.add_space(4.0);
                labels::cheat_sheet(ui);
            });
    });
}

fn preset_name(preset: ModPreset) -> (&'static str, &'static str) {
    match preset {
        ModPreset::WindowsPositions => ("Apple positions", "Command → Win, Option → Alt"),
        ModPreset::PcOrder => ("PC order", "Command → Alt, Option → Win"),
        ModPreset::MacLikeShortcuts => ("Mac-like shortcuts", "Command → Ctrl, Control → Win"),
    }
}

/// Preset dropdown plus one card per physical modifier, left-hand keys beside right-hand keys.
pub fn modifiers_section(ui: &mut egui::Ui, config: &mut Config, model: Option<&Model>) {
    use ModSource::*;
    widgets::section_header(ui, "Modifier keys");
    let current = config.matching_preset();
    let hint = current.map_or("Your own mapping", |p| preset_name(p).1);
    card(ui, "Preset", Some(hint), |ui| {
        let selected = current.map_or("Custom", |p| preset_name(p).0);
        egui::ComboBox::from_id_salt("preset")
            .selected_text(selected)
            .width(CONTROL_WIDTH)
            .show_ui(ui, |ui| {
                for preset in ModPreset::ALL {
                    let (name, hint) = preset_name(preset);
                    if ui
                        .selectable_label(current == Some(preset), name)
                        .on_hover_text(hint)
                        .clicked()
                    {
                        config.apply_preset(preset);
                    }
                }
            });
    });

    let layout = keyboard::layout_for(model);
    let present = |src: ModSource| {
        layout
            .keys
            .iter()
            .any(|k| k.target == Some(Target::Modifier(src)))
    };
    // Left-hand key beside its right-hand twin.
    let rows = [
        (LeftShift, RightShift),
        (LeftControl, RightControl),
        (LeftOption, RightOption),
        (LeftCommand, RightCommand),
        (CapsLock, Fn),
    ];
    let changed = keyboard::Mark::Remapped.color();
    for (a, b) in rows {
        ui.columns(2, |columns| {
            for (ui, src) in columns.iter_mut().zip([a, b]) {
                if !present(src) {
                    continue;
                }
                widgets::card(ui, src.label(), None, |ui| {
                    modifier_choice(ui, src, config, MODIFIER_WIDTH);
                    if config.modifier(src) != src.native_target() {
                        widgets::changed_dot(ui, changed);
                    }
                });
            }
        });
    }
    caption(
        ui,
        "Tip: Right Command → AltGr types the third character of international layouts.",
    );
}

/// F13–F15 remaps and the F16–F19 launchers (numeric-keypad models).
pub fn extra_keys_section(
    ui: &mut egui::Ui,
    config: &mut Config,
    app: &mut AppConfig,
    driver_ok: bool,
) {
    widgets::section_header(ui, "Extra keys (F13–F19)");
    caption(
        ui,
        "F13–F15 sit where PC keyboards have Print Screen, Scroll Lock and Pause. \
         F16–F19 can open a program, file or web address.",
    );
    ui.add_space(4.0);
    ui.add_enabled_ui(driver_ok, |ui| {
        for i in 0..3 {
            card(ui, &format!("F{} key", 13 + i), None, |ui| {
                f_ext(ui, i, config, CONTROL_WIDTH)
            });
        }
    });
    for i in 0..LAUNCHER_COUNT {
        let title = format!("F{} key", 16 + i);
        let description =
            (!app.launchers_enabled[i]).then(|| format!("Off: the key sends F{}", 16 + i));
        widgets::card_with(
            ui,
            |ui| widgets::title_block(ui, &title, description.as_deref()),
            |ui| {
                launcher_browse(ui, i, app);
                launcher_path(ui, i, app, CONTROL_WIDTH + 40.0);
                launcher_toggle(ui, i, config, app);
            },
        );
    }
}

/// Low-battery warning: a switch (0 % = off) and the threshold.
pub fn low_battery(ui: &mut egui::Ui, app: &mut AppConfig, last: &mut u8) {
    const STEPS: [u8; 6] = [5, 10, 15, 20, 25, 30];
    let mut on = app.low_battery > 0;
    if on {
        *last = app.low_battery;
    }
    ui.add_enabled_ui(on, |ui| {
        let mut value = if on { app.low_battery } else { *last };
        egui::ComboBox::from_id_salt("low-battery")
            .selected_text(format!("Below {value} %"))
            .width(120.0)
            .show_ui(ui, |ui| {
                for step in STEPS
                    .into_iter()
                    .chain((!STEPS.contains(&value)).then_some(value))
                {
                    ui.selectable_value(&mut value, step, format!("Below {step} %"));
                }
            });
        if on {
            app.low_battery = value;
        }
    });
    if widgets::toggle(ui, &mut on).changed() {
        app.low_battery = if on { (*last).max(1) } else { 0 };
    }
}

/// Small caption text in the subtle color.
pub fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .small()
            .color(theme::palette(ui.visuals().dark_mode).subtle),
    );
}
