//! Settings window (`mk-app settings`), a separate process started from the tray menu so the tray
//! keeps its plain Win32 message loop.
//!
//! Layout: a header with one pill per connected keyboard and its battery; a drawing of that model
//! where configurable keys are highlighted; an inspector on the right for the selected key; and
//! setting cards for everything, grouped by kind. Mappings are shared by all keyboards (the driver
//! keeps one config). Changes take effect immediately: driver options go through the control
//! interface, app options are saved to app.toml (after a short pause while typing) and the tray is
//! told to reload them.

mod keyboard;
mod labels;
mod panels;
mod theme;
mod widgets;

use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Margin, RichText};
use mk_core::{model, Config};
use mk_hid::control::Control;

use crate::appconfig::{self, AppConfig};
use crate::devices::{self, Keyboard};
use crate::system;
use keyboard::{Click, Target};
use theme::glyph;
use widgets::{card, Severity};

/// How often the keyboard list and batteries are refreshed while the window is open.
const REFRESH: Duration = Duration::from_secs(10);
/// App options are saved once they have not changed for this long.
const SAVE_DELAY: Duration = Duration::from_millis(500);
const INSPECTOR_WIDTH: f32 = 300.0;
/// Largest key size of the drawing, in points.
const MAX_KEY: f32 = 46.0;

pub fn run() {
    let Some(_instance) = system::named_mutex("Local\\MagicKeyboardWindowsSettings") else {
        return; // already open
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Magic Keyboard settings")
            .with_icon(theme::window_icon())
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "Magic Keyboard settings",
        options,
        Box::new(|cc| {
            theme::install(&cc.egui_ctx);
            Ok(Box::new(Settings::load()))
        }),
    );
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
    /// App options waiting to be saved, and when they last changed.
    pending_app: Option<(AppConfig, Instant)>,
    /// Threshold restored when the low-battery warning is switched back on.
    last_low_battery: u8,
    autostart: bool,
    /// Key shown in the inspector.
    selected: Option<Target>,
    /// The "reset everything" confirmation is open.
    confirm_reset: bool,
    /// Last failure to apply or save a change.
    error: Option<String>,
}

