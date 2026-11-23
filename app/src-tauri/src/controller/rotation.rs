use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::thread;

use blast_library::{
    default_clock, path_key, pick_weighted, scheduled, Kind, LibraryTrack, PlaylistSets, Rotation,
    RotationCategory, Scheduled, TrackMeta,
};
use chrono::{Datelike, Timelike};

use super::views::{ClockView, LibraryView, EV_LIBRARY};
use super::{now_ms, now_secs, Controller, Work};
use crate::config::{CategorySettings, LibrarySettings};
use crate::journal;
use crate::playlist::{self, Entry, SlotTag};

// [CTL-135]
const LOOKAHEAD: usize = 3;
// [CTL-136]
const SCAN_BATCH: usize = 200;
// [CTL-137]
const PICK_ATTEMPTS: usize = 8;

// [CTL-138]
pub struct RotationState {
    pub rotation: Rotation,
    pub library_dirs: Vec<PathBuf>,
    pub tracks: Vec<LibraryTrack>,
    pub index: HashMap<String, usize>,
    pub scanning: bool,
    pub scanned: usize,
    pub clock_id: Option<String>,
    pub cursor: usize,
    pub fb_cursor: usize,
    pub rng: u64,
    pub warned: HashSet<String>,
    pub lists: PlaylistSets,
    pub program: Vec<PathBuf>,
    pub program_done: bool,
}

impl RotationState {
    pub fn new(rotation: Rotation, library: &LibrarySettings) -> Self {
        Self {
            rotation,
            library_dirs: library.dirs.clone(),
            tracks: Vec::new(),
            index: HashMap::new(),
            scanning: false,
            scanned: 0,
            clock_id: None,
            cursor: 0,
            fb_cursor: 0,
            rng: now_ms() | 1,
            warned: HashSet::new(),
            lists: PlaylistSets::new(),
            program: Vec::new(),
            program_done: false,
        }
    }

    pub fn roll(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    fn set_tracks(&mut self, tracks: Vec<LibraryTrack>) {
        self.index = tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.path.clone(), i))
            .collect();
        self.tracks = tracks;
    }

    // [CTL-164]
    pub fn refresh_lists(&mut self) {
        self.lists = self
            .rotation
            .categories
            .iter()
            .flat_map(|c| c.rule.playlists.iter())
            .map(|p| {
                let set = playlist::read_m3u(Path::new(p))
                    .iter()
                    .map(|t| path_key(&t.to_string_lossy()))
                    .collect();
                (path_key(p), set)
            })
            .collect();
    }

    fn scan_targets(&self) -> Vec<PathBuf> {
        self.library_dirs
            .iter()
            .cloned()
            .chain(
                self.rotation
                    .playlist_paths()
                    .into_iter()
                    .map(PathBuf::from),
            )
            .collect()
    }

    // [CTL-139]
    pub fn mark_played(&mut self, path: &str, at: i64) {
        if let Some(&i) = self.index.get(path) {
            self.tracks[i].last_played = Some(at);
            self.tracks[i].play_count += 1;
        }
    }
}

// [CTL-140]
fn scan_library(
    dirs: Vec<PathBuf>,
    cats: CategorySettings,
    known: HashMap<String, i64>,
    tx: crossbeam_channel::Sender<Work>,
) {
    let files = playlist::expand(&dirs);
    let mut batch: Vec<TrackMeta> = Vec::with_capacity(SCAN_BATCH);
    for f in &files {
        let mtime = playlist::file_mtime(f);
        if mtime.is_some() && known.get(&*f.to_string_lossy()) == mtime.as_ref() {
            continue;
        }
        let entry = playlist::make_entry(f, &cats);
        if entry.error.is_none() {
            batch.push(entry.to_meta());
        }
        if batch.len() >= SCAN_BATCH
            && tx
                .send(Work::LibraryBatch(std::mem::take(&mut batch)))
                .is_err()
        {
            return;
        }
    }
    let _ = tx.send(Work::LibraryBatch(batch));
    let _ = tx.send(Work::LibraryDone { files: files.len() });
}

impl Controller {
    // [CTL-141]
    pub(super) fn load_library_tracks(&mut self) {
        let Some(history) = self.auto.history.as_ref() else {
            return;
        };
        match history.tracks() {
            Ok(t) => self.rot.set_tracks(t),
            Err(e) => journal::warn(format!("bibliothèque (lecture) : {e}")),
        }
    }

