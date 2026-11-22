use std::path::{Path, PathBuf};

use blast_core::{is_audio_path, probe_file};
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Entry {
    pub id: u64,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: Option<i32>,
    pub duration: Option<f64>,
    pub error: Option<String>,
    pub blocked: Option<String>,
}

impl Entry {
    pub fn to_meta(&self) -> blast_library::TrackMeta {
        blast_library::TrackMeta {
            path: self.path.to_string_lossy().into_owned(),
            title: self.title.clone(),
            artist: self.artist.clone(),
            album: self.album.clone(),
            genre: self.genre.clone(),
            year: self.year,
            duration: self.duration,
        }
    }
}

#[derive(Debug, Default)]
pub struct Playlist {
    entries: Vec<Entry>,
    next_id: u64,
}

impl Playlist {
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn total_secs(&self) -> f64 {
        self.entries.iter().filter_map(|e| e.duration).sum()
    }

    pub fn insert(&mut self, mut items: Vec<Entry>, index: Option<usize>) {
        for e in items.iter_mut() {
            self.next_id += 1;
            e.id = self.next_id;
        }
        let at = index.unwrap_or(self.entries.len()).min(self.entries.len());
        self.entries.splice(at..at, items);
    }

    pub fn remove(&mut self, id: u64) -> Option<Entry> {
        let pos = self.entries.iter().position(|e| e.id == id)?;
        Some(self.entries.remove(pos))
    }

    pub fn move_to(&mut self, id: u64, to: usize) {
        let Some(from) = self.entries.iter().position(|e| e.id == id) else {
            return;
        };
        let item = self.entries.remove(from);
        let to = if to > from { to - 1 } else { to };
        let to = to.min(self.entries.len());
        self.entries.insert(to, item);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    #[cfg(test)]
    pub fn take_next(&mut self) -> Option<Entry> {
        let pos = self.entries.iter().position(|e| e.error.is_none())?;
        Some(self.entries.remove(pos))
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
    items.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    for p in items {
        if p.is_dir() {
            walk(&p, out);
        } else if is_audio_path(&p) {
            out.push(p);
        }
    }
}

pub fn expand(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            walk(p, &mut out);
        } else if is_audio_path(p) {
            out.push(p.clone());
        }
    }
    out
}

pub fn make_entry(path: &Path) -> Entry {
    match probe_file(path) {
        Ok(info) => Entry {
            id: 0,
            path: path.to_path_buf(),
            title: info.display_title(),
            artist: info.artist.unwrap_or_default(),
            album: info.album.unwrap_or_default(),
            genre: info.genre.unwrap_or_default(),
            year: info.year,
            duration: info.duration_secs,
            error: None,
            blocked: None,
        },
        Err(e) => Entry {
            id: 0,
            path: path.to_path_buf(),
            title: path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            error: Some(e.to_string()),
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(t: &str) -> Entry {
        Entry {
            path: PathBuf::from(t),
            title: t.into(),
            duration: Some(1.0),
            ..Default::default()
        }
    }

    fn titles(p: &Playlist) -> Vec<String> {
        p.entries().iter().map(|e| e.title.clone()).collect()
    }

    #[test]
    fn insert_move_take() {
        let mut p = Playlist::default();
        p.insert(vec![e("a"), e("b"), e("c")], None);
        p.insert(vec![e("x")], Some(1));
        assert_eq!(titles(&p), ["a", "x", "b", "c"]);
        let c = p.entries()[3].id;
        p.move_to(c, 0);
        assert_eq!(titles(&p), ["c", "a", "x", "b"]);
        let c = p.entries()[0].id;
        p.move_to(c, 4);
        assert_eq!(titles(&p), ["a", "x", "b", "c"]);
        assert_eq!(p.take_next().unwrap().title, "a");
        assert_eq!(p.total_secs(), 3.0);
    }

    #[test]
    fn take_next_skips_errors() {
        let mut p = Playlist::default();
        let mut bad = e("bad");
        bad.error = Some("x".into());
        p.insert(vec![bad, e("ok")], None);
        assert_eq!(p.take_next().unwrap().title, "ok");
        assert_eq!(p.entries().len(), 1);
    }
}