impl Settings {
    fn load() -> Settings {
        let control = Control::open();
        let last_model = control
            .as_ref()
            .ok()
            .and_then(|c| c.status().ok())
            .and_then(|s| model::find(model::APPLE_USB_VID, s.last_pid));
        let driver = control.and_then(|c| c.config()).map_err(|e| e.to_string());
        let edited = driver.clone().unwrap_or_default();
        let app = appconfig::load();
        let last_low_battery = if app.low_battery > 0 {
            app.low_battery
        } else {
            AppConfig::default().low_battery
        };
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
            pending_app: None,
            last_low_battery,
            autostart: system::autostart_enabled(),
            selected: None,
            confirm_reset: false,
            error: None,
        }
    }

    /// Sends edited driver options. On failure the edit is undone so it is not retried every frame.
    fn apply_driver(&mut self) {
        let Some(snapshot) = self.driver.clone().ok().filter(|c| *c != self.edited) else {
            return;
        };
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
            Err(e) => {
                self.edited = snapshot;
                self.error = Some(format!(
                    "The keyboard driver did not accept the change: {e}"
                ));
            }
        }
    }

    fn save_app(&mut self) {
        self.pending_app = None;
        if self.app == self.saved_app {
            return;
        }
        match appconfig::save(&self.app) {
            Ok(()) => {
                self.saved_app = self.app.clone();
                system::notify_tray_reload();
            }
            Err(e) => {
                self.app = self.saved_app.clone();
                self.error = Some(format!("The app settings could not be saved: {e}"));
            }
        }
    }

    /// Applies what changed this frame: driver options at once, app options after a pause and
    /// once no text field has focus.
    fn commit(&mut self, ctx: &egui::Context) {
        self.apply_driver();
        if self.app == self.saved_app {
            self.pending_app = None;
            return;
        }
        let now = Instant::now();
        match &self.pending_app {
            Some((app, _)) if *app == self.app => {}
            _ => self.pending_app = Some((self.app.clone(), now)),
        }
        let waited = self
            .pending_app
            .as_ref()
            .map_or(Duration::ZERO, |(_, t)| now - *t);
        if waited >= SAVE_DELAY && !ctx.wants_keyboard_input() {
            self.save_app();
        } else {
            ctx.request_repaint_after(
                SAVE_DELAY
                    .saturating_sub(waited)
                    .max(Duration::from_millis(50)),
            );
        }
    }

    fn defaults_config(&self) -> Config {
        self.model()
            .or(self.last_model)
            .map(Config::default_for)
            .unwrap_or_default()
    }

    fn reset_all(&mut self) {
        self.edited = self.defaults_config();
        self.app = AppConfig::default();
        self.selected = None;
        self.error = None;
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

    fn header(&mut self, ui: &mut egui::Ui) {
        let battery = self.keyboards.get(self.tab).map(|k| k.battery);
        ui.horizontal(|ui| {
            ui.set_min_height(34.0);
            if self.keyboards.is_empty() {
                let text = if self.devices_rx.is_some() {
                    "Looking for keyboards…"
                } else {
                    "No Magic Keyboard connected"
                };
                let _ = widgets::pill(ui, glyph::INFO, text, "", true);
            }
            for (i, k) in self.keyboards.iter().enumerate() {
                let icon = if k.bluetooth {
                    glyph::BLUETOOTH
                } else {
                    glyph::USB
                };
                if widgets::pill(ui, icon, &k.name(), k.transport(), self.tab == i).clicked()
                    && self.tab != i
                {
                    self.tab = i;
                    self.selected = None;
                }
            }
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| match battery {
                    Some(Some(b)) => widgets::battery(ui, &b),
                    Some(None) => panels::caption(ui, "Battery unavailable"),
                    None => {}
                },
            );
        });
    }

    fn hero(&mut self, ui: &mut egui::Ui) {
        let layout = keyboard::layout_for(self.model());
        ui.vertical_centered(|ui| {
            match keyboard::show(ui, &layout, &self.edited, &self.app, self.selected, MAX_KEY) {
                Some(Click::Key(target)) => self.selected = Some(target),
                Some(Click::Nothing) => self.selected = None,
                None => {}
            }
            if self.keyboards.is_empty() {
                panels::caption(
                    ui,
                    "No keyboard connected: showing the numeric-keypad layout",
                );
            }
        });
        // Centered by measuring the legend in the previous frame.
        let id = ui.id().with("legend-width");
        let width: f32 = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(0.0);
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
            let start = ui.cursor().min.x;
            keyboard::legend(ui);
            let measured = ui.min_rect().max.x - start;
            ui.ctx().data_mut(|d| d.insert_temp(id, measured));
        });
    }

    /// Every configurable key of the shown layout, once each.
    fn targets(&self) -> Vec<Target> {
        let mut targets = Vec::new();
        for t in keyboard::layout_for(self.model())
            .keys
            .iter()
            .filter_map(|k| k.target)
        {
            if !targets.contains(&t) {
                targets.push(t);
            }
        }
        targets
    }

    fn is_default(&self, target: Target, defaults: &Config) -> bool {
        labels::value(target, &self.edited, &self.app)
            == labels::value(target, defaults, &AppConfig::default())
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        let defaults = self.defaults_config();
        let Some(target) = self.selected else {
            self.inspector_overview(ui, &defaults);
            return;
        };

        ui.horizontal(|ui| {
            ui.label(RichText::new(labels::title(target)).font(theme::semibold(20.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(theme::icon(glyph::CLOSE, 12.0)).frame(false))
                    .on_hover_text("Close (Esc)")
                    .clicked()
                {
                    self.selected = None;
                }
            });
        });
        panels::caption(ui, labels::description(target));
        ui.add_space(12.0);
        let launcher = matches!(target, Target::FExt(i) if i >= 3);
        if self.driver.is_err() && !launcher {
            widgets::info_bar(
                ui,
                Severity::Warning,
                "Driver not reachable",
                "This key is handled by the keyboard driver.",
            );
            return;
        }
        let width = ui.available_width();
        panels::target_control(ui, target, &mut self.edited, &mut self.app, width);
        ui.add_space(14.0);
        panels::caption(
            ui,
            &format!(
                "Default: {}",
                labels::value(target, &defaults, &AppConfig::default())
            ),
        );
        ui.add_space(4.0);
        let label = RichText::new(format!("{}  Reset this key", glyph::RESET));
        let label = label.family(egui::FontFamily::Name("icons".into()));
        let at_default = self.is_default(target, &defaults);
        if ui
            .add_enabled(!at_default, egui::Button::new(label))
            .clicked()
        {
            panels::reset(target, &mut self.edited, &mut self.app, &defaults);
        }
    }

    /// Inspector with no key selected: a hint and the keys that differ from their defaults.
    fn inspector_overview(&mut self, ui: &mut egui::Ui, defaults: &Config) {
        ui.label(RichText::new("Customize a key").font(theme::semibold(18.0)));
        ui.add_space(2.0);
        panels::caption(
            ui,
            "Select a highlighted key on the keyboard to change what it does.",
        );
        ui.add_space(18.0);
        ui.label(RichText::new("Changed keys").font(theme::semibold(14.0)));
        ui.add_space(2.0);
        let changed: Vec<Target> = self
            .targets()
            .into_iter()
            .filter(|&t| !self.is_default(t, defaults))
            .collect();
        if changed.is_empty() {
            panels::caption(ui, "None. Every key works as labeled.");
        }
        for t in changed {
            let text = format!(
                "{}  →  {}",
                labels::title(t),
                labels::value(t, &self.edited, &self.app)
            );
            let button = egui::Button::new(text)
                .frame(false)
                .min_size(egui::vec2(ui.available_width(), 26.0));
            if ui.add(button).on_hover_text("Select this key").clicked() {
                self.selected = Some(t);
            }
        }
    }

    fn cards(&mut self, ui: &mut egui::Ui) {
        if let Err(e) = &self.driver {
            widgets::info_bar(
                ui,
                Severity::Error,
                "Keyboard driver not reachable",
                &format!(
                    "{e}\nInstall the driver (scripts\\install-dev.ps1) and reopen this window. \
                     Launchers and app options still work."
                ),
            );
        }
        if let Some(e) = self.error.clone() {
            widgets::info_bar(ui, Severity::Warning, "Change not applied", &e);
        }
        let model = self.model();
        let driver_ok = self.driver.is_ok();
        ui.add_enabled_ui(driver_ok, |ui| {
            panels::keys_section(ui, &mut self.edited, model);
            panels::modifiers_section(ui, &mut self.edited, model);
        });
        if model.is_none_or(|m| m.has_numpad) {
            panels::extra_keys_section(ui, &mut self.edited, &mut self.app, driver_ok);
        }
        self.app_section(ui);
        ui.add_space(8.0);
        panels::caption(ui, "Key settings apply to every connected Magic Keyboard.");
        ui.add_space(16.0);
    }

    fn app_section(&mut self, ui: &mut egui::Ui) {
        widgets::section_header(ui, "App");
        let mut error = None;
        card(
            ui,
            "Start with Windows",
            Some("Show the battery in the notification area after you sign in"),
            |ui| {
                // Applied at once by writing the Run key.
                if widgets::labeled_toggle(ui, &mut self.autostart).changed() {
                    if let Err(e) = system::set_autostart(self.autostart) {
                        error = Some(format!("Start with Windows could not be changed: {e}"));
                        self.autostart = system::autostart_enabled();
                    }
                }
            },
        );
        card(
            ui,
            "Low-battery warning",
            Some("Show a notification when the battery runs low"),
            |ui| panels::low_battery(ui, &mut self.app, &mut self.last_low_battery),
        );
        card(
            ui,
            "Reset all settings",
            Some("Every key and app option goes back to its default"),
            |ui| {
                if ui.button("Reset…").clicked() {
                    self.confirm_reset = true;
                }
            },
        );
        if error.is_some() {
            self.error = error;
        }
    }

    fn reset_dialog(&mut self, ctx: &egui::Context) {
        if !self.confirm_reset {
            return;
        }
        let modal = egui::Modal::new(egui::Id::new("confirm-reset")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new("Reset all settings?").font(theme::semibold(18.0)));
            ui.add_space(4.0);
            ui.label(
                "Every key mapping, launcher and app option goes back to its default. \
                 Start with Windows is not changed.",
            );
            ui.add_space(14.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Cancel").clicked() {
                    self.confirm_reset = false;
                }
                let p = theme::palette(ui.visuals().dark_mode);
                let reset =
                    egui::Button::new(RichText::new("Reset").color(p.on_accent)).fill(p.accent);
                if ui.add(reset).clicked() {
                    self.reset_all();
                    self.confirm_reset = false;
                }
            });
        });
        if modal.should_close() {
            self.confirm_reset = false;
        }
    }
}

