#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Mp3,
    AacLc,
    HeAac,
    Opus,
    Vorbis,
    Flac,
}

impl Codec {
    pub fn content_type(self) -> &'static str {
        match self {
            Codec::Mp3 => "audio/mpeg",
            Codec::AacLc | Codec::HeAac => "audio/aac",
            Codec::Opus | Codec::Vorbis => "audio/ogg",
            Codec::Flac => "audio/flac",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerKind {
    Icecast,
    ShoutcastV1,
    ShoutcastV2,
}

#[derive(Debug, Clone)]
pub struct EncoderConfig {
    pub codec: Codec,
    pub bitrate_kbps: u32,
    pub sample_rate: u32,
    pub channels: u8,
}

#[derive(Debug, Clone)]
pub struct ServerTarget {
    pub kind: ServerKind,
    pub host: String,
    pub port: u16,
    pub mount: String,
    pub user: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct Output {
    pub name: String,
    pub encoder: EncoderConfig,
    pub target: ServerTarget,
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_types() {
        assert_eq!(Codec::Mp3.content_type(), "audio/mpeg");
        assert_eq!(Codec::Opus.content_type(), "audio/ogg");
        assert_eq!(Codec::Flac.content_type(), "audio/flac");
    }
}
