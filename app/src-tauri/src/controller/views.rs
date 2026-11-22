use std::path::PathBuf;

use serde::Serialize;

use blast_cast::OutputStatus;
use blast_library::{HistoryRow, LibraryStats};

use super::commands::Deck;
use crate::playlist::Entry;

pub const EV_STATE: &str = "blast://state";
pub const EV_PLAYLIST: &str = "blast://playlist";
pub const EV_NOTICE: &str = "blast://notice";
pub const EV_CAST: &str = "blast://cast";
pub const EV_PADS: &str = "blast://pads";
pub const EV_OBS: &str = "blast://obs";
pub const EV_HISTORY: &str = "blast://history";

#[derive(Serialize, Clone)]
pub struct Notice {
    pub level: &'static str,
    pub message: String,
}

#[derive(Serialize, Clone)]
pub struct ObsInputView {
    pub name: String,
    pub kind: String,
    pub muted: bool,
    pub volume: f32,
}

#[derive(Serialize, Clone, Default)]
pub struct ObsView {
    pub status: &'static str,
    pub detail: String,
    pub obs_version: String,
    pub scenes: Vec<String>,
    pub current: Option<String>,
    pub stream: bool,
    pub stream_since: Option<u64>,
    pub record: bool,
    pub record_paused: bool,
    pub record_since: Option<u64>,
    pub inputs: Vec<ObsInputView>,
    pub now_playing_source: String,
}

impl ObsView {
    pub fn idle(status: &'static str, now_playing_source: String) -> Self {
        Self {
            status,
            now_playing_source,
            ..Default::default()
        }
    }
}

#[derive(Serialize, Clone)]
pub struct PadView {
    pub index: u16,
    pub name: String,
    pub color: String,
    pub path: PathBuf,
    pub ready: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct PadsView {
    pub pads: Vec<PadView>,
    pub count: u16,
}

#[derive(Serialize, Clone)]
pub struct MicView {
    pub available: bool,
    pub open: bool,
    pub peak: f32,
    pub name: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct DeckView {
    pub state: &'static str,
    pub position: f64,
    pub duration: f64,
    pub title: String,
    pub artist: String,
    pub path: Option<PathBuf>,
}

#[derive(Serialize, Clone)]
pub struct StateView {
    pub decks: [DeckView; 2],
    pub crossfader: f32,
    pub peak: [f32; 2],
    pub automix: bool,
    pub fade: f32,
    pub on_air: Option<Deck>,
    pub device: String,
    pub sample_rate: u32,
    pub audio_error: Option<String>,
    pub mic: MicView,
    pub sfx: u64,
    pub duck: f32,
    pub reduction: f32,
}

#[derive(Serialize, Clone)]
pub struct PlaylistView<'a> {
    pub entries: &'a [Entry],
    pub total: f64,
}

#[derive(Serialize, Clone)]
pub struct CastOutputView {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub target: String,
    pub enabled: bool,
    pub status: OutputStatus,
    pub listeners: Option<u32>,
    pub peak: Option<u32>,
    pub live_since: Option<u64>,
}

#[derive(Serialize, Clone)]
pub struct CastView {
    pub casting: bool,
    pub song: String,
    pub outputs: Vec<CastOutputView>,
    pub listeners: Option<u32>,
    pub session_peak: u32,
    pub since: Option<u64>,
}

#[derive(Serialize, Clone)]
pub struct HistoryView {
    pub rows: Vec<HistoryRow>,
    pub stats: Option<LibraryStats>,
    pub current: Option<i64>,
}
