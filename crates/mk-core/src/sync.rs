//! The configuration shared by every keyboard filter in the driver, with a generation counter so
//! each filter knows when to re-apply it.

use crate::config::{Config, FnMode};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SharedConfig {
    /// User configuration (registry or app); `None` means model defaults.
    pub config: Option<Config>,
    /// Bumped on every change.
    pub generation: u32,
}

impl SharedConfig {
    pub const fn new() -> Self {
        SharedConfig {
            config: None,
            generation: 0,
        }
    }

    /// Replaces the configuration (registry at load, or the app).
    pub fn set(&mut self, config: Option<Config>) {
        self.config = config;
        self.generation = self.generation.wrapping_add(1);
    }

    /// The generation and configuration a filter that last applied `seen` must switch to.
    pub fn update_since(&self, seen: u32) -> Option<(u32, Option<Config>)> {
        (self.generation != seen).then_some((self.generation, self.config))
    }

    /// Records a Fn-lock toggle made on a keyboard. Only the top-row mode changes; everything else
    /// keeps what was set last, even if that keyboard has not applied it yet. `defaults` is the
    /// configuration in effect while none is set.
    pub fn set_fn_mode(&mut self, mode: FnMode, defaults: Config) {
        let mut config = self.config.unwrap_or(defaults);
        config.fn_mode = mode;
        self.set(Some(config));
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::config::{ModSource, ModTarget};

    fn remapped() -> Config {
        let mut c = Config::default();
        c.modifiers[ModSource::LeftCommand as usize] = ModTarget::Ctrl;
        c
    }

    #[test]
    fn set_bumps_generation() {
        let mut s = SharedConfig::new();
        s.set(Some(remapped()));
        assert_eq!(s.generation, 1);
        assert_eq!(s.update_since(0), Some((1, Some(remapped()))));
        assert_eq!(s.update_since(1), None);
    }

    #[test]
    fn fn_lock_toggle_on_a_stale_keyboard_keeps_the_apps_newer_config() {
        let mut s = SharedConfig::new();
        let seen = s.generation;
        // The app sends a remap; the keyboard has not applied it yet when its double tap fires.
        s.set(Some(remapped()));
        s.set_fn_mode(FnMode::FKeysFirst, Config::default());

        let mut expected = remapped();
        expected.fn_mode = FnMode::FKeysFirst;
        assert_eq!(s.config, Some(expected));
        assert_eq!(
            s.update_since(seen),
            Some((s.generation, Some(expected))),
            "the keyboard still picks up the remap"
        );
    }

    #[test]
    fn fn_lock_toggle_without_user_config_starts_from_defaults() {
        let mut s = SharedConfig::new();
        let mut defaults = Config::default();
        defaults.sleep_combo = false;
        s.set_fn_mode(FnMode::FKeysFirst, defaults);
        let mut expected = defaults;
        expected.fn_mode = FnMode::FKeysFirst;
        assert_eq!(s.config, Some(expected));
        assert_eq!(s.generation, 1);
    }
}
