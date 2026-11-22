use std::path::{Path, PathBuf};

use blast_aura::AuraSettings;
use blast_cast::OutputConfig;
use serde::{Deserialize, Serialize};

// [CFG-01]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub device: Option<String>,
    pub master_preset: String,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            device: None,
            master_preset: "doux".into(),
        }
    }
}

// [CFG-02]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
    pub enabled: bool,
    pub input_device: Option<String>,
    pub mic_gain_db: f32,
    pub music_gain_db: f32,
    pub duck: bool,
    pub duck_db: f32,
    pub low_db: f32,
    pub mid_db: f32,
    pub high_db: f32,
    pub highpass: bool,
    pub monitor: bool,
    pub preset: String,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            input_device: None,
            mic_gain_db: 0.0,
            music_gain_db: 0.0,
            duck: true,
            duck_db: -12.0,
            low_db: 0.0,
            mid_db: 0.0,
            high_db: 0.0,
            highpass: true,
            monitor: false,
            preset: "naturel".into(),
        }
    }
}

// [CFG-03]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PadConfig {
    pub index: u16,
    pub name: String,
    pub path: PathBuf,
    pub color: String,
}

// [CFG-04]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundboardSettings {
    pub volume_db: f32,
    pub pads: Vec<PadConfig>,
}

// [CFG-05]
pub const PAD_COUNT: u16 = 36;

// [CFG-06]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutomationSettings {
    pub rules: blast_library::SeparationRules,
    pub history_days: u32,
}

impl Default for AutomationSettings {
    fn default() -> Self {
        Self {
            rules: blast_library::SeparationRules::default(),
            history_days: 365,
        }
    }
}

// [CFG-07]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    pub check_on_start: bool,
    pub skipped: Option<String>,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            check_on_start: true,
            skipped: None,
        }
    }
}

// [CFG-08]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub audio: AudioSettings,
    pub voice: VoiceSettings,
    pub soundboard: SoundboardSettings,
    pub automation: AutomationSettings,
    pub aura: AuraSettings,
    pub outputs: Vec<OutputConfig>,
    pub updates: UpdateSettings,
}

// [CFG-09]
pub fn load(path: &Path) -> (AppConfig, Option<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (AppConfig::default(), None);
    };
    match toml::from_str(&text) {
        Ok(cfg) => (cfg, None),
        Err(e) => {
            let backup: PathBuf = path.with_extension("toml.bad");
            let _ = std::fs::rename(path, &backup);
            (
                AppConfig::default(),
                Some(format!(
                    "Réglages illisibles, remis à zéro (ancien fichier : {}) : {e}",
                    backup.display()
                )),
            )
        }
    }
}

// [CFG-10]
pub fn save(path: &Path, cfg: &AppConfig) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use blast_cast::ServerKind;

    #[test]
    fn round_trip_without_secrets() {
        let dir = std::env::temp_dir().join(format!("blast-cfg-{}", std::process::id()));
        let path = dir.join("config.toml");
        let cfg = AppConfig {
            outputs: vec![OutputConfig {
                id: "out-1".into(),
                kind: ServerKind::ShoutcastV2,
                ..Default::default()
            }],
            ..Default::default()
        };
        save(&path, &cfg).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.to_lowercase().contains("password"));
        let (back, warn) = load(&path);
        assert!(warn.is_none());
        assert_eq!(back, cfg);
        std::fs::write(&path, "outputs = 12").unwrap();
        let (reset, warn) = load(&path);
        assert!(warn.is_some());
        assert_eq!(reset, AppConfig::default());
        let _ = std::fs::remove_dir_all(dir);
    }
}
