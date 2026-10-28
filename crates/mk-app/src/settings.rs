//! Settings window (`mk-app settings`), a separate process started from the tray menu so the tray
//! keeps its plain Win32 message loop. Driver options go through the control interface; app
//! options are saved to app.toml and the tray is told to reload them.

use eframe::egui;
use mk_core::config::{
    ClearAction, EjectAction, F6Action, FnDoubleTap, FnMode, LockAction, ModPreset, ModSource,
    ModTarget, F_EXT_COUNT,
};
use mk_core::usage::kbd;
use mk_core::{model, Config};
use mk_hid::control::Control;

use crate::appconfig::{self, AppConfig, LAUNCHER_COUNT};
use crate::system;

pub fn run() {
    let Some(_instance) = system::named_mutex("Local\\MagicKeyboardWindowsSettings") else {
        return; // already open
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Magic Keyboard settings")
            .with_inner_size([620.0, 640.0])
            .with_min_inner_size([520.0, 420.0]),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "Magic Keyboard settings",
        options,
        Box::new(|_cc| Ok(Box::new(Settings::load()))),
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Keys,
    Modifiers,
    App,
    CheatSheet,
}

struct Settings {
    tab: Tab,
    /// Driver config as last read or applied; `Err` if the driver is unreachable.
    driver: Result<Config, String>,
    edited: Config,
    /// Model of the most recently attached keyboard, for model defaults and Eject visibility.
    model: Option<&'static model::Model>,
    app: AppConfig,
    saved_app: AppConfig,
    autostart: bool,
    /// Result of the last Apply (`true` = success).
    message: Option<(bool, String)>,
}

impl Settings {
    fn load() -> Settings {
        let control = Control::open();
        let model = control
            .as_ref()
            .ok()
            .and_then(|c| c.status().ok())
            .and_then(|s| model::find(model::APPLE_USB_VID, s.last_pid));
        let driver = control
            .and_then(|c| c.config())
            .map_err(|e| format!("Driver not reachable: {e}"));
        let edited = driver.clone().unwrap_or_default();
        let app = appconfig::load();
        Settings {
            tab: Tab::Keys,
            driver,
            edited,
            model,
            saved_app: app.clone(),
            app,
            autostart: system::autostart_enabled(),
            message: None,
        }
    }

    fn dirty(&self) -> bool {
        self.driver.as_ref().is_ok_and(|c| *c != self.edited) || self.app != self.saved_app
    }

    fn apply(&mut self) {
        let mut errors = Vec::new();
        if self.driver.is_ok() {
            match Control::open().and_then(|c| c.set_config(&self.edited)) {
                Ok(()) => self.driver = Ok(self.edited),
                Err(e) => errors.push(format!("driver: {e}")),
            }
        }
        if self.app != self.saved_app {
            match appconfig::save(&self.app) {
                Ok(()) => {
                    self.saved_app = self.app.clone();
                    system::notify_tray_reload();
                }
                Err(e) => errors.push(format!("app settings: {e}")),
            }
        }
        self.message = Some(if errors.is_empty() {
            (true, "Saved.".into())
        } else {
            (false, errors.join("; "))
        });
    }

    fn revert(&mut self) {
        if let Ok(config) = &self.driver {
            self.edited = *config;
        }
        self.app = self.saved_app.clone();
        self.message = None;
    }

    fn defaults(&mut self) {
        self.edited = self.model.map(Config::default_for).unwrap_or_default();
        self.app = AppConfig::default();
        self.message = None;
    }
}

impl eframe::App for Settings {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Keys, "Keys");
                ui.selectable_value(&mut self.tab, Tab::Modifiers, "Modifiers");
                ui.selectable_value(&mut self.tab, Tab::App, "App");
                ui.selectable_value(&mut self.tab, Tab::CheatSheet, "Cheat sheet");
            });
        });
        egui::TopBottomPanel::bottom("actions").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let dirty = self.dirty();
                if ui.add_enabled(dirty, egui::Button::new("Apply")).clicked() {
                    self.apply();
                }
                if ui.add_enabled(dirty, egui::Button::new("Revert")).clicked() {
                    self.revert();
                }
                if ui.button("Defaults").clicked() {
                    self.defaults();
                }
                match &self.message {
                    Some((true, text)) => {
                        ui.label(text);
                    }
                    Some((false, text)) => {
                        ui.colored_label(egui::Color32::RED, text);
                    }
                    None if dirty => {
                        ui.label("Unsaved changes");
                    }
                    None => {}
                }
            });
            ui.add_space(4.0);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.tab {
                Tab::Keys => self.keys_tab(ui),
                Tab::Modifiers => self.modifiers_tab(ui),
                Tab::App => self.app_tab(ui),
                Tab::CheatSheet => cheat_sheet(ui),
            });
        });
    }
}

