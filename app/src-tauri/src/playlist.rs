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
    pub bpm: Option<f32>,
    pub slot: Option<SlotTag>,
}

// [PL-10]
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SlotTag {
    pub name: String,
    pub color: String,
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
            bpm: self.bpm,
            mtime: file_mtime(&self.path),
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

    // [PL-13]
    pub fn shuffle(&mut self, roll: &mut impl FnMut() -> f64) {
        let n = self.entries.len();
        for i in (1..n).rev() {
            let j = ((roll() * (i + 1) as f64) as usize).min(i);
            self.entries.swap(i, j);
        }
    }

    // [PL-14]
    pub fn shuffle_spread(&mut self, roll: &mut impl FnMut() -> f64) {
        self.shuffle(roll);
        let mut groups: Vec<(String, Vec<Entry>)> = Vec::new();
        for (i, e) in std::mem::take(&mut self.entries).into_iter().enumerate() {
            let artist = e.artist.trim().to_lowercase();
            let key = if artist.is_empty() {
                format!("\u{0}{i}")
            } else {
                artist
            };
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, list)) => list.push(e),
                None => groups.push((key, vec![e])),
            }
        }
        let mut last: Option<String> = None;
        while !groups.is_empty() {
            let pick = groups
                .iter()
                .enumerate()
                .filter(|(_, (k, _))| groups.len() == 1 || Some(k) != last.as_ref())
                .max_by(|a, b| a.1 .1.len().cmp(&b.1 .1.len()).then_with(|| b.0.cmp(&a.0)))
                .map(|(i, _)| i)
                .unwrap_or(0);
            let (key, list) = &mut groups[pick];
            if let Some(e) = list.pop() {
                self.entries.push(e);
            }
            last = Some(key.clone());
            if list.is_empty() {
                groups.remove(pick);
            }
        }
    }

    // [PL-15]
    pub fn dedupe(&mut self) -> usize {
        let before = self.entries.len();
        let mut seen_paths = std::collections::HashSet::new();
        let mut seen_tracks = std::collections::HashSet::new();
        self.entries.retain(|e| {
            let title = e.title.trim().to_lowercase();
            let track = (!title.is_empty()).then(|| (title, e.artist.trim().to_lowercase()));
            let keep = !seen_paths.contains(&e.path)
                && track.as_ref().is_none_or(|t| !seen_tracks.contains(t));
            if keep {
                seen_paths.insert(e.path.clone());
                seen_tracks.extend(track);
            }
            keep
        });
        before - self.entries.len()
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

// [PL-11]
pub fn file_mtime(path: &Path) -> Option<i64> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

// [PL-12]
pub fn from_library(t: &blast_library::LibraryTrack, slot: SlotTag) -> Entry {
    let path = PathBuf::from(&t.path);
    let title = if t.title.trim().is_empty() {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        t.title.clone()
    };
    Entry {
        id: 0,
        path,
        title,
        artist: t.artist.clone(),
        genre: t.genre.clone(),
        year: t.year,
        duration: t.duration,
        kind: t.kind,
        bpm: t.bpm,
        slot: Some(slot),
        ..Default::default()
    }
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
            bpm: info.bpm,
            slot: None,
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

    fn artists(p: &Playlist) -> Vec<String> {
        p.entries().iter().map(|e| e.artist.clone()).collect()
    }

    fn named(title: &str, artist: &str, path: &str) -> Entry {
        Entry {
            title: title.into(),
            artist: artist.into(),
            path: path.into(),
            ..Default::default()
        }
    }

    #[test]
    fn shuffles_and_spreads_artists() {
        let mut seed = 0.37_f64;
        let mut roll = move || {
            seed = (seed * 9301.0 + 0.49297).fract();
            seed
        };
        let mut p = Playlist::default();
        let mut items: Vec<Entry> = (0..6)
            .map(|i| named(&format!("a{i}"), "dgoth", &format!("/a{i}")))
            .collect();
        items.extend((0..5).map(|i| named(&format!("b{i}"), "shakyvibrato242", &format!("/b{i}"))));
        p.insert(items, None);
        p.shuffle(&mut roll);
        assert_eq!(p.entries().len(), 11);
        p.shuffle_spread(&mut roll);
        let a = artists(&p);
        assert_eq!(a.len(), 11);
        assert!(a.windows(2).all(|w| w[0] != w[1]), "{a:?}");
        let mut ids: Vec<u64> = p.entries().iter().map(|e| e.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 11);
    }

    #[test]
    fn removes_duplicates() {
        let mut p = Playlist::default();
        p.insert(
            vec![
                named("The Unchosen", "dgoth", "/1.mp3"),
                named("the unchosen ", "DGOTH", "/2.mp3"),
                named("Autre", "dgoth", "/1.mp3"),
                named("Autre", "dgoth", "/3.mp3"),
                named("", "", "/4.mp3"),
                named("", "", "/5.mp3"),
            ],
            None,
        );
        assert_eq!(p.dedupe(), 2);
        let paths: Vec<_> = p
            .entries()
            .iter()
            .map(|e| e.path.to_string_lossy().into_owned())
            .collect();
        assert_eq!(paths, ["/1.mp3", "/3.mp3", "/4.mp3", "/5.mp3"]);
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
