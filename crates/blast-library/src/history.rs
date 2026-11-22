use std::cell::Cell;
use std::collections::VecDeque;

use crate::{
    check_rules, rules_window, HistoryCache, HistoryRow, HistoryStore, Kind, LibraryStats, NewPlay,
    SeparationRules, StoreError, TrackMeta, Violation,
};

// [LIB-36]
pub struct History {
    store: Box<dyn HistoryStore>,
    cache: HistoryCache,
    offline_id: i64,
    store_lookups: Cell<u64>,
}

impl History {
    // [LIB-37]
    pub fn new(store: Box<dyn HistoryStore>, capacity: usize) -> Result<Self, StoreError> {
        let mut h = Self {
            store,
            cache: HistoryCache::new(capacity),
            offline_id: 0,
            store_lookups: Cell::new(0),
        };
        h.reload()?;
        Ok(h)
    }

    pub fn reload(&mut self) -> Result<(), StoreError> {
        let cap = self.cache.capacity() as u32;
        let mut cache = HistoryCache::new(cap as usize);
        cache.prefill(None, self.store.history_between(0, i64::MAX, None, cap)?);
        for k in Kind::ALL {
            cache.prefill(
                Some(k),
                self.store.history_between(0, i64::MAX, Some(k), cap)?,
            );
        }
        self.cache = cache;
        Ok(())
    }

    pub fn set_capacity(&mut self, capacity: usize) -> Result<(), StoreError> {
        if capacity.max(1) == self.cache.capacity() {
            return Ok(());
        }
        self.cache = HistoryCache::new(capacity);
        self.reload()
    }

    pub fn backend(&self) -> String {
        self.store.backend()
    }

    pub fn upsert_tracks(&mut self, tracks: &[TrackMeta], now: i64) -> Result<usize, StoreError> {
        self.store.upsert_tracks(tracks, now)
    }

    // [LIB-38]
    pub fn record(&mut self, play: NewPlay) -> (i64, Option<StoreError>) {
        let (id, error) = match self.store.record_play(&play) {
            Ok(id) => (id, None),
            Err(e) => {
                self.offline_id -= 1;
                (self.offline_id, Some(e))
            }
        };
        self.cache.push(HistoryRow {
            id,
            path: play.path,
            title: play.title,
            artist: play.artist,
            kind: play.kind,
            started_at: play.started_at,
            duration_played: None,
            on_air: play.on_air,
            listeners: play.listeners,
        });
        (id, error)
    }

    pub fn finish(&mut self, id: i64, duration_played: f64) -> Result<(), StoreError> {
        self.cache.finish(id, duration_played);
        if id > 0 {
            self.store.finish_play(id, duration_played)
        } else {
            Ok(())
        }
    }

    // [LIB-39]
    pub fn check(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        now: i64,
        rules: &SeparationRules,
    ) -> Result<Option<Violation>, StoreError> {
        if self.cache.covers(Kind::Morceau, now - rules_window(rules)) {
            check_rules(&self.cache, path, title, artist, now, rules)
        } else {
            self.store_lookups.set(self.store_lookups.get() + 1);
            check_rules(self.store.as_ref(), path, title, artist, now, rules)
        }
    }

    pub fn store_lookups(&self) -> u64 {
        self.store_lookups.get()
    }

    pub fn rows(&self, kind: Option<Kind>) -> &VecDeque<HistoryRow> {
        self.cache.rows(kind)
    }

    // [LIB-40]
    pub fn between(
        &self,
        from: i64,
        to: i64,
        kind: Option<Kind>,
    ) -> Result<Vec<HistoryRow>, StoreError> {
        self.store.history_between(from, to, kind, u32::MAX)
    }

    pub fn purge_older_than(&mut self, cutoff: i64) -> Result<usize, StoreError> {
        let n = self.store.purge_older_than(cutoff)?;
        self.cache.purge_older_than(cutoff);
        Ok(n)
    }

    pub fn clear(&mut self) -> Result<usize, StoreError> {
        let n = self.store.clear_history()?;
        self.cache.clear();
        Ok(n)
    }

    pub fn stats(&self) -> Result<LibraryStats, StoreError> {
        self.store.stats()
    }

    pub fn tracks(&self) -> Result<Vec<crate::LibraryTrack>, StoreError> {
        self.store.tracks()
    }

    pub fn track_mtimes(&self) -> Result<std::collections::HashMap<String, i64>, StoreError> {
        self.store.track_mtimes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteStore;

    fn play(path: &str, artist: &str, kind: Kind, at: i64) -> NewPlay {
        NewPlay {
            path: path.into(),
            title: path.trim_start_matches('/').into(),
            artist: artist.into(),
            kind,
            started_at: at,
            on_air: true,
            listeners: None,
        }
    }

    #[test]
    fn single_entry_point_feeds_cache_and_store() {
        let mut h = History::new(Box::new(SqliteStore::open_in_memory().unwrap()), 200).unwrap();
        let (id, e) = h.record(play("/a.mp3", "D-Goth", Kind::Morceau, 1_000));
        assert!(e.is_none());
        h.record(play("/j.mp3", "", Kind::Jingle, 1_100));
        h.finish(id, 200.0).unwrap();
        assert_eq!(h.rows(None).len(), 2);
        assert_eq!(h.rows(Some(Kind::Jingle)).len(), 1);
        assert_eq!(h.rows(Some(Kind::Morceau))[0].duration_played, Some(200.0));
        let stored = h.between(0, i64::MAX, Some(Kind::Morceau)).unwrap();
        assert_eq!(stored[0].duration_played, Some(200.0));
        assert_eq!(h.stats().unwrap().plays, 2);
    }

    #[test]
    fn rules_use_memory_and_fall_back_to_the_store() {
        let mut store = SqliteStore::open_in_memory().unwrap();
        for i in 0..5 {
            store
                .record_play(&play(&format!("/{i}.mp3"), "X", Kind::Morceau, i * 3600))
                .unwrap();
        }
        let rules = SeparationRules {
            enabled: true,
            artist_minutes: 60,
            title_minutes: 120,
        };
        let h = History::new(Box::new(store), 3).unwrap();
        let now = 4 * 3600 + 60;
        assert!(h.check("/z.mp3", "z", "X", now, &rules).unwrap().is_some());
        assert_eq!(h.store_lookups(), 0);
        let week = SeparationRules {
            title_minutes: 7 * 24 * 60,
            ..rules
        };
        assert!(matches!(
            h.check("/0.mp3", "0.mp3", "Y", now, &week).unwrap(),
            Some(Violation::Title { .. })
        ));
        assert_eq!(h.store_lookups(), 1);
    }

    #[test]
    fn prefills_from_the_store_on_start() {
        let mut store = SqliteStore::open_in_memory().unwrap();
        for i in 0..4 {
            store
                .record_play(&play(&format!("/{i}.mp3"), "X", Kind::Morceau, i))
                .unwrap();
        }
        store
            .record_play(&play("/p.mp3", "", Kind::Pub, 10))
            .unwrap();
        let mut h = History::new(Box::new(store), 3).unwrap();
        assert_eq!(h.rows(None).len(), 3);
        assert_eq!(h.rows(None)[0].kind, Kind::Pub);
        assert_eq!(h.rows(Some(Kind::Morceau)).len(), 3);
        assert_eq!(h.rows(Some(Kind::Pub)).len(), 1);
        h.set_capacity(10).unwrap();
        assert_eq!(h.rows(Some(Kind::Morceau)).len(), 4);
        h.clear().unwrap();
        assert!(h.rows(None).is_empty());
    }
}
