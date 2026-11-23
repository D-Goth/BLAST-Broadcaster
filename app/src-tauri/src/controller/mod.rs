mod automation;
mod cast;
mod chat;
mod commands;
mod decks;
mod jingles;
mod obs;
mod pads;
mod playlist_ops;
mod queue;
mod rotation;
mod views;
mod voice;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use blast_core::engine::Engine;
use blast_core::{master_preset, Command, Event, Track};
use blast_library::{History, SqliteStore};
use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::config::{AppConfig, CategorySettings};
use crate::journal;
use crate::playlist::{Entry, Playlist};
use crate::secrets::Secrets;

#[cfg(test)]
pub use commands::ObsAction;
pub use commands::{Deck, UiCommand};

use automation::AutoState;
use cast::CastState;
use chat::ChatState;
use decks::DeckSide;
use jingles::JingleState;
use obs::ObsState;
use pads::PadState;
use queue::QueueState;
use rotation::RotationState;
use views::{Notice, EV_NOTICE};
use voice::MicState;

// [CTL-189]
const DECK_GAIN_MIN_DB: f32 = -60.0;
const DECK_GAIN_MAX_DB: f32 = 6.0;

// [CTL-01]
pub trait Ui: Send + 'static {
    fn emit(&self, event: &'static str, payload: serde_json::Value);
    fn pictures_dir(&self) -> PathBuf;
}

// [CTL-02]
struct TauriUi(AppHandle);

impl Ui for TauriUi {
    fn emit(&self, event: &'static str, payload: serde_json::Value) {
        if let Err(e) = self.0.emit(event, payload) {
            journal::warn(format!("émission {event} impossible : {e}"));
        }
    }

    fn pictures_dir(&self) -> PathBuf {
        self.0
            .path()
            .picture_dir()
            .unwrap_or_else(|_| std::env::temp_dir())
    }
}

// [CTL-03]
pub struct Audio {
    engine: Option<Engine>,
    error: Option<String>,
    device: String,
    sample_rate: u32,
}

// [CTL-04]
impl Audio {
    pub fn start(config: &AppConfig) -> Self {
        let wanted = config.audio.device.clone();
        let mic = voice::mic_input(&config.voice);
        let (engine, error) = match Engine::start_with(wanted.as_deref(), mic.clone()) {
            Ok(e) => (Some(e), None),
            Err(first) => match wanted {
                Some(_) => match Engine::start_with(None, mic) {
                    Ok(e) => (
                        Some(e),
                        Some(format!("{first} — sortie par défaut utilisée")),
                    ),
                    Err(e) => (None, Some(e.to_string())),
                },
                None => (None, Some(first.to_string())),
            },
        };
        if let Some(e) = &error {
            journal::error(format!("audio : {e}"));
        }
        Self {
            sample_rate: engine.as_ref().map_or(48_000, |e| e.sample_rate()),
            device: engine
                .as_ref()
                .map(|e| e.device_name().to_string())
                .unwrap_or_default(),
            engine,
            error,
        }
    }

    #[cfg(test)]
    pub fn none(sample_rate: u32) -> Self {
        Self {
            engine: None,
            error: Some("test".into()),
            device: String::new(),
            sample_rate,
        }
    }
}

// [CTL-05]
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
        result: Result<blast_cast::Listeners, String>,
    },
    LibraryBatch(Vec<blast_library::TrackMeta>),
    LibraryDone {
        files: usize,
    },
    JinglesScanned {
        generation: u64,
        entries: Vec<Entry>,
    },
    JingleDecoded {
        generation: u64,
        index: usize,
        result: Result<Track, String>,
    },
    Chat(chat::ChatSource, u64, blast_live::ChatEvent),
}

// [CTL-06]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// [CTL-07]
pub fn now_secs() -> i64 {
    (now_ms() / 1000) as i64
}

