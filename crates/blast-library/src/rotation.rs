use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{Kind, LibraryTrack};

// [LIB-44]
pub const SCHEDULE_SLOTS: usize = 7 * 24;
// [LIB-45]
const STALE_HOURS: f64 = 168.0;

// [LIB-46]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CategoryRule {
    pub kinds: Vec<Kind>,
    pub genres: Vec<String>,
    pub year_min: Option<i32>,
    pub year_max: Option<i32>,
    pub bpm_min: Option<f32>,
    pub bpm_max: Option<f32>,
    pub folders: Vec<String>,
    pub playlists: Vec<String>,
}

// [LIB-56]
pub type PlaylistSets = HashMap<String, HashSet<String>>;

// [LIB-57]
pub fn path_key(p: &str) -> String {
    norm_path(p)
}

fn norm_path(p: &str) -> String {
    p.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

fn in_range<T: PartialOrd + Copy>(v: Option<T>, min: Option<T>, max: Option<T>) -> bool {
    if min.is_none() && max.is_none() {
        return true;
    }
    match v {
        Some(v) => min.is_none_or(|m| v >= m) && max.is_none_or(|m| v <= m),
        None => false,
    }
}

impl CategoryRule {
    // [LIB-47]
    pub fn matches(&self, t: &LibraryTrack) -> bool {
        if !self.kinds.is_empty() && !self.kinds.contains(&t.kind) {
            return false;
        }
        if !self.genres.is_empty() {
            let genre = t.genre.to_lowercase();
            let hit = self
                .genres
                .iter()
                .map(|g| g.trim().to_lowercase())
                .any(|g| !g.is_empty() && genre.contains(&g));
            if !hit {
                return false;
            }
        }
        let year = |v: Option<i32>| v.filter(|y| *y > 0);
        let bpm = |v: Option<f32>| v.filter(|b| *b > 0.0);
        if !in_range(t.year, year(self.year_min), year(self.year_max))
            || !in_range(t.bpm, bpm(self.bpm_min), bpm(self.bpm_max))
        {
            return false;
        }
        if !self.folders.is_empty() {
            let path = norm_path(&t.path);
            let hit = self
                .folders
                .iter()
                .map(|f| norm_path(f.trim()))
                .any(|f| !f.is_empty() && path.starts_with(&format!("{f}/")));
            if !hit {
                return false;
            }
        }
        true
    }

    // [LIB-58]
    pub fn matches_with(&self, t: &LibraryTrack, lists: &PlaylistSets) -> bool {
        if !self.matches(t) {
            return false;
        }
        if self.playlists.is_empty() {
            return true;
        }
        let key = norm_path(&t.path);
        self.playlists
            .iter()
            .filter_map(|p| lists.get(&norm_path(p)))
            .any(|set| set.contains(&key))
    }
}

// [LIB-48]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RotationCategory {
    pub id: String,
    pub name: String,
    pub color: String,
    pub rule: CategoryRule,
}

// [LIB-49]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Clock {
    pub id: String,
    pub name: String,
    pub color: String,
    pub slots: Vec<String>,
}

// [LIB-59]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Program {
    pub id: String,
    pub name: String,
    pub color: String,
    pub path: String,
}

// [LIB-50]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rotation {
    pub categories: Vec<RotationCategory>,
    pub clocks: Vec<Clock>,
    pub programs: Vec<Program>,
    pub schedule: Vec<String>,
    pub default_clock: Option<String>,
}

// [LIB-60]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scheduled<'a> {
    Clock(&'a Clock),
    Program(&'a Program),
}

impl Scheduled<'_> {
    pub fn id(&self) -> &str {
        match self {
            Scheduled::Clock(c) => &c.id,
            Scheduled::Program(p) => &p.id,
        }
    }
}

impl Rotation {
    pub fn category(&self, id: &str) -> Option<&RotationCategory> {
        self.categories.iter().find(|c| c.id == id)
    }

    pub fn clock(&self, id: &str) -> Option<&Clock> {
        self.clocks.iter().find(|c| c.id == id)
    }

    pub fn program(&self, id: &str) -> Option<&Program> {
        self.programs.iter().find(|p| p.id == id)
    }

