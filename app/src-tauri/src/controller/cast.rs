use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use blast_cast::{fetch_listeners, Caster, Listeners, OutputConfig, OutputStatus, ServerKind};

use super::views::{CastOutputView, CastView, EV_CAST};
use super::{now_ms, Controller, Work};
use crate::secrets::output_key;

const STATS_EVERY: Duration = Duration::from_secs(10);

pub struct CastState {
    pub outputs: Vec<OutputConfig>,
    pub caster: Caster,
    pub casting: bool,
    pub status: HashMap<String, OutputStatus>,
    pub stats: HashMap<String, (Option<Listeners>, u32)>,
    pub live_since: HashMap<String, u64>,
    pub session_peak: u32,
    pub last_stats: Instant,
    pub master: Vec<f32>,
    pub song: String,
}

impl CastState {
    pub fn new(outputs: Vec<OutputConfig>, sample_rate: u32) -> Self {
        Self {
            outputs,
            caster: Caster::new(sample_rate),
            casting: false,
            status: HashMap::new(),
            stats: HashMap::new(),
            live_since: HashMap::new(),
            session_peak: 0,
            last_stats: Instant::now(),
            master: Vec::with_capacity(sample_rate as usize),
            song: String::new(),
        }
    }

    fn is_live(&self, id: &str) -> bool {
        self.status.get(id) == Some(&OutputStatus::Live)
    }

    pub fn total_listeners(&self) -> Option<u32> {
        let live: Vec<u32> = self
            .stats
            .iter()
            .filter(|(id, _)| self.is_live(id))
            .filter_map(|(_, (l, _))| l.map(|l| l.current))
            .collect();
        (!live.is_empty()).then(|| live.iter().sum())
    }

    fn output_view(&self, o: &OutputConfig) -> CastOutputView {
        let stats = self.stats.get(&o.id);
        CastOutputView {
            id: o.id.clone(),
            name: o.name.clone(),
            kind: o.kind.label(),
            target: match o.kind {
                ServerKind::Icecast => format!("{}:{}{}", o.host, o.port, o.mount_path()),
                _ => format!("{}:{}", o.host, o.port),
            },
            enabled: o.enabled,
            status: self.status.get(&o.id).cloned().unwrap_or(OutputStatus::Off),
            listeners: stats.and_then(|(l, _)| l.map(|l| l.current)),
            peak: stats.map(|(l, p)| (*p).max(l.map_or(0, |l| l.peak))),
            live_since: self.live_since.get(&o.id).copied(),
        }
    }

    fn view(&self) -> CastView {
        CastView {
            casting: self.casting,
            song: self.song.clone(),
            listeners: self.total_listeners(),
            session_peak: self.session_peak,
            since: self.live_since.values().min().copied(),
            outputs: self.outputs.iter().map(|o| self.output_view(o)).collect(),
        }
    }
}

