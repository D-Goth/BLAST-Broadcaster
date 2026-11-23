use std::path::PathBuf;

use serde::Serialize;

use blast_cast::OutputStatus;
use blast_library::{HistoryRow, LibraryStats};

use super::commands::Deck;
use crate::playlist::Entry;

// [CTL-27]
pub const EV_STATE: &str = "blast://state";
pub const EV_PLAYLIST: &str = "blast://playlist";
pub const EV_NOTICE: &str = "blast://notice";
pub const EV_CAST: &str = "blast://cast";
pub const EV_PADS: &str = "blast://pads";
pub const EV_OBS: &str = "blast://obs";
pub const EV_HISTORY: &str = "blast://history";
pub const EV_JINGLES: &str = "blast://jingles";
pub const EV_LIBRARY: &str = "blast://library";
pub const EV_BLOCKED: &str = "blast://blocked";
pub const EV_CHAT_STATE: &str = "blast://chat-state";
pub const EV_CHAT_EVENT: &str = "blast://chat-event";
pub const EV_CHAT_HISTORY: &str = "blast://chat-history";

// [CTL-28]
#[derive(Serialize, Clone)]
pub struct Notice {
    pub level: &'static str,
    pub message: String,
}

// [CTL-29]
#[derive(Serialize, Clone)]
pub struct ObsInputView {
    pub name: String,
    pub kind: String,
    pub muted: bool,
    pub volume: f32,
}

// [CTL-30]
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

// [CTL-31]
#[derive(Serialize, Clone)]
pub struct PadView {
    pub index: u16,
    pub name: String,
    pub color: String,
    pub path: PathBuf,
    pub ready: bool,
    pub error: Option<String>,
}

// [CTL-32]
#[derive(Serialize, Clone)]
pub struct PadsView {
    pub pads: Vec<PadView>,
    pub count: u16,
}

// [CTL-33]
#[derive(Serialize, Clone)]
pub struct MicView {
    pub available: bool,
    pub open: bool,
    pub peak: f32,
    pub name: Option<String>,
    pub error: Option<String>,
}

// [CTL-34]
#[derive(Serialize, Clone)]
pub struct DeckView {
    pub state: &'static str,
    pub position: f64,
    pub duration: f64,
    pub title: String,
    pub artist: String,
    pub path: Option<PathBuf>,
    pub format: Option<String>,
    pub cover: Option<u64>,
}

// [CTL-35]
// [CTL-153]
#[derive(Serialize, Clone)]
pub struct BlockedView {
    pub blocked: Vec<(u64, String)>,
}

// [CTL-148]
#[derive(Serialize, Clone)]
pub struct ClockView {
    pub id: String,
    pub name: String,
    pub color: String,
    pub program: bool,
}

// [CTL-149]
#[derive(Serialize, Clone)]
pub struct LibraryView {
    pub dirs: Vec<PathBuf>,
    pub tracks: usize,
    pub scanning: bool,
    pub scanned: usize,
}

#[derive(Serialize, Clone)]
pub struct StateView {
    pub decks: [DeckView; 2],
    pub crossfader: f32,
    pub peak: [f32; 2],
    pub deck_peak: [[f32; 2]; 2],
    pub deck_gain_db: [f32; 2],
    pub deck_pitch: [f32; 2],
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
    pub clock: Option<ClockView>,
}

// [CTL-36]
#[derive(Serialize, Clone)]
pub struct PlaylistView<'a> {
    pub entries: &'a [Entry],
    pub total: f64,
}

// [CTL-37]
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

// [CTL-38]
#[derive(Serialize, Clone)]
pub struct CastView {
    pub casting: bool,
    pub song: String,
    pub outputs: Vec<CastOutputView>,
    pub listeners: Option<u32>,
    pub session_peak: u32,
    pub since: Option<u64>,
}

// [CTL-39]
#[derive(Serialize, Clone, Default)]
pub struct HistoryView {
    pub all: Vec<HistoryRow>,
    pub morceau: Vec<HistoryRow>,
    pub jingle: Vec<HistoryRow>,
    #[serde(rename = "pub")]
    pub pub_: Vec<HistoryRow>,
    pub stats: Option<LibraryStats>,
    pub current: Option<i64>,
    pub capacity: u32,
}

// [CTL-123]
#[derive(Serialize, Clone)]
pub struct JingleView {
    pub index: usize,
    pub name: String,
    pub path: PathBuf,
    pub duration: Option<f64>,
    pub ready: bool,
    pub error: Option<String>,
    pub playing: bool,
}

// [CTL-124]
#[derive(Serialize, Clone)]
pub struct JinglesView {
    pub items: Vec<JingleView>,
    pub dirs: Vec<PathBuf>,
    pub scanning: bool,
}
