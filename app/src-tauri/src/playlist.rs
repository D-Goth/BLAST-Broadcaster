use std::path::{Path, PathBuf};

use blast_core::{is_audio_path, probe_file};
use blast_library::Kind;
use serde::Serialize;

use crate::config::CategorySettings;

// [PL-01]
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
    pub kind: Kind,
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
            kind: self.kind,
        }
    }
}

// [PL-02]
#[derive(Debug, Default)]
pub struct Playlist {
    entries: Vec<Entry>,
    next_id: u64,
}

// [PL-03]
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

    pub fn reclassify(&mut self, cats: &CategorySettings) -> bool {
        let mut changed = false;
        for e in &mut self.entries {
            let kind = classify(&e.path, cats);
            changed |= kind != e.kind;
            e.kind = kind;
        }
        changed
    }

    #[cfg(test)]
    pub fn take_next(&mut self) -> Option<Entry> {
        let pos = self.entries.iter().position(|e| e.error.is_none())?;
        Some(self.entries.remove(pos))
    }
}

// [PL-04]
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

// [PL-05]
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

// [PL-07]
fn fold(word: &str) -> String {
    word.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'À' | 'Â' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'î' | 'ï' | 'Î' | 'Ï' => 'i',
            'ô' | 'ö' | 'Ô' | 'Ö' => 'o',
            'ù' | 'û' | 'ü' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

// [PL-08]
fn folder_matches(folder: &str, keywords: &[String]) -> bool {
    let words: Vec<String> = folder
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(fold)
        .collect();
    keywords.iter().map(|k| fold(k.trim())).any(|k| {
        !k.is_empty()
            && words
                .iter()
                .any(|w| *w == k || w.strip_suffix('s') == Some(k.as_str()))
    })
}

// [PL-09]
pub fn classify(path: &Path, cats: &CategorySettings) -> Kind {
    let folders: Vec<String> = path
        .parent()
        .into_iter()
        .flat_map(|p| p.components())
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    for folder in folders.iter().rev() {
        if folder_matches(folder, &cats.jingle_keywords) {
            return Kind::Jingle;
        }
        if folder_matches(folder, &cats.pub_keywords) {
            return Kind::Pub;
        }
    }
    Kind::Morceau
}

// [PL-06]
pub fn make_entry(path: &Path, cats: &CategorySettings) -> Entry {
    let kind = classify(path, cats);
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
            kind,
        },
        Err(e) => Entry {
            id: 0,
            path: path.to_path_buf(),
            title: path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            error: Some(e.to_string()),
            kind,
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
    fn classifies_by_folder_keyword() {
        let cats = CategorySettings::default();
        let k = |p: &str| classify(Path::new(p), &cats);
        assert_eq!(k("/radio/Jingles/station-id.mp3"), Kind::Jingle);
        assert_eq!(k("/radio/JINGLE BLAST/id.mp3"), Kind::Jingle);
        assert_eq!(k("/radio/Publicités/garage.mp3"), Kind::Pub);
        assert_eq!(k("/radio/pub/garage.mp3"), Kind::Pub);
        assert_eq!(k("/radio/Public Enemy/fight.mp3"), Kind::Morceau);
        assert_eq!(k("/radio/Metal/jingle.mp3"), Kind::Morceau);
        let custom = CategorySettings {
            jingle_keywords: vec!["Habillage".into()],
            pub_keywords: vec![],
        };
        assert_eq!(
            classify(Path::new("/r/habillage/a.mp3"), &custom),
            Kind::Jingle
        );
        assert_eq!(classify(Path::new("/r/pub/a.mp3"), &custom), Kind::Morceau);
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
