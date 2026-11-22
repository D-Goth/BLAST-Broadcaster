use std::fs::File;
use std::path::{Path, PathBuf};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTagKey};
use symphonia::core::probe::Hint;

use crate::resample;

// [DEC-01]
#[derive(Debug, Clone)]
pub struct Track {
    pub source: PathBuf,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub source_rate: u32,
    pub title: Option<String>,
    pub artist: Option<String>,
}

impl Track {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    pub fn duration_secs(&self) -> f64 {
        self.frames() as f64 / self.sample_rate as f64
    }

    pub fn display_name(&self) -> String {
        match (&self.artist, &self.title) {
            (Some(a), Some(t)) => format!("{a} - {t}"),
            (None, Some(t)) => t.clone(),
            _ => self
                .source
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }
}

// [DEC-02]
#[derive(Debug)]
pub enum DecodeError {
    Io(std::io::Error),
    Format(String),
    NoAudioTrack,
    UnknownSampleRate,
    Resample(String),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Io(e) => write!(f, "lecture impossible : {e}"),
            DecodeError::Format(e) => write!(f, "format non pris en charge : {e}"),
            DecodeError::NoAudioTrack => write!(f, "aucune piste audio"),
            DecodeError::UnknownSampleRate => write!(f, "fréquence d'échantillonnage inconnue"),
            DecodeError::Resample(e) => write!(f, "rééchantillonnage : {e}"),
        }
    }
}

impl std::error::Error for DecodeError {}

impl From<std::io::Error> for DecodeError {
    fn from(e: std::io::Error) -> Self {
        DecodeError::Io(e)
    }
}

impl From<SymError> for DecodeError {
    fn from(e: SymError) -> Self {
        match e {
            SymError::IoError(io) => DecodeError::Io(io),
            other => DecodeError::Format(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Tags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    year: Option<i32>,
}

// [DEC-03]
fn parse_year(v: &str) -> Option<i32> {
    let digits: String = v.chars().filter(|c| c.is_ascii_digit()).take(4).collect();
    if digits.len() == 4 {
        digits.parse().ok()
    } else {
        None
    }
}

// [DEC-04]
fn read_tags(rev: &MetadataRevision, t: &mut Tags) {
    for tag in rev.tags() {
        let v = tag.value.to_string();
        let v = v.trim();
        if v.is_empty() {
            continue;
        }
        match tag.std_key {
            Some(StandardTagKey::TrackTitle) if t.title.is_none() => t.title = Some(v.to_string()),
            Some(StandardTagKey::Artist) if t.artist.is_none() => t.artist = Some(v.to_string()),
            Some(StandardTagKey::AlbumArtist) if t.artist.is_none() => {
                t.artist = Some(v.to_string())
            }
            Some(StandardTagKey::Album) if t.album.is_none() => t.album = Some(v.to_string()),
            Some(StandardTagKey::Genre) if t.genre.is_none() => t.genre = Some(v.to_string()),
            Some(
                StandardTagKey::Date | StandardTagKey::ReleaseDate | StandardTagKey::OriginalDate,
            ) if t.year.is_none() => t.year = parse_year(v),
            _ => {}
        }
    }
}

// [DEC-05]
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "mp2", "m4a", "mp4", "aac", "alac", "flac", "ogg", "oga", "wav", "aif", "aiff", "aifc",
    "caf", "mka", "mkv", "webm",
];

// [DEC-06]
pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

// [DEC-07]
#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub duration_secs: Option<f64>,
    pub sample_rate: Option<u32>,
}

impl TrackInfo {
    pub fn display_title(&self) -> String {
        self.title.clone().unwrap_or_else(|| {
            self.path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
    }
}

// [DEC-08]
fn open(path: &Path) -> Result<symphonia::core::probe::ProbeResult, DecodeError> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    Ok(symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?)
}

// [DEC-09]
fn collect_tags(probed: &mut symphonia::core::probe::ProbeResult) -> Tags {
    let mut t = Tags::default();
    if let Some(meta) = probed.metadata.get() {
        if let Some(rev) = meta.current() {
            read_tags(rev, &mut t);
        }
    }
    if let Some(rev) = probed.format.metadata().current() {
        read_tags(rev, &mut t);
    }
    t
}

// [DEC-10]
pub fn probe_file(path: &Path) -> Result<TrackInfo, DecodeError> {
    let mut probed = open(path)?;
    let tags = collect_tags(&mut probed);
    let track = probed
        .format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(DecodeError::NoAudioTrack)?;
    let params = &track.codec_params;
    symphonia::default::get_codecs().make(params, &DecoderOptions::default())?;
    let duration_secs = match (params.n_frames, params.sample_rate) {
        (Some(n), Some(r)) if r > 0 => Some(n as f64 / r as f64),
        _ => None,
    };
    Ok(TrackInfo {
        path: path.to_path_buf(),
        title: tags.title,
        artist: tags.artist,
        album: tags.album,
        genre: tags.genre,
        year: tags.year,
        duration_secs,
        sample_rate: params.sample_rate,
    })
}

// [DEC-11]
pub fn decode_file(path: &Path, target_rate: u32) -> Result<Track, DecodeError> {
    let mut probed = open(path)?;
    let Tags { title, artist, .. } = collect_tags(&mut probed);
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(DecodeError::NoAudioTrack)?;
    let track_id = track.id;
    let source_rate = track
        .codec_params
        .sample_rate
        .ok_or(DecodeError::UnknownSampleRate)?;

    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    let mut samples: Vec<f32> = Vec::new();
    if let Some(n) = track.codec_params.n_frames {
        samples.reserve(n as usize * 2);
    }

    let mut buf: Option<SampleBuffer<f32>> = None;
    let mut buf_frames = 0usize;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        };

        let spec = *decoded.spec();
        let channels = spec.channels.count();
        let frames = decoded.frames();
        if frames == 0 || channels == 0 {
            continue;
        }
        if buf.is_none() || buf_frames < decoded.capacity() {
            buf_frames = decoded.capacity();
            buf = Some(SampleBuffer::new(buf_frames as u64, spec));
        }
        let sb = buf.as_mut().expect("buffer");
        sb.copy_interleaved_ref(decoded);
        let data = sb.samples();

        if channels == 1 {
            for &s in &data[..frames] {
                samples.push(s);
                samples.push(s);
            }
        } else {
            for f in 0..frames {
                let base = f * channels;
                samples.push(data[base]);
                samples.push(data[base + 1]);
            }
        }
    }

    let samples = if source_rate != target_rate {
        resample::resample_stereo(&samples, source_rate, target_rate)
            .map_err(DecodeError::Resample)?
    } else {
        samples
    };

    Ok(Track {
        source: path.to_path_buf(),
        samples,
        sample_rate: target_rate,
        source_rate,
        title,
        artist,
    })
}
