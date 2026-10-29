//! Settings window (`mk-app settings`), a separate process started from the tray menu so the tray
//! keeps its plain Win32 message loop.
//!
//! One tab per connected keyboard shows a drawing of that model with its battery; configurable keys
//! are highlighted and open their control when clicked. The panels on the right hold the same
//! controls grouped by kind. Mappings are shared by all keyboards (the driver keeps one config).
//! Driver options go through the control interface; app options are saved to app.toml and the
//! tray is told to reload them.

mod keyboard;
mod labels;
mod panels;

use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText};
use mk_core::{model, Config};
use mk_hid::control::Control;

use crate::appconfig::{self, AppConfig};
use crate::devices::{self, Keyboard};
use crate::system;
use keyboard::Target;

/// How often the keyboard list and batteries are refreshed while the window is open.
const REFRESH: Duration = Duration::from_secs(10);

pub fn run() {
    let Some(_instance) = system::named_mutex("Local\\MagicKeyboardWindowsSettings") else {
        return; // already open
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Magic Keyboard settings")
            .with_inner_size([1180.0, 720.0])
            .with_min_inner_size([980.0, 600.0]),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "Magic Keyboard settings",
        options,
        Box::new(|cc| {
            use_system_fonts(&cc.egui_ctx);
            Ok(Box::new(Settings::load()))
        }),
    );
}