    // [CTL-142]
    pub(super) fn library_rescan(&mut self) {
        if self.rot.scanning {
            self.notice("info", "Scan de la bibliothèque déjà en cours…");
            return;
        }
        let targets = self.rot.scan_targets();
        if targets.is_empty() {
            self.emit_library();
            return;
        }
        let known = self
            .auto
            .history
            .as_ref()
            .and_then(|h| h.track_mtimes().ok())
            .unwrap_or_default();
        self.rot.scanning = true;
        self.rot.scanned = 0;
        let cats = self.categories.clone();
        let tx = self.work_tx.clone();
        thread::spawn(move || scan_library(targets, cats, known, tx));
        self.emit_library();
    }

    pub(super) fn on_library_batch(&mut self, batch: Vec<TrackMeta>) {
        self.rot.scanned += batch.len();
        if let Some(h) = self.auto.history.as_mut() {
            if let Err(e) = h.upsert_tracks(&batch, now_secs()) {
                journal::warn(format!("bibliothèque (enregistrement) : {e}"));
            }
        }
    }

    pub(super) fn on_library_done(&mut self, files: usize) {
        self.rot.scanning = false;
        self.load_library_tracks();
        self.rot.refresh_lists();
        journal::info(format!(
            "bibliothèque : {files} fichiers, {} nouveaux ou modifiés",
            self.rot.scanned
        ));
        self.emit_library();
        self.fill_automix();
    }

    // [CTL-143]
    pub(super) fn apply_rotation(&mut self, rotation: Rotation, library: &LibrarySettings) {
        let mut rescan = false;
        if rotation != self.rot.rotation {
            rescan = rotation.playlist_paths() != self.rot.rotation.playlist_paths();
            self.rot.rotation = rotation;
            self.rot.clock_id = None;
            self.rot.warned.clear();
            self.rot.refresh_lists();
            self.emit_state();
        }
        if library.dirs != self.rot.library_dirs {
            self.rot.library_dirs = library.dirs.clone();
            rescan = true;
        }
        if rescan {
            self.library_rescan();
        }
    }

    // [CTL-144]
    pub(super) fn current_clock(&self) -> Option<ClockView> {
        let now = chrono::Local::now();
        let wd = now.weekday().num_days_from_monday() as usize;
        scheduled(&self.rot.rotation, wd, now.hour() as usize).map(|s| match s {
            Scheduled::Clock(c) => ClockView {
                id: c.id.clone(),
                name: c.name.clone(),
                color: c.color.clone(),
                program: false,
            },
            Scheduled::Program(p) => ClockView {
                id: p.id.clone(),
                name: p.name.clone(),
                color: p.color.clone(),
                program: true,
            },
        })
    }

    // [CTL-145]
    pub(super) fn rotation_fill(&mut self) -> bool {
        let Some(clock) = self.current_clock() else {
            self.rot.clock_id = None;
            return false;
        };
        if self.rot.clock_id.as_deref() != Some(clock.id.as_str()) {
            self.rot.clock_id = Some(clock.id.clone());
            self.rot.cursor = 0;
            self.rot.fb_cursor = 0;
            self.rot.program_done = false;
            self.rot.program = Vec::new();
            if clock.program {
                self.start_program(&clock.id);
            }
        }
        let mut added = Vec::new();
        while self.playlist.entries().len() + added.len() < LOOKAHEAD {
            let next = if clock.program {
                let tag = SlotTag {
                    name: clock.name.clone(),
                    color: clock.color.clone(),
                    fixed: true,
                };
                match self.program_next(tag) {
                    Some(e) => Some(e),
                    None => self.fallback_next(&clock.name, &added),
                }
            } else {
                self.rotation_next(&clock.id, &added, false)
            };
            match next {
                Some(e) => added.push(e),
                None => break,
            }
        }
        if added.is_empty() {
            return false;
        }
        self.playlist.insert(added, None);
        true
    }

    // [CTL-165]
    fn start_program(&mut self, id: &str) {
        let Some(p) = self.rot.rotation.program(id).cloned() else {
            return;
        };
        self.rot.program = playlist::read_m3u(Path::new(&p.path));
        if self.rot.program.is_empty() {
            self.notice(
                "warn",
                format!(
                    "Émission « {} » : playlist {} vide ou introuvable.",
                    p.name, p.path
                ),
            );
        } else {
            journal::info(format!(
                "émission « {} » : {} titre(s) programmé(s)",
                p.name,
                self.rot.program.len()
            ));
        }
    }

    // [CTL-166]
    fn program_next(&mut self, tag: SlotTag) -> Option<Entry> {
        while self.rot.cursor < self.rot.program.len() {
            let path = self.rot.program[self.rot.cursor].clone();
            self.rot.cursor += 1;
            if !path.is_file() {
                continue;
            }
            let known = self.rot.index.get(&*path.to_string_lossy()).copied();
            let entry = match known {
                Some(i) => playlist::from_library(&self.rot.tracks[i], tag),
                None => Entry {
                    slot: Some(tag),
                    ..playlist::make_entry(&path, &self.categories)
                },
            };
            return Some(entry);
        }
        None
    }