/// A labelled combo box over `options`.
fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut T,
    options: &[T],
    label: fn(T) -> &'static str,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(label(*value))
        .width(260.0)
        .show_ui(ui, |ui| {
            for &option in options {
                ui.selectable_value(value, option, label(option));
            }
        });
}

impl Settings {
    fn driver_unavailable(&self, ui: &mut egui::Ui) -> bool {
        if let Err(e) = &self.driver {
            ui.colored_label(egui::Color32::RED, e);
            ui.label("Install the driver (scripts\\install-dev.ps1) and reopen this window.");
            return true;
        }
        false
    }

    fn keys_tab(&mut self, ui: &mut egui::Ui) {
        if self.driver_unavailable(ui) {
            return;
        }
        let c = &mut self.edited;
        ui.heading("Top row");
        ui.radio_value(
            &mut c.fn_mode,
            FnMode::MediaFirst,
            "Media keys (hold Fn for F1–F12)",
        );
        ui.radio_value(
            &mut c.fn_mode,
            FnMode::FKeysFirst,
            "F1–F12 (hold Fn for media keys)",
        );
        ui.add_space(8.0);

        egui::Grid::new("keys")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("F6 key");
                choice(ui, "f6", &mut c.f6, F6Action::ALL, f6_label);
                ui.end_row();
                ui.label("Lock / Touch ID key");
                choice(ui, "lock", &mut c.lock, LockAction::ALL, lock_label);
                ui.end_row();
                if self.model.is_none_or(|m| m.has_numpad) {
                    ui.label("Keypad Clear key");
                    choice(ui, "clear", &mut c.clear, ClearAction::ALL, clear_label);
                    ui.end_row();
                }
                if self.model.is_none_or(|m| m.has_eject) {
                    ui.label("Eject key (older models)");
                    choice(ui, "eject", &mut c.eject, EjectAction::ALL, eject_label);
                    ui.end_row();
                }
                ui.label("Fn double tap");
                choice(
                    ui,
                    "fn2",
                    &mut c.fn_double_tap,
                    FnDoubleTap::ALL,
                    double_tap_label,
                );
                ui.end_row();
            });
        ui.checkbox(
            &mut c.sleep_combo,
            "Fn, Fn, Fn, Space puts the computer to sleep",
        );
        ui.add_space(12.0);

        ui.heading("F13–F19");
        ui.label("F16–F19 must stay on their own key for the launchers (App tab) to work.");
        egui::Grid::new("fext")
            .num_columns(2)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                for i in 0..F_EXT_COUNT {
                    let own = kbd::F13 + i as u8;
                    let options = [
                        own,
                        kbd::PRINT_SCREEN,
                        kbd::SCROLL_LOCK,
                        kbd::PAUSE,
                        kbd::INSERT,
                        0,
                    ];
                    ui.label(format!("F{}", 13 + i));
                    egui::ComboBox::from_id_salt(("fext", i))
                        .selected_text(f_ext_label(c.f_ext[i], own))
                        .width(260.0)
                        .show_ui(ui, |ui| {
                            for option in options {
                                ui.selectable_value(
                                    &mut c.f_ext[i],
                                    option,
                                    f_ext_label(option, own),
                                );
                            }
                        });
                    ui.end_row();
                }
            });
    }

    fn modifiers_tab(&mut self, ui: &mut egui::Ui) {
        if self.driver_unavailable(ui) {
            return;
        }
        let c = &mut self.edited;
        ui.heading("Presets");
        ui.horizontal_wrapped(|ui| {
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
                    c.apply_preset(preset);
                }
            }
        });
        ui.add_space(8.0);
        ui.heading("Each key");
        egui::Grid::new("mods")
            .num_columns(2)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                for src in ModSource::ALL {
                    ui.label(src.label());
                    let mut target = c.modifier(src);
                    choice(ui, src.label(), &mut target, ModTarget::ALL, target_label);
                    c.set_modifier(src, target);
                    ui.end_row();
                }
            });
        ui.add_space(4.0);
        ui.label("Tip: Right Command → AltGr types the third character of international layouts.");
    }

    fn app_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Launchers");
        ui.label("Program, document or web address opened by each key.");
        egui::Grid::new("launch")
            .num_columns(2)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                for i in 0..LAUNCHER_COUNT {
                    ui.label(format!("F{}", 16 + i));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.app.launchers[i]).desired_width(360.0),
                    );
                    ui.end_row();
                }
            });
        ui.add_space(12.0);
        ui.heading("Battery");
        ui.add(
            egui::Slider::new(&mut self.app.low_battery, 0..=50)
                .text("% low-battery warning (0 = off)"),
        );
        ui.add_space(12.0);
        ui.heading("Startup");
        // Applied immediately; it is not part of Apply/Revert.
        if ui
            .checkbox(&mut self.autostart, "Start with Windows")
            .changed()
        {
            if let Err(e) = system::set_autostart(self.autostart) {
                self.message = Some((false, format!("autostart: {e}")));
                self.autostart = system::autostart_enabled();
            }
        }
        if let Some(path) = appconfig::path() {
            ui.add_space(12.0);
            ui.weak(format!("App settings file: {}", path.display()));
        }
    }
}

