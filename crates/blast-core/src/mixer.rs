use std::sync::Arc;

use crate::{crossfade_gains, DeckId, DeckState, Track};

#[derive(Debug, Clone)]
pub enum Command {
    Load(DeckId, Arc<Track>),
    Eject(DeckId),
    Play(DeckId),
    Pause(DeckId),
    Stop(DeckId),
    Seek(DeckId, usize),
    SetDeckGain(DeckId, f32),
    SetCrossfader(f32),
    StartCrossfade { to: DeckId, frames: usize },
    SetMaster(f32),
    SetAutoMix { enabled: bool, fade_frames: usize },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeckStatus {
    pub state: DeckState,
    pub position: usize,
    pub frames: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Ended(DeckId),
    AutoTransition {
        from: DeckId,
        to: DeckId,
    },
    CrossfadeDone {
        to: f32,
    },
    Tick {
        decks: [DeckStatus; 2],
        crossfader: f32,
        peak: [f32; 2],
    },
}

#[derive(Debug, Default)]
struct Slot {
    track: Option<Arc<Track>>,
    pos: usize,
    playing: bool,
    gain: f32,
}

impl Slot {
    fn frames(&self) -> usize {
        self.track.as_ref().map_or(0, |t| t.frames())
    }

    fn remaining(&self) -> usize {
        self.frames().saturating_sub(self.pos)
    }

