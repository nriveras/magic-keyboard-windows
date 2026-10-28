//! The grouped panels beside the drawing, and the per-key controls they share with the key popups.

use eframe::egui::{self, Align, Layout, RichText};
use mk_core::config::{
    ClearAction, EjectAction, F6Action, FnDoubleTap, FnMode, LockAction, ModPreset, ModSource,
    ModTarget,
};
use mk_core::model::Model;
use mk_core::usage::kbd;
use mk_core::Config;

use super::keyboard::Target;
use super::labels::{self, choice};
use crate::appconfig::{AppConfig, LAUNCHER_COUNT};
use crate::system;

const WIDE: f32 = 290.0;
const NARROW: f32 = 170.0;

/// A titled group box.
pub fn section(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).heading());
        ui.add_space(4.0);
        add(ui);
    });
    ui.add_space(8.0);
}

/// The control for one key. Used by the panels and by the popup on the drawn keyboard.
pub fn target_control(ui: &mut egui::Ui, target: Target, config: &mut Config, app: &mut AppConfig) {
    match target {
        Target::Modifier(src) => modifier_choice(ui, src, config, NARROW),
        Target::TopRow => top_row(ui, config),
        Target::F6 => {
            top_row(ui, config);
            ui.horizontal(|ui| {
                ui.label("F6 action");
                choice(
                    ui,
                    "f6",
                    &mut config.f6,
                    F6Action::ALL,
                    labels::f6_label,
                    NARROW,
                );
            });
        }
        Target::Lock => choice(
            ui,
            "lock",
            &mut config.lock,
            LockAction::ALL,
            labels::lock_label,
            WIDE,
        ),
        Target::Eject => choice(
            ui,
            "eject",
            &mut config.eject,
            EjectAction::ALL,
            labels::eject_label,
            WIDE,
        ),
        Target::Clear => choice(
            ui,
            "clear",
            &mut config.clear,
            ClearAction::ALL,
            labels::clear_label,
            NARROW,
        ),
        Target::FExt(i) if usize::from(i) >= 3 => launcher(ui, usize::from(i) - 3, config, app),
        Target::FExt(i) => f_ext(ui, usize::from(i), config),
    }
}

fn top_row(ui: &mut egui::Ui, config: &mut Config) {
    choice(
        ui,
        "fn-mode",
        &mut config.fn_mode,
        FnMode::ALL,
        labels::fn_mode_label,
        WIDE,
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

fn f_ext(ui: &mut egui::Ui, i: usize, config: &mut Config) {
    ui.horizontal(|ui| {
        ui.label(format!("F{} =", 13 + i));
        egui::ComboBox::from_id_salt(("fext", i))
            .selected_text(labels::f_ext_label(config.f_ext[i], i))
            .width(NARROW)
            .show_ui(ui, |ui| {
                for option in labels::f_ext_options(i) {
                    ui.selectable_value(
                        &mut config.f_ext[i],
                        option,
                        labels::f_ext_label(option, i),
                    );
                }
            });
    });
}

/// F16–F19: enable + target + browse. An enabled launcher needs its key to reach Windows as
/// itself, so enabling it also resets the driver mapping of that key.
fn launcher(ui: &mut egui::Ui, i: usize, config: &mut Config, app: &mut AppConfig) {
    ui.horizontal(|ui| {
        let key = 3 + i;
        if ui
            .checkbox(&mut app.launchers_enabled[i], format!("F{} =", 16 + i))
            .changed()
            && app.launchers_enabled[i]
        {
            config.f_ext[key] = kbd::F13 + key as u8;
        }
        ui.add_enabled(
            app.launchers_enabled[i],
            egui::TextEdit::singleline(&mut app.launchers[i])
                .hint_text("program, file or web address")
                .desired_width(190.0),
        );
        if ui
            .add_enabled(app.launchers_enabled[i], egui::Button::new("…"))
            .on_hover_text("Choose a program or file")
            .clicked()
        {
            if let Some(path) = system::browse_file() {
                app.launchers[i] = path;
            }
        }
    });
}

pub fn top_row_panel(ui: &mut egui::Ui, config: &mut Config) {
    section(ui, "Top row (F1–F12)", |ui| {
        ui.label("Without Fn, the top row keys act as:");
        top_row(ui, config);
    });
}

pub fn extended_f_keys_panel(ui: &mut egui::Ui, config: &mut Config, app: &mut AppConfig) {
    section(ui, "F13–F19", |ui| {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                for i in 0..3 {
                    f_ext(ui, i, config);
                }
            });
            ui.add_space(12.0);
            ui.vertical(|ui| {
                for i in 0..LAUNCHER_COUNT {
                    launcher(ui, i, config, app);
                }
            });
        });
    });
}