fn cheat_sheet(ui: &mut egui::Ui) {
    ui.heading("Fn combinations");
    egui::Grid::new("cheat")
        .num_columns(2)
        .striped(true)
        .spacing([24.0, 4.0])
        .show(ui, |ui| {
            for (keys, result) in [
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
            ] {
                ui.label(keys);
                ui.label(result);
                ui.end_row();
            }
        });
    ui.add_space(12.0);
    ui.heading("Top row (media mode)");
    egui::Grid::new("toprow")
        .num_columns(2)
        .striped(true)
        .spacing([24.0, 4.0])
        .show(ui, |ui| {
            for (keys, result) in [
                ("F1 / F2", "brightness down / up"),
                ("F3", "Task View"),
                ("F4", "Search"),
                ("F5", "Dictation"),
                ("F6", "sleep or Do Not Disturb"),
                ("F7 / F8 / F9", "previous / play-pause / next"),
                ("Shift + F7 / F8 / F9", "rewind / stop / fast-forward"),
                ("F10 / F11 / F12", "mute / volume down / volume up"),
                ("Lock key", "lock the computer"),
            ] {
                ui.label(keys);
                ui.label(result);
                ui.end_row();
            }
        });
}

fn f6_label(a: F6Action) -> &'static str {
    match a {
        F6Action::Sleep => "Sleep",
        F6Action::DoNotDisturb => "Do Not Disturb (Win+N)",
        F6Action::None => "Nothing",
    }
}

fn lock_label(a: LockAction) -> &'static str {
    match a {
        LockAction::LockWorkstation => "Lock the computer (Win+L)",
        LockAction::Insert => "Insert",
        LockAction::Disabled => "Nothing",
    }
}

fn clear_label(a: ClearAction) -> &'static str {
    match a {
        ClearAction::NumLock => "Num Lock",
        ClearAction::Delete => "Delete",
        ClearAction::Backspace => "Backspace",
        ClearAction::Disabled => "Nothing",
    }
}

fn eject_label(a: EjectAction) -> &'static str {
    match a {
        EjectAction::Delete => "Delete (Fn + Eject = Insert)",
        EjectAction::Fn => "Extra Fn key",
        EjectAction::Disabled => "Nothing",
    }
}

fn double_tap_label(a: FnDoubleTap) -> &'static str {
    match a {
        FnDoubleTap::Off => "Nothing",
        FnDoubleTap::ToggleFnLock => "Switch the top-row mode",
        FnDoubleTap::Dictation => "Dictation (Win+H)",
    }
}

fn target_label(t: ModTarget) -> &'static str {
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

fn f_ext_label(usage: u8, own: u8) -> String {
    match usage {
        0 => "Nothing".into(),
        u if u == own => format!("F{} (itself)", 13 + (own - kbd::F13)),
        kbd::PRINT_SCREEN => "Print Screen".into(),
        kbd::SCROLL_LOCK => "Scroll Lock".into(),
        kbd::PAUSE => "Pause".into(),
        kbd::INSERT => "Insert".into(),
        u => format!("usage {u:#04x}"),
    }
}
