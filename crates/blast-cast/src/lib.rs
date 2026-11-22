pub mod caster;
pub mod encoder;
pub mod protocol;

pub use caster::{Caster, OutputStatus, StatusEvent};
pub use encoder::Mp3Encoder;
pub use protocol::{
    fetch_listeners, open_source, test_output, update_metadata, CastError, Listeners,
};

use serde::{Deserialize, Serialize};

// [CAST-01]
pub const BITRATES: &[u32] = &[32, 40, 48, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];

// [CAST-02]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Mp3,
}

impl Codec {
    pub fn content_type(self) -> &'static str {
        match self {
            Codec::Mp3 => "audio/mpeg",
        }
    }
}

// [CAST-03]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerKind {
    #[default]
    Icecast,
    ShoutcastV1,
    ShoutcastV2,
}

impl ServerKind {
    pub fn label(self) -> &'static str {
        match self {
            ServerKind::Icecast => "Icecast",
            ServerKind::ShoutcastV1 => "Shoutcast v1",
            ServerKind::ShoutcastV2 => "Shoutcast v2",
        }
    }
}

// [CAST-04]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub kind: ServerKind,
    pub host: String,
    pub port: u16,
    pub mount: String,
    pub sid: u32,
    pub user: String,
    pub bitrate: u32,
    pub mono: bool,
    pub station: String,
    pub genre: String,
    pub description: String,
    pub url: String,
    pub public: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "Ma webradio".into(),
            enabled: true,
            kind: ServerKind::Icecast,
            host: "localhost".into(),
            port: 8000,
            mount: "/live".into(),
            sid: 1,
            user: "source".into(),
            bitrate: 128,
            mono: false,
            station: "BLAST Broadcaster".into(),
            genre: String::new(),
            description: String::new(),
            url: String::new(),
            public: false,
        }
    }
}

// [CAST-05]
impl OutputConfig {
    pub fn mount_path(&self) -> String {
        let m = self.mount.trim();
        if m.starts_with('/') {
            m.to_string()
        } else {
            format!("/{m}")
        }
    }

    pub fn channels(&self) -> u8 {
        if self.mono {
            1
        } else {
            2
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Donne un nom à cette sortie.".into());
        }
        if self.host.trim().is_empty() {
            return Err("L'adresse du serveur est vide.".into());
        }
        if self.port == 0 {
            return Err("Le port doit être compris entre 1 et 65535.".into());
        }
        if matches!(self.kind, ServerKind::ShoutcastV1 | ServerKind::ShoutcastV2)
            && self.port == u16::MAX
        {
            return Err(
                "Shoutcast utilise le port suivant pour la source : choisis un port plus petit."
                    .into(),
            );
        }
        if self.kind == ServerKind::Icecast && self.mount.trim().trim_start_matches('/').is_empty()
        {
            return Err("Indique un point de montage (par exemple /live).".into());
        }
        if !BITRATES.contains(&self.bitrate) {
            return Err(format!("Débit non pris en charge : {} kb/s.", self.bitrate));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert!(OutputConfig::default().validate().is_ok());
    }

    #[test]
    fn mount_is_normalized() {
        let c = OutputConfig {
            mount: "radio".into(),
            ..Default::default()
        };
        assert_eq!(c.mount_path(), "/radio");
    }

    #[test]
    fn validation_catches_errors() {
        let c = OutputConfig {
            mount: "/".into(),
            ..Default::default()
        };
        assert!(c.validate().is_err());
        let c = OutputConfig {
            bitrate: 100,
            ..Default::default()
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn kind_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&ServerKind::ShoutcastV2).unwrap(),
            "\"shoutcast_v2\""
        );
        let back: OutputConfig =
            serde_json::from_str(r#"{"kind":"shoutcast_v1","port":9000}"#).unwrap();
        assert_eq!(back.kind, ServerKind::ShoutcastV1);
        assert_eq!(back.port, 9000);
        assert_eq!(back.mount, "/live");
    }
}
