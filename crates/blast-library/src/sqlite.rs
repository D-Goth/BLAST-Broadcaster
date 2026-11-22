use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    HistoryRow, HistoryStore, Kind, LibraryStats, LibraryTrack, NewPlay, PlayLookup, StoreError,
    TrackMeta,
};

// [LIB-01]
const SCHEMA_VERSION: i64 = 3;

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
    kind        TEXT NOT NULL DEFAULT 'morceau',
    bpm         REAL,
    mtime       INTEGER,
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
    kind            TEXT NOT NULL DEFAULT 'morceau',
    started_at      INTEGER NOT NULL,
    duration_played REAL,
    on_air          INTEGER NOT NULL DEFAULT 0,
    listeners       INTEGER
);
CREATE INDEX IF NOT EXISTS idx_hist_started ON historique_diffusion(started_at);
CREATE INDEX IF NOT EXISTS idx_hist_artist ON historique_diffusion(artist COLLATE NOCASE, started_at);
CREATE INDEX IF NOT EXISTS idx_hist_path ON historique_diffusion(path, started_at);
";

// [LIB-27]
const INDEXES_V2: &str = "
CREATE INDEX IF NOT EXISTS idx_hist_kind ON historique_diffusion(kind, started_at);
CREATE INDEX IF NOT EXISTS idx_tracks_kind ON tracks(kind);
";

fn err(e: rusqlite::Error) -> StoreError {
    StoreError(e.to_string())
}

// [LIB-08]
pub struct SqliteStore {
    conn: Connection,
    label: String,
}

impl SqliteStore {
    // [LIB-09]
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path).map_err(err)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(err)?;
        conn.busy_timeout(std::time::Duration::from_secs(3))
            .map_err(err)?;
        Self::init(conn, format!("SQLite ({})", path.display()))
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(
            Connection::open_in_memory().map_err(err)?,
            "SQLite (mémoire)".into(),
        )
    }

    fn init(conn: Connection, label: String) -> Result<Self, StoreError> {
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(err)?;
        conn.execute_batch(SCHEMA).map_err(err)?;
        migrate(&conn).map_err(err)?;
        conn.execute_batch(INDEXES_V2).map_err(err)?;
        conn.execute(
            "INSERT INTO meta(key, value) VALUES ('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![SCHEMA_VERSION.to_string()],
        )
        .map_err(err)?;
        Ok(Self { conn, label })
    }
}

// [LIB-28]
fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    let columns = [
        ("tracks", "kind", "TEXT NOT NULL DEFAULT 'morceau'"),
        (
            "historique_diffusion",
            "kind",
            "TEXT NOT NULL DEFAULT 'morceau'",
        ),
        ("tracks", "bpm", "REAL"),
        ("tracks", "mtime", "INTEGER"),
    ];
    for (table, column, decl) in columns {
        let exists: bool = conn
            .prepare(&format!(
                "SELECT 1 FROM pragma_table_info('{table}') WHERE name = '{column}'"
            ))?
            .exists([])?;
        if !exists {
            conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"))?;
        }
    }
    Ok(())
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryRow> {
    let kind: String = r.get(4)?;
    Ok(HistoryRow {
        id: r.get(0)?,
        path: r.get(1)?,
        title: r.get(2)?,
        artist: r.get(3)?,
        kind: Kind::parse(&kind).unwrap_or_default(),
        started_at: r.get(5)?,
        duration_played: r.get(6)?,
        on_air: r.get::<_, i64>(7)? != 0,
        listeners: r.get(8)?,
    })
}

// [LIB-13]
impl PlayLookup for SqliteStore {
    fn last_title_play(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        since: i64,
    ) -> Result<Option<i64>, StoreError> {
        self.conn
            .query_row(
                "SELECT MAX(started_at) FROM historique_diffusion
                 WHERE started_at >= ?1 AND kind = 'morceau'
                   AND (path = ?2 OR (?3 <> '' AND title = ?3 COLLATE NOCASE AND artist = ?4 COLLATE NOCASE))",
                params![since, path, title, artist],
                |r| r.get(0),
            )
            .map_err(err)
    }

