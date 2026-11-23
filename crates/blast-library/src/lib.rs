mod cache;
mod history;
mod rotation;
mod sqlite;

use serde::{Deserialize, Serialize};

pub use cache::HistoryCache;
pub use history::History;
pub use rotation::{
    default_clock, path_key, pick_weighted, scheduled, weight, CategoryRule, Clock, PlaylistSets,
    Program, Rotation, RotationCategory, Scheduled, SCHEDULE_SLOTS,
};
pub use sqlite::SqliteStore;

// [LIB-21]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Morceau,
    Jingle,
    Pub,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Morceau, Kind::Jingle, Kind::Pub];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Morceau => "morceau",
            Kind::Jingle => "jingle",
            Kind::Pub => "pub",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == s)
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

// [LIB-22]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError(pub String);

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StoreError {}

// [LIB-03]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackMeta {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: Option<i32>,
    pub duration: Option<f64>,
    pub kind: Kind,
    pub bpm: Option<f32>,
    pub mtime: Option<i64>,
}

// [LIB-41]
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct LibraryTrack {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub genre: String,
    pub year: Option<i32>,
    pub bpm: Option<f32>,
    pub duration: Option<f64>,
    pub kind: Kind,
    pub play_count: i64,
    pub last_played: Option<i64>,
}

// [LIB-23]
#[derive(Debug, Clone, PartialEq)]
pub struct NewPlay {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub kind: Kind,
    pub started_at: i64,
    pub on_air: bool,
    pub listeners: Option<u32>,
}

// [LIB-04]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeparationRules {
    pub enabled: bool,
    pub artist_minutes: u32,
    pub title_minutes: u32,
}

impl Default for SeparationRules {
    fn default() -> Self {
        Self {
            enabled: true,
            artist_minutes: 120,
            title_minutes: 240,
        }
    }
}

// [LIB-05]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum Violation {
    Title { minutes_ago: u32, limit: u32 },
    Artist { minutes_ago: u32, limit: u32 },
}

impl Violation {
    pub fn describe(&self) -> String {
        match self {
            Violation::Title { minutes_ago, limit } => {
                format!("titre diffusé il y a {minutes_ago} min (règle : {limit} min)")
            }
            Violation::Artist { minutes_ago, limit } => {
                format!("artiste diffusé il y a {minutes_ago} min (règle : {limit} min)")
            }
        }
    }
}

// [LIB-06]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryRow {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub kind: Kind,
    pub started_at: i64,
    pub duration_played: Option<f64>,
    pub on_air: bool,
    pub listeners: Option<u32>,
}

// [LIB-07]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LibraryStats {
    pub tracks: i64,
    pub plays: i64,
}

// [LIB-24]
pub trait PlayLookup {
    fn last_title_play(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        since: i64,
    ) -> Result<Option<i64>, StoreError>;

    fn last_artist_play(&self, artist: &str, since: i64) -> Result<Option<i64>, StoreError>;
}

// [LIB-25]
pub trait HistoryStore: PlayLookup + Send {
    fn backend(&self) -> String;
    fn upsert_tracks(&mut self, tracks: &[TrackMeta], now: i64) -> Result<usize, StoreError>;
    fn record_play(&mut self, play: &NewPlay) -> Result<i64, StoreError>;
    fn finish_play(&mut self, id: i64, duration_played: f64) -> Result<(), StoreError>;
    fn history_between(
        &self,
        from: i64,
        to: i64,
        kind: Option<Kind>,
        limit: u32,
    ) -> Result<Vec<HistoryRow>, StoreError>;
    fn purge_older_than(&mut self, cutoff: i64) -> Result<usize, StoreError>;
    fn clear_history(&mut self) -> Result<usize, StoreError>;
    fn stats(&self) -> Result<LibraryStats, StoreError>;
    fn tracks(&self) -> Result<Vec<LibraryTrack>, StoreError>;
    fn track_mtimes(&self) -> Result<std::collections::HashMap<String, i64>, StoreError>;
}

// [LIB-26]
pub fn check_rules<L: PlayLookup + ?Sized>(
    source: &L,
    path: &str,
    title: &str,
    artist: &str,
    now: i64,
    rules: &SeparationRules,
) -> Result<Option<Violation>, StoreError> {
    if !rules.enabled {
        return Ok(None);
    }
    let ago = |t: i64| ((now - t).max(0) / 60) as u32;
    if rules.title_minutes > 0 {
        let since = now - rules.title_minutes as i64 * 60;
        if let Some(t) = source.last_title_play(path, title.trim(), artist.trim(), since)? {
            return Ok(Some(Violation::Title {
                minutes_ago: ago(t),
                limit: rules.title_minutes,
            }));
        }
    }
    let artist = artist.trim();
    if rules.artist_minutes > 0 && !artist.is_empty() {
        let since = now - rules.artist_minutes as i64 * 60;
        if let Some(t) = source.last_artist_play(artist, since)? {
            return Ok(Some(Violation::Artist {
                minutes_ago: ago(t),
                limit: rules.artist_minutes,
            }));
        }
    }
    Ok(None)
}

pub fn rules_window(rules: &SeparationRules) -> i64 {
    rules.title_minutes.max(rules.artist_minutes) as i64 * 60
}

// [LIB-19]
fn csv_field(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// [LIB-20]
pub fn history_csv(rows: &[HistoryRow], format_time: impl Fn(i64) -> String) -> String {
    let mut out = String::from(
        "\u{feff}date;type;artiste;titre;duree_diffusee_s;antenne;auditeurs;fichier\r\n",
    );
    for r in rows.iter().rev() {
        out.push_str(&format!(
            "{};{};{};{};{};{};{};{}\r\n",
            format_time(r.started_at),
            r.kind.as_str(),
            csv_field(&r.artist),
            csv_field(&r.title),
            r.duration_played
                .map(|d| format!("{d:.0}"))
                .unwrap_or_default(),
            if r.on_air { "oui" } else { "non" },
            r.listeners.map(|l| l.to_string()).unwrap_or_default(),
            csv_field(&r.path),
        ));
    }
    out
}
