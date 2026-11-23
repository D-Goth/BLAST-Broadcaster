use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use blast_core::{decode_file, Command, DeckState, DeckStatus, Track};
use blast_library::Kind;

use super::commands::Deck;
use super::views::{DeckView, MicView, StateView, EV_STATE};
use super::{Controller, Work};
use crate::playlist::{self, Entry};

// [CTL-40]
pub const EMPTY_STATUS: DeckStatus = DeckStatus {
    state: DeckState::Empty,
    position: 0,
    frames: 0,
};

// [CTL-41]
#[derive(Debug, Default, Clone)]
pub struct DeckMeta {
    pub title: String,
    pub artist: String,
    pub path: Option<PathBuf>,
    pub loading: Option<u64>,
    pub finished: bool,
    pub kind: Kind,
}

// [CTL-42]
#[derive(Debug, Clone)]
pub struct DeckSide {
    pub meta: DeckMeta,
    pub status: DeckStatus,
}

impl Default for DeckSide {
    fn default() -> Self {
        Self {
            meta: DeckMeta::default(),
            status: EMPTY_STATUS,
        }
    }
}

// [CTL-43]
impl DeckSide {
    pub fn is_playing(&self) -> bool {
        self.status.state == DeckState::Playing && !self.meta.finished
    }

    pub fn is_cued(&self) -> bool {
        self.meta.path.is_some() && !self.meta.finished
    }

    pub fn is_active(&self) -> bool {
        self.meta.loading.is_some() || self.is_playing() || self.is_cued()
    }

    pub fn is_ready(&self) -> bool {
        self.meta.path.is_some() && self.meta.loading.is_none()
    }

