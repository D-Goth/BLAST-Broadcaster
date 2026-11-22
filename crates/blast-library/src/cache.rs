use std::collections::VecDeque;

use crate::{HistoryRow, Kind, PlayLookup, StoreError};

// [LIB-29]
#[derive(Debug, Clone)]
struct Ring {
    rows: VecDeque<HistoryRow>,
    complete: bool,
}

impl Ring {
    fn new(cap: usize) -> Self {
        Self {
            rows: VecDeque::with_capacity(cap),
            complete: true,
        }
    }

    fn push(&mut self, row: HistoryRow, cap: usize) {
        self.rows.push_front(row);
        while self.rows.len() > cap {
            self.rows.pop_back();
            self.complete = false;
        }
    }

    fn covers(&self, since: i64) -> bool {
        self.complete || self.rows.back().is_some_and(|r| r.started_at <= since)
    }
}

// [LIB-30]
#[derive(Debug, Clone)]
pub struct HistoryCache {
    cap: usize,
    all: Ring,
    by_kind: [Ring; 3],
}

impl HistoryCache {
    pub fn new(cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            cap,
            all: Ring::new(cap),
            by_kind: [Ring::new(cap), Ring::new(cap), Ring::new(cap)],
        }
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    // [LIB-31]
    pub fn prefill(&mut self, kind: Option<Kind>, newest_first: Vec<HistoryRow>) {
        let ring = match kind {
            None => &mut self.all,
            Some(k) => &mut self.by_kind[k.index()],
        };
        ring.complete = newest_first.len() < self.cap;
        ring.rows = newest_first.into_iter().take(self.cap).collect();
    }

    // [LIB-32]
    pub fn push(&mut self, row: HistoryRow) {
        let k = row.kind.index();
        self.by_kind[k].push(row.clone(), self.cap);
        self.all.push(row, self.cap);
    }

    // [LIB-33]
    pub fn finish(&mut self, id: i64, duration_played: f64) {
        for ring in std::iter::once(&mut self.all).chain(self.by_kind.iter_mut()) {
            if let Some(r) = ring.rows.iter_mut().find(|r| r.id == id) {
                r.duration_played = Some(duration_played);
            }
        }
    }

    pub fn rows(&self, kind: Option<Kind>) -> &VecDeque<HistoryRow> {
        match kind {
            None => &self.all.rows,
            Some(k) => &self.by_kind[k.index()].rows,
        }
    }

    // [LIB-34]
    pub fn covers(&self, kind: Kind, since: i64) -> bool {
        self.by_kind[kind.index()].covers(since)
    }

    pub fn purge_older_than(&mut self, cutoff: i64) {
        for ring in std::iter::once(&mut self.all).chain(self.by_kind.iter_mut()) {
            ring.rows.retain(|r| r.started_at >= cutoff);
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new(self.cap);
    }

    fn songs(&self, since: i64) -> impl Iterator<Item = &HistoryRow> {
        self.by_kind[Kind::Morceau.index()]
            .rows
            .iter()
            .take_while(move |r| r.started_at >= since)
    }
}

// [LIB-35]
impl PlayLookup for HistoryCache {
    fn last_title_play(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        since: i64,
    ) -> Result<Option<i64>, StoreError> {
        Ok(self
            .songs(since)
            .find(|r| {
                r.path == path
                    || (!title.is_empty()
                        && r.title.trim().eq_ignore_ascii_case(title)
                        && r.artist.trim().eq_ignore_ascii_case(artist))
            })
            .map(|r| r.started_at))
    }

    fn last_artist_play(&self, artist: &str, since: i64) -> Result<Option<i64>, StoreError> {
        Ok(self
            .songs(since)
            .find(|r| r.artist.trim().eq_ignore_ascii_case(artist))
            .map(|r| r.started_at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, kind: Kind, at: i64, artist: &str) -> HistoryRow {
        HistoryRow {
            id,
            path: format!("/{id}.mp3"),
            title: format!("T{id}"),
            artist: artist.into(),
            kind,
            started_at: at,
            duration_played: None,
            on_air: true,
            listeners: None,
        }
    }

    #[test]
    fn keeps_global_and_per_kind_rings() {
        let mut c = HistoryCache::new(3);
        for i in 0..5 {
            c.push(row(i, Kind::Morceau, i * 100, "A"));
        }
        c.push(row(10, Kind::Jingle, 600, ""));
        assert_eq!(c.rows(None).len(), 3);
        assert_eq!(c.rows(None)[0].id, 10);
        assert_eq!(c.rows(Some(Kind::Morceau)).len(), 3);
        assert_eq!(c.rows(Some(Kind::Morceau))[0].id, 4);
        assert_eq!(c.rows(Some(Kind::Jingle)).len(), 1);
        assert_eq!(c.rows(Some(Kind::Pub)).len(), 0);
        c.finish(4, 180.0);
        assert_eq!(c.rows(None)[1].duration_played, Some(180.0));
        assert_eq!(c.rows(Some(Kind::Morceau))[0].duration_played, Some(180.0));
    }

    #[test]
    fn knows_what_it_covers() {
        let mut c = HistoryCache::new(2);
        c.push(row(1, Kind::Morceau, 100, "A"));
        assert!(c.covers(Kind::Morceau, 0));
        c.push(row(2, Kind::Morceau, 200, "A"));
        c.push(row(3, Kind::Morceau, 300, "A"));
        assert!(!c.covers(Kind::Morceau, 150));
        assert!(c.covers(Kind::Morceau, 250));
        c.prefill(Some(Kind::Pub), vec![row(9, Kind::Pub, 50, "")]);
        assert!(c.covers(Kind::Pub, 0));
    }

    #[test]
    fn lookups_ignore_jingles_and_old_rows() {
        let mut c = HistoryCache::new(10);
        c.push(row(1, Kind::Morceau, 100, "D-Goth"));
        c.push(row(2, Kind::Jingle, 200, "Black-Lab"));
        assert_eq!(c.last_artist_play("d-goth", 50).unwrap(), Some(100));
        assert_eq!(c.last_artist_play("d-goth", 150).unwrap(), None);
        assert_eq!(c.last_artist_play("Black-Lab", 0).unwrap(), None);
        assert_eq!(c.last_title_play("/1.mp3", "", "", 0).unwrap(), Some(100));
        assert_eq!(
            c.last_title_play("/x.mp3", "t1", "D-GOTH", 0).unwrap(),
            Some(100)
        );
    }
}
