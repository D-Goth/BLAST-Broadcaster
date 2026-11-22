use blast_aura::{AuraClient, AuraSettings, ObsCommand, ObsEvent};

use super::commands::ObsAction;
use super::views::{ObsInputView, ObsView, EV_OBS};
use super::{now_ms, Controller};
use crate::secrets::AURA_KEY;

// [CTL-89]
pub struct ObsState {
    pub client: Option<AuraClient>,
    pub settings: AuraSettings,
    pub view: ObsView,
}

// [CTL-90]
impl ObsState {
    pub fn new(settings: AuraSettings) -> Self {
        Self {
            client: None,
            view: ObsView::idle("off", settings.now_playing_source.clone()),
            settings,
        }
    }

    pub fn connected(&self) -> bool {
        self.view.status == "connected"
    }

    fn since(active: bool, duration_ms: Option<u64>, previous: Option<u64>) -> Option<u64> {
        if !active {
            return None;
        }
        duration_ms.map(|d| now_ms().saturating_sub(d)).or(previous)
    }

    fn apply(&mut self, e: ObsEvent) -> Option<String> {
        let v = &mut self.view;
        match e {
            ObsEvent::Connecting => {
                if v.status != "connected" {
                    v.status = "connecting";
                }
            }
            ObsEvent::Connected { obs_version, .. } => {
                v.status = "connected";
                v.detail.clear();
                v.obs_version = obs_version;
            }
            ObsEvent::Disconnected { reason, retrying } => {
                let np = std::mem::take(&mut v.now_playing_source);
                *v = ObsView::idle(if retrying { "retrying" } else { "error" }, np);
                v.detail = reason;
            }
            ObsEvent::Scenes { scenes, current } => {
                v.scenes = scenes;
                v.current = current;
            }
            ObsEvent::CurrentScene(s) => v.current = Some(s),
            ObsEvent::Stream {
                active,
                duration_ms,
            } => {
                v.stream = active;
                v.stream_since = Self::since(active, duration_ms, v.stream_since);
            }
            ObsEvent::Record {
                active,
                paused,
                duration_ms,
            } => {
                v.record = active;
                v.record_paused = paused;
                v.record_since = Self::since(active, duration_ms, v.record_since);
            }
            ObsEvent::Inputs(list) => {
                let old = std::mem::take(&mut v.inputs);
                v.inputs = list
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
                if let Some(i) = v.inputs.iter_mut().find(|i| i.name == name) {
                    i.muted = muted;
                }
            }
            ObsEvent::InputVolume { name, mul } => {
                if let Some(i) = v.inputs.iter_mut().find(|i| i.name == name) {
                    i.volume = mul;
                }
            }
            ObsEvent::Info(m) => return Some(m),
            ObsEvent::Error(m) => return Some(format!("OBS — {m}")),
        }
        None
    }
}

impl Controller {
    // [CTL-91]
    pub(super) fn obs_connect(&mut self) {
        self.obs.client = None;
        let pw = self.secrets.get(AURA_KEY);
        self.obs.client = Some(AuraClient::start(self.obs.settings.clone(), pw));
        self.obs.view.status = "connecting";
        self.obs.view.detail.clear();
        self.emit_obs();
    }

    // [CTL-92]
    fn obs_disconnect(&mut self) {
        self.obs.client = None;
        self.obs.view = ObsView::idle("off", self.obs.settings.now_playing_source.clone());
        self.emit_obs();
    }

    // [CTL-93]
    fn obs_send(&self, cmd: ObsCommand) {
        match &self.obs.client {
            Some(a) if self.obs.connected() => a.send(cmd),
            _ => self.notice(
                "warn",
                "OBS n'est pas connecté (panneau AURA OBS → Connecter).",
            ),
        }
    }

    // [CTL-94]
    pub(super) fn obs_now_playing(&self) {
        let src = self.obs.settings.now_playing_source.trim();
        if src.is_empty() || !self.obs.connected() || self.cast.song.is_empty() {
            return;
        }
        if let Some(a) = &self.obs.client {
            a.send(ObsCommand::SetText {
                source: src.to_string(),
                text: self.cast.song.clone(),
            });
        }
    }

    // [CTL-95]
    pub(super) fn handle_obs(&mut self, action: ObsAction) {
        match action {
            ObsAction::Connect => self.obs_connect(),
            ObsAction::Disconnect => self.obs_disconnect(),
            ObsAction::Scene { name } => self.obs_send(ObsCommand::SetScene(name)),
            ObsAction::Stream => self.obs_send(ObsCommand::ToggleStream),
            ObsAction::Record => self.obs_send(ObsCommand::ToggleRecord),
            ObsAction::RecordPause => {
                if self.obs.view.record {
                    self.obs_send(ObsCommand::ToggleRecordPause)
                } else {
                    self.notice("warn", "Aucun enregistrement en cours dans OBS.")
                }
            }
            ObsAction::Replay => self.obs_send(ObsCommand::SaveReplay),
            ObsAction::Snap => {
                let dir = self.ui.pictures_dir().join("BLAST");
                let _ = std::fs::create_dir_all(&dir);
                let file = dir.join(format!("obs-{}.png", now_ms()));
                self.obs_send(ObsCommand::Screenshot(file));
            }
            ObsAction::Volume { input, mul } => self.obs_send(ObsCommand::SetVolume { input, mul }),
            ObsAction::Mute { input } => self.obs_send(ObsCommand::ToggleMute(input)),
        }
    }

    // [CTL-96]
    pub(super) fn pump_obs(&mut self) {
        let Some(a) = &self.obs.client else {
            return;
        };
        let events = a.poll();
        if events.is_empty() {
            return;
        }
        let mut send_song = false;
        for e in events {
            send_song |= matches!(e, ObsEvent::Connected { .. });
            let level = if matches!(e, ObsEvent::Error(_)) {
                "error"
            } else {
                "info"
            };
            if let Some(msg) = self.obs.apply(e) {
                self.notice(level, msg);
            }
        }
        if send_song {
            self.obs_now_playing();
        }
        self.emit_obs();
    }

    // [CTL-97]
    pub(super) fn apply_aura(&mut self, settings: AuraSettings) {
        let errored = self.obs.client.is_some() && self.obs.view.status == "error";
        if settings == self.obs.settings && !errored {
            return;
        }
        let reconnect = self.obs.client.is_some();
        self.obs.view.now_playing_source = settings.now_playing_source.clone();
        self.obs.settings = settings;
        if reconnect {
            self.obs_connect();
        } else {
            self.emit_obs();
        }
    }

    // [CTL-98]
    pub(super) fn emit_obs(&self) {
        self.emit(EV_OBS, &self.obs.view);
    }
}