    pub fn state_label(&self) -> &'static str {
        if self.meta.loading.is_some() {
            return "loading";
        }
        match self.status.state {
            DeckState::Empty => "empty",
            DeckState::Loaded => "loaded",
            DeckState::Playing => "playing",
            DeckState::Paused => "paused",
            DeckState::Finished => "finished",
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

impl Controller {
    pub(super) fn is_playing(&self, i: usize) -> bool {
        self.decks[i].is_playing()
    }

    // [CTL-44]
    pub(super) fn request_load(&mut self, deck: Deck, entry: Entry, play: bool) {
        self.next_job += 1;
        let job = self.next_job;
        self.decks[deck.index()].meta = DeckMeta {
            title: entry.title.clone(),
            artist: entry.artist.clone(),
            path: Some(entry.path.clone()),
            loading: Some(job),
            finished: false,
            kind: entry.kind,
        };
        let tx = self.work_tx.clone();
        let rate = self.sample_rate;
        thread::spawn(move || {
            let result = decode_file(&entry.path, rate).map_err(|e| e.to_string());
            let _ = tx.send(Work::Decoded {
                deck,
                job,
                result,
                play,
            });
        });
    }

    // [CTL-45]
    pub(super) fn on_decoded(
        &mut self,
        deck: Deck,
        job: u64,
        result: Result<Track, String>,
        play: bool,
    ) {
        let i = deck.index();
        if self.decks[i].meta.loading != Some(job) {
            return;
        }
        self.decks[i].meta.loading = None;
        match result {
            Ok(track) => {
                let side = &mut self.decks[i];
                if let Some(t) = &track.title {
                    side.meta.title = t.clone();
                }
                side.meta.artist = track.artist.clone().unwrap_or_default();
                side.meta.finished = false;
                side.status = DeckStatus {
                    state: DeckState::Loaded,
                    position: 0,
                    frames: track.frames(),
                };
                self.send(Command::Load(deck.id(), Arc::new(track)));
                if play {
                    self.crossfader = deck.id().crossfader_side();
                    self.send(Command::SetCrossfader(self.crossfader));
                    self.send(Command::Play(deck.id()));
                }
                if self.pending_fade == Some(deck) {
                    self.pending_fade = None;
                    self.crossfade_to(deck);
                }
            }
            Err(e) => {
                if self.pending_fade == Some(deck) {
                    self.pending_fade = None;
                }
                self.notice(
                    "error",
                    format!(
                        "Impossible de charger « {} » : {e}",
                        self.decks[i].meta.title
                    ),
                );
                self.decks[i].reset();
            }
        }
        self.fill_automix();
        self.emit_state();
    }

    // [CTL-46]
    pub(super) fn on_ended(&mut self, i: usize) {
        let side = &mut self.decks[i];
        side.meta.finished = true;
        side.status.state = DeckState::Finished;
        side.status.position = side.status.frames;
    }

    // [CTL-47]
    fn rewind_if_finished(&mut self, deck: Deck) {
        let side = &mut self.decks[deck.index()];
        if side.meta.finished {
            side.meta.finished = false;
            self.send(Command::Stop(deck.id()));
        }
    }

    // [CTL-48]
    pub(super) fn crossfade_to(&mut self, to: Deck) {
        if !self.decks[to.index()].is_ready() {
            return;
        }
        self.rewind_if_finished(to);
        self.send(Command::Play(to.id()));
        self.send(Command::StartCrossfade {
            to: to.id(),
            frames: (self.fade.max(0.1) * self.sample_rate as f32) as usize,
        });
    }

    // [CTL-49]
    pub(super) fn fade_or_fetch(&mut self, to: Deck) {
        let side = &self.decks[to.index()];
        if side.meta.loading.is_some() {
            self.pending_fade = Some(to);
        } else if side.is_playing() || side.is_cued() {
            self.crossfade_to(to);
        } else if let Some(e) = self.take_next_auto() {
            self.request_load(to, e, false);
            self.pending_fade = Some(to);
            self.emit_playlist();
        } else {
            self.notice("warn", "Plus aucun titre dans la playlist.");
        }
    }

    // [CTL-50]
    pub(super) fn next(&mut self, deck: Deck) {
        let i = deck.index();
        if self.is_playing(i) {
            self.fade_or_fetch(Deck::from_index(1 - i));
        } else if self.decks[i].meta.loading.is_none() {
            match self.take_next_auto() {
                Some(e) => {
                    self.request_load(deck, e, false);
                    self.emit_playlist();
                }
                None => self.notice("warn", "Plus aucun titre dans la playlist."),
            }
        }
    }

    // [CTL-51]
    pub(super) fn return_cued(&mut self, deck: Deck) {
        let side = &self.decks[deck.index()];
        if side.meta.loading.is_some() || side.meta.finished || side.status.position != 0 {
            return;
        }
        if let Some(path) = side.meta.path.clone() {
            self.playlist
                .insert(vec![playlist::make_entry(&path, &self.categories)], Some(0));
        }
    }

    // [CTL-52]
    pub(super) fn fill_automix(&mut self) {
        if !self.automix {
            return;
        }
        let mut changed = self.rotation_fill();
        if !self.decks[0].is_active() && !self.decks[1].is_active() {
            if let Some(e) = self.take_next_auto() {
                let start = Deck::from_index(self.on_air_side());
                self.request_load(start, e, true);
                changed = true;
            }
        }
        for i in 0..2 {
            if !self.decks[i].is_active() && self.decks[1 - i].is_active() {
                if let Some(e) = self.take_next_auto() {
                    self.request_load(Deck::from_index(i), e, false);
                    changed = true;
                }
            }
        }
        if changed {
            self.emit_playlist();
            self.emit_state();
        }
    }

    // [CTL-53]
    pub(super) fn load_entry(&mut self, id: u64, deck: Deck) {
        if self.is_playing(deck.index()) {
            self.notice(
                "warn",
                format!(
                    "La platine {} est à l'antenne : arrête-la avant d'y charger un titre.",
                    deck.label()
                ),
            );
        } else if let Some(bad) = self
            .playlist
            .entries()
            .iter()
            .find(|e| e.id == id && e.error.is_some())
        {
            self.notice("error", format!("Fichier illisible : {}", bad.title));
        } else if let Some(e) = self.playlist.remove(id) {
            self.return_cued(deck);
            self.request_load(deck, e, false);
            self.emit_playlist();
        }
    }

    // [CTL-54]
    pub(super) fn play(&mut self, deck: Deck) {
        if self.decks[deck.index()].is_ready() {
            self.rewind_if_finished(deck);
            self.send(Command::Play(deck.id()));
        }
    }

    // [CTL-55]
    pub(super) fn stop(&mut self, deck: Deck) {
        self.send(Command::Stop(deck.id()));
        self.decks[deck.index()].meta.finished = false;
    }

    // [CTL-56]
    pub(super) fn eject(&mut self, deck: Deck) {
        if !self.is_playing(deck.index()) {
            self.send(Command::Eject(deck.id()));
            self.decks[deck.index()].reset();
            self.fill_automix();
        }
    }

    // [CTL-57]
    pub(super) fn seek(&mut self, deck: Deck, fraction: f32) {
        let side = &mut self.decks[deck.index()];
        let frames = side.status.frames;
        if frames == 0 {
            return;
        }
        let target = (fraction.clamp(0.0, 1.0) as f64 * frames as f64) as usize;
        side.meta.finished = false;
        self.send(Command::Seek(
            deck.id(),
            target.min(frames.saturating_sub(1)),
        ));
    }

    // [CTL-58]
    pub(super) fn send_automix(&self) {
        self.send(Command::SetAutoMix {
            enabled: self.automix,
            fade_frames: (self.fade * self.sample_rate as f32) as usize,
        });
    }

    // [CTL-59]
    fn deck_view(&self, i: usize) -> DeckView {
        let side = &self.decks[i];
        let rate = self.sample_rate as f64;
        DeckView {
            state: side.state_label(),
            position: side.status.position as f64 / rate,
            duration: side.status.frames as f64 / rate,
            title: side.meta.title.clone(),
            artist: side.meta.artist.clone(),
            path: side.meta.path.clone(),
        }
    }

    // [CTL-60]
    pub(super) fn emit_state(&self) {
        let side = self.on_air_side();
        let view = StateView {
            decks: [self.deck_view(0), self.deck_view(1)],
            crossfader: self.crossfader,
            peak: self.peak,
            deck_peak: self.deck_peak,
            deck_gain_db: self.deck_gain_db,
            automix: self.automix,
            fade: self.fade,
            on_air: self.is_playing(side).then(|| Deck::from_index(side)),
            device: self.device.clone(),
            sample_rate: self.sample_rate,
            audio_error: self.audio_error.clone(),
            mic: MicView {
                available: self.mic.available,
                open: self.mic.open,
                peak: self.mic.peak,
                name: self.mic.name.clone(),
                error: self.mic.error.clone(),
            },
            sfx: self.pads.mask,
            duck: self.mic.duck,
            reduction: self.reduction,
            clock: self.current_clock(),
        };
        self.emit(EV_STATE, &view);
    }
}