// [CTL-08]
pub fn spawn(
    app: AppHandle,
    secrets: Arc<Secrets>,
    config: AppConfig,
    warning: Option<String>,
    db_path: PathBuf,
) -> Result<Sender<UiCommand>, String> {
    let (tx, rx) = unbounded();
    thread::Builder::new()
        .name("blast-controller".into())
        .spawn(move || {
            let audio = Audio::start(&config);
            let (library, warning) = open_library(&db_path, config.automation.memory_size, warning);
            let mut c = Controller::new(
                Box::new(TauriUi(app)),
                rx,
                secrets,
                config,
                warning,
                audio,
                library,
            );
            c.restore_queue(db_path.with_file_name("playlist.json"));
            if c.chat.settings.autoconnect {
                c.chat_connect();
            }
            c.run()
        })
        .map_err(|e| format!("contrôleur : {e}"))?;
    Ok(tx)
}

// [CTL-09]
fn open_library(
    db_path: &Path,
    memory_size: u32,
    warning: Option<String>,
) -> (Option<History>, Option<String>) {
    let opened = SqliteStore::open(db_path)
        .and_then(|store| History::new(Box::new(store), memory_size as usize));
    match opened {
        Ok(h) => (Some(h), warning),
        Err(e) => {
            journal::error(format!("base {} : {e}", db_path.display()));
            (
                None,
                Some(format!(
                    "Base de données indisponible ({e}) : historique et règles de séparation désactivés. {}",
                    warning.unwrap_or_default()
                )),
            )
        }
    }
}

// [CTL-10]
struct Controller {
    ui: Box<dyn Ui>,
    rx: Receiver<UiCommand>,
    work_tx: Sender<Work>,
    work_rx: Receiver<Work>,
    secrets: Arc<Secrets>,
    engine: Option<Engine>,
    audio_error: Option<String>,
    device: String,
    sample_rate: u32,
    playlist: Playlist,
    decks: [DeckSide; 2],
    crossfader: f32,
    peak: [f32; 2],
    deck_peak: [[f32; 2]; 2],
    deck_gain_db: [f32; 2],
    reduction: f32,
    automix: bool,
    fade: f32,
    next_job: u64,
    pending_fade: Option<Deck>,
    master_preset: String,
    startup_warning: Option<String>,
    cast: CastState,
    pads: PadState,
    mic: MicState,
    obs: ObsState,
    auto: AutoState,
    jingles: JingleState,
    categories: CategorySettings,
    rot: RotationState,
    queue: QueueState,
    chat: ChatState,
}

impl Controller {
    // [CTL-11]
    fn new(
        ui: Box<dyn Ui>,
        rx: Receiver<UiCommand>,
        secrets: Arc<Secrets>,
        config: AppConfig,
        startup_warning: Option<String>,
        audio: Audio,
        library: Option<History>,
    ) -> Self {
        let (work_tx, work_rx) = unbounded();
        let mic = MicState::from_engine(audio.engine.as_ref());
        let mut c = Self {
            ui,
            rx,
            work_tx,
            work_rx,
            secrets,
            cast: CastState::new(config.outputs.clone(), audio.sample_rate),
            engine: audio.engine,
            audio_error: audio.error,
            device: audio.device,
            sample_rate: audio.sample_rate,
            playlist: Playlist::default(),
            decks: Default::default(),
            crossfader: 0.0,
            peak: [0.0; 2],
            deck_peak: [[0.0; 2]; 2],
            deck_gain_db: [f32::NAN; 2],
            reduction: 0.0,
            automix: false,
            fade: 6.0,
            next_job: 0,
            pending_fade: None,
            master_preset: config.audio.master_preset.clone(),
            startup_warning,
            pads: PadState::default(),
            mic,
            obs: ObsState::new(config.aura.clone()),
            auto: AutoState::new(library, config.automation.clone()),
            jingles: JingleState::default(),
            categories: config.categories.clone(),
            rot: RotationState::new(config.rotation.clone(), &config.library),
            queue: QueueState::default(),
            chat: ChatState::new(config.chat.clone()),
        };
        c.send(Command::SetMasterParams(master_preset(&c.master_preset)));
        if c.obs.settings.auto_connect {
            c.obs_connect();
        }
        c.send_automix();
        c.apply_voice(&config.voice);
        c.apply_deck_gains(config.audio.deck_gain_db);
        c.sync_pads(&config.soundboard);
        c.sync_jingles(&config.jingles);
        c.load_library_tracks();
        c.rot.refresh_lists();
        c.library_rescan();
        c
    }