impl Controller {
    pub(super) fn pump_master(&mut self) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        let cast = &mut self.cast;
        cast.master.clear();
        engine.read_master(&mut cast.master);
        if cast.casting && cast.caster.is_active() && !cast.master.is_empty() {
            cast.caster.push(Arc::from(&cast.master[..]));
        }
    }

    pub(super) fn pump_caster(&mut self) {
        let updates = self.cast.caster.poll();
        if updates.is_empty() {
            return;
        }
        for u in updates {
            if u.status == OutputStatus::Live {
                self.cast
                    .live_since
                    .entry(u.id.clone())
                    .or_insert_with(now_ms);
                self.cast.last_stats = Instant::now() - Duration::from_secs(8);
            } else if !matches!(
                u.status,
                OutputStatus::Retrying(_) | OutputStatus::Connecting
            ) {
                self.cast.live_since.remove(&u.id);
            }
            self.cast.status.insert(u.id, u.status);
        }
        self.emit_cast();
    }

    pub(super) fn now_playing(&self) -> String {
        let side = self.on_air_side();
        if !self.is_playing(side) {
            return String::new();
        }
        let m = &self.decks[side].meta;
        match (m.artist.trim(), m.title.trim()) {
            ("", t) => t.to_string(),
            (a, t) => format!("{a} - {t}"),
        }
    }

    pub(super) fn update_song(&mut self) {
        let song = self.now_playing();
        if !song.is_empty() && song != self.cast.song {
            self.cast.song = song;
            self.cast.caster.set_metadata(&self.cast.song);
            self.obs_now_playing();
            self.emit_cast();
        }
    }

    pub(super) fn start_casting(&mut self) {
        self.cast.session_peak = 0;
        let enabled: Vec<OutputConfig> = self
            .cast
            .outputs
            .iter()
            .filter(|o| o.enabled)
            .cloned()
            .collect();
        if enabled.is_empty() {
            self.notice(
                "warn",
                "Aucune sortie de diffusion active : ouvre les Réglages, onglet Diffusion.",
            );
            return;
        }
        self.cast.casting = true;
        let song = self.now_playing();
        if !song.is_empty() {
            self.cast.song = song;
        }
        self.cast.caster.set_metadata(&self.cast.song);
        for o in enabled {
            let password = self
                .secrets
                .get(&output_key(&o.id))
                .filter(|p| !p.is_empty());
            match password {
                Some(pw) => {
                    self.cast
                        .status
                        .insert(o.id.clone(), OutputStatus::Connecting);
                    self.cast.caster.start(o, pw);
                }
                None => {
                    self.cast.status.insert(
                        o.id.clone(),
                        OutputStatus::Error("mot de passe manquant (Réglages → Diffusion)".into()),
                    );
                }
            }
        }
        self.emit_cast();
    }

    pub(super) fn stop_casting(&mut self) {
        let cast = &mut self.cast;
        cast.casting = false;
        cast.live_since.clear();
        cast.stats.clear();
        cast.caster.stop_all();
        for s in cast.status.values_mut() {
            *s = OutputStatus::Off;
        }
        self.emit_cast();
    }

    pub(super) fn apply_outputs(&mut self, outputs: Vec<OutputConfig>) {
        let errored = self
            .cast
            .status
            .values()
            .any(|s| matches!(s, OutputStatus::Error(_)));
        if outputs == self.cast.outputs && !(self.cast.casting && errored) {
            return;
        }
        let cast = &mut self.cast;
        cast.outputs = outputs;
        let ids: Vec<String> = cast.outputs.iter().map(|o| o.id.clone()).collect();
        cast.status.retain(|id, _| ids.contains(id));
        if cast.casting {
            cast.caster.stop_all();
            cast.status.clear();
            self.start_casting();
        } else {
            self.emit_cast();
        }
    }

    pub(super) fn poll_listeners(&mut self) {
        if !self.cast.casting || self.cast.last_stats.elapsed() < STATS_EVERY {
            return;
        }
        self.cast.last_stats = Instant::now();
        for o in self
            .cast
            .outputs
            .iter()
            .filter(|o| self.cast.is_live(&o.id))
        {
            let cfg = o.clone();
            let tx = self.work_tx.clone();
            thread::spawn(move || {
                let result = fetch_listeners(&cfg).map_err(|e| e.to_string());
                let _ = tx.send(Work::Listeners { id: cfg.id, result });
            });
        }
    }

    pub(super) fn on_listeners(&mut self, id: String, result: Result<Listeners, String>) {
        let l = match result {
            Ok(l) => l,
            Err(e) => {
                crate::journal::warn(format!("statistiques {id} : {e}"));
                return;
            }
        };
        let entry = self.cast.stats.entry(id).or_insert((None, 0));
        entry.0 = Some(l);
        entry.1 = entry.1.max(l.current);
        if let Some(total) = self.cast.total_listeners() {
            self.cast.session_peak = self.cast.session_peak.max(total);
        }
        self.emit_cast();
    }

    pub(super) fn emit_cast(&self) {
        self.emit(EV_CAST, &self.cast.view());
    }
}