    // [LIB-61]
    pub fn playlist_paths(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .categories
            .iter()
            .flat_map(|c| c.rule.playlists.iter().cloned())
            .chain(self.programs.iter().map(|p| p.path.clone()))
            .filter(|p| !p.trim().is_empty())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    // [LIB-51]
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for c in &self.categories {
            if c.name.trim().is_empty() {
                return Err("Chaque catégorie doit avoir un nom.".into());
            }
            if c.id.is_empty() || !ids.insert(c.id.as_str()) {
                return Err(format!(
                    "Identifiant de catégorie invalide : « {} ».",
                    c.name
                ));
            }
        }
        for k in &self.clocks {
            if k.name.trim().is_empty() {
                return Err("Chaque grille doit avoir un nom.".into());
            }
            if k.id.is_empty() || !ids.insert(k.id.as_str()) {
                return Err(format!("Identifiant de grille invalide : « {} ».", k.name));
            }
            if let Some(bad) = k.slots.iter().find(|s| self.category(s).is_none()) {
                return Err(format!(
                    "La grille « {} » utilise une catégorie supprimée ({bad}).",
                    k.name
                ));
            }
        }
        for p in &self.programs {
            if p.name.trim().is_empty() {
                return Err("Chaque émission doit avoir un nom.".into());
            }
            if p.id.is_empty() || !ids.insert(p.id.as_str()) {
                return Err(format!("Identifiant d'émission invalide : « {} ».", p.name));
            }
            if p.path.trim().is_empty() {
                return Err(format!("L'émission « {} » n'a pas de fichier M3U.", p.name));
            }
        }
        let known_clock = |id: &str| id.is_empty() || self.clock(id).is_some();
        let known = |id: &str| known_clock(id) || self.program(id).is_some();
        if self.schedule.len() > SCHEDULE_SLOTS || !self.schedule.iter().all(|s| known(s)) {
            return Err("Le planning référence une grille ou une émission inconnue.".into());
        }
        if !known_clock(self.default_clock.as_deref().unwrap_or_default()) {
            return Err("La grille par défaut n'existe plus.".into());
        }
        Ok(())
    }
}

// [LIB-52]
pub fn scheduled(rotation: &Rotation, weekday: usize, hour: usize) -> Option<Scheduled<'_>> {
    let slot = weekday.min(6) * 24 + hour.min(23);
    if let Some(id) = rotation.schedule.get(slot).filter(|id| !id.is_empty()) {
        if let Some(p) = rotation.program(id) {
            return Some(Scheduled::Program(p));
        }
        if let Some(c) = rotation.clock(id).filter(|c| !c.slots.is_empty()) {
            return Some(Scheduled::Clock(c));
        }
    }
    default_clock(rotation).map(Scheduled::Clock)
}

// [LIB-62]
pub fn default_clock(rotation: &Rotation) -> Option<&Clock> {
    rotation
        .default_clock
        .as_deref()
        .and_then(|id| rotation.clock(id))
        .filter(|c| !c.slots.is_empty())
}

// [LIB-53]
pub fn weight(t: &LibraryTrack, now: i64) -> f64 {
    let hours = t
        .last_played
        .map_or(STALE_HOURS, |at| (now - at).max(0) as f64 / 3600.0);
    hours.min(STALE_HOURS) + 1.0
}

