use std::path::PathBuf;
use std::time::{Duration, Instant};

use blast_core::DeckState;
use blast_library::{Library, SeparationRules};

use super::views::{HistoryView, EV_HISTORY};
use super::{now_secs, Controller};
use crate::config::AutomationSettings;
use crate::journal;
use crate::playlist::Entry;

// [CTL-61]
const PURGE_EVERY: Duration = Duration::from_secs(3600);
// [CTL-62]
const RULES_REFRESH_EVERY: Duration = Duration::from_secs(30);
// [CTL-63]
const HISTORY_ROWS: u32 = 300;

// [CTL-64]
pub struct CurrentPlay {
    pub deck: usize,
    pub path: PathBuf,
    pub hist_id: Option<i64>,
    pub started: Instant,
}

// [CTL-65]
pub struct AutoState {
    pub library: Option<Library>,
    pub settings: AutomationSettings,
    pub current: Option<CurrentPlay>,
    pub last_purge: Option<Instant>,
    pub last_rules_refresh: Instant,
}

impl AutoState {
    pub fn new(library: Option<Library>, settings: AutomationSettings) -> Self {
        Self {
            library,
            settings,
            current: None,
            last_purge: None,
            last_rules_refresh: Instant::now(),
        }
    }

    fn rules(&self) -> SeparationRules {
        self.settings.rules
    }
}

// [CTL-66]
pub enum NextPick {
    Clean(u64),
    Forced { id: u64, title: String, why: String },
    Empty,
}

// [CTL-67]
pub fn pick_next<'a>(
    entries: impl IntoIterator<Item = &'a Entry>,
    mut violation: impl FnMut(&Entry) -> Option<String>,
) -> NextPick {
    let mut fallback = None;
    for e in entries.into_iter().filter(|e| e.error.is_none()) {
        match violation(e) {
            None => return NextPick::Clean(e.id),
            Some(why) => {
                fallback.get_or_insert_with(|| (e.id, e.title.clone(), why));
            }
        }
    }
    match fallback {
        Some((id, title, why)) => NextPick::Forced { id, title, why },
        None => NextPick::Empty,
    }
}

impl Controller {
    // [CTL-68]
    pub(super) fn violation_for(&self, e: &Entry, now: i64) -> Option<String> {
        if e.error.is_some() {
            return None;
        }
        let rules = self.auto.rules();
        if !rules.enabled {
            return None;
        }
        let artist = e.artist.trim();
        if rules.artist_minutes > 0 && !artist.is_empty() {
            for (i, side) in self.decks.iter().enumerate() {
                let busy = side.meta.loading.is_some() || side.is_cued();
                if busy && side.meta.artist.trim().eq_ignore_ascii_case(artist) {
                    return Some(format!(
                        "même artiste déjà sur la platine {}",
                        super::Deck::from_index(i).label()
                    ));
                }
            }
        }
        let lib = self.auto.library.as_ref()?;
        match lib.check(&e.path.to_string_lossy(), &e.title, &e.artist, now, &rules) {
            Ok(v) => v.map(|v| v.describe()),
            Err(err) => {
                journal::warn(format!("règles de séparation : {err}"));
                None
            }
        }
    }

    // [CTL-69]
    pub(super) fn take_next_auto(&mut self) -> Option<Entry> {
        let now = now_secs();
        let pick = pick_next(self.playlist.entries(), |e| self.violation_for(e, now));
        let id = match pick {
            NextPick::Clean(id) => id,
            NextPick::Forced { id, title, why } => {
                self.notice(
                    "warn",
                    format!("Aucun titre ne respecte les règles de séparation : « {title} » passe quand même ({why})."),
                );
                id
            }
            NextPick::Empty => return None,
        };
        self.playlist.remove(id)
    }

