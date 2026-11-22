use blast_core::dsp::db_to_gain;
use blast_core::engine::{Engine, MicInput};
use blast_core::{voice_preset, Command, MicParams};

use super::Controller;
use crate::config::VoiceSettings;

pub struct MicState {
    pub available: bool,
    pub open: bool,
    pub peak: f32,
    pub duck: f32,
    pub name: Option<String>,
    pub error: Option<String>,
}

impl MicState {
    pub fn from_engine(engine: Option<&Engine>) -> Self {
        let name = engine.and_then(|e| e.input_name().map(String::from));
        Self {
            available: name.is_some(),
            open: false,
            peak: 0.0,
            duck: 1.0,
            error: engine.and_then(|e| e.input_error().map(String::from)),
            name,
        }
    }
}

pub fn mic_input(v: &VoiceSettings) -> MicInput {
    if !v.enabled {
        return MicInput::Off;
    }
    match v.input_device.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => MicInput::Named(name.to_string()),
        _ => MicInput::Default,
    }
}

impl Controller {
    pub(super) fn apply_voice(&self, v: &VoiceSettings) {
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
        self.send(Command::SetVoiceParams(voice_preset(&v.preset)));
    }

    pub(super) fn mic_open(&mut self, open: bool) {
        if self.mic.available {
            self.mic.open = open;
            self.send(Command::SetMicOpen(open));
        } else if open {
            self.notice(
                "warn",
                "Aucun micro disponible : vérifie Réglages → Audio, puis redémarre BLAST.",
            );
        }
    }
}
