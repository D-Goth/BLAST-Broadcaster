use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use std::collections::HashMap;

use blast_aura::{AuraClient, AuraSettings, ObsCommand, ObsEvent};
use blast_cast::{fetch_listeners, Caster, Listeners, OutputConfig, OutputStatus};
use blast_core::dsp::db_to_gain;
use blast_core::engine::{Engine, MicInput};
use blast_core::{decode_file, Command, DeckId, DeckState, DeckStatus, Event, MicParams, Track};
use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::config::{AppConfig, AutomationSettings, SoundboardSettings, VoiceSettings, PAD_COUNT};
use crate::playlist::{self, Entry, Playlist};
use crate::secrets::{output_key, Secrets, AURA_KEY};
use blast_library::{HistoryRow, Library, LibraryStats, SeparationRules};

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ObsAction {
    Connect,
    Disconnect,
    Scene { name: String },
    Stream,
    Record,
    RecordPause,
    Replay,
    Snap,
    Volume { input: String, mul: f32 },
    Mute { input: String },
}

#[derive(Serialize, Clone)]
struct ObsInputView {
    name: String,
    kind: String,
    muted: bool,
    volume: f32,
}

#[derive(Serialize, Clone, Default)]
struct ObsView {
    status: &'static str,
    detail: String,
    obs_version: String,
    scenes: Vec<String>,
    current: Option<String>,
    stream: bool,
    stream_since: Option<u64>,
    record: bool,
    record_paused: bool,
    record_since: Option<u64>,
    inputs: Vec<ObsInputView>,
    now_playing_source: String,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

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
    Next {
        deck: Deck,
    },
    MicOpen {
        open: bool,
    },
    PadPlay {
        index: u16,
    },
    PadStopAll,
    Obs(ObsAction),
    HistoryClear,
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
    PadDecoded {
        index: u16,
        path: PathBuf,
        result: Result<Track, String>,
    },
    Listeners {
        id: String,
        result: Result<Listeners, String>,
    },
}

struct PadState {
    name: String,
    color: String,
    path: PathBuf,
    track: Option<Arc<Track>>,
    error: Option<String>,
}

#[derive(Serialize, Clone)]
struct PadView {
    index: u16,
    name: String,
    color: String,
    path: PathBuf,
    ready: bool,
    error: Option<String>,
}

#[derive(Serialize, Clone)]
struct PadsView {
    pads: Vec<PadView>,
    count: u16,
}

#[derive(Serialize, Clone)]
struct MicView {
    available: bool,
    open: bool,
    peak: f32,
    name: Option<String>,
    error: Option<String>,
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
    mic: MicView,
    sfx: u64,
    duck: f32,
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
    listeners: Option<u32>,
    peak: Option<u32>,
    live_since: Option<u64>,
}

#[derive(Serialize, Clone)]
struct CastView {
    casting: bool,
    song: String,
    outputs: Vec<CastOutputView>,
    listeners: Option<u32>,
    session_peak: u32,
    since: Option<u64>,
}

#[derive(Serialize, Clone)]
struct HistoryView {
    rows: Vec<HistoryRow>,
    stats: Option<LibraryStats>,
    current: Option<i64>,
}

struct CurrentPlay {
    deck: usize,
    path: PathBuf,
    hist_id: Option<i64>,
    started: std::time::Instant,
}

fn now_secs() -> i64 {
    (now_ms() / 1000) as i64
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
    db_path: PathBuf,
) -> Sender<UiCommand> {
    let (tx, rx) = unbounded();
    thread::Builder::new()
        .name("blast-controller".into())
        .spawn(move || Controller::new(app, rx, secrets, config, warning, db_path).run())
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
    pending_fade: Option<Deck>,
    pads: HashMap<u16, PadState>,
    sfx_mask: u64,
    mic_open: bool,
    mic_peak: f32,
    duck: f32,
    mic_name: Option<String>,
    mic_error: Option<String>,
    mic_available: bool,
    aura: Option<AuraClient>,
    aura_settings: AuraSettings,
    obs: ObsView,
    stats: HashMap<String, (Option<Listeners>, u32)>,
    live_since: HashMap<String, u64>,
    session_peak: u32,
    last_stats: std::time::Instant,
    library: Option<Library>,
    automation: AutomationSettings,
    current_play: Option<CurrentPlay>,
    last_purge: std::time::Instant,
    last_rules_refresh: std::time::Instant,
}