impl eframe::App for Settings {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.refresh_devices(ctx);
        ctx.request_repaint_after(REFRESH);
        let panel = ctx.style().visuals.panel_fill;
        let p = theme::palette(ctx.style().visuals.dark_mode);
        let margin = |top, bottom| Margin {
            left: 20,
            right: 20,
            top,
            bottom,
        };

        egui::TopBottomPanel::top("header")
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(panel).inner_margin(margin(14, 4)))
            .show(ctx, |ui| self.header(ui));
        egui::SidePanel::right("inspector")
            .resizable(false)
            .exact_width(INSPECTOR_WIDTH)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(p.card)
                    .stroke(egui::Stroke::new(1.0_f32, p.card_stroke))
                    .inner_margin(Margin::same(18)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.inspector(ui));
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(panel).inner_margin(Margin {
                left: 20,
                right: 16,
                top: 0,
                bottom: 0,
            }))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(8.0);
                    self.hero(ui);
                    self.cards(ui);
                });
            });
        self.reset_dialog(ctx);
        if !self.confirm_reset && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.selected = None;
        }
        self.commit(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.apply_driver();
        self.save_app();
    }
}

/// The config to send after an edit: `None` if no driver option was edited since the window read
/// `snapshot`; otherwise only the edited fields, applied to the driver's `current` config so
/// changes made meanwhile (tray menu, Fn double-tap) are kept.
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