pub fn other_keys_panel(
    ui: &mut egui::Ui,
    config: &mut Config,
    model: Option<&Model>,
    show_mappings: &mut bool,
) {
    section(ui, "Other keys", |ui| {
        egui::Grid::new("other-keys")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                if model.is_none_or(|m| !m.has_eject) {
                    ui.label("Lock key");
                    choice(
                        ui,
                        "lock",
                        &mut config.lock,
                        LockAction::ALL,
                        labels::lock_label,
                        WIDE,
                    );
                    ui.end_row();
                }
                if model.is_some_and(|m| m.has_eject) {
                    ui.label("Eject key");
                    choice(
                        ui,
                        "eject",
                        &mut config.eject,
                        EjectAction::ALL,
                        labels::eject_label,
                        WIDE,
                    );
                    ui.end_row();
                }
                if model.is_none_or(|m| m.has_numpad) {
                    ui.label("Clear key");
                    choice(
                        ui,
                        "clear",
                        &mut config.clear,
                        ClearAction::ALL,
                        labels::clear_label,
                        WIDE,
                    );
                    ui.end_row();
                }
                ui.label("F6 action");
                choice(
                    ui,
                    "f6",
                    &mut config.f6,
                    F6Action::ALL,
                    labels::f6_label,
                    WIDE,
                );
                ui.end_row();
                ui.label("Fn double tap");
                choice(
                    ui,
                    "fn2",
                    &mut config.fn_double_tap,
                    FnDoubleTap::ALL,
                    labels::double_tap_label,
                    WIDE,
                );
                ui.end_row();
            });
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut config.sleep_combo,
                "Fn, Fn, Fn, Space puts the computer to sleep",
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Show all key mappings").clicked() {
                    *show_mappings = true;
                }
            });
        });
    });
}

/// Modifier dropdowns laid out like the keyboard's lower-left and bottom rows.
pub fn modifier_panel(ui: &mut egui::Ui, config: &mut Config, model: Option<&Model>) {
    use ModSource::*;
    let numpad = model.is_none_or(|m| m.has_numpad);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new("Modifier keys").heading());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.menu_button("Presets ≡", |ui| {
                    for (preset, name, hint) in [
                        (
                            ModPreset::WindowsPositions,
                            "Apple positions",
                            "Command → Win, Option → Alt",
                        ),
                        (
                            ModPreset::PcOrder,
                            "PC order",
                            "Command → Alt, Option → Win",
                        ),
                        (
                            ModPreset::MacLikeShortcuts,
                            "Mac-like shortcuts",
                            "Command → Ctrl, Control → Win",
                        ),
                    ] {
                        if ui.button(name).on_hover_text(hint).clicked() {
                            config.apply_preset(preset);
                            ui.close();
                        }
                    }
                });
            });
        });
        ui.add_space(4.0);

        let side = 110.0;
        modifier_choice(ui, CapsLock, config, side);
        ui.horizontal(|ui| {
            modifier_choice(ui, LeftShift, config, side);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                modifier_choice(ui, RightShift, config, side);
            });
        });

        // Bottom row: dropdowns share the width left after a fixed "space" placeholder.
        let row: &[Option<ModSource>] = if numpad {
            &[
                Some(LeftControl),
                Some(LeftOption),
                Some(LeftCommand),
                None,
                Some(RightCommand),
                Some(RightOption),
                Some(RightControl),
            ]
        } else {
            &[
                Some(Fn),
                Some(LeftControl),
                Some(LeftOption),
                Some(LeftCommand),
                None,
                Some(RightCommand),
                Some(RightOption),
            ]
        };
        let gap = ui.spacing().item_spacing.x;
        let space_w = 70.0;
        let combos = row.iter().filter(|s| s.is_some()).count() as f32;
        // Slightly under the exact share so rounding never wraps the row.
        let each = ((ui.available_width() - space_w - gap * (row.len() as f32 - 1.0)) / combos
            - 1.0)
            .max(40.0);
        ui.horizontal(|ui| {
            for src in row {
                match src {
                    Some(src) => modifier_choice(ui, *src, config, each),
                    None => {
                        ui.add_enabled(
                            false,
                            egui::Button::new("space").min_size(egui::vec2(space_w, 0.0)),
                        );
                    }
                }
            }
        });
        if numpad {
            ui.horizontal(|ui| {
                ui.label("fn (above Delete)");
                modifier_choice(ui, Fn, config, side);
            });
        }
        ui.add_space(2.0);
        ui.weak("Tip: Right Command → altgr types the third character of international layouts.");
    });
}
