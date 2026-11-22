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
        thread::spawn(move || {
            let files = playlist::expand(&paths);
            let entries = files.iter().map(|p| playlist::make_entry(p)).collect();
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
        if let Some(lib) = self.auto.library.as_mut() {
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
