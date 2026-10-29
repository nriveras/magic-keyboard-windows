//! Display names for configuration values, the generic dropdown, and the key-mapping reference.

use eframe::egui;
use mk_core::config::{
    ClearAction, EjectAction, F6Action, FnDoubleTap, FnMode, LockAction, ModTarget,
};
use mk_core::usage::kbd;
use mk_core::Config;

use super::keyboard::Target;
use crate::appconfig::AppConfig;

/// A dropdown over `options`.
pub fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut T,
    options: &[T],
    label: fn(T) -> &'static str,
    width: f32,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(label(*value))
        .width(width)
        .show_ui(ui, |ui| {
            for &option in options {
                ui.selectable_value(value, option, label(option));
            }
        });
}

pub fn fn_mode_label(m: FnMode) -> &'static str {
    match m {
        FnMode::MediaFirst => "Media keys",
        FnMode::FKeysFirst => "F1–F12",
    }
}

pub fn f6_label(a: F6Action) -> &'static str {
    match a {
        F6Action::Sleep => "Sleep",
        F6Action::DoNotDisturb => "Do Not Disturb (Win+N)",
        F6Action::None => "Nothing",
    }
}

pub fn lock_label(a: LockAction) -> &'static str {
    match a {
        LockAction::LockWorkstation => "Lock the computer (Win+L)",
        LockAction::Insert => "Insert",
        LockAction::Disabled => "Nothing",
    }
}

pub fn clear_label(a: ClearAction) -> &'static str {
    match a {
        ClearAction::NumLock => "Num Lock",
        ClearAction::Delete => "Delete",
        ClearAction::Backspace => "Backspace",
        ClearAction::Disabled => "Nothing",
    }
}

pub fn eject_label(a: EjectAction) -> &'static str {
    match a {
        EjectAction::Delete => "Delete",
        EjectAction::Fn => "Extra Fn key",
        EjectAction::Disabled => "Nothing",
    }
}

pub fn double_tap_label(a: FnDoubleTap) -> &'static str {
    match a {
        FnDoubleTap::Off => "Nothing",
        FnDoubleTap::ToggleFnLock => "Switch top-row mode",
        FnDoubleTap::Dictation => "Dictation (Win+H)",
    }
}

pub fn target_label(t: ModTarget) -> &'static str {
    match t {
        ModTarget::Ctrl => "Ctrl",
        ModTarget::Shift => "Shift",
        ModTarget::Alt => "Alt",
        ModTarget::Win => "Win",
        ModTarget::AltGr => "AltGr",
        ModTarget::CapsLock => "Caps Lock",
        ModTarget::Fn => "Fn",
        ModTarget::Menu => "Menu",
        ModTarget::Insert => "Insert",
        ModTarget::Disabled => "Nothing",
    }
}

/// Choices offered for F13–F15: the key itself plus the keys PC keyboards have there.
pub fn f_ext_options(index: usize) -> [u8; 6] {
    let own = kbd::F13 + index as u8;
    [
        own,
        kbd::PRINT_SCREEN,
        kbd::SCROLL_LOCK,
        kbd::PAUSE,
        kbd::INSERT,
        0,
    ]
}

pub fn f_ext_label(usage: u8, index: usize) -> String {
    let own = kbd::F13 + index as u8;
    match usage {
        0 => "Nothing".into(),
        u if u == own => format!("F{} (itself)", 13 + index),
        kbd::PRINT_SCREEN => "Print Screen".into(),
        kbd::SCROLL_LOCK => "Scroll Lock".into(),
        kbd::PAUSE => "Pause / Break".into(),
        kbd::INSERT => "Insert".into(),
        u => format!("usage {u:#04x}"),
    }
}

/// Name of the key a target belongs to.
pub fn title(target: Target) -> String {
    match target {
        Target::Modifier(src) => src.label().to_string(),
        Target::TopRow => "F1–F12".into(),
        Target::F6 => "F6".into(),
        Target::Lock => "Lock key".into(),
        Target::Eject => "Eject key".into(),
        Target::Clear => "Keypad Clear key".into(),
        Target::FExt(i) => format!("F{}", 13 + i),
    }
}