// [LIB-54]
pub fn pick_weighted<'a>(
    candidates: &[&'a LibraryTrack],
    now: i64,
    roll: f64,
) -> Option<&'a LibraryTrack> {
    let total: f64 = candidates.iter().map(|t| weight(t, now)).sum();
    if candidates.is_empty() || total <= 0.0 {
        return None;
    }
    let mut target = roll.clamp(0.0, 0.999_999) * total;
    for t in candidates {
        target -= weight(t, now);
        if target < 0.0 {
            return Some(t);
        }
    }
    candidates.last().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(path: &str, genre: &str, year: Option<i32>, bpm: Option<f32>) -> LibraryTrack {
        LibraryTrack {
            path: path.into(),
            genre: genre.into(),
            year,
            bpm,
            ..Default::default()
        }
    }

    #[test]
    fn rules_combine_tags_and_folders() {
        let rule = CategoryRule {
            kinds: vec![Kind::Morceau],
            genres: vec!["rock".into(), "metal".into()],
            year_min: Some(1990),
            year_max: Some(1999),
            ..Default::default()
        };
        assert!(rule.matches(&track("/m/a.mp3", "Hard Rock", Some(1994), None)));
        assert!(rule.matches(&track("/m/b.mp3", "Black Metal", Some(1999), None)));
        assert!(!rule.matches(&track("/m/c.mp3", "Pop", Some(1994), None)));
        assert!(!rule.matches(&track("/m/d.mp3", "Rock", Some(2004), None)));
        assert!(!rule.matches(&track("/m/e.mp3", "Rock", None, None)));
        let folder = CategoryRule {
            folders: vec!["C:\\Musique\\Synthwave\\".into()],
            bpm_min: Some(100.0),
            ..Default::default()
        };
        assert!(folder.matches(&track("c:/musique/synthwave/x.flac", "", None, Some(118.0))));
        assert!(!folder.matches(&track(
            "C:\\Musique\\Synthwave2\\x.flac",
            "",
            None,
            Some(118.0)
        )));
        assert!(!folder.matches(&track(
            "C:\\Musique\\Synthwave\\x.flac",
            "",
            None,
            Some(90.0)
        )));
        assert!(CategoryRule::default().matches(&track("/x.mp3", "", None, None)));
        let zeros = CategoryRule {
            year_min: Some(0),
            year_max: Some(0),
            bpm_min: Some(0.0),
            bpm_max: Some(0.0),
            ..Default::default()
        };
        assert!(zeros.matches(&track("/x.mp3", "", Some(1994), None)));
        let listed = CategoryRule {
            playlists: vec!["C:\\Prog\\Fete.m3u".into()],
            ..Default::default()
        };
        let mut lists = PlaylistSets::new();
        lists.insert(
            path_key("c:/prog/fete.m3u"),
            [path_key("C:\\Musique\\a.mp3")].into_iter().collect(),
        );
        assert!(listed.matches_with(&track("c:/musique/A.mp3", "", None, None), &lists));
        assert!(!listed.matches_with(&track("c:/musique/b.mp3", "", None, None), &lists));
        assert!(CategoryRule::default().matches_with(&track("/b.mp3", "", None, None), &lists));
    }

    fn rotation() -> Rotation {
        let cat = |id: &str| RotationCategory {
            id: id.into(),
            name: id.into(),
            ..Default::default()
        };
        let clock = |id: &str, slots: &[&str]| Clock {
            id: id.into(),
            name: id.into(),
            slots: slots.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        let mut schedule = vec![String::new(); SCHEDULE_SLOTS];
        schedule[4 * 24 + 21] = "soiree".to_string();
        schedule[5 * 24 + 22] = "halloween".to_string();
        Rotation {
            categories: vec![cat("rock"), cat("metal")],
            clocks: vec![
                clock("jour", &["rock"]),
                clock("soiree", &["metal", "rock"]),
            ],
            programs: vec![Program {
                id: "halloween".into(),
                name: "Halloween".into(),
                path: "/m/halloween.m3u".into(),
                ..Default::default()
            }],
            schedule,
            default_clock: Some("jour".into()),
        }
    }

    #[test]
    fn schedule_falls_back_to_the_default_clock() {
        let r = rotation();
        assert_eq!(scheduled(&r, 4, 21).unwrap().id(), "soiree");
        assert_eq!(scheduled(&r, 4, 20).unwrap().id(), "jour");
        assert!(
            matches!(scheduled(&r, 5, 22), Some(Scheduled::Program(p)) if p.name == "Halloween")
        );
        assert_eq!(r.playlist_paths(), vec!["/m/halloween.m3u".to_string()]);
        assert!(r.validate().is_ok());
        let mut nameless = r.clone();
        nameless.programs[0].path.clear();
        assert!(nameless.validate().is_err());
        let mut broken = r.clone();
        broken.clocks[0].slots.push("disparue".into());
        assert!(broken.validate().is_err());
        let mut none = r;
        none.default_clock = None;
        assert!(scheduled(&none, 0, 0).is_none());
    }

    #[test]
    fn weighting_prefers_tracks_not_heard_for_a_while() {
        let now = 1_000_000;
        let mut fresh = track("/fresh.mp3", "", None, None);
        fresh.last_played = Some(now - 600);
        let stale = track("/stale.mp3", "", None, None);
        assert!(weight(&stale, now) > 100.0 * weight(&fresh, now));
        let list = [&fresh, &stale];
        let picks = (0..100)
            .filter(|i| pick_weighted(&list, now, *i as f64 / 100.0).unwrap().path == "/stale.mp3")
            .count();
        assert!(picks > 95);
        assert!(pick_weighted(&[], now, 0.5).is_none());
    }
}
