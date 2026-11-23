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
    pub deck_gain_db: [f32; 2],
    pub cue_device: Option<String>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            device: None,
            master_preset: "doux".into(),
            deck_gain_db: [0.0; 2],
            cue_device: None,
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
    pub duck_attack_ms: u32,
    pub duck_release_ms: u32,
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
            duck_attack_ms: 200,
            duck_release_ms: 800,
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
    pub memory_size: u32,
}

impl Default for AutomationSettings {
    fn default() -> Self {
        Self {
            rules: blast_library::SeparationRules::default(),
            history_days: 365,
            memory_size: 200,
        }
    }
}

// [CFG-11]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CategorySettings {
    pub jingle_keywords: Vec<String>,
    pub pub_keywords: Vec<String>,
}

impl Default for CategorySettings {
    fn default() -> Self {
        Self {
            jingle_keywords: vec!["JINGLE".into()],
            pub_keywords: vec!["PUB".into(), "PUBLICITE".into()],
        }
    }
}

// [CFG-12]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JingleSettings {
    pub dirs: Vec<PathBuf>,
    pub volume_db: f32,
}

impl Default for JingleSettings {
    fn default() -> Self {
        Self {
            dirs: Vec::new(),
            volume_db: 0.0,
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

// [CFG-17]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatSettings {
    pub twitch_enabled: bool,
    pub ssn_enabled: bool,
    pub channel: String,
    pub bot_login: String,
    pub autoconnect: bool,
    pub announce: bool,
    pub cmd_np: bool,
    pub cmd_request: bool,
    pub cmd_sfx: bool,
    pub request_limit: u32,
}

impl Default for ChatSettings {
    fn default() -> Self {
        Self {
            twitch_enabled: true,
            ssn_enabled: false,
            channel: String::new(),
            bot_login: String::new(),
            autoconnect: false,
            announce: false,
            cmd_np: true,
            cmd_request: true,
            cmd_sfx: false,
            request_limit: 2,
        }
    }
}

// [CFG-08]
// [CFG-14]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibrarySettings {
    pub dirs: Vec<PathBuf>,
}

// [CFG-13]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub glass: bool,
    pub blur: f64,
    pub opacity: f64,
    pub dim: f64,
    pub background: Option<String>,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            glass: false,
            blur: 18.0,
            opacity: 0.55,
            dim: 0.25,
            background: None,
        }
    }
}

impl AppearanceSettings {
    pub fn clamped(mut self) -> Self {
        self.blur = self.blur.clamp(0.0, 40.0);
        self.opacity = self.opacity.clamp(0.15, 0.95);
        self.dim = self.dim.clamp(0.0, 0.85);
        self
    }
}

// [CFG-15]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IceServerProfile {
    pub id: String,
    pub name: String,
    pub remote: bool,
    pub install_dir: Option<PathBuf>,
    pub exe: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub host: String,
    pub port: u16,
    pub https: bool,
    pub admin_user: String,
    pub autostart: bool,
    pub stop_on_exit: bool,
}

impl Default for IceServerProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "Icecast local".into(),
            remote: false,
            install_dir: None,
            exe: None,
            config_path: None,
            host: "localhost".into(),
            port: 8000,
            https: false,
            admin_user: "admin".into(),
            autostart: false,
            stop_on_exit: false,
        }
    }
}

// [CFG-16]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IcecastSettings {
    pub servers: Vec<IceServerProfile>,
}

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
    pub categories: CategorySettings,
    pub jingles: JingleSettings,
    pub appearance: AppearanceSettings,
    pub library: LibrarySettings,
    pub rotation: blast_library::Rotation,
    pub icecast: IcecastSettings,
    pub chat: ChatSettings,
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
    #[test]
    fn full_config_survives_a_toml_round_trip() {
        let mut cfg = super::AppConfig::default();
        cfg.rotation.clocks.push(blast_library::Clock {
            id: "soir".into(),
            name: "Soirée".into(),
            color: "#7A5AF8".into(),
            slots: vec!["metal".into()],
        });
        cfg.rotation
            .categories
            .push(blast_library::RotationCategory {
                id: "metal".into(),
                name: "Metal".into(),
                ..Default::default()
            });
        cfg.rotation.schedule = vec![String::new(); blast_library::SCHEDULE_SLOTS];
        cfg.rotation.schedule[5] = "soir".into();
        cfg.rotation.default_clock = Some("soir".into());
        cfg.appearance.background = Some("background-1.jpg".into());
        cfg.library.dirs.push("/musique".into());
        cfg.icecast.servers.push(super::IceServerProfile {
            id: "ice-1".into(),
            install_dir: Some("C:/Program Files/Icecast".into()),
            ..Default::default()
        });
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: super::AppConfig = toml::from_str(&text).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn appearance_is_clamped() {
        let a = super::AppearanceSettings {
            blur: 500.0,
            opacity: 0.0,
            dim: 2.0,
            ..Default::default()
        }
        .clamped();
        assert_eq!((a.blur, a.opacity, a.dim), (40.0, 0.15, 0.85));
    }

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
