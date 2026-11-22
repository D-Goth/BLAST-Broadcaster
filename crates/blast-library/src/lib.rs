use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub use rusqlite::Error as DbError;

// [LIB-01]
const SCHEMA_VERSION: i64 = 1;

// [LIB-02]
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tracks (
    id          INTEGER PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    title       TEXT NOT NULL DEFAULT '',
    artist      TEXT NOT NULL DEFAULT '',
    album       TEXT NOT NULL DEFAULT '',
    genre       TEXT NOT NULL DEFAULT '',
    year        INTEGER,
    duration    REAL,
    category    TEXT NOT NULL DEFAULT 'Musique',
    added_at    INTEGER NOT NULL,
    play_count  INTEGER NOT NULL DEFAULT 0,
    last_played INTEGER
);
CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist COLLATE NOCASE);
CREATE TABLE IF NOT EXISTS historique_diffusion (
    id              INTEGER PRIMARY KEY,
    track_id        INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
    path            TEXT NOT NULL,
    title           TEXT NOT NULL,
    artist          TEXT NOT NULL,
    started_at      INTEGER NOT NULL,
    duration_played REAL,
    on_air          INTEGER NOT NULL DEFAULT 0,
    listeners       INTEGER
);
CREATE INDEX IF NOT EXISTS idx_hist_started ON historique_diffusion(started_at);
CREATE INDEX IF NOT EXISTS idx_hist_artist ON historique_diffusion(artist COLLATE NOCASE, started_at);
CREATE INDEX IF NOT EXISTS idx_hist_path ON historique_diffusion(path, started_at);
";

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

// [LIB-08]
pub struct Library {
    conn: Connection,
}

