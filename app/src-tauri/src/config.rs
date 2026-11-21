use std::path::{Path, PathBuf};

use blast_aura::AuraSettings;
use blast_cast::OutputConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub device: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub audio: AudioSettings,
    pub aura: AuraSettings,
    pub outputs: Vec<OutputConfig>,
}

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