    fn last_artist_play(&self, artist: &str, since: i64) -> Result<Option<i64>, StoreError> {
        self.conn
            .query_row(
                "SELECT MAX(started_at) FROM historique_diffusion
                 WHERE started_at >= ?1 AND kind = 'morceau' AND artist = ?2 COLLATE NOCASE",
                params![since, artist],
                |r| r.get(0),
            )
            .map_err(err)
    }
}

impl HistoryStore for SqliteStore {
    fn backend(&self) -> String {
        self.label.clone()
    }

    // [LIB-10]
    fn upsert_tracks(&mut self, tracks: &[TrackMeta], now: i64) -> Result<usize, StoreError> {
        let tx = self.conn.transaction().map_err(err)?;
        {
            let mut st = tx
                .prepare_cached(
                    "INSERT INTO tracks(path, title, artist, album, genre, year, duration, kind, bpm, mtime, added_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                     ON CONFLICT(path) DO UPDATE SET
                        title = excluded.title, artist = excluded.artist, album = excluded.album,
                        genre = excluded.genre, year = excluded.year, duration = excluded.duration,
                        kind = excluded.kind, bpm = COALESCE(excluded.bpm, tracks.bpm),
                        mtime = COALESCE(excluded.mtime, tracks.mtime)",
                )
                .map_err(err)?;
            for t in tracks {
                st.execute(params![
                    t.path,
                    t.title,
                    t.artist,
                    t.album,
                    t.genre,
                    t.year,
                    t.duration,
                    t.kind.as_str(),
                    t.bpm,
                    t.mtime,
                    now
                ])
                .map_err(err)?;
            }
        }
        tx.commit().map_err(err)?;
        Ok(tracks.len())
    }