fn mic_input(v: &VoiceSettings) -> MicInput {
    if !v.enabled {
        return MicInput::Off;
    }
    match v.input_device.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => MicInput::Named(name.to_string()),
        _ => MicInput::Default,
    }
}

impl Controller {
    fn new(
        app: AppHandle,
        rx: Receiver<UiCommand>,
        secrets: Arc<Secrets>,
        config: AppConfig,
        startup_warning: Option<String>,
        db_path: PathBuf,
    ) -> Self {
        let wanted = config.audio.device.clone();
        let mic = mic_input(&config.voice);
        let (engine, audio_error) = match Engine::start_with(wanted.as_deref(), mic.clone()) {
            Ok(e) => (Some(e), None),
            Err(first) => match wanted {
                Some(_) => match Engine::start_with(None, mic.clone()) {
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
        let mic_name = engine
            .as_ref()
            .and_then(|e| e.input_name().map(String::from));
        let mic_error = engine
            .as_ref()
            .and_then(|e| e.input_error().map(String::from));
        let mic_available = mic_name.is_some();
        let voice = config.voice.clone();
        let soundboard = config.soundboard.clone();
        let aura_settings = config.aura.clone();
        let (library, startup_warning) = match Library::open(&db_path) {
            Ok(l) => (Some(l), startup_warning),
            Err(e) => (
                None,
                Some(format!(
                    "Base de données indisponible ({}) : historique et règles de séparation désactivés. {}",
                    e,
                    startup_warning.unwrap_or_default()
                )),
            ),
        };
        let mut c = Self {
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
            pending_fade: None,
            pads: HashMap::new(),
            sfx_mask: 0,
            mic_open: false,
            mic_peak: 0.0,
            duck: 1.0,
            mic_name,
            mic_error,
            mic_available,
            aura: None,
            obs: ObsView {
                status: "off",
                now_playing_source: aura_settings.now_playing_source.clone(),
                ..Default::default()
            },
            aura_settings,
            stats: HashMap::new(),
            live_since: HashMap::new(),
            session_peak: 0,
            last_stats: std::time::Instant::now(),
            library,
            automation: config.automation.clone(),
            current_play: None,
            last_purge: std::time::Instant::now() - Duration::from_secs(7200),
            last_rules_refresh: std::time::Instant::now(),
        };
        if c.aura_settings.auto_connect {
            c.obs_connect();
        }
        c.send_automix();
        c.apply_voice(&voice);
        c.sync_pads(&soundboard);
        c
    }

    fn apply_voice(&self, v: &VoiceSettings) {
        self.send(Command::SetMicParams(MicParams {
            gain_db: v.mic_gain_db,
            low_db: v.low_db,
            mid_db: v.mid_db,
            high_db: v.high_db,
            highpass: v.highpass,
        }));
        self.send(Command::SetMusicGain(db_to_gain(v.music_gain_db)));
        self.send(Command::SetDuck {
            enabled: v.duck,
            level_db: v.duck_db,
        });
        self.send(Command::SetMicMonitor(v.monitor));
    }

    fn sync_pads(&mut self, sb: &SoundboardSettings) {
        self.send(Command::SetSfxGain(db_to_gain(sb.volume_db)));
        let wanted: Vec<_> = sb
            .pads
            .iter()
            .filter(|p| p.index < PAD_COUNT && !p.path.as_os_str().is_empty())
            .collect();
        let gone: Vec<u16> = self
            .pads
            .keys()
            .copied()
            .filter(|i| !wanted.iter().any(|p| p.index == *i))
            .collect();
        for i in gone {
            self.pads.remove(&i);
            self.send(Command::StopSfx(i));
        }
        for p in wanted {
            let same = self.pads.get(&p.index).is_some_and(|s| s.path == p.path);
            if same {
                if let Some(s) = self.pads.get_mut(&p.index) {
                    s.name = p.name.clone();
                    s.color = p.color.clone();
                }
                continue;
            }
            self.send(Command::StopSfx(p.index));
            self.pads.insert(
                p.index,
                PadState {
                    name: p.name.clone(),
                    color: p.color.clone(),
                    path: p.path.clone(),
                    track: None,
                    error: None,
                },
            );
            let tx = self.work_tx.clone();
            let rate = self.sample_rate;
            let index = p.index;
            let path = p.path.clone();
            thread::spawn(move || {
                let result = decode_file(&path, rate).map_err(|e| e.to_string());
                let _ = tx.send(Work::PadDecoded {
                    index,
                    path,
                    result,
                });
            });
        }
        self.emit_pads();
    }

    fn obs_connect(&mut self) {
        self.aura = None;
        let pw = self.secrets.get(AURA_KEY);
        self.aura = Some(AuraClient::start(self.aura_settings.clone(), pw));
        self.obs.status = "connecting";
        self.obs.detail.clear();
        self.emit_obs();
    }

    fn obs_disconnect(&mut self) {
        self.aura = None;
        self.obs = ObsView {
            status: "off",
            now_playing_source: self.aura_settings.now_playing_source.clone(),
            ..Default::default()
        };
        self.emit_obs();
    }

    fn obs_send(&self, cmd: ObsCommand) {
        match &self.aura {
            Some(a) if self.obs.status == "connected" => a.send(cmd),
            _ => self.notice(
                "warn",
                "OBS n'est pas connecté (panneau AURA OBS → Connecter).",
            ),
        }
    }

    fn obs_now_playing(&self) {
        let src = self.aura_settings.now_playing_source.trim();
        if src.is_empty() || self.obs.status != "connected" || self.song.is_empty() {
            return;
        }
        if let Some(a) = &self.aura {
            a.send(ObsCommand::SetText {
                source: src.to_string(),
                text: self.song.clone(),
            });
        }
    }

    fn handle_obs(&mut self, action: ObsAction) {
        match action {
            ObsAction::Connect => self.obs_connect(),
            ObsAction::Disconnect => self.obs_disconnect(),
            ObsAction::Scene { name } => self.obs_send(ObsCommand::SetScene(name)),
            ObsAction::Stream => self.obs_send(ObsCommand::ToggleStream),
            ObsAction::Record => self.obs_send(ObsCommand::ToggleRecord),
            ObsAction::RecordPause => {
                if self.obs.record {
                    self.obs_send(ObsCommand::ToggleRecordPause)
                } else {
                    self.notice("warn", "Aucun enregistrement en cours dans OBS.")
                }
            }
            ObsAction::Replay => self.obs_send(ObsCommand::SaveReplay),
            ObsAction::Snap => {
                let dir = self
                    .app
                    .path()
                    .picture_dir()
                    .unwrap_or_else(|_| std::env::temp_dir())
                    .join("BLAST");
                let _ = std::fs::create_dir_all(&dir);
                let file = dir.join(format!("obs-{}.png", now_ms()));
                self.obs_send(ObsCommand::Screenshot(file));
            }
            ObsAction::Volume { input, mul } => self.obs_send(ObsCommand::SetVolume { input, mul }),
            ObsAction::Mute { input } => self.obs_send(ObsCommand::ToggleMute(input)),
        }
    }

    fn pump_obs(&mut self) {
        let Some(a) = &self.aura else {
            return;
        };
        let events = a.poll();
        if events.is_empty() {
            return;
        }
        let mut send_song = false;
        for e in events {
            match e {
                ObsEvent::Connecting => {
                    if self.obs.status != "connected" {
                        self.obs.status = "connecting";
                    }
                }
                ObsEvent::Connected { obs_version, .. } => {
                    self.obs.status = "connected";
                    self.obs.detail.clear();
                    self.obs.obs_version = obs_version;
                    send_song = true;
                }
                ObsEvent::Disconnected { reason, retrying } => {
                    let np = self.obs.now_playing_source.clone();
                    self.obs = ObsView {
                        status: if retrying { "retrying" } else { "error" },
                        detail: reason,
                        now_playing_source: np,
                        ..Default::default()
                    };
                }
                ObsEvent::Scenes { scenes, current } => {
                    self.obs.scenes = scenes;
                    self.obs.current = current;
                }
                ObsEvent::CurrentScene(s) => self.obs.current = Some(s),
                ObsEvent::Stream {
                    active,
                    duration_ms,
                } => {
                    self.obs.stream = active;
                    self.obs.stream_since = if active {
                        duration_ms
                            .map(|d| now_ms().saturating_sub(d))
                            .or(self.obs.stream_since)
                    } else {
                        None
                    };
                }
                ObsEvent::Record {
                    active,
                    paused,
                    duration_ms,
                } => {
                    self.obs.record = active;
                    self.obs.record_paused = paused;
                    self.obs.record_since = if active {
                        duration_ms
                            .map(|d| now_ms().saturating_sub(d))
                            .or(self.obs.record_since)
                    } else {
                        None
                    };
                }
                ObsEvent::Inputs(list) => {
                    let old = std::mem::take(&mut self.obs.inputs);
                    self.obs.inputs = list
                        .into_iter()
                        .map(|i| {
                            let prev = old.iter().find(|o| o.name == i.name);
                            ObsInputView {
                                muted: prev.is_some_and(|p| p.muted),
                                volume: prev.map_or(1.0, |p| p.volume),
                                name: i.name,
                                kind: i.kind,
                            }
                        })
                        .collect();
                }
                ObsEvent::InputMuted { name, muted } => {
                    if let Some(i) = self.obs.inputs.iter_mut().find(|i| i.name == name) {
                        i.muted = muted;
                    }
                }
                ObsEvent::InputVolume { name, mul } => {
                    if let Some(i) = self.obs.inputs.iter_mut().find(|i| i.name == name) {
                        i.volume = mul;
                    }
                }
                ObsEvent::Info(m) => self.notice("info", m),
                ObsEvent::Error(m) => self.notice("error", format!("OBS — {m}")),
            }
        }
        if send_song {
            self.obs_now_playing();
        }
        self.emit_obs();
    }

    fn emit_obs(&self) {
        let _ = self.app.emit("blast://obs", self.obs.clone());
    }

    fn emit_pads(&self) {
        let mut pads: Vec<PadView> = self
            .pads
            .iter()
            .map(|(i, p)| PadView {
                index: *i,
                name: p.name.clone(),
                color: p.color.clone(),
                path: p.path.clone(),
                ready: p.track.is_some(),
                error: p.error.clone(),
            })
            .collect();
        pads.sort_by_key(|p| p.index);
        let _ = self.app.emit(
            "blast://pads",
            PadsView {
                pads,
                count: PAD_COUNT,
            },
        );
    }

    fn crossfade_to(&mut self, to: Deck) {
        let i = to.index();
        if self.meta[i].path.is_none() || self.meta[i].loading.is_some() {
            return;
        }
        if self.meta[i].finished {
            self.send(Command::Stop(to.id()));
            self.meta[i].finished = false;
        }
        self.send(Command::Play(to.id()));
        self.send(Command::StartCrossfade {
            to: to.id(),
            frames: (self.fade.max(0.1) * self.sample_rate as f32) as usize,
        });
    }

    fn fade_or_fetch(&mut self, to: Deck) {
        let o = to.index();
        if self.meta[o].loading.is_some() {
            self.pending_fade = Some(to);
        } else if self.is_playing(o) || (self.meta[o].path.is_some() && !self.meta[o].finished) {
            self.crossfade_to(to);
        } else if let Some(e) = self.take_next_auto() {
            self.request_load(to, e, false);
            self.pending_fade = Some(to);
            self.emit_playlist();
        } else {
            self.notice("warn", "Plus aucun titre dans la playlist.");
        }
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
            self.pump_obs();
            let updates = self.caster.poll();
            if !updates.is_empty() {
                for u in updates {
                    if u.status == OutputStatus::Live {
                        self.live_since.entry(u.id.clone()).or_insert_with(now_ms);
                        self.last_stats = std::time::Instant::now() - Duration::from_secs(8);
                    } else if !matches!(
                        u.status,
                        OutputStatus::Retrying(_) | OutputStatus::Connecting
                    ) {
                        self.live_since.remove(&u.id);
                    }
                    self.cast_status.insert(u.id, u.status);
                }
                self.emit_cast();
            }
            self.poll_listeners();
            self.housekeeping();
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
            self.obs_now_playing();
            self.emit_cast();
        }
    }

    fn start_casting(&mut self) {
        self.session_peak = 0;
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
        self.live_since.clear();
        self.stats.clear();
        self.caster.stop_all();
        for s in self.cast_status.values_mut() {
            *s = OutputStatus::Off;
        }
        self.emit_cast();
    }

    fn housekeeping(&mut self) {
        if self.last_purge.elapsed() > Duration::from_secs(3600) {
            self.last_purge = std::time::Instant::now();
            let days = self.automation.history_days;
            if days > 0 {
                if let Some(lib) = &self.library {
                    let cutoff = now_secs() - days as i64 * 86_400;
                    if let Ok(n) = lib.purge_older_than(cutoff) {
                        if n > 0 {
                            self.emit_history();
                        }
                    }
                }
            }
        }
        if self.last_rules_refresh.elapsed() > Duration::from_secs(30) {
            self.last_rules_refresh = std::time::Instant::now();
            self.emit_playlist();
        }
    }

    fn rules(&self) -> SeparationRules {
        self.automation.rules
    }

    fn violation_for(&self, e: &Entry, now: i64) -> Option<String> {
        if e.error.is_some() {
            return None;
        }
        let rules = self.rules();
        if !rules.enabled {
            return None;
        }
        let artist = e.artist.trim();
        if rules.artist_minutes > 0 && !artist.is_empty() {
            for (i, m) in self.meta.iter().enumerate() {
                let busy = m.loading.is_some() || (m.path.is_some() && !m.finished);
                if busy && m.artist.trim().eq_ignore_ascii_case(artist) {
                    return Some(format!(
                        "même artiste déjà sur la platine {}",
                        if i == 0 { "A" } else { "B" }
                    ));
                }
            }
        }
        let lib = self.library.as_ref()?;
        lib.check(&e.path.to_string_lossy(), &e.title, &e.artist, now, &rules)
            .ok()
            .flatten()
            .map(|v| v.describe())
    }

    fn take_next_auto(&mut self) -> Option<Entry> {
        let now = now_secs();
        let mut pick = None;
        let mut fallback = None;
        for e in self.playlist.entries() {
            if e.error.is_some() {
                continue;
            }
            match self.violation_for(e, now) {
                None => {
                    pick = Some(e.id);
                    break;
                }
                Some(why) => {
                    if fallback.is_none() {
                        fallback = Some((e.id, e.title.clone(), why));
                    }
                }
            }
        }
        let id = match (pick, fallback) {
            (Some(id), _) => id,
            (None, Some((id, title, why))) => {
                self.notice(
                    "warn",
                    format!("Aucun titre ne respecte les règles de séparation : « {title} » passe quand même ({why})."),
                );
                id
            }
            (None, None) => return None,
        };
        self.playlist.remove(id)
    }

    fn track_history(&mut self) {
        let side = if self.crossfader < 0.5 { 0 } else { 1 };
        let on_air = matches!(
            self.status[side].state,
            DeckState::Playing | DeckState::Paused
        ) && !self.meta[side].finished;
        let now_path = if on_air {
            self.meta[side].path.clone()
        } else {
            None
        };
        let unchanged = matches!(
            (&self.current_play, &now_path),
            (Some(cp), Some(p)) if cp.deck == side && &cp.path == p
        );
        if unchanged {
            return;
        }
        let mut changed = false;
        if let Some(cp) = self.current_play.take() {
            if let (Some(lib), Some(id)) = (&self.library, cp.hist_id) {
                let _ = lib.finish_play(id, cp.started.elapsed().as_secs_f64());
            }
            changed = true;
        }
        if let Some(path) = now_path {
            let m = &self.meta[side];
            let hist_id = self.library.as_ref().and_then(|lib| {
                lib.record_play(
                    &path.to_string_lossy(),
                    m.title.trim(),
                    m.artist.trim(),
                    now_secs(),
                    self.casting,
                    self.total_listeners(),
                )
                .ok()
            });
            self.current_play = Some(CurrentPlay {
                deck: side,
                path,
                hist_id,
                started: std::time::Instant::now(),
            });
            changed = true;
        }
        if changed {
            self.emit_history();
            self.emit_playlist();
        }
    }

    fn emit_history(&self) {
        let (rows, stats) = match &self.library {
            Some(lib) => (lib.recent(300).unwrap_or_default(), lib.stats().ok()),
            None => (Vec::new(), None),
        };
        let _ = self.app.emit(
            "blast://history",
            HistoryView {
                rows,
                stats,
                current: self.current_play.as_ref().and_then(|c| c.hist_id),
            },
        );
    }

    fn poll_listeners(&mut self) {
        if !self.casting || self.last_stats.elapsed() < Duration::from_secs(10) {
            return;
        }
        self.last_stats = std::time::Instant::now();
        for o in &self.outputs {
            if self.cast_status.get(&o.id) != Some(&OutputStatus::Live) {
                continue;
            }
            let cfg = o.clone();
            let tx = self.work_tx.clone();
            thread::spawn(move || {
                let result = fetch_listeners(&cfg).map_err(|e| e.to_string());
                let _ = tx.send(Work::Listeners { id: cfg.id, result });
            });
        }
    }

    fn total_listeners(&self) -> Option<u32> {
        let live: Vec<u32> = self
            .stats
            .iter()
            .filter(|(id, _)| self.cast_status.get(*id) == Some(&OutputStatus::Live))
            .filter_map(|(_, (l, _))| l.map(|l| l.current))
            .collect();
        (!live.is_empty()).then(|| live.iter().sum())
    }

    fn emit_cast(&self) {
        let view = CastView {
            casting: self.casting,
            song: self.song.clone(),
            listeners: self.total_listeners(),
            session_peak: self.session_peak,
            since: self.live_since.values().min().copied(),
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
                    listeners: self
                        .stats
                        .get(&o.id)
                        .and_then(|(l, _)| l.map(|l| l.current)),
                    peak: self
                        .stats
                        .get(&o.id)
                        .map(|(l, p)| (*p).max(l.map_or(0, |l| l.peak))),
                    live_since: self.live_since.get(&o.id).copied(),
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
            if let Some(e) = self.take_next_auto() {
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

    fn handle(&mut self, cmd: UiCommand) {
        match cmd {
            UiCommand::Refresh => {
                self.emit_playlist();
                self.emit_cast();
                self.emit_pads();
                self.emit_obs();
                self.emit_history();
                if let Some(w) = self.startup_warning.take() {
                    self.notice("warn", w);
                }
            }
            UiCommand::CastStart => self.start_casting(),
            UiCommand::CastStop => self.stop_casting(),
            UiCommand::Next { deck } => {
                let i = deck.index();
                let other = Deck::from_index(1 - i);
                if self.is_playing(i) {
                    self.fade_or_fetch(other);
                } else if self.meta[i].loading.is_none() {
                    match self.take_next_auto() {
                        Some(e) => {
                            self.request_load(deck, e, false);
                            self.emit_playlist();
                        }
                        None => self.notice("warn", "Plus aucun titre dans la playlist."),
                    }
                }
            }
            UiCommand::MicOpen { open } => {
                if self.mic_available {
                    self.mic_open = open;
                    self.send(Command::SetMicOpen(open));
                } else if open {
                    self.notice(
                        "warn",
                        "Aucun micro disponible : vérifie Réglages → Audio, puis redémarre BLAST.",
                    );
                }
            }
            UiCommand::PadPlay { index } => {
                if index < 64 && self.sfx_mask & (1u64 << index) != 0 {
                    self.send(Command::StopSfx(index));
                } else if let Some(p) = self.pads.get(&index) {
                    match (&p.track, &p.error) {
                        (Some(t), _) => self.send(Command::PlaySfx {
                            pad: index,
                            track: t.clone(),
                        }),
                        (None, Some(e)) => self.notice("error", format!("{} : {e}", p.name)),
                        (None, None) => {
                            self.notice("info", format!("{} : chargement en cours…", p.name))
                        }
                    }
                }
            }
            UiCommand::PadStopAll => self.send(Command::StopAllSfx),
            UiCommand::Obs(action) => self.handle_obs(action),
            UiCommand::HistoryClear => {
                if let Some(lib) = &self.library {
                    let _ = lib.clear_history();
                }
                if let Some(cp) = self.current_play.as_mut() {
                    cp.hist_id = None;
                }
                self.emit_history();
                self.emit_playlist();
            }
            UiCommand::Config(cfg) => {
                if cfg.automation != self.automation {
                    self.automation = cfg.automation.clone();
                    self.last_purge = std::time::Instant::now() - Duration::from_secs(7200);
                    self.emit_playlist();
                }
                if cfg.aura != self.aura_settings
                    || (self.aura.is_some() && self.obs.status == "error")
                {
                    let reconnect = self.aura.is_some();
                    self.aura_settings = cfg.aura.clone();
                    self.obs.now_playing_source = cfg.aura.now_playing_source.clone();
                    if reconnect {
                        self.obs_connect();
                    } else {
                        self.emit_obs();
                    }
                }
                self.apply_voice(&cfg.voice);
                self.sync_pads(&cfg.soundboard);
                let errored = self
                    .cast_status
                    .values()
                    .any(|s| matches!(s, OutputStatus::Error(_)));
                if cfg.outputs != self.outputs || (self.casting && errored) {
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
            UiCommand::Crossfade { to } => self.fade_or_fetch(to),
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
                if let Some(lib) = self.library.as_mut() {
                    let metas: Vec<_> = entries
                        .iter()
                        .filter(|e| e.error.is_none())
                        .map(|e| e.to_meta())
                        .collect();
                    let _ = lib.upsert_tracks(&metas, now_secs());
                }
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
            Work::Listeners { id, result } => {
                if let Ok(l) = result {
                    let entry = self.stats.entry(id).or_insert((None, 0));
                    entry.0 = Some(l);
                    entry.1 = entry.1.max(l.current);
                    if let Some(total) = self.total_listeners() {
                        self.session_peak = self.session_peak.max(total);
                    }
                    self.emit_cast();
                }
            }
            Work::PadDecoded {
                index,
                path,
                result,
            } => {
                if let Some(p) = self.pads.get_mut(&index) {
                    if p.path == path {
                        match result {
                            Ok(t) => {
                                p.track = Some(Arc::new(t));
                                p.error = None;
                            }
                            Err(e) => p.error = Some(e),
                        }
                        self.emit_pads();
                    }
                }
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
                sfx,
                mic_peak,
                mic_open,
                duck,
            } => {
                self.sfx_mask = sfx;
                self.mic_peak = mic_peak;
                self.mic_open = mic_open;
                self.duck = duck;
                for (i, d) in decks.iter().enumerate() {
                    if self.meta[i].loading.is_none() {
                        self.status[i] = *d;
                    }
                }
                self.crossfader = crossfader;
                self.peak = peak;
                self.fill_automix();
                self.update_song();
                self.track_history();
                self.emit_state();
            }
            Event::Ended(id) => {
                let i = id.index();
                self.meta[i].finished = true;
                self.status[i].state = DeckState::Finished;
                self.status[i].position = self.status[i].frames;
                self.track_history();
                self.fill_automix();
                self.emit_state();
            }
            Event::AutoTransition { .. } | Event::CrossfadeDone { .. } => self.emit_state(),
            Event::SfxEnded(i) => {
                if i < 64 {
                    self.sfx_mask &= !(1u64 << i);
                }
                self.emit_state();
            }
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
            mic: MicView {
                available: self.mic_available,
                open: self.mic_open,
                peak: self.mic_peak,
                name: self.mic_name.clone(),
                error: self.mic_error.clone(),
            },
            sfx: self.sfx_mask,
            duck: self.duck,
        };
        let _ = self.app.emit("blast://state", view);
    }

    fn emit_playlist(&self) {
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
        let _ = self.app.emit(
            "blast://playlist",
            PlaylistView {
                entries: &entries,
                total: self.playlist.total_secs(),
            },
        );
    }
}
