use std::path::PathBuf;
use std::thread;

use super::commands::Deck;
use super::views::{PlaylistView, EV_PLAYLIST};
use super::{now_secs, Controller, Work};
use crate::journal;
use crate::playlist::{self, Entry};

impl Controller {
    // [CTL-105]
    pub(super) fn scan(&self, paths: Vec<PathBuf>, index: Option<usize>, to_deck: Option<Deck>) {
        let tx = self.work_tx.clone();
        let cats = self.categories.clone();
        thread::spawn(move || {
            let files = playlist::expand(&paths);
            let entries = files
                .iter()
                .map(|p| playlist::make_entry(p, &cats))
                .collect();
            let _ = tx.send(Work::Scanned {
                entries,
                index,
                to_deck,
            });
        });
    }

    // [CTL-106]
    pub(super) fn on_scanned(
        &mut self,
        mut entries: Vec<Entry>,
        index: Option<usize>,
        to_deck: Option<Deck>,
    ) {
        if entries.is_empty() {
            self.notice("warn", "Aucun fichier audio trouvé.");
            return;
        }
        let errors = entries.iter().filter(|e| e.error.is_some()).count();
        if let Some(lib) = self.auto.history.as_mut() {
            let metas: Vec<_> = entries
                .iter()
                .filter(|e| e.error.is_none())
                .map(|e| e.to_meta())
                .collect();
            if let Err(e) = lib.upsert_tracks(&metas, now_secs()) {
                journal::warn(format!("bibliothèque : {e}"));
            }
        }
        if let Some(deck) = to_deck {
            if self.is_playing(deck.index()) {
                self.notice(
                    "warn",
                    format!(
                        "La platine {} est à l'antenne : titres ajoutés à la playlist.",
                        deck.label()
                    ),
                );
            } else if let Some(pos) = entries.iter().position(|e| e.error.is_none()) {
                let first = entries.remove(pos);
                self.return_cued(deck);
                self.request_load(deck, first, false);
            }
        }
        if !entries.is_empty() {
            self.playlist.insert(entries, index);
        }
        if errors > 0 {
            self.notice(
                "warn",
                format!("{errors} fichier(s) illisible(s), signalé(s) dans la playlist."),
            );
        }
        self.emit_playlist();
        self.fill_automix();
        self.emit_state();
    }

    // [CTL-107]
    // [CTL-151]
    pub(super) fn shuffle(&mut self, spread: bool) {
        let rot = &mut self.rot;
        let mut roll = || rot.roll();
        if spread {
            self.playlist.shuffle_spread(&mut roll);
        } else {
            self.playlist.shuffle(&mut roll);
        }
        self.emit_playlist();
    }

    // [CTL-152]
    pub(super) fn dedupe(&mut self) {
        let n = self.playlist.dedupe();
        let msg = match n {
            0 => "Aucun doublon dans la playlist.".to_string(),
            1 => "1 doublon retiré de la playlist.".to_string(),
            n => format!("{n} doublons retirés de la playlist."),
        };
        self.notice("info", msg);
        self.emit_playlist();
    }

    pub(super) fn emit_playlist(&self) {
        let now = now_secs();
        let entries: Vec<Entry> = self
            .playlist
            .entries()
            .iter()
            .map(|e| Entry {
                blocked: self.violation_for(e, now),
                ..e.clone()
            })
            .collect();
        self.emit(
            EV_PLAYLIST,
            &PlaylistView {
                entries: &entries,
                total: self.playlist.total_secs(),
            },
        );
    }
}