    // [CTL-70]
    pub(super) fn track_history(&mut self) {
        let side = self.on_air_side();
        let d = &self.decks[side];
        let on_air =
            matches!(d.status.state, DeckState::Playing | DeckState::Paused) && !d.meta.finished;
        let now_path = if on_air { d.meta.path.clone() } else { None };
        let unchanged = matches!(
            (&self.auto.current, &now_path),
            (Some(cp), Some(p)) if cp.deck == side && &cp.path == p
        );
        if unchanged {
            return;
        }
        let mut changed = false;
        if let Some(cp) = self.auto.current.take() {
            if let (Some(lib), Some(id)) = (&self.auto.library, cp.hist_id) {
                if let Err(e) = lib.finish_play(id, cp.started.elapsed().as_secs_f64()) {
                    journal::warn(format!("historique (fin de passage) : {e}"));
                }
            }
            changed = true;
        }
        if let Some(path) = now_path {
            let m = &self.decks[side].meta;
            let listeners = self.cast.total_listeners();
            let hist_id = self.auto.library.as_ref().and_then(|lib| {
                lib.record_play(
                    &path.to_string_lossy(),
                    m.title.trim(),
                    m.artist.trim(),
                    now_secs(),
                    self.cast.casting,
                    listeners,
                )
                .map_err(|e| journal::warn(format!("historique (passage) : {e}")))
                .ok()
            });
            self.auto.current = Some(CurrentPlay {
                deck: side,
                path,
                hist_id,
                started: Instant::now(),
            });
            changed = true;
        }
        if changed {
            self.emit_history();
            self.emit_playlist();
        }
    }

    // [CTL-71]
    pub(super) fn history_clear(&mut self) {
        if let Some(lib) = &self.auto.library {
            if let Err(e) = lib.clear_history() {
                self.notice("error", format!("Impossible de vider l'historique : {e}"));
            }
        }
        if let Some(cp) = self.auto.current.as_mut() {
            cp.hist_id = None;
        }
        self.emit_history();
        self.emit_playlist();
    }

    // [CTL-72]
    pub(super) fn apply_automation(&mut self, settings: AutomationSettings) {
        if settings != self.auto.settings {
            self.auto.settings = settings;
            self.auto.last_purge = None;
            self.emit_playlist();
        }
    }

    // [CTL-73]
    pub(super) fn housekeeping(&mut self) {
        let purge_due = self
            .auto
            .last_purge
            .is_none_or(|t| t.elapsed() > PURGE_EVERY);
        if purge_due {
            self.auto.last_purge = Some(Instant::now());
            self.purge_history();
        }
        if self.auto.last_rules_refresh.elapsed() > RULES_REFRESH_EVERY {
            self.auto.last_rules_refresh = Instant::now();
            self.emit_playlist();
        }
    }

    // [CTL-74]
    fn purge_history(&mut self) {
        let days = self.auto.settings.history_days;
        let Some(lib) = self.auto.library.as_ref().filter(|_| days > 0) else {
            return;
        };
        let cutoff = now_secs() - days as i64 * 86_400;
        match lib.purge_older_than(cutoff) {
            Ok(0) => {}
            Ok(n) => {
                journal::info(format!(
                    "historique : {n} passage(s) de plus de {days} jours purgé(s)"
                ));
                self.emit_history();
            }
            Err(e) => journal::warn(format!("purge de l'historique : {e}")),
        }
    }

    // [CTL-75]
    pub(super) fn emit_history(&self) {
        let (rows, stats) = match &self.auto.library {
            Some(lib) => (
                lib.recent(HISTORY_ROWS).unwrap_or_default(),
                lib.stats().ok(),
            ),
            None => (Vec::new(), None),
        };
        self.emit(
            EV_HISTORY,
            &HistoryView {
                rows,
                stats,
                current: self.auto.current.as_ref().and_then(|c| c.hist_id),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u64, title: &str, error: bool) -> Entry {
        Entry {
            id,
            title: title.into(),
            error: error.then(|| "illisible".into()),
            ..Default::default()
        }
    }

    #[test]
    fn picks_first_clean_entry() {
        let list = [entry(1, "a", false), entry(2, "b", false)];
        let pick = pick_next(&list, |e| (e.id == 1).then(|| "bloqué".into()));
        assert!(matches!(pick, NextPick::Clean(2)));
    }

    #[test]
    fn forces_first_blocked_when_nothing_clean() {
        let list = [
            entry(1, "a", true),
            entry(2, "b", false),
            entry(3, "c", false),
        ];
        let pick = pick_next(&list, |_| Some("règle".into()));
        assert!(matches!(pick, NextPick::Forced { id: 2, .. }));
    }

    #[test]
    fn skips_unreadable_entries() {
        let list = [entry(1, "a", true)];
        assert!(matches!(pick_next(&list, |_| None), NextPick::Empty));
    }
}