    // [CTL-12]
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
            self.drain_work();
            self.pump_engine();
            self.pump_master();
            self.pump_obs();
            self.pump_caster();
            self.poll_listeners();
            self.housekeeping();
            self.save_queue_if_due();
        }
        self.flush_queue();
        self.cast.caster.stop_all();
    }

    // [CTL-13]
    fn drain_work(&mut self) {
        while let Ok(w) = self.work_rx.try_recv() {
            self.on_work(w);
        }
    }

    // [CTL-14]
    fn pump_engine(&mut self) {
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
    }

    // [CTL-15]
    fn send(&self, cmd: Command) {
        if let Some(e) = &self.engine {
            e.send(cmd);
        }
    }

    // [CTL-16]
    fn emit<T: Serialize>(&self, event: &'static str, payload: &T) {
        match serde_json::to_value(payload) {
            Ok(v) => self.ui.emit(event, v),
            Err(e) => journal::error(format!("sérialisation {event} : {e}")),
        }
    }

    // [CTL-17]
    fn notice(&self, level: &'static str, message: impl Into<String>) {
        let message = message.into();
        if level != "info" {
            journal::log(level, &message);
        }
        self.emit(EV_NOTICE, &Notice { level, message });
    }

    // [CTL-18]
    fn handle(&mut self, cmd: UiCommand) {
        match cmd {
            UiCommand::Refresh => self.refresh(),
            UiCommand::Cover { deck, reply } => {
                let cover = self.decks[deck.index()].meta.cover.as_ref();
                let _ = reply.send(cover.map(|(_, c)| c.clone()));
            }
            UiCommand::Shutdown { done } => {
                self.flush_queue();
                let _ = done.send(());
            }
            UiCommand::CastStart => self.start_casting(),
            UiCommand::CastStop => self.stop_casting(),
            UiCommand::Next { deck } => self.next(deck),
            UiCommand::MicOpen { open } => self.mic_open(open),
            UiCommand::PadPlay { index } => self.pad_play(index),
            UiCommand::PadStopAll => self.send(Command::StopAllSfx),
            UiCommand::JinglePlay { index } => self.jingle_play(index),
            UiCommand::JingleStopAll => self.jingle_stop_all(),
            UiCommand::JingleRescan => self.rescan_jingles(),
            UiCommand::ChatConnect => self.chat_connect(),
            UiCommand::ChatDisconnect => self.chat_disconnect(),
            UiCommand::ChatSay { text } => self.chat_broadcast(&text),
            UiCommand::RequestAccept { id } => self.request_accept(id),
            UiCommand::RequestReject { id } => self.request_reject(id),
            UiCommand::LibraryRescan => self.library_rescan(),
            UiCommand::Obs(action) => self.handle_obs(action),
            UiCommand::HistoryClear => self.history_clear(),
            UiCommand::Config(cfg) => self.apply_config(*cfg),
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
            UiCommand::Shuffle { spread } => self.shuffle(spread),
            UiCommand::Dedupe => self.dedupe(),
            UiCommand::SavePlaylist { path } => self.save_m3u(path),
            UiCommand::Clear => {
                self.playlist.clear();
                self.emit_playlist();
            }
            UiCommand::LoadEntry { id, deck } => self.load_entry(id, deck),
            UiCommand::Play { deck } => self.play(deck),
            UiCommand::Pause { deck } => self.send(Command::Pause(deck.id())),
            UiCommand::Stop { deck } => self.stop(deck),
            UiCommand::Eject { deck } => self.eject(deck),
            UiCommand::Seek { deck, fraction } => self.seek(deck, fraction),
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
            UiCommand::DeckGain { deck, db } => {
                let mut gains = self.deck_gain_db;
                gains[deck.index()] = db;
                self.apply_deck_gains(gains);
            }
            UiCommand::FadeSeconds { value } => {
                self.fade = value.clamp(0.0, 30.0);
                self.send_automix();
            }
        }
        self.emit_state();
    }

    // [CTL-188]
    fn apply_deck_gains(&mut self, gains: [f32; 2]) {
        for (i, (db, deck)) in gains.into_iter().zip([Deck::A, Deck::B]).enumerate() {
            let db = db.clamp(DECK_GAIN_MIN_DB, DECK_GAIN_MAX_DB);
            if self.deck_gain_db[i] == db {
                continue;
            }
            self.deck_gain_db[i] = db;
            let gain = if db <= DECK_GAIN_MIN_DB {
                0.0
            } else {
                blast_core::dsp::db_to_gain(db)
            };
            self.send(Command::SetDeckGain(deck.id(), gain));
        }
    }

    // [CTL-19]
    fn refresh(&mut self) {
        self.emit_playlist();
        self.emit_cast();
        self.emit_pads();
        self.emit_jingles();
        self.emit_library();
        self.emit_obs();
        self.emit_history();
        self.emit_chat_state();
        self.emit_chat_history();
        if let Some(w) = self.startup_warning.take() {
            self.notice("warn", w);
        }
    }

    // [CTL-20]
    fn apply_config(&mut self, cfg: AppConfig) {
        if cfg.audio.master_preset != self.master_preset {
            self.master_preset = cfg.audio.master_preset.clone();
            self.send(Command::SetMasterParams(master_preset(&self.master_preset)));
        }
        self.apply_automation(cfg.automation.clone());
        self.apply_aura(cfg.aura.clone());
        self.apply_voice(&cfg.voice);
        self.apply_deck_gains(cfg.audio.deck_gain_db);
        self.sync_pads(&cfg.soundboard);
        self.sync_jingles(&cfg.jingles);
        if cfg.categories != self.categories {
            self.categories = cfg.categories;
            if self.playlist.reclassify(&self.categories) {
                self.emit_playlist();
            }
        }
        self.apply_rotation(cfg.rotation, &cfg.library);
        self.apply_outputs(cfg.outputs);
        self.apply_chat(cfg.chat);
    }

    // [CTL-21]
    fn on_work(&mut self, w: Work) {
        match w {
            Work::Scanned {
                entries,
                index,
                to_deck,
            } => self.on_scanned(entries, index, to_deck),
            Work::Listeners { id, result } => self.on_listeners(id, result),
            Work::PadDecoded {
                index,
                path,
                result,
            } => self.on_pad_decoded(index, path, result),
            Work::Decoded {
                deck,
                job,
                result,
                play,
            } => self.on_decoded(deck, job, result, play),
            Work::JinglesScanned {
                generation,
                entries,
            } => self.on_jingles_scanned(generation, entries),
            Work::LibraryBatch(batch) => self.on_library_batch(batch),
            Work::LibraryDone { files } => self.on_library_done(files),
            Work::JingleDecoded {
                generation,
                index,
                result,
            } => self.on_jingle_decoded(generation, index, result),
            Work::Chat(source, generation, e) => self.on_chat(source, generation, e),
        }
    }

    // [CTL-22]
    fn on_event(&mut self, e: Event) {
        match e {
            Event::Tick {
                decks,
                crossfader,
                peak,
                deck_peak,
                sfx,
                mic_peak,
                mic_open,
                duck,
                reduction,
            } => {
                self.reduction = reduction;
                self.pads.mask = sfx;
                self.mic.peak = mic_peak;
                self.mic.open = mic_open;
                self.mic.duck = duck;
                for (side, d) in self.decks.iter_mut().zip(decks.iter()) {
                    if side.meta.loading.is_none() {
                        side.status = *d;
                    }
                }
                self.crossfader = crossfader;
                self.peak = peak;
                self.deck_peak = deck_peak;
                self.fill_automix();
                self.update_song();
                self.track_history();
                self.emit_state();
            }
            Event::Ended(id) => {
                self.on_ended(id.index());
                self.track_history();
                self.fill_automix();
                self.emit_state();
            }
            Event::AutoTransition { .. } | Event::CrossfadeDone { .. } => self.emit_state(),
            Event::SfxEnded(i) => {
                self.on_voice_ended(i);
                self.emit_state();
            }
        }
    }

    // [CTL-23]
    fn on_air_side(&self) -> usize {
        if self.crossfader < 0.5 {
            0
        } else {
            1
        }
    }
}