    // [LIB-11]
    fn record_play(&mut self, p: &NewPlay) -> Result<i64, StoreError> {
        let track_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM tracks WHERE path = ?1",
                params![p.path],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        self.conn
            .execute(
                "INSERT INTO historique_diffusion(track_id, path, title, artist, kind, started_at, on_air, listeners)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    track_id,
                    p.path,
                    p.title,
                    p.artist,
                    p.kind.as_str(),
                    p.started_at,
                    p.on_air,
                    p.listeners
                ],
            )
            .map_err(err)?;
        let id = self.conn.last_insert_rowid();
        if let Some(tid) = track_id {
            self.conn
                .execute(
                    "UPDATE tracks SET play_count = play_count + 1, last_played = ?2 WHERE id = ?1",
                    params![tid, p.started_at],
                )
                .map_err(err)?;
        }
        Ok(id)
    }

    // [LIB-12]
    fn finish_play(&mut self, id: i64, duration_played: f64) -> Result<(), StoreError> {
        self.conn
            .execute(
                "UPDATE historique_diffusion SET duration_played = ?2 WHERE id = ?1",
                params![id, duration_played],
            )
            .map_err(err)?;
        Ok(())
    }

    // [LIB-15]
    fn history_between(
        &self,
        from: i64,
        to: i64,
        kind: Option<Kind>,
        limit: u32,
    ) -> Result<Vec<HistoryRow>, StoreError> {
        let mut st = self
            .conn
            .prepare_cached(
                "SELECT id, path, title, artist, kind, started_at, duration_played, on_air, listeners
                 FROM historique_diffusion
                 WHERE started_at >= ?1 AND started_at < ?2 AND (?3 IS NULL OR kind = ?3)
                 ORDER BY started_at DESC, id DESC LIMIT ?4",
            )
            .map_err(err)?;
        let rows = st
            .query_map(params![from, to, kind.map(Kind::as_str), limit], row)
            .map_err(err)?;
        rows.collect::<Result<_, _>>().map_err(err)
    }

    // [LIB-16]
    fn purge_older_than(&mut self, cutoff: i64) -> Result<usize, StoreError> {
        self.conn
            .execute(
                "DELETE FROM historique_diffusion WHERE started_at < ?1",
                params![cutoff],
            )
            .map_err(err)
    }

    // [LIB-17]
    fn clear_history(&mut self) -> Result<usize, StoreError> {
        self.conn
            .execute("DELETE FROM historique_diffusion", [])
            .map_err(err)
    }

    // [LIB-42]
    fn tracks(&self) -> Result<Vec<LibraryTrack>, StoreError> {
        let mut st = self
            .conn
            .prepare_cached(
                "SELECT path, title, artist, genre, year, bpm, duration, kind, play_count, last_played
                 FROM tracks ORDER BY path",
            )
            .map_err(err)?;
        let rows = st
            .query_map([], |r| {
                let kind: String = r.get(7)?;
                Ok(LibraryTrack {
                    path: r.get(0)?,
                    title: r.get(1)?,
                    artist: r.get(2)?,
                    genre: r.get(3)?,
                    year: r.get(4)?,
                    bpm: r.get(5)?,
                    duration: r.get(6)?,
                    kind: Kind::parse(&kind).unwrap_or_default(),
                    play_count: r.get(8)?,
                    last_played: r.get(9)?,
                })
            })
            .map_err(err)?;
        rows.collect::<Result<_, _>>().map_err(err)
    }

    // [LIB-43]
    fn track_mtimes(&self) -> Result<std::collections::HashMap<String, i64>, StoreError> {
        let mut st = self
            .conn
            .prepare_cached("SELECT path, mtime FROM tracks WHERE mtime IS NOT NULL")
            .map_err(err)?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map_err(err)?;
        rows.collect::<Result<_, _>>().map_err(err)
    }

    // [LIB-18]
    fn stats(&self) -> Result<LibraryStats, StoreError> {
        let count = |sql: &str| self.conn.query_row(sql, [], |r| r.get(0)).map_err(err);
        Ok(LibraryStats {
            tracks: count("SELECT COUNT(*) FROM tracks")?,
            plays: count("SELECT COUNT(*) FROM historique_diffusion")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check_rules, SeparationRules, Violation};

    pub fn play(path: &str, title: &str, artist: &str, kind: Kind, at: i64) -> NewPlay {
        NewPlay {
            path: path.into(),
            title: title.into(),
            artist: artist.into(),
            kind,
            started_at: at,
            on_air: true,
            listeners: None,
        }
    }

    #[test]
    fn records_and_counts_plays() {
        let mut lib = SqliteStore::open_in_memory().unwrap();
        lib.upsert_tracks(
            &[TrackMeta {
                path: "/a.mp3".into(),
                title: "Void Relay".into(),
                artist: "D-Goth".into(),
                ..Default::default()
            }],
            100,
        )
        .unwrap();
        let mut p = play("/a.mp3", "Void Relay", "D-Goth", Kind::Morceau, 1_000);
        p.listeners = Some(4);
        let id = lib.record_play(&p).unwrap();
        lib.finish_play(id, 212.4).unwrap();
        let rows = lib.history_between(0, i64::MAX, None, 10).unwrap();
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
    fn separation_rules_in_sql() {
        let mut lib = SqliteStore::open_in_memory().unwrap();
        let rules = SeparationRules::default();
        let t0 = 1_000_000;
        lib.record_play(&play("/a.mp3", "Void Relay", "D-Goth", Kind::Morceau, t0))
            .unwrap();
        lib.record_play(&play("/j.mp3", "Jingle", "Black-Lab", Kind::Jingle, t0))
            .unwrap();
        let check =
            |p: &str, t: &str, a: &str, now: i64| check_rules(&lib, p, t, a, now, &rules).unwrap();
        assert_eq!(
            check("/a.mp3", "Void Relay", "D-Goth", t0 + 30 * 60),
            Some(Violation::Title {
                minutes_ago: 30,
                limit: 240
            })
        );
        assert!(matches!(
            check("/copie/a.flac", "void relay", "d-goth", t0 + 60),
            Some(Violation::Title { .. })
        ));
        assert_eq!(
            check("/b.mp3", "Orokin Dawn", "D-GOTH", t0 + 90 * 60),
            Some(Violation::Artist {
                minutes_ago: 90,
                limit: 120
            })
        );
        assert_eq!(
            check("/b.mp3", "Orokin Dawn", "D-Goth", t0 + 121 * 60),
            None
        );
        assert_eq!(check("/c.mp3", "Autre", "Black-Lab", t0 + 60), None);
        assert_eq!(check("/d.mp3", "Sans artiste", "", t0 + 60), None);
    }

    #[test]
    fn filters_by_kind_and_purges() {
        let mut lib = SqliteStore::open_in_memory().unwrap();
        lib.record_play(&play("/old.mp3", "Old", "X", Kind::Morceau, 100))
            .unwrap();
        lib.record_play(&play("/j.mp3", "J", "", Kind::Jingle, 10_000))
            .unwrap();
        lib.record_play(&play("/p.mp3", "P", "", Kind::Pub, 10_100))
            .unwrap();
        let pubs = lib
            .history_between(0, i64::MAX, Some(Kind::Pub), 10)
            .unwrap();
        assert_eq!(pubs.len(), 1);
        assert_eq!(pubs[0].kind, Kind::Pub);
        assert_eq!(lib.purge_older_than(5_000).unwrap(), 1);
        assert_eq!(lib.history_between(0, i64::MAX, None, 10).unwrap().len(), 2);
    }

    #[test]
    fn csv_has_the_type_column() {
        let mut lib = SqliteStore::open_in_memory().unwrap();
        let mut p = play("/new.mp3", "Titre; \"spécial\"", "Y", Kind::Pub, 10_000);
        p.listeners = Some(2);
        lib.record_play(&p).unwrap();
        let rows = lib.history_between(0, i64::MAX, None, 10).unwrap();
        let csv = crate::history_csv(&rows, |t| t.to_string());
        assert!(csv.starts_with("\u{feff}date;type;artiste"));
        assert!(csv.contains("10000;pub;Y;\"Titre; \"\"spécial\"\"\";;oui;2;/new.mp3"));
    }

    #[test]
    fn migrates_a_version_1_database() {
        let dir = std::env::temp_dir().join(format!("blast-mig-{}", std::process::id()));
        let path = dir.join("blast.db");
        std::fs::create_dir_all(&dir).unwrap();
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE tracks (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, title TEXT NOT NULL DEFAULT '',
                    artist TEXT NOT NULL DEFAULT '', album TEXT NOT NULL DEFAULT '', genre TEXT NOT NULL DEFAULT '',
                    year INTEGER, duration REAL, category TEXT NOT NULL DEFAULT 'Musique', added_at INTEGER NOT NULL,
                    play_count INTEGER NOT NULL DEFAULT 0, last_played INTEGER);
                 CREATE TABLE historique_diffusion (id INTEGER PRIMARY KEY, track_id INTEGER, path TEXT NOT NULL,
                    title TEXT NOT NULL, artist TEXT NOT NULL, started_at INTEGER NOT NULL, duration_played REAL,
                    on_air INTEGER NOT NULL DEFAULT 0, listeners INTEGER);
                 INSERT INTO historique_diffusion(path, title, artist, started_at) VALUES ('/a.mp3', 'A', 'B', 5);",
            )
            .unwrap();
        }
        let lib = SqliteStore::open(&path).unwrap();
        let rows = lib.history_between(0, i64::MAX, None, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, Kind::Morceau);
        let v: String = lib
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key='schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v, "3");
        drop(lib);
        let _ = std::fs::remove_dir_all(dir);
    }
}
