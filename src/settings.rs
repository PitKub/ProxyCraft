//! Remembered settings.
//!
//! Stored as JSON in `%APPDATA%\PitKub\ProxyCraft\settings.json`. On the first start, the
//! settings of the Python ProxyCraft 1.x (Windows registry, `HKCU\Software\PitKub\ProxyCraft`,
//! written by Qt's QSettings) are imported, so an upgrade keeps them.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::engine::{Destination, DEFAULT_SUFFIX, PRESETS};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub ffmpeg: String,
    pub ffprobe: String,
    pub suffix: String,
    pub preset: String,
    pub destination: Destination,
    pub custom_dir: String,
    pub skip_existing: bool,
    pub include_subfolders: bool,
    pub low_priority: bool,
    pub verify: bool,
    /// 0 = automatic.
    pub parallel: usize,
    pub last_dir: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "dark".into(),
            ffmpeg: String::new(),
            ffprobe: String::new(),
            suffix: DEFAULT_SUFFIX.into(),
            preset: PRESETS[0].key.into(),
            destination: Destination::ProxiesFolder,
            custom_dir: String::new(),
            skip_existing: true,
            include_subfolders: false,
            low_priority: true,
            verify: true,
            parallel: 0,
            last_dir: String::new(),
        }
    }
}

pub fn settings_path() -> PathBuf {
    if let Ok(p) = std::env::var("PROXYCRAFT_SETTINGS") {
        return PathBuf::from(p);
    }
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("PitKub")
        .join("ProxyCraft")
        .join("settings.json")
}

impl Settings {
    pub fn load() -> Settings {
        let path = settings_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => legacy::import().unwrap_or_default(),
        }
    }

    pub fn save(&self) {
        let path = settings_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            // write to a temporary file first, so a crash never leaves half a settings file
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }
}

#[cfg(windows)]
mod legacy {
    use super::Settings;
    use crate::engine::{preset_by_key, Destination};
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    /// Settings of ProxyCraft 1.x (QSettings in the registry).
    pub fn import() -> Option<Settings> {
        let root = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\PitKub\ProxyCraft")
            .ok()?;
        let get = |group: &str, name: &str| -> Option<String> {
            root.open_subkey(group)
                .ok()?
                .get_value::<String, _>(name)
                .ok()
        };
        let flag = |group: &str, name: &str, default: bool| -> bool {
            get(group, name)
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(default)
        };
        let mut s = Settings::default();
        if let Some(t) = get("appearance", "theme").filter(|t| t == "dark" || t == "light") {
            s.theme = t;
        }
        s.ffmpeg = get("tools", "ffmpeg").unwrap_or_default();
        s.ffprobe = get("tools", "ffprobe").unwrap_or_default();
        if let Some(x) = get("naming", "suffix").filter(|x| !x.is_empty()) {
            s.suffix = x;
        }
        if let Some(p) = get("output", "preset").filter(|p| preset_by_key(p).is_some()) {
            s.preset = p;
        }
        if let Some(d) = get("output", "destination").and_then(|d| Destination::from_key(&d)) {
            s.destination = d;
        }
        s.custom_dir = get("output", "custom_dir").unwrap_or_default();
        s.skip_existing = flag("options", "skip_existing", true);
        s.include_subfolders = flag("options", "include_subfolders", false);
        s.low_priority = flag("options", "low_priority", true);
        s.verify = flag("options", "verify", true);
        s.parallel = get("options", "parallel")
            .and_then(|p| p.parse().ok())
            .filter(|p| *p <= 4)
            .unwrap_or(0);
        s.last_dir = get("paths", "last_dir").unwrap_or_default();
        Some(s)
    }
}

#[cfg(not(windows))]
mod legacy {
    pub fn import() -> Option<super::Settings> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("PROXYCRAFT_SETTINGS", dir.path().join("s.json"));
        let mut s = Settings::load();
        assert_eq!(s, Settings::default());
        s.theme = "light".into();
        s.destination = Destination::Custom;
        s.save();
        let back = Settings::load();
        assert_eq!(back.theme, "light");
        assert_eq!(back.destination, Destination::Custom);
        // unknown or missing fields fall back to defaults
        std::fs::write(dir.path().join("s.json"), r#"{"theme":"dark","future":1}"#).unwrap();
        assert!(Settings::load().verify);
        std::env::remove_var("PROXYCRAFT_SETTINGS");
    }
}