/// Segoe UI for text, with Segoe UI Symbol as fallback for arrows and other symbols the bundled
/// fonts lack. Keeps the bundled fonts if the files are missing.
fn use_system_fonts(ctx: &egui::Context) {
    let fonts_dir = std::env::var_os("WINDIR")
        .map(|w| std::path::PathBuf::from(w).join("Fonts"))
        .unwrap_or_else(|| "C:\\Windows\\Fonts".into());
    let mut fonts = egui::FontDefinitions::default();
    let proportional = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default();
    let mut position = 0;
    for (name, file) in [
        ("segoe-ui", "segoeui.ttf"),
        ("segoe-ui-symbol", "seguisym.ttf"),
    ] {
        if let Ok(bytes) = std::fs::read(fonts_dir.join(file)) {
            fonts.font_data.insert(
                name.into(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            proportional.insert(position, name.into());
            position += 1;
        }
    }
    ctx.set_fonts(fonts);
}

struct Settings {
    /// Index into `keyboards` (0 when none is connected).
    tab: usize,
    keyboards: Vec<Keyboard>,
    /// Start of the last device poll (`None` = not polled yet).
    refreshed: Option<Instant>,
    /// Result of the device poll in progress.
    devices_rx: Option<mpsc::Receiver<Vec<Keyboard>>>,
    /// Driver config as last read or applied; `Err` if the driver is unreachable.
    driver: Result<Config, String>,
    edited: Config,
    /// Model of the most recently attached keyboard, for model defaults.
    last_model: Option<&'static model::Model>,
    app: AppConfig,
    saved_app: AppConfig,
    autostart: bool,
    /// Key popup: target, anchor rectangle, and the pass in which it opened.
    popup: Option<(Target, egui::Rect, u64)>,
    show_mappings: bool,
    /// Result of the last Apply (`true` = success).
    message: Option<(bool, String)>,
}

impl Settings {
    fn load() -> Settings {
        let control = Control::open();
        let last_model = control
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
            tab: 0,
            keyboards: Vec::new(),
            refreshed: None,
            devices_rx: None,
            driver,
            edited,
            last_model,
            saved_app: app.clone(),
            app,
            autostart: system::autostart_enabled(),
            popup: None,
            show_mappings: false,
            message: None,
        }
    }

    fn dirty(&self) -> bool {
        self.driver.as_ref().is_ok_and(|c| *c != self.edited) || self.app != self.saved_app
    }

    fn apply(&mut self) {
        let mut errors = Vec::new();
        // Only when a driver option was edited, so an app-only change never rewrites the driver.
        if let Some(snapshot) = self.driver.clone().ok().filter(|c| *c != self.edited) {
            let result = Control::open().and_then(|c| {
                let current = c.config()?;
                let config = driver_update(&snapshot, &self.edited, &current).unwrap_or(current);
                c.set_config(&config).map(|()| config)
            });
            match result {
                Ok(config) => {
                    self.driver = Ok(config);
                    self.edited = config;
                }
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
        let model = self.model().or(self.last_model);
        self.edited = model.map(Config::default_for).unwrap_or_default();
        self.app = AppConfig::default();
        self.message = None;
    }

    /// Model shown in the current tab.
    fn model(&self) -> Option<&'static model::Model> {
        self.keyboards.get(self.tab).and_then(|k| k.model)
    }

    /// Polls the keyboards on a worker thread (battery reads can block) and picks up the result.
    fn refresh_devices(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.devices_rx {
            match rx.try_recv() {
                Ok(keyboards) => {
                    self.keyboards = keyboards;
                    if self.tab >= self.keyboards.len() {
                        self.tab = 0;
                    }
                    self.devices_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => self.devices_rx = None,
            }
        }
        if self.refreshed.is_none_or(|t| t.elapsed() >= REFRESH) {
            self.refreshed = Some(Instant::now());
            let (tx, rx) = mpsc::channel();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(devices::connected());
                ctx.request_repaint();
            });
            self.devices_rx = Some(rx);
        }
    }

    fn tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.keyboards.is_empty() {
                let _ = ui.selectable_label(
                    true,
                    RichText::new("● No keyboard connected").color(Color32::GRAY),
                );
            }
            for (i, k) in self.keyboards.iter().enumerate() {
                let text = RichText::new(format!("{}  ·  {}", k.name(), k.transport()));
                let dot = RichText::new("● ").color(Color32::from_rgb(0x2E, 0xB8, 0x4A));
                let mut job = egui::text::LayoutJob::default();
                dot.append_to(
                    &mut job,
                    ui.style(),
                    egui::FontSelection::Default,
                    egui::Align::Center,
                );
                text.append_to(
                    &mut job,
                    ui.style(),
                    egui::FontSelection::Default,
                    egui::Align::Center,
                );
                if ui.selectable_label(self.tab == i, job).clicked() {
                    self.tab = i;
                    self.popup = None;
                }
            }
        });
    }

    fn device_column(&mut self, ui: &mut egui::Ui) {
        let model = self.model();
        let keyboard = self.keyboards.get(self.tab);
        panels::section(ui, "Device", |ui| {
            ui.vertical_centered(|ui| match keyboard {
                Some(k) => {
                    ui.label(RichText::new(k.name()).strong());
                    ui.weak(format!("{} · rechargeable", k.transport()));
                }
                None => {
                    ui.label(RichText::new("No Magic Keyboard connected").strong());
                    ui.weak("Showing the numeric-keypad layout");
                }
            });
            ui.add_space(8.0);
            let layout = keyboard::layout_for(model);
            let selected = self.popup.map(|(t, _, _)| t);
            if let Some((target, rect)) =
                keyboard::show(ui, &layout, &self.edited, &self.app, selected)
            {
                self.popup = Some((target, rect, ui.ctx().cumulative_pass_nr()));
            }
            ui.add_space(4.0);
            keyboard::legend(ui);
            ui.add_space(8.0);
            match keyboard.and_then(|k| k.battery) {
                Some(b) => {
                    ui.add(
                        egui::ProgressBar::new(f32::from(b.percent) / 100.0)
                            .text(format!("{}%", b.percent)),
                    );
                    ui.vertical_centered(|ui| {
                        ui.label(if b.fully_charged {
                            "Battery is fully charged."
                        } else if b.charging {
                            "Battery is charging."
                        } else {
                            "Battery is discharging."
                        });
                    });
                }
                None => {
                    ui.weak("Battery state unavailable.");
                }
            }
        });

        panels::section(ui, "App", |ui| {
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
            ui.add(
                egui::Slider::new(&mut self.app.low_battery, 0..=50)
                    .text("% low-battery warning (0 = off)"),
            );
            if let Some(path) = appconfig::path() {
                ui.weak(format!("App settings file: {}", path.display()));
            }
        });
    }

    fn mapping_column(&mut self, ui: &mut egui::Ui) {
        if let Err(e) = &self.driver {
            panels::section(ui, "Key mappings", |ui| {
                ui.colored_label(Color32::RED, e);
                ui.label("Install the driver (scripts\\install-dev.ps1) and reopen this window.");
            });
            return;
        }
        let model = self.model();
        ui.weak("Settings apply to all connected Magic Keyboards.");
        ui.add_space(4.0);
        panels::top_row_panel(ui, &mut self.edited);
        if model.is_none_or(|m| m.has_numpad) {
            panels::extended_f_keys_panel(ui, &mut self.edited, &mut self.app);
        }
        panels::other_keys_panel(ui, &mut self.edited, model, &mut self.show_mappings);
        panels::modifier_panel(ui, &mut self.edited, model);
    }

    fn key_popup(&mut self, ctx: &egui::Context) {
        let Some((target, rect, opened)) = self.popup else {
            return;
        };
        if self.driver.is_err() && !matches!(target, Target::FExt(i) if i >= 3) {
            self.popup = None;
            return;
        }
        let area = egui::Area::new(egui::Id::new("key-popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.left_bottom() + egui::vec2(0.0, 6.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new(labels::title(target)).strong());
                    panels::target_control(ui, target, &mut self.edited, &mut self.app);
                });
            });
        // The click that opened the popup lands outside it; ignore that pass.
        let outside = area.response.clicked_elsewhere() && ctx.cumulative_pass_nr() > opened;
        if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.popup = None;
        }
    }
}

