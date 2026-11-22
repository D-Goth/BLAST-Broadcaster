use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use blast_core::dsp::db_to_gain;
use blast_core::{decode_file, Command, Track};

use super::views::{PadView, PadsView, EV_PADS};
use super::{Controller, Work};
use crate::config::{SoundboardSettings, PAD_COUNT};

// [CTL-99]
pub struct Pad {
    pub name: String,
    pub color: String,
    pub path: PathBuf,
    pub track: Option<Arc<Track>>,
    pub error: Option<String>,
}

// [CTL-100]
#[derive(Default)]
pub struct PadState {
    pub pads: HashMap<u16, Pad>,
    pub mask: u64,
}

impl PadState {
    pub fn is_playing(&self, index: u16) -> bool {
        index < 64 && self.mask & (1u64 << index) != 0
    }

    pub fn clear_playing(&mut self, index: u16) {
        if index < 64 {
            self.mask &= !(1u64 << index);
        }
    }
}

impl Controller {
    // [CTL-101]
    pub(super) fn sync_pads(&mut self, sb: &SoundboardSettings) {
        self.send(Command::SetSfxGain(db_to_gain(sb.volume_db)));
        let wanted: Vec<_> = sb
            .pads
            .iter()
            .filter(|p| p.index < PAD_COUNT && !p.path.as_os_str().is_empty())
            .collect();
        let gone: Vec<u16> = self
            .pads
            .pads
            .keys()
            .copied()
            .filter(|i| !wanted.iter().any(|p| p.index == *i))
            .collect();
        for i in gone {
            self.pads.pads.remove(&i);
            self.send(Command::StopSfx(i));
        }
        for p in wanted {
            if let Some(s) = self
                .pads
                .pads
                .get_mut(&p.index)
                .filter(|s| s.path == p.path)
            {
                s.name = p.name.clone();
                s.color = p.color.clone();
                continue;
            }
            self.send(Command::StopSfx(p.index));
            self.pads.pads.insert(
                p.index,
                Pad {
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

    // [CTL-102]
    pub(super) fn on_pad_decoded(
        &mut self,
        index: u16,
        path: PathBuf,
        result: Result<Track, String>,
    ) {
        let Some(p) = self.pads.pads.get_mut(&index).filter(|p| p.path == path) else {
            return;
        };
        match result {
            Ok(t) => {
                p.track = Some(Arc::new(t));
                p.error = None;
            }
            Err(e) => p.error = Some(e),
        }
        self.emit_pads();
    }

    // [CTL-103]
    pub(super) fn pad_play(&mut self, index: u16) {
        if self.pads.is_playing(index) {
            self.send(Command::StopSfx(index));
            return;
        }
        let Some(p) = self.pads.pads.get(&index) else {
            return;
        };
        match (&p.track, &p.error) {
            (Some(t), _) => self.send(Command::PlaySfx {
                pad: index,
                track: t.clone(),
            }),
            (None, Some(e)) => self.notice("error", format!("{} : {e}", p.name)),
            (None, None) => self.notice("info", format!("{} : chargement en cours…", p.name)),
        }
    }

    // [CTL-104]
    pub(super) fn emit_pads(&self) {
        let mut pads: Vec<PadView> = self
            .pads
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
        self.emit(
            EV_PADS,
            &PadsView {
                pads,
                count: PAD_COUNT,
            },
        );
    }
}
