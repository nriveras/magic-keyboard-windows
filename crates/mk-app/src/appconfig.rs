//! App-side settings (not used by the driver): `%APPDATA%\magic-keyboard-windows\app.toml`.

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Number of launcher keys (F16–F19).
pub const LAUNCHER_COUNT: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct AppConfig {
    /// Battery percentage at which the low-battery notification appears (0 disables it).
    pub low_battery: u8,
    /// Program, document or URL opened by F16, F17, F18 and F19 (empty = nothing).
    pub launchers: [String; LAUNCHER_COUNT],
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            low_battery: 20,
            launchers: [
                "explorer.exe".into(),
                "calc.exe".into(),
                "notepad.exe".into(),
                "taskmgr.exe".into(),
            ],
        }
    }
}

pub fn path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| {
        PathBuf::from(dir)
            .join("magic-keyboard-windows")
            .join("app.toml")
    })
}

/// Loads the settings; a missing or unreadable file gives the defaults.
pub fn load() -> AppConfig {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(config: &AppConfig) -> io::Result<()> {
    let path = path().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA not set"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string_pretty(config).map_err(io::Error::other)?;
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_roundtrip_and_partial_files() {
        let config = AppConfig::default();
        let text = toml::to_string_pretty(&config).unwrap();
        assert_eq!(toml::from_str::<AppConfig>(&text).unwrap(), config);
        // Missing keys fall back to the defaults.
        let partial: AppConfig = toml::from_str("low-battery = 10").unwrap();
        assert_eq!(partial.low_battery, 10);
        assert_eq!(partial.launchers, AppConfig::default().launchers);
    }
}
