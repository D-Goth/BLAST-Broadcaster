use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use blast_core::dsp::db_to_gain;
use blast_core::{decode_file, Command, Track, CART_BASE};
use blast_library::{Kind, NewPlay};

use super::views::{JingleView, JinglesView, EV_JINGLES};
use super::{now_secs, Controller, Work};
use crate::config::{CategorySettings, JingleSettings};
use crate::playlist::{self, Entry};

// [CTL-125]
const MAX_JINGLES: usize = 500;

// [CTL-126]
pub struct Jingle {
    pub entry: Entry,
    pub track: Option<Arc<Track>>,
    pub error: Option<String>,
}

// [CTL-127]
pub struct Cart {
    pub hist_id: Option<i64>,
    pub started: Instant,
}

// [CTL-128]
#[derive(Default)]
pub struct JingleState {
    pub dirs: Vec<PathBuf>,
    pub items: Vec<Jingle>,
    pub generation: u64,
    pub scanning: bool,
    pub playing: HashMap<usize, Cart>,
}

fn voice(index: usize) -> u16 {
    CART_BASE + index as u16
}

impl Controller {
    // [CTL-129]
    pub(super) fn sync_jingles(&mut self, settings: &JingleSettings) {
        self.send(Command::SetCartGain(db_to_gain(settings.volume_db)));
        if settings.dirs != self.jingles.dirs {
            self.jingles.dirs = settings.dirs.clone();
            self.rescan_jingles();
        }
    }

    // [CTL-130]
    pub(super) fn rescan_jingles(&mut self) {
        self.jingle_stop_all();
        self.jingles.generation += 1;
        self.jingles.items.clear();
        self.jingles.scanning = !self.jingles.dirs.is_empty();
        self.emit_jingles();
        if self.jingles.dirs.is_empty() {
            return;
        }
        let generation = self.jingles.generation;
        let dirs = self.jingles.dirs.clone();
        let cats = CategorySettings {
            jingle_keywords: Vec::new(),
            pub_keywords: Vec::new(),
        };
        let rate = self.sample_rate;
        let tx = self.work_tx.clone();
        thread::spawn(move || {
            let mut entries: Vec<Entry> = playlist::expand(&dirs)
                .iter()
                .take(MAX_JINGLES)
                .map(|p| playlist::make_entry(p, &cats))
                .collect();
            for e in &mut entries {
                e.kind = Kind::Jingle;
            }
            let paths: Vec<PathBuf> = entries.iter().map(|e| e.path.clone()).collect();
            if tx
                .send(Work::JinglesScanned {
                    generation,
                    entries,
                })
                .is_err()
            {
                return;
            }
            for (index, path) in paths.into_iter().enumerate() {
                let result = decode_file(&path, rate).map_err(|e| e.to_string());
                let work = Work::JingleDecoded {
                    generation,
                    index,
                    result,
                };
                if tx.send(work).is_err() {
                    return;
                }
            }
        });
    }

    pub(super) fn on_jingles_scanned(&mut self, generation: u64, entries: Vec<Entry>) {
        if generation != self.jingles.generation {
            return;
        }
        self.jingles.scanning = false;
        self.jingles.items = entries
            .into_iter()
            .map(|entry| Jingle {
                error: entry.error.clone(),
                entry,
                track: None,
            })
            .collect();
        if self.jingles.items.is_empty() {
            self.notice("warn", "Aucun fichier audio dans le dossier des jingles.");
        }
        self.emit_jingles();
    }

    pub(super) fn on_jingle_decoded(
        &mut self,
        generation: u64,
        index: usize,
        result: Result<Track, String>,
    ) {
        if generation != self.jingles.generation {
            return;
        }
        if let Some(j) = self.jingles.items.get_mut(index) {
            match result {
                Ok(t) => j.track = Some(Arc::new(t)),
                Err(e) => j.error = Some(e),
            }
            self.emit_jingles();
        }
    }

    // [CTL-131]
    pub(super) fn jingle_play(&mut self, index: usize) {
        if self.jingles.playing.contains_key(&index) {
            self.send(Command::StopSfx(voice(index)));
            self.on_cart_ended(index);
            return;
        }
        let Some(j) = self.jingles.items.get(index) else {
            return;
        };
        let track = match (&j.track, &j.error) {
            (Some(t), _) => t.clone(),
            (None, Some(e)) => {
                let msg = format!("{} : {e}", j.entry.title);
                self.notice("error", msg);
                return;
            }
            (None, None) => {
                let msg = format!("{} : chargement en cours…", j.entry.title);
                self.notice("info", msg);
                return;
            }
        };
        let play = NewPlay {
            path: j.entry.path.to_string_lossy().into_owned(),
            title: j.entry.title.clone(),
            artist: j.entry.artist.clone(),
            kind: Kind::Jingle,
            started_at: now_secs(),
            on_air: self.cast.casting,
            listeners: self.cast.total_listeners(),
        };
        self.send(Command::PlaySfx {
            pad: voice(index),
            track,
        });
        let hist_id = self.record_play(play);
        self.jingles.playing.insert(
            index,
            Cart {
                hist_id,
                started: Instant::now(),
            },
        );
        self.emit_jingles();
        self.emit_history();
    }

    // [CTL-132]
    pub(super) fn jingle_stop_all(&mut self) {
        self.send(Command::StopAllCarts);
        let playing: Vec<usize> = self.jingles.playing.keys().copied().collect();
        for index in playing {
            self.on_cart_ended(index);
        }
    }

    // [CTL-133]
    pub(super) fn on_cart_ended(&mut self, index: usize) {
        let Some(cart) = self.jingles.playing.remove(&index) else {
            return;
        };
        if let Some(id) = cart.hist_id {
            self.finish_play(id, cart.started.elapsed().as_secs_f64());
        }
        self.emit_jingles();
        self.emit_history();
    }

    pub(super) fn on_voice_ended(&mut self, voice_id: u16) {
        if voice_id >= CART_BASE {
            self.on_cart_ended((voice_id - CART_BASE) as usize);
        } else {
            self.pads.clear_playing(voice_id);
        }
    }

    // [CTL-134]
    pub(super) fn emit_jingles(&self) {
        let items = self
            .jingles
            .items
            .iter()
            .enumerate()
            .map(|(index, j)| JingleView {
                index,
                name: j.entry.title.clone(),
                path: j.entry.path.clone(),
                duration: j.entry.duration,
                ready: j.track.is_some(),
                error: j.error.clone(),
                playing: self.jingles.playing.contains_key(&index),
            })
            .collect();
        self.emit(
            EV_JINGLES,
            &JinglesView {
                items,
                dirs: self.jingles.dirs.clone(),
                scanning: self.jingles.scanning,
            },
        );
    }
}