    // [CTL-167]
    fn fallback_next(&mut self, program: &str, pending: &[Entry]) -> Option<Entry> {
        if !self.rot.program_done {
            self.rot.program_done = true;
            if !self.rot.program.is_empty() {
                self.notice(
                    "info",
                    format!("Émission « {program} » terminée : retour à la grille par défaut."),
                );
            }
        }
        let id = default_clock(&self.rot.rotation)?.id.clone();
        self.rotation_next(&id, pending, true)
    }

    fn rotation_next(
        &mut self,
        clock_id: &str,
        pending: &[Entry],
        fallback: bool,
    ) -> Option<Entry> {
        let clock = self.rot.rotation.clock(clock_id)?.clone();
        for _ in 0..clock.slots.len() {
            let cursor = if fallback {
                &mut self.rot.fb_cursor
            } else {
                &mut self.rot.cursor
            };
            let slot = clock.slots[*cursor % clock.slots.len()].clone();
            *cursor += 1;
            let Some(cat) = self.rot.rotation.category(&slot).cloned() else {
                continue;
            };
            if let Some(e) = self.pick_from(&cat, pending) {
                return Some(e);
            }
            if self.rot.warned.insert(format!("vide:{}", cat.id)) {
                self.notice(
                    "warn",
                    format!(
                        "Grille « {} » : aucun titre disponible dans la catégorie « {} », case passée.",
                        clock.name, cat.name
                    ),
                );
            }
        }
        None
    }

    // [CTL-146]
    fn pick_from(&mut self, cat: &RotationCategory, pending: &[Entry]) -> Option<Entry> {
        let busy: HashSet<PathBuf> = self
            .playlist
            .entries()
            .iter()
            .chain(pending)
            .map(|e| e.path.clone())
            .chain(self.decks.iter().filter_map(|d| d.meta.path.clone()))
            .collect();
        let queued_artists: HashSet<String> = self
            .playlist
            .entries()
            .iter()
            .chain(pending)
            .filter(|e| e.kind == Kind::Morceau && !e.artist.trim().is_empty())
            .map(|e| e.artist.trim().to_lowercase())
            .collect();
        let now = now_secs();
        let separation = self.auto.settings.rules.enabled;
        let artist_rule = self.auto.settings.rules.artist_minutes > 0;
        let tracks = std::mem::take(&mut self.rot.tracks);
        let mut matching: Vec<&LibraryTrack> = tracks
            .iter()
            .filter(|t| {
                cat.rule.matches_with(t, &self.rot.lists) && !busy.contains(Path::new(&t.path))
            })
            .collect();
        let allowed: Vec<&LibraryTrack> = matching
            .iter()
            .copied()
            .filter(|t| {
                t.kind != Kind::Morceau
                    || !separation
                    || (!(artist_rule && queued_artists.contains(&t.artist.trim().to_lowercase()))
                        && self
                            .violation_of(&t.path, &t.title, &t.artist, t.kind, now)
                            .is_none())
            })
            .collect();
        let relaxed = allowed.is_empty() && !matching.is_empty();
        let mut pool = if relaxed { matching.clone() } else { allowed };
        let mut picked = None;
        for _ in 0..PICK_ATTEMPTS {
            let roll = self.rot.roll();
            let Some(t) = pick_weighted(&pool, now, roll) else {
                break;
            };
            if Path::new(&t.path).is_file() {
                picked = Some(playlist::from_library(
                    t,
                    SlotTag {
                        name: cat.name.clone(),
                        color: cat.color.clone(),
                        fixed: false,
                    },
                ));
                break;
            }
            pool.retain(|c| c.path != t.path);
            matching.retain(|c| c.path != t.path);
        }
        self.rot.tracks = tracks;
        if picked.is_some() && relaxed && self.rot.warned.insert(format!("regles:{}", cat.id)) {
            self.notice(
                "warn",
                format!(
                    "Catégorie « {} » : aucun titre ne respecte les règles de séparation, BLAST les assouplit pour cette catégorie.",
                    cat.name
                ),
            );
        }
        picked
    }

    // [CTL-147]
    pub(super) fn emit_library(&self) {
        self.emit(
            EV_LIBRARY,
            &LibraryView {
                dirs: self.rot.library_dirs.clone(),
                tracks: self.rot.tracks.len(),
                scanning: self.rot.scanning,
                scanned: self.rot.scanned,
            },
        );
    }
}
