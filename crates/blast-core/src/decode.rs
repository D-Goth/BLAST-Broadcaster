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

fn read_tags(rev: &MetadataRevision, title: &mut Option<String>, artist: &mut Option<String>) {
    for tag in rev.tags() {
        match tag.std_key {
            Some(StandardTagKey::TrackTitle) if title.is_none() => {
                *title = Some(tag.value.to_string());
            }
            Some(StandardTagKey::Artist) if artist.is_none() => {
                *artist = Some(tag.value.to_string());
            }
            _ => {}
        }
    }
}

pub fn decode_file(path: &Path, target_rate: u32) -> Result<Track, DecodeError> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;

    let mut title = None;
    let mut artist = None;
    if let Some(meta) = probed.metadata.get() {
        if let Some(rev) = meta.current() {
            read_tags(rev, &mut title, &mut artist);
        }
    }

    let mut format = probed.format;
    if let Some(rev) = format.metadata().current() {
        read_tags(rev, &mut title, &mut artist);
    }

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