impl Library {
    // [LIB-09]
    pub fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(3))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, DbError> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)?;
        conn.execute(
            "INSERT INTO meta(key, value) VALUES ('schema_version', ?1)
             ON CONFLICT(key) DO NOTHING",
            params![SCHEMA_VERSION.to_string()],
        )?;
        Ok(Self { conn })
    }

    // [LIB-10]
    pub fn upsert_tracks(&mut self, tracks: &[TrackMeta], now: i64) -> Result<usize, DbError> {
        let tx = self.conn.transaction()?;
        {
            let mut st = tx.prepare_cached(
                "INSERT INTO tracks(path, title, artist, album, genre, year, duration, added_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(path) DO UPDATE SET
                    title = excluded.title, artist = excluded.artist, album = excluded.album,
                    genre = excluded.genre, year = excluded.year, duration = excluded.duration",
            )?;
            for t in tracks {
                st.execute(params![
                    t.path, t.title, t.artist, t.album, t.genre, t.year, t.duration, now
                ])?;
            }
        }
        tx.commit()?;
        Ok(tracks.len())
    }

    // [LIB-11]
    pub fn record_play(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        at: i64,
        on_air: bool,
        listeners: Option<u32>,
    ) -> Result<i64, DbError> {
        let track_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM tracks WHERE path = ?1",
                params![path],
                |r| r.get(0),
            )
            .optional()?;
        self.conn.execute(
            "INSERT INTO historique_diffusion(track_id, path, title, artist, started_at, on_air, listeners)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![track_id, path, title, artist, at, on_air, listeners],
        )?;
        let id = self.conn.last_insert_rowid();
        if let Some(tid) = track_id {
            self.conn.execute(
                "UPDATE tracks SET play_count = play_count + 1, last_played = ?2 WHERE id = ?1",
                params![tid, at],
            )?;
        }
        Ok(id)
    }

    // [LIB-12]
    pub fn finish_play(&self, id: i64, duration_played: f64) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE historique_diffusion SET duration_played = ?2 WHERE id = ?1",
            params![id, duration_played],
        )?;
        Ok(())
    }

    // [LIB-13]
    pub fn check(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        now: i64,
        rules: &SeparationRules,
    ) -> Result<Option<Violation>, DbError> {
        if !rules.enabled {
            return Ok(None);
        }
        if rules.title_minutes > 0 {
            let since = now - rules.title_minutes as i64 * 60;
            let last: Option<i64> = self.conn.query_row(
                "SELECT MAX(started_at) FROM historique_diffusion
                 WHERE started_at >= ?1
                   AND (path = ?2 OR (?3 <> '' AND title = ?3 COLLATE NOCASE AND artist = ?4 COLLATE NOCASE))",
                params![since, path, title.trim(), artist.trim()],
                |r| r.get(0),
            )?;
            if let Some(t) = last {
                return Ok(Some(Violation::Title {
                    minutes_ago: ((now - t).max(0) / 60) as u32,
                    limit: rules.title_minutes,
                }));
            }
        }
        let artist = artist.trim();
        if rules.artist_minutes > 0 && !artist.is_empty() {
            let since = now - rules.artist_minutes as i64 * 60;
            let last: Option<i64> = self.conn.query_row(
                "SELECT MAX(started_at) FROM historique_diffusion
                 WHERE started_at >= ?1 AND artist = ?2 COLLATE NOCASE",
                params![since, artist],
                |r| r.get(0),
            )?;
            if let Some(t) = last {
                return Ok(Some(Violation::Artist {
                    minutes_ago: ((now - t).max(0) / 60) as u32,
                    limit: rules.artist_minutes,
                }));
            }
        }
        Ok(None)
    }

    // [LIB-14]
    pub fn recent(&self, limit: u32) -> Result<Vec<HistoryRow>, DbError> {
        self.history_between(0, i64::MAX, limit)
    }

    // [LIB-15]
    pub fn history_between(
        &self,
        from: i64,
        to: i64,
        limit: u32,
    ) -> Result<Vec<HistoryRow>, DbError> {
        let mut st = self.conn.prepare_cached(
            "SELECT id, path, title, artist, started_at, duration_played, on_air, listeners
             FROM historique_diffusion
             WHERE started_at >= ?1 AND started_at < ?2
             ORDER BY started_at DESC, id DESC LIMIT ?3",
        )?;
        let rows = st.query_map(params![from, to, limit], |r| {
            Ok(HistoryRow {
                id: r.get(0)?,
                path: r.get(1)?,
                title: r.get(2)?,
                artist: r.get(3)?,
                started_at: r.get(4)?,
                duration_played: r.get(5)?,
                on_air: r.get::<_, i64>(6)? != 0,
                listeners: r.get(7)?,
            })
        })?;
        rows.collect()
    }

    // [LIB-16]
    pub fn purge_older_than(&self, cutoff: i64) -> Result<usize, DbError> {
        self.conn.execute(
            "DELETE FROM historique_diffusion WHERE started_at < ?1",
            params![cutoff],
        )
    }

    // [LIB-17]
    pub fn clear_history(&self) -> Result<usize, DbError> {
        self.conn.execute("DELETE FROM historique_diffusion", [])
    }

    // [LIB-18]
    pub fn stats(&self) -> Result<LibraryStats, DbError> {
        Ok(LibraryStats {
            tracks: self
                .conn
                .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))?,
            plays: self
                .conn
                .query_row("SELECT COUNT(*) FROM historique_diffusion", [], |r| {
                    r.get(0)
                })?,
        })
    }
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
    let mut out =
        String::from("\u{feff}date;artiste;titre;duree_diffusee_s;antenne;auditeurs;fichier\r\n");
    for r in rows.iter().rev() {
        out.push_str(&format!(
            "{};{};{};{};{};{};{}\r\n",
            format_time(r.started_at),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(path: &str, title: &str, artist: &str) -> TrackMeta {
        TrackMeta {
            path: path.into(),
            title: title.into(),
            artist: artist.into(),
            ..Default::default()
        }
    }

    #[test]
    fn records_and_counts_plays() {
        let mut lib = Library::open_in_memory().unwrap();
        lib.upsert_tracks(&[meta("/a.mp3", "Void Relay", "D-Goth")], 100)
            .unwrap();
        let id = lib
            .record_play("/a.mp3", "Void Relay", "D-Goth", 1_000, true, Some(4))
            .unwrap();
        lib.finish_play(id, 212.4).unwrap();
        let rows = lib.recent(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].duration_played, Some(212.4));
        assert_eq!(rows[0].listeners, Some(4));
        let count: i64 = lib
            .conn
            .query_row(
                "SELECT play_count FROM tracks WHERE path='/a.mp3'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(
            lib.stats().unwrap(),
            LibraryStats {
                tracks: 1,
                plays: 1
            }
        );
    }

    #[test]
    fn separation_rules() {
        let lib = Library::open_in_memory().unwrap();
        let rules = SeparationRules::default();
        let t0 = 1_000_000;
        lib.record_play("/a.mp3", "Void Relay", "D-Goth", t0, true, None)
            .unwrap();

        let same_file = lib
            .check("/a.mp3", "Void Relay", "D-Goth", t0 + 30 * 60, &rules)
            .unwrap();
        assert_eq!(
            same_file,
            Some(Violation::Title {
                minutes_ago: 30,
                limit: 240
            })
        );

        let copy = lib
            .check("/copie/a.flac", "void relay", "d-goth", t0 + 60, &rules)
            .unwrap();
        assert!(matches!(copy, Some(Violation::Title { .. })));

        let same_artist = lib
            .check("/b.mp3", "Orokin Dawn", "D-GOTH", t0 + 90 * 60, &rules)
            .unwrap();
        assert_eq!(
            same_artist,
            Some(Violation::Artist {
                minutes_ago: 90,
                limit: 120
            })
        );

        assert_eq!(
            lib.check("/b.mp3", "Orokin Dawn", "D-Goth", t0 + 121 * 60, &rules)
                .unwrap(),
            None
        );
        assert_eq!(
            lib.check("/c.mp3", "Autre", "Black-Lab", t0 + 60, &rules)
                .unwrap(),
            None
        );
        assert_eq!(
            lib.check("/d.mp3", "Sans artiste", "", t0 + 60, &rules)
                .unwrap(),
            None
        );

        let off = SeparationRules {
            enabled: false,
            ..rules
        };
        assert_eq!(
            lib.check("/a.mp3", "Void Relay", "D-Goth", t0 + 60, &off)
                .unwrap(),
            None
        );
    }

    #[test]
    fn purge_and_csv() {
        let lib = Library::open_in_memory().unwrap();
        lib.record_play("/old.mp3", "Old", "X", 100, false, None)
            .unwrap();
        lib.record_play("/new.mp3", "Titre; \"spécial\"", "Y", 10_000, true, Some(2))
            .unwrap();
        assert_eq!(lib.purge_older_than(5_000).unwrap(), 1);
        let rows = lib.recent(10).unwrap();
        assert_eq!(rows.len(), 1);
        let csv = history_csv(&rows, |t| t.to_string());
        assert!(csv.contains("10000;Y;\"Titre; \"\"spécial\"\"\";;oui;2;/new.mp3"));
    }

    #[test]
    fn opens_on_disk_twice() {
        let dir = std::env::temp_dir().join(format!("blast-lib-{}", std::process::id()));
        let path = dir.join("blast.db");
        let a = Library::open(&path).unwrap();
        a.record_play("/a.mp3", "A", "B", 1, true, None).unwrap();
        let b = Library::open(&path).unwrap();
        assert_eq!(b.recent(5).unwrap().len(), 1);
        drop(a);
        drop(b);
        let _ = std::fs::remove_dir_all(dir);
    }
}
