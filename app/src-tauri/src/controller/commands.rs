use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use blast_core::DeckId;

use crate::config::AppConfig;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum Deck {
    A,
    B,
}

impl Deck {
    pub fn id(self) -> DeckId {
        match self {
            Deck::A => DeckId::A,
            Deck::B => DeckId::B,
        }
    }

    pub fn index(self) -> usize {
        self.id().index()
    }

    pub fn from_index(i: usize) -> Deck {
        if i == 0 {
            Deck::A
        } else {
            Deck::B
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Deck::A => "A",
            Deck::B => "B",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ObsAction {
    Connect,
    Disconnect,
    Scene { name: String },
    Stream,
    Record,
    RecordPause,
    Replay,
    Snap,
    Volume { input: String, mul: f32 },
    Mute { input: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiCommand {
    Refresh,
    AddPaths {
        paths: Vec<PathBuf>,
        index: Option<usize>,
    },
    LoadPaths {
        deck: Deck,
        paths: Vec<PathBuf>,
    },
    Remove {
        id: u64,
    },
    Move {
        id: u64,
        to: usize,
    },
    Clear,
    LoadEntry {
        id: u64,
        deck: Deck,
    },
    Play {
        deck: Deck,
    },
    Pause {
        deck: Deck,
    },
    Stop {
        deck: Deck,
    },
    Eject {
        deck: Deck,
    },
    Seek {
        deck: Deck,
        fraction: f32,
    },
    Crossfader {
        value: f32,
    },
    Crossfade {
        to: Deck,
    },
    AutoMix {
        enabled: bool,
    },
    FadeSeconds {
        value: f32,
    },
    CastStart,
    CastStop,
    Next {
        deck: Deck,
    },
    MicOpen {
        open: bool,
    },
    PadPlay {
        index: u16,
    },
    PadStopAll,
    Obs(ObsAction),
    HistoryClear,
    #[serde(skip)]
    Config(Box<AppConfig>),
}