/// What the key's setting controls, shown under its name in the inspector.
pub fn description(target: Target) -> &'static str {
    match target {
        Target::Modifier(_) => "The key Windows receives when you press it.",
        Target::TopRow => "What the top row does without Fn. Hold Fn for the other function.",
        Target::F6 => "The top-row mode, and what F6 does in media mode.",
        Target::Lock => "The key in the top-right corner (Touch ID).",
        Target::Eject => "The key in the top-right corner. Fn + Eject = Insert.",
        Target::Clear => "The top-left key of the numeric keypad.",
        Target::FExt(i) if i >= 3 => "Open a program, file or web address with this key.",
        Target::FExt(_) => "PC keyboards have these keys here.",
    }
}

/// What a key's own setting currently is (for F6, its media-mode action).
pub fn value(target: Target, config: &Config, app: &AppConfig) -> String {
    match target {
        Target::Modifier(src) => target_label(config.modifier(src)).to_string(),
        Target::TopRow => fn_mode_label(config.fn_mode).to_string(),
        Target::F6 => f6_label(config.f6).to_string(),
        Target::Lock => lock_label(config.lock).to_string(),
        Target::Eject => eject_label(config.eject).to_string(),
        Target::Clear => clear_label(config.clear).to_string(),
        Target::FExt(i) if i >= 3 => match app.launcher(usize::from(i - 3)) {
            Some(target) => format!("Opens {target}"),
            None => format!("F{} (itself)", 13 + i),
        },
        Target::FExt(i) => f_ext_label(config.f_ext[usize::from(i)], usize::from(i)),
    }
}

/// One-line description of what a key currently does (hover text on the drawing).
pub fn summary(target: Target, config: &Config, app: &AppConfig) -> String {
    let value = value(target, config, app);
    match target {
        Target::F6 => format!("F6 → {value} ({})", fn_mode_label(config.fn_mode)),
        _ => format!("{} → {value}", title(target)),
    }
}

/// Reference of every built-in combination.
pub fn cheat_sheet(ui: &mut egui::Ui) {
    let table = |ui: &mut egui::Ui, id: &str, rows: &[(&str, &str)]| {
        egui::Grid::new(id)
            .num_columns(2)
            .striped(true)
            .spacing([24.0, 4.0])
            .show(ui, |ui| {
                for (keys, result) in rows {
                    ui.label(*keys);
                    ui.label(*result);
                    ui.end_row();
                }
            });
    };
    ui.heading("Fn combinations");
    table(
        ui,
        "cheat-fn",
        &[
            ("Fn + ⌫", "Delete"),
            ("Fn + ← / →", "Home / End"),
            ("Fn + ↑ / ↓", "Page Up / Page Down"),
            ("Fn + Return", "Insert"),
            ("Fn + Esc", "Pause"),
            ("Fn + P", "Print Screen"),
            ("Fn + Left Shift + F11", "Print Screen"),
            ("Fn + Left Shift + F12", "Scroll Lock"),
            ("Fn + Ctrl + Option + ⌫", "Ctrl + Alt + Del"),
            ("Fn + F1–F12", "the other top-row function"),
            ("Fn, Fn, Fn, Space", "sleep (if enabled)"),
        ],
    );
    ui.add_space(12.0);
    ui.heading("Top row (media mode)");
    table(
        ui,
        "cheat-top",
        &[
            ("F1 / F2", "brightness down / up"),
            ("F3", "Task View"),
            ("F4", "Search"),
            ("F5", "Dictation"),
            ("F6", "sleep or Do Not Disturb"),
            ("F7 / F8 / F9", "previous / play-pause / next"),
            ("Shift + F7 / F8 / F9", "rewind / stop / fast-forward"),
            ("F10 / F11 / F12", "mute / volume down / volume up"),
            ("Lock key", "lock the computer"),
        ],
    );
}
