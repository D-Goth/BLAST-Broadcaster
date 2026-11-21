use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use std::collections::HashMap;

use blast_cast::{Caster, OutputConfig, OutputStatus};
use blast_core::engine::Engine;
use blast_core::{decode_file, Command, DeckId, DeckState, DeckStatus, Event, Track};
use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::config::AppConfig;
use crate::playlist::{self, Entry, Playlist};
use crate::secrets::{output_key, Secrets};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum Deck {
    A,
    B,
}

impl Deck {
    fn id(self) -> DeckId {
        match self {
            Deck::A => DeckId::A,
            Deck::B => DeckId::B,
        }
    }

    fn index(self) -> usize {
        self.id().index()
    }

    fn from_index(i: usize) -> Deck {
        if i == 0 {
            Deck::A
        } else {
            Deck::B
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiCommand {
    Refresh,
    AddPaths {
        paths: Vec<PathBuf>,
        index: Option<usize>,
    },
    LoadPaths {
        deck: Deck,
        paths: Vec<PathBuf>,
    },
    Remove {
        id: u64,
    },
    Move {
        id: u64,
        to: usize,
    },
    Clear,
    LoadEntry {
        id: u64,
        deck: Deck,
    },
    Play {
        deck: Deck,
    },
    Pause {
        deck: Deck,
    },
    Stop {
        deck: Deck,
    },
    Eject {
        deck: Deck,
    },
    Seek {
        deck: Deck,
        fraction: f32,
    },
    Crossfader {
        value: f32,
    },
    Crossfade {
        to: Deck,
    },
    AutoMix {
        enabled: bool,
    },
    FadeSeconds {
        value: f32,
    },
    CastStart,
    CastStop,
    #[serde(skip)]
    Config(Box<AppConfig>),
}

enum Work {
    Scanned {
        entries: Vec<Entry>,
        index: Option<usize>,
        to_deck: Option<Deck>,
    },
    Decoded {
        deck: Deck,
        job: u64,
        result: Result<Track, String>,
        play: bool,
    },
}

#[derive(Debug, Default, Clone)]
struct DeckMeta {
    title: String,
    artist: String,
    path: Option<PathBuf>,
    loading: Option<u64>,
    finished: bool,
}

#[derive(Serialize, Clone)]
struct DeckView {
    state: &'static str,
    position: f64,
    duration: f64,
    title: String,
    artist: String,
    path: Option<PathBuf>,
}

#[derive(Serialize, Clone)]
struct StateView {
    decks: [DeckView; 2],
    crossfader: f32,
    peak: [f32; 2],
    automix: bool,
    fade: f32,
    on_air: Option<Deck>,
    device: String,
    sample_rate: u32,
    audio_error: Option<String>,
}

#[derive(Serialize, Clone)]
struct PlaylistView<'a> {
    entries: &'a [Entry],
    total: f64,
}

#[derive(Serialize, Clone)]
struct CastOutputView {
    id: String,
    name: String,
    kind: &'static str,
    target: String,
    enabled: bool,
    status: OutputStatus,
}

#[derive(Serialize, Clone)]
struct CastView {
    casting: bool,
    song: String,
    outputs: Vec<CastOutputView>,
}

#[derive(Serialize, Clone)]
struct Notice {
    level: &'static str,
    message: String,
}

pub fn spawn(
    app: AppHandle,
    secrets: Arc<Secrets>,
    config: AppConfig,
    warning: Option<String>,
) -> Sender<UiCommand> {
    let (tx, rx) = unbounded();
    thread::Builder::new()
        .name("blast-controller".into())
        .spawn(move || Controller::new(app, rx, secrets, config, warning).run())
        .expect("controller thread");
    tx
}

const EMPTY_STATUS: DeckStatus = DeckStatus {
    state: DeckState::Empty,
    position: 0,
    frames: 0,
};

struct Controller {
    app: AppHandle,
    rx: Receiver<UiCommand>,
    engine: Option<Engine>,
    audio_error: Option<String>,
    device: String,
    sample_rate: u32,
    work_tx: Sender<Work>,
    work_rx: Receiver<Work>,
    playlist: Playlist,
    meta: [DeckMeta; 2],
    status: [DeckStatus; 2],
    crossfader: f32,
    peak: [f32; 2],
    automix: bool,
    fade: f32,
    next_job: u64,
    secrets: Arc<Secrets>,
    outputs: Vec<OutputConfig>,
    caster: Caster,
    casting: bool,
    cast_status: HashMap<String, OutputStatus>,
    master: Vec<f32>,
    song: String,
    startup_warning: Option<String>,
}

impl Controller {
    fn new(
        app: AppHandle,
        rx: Receiver<UiCommand>,
        secrets: Arc<Secrets>,
        config: AppConfig,
        startup_warning: Option<String>,
    ) -> Self {
        let wanted = config.audio.device.clone();
        let (engine, audio_error) = match Engine::start(wanted.as_deref()) {
            Ok(e) => (Some(e), None),
            Err(first) => match wanted {
                Some(_) => match Engine::start(None) {
                    Ok(e) => (
                        Some(e),
                        Some(format!("{first} — sortie par défaut utilisée")),
                    ),
                    Err(e) => (None, Some(e.to_string())),
                },
                None => (None, Some(first.to_string())),
            },
        };
        let sample_rate = engine.as_ref().map_or(48_000, |e| e.sample_rate());
        let device = engine
            .as_ref()
            .map(|e| e.device_name().to_string())
            .unwrap_or_default();
        let (work_tx, work_rx) = unbounded();
        let c = Self {
            app,
            rx,
            engine,
            audio_error,
            device,
            sample_rate,
            work_tx,
            work_rx,
            playlist: Playlist::default(),
            meta: Default::default(),
            status: [EMPTY_STATUS; 2],
            crossfader: 0.0,
            peak: [0.0; 2],
            automix: false,
            fade: 6.0,
            next_job: 0,
            secrets,
            outputs: config.outputs,
            caster: Caster::new(sample_rate),
            casting: false,
            cast_status: HashMap::new(),
            master: Vec::with_capacity(sample_rate as usize),
            song: String::new(),
            startup_warning,
        };
        c.send_automix();
        c
    }

    fn run(mut self) {
        loop {
            match self.rx.recv_timeout(Duration::from_millis(15)) {
                Ok(cmd) => {
                    self.handle(cmd);
                    while let Ok(cmd) = self.rx.try_recv() {
                        self.handle(cmd);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            while let Ok(w) = self.work_rx.try_recv() {
                self.on_work(w);
            }
            let mut events = Vec::new();
            if let Some(engine) = &self.engine {
                let mut tick = None;
                for _ in 0..64 {
                    match engine.try_event() {
                        Some(e @ Event::Tick { .. }) => tick = Some(e),
                        Some(e) => events.push(e),
                        None => break,
                    }
                }
                events.extend(tick);
                engine.collect_garbage();
            }
            for e in events {
                self.on_event(e);
            }
            self.pump_master();
            let updates = self.caster.poll();
            if !updates.is_empty() {
                for u in updates {
                    self.cast_status.insert(u.id, u.status);
                }
                self.emit_cast();
            }
        }
        self.caster.stop_all();
    }

    fn pump_master(&mut self) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        self.master.clear();
        engine.read_master(&mut self.master);
        if self.casting && self.caster.is_active() && !self.master.is_empty() {
            self.caster.push(Arc::from(&self.master[..]));
        }
    }

    fn now_playing(&self) -> String {
        let side = if self.crossfader < 0.5 { 0 } else { 1 };
        if !self.is_playing(side) {
            return String::new();
        }
        let m = &self.meta[side];
        match (m.artist.trim(), m.title.trim()) {
            ("", t) => t.to_string(),
            (a, t) => format!("{a} - {t}"),
        }
    }

    fn update_song(&mut self) {
        let song = self.now_playing();
        if !song.is_empty() && song != self.song {
            self.song = song;
            self.caster.set_metadata(&self.song);
            self.emit_cast();
        }
    }

    fn start_casting(&mut self) {
        let enabled: Vec<OutputConfig> =
            self.outputs.iter().filter(|o| o.enabled).cloned().collect();
        if enabled.is_empty() {
            self.notice(
                "warn",
                "Aucune sortie de diffusion active : ouvre les Réglages, onglet Diffusion.",
            );
            return;
        }
        self.casting = true;
        let song = self.now_playing();
        if !song.is_empty() {
            self.song = song;
        }
        self.caster.set_metadata(&self.song);
        for o in enabled {
            match self
                .secrets
                .get(&output_key(&o.id))
                .filter(|p| !p.is_empty())
            {
                Some(pw) => {
                    self.cast_status
                        .insert(o.id.clone(), OutputStatus::Connecting);
                    self.caster.start(o, pw);
                }
                None => {
                    self.cast_status.insert(
                        o.id.clone(),
                        OutputStatus::Error("mot de passe manquant (Réglages → Diffusion)".into()),
                    );
                }
            }
        }
        self.emit_cast();
    }

    fn stop_casting(&mut self) {
        self.casting = false;
        self.caster.stop_all();
        for s in self.cast_status.values_mut() {
            *s = OutputStatus::Off;
        }
        self.emit_cast();
    }

    fn emit_cast(&self) {
        let view = CastView {
            casting: self.casting,
            song: self.song.clone(),
            outputs: self
                .outputs
                .iter()
                .map(|o| CastOutputView {
                    id: o.id.clone(),
                    name: o.name.clone(),
                    kind: o.kind.label(),
                    target: match o.kind {
                        blast_cast::ServerKind::Icecast => {
                            format!("{}:{}{}", o.host, o.port, o.mount_path())
                        }
                        _ => format!("{}:{}", o.host, o.port),
                    },
                    enabled: o.enabled,
                    status: self
                        .cast_status
                        .get(&o.id)
                        .cloned()
                        .unwrap_or(OutputStatus::Off),
                })
                .collect(),
        };
        let _ = self.app.emit("blast://cast", view);
    }

    fn send(&self, cmd: Command) {
        if let Some(e) = &self.engine {
            e.send(cmd);
        }
    }

    fn send_automix(&self) {
        self.send(Command::SetAutoMix {
            enabled: self.automix,
            fade_frames: (self.fade * self.sample_rate as f32) as usize,
        });
    }

    fn notice(&self, level: &'static str, message: impl Into<String>) {
        let _ = self.app.emit(
            "blast://notice",
            Notice {
                level,
                message: message.into(),
            },
        );
    }

    fn is_playing(&self, i: usize) -> bool {
        self.status[i].state == DeckState::Playing && !self.meta[i].finished
    }

    fn is_active(&self, i: usize) -> bool {
        self.meta[i].loading.is_some()
            || self.is_playing(i)
            || (self.meta[i].path.is_some() && !self.meta[i].finished)
    }

    fn request_load(&mut self, deck: Deck, entry: Entry, play: bool) {
        self.next_job += 1;
        let job = self.next_job;
        let i = deck.index();
        self.meta[i] = DeckMeta {
            title: entry.title.clone(),
            artist: entry.artist.clone(),
            path: Some(entry.path.clone()),
            loading: Some(job),
            finished: false,
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

    fn return_cued(&mut self, deck: Deck) {
        let m = &self.meta[deck.index()];
        if m.loading.is_some() || m.finished {
            return;
        }
        if let Some(path) = m.path.clone() {
            if self.status[deck.index()].position == 0 {
                self.playlist
                    .insert(vec![playlist::make_entry(&path)], Some(0));
            }
        }
    }

    fn fill_automix(&mut self) {
        if !self.automix {
            return;
        }
        let mut changed = false;
        if !self.is_active(0) && !self.is_active(1) {
            if let Some(e) = self.playlist.take_next() {
                let start = if self.crossfader < 0.5 {
                    Deck::A
                } else {
                    Deck::B
                };
                self.request_load(start, e, true);
                changed = true;
            }
        }
        for i in 0..2 {
            if !self.is_active(i) && self.is_active(1 - i) {
                if let Some(e) = self.playlist.take_next() {
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

    fn handle(&mut self, cmd: UiCommand) {
        match cmd {
            UiCommand::Refresh => {
                self.emit_playlist();
                self.emit_cast();
                if let Some(w) = self.startup_warning.take() {
                    self.notice("warn", w);
                }
            }
            UiCommand::CastStart => self.start_casting(),
            UiCommand::CastStop => self.stop_casting(),
            UiCommand::Config(cfg) => {
                self.outputs = cfg.outputs;
                self.cast_status
                    .retain(|id, _| self.outputs.iter().any(|o| &o.id == id));
                if self.casting {
                    self.caster.stop_all();
                    self.cast_status.clear();
                    self.start_casting();
                } else {
                    self.emit_cast();
                }
            }
            UiCommand::AddPaths { paths, index } => self.scan(paths, index, None),
            UiCommand::LoadPaths { deck, paths } => self.scan(paths, None, Some(deck)),
            UiCommand::Remove { id } => {
                self.playlist.remove(id);
                self.emit_playlist();
            }
            UiCommand::Move { id, to } => {
                self.playlist.move_to(id, to);
                self.emit_playlist();
            }
            UiCommand::Clear => {
                self.playlist.clear();
                self.emit_playlist();
            }
            UiCommand::LoadEntry { id, deck } => {
                if self.is_playing(deck.index()) {
                    self.notice(
                        "warn",
                        format!("La platine {deck:?} est à l'antenne : arrête-la avant d'y charger un titre."),
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
            UiCommand::Play { deck } => {
                let i = deck.index();
                if self.meta[i].path.is_some() && self.meta[i].loading.is_none() {
                    if self.meta[i].finished {
                        self.send(Command::Stop(deck.id()));
                        self.meta[i].finished = false;
                    }
                    self.send(Command::Play(deck.id()));
                }
            }
            UiCommand::Pause { deck } => self.send(Command::Pause(deck.id())),
            UiCommand::Stop { deck } => {
                self.send(Command::Stop(deck.id()));
                self.meta[deck.index()].finished = false;
            }
            UiCommand::Eject { deck } => {
                if !self.is_playing(deck.index()) {
                    self.send(Command::Eject(deck.id()));
                    self.meta[deck.index()] = DeckMeta::default();
                    self.status[deck.index()] = EMPTY_STATUS;
                    self.fill_automix();
                }
            }
            UiCommand::Seek { deck, fraction } => {
                let i = deck.index();
                let frames = self.status[i].frames;
                if frames > 0 {
                    let target = (fraction.clamp(0.0, 1.0) as f64 * frames as f64) as usize;
                    self.send(Command::Seek(
                        deck.id(),
                        target.min(frames.saturating_sub(1)),
                    ));
                    self.meta[i].finished = false;
                }
            }
            UiCommand::Crossfader { value } => {
                self.crossfader = value.clamp(0.0, 1.0);
                self.send(Command::SetCrossfader(self.crossfader));
            }
            UiCommand::Crossfade { to } => {
                let i = to.index();
                if self.meta[i].path.is_some() && self.meta[i].loading.is_none() {
                    if self.meta[i].finished {
                        self.send(Command::Stop(to.id()));
                        self.meta[i].finished = false;
                    }
                    self.send(Command::Play(to.id()));
                }
                self.send(Command::StartCrossfade {
                    to: to.id(),
                    frames: ((self.fade.max(0.1)) * self.sample_rate as f32) as usize,
                });
            }
            UiCommand::AutoMix { enabled } => {
                self.automix = enabled;
                self.send_automix();
                self.fill_automix();
            }
            UiCommand::FadeSeconds { value } => {
                self.fade = value.clamp(0.0, 30.0);
                self.send_automix();
            }
        }
        self.emit_state();
    }

    fn scan(&self, paths: Vec<PathBuf>, index: Option<usize>, to_deck: Option<Deck>) {
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

    fn on_work(&mut self, w: Work) {
        match w {
            Work::Scanned {
                mut entries,
                index,
                to_deck,
            } => {
                if entries.is_empty() {
                    self.notice("warn", "Aucun fichier audio trouvé.");
                    return;
                }
                let errors = entries.iter().filter(|e| e.error.is_some()).count();
                if let Some(deck) = to_deck {
                    if self.is_playing(deck.index()) {
                        self.notice(
                            "warn",
                            format!("La platine {deck:?} est à l'antenne : titres ajoutés à la playlist."),
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
            Work::Decoded {
                deck,
                job,
                result,
                play,
            } => {
                let i = deck.index();
                if self.meta[i].loading != Some(job) {
                    return;
                }
                self.meta[i].loading = None;
                match result {
                    Ok(track) => {
                        self.meta[i].title = track
                            .title
                            .clone()
                            .unwrap_or_else(|| self.meta[i].title.clone());
                        self.meta[i].artist = track.artist.clone().unwrap_or_default();
                        self.meta[i].finished = false;
                        self.status[i] = DeckStatus {
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
                    }
                    Err(e) => {
                        self.notice(
                            "error",
                            format!("Impossible de charger « {} » : {e}", self.meta[i].title),
                        );
                        self.meta[i] = DeckMeta::default();
                        self.status[i] = EMPTY_STATUS;
                    }
                }
                self.fill_automix();
                self.emit_state();
            }
        }
    }

    fn on_event(&mut self, e: Event) {
        match e {
            Event::Tick {
                decks,
                crossfader,
                peak,
            } => {
                for (i, d) in decks.iter().enumerate() {
                    if self.meta[i].loading.is_none() {
                        self.status[i] = *d;
                    }
                }
                self.crossfader = crossfader;
                self.peak = peak;
                self.fill_automix();
                self.update_song();
                self.emit_state();
            }
            Event::Ended(id) => {
                let i = id.index();
                self.meta[i].finished = true;
                self.status[i].state = DeckState::Finished;
                self.status[i].position = self.status[i].frames;
                self.fill_automix();
                self.emit_state();
            }
            Event::AutoTransition { .. } | Event::CrossfadeDone { .. } => self.emit_state(),
        }
    }

    fn deck_view(&self, i: usize) -> DeckView {
        let m = &self.meta[i];
        let s = &self.status[i];
        let state = if m.loading.is_some() {
            "loading"
        } else {
            match s.state {
                DeckState::Empty => "empty",
                DeckState::Loaded => "loaded",
                DeckState::Playing => "playing",
                DeckState::Paused => "paused",
                DeckState::Finished => "finished",
            }
        };
        let rate = self.sample_rate as f64;
        DeckView {
            state,
            position: s.position as f64 / rate,
            duration: s.frames as f64 / rate,
            title: m.title.clone(),
            artist: m.artist.clone(),
            path: m.path.clone(),
        }
    }

    fn emit_state(&self) {
        let side = if self.crossfader < 0.5 { 0 } else { 1 };
        let on_air = self.is_playing(side).then(|| Deck::from_index(side));
        let view = StateView {
            decks: [self.deck_view(0), self.deck_view(1)],
            crossfader: self.crossfader,
            peak: self.peak,
            automix: self.automix,
            fade: self.fade,
            on_air,
            device: self.device.clone(),
            sample_rate: self.sample_rate,
            audio_error: self.audio_error.clone(),
        };
        let _ = self.app.emit("blast://state", view);
    }

    fn emit_playlist(&self) {
        let _ = self.app.emit(
            "blast://playlist",
            PlaylistView {
                entries: self.playlist.entries(),
                total: self.playlist.total_secs(),
            },
        );
    }
}