    fn status(&self) -> DeckStatus {
        let state = match (&self.track, self.playing) {
            (None, _) => DeckState::Empty,
            (Some(_), true) => DeckState::Playing,
            (Some(_), false) if self.pos >= self.frames() => DeckState::Finished,
            (Some(_), false) if self.pos > 0 => DeckState::Paused,
            (Some(_), false) => DeckState::Loaded,
        };
        DeckStatus {
            state,
            position: self.pos,
            frames: self.frames(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Fade {
    from: f32,
    to: f32,
    total: usize,
    done: usize,
    outgoing: Option<usize>,
}

#[derive(Debug)]
pub struct Mixer {
    sample_rate: u32,
    decks: [Slot; 2],
    crossfader: f32,
    fade: Option<Fade>,
    master: f32,
    automix: bool,
    automix_frames: usize,
    tick_every: usize,
    tick_count: usize,
    peak: [f32; 2],
}

impl Mixer {
    pub fn new(sample_rate: u32) -> Self {
        let mut decks: [Slot; 2] = Default::default();
        for d in decks.iter_mut() {
            d.gain = 1.0;
        }
        Self {
            sample_rate,
            decks,
            crossfader: 0.0,
            fade: None,
            master: 1.0,
            automix: false,
            automix_frames: sample_rate as usize * 6,
            tick_every: (sample_rate / 10).max(1) as usize,
            tick_count: 0,
            peak: [0.0; 2],
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn crossfader(&self) -> f32 {
        self.crossfader
    }

    pub fn status(&self, deck: DeckId) -> DeckStatus {
        self.decks[deck.index()].status()
    }

    pub fn apply(&mut self, cmd: Command) -> Option<Arc<Track>> {
        match cmd {
            Command::Load(id, track) => {
                let slot = &mut self.decks[id.index()];
                slot.pos = 0;
                slot.playing = false;
                return slot.track.replace(track);
            }
            Command::Eject(id) => {
                let slot = &mut self.decks[id.index()];
                slot.pos = 0;
                slot.playing = false;
                return slot.track.take();
            }
            Command::Play(id) => {
                let slot = &mut self.decks[id.index()];
                if slot.remaining() > 0 {
                    slot.playing = true;
                }
            }
            Command::Pause(id) => self.decks[id.index()].playing = false,
            Command::Stop(id) => {
                let slot = &mut self.decks[id.index()];
                slot.playing = false;
                slot.pos = 0;
            }
            Command::Seek(id, frame) => {
                let slot = &mut self.decks[id.index()];
                slot.pos = frame.min(slot.frames());
            }
            Command::SetDeckGain(id, g) => self.decks[id.index()].gain = g.max(0.0),
            Command::SetCrossfader(p) => {
                self.fade = None;
                self.crossfader = p.clamp(0.0, 1.0);
            }
            Command::StartCrossfade { to, frames } => {
                let other = to.other().index();
                let outgoing = self.decks[other].playing.then_some(other);
                self.fade = Some(Fade {
                    from: self.crossfader,
                    to: to.crossfader_side(),
                    total: frames.max(1),
                    done: 0,
                    outgoing,
                });
            }
            Command::SetMaster(g) => self.master = g.max(0.0),
            Command::SetAutoMix {
                enabled,
                fade_frames,
            } => {
                self.automix = enabled;
                self.automix_frames = fade_frames.max(1);
            }
        }
        None
    }

    fn on_air(&self) -> usize {
        if self.crossfader < 0.5 {
            0
        } else {
            1
        }
    }

    fn check_automix<F: FnMut(Event)>(&mut self, emit: &mut F) {
        if !self.automix || self.fade.is_some() {
            return;
        }
        let cur = self.on_air();
        let next = 1 - cur;
        let remaining = self.decks[cur].remaining();
        if !self.decks[cur].playing
            && self.decks[cur].track.is_some()
            && remaining == 0
            && !self.decks[next].playing
            && self.decks[next].remaining() > 0
        {
            self.decks[next].playing = true;
            let to = DeckId::from_index(next);
            self.crossfader = to.crossfader_side();
            emit(Event::AutoTransition {
                from: DeckId::from_index(cur),
                to,
            });
            return;
        }
        if !self.decks[cur].playing || remaining > self.automix_frames {
            return;
        }
        if self.decks[next].playing || self.decks[next].remaining() == 0 {
            return;
        }
        self.decks[next].playing = true;
        let to = DeckId::from_index(next);
        self.fade = Some(Fade {
            from: self.crossfader,
            to: to.crossfader_side(),
            total: remaining.max(1),
            done: 0,
            outgoing: Some(cur),
        });
        emit(Event::AutoTransition {
            from: DeckId::from_index(cur),
            to,
        });
    }

    fn step_fade<F: FnMut(Event)>(&mut self, emit: &mut F) {
        let Some(mut f) = self.fade else {
            return;
        };
        f.done += 1;
        if f.done >= f.total {
            self.crossfader = f.to;
            self.fade = None;
            if let Some(out) = f.outgoing {
                let slot = &mut self.decks[out];
                if slot.playing {
                    slot.playing = false;
                    slot.pos = slot.frames();
                    emit(Event::Ended(DeckId::from_index(out)));
                }
            }
            emit(Event::CrossfadeDone { to: f.to });
        } else {
            self.crossfader = f.from + (f.to - f.from) * (f.done as f32 / f.total as f32);
            self.fade = Some(f);
        }
    }

    fn next_sample<F: FnMut(Event)>(&mut self, i: usize, emit: &mut F) -> (f32, f32) {
        let slot = &mut self.decks[i];
        if !slot.playing {
            return (0.0, 0.0);
        }
        let Some(track) = slot.track.as_ref() else {
            slot.playing = false;
            return (0.0, 0.0);
        };
        let frames = track.frames();
        if slot.pos >= frames {
            slot.playing = false;
            emit(Event::Ended(DeckId::from_index(i)));
            return (0.0, 0.0);
        }
        let l = track.samples[2 * slot.pos];
        let r = track.samples[2 * slot.pos + 1];
        slot.pos += 1;
        if slot.pos >= frames {
            slot.playing = false;
            emit(Event::Ended(DeckId::from_index(i)));
        }
        (l * slot.gain, r * slot.gain)
    }

    pub fn render<F: FnMut(Event)>(&mut self, out: &mut [f32], mut emit: F) {
        let frames = out.len() / 2;
        for f in 0..frames {
            self.check_automix(&mut emit);
            let (ga, gb) = crossfade_gains(self.crossfader);
            let (la, ra) = self.next_sample(0, &mut emit);
            let (lb, rb) = self.next_sample(1, &mut emit);
            self.step_fade(&mut emit);

            let l = ((la * ga + lb * gb) * self.master).clamp(-1.0, 1.0);
            let r = ((ra * ga + rb * gb) * self.master).clamp(-1.0, 1.0);
            out[2 * f] = l;
            out[2 * f + 1] = r;

            self.peak[0] = self.peak[0].max(l.abs());
            self.peak[1] = self.peak[1].max(r.abs());
            self.tick_count += 1;
            if self.tick_count >= self.tick_every {
                self.tick_count = 0;
                emit(Event::Tick {
                    decks: [self.decks[0].status(), self.decks[1].status()],
                    crossfader: self.crossfader,
                    peak: self.peak,
                });
                self.peak = [0.0; 2];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tone(frames: usize, value: f32) -> Arc<Track> {
        Arc::new(Track {
            source: PathBuf::from("test"),
            samples: vec![value; frames * 2],
            sample_rate: 1000,
            source_rate: 1000,
            title: None,
            artist: None,
        })
    }

    fn run(m: &mut Mixer, frames: usize) -> (Vec<f32>, Vec<Event>) {
        let mut out = vec![0.0; frames * 2];
        let mut ev = Vec::new();
        m.render(&mut out, |e| {
            if !matches!(e, Event::Tick { .. }) {
                ev.push(e)
            }
        });
        (out, ev)
    }

    #[test]
    fn plays_and_ends() {
        let mut m = Mixer::new(1000);
        m.apply(Command::Load(DeckId::A, tone(100, 0.5)));
        m.apply(Command::Play(DeckId::A));
        let (out, ev) = run(&mut m, 150);
        assert!((out[0] - 0.5).abs() < 1e-6);
        assert_eq!(out[2 * 120], 0.0);
        assert_eq!(ev, vec![Event::Ended(DeckId::A)]);
        assert_eq!(m.status(DeckId::A).state, DeckState::Finished);
    }

    #[test]
    fn automix_transitions_and_overlaps() {
        let mut m = Mixer::new(1000);
        m.apply(Command::Load(DeckId::A, tone(100, 0.4)));
        m.apply(Command::Load(DeckId::B, tone(100, 0.4)));
        m.apply(Command::SetAutoMix {
            enabled: true,
            fade_frames: 20,
        });
        m.apply(Command::Play(DeckId::A));
        let (out, ev) = run(&mut m, 200);
        assert_eq!(
            ev[0],
            Event::AutoTransition {
                from: DeckId::A,
                to: DeckId::B
            }
        );
        assert!(ev.contains(&Event::Ended(DeckId::A)));
        assert!(ev.contains(&Event::Ended(DeckId::B)));
        assert!((m.crossfader() - 1.0).abs() < 1e-6);
        let last_sound = (0..200).rev().find(|&f| out[2 * f] != 0.0).unwrap();
        assert_eq!(last_sound, 179);
        let ended_a = ev.iter().filter(|e| **e == Event::Ended(DeckId::A)).count();
        assert_eq!(ended_a, 1);
    }

    #[test]
    fn automix_starts_late_loaded_deck() {
        let mut m = Mixer::new(1000);
        m.apply(Command::Load(DeckId::A, tone(30, 0.4)));
        m.apply(Command::SetAutoMix {
            enabled: true,
            fade_frames: 20,
        });
        m.apply(Command::Play(DeckId::A));
        let (_, ev) = run(&mut m, 50);
        assert_eq!(ev, vec![Event::Ended(DeckId::A)]);
        m.apply(Command::Load(DeckId::B, tone(30, 0.4)));
        let (out, ev) = run(&mut m, 10);
        assert_eq!(
            ev,
            vec![Event::AutoTransition {
                from: DeckId::A,
                to: DeckId::B
            }]
        );
        assert!((out[0] - 0.4).abs() < 1e-6);
        assert!((m.crossfader() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn automix_ignores_empty_on_air_deck() {
        let mut m = Mixer::new(1000);
        m.apply(Command::SetAutoMix {
            enabled: true,
            fade_frames: 20,
        });
        m.apply(Command::Load(DeckId::B, tone(30, 0.4)));
        let (out, ev) = run(&mut m, 10);
        assert!(ev.is_empty());
        assert!(out.iter().all(|s| *s == 0.0));
        assert_eq!(m.status(DeckId::B).state, DeckState::Loaded);
    }

    #[test]
    fn manual_crossfade() {
        let mut m = Mixer::new(1000);
        m.apply(Command::StartCrossfade {
            to: DeckId::B,
            frames: 10,
        });
        let (_, ev) = run(&mut m, 20);
        assert!(ev.contains(&Event::CrossfadeDone { to: 1.0 }));
        assert!((m.crossfader() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn manual_crossfade_stops_outgoing_deck() {
        let mut m = Mixer::new(1000);
        m.apply(Command::Load(DeckId::A, tone(100, 0.4)));
        m.apply(Command::Load(DeckId::B, tone(100, 0.4)));
        m.apply(Command::Play(DeckId::A));
        let _ = run(&mut m, 10);
        m.apply(Command::Play(DeckId::B));
        m.apply(Command::StartCrossfade {
            to: DeckId::B,
            frames: 10,
        });
        let (_, ev) = run(&mut m, 20);
        assert!(ev.contains(&Event::Ended(DeckId::A)));
        assert_eq!(m.status(DeckId::A).state, DeckState::Finished);
        assert_eq!(m.status(DeckId::B).state, DeckState::Playing);
    }

    #[test]
    fn output_is_clamped() {
        let mut m = Mixer::new(1000);
        m.apply(Command::Load(DeckId::A, tone(50, 0.9)));
        m.apply(Command::Load(DeckId::B, tone(50, 0.9)));
        m.apply(Command::SetCrossfader(0.5));
        m.apply(Command::Play(DeckId::A));
        m.apply(Command::Play(DeckId::B));
        let (out, _) = run(&mut m, 50);
        assert!(out.iter().all(|s| s.abs() <= 1.0));
    }

    #[test]
    fn load_returns_previous_track() {
        let mut m = Mixer::new(1000);
        assert!(m.apply(Command::Load(DeckId::A, tone(10, 0.1))).is_none());
        assert!(m.apply(Command::Load(DeckId::A, tone(10, 0.1))).is_some());
    }
}