impl eframe::App for Settings {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.refresh_devices(ctx);
        ctx.request_repaint_after(REFRESH);

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.add_space(4.0);
            self.tabs(ui);
            ui.add_space(2.0);
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
                        ui.colored_label(Color32::RED, text);
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
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.columns(2, |columns| {
                    self.device_column(&mut columns[0]);
                    self.mapping_column(&mut columns[1]);
                });
            });
        });
        self.key_popup(ctx);
        egui::Window::new("All key mappings")
            .open(&mut self.show_mappings)
            .resizable(false)
            .collapsible(false)
            .show(ctx, labels::cheat_sheet);
    }
}

/// The config to send when Apply is clicked: `None` if no driver option was edited since the
/// window read `snapshot`; otherwise only the edited fields, applied to the driver's `current`
/// config so changes made meanwhile (tray menu, Fn double-tap) are kept.
fn driver_update(snapshot: &Config, edited: &Config, current: &Config) -> Option<Config> {
    (edited != snapshot).then(|| current.merge_changes(snapshot, edited))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::config::{FnMode, ModSource, ModTarget};

    #[test]
    fn app_only_changes_do_not_touch_the_driver() {
        let snapshot = Config::default();
        let mut current = snapshot;
        current.fn_mode = FnMode::FKeysFirst; // switched from the tray while the window was open
        assert_eq!(driver_update(&snapshot, &snapshot, &current), None);
    }

    #[test]
    fn driver_changes_keep_changes_made_elsewhere() {
        let snapshot = Config::default();
        let mut current = snapshot;
        current.fn_mode = FnMode::FKeysFirst;
        let mut edited = snapshot;
        edited.set_modifier(ModSource::LeftCommand, ModTarget::Ctrl);

        let sent = driver_update(&snapshot, &edited, &current).expect("driver change");
        assert_eq!(sent.fn_mode, FnMode::FKeysFirst);
        assert_eq!(sent.modifier(ModSource::LeftCommand), ModTarget::Ctrl);
    }
}
