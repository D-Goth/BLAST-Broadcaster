use std::sync::Arc;

use crate::dsp::{db_to_gain, Biquad, MasterChain, MasterParams, VoiceFx, VoiceParams};
use crate::{crossfade_gains, DeckId, DeckState, Track};

// [MIX-01]
pub const MAX_VOICES: usize = 16;
// [MIX-16]
pub const CART_BASE: u16 = 1000;
// [MIX-02]
const MIC_MAX_BACKLOG_MS: usize = 80;

// [MIX-03]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MicParams {
    pub gain_db: f32,
    pub low_db: f32,
    pub mid_db: f32,
    pub high_db: f32,
    pub highpass: bool,
}

impl Default for MicParams {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            low_db: 0.0,
            mid_db: 0.0,
            high_db: 0.0,
            highpass: true,
        }
    }
}

// [MIX-04]
#[derive(Debug, Clone)]
pub enum Command {
    Load(DeckId, Arc<Track>),
    Eject(DeckId),
    Play(DeckId),
    Pause(DeckId),
    Stop(DeckId),
    Seek(DeckId, usize),
    SetDeckGain(DeckId, f32),
    SetDeckRate(DeckId, f64),
    SetCue(DeckId, bool),
    SetCrossfader(f32),
    StartCrossfade {
        to: DeckId,
        frames: usize,
    },
    SetMaster(f32),
    SetAutoMix {
        enabled: bool,
        fade_frames: usize,
    },
    PlaySfx {
        pad: u16,
        track: Arc<Track>,
    },
    StopSfx(u16),
    StopAllSfx,
    StopAllCarts,
    SetSfxGain(f32),
    SetCartGain(f32),
    SetMusicGain(f32),
    SetMicOpen(bool),
    SetMicParams(MicParams),
    SetDuck {
        enabled: bool,
        level_db: f32,
        attack_s: f32,
        release_s: f32,
    },
    SetMicMonitor(bool),
    SetMasterParams(MasterParams),
    SetVoiceParams(VoiceParams),
}

// [MIX-05]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeckStatus {
    pub state: DeckState,
    pub position: usize,
    pub frames: usize,
}

// [MIX-06]
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
    SfxEnded(u16),
    Tick {
        decks: [DeckStatus; 2],
        crossfader: f32,
        peak: [f32; 2],
        deck_peak: [[f32; 2]; 2],
        sfx: u64,
        mic_peak: f32,
        mic_open: bool,
        duck: f32,
        reduction: f32,
    },
}

// [MIX-07]
struct Voice {
    pad: u16,
    track: Arc<Track>,
    pos: usize,
}

// [MIX-08]
struct MicChain {
    hp: Biquad,
    low: Biquad,
    mid: Biquad,
    high: Biquad,
}

// [MIX-09]
#[derive(Debug, Default)]
struct Slot {
    track: Option<Arc<Track>>,
    pos: usize,
    playing: bool,
    gain: f32,
    rate: f64,
    frac: f64,
    cue: bool,
}

// [MIX-18]
pub const DECK_RATE_RANGE: (f64, f64) = (0.5, 2.0);

impl Slot {
    fn frames(&self) -> usize {
        self.track.as_ref().map_or(0, |t| t.frames())
    }

    fn remaining(&self) -> usize {
        self.frames().saturating_sub(self.pos)
    }

    // [MIX-19]
    fn remaining_out(&self) -> usize {
        (self.remaining() as f64 / self.rate) as usize
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
            position: (self.pos as f64 / self.rate) as usize,
            frames: (self.frames() as f64 / self.rate) as usize,
        }
    }
}

// [MIX-10]
#[derive(Debug, Clone, Copy)]
struct Fade {
    from: f32,
    to: f32,
    total: usize,
    done: usize,
    outgoing: Option<usize>,
}

// [MIX-11]
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
    deck_peak: [[f32; 2]; 2],
    voices: Vec<Voice>,
    retired: Vec<Arc<Track>>,
    sfx_gain: f32,
    cart_gain: f32,
    music_gain: f32,
    mic: Option<rtrb::Consumer<f32>>,
    cue_out: Option<rtrb::Producer<f32>>,
    mic_open: bool,
    mic_level: f32,
    mic_gain: f32,
    mic_params: MicParams,
    mic_chain: MicChain,
    mic_peak: f32,
    duck_enabled: bool,
    duck_target: f32,
    duck: f32,
    duck_attack_s: f32,
    duck_release_s: f32,
    mic_monitor: bool,
    master_chain: Option<MasterChain>,
    master_spare: Option<MasterChain>,
    voice_fx: VoiceFx,
}

impl Mixer {
    pub fn new(sample_rate: u32) -> Self {
        let mut decks: [Slot; 2] = Default::default();
        for d in decks.iter_mut() {
            d.gain = 1.0;
            d.rate = 1.0;
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
            deck_peak: [[0.0; 2]; 2],
            voices: Vec::with_capacity(MAX_VOICES),
            retired: Vec::with_capacity(MAX_VOICES * 2),
            sfx_gain: 1.0,
            cart_gain: 1.0,
            music_gain: 1.0,
            mic: None,
            cue_out: None,
            mic_open: false,
            mic_level: 0.0,
            mic_gain: 1.0,
            mic_params: MicParams::default(),
            mic_chain: MicChain::build(&MicParams::default(), sample_rate as f32),
            mic_peak: 0.0,
            duck_enabled: true,
            duck_target: db_to_gain(-12.0),
            duck: 1.0,
            duck_attack_s: 0.2,
            duck_release_s: 0.8,
            mic_monitor: false,
            master_chain: None,
            master_spare: Some(MasterChain::new(sample_rate as f32)),
            voice_fx: VoiceFx::new(sample_rate as f32),
        }
    }

    pub fn attach_mic(&mut self, consumer: rtrb::Consumer<f32>) {
        self.mic = Some(consumer);
    }

    // [MIX-21]
    pub fn attach_cue(&mut self, producer: rtrb::Producer<f32>) {
        self.cue_out = Some(producer);
    }

    pub fn has_mic(&self) -> bool {
        self.mic.is_some()
    }

    pub fn take_retired(&mut self) -> Option<Arc<Track>> {
        self.retired.pop()
    }

    pub fn sfx_mask(&self) -> u64 {
        self.voices
            .iter()
            .filter(|v| v.pad < 64)
            .fold(0u64, |m, v| m | (1u64 << v.pad))
    }

    fn stop_voices(&mut self, carts: bool) {
        let mut i = 0;
        while i < self.voices.len() {
            if (self.voices[i].pad >= CART_BASE) == carts {
                let v = self.voices.swap_remove(i);
                self.retire(v.track);
            } else {
                i += 1;
            }
        }
    }

    fn retire(&mut self, track: Arc<Track>) {
        if self.retired.len() < self.retired.capacity() {
            self.retired.push(track);
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

    // [MIX-12]
    pub fn apply(&mut self, cmd: Command) -> Option<Arc<Track>> {
        match cmd {
            Command::Load(id, track) => {
                let slot = &mut self.decks[id.index()];
                slot.pos = 0;
                slot.frac = 0.0;
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
                slot.frac = 0.0;
            }
            Command::Seek(id, frame) => {
                let slot = &mut self.decks[id.index()];
                slot.pos = ((frame as f64 * slot.rate) as usize).min(slot.frames());
                slot.frac = 0.0;
            }
            Command::SetDeckGain(id, g) => self.decks[id.index()].gain = g.max(0.0),
            Command::SetCue(id, on) => self.decks[id.index()].cue = on,
            Command::SetDeckRate(id, r) => {
                let (lo, hi) = DECK_RATE_RANGE;
                self.decks[id.index()].rate = if r.is_finite() { r.clamp(lo, hi) } else { 1.0 };
            }
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
            Command::PlaySfx { pad, track } => {
                if let Some(i) = self.voices.iter().position(|v| v.pad == pad) {
                    let old = self.voices.swap_remove(i);
                    self.retire(old.track);
                }
                if self.voices.len() >= MAX_VOICES {
                    let old = self.voices.remove(0);
                    self.retire(old.track);
                }
                self.voices.push(Voice { pad, track, pos: 0 });
            }
            Command::StopSfx(pad) => {
                if let Some(i) = self.voices.iter().position(|v| v.pad == pad) {
                    let old = self.voices.swap_remove(i);
                    self.retire(old.track);
                }
            }
            Command::StopAllSfx => self.stop_voices(false),
            Command::StopAllCarts => self.stop_voices(true),
            Command::SetCartGain(g) => self.cart_gain = g.max(0.0),
            Command::SetSfxGain(g) => self.sfx_gain = g.max(0.0),
            Command::SetMusicGain(g) => self.music_gain = g.max(0.0),
            Command::SetMicOpen(open) => self.mic_open = open,
            Command::SetMicParams(p) => {
                self.mic_params = p;
                self.mic_gain = db_to_gain(p.gain_db);
                let fresh = MicChain::build(&p, self.sample_rate as f32);
                self.mic_chain.hp.retune(fresh.hp);
                self.mic_chain.low.retune(fresh.low);
                self.mic_chain.mid.retune(fresh.mid);
                self.mic_chain.high.retune(fresh.high);
            }
            Command::SetMicMonitor(on) => self.mic_monitor = on,
            Command::SetMasterParams(p) => {
                if self.master_chain.is_none() {
                    self.master_chain = self.master_spare.take();
                }
                if let Some(c) = self.master_chain.as_mut() {
                    c.configure(p);
                }
            }
            Command::SetVoiceParams(p) => self.voice_fx.configure(p),
            Command::SetDuck {
                enabled,
                level_db,
                attack_s,
                release_s,
            } => {
                self.duck_enabled = enabled;
                self.duck_target = db_to_gain(level_db.min(0.0));
                self.duck_attack_s = attack_s.clamp(0.0, 10.0);
                self.duck_release_s = release_s.clamp(0.0, 10.0);
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
        let remaining = self.decks[cur].remaining_out();
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
        let (l, r) = if slot.rate == 1.0 && slot.frac == 0.0 {
            let lr = (track.samples[2 * slot.pos], track.samples[2 * slot.pos + 1]);
            slot.pos += 1;
            lr
        } else {
            // [MIX-20]
            let n = (slot.pos + 1).min(frames - 1);
            let t = slot.frac as f32;
            let s = &track.samples;
            let lr = (
                s[2 * slot.pos] + (s[2 * n] - s[2 * slot.pos]) * t,
                s[2 * slot.pos + 1] + (s[2 * n + 1] - s[2 * slot.pos + 1]) * t,
            );
            slot.frac += slot.rate;
            let step = slot.frac.floor();
            slot.frac -= step;
            slot.pos += step as usize;
            lr
        };
        if slot.pos >= frames {
            slot.playing = false;
            emit(Event::Ended(DeckId::from_index(i)));
        }
        (l * slot.gain, r * slot.gain)
    }

    fn mic_sample(&mut self) -> f32 {
        let Some(mic) = self.mic.as_mut() else {
            return 0.0;
        };
        match mic.pop() {
            Ok(s) => {
                let c = &mut self.mic_chain;
                let mut x = s;
                if self.mic_params.highpass {
                    x = c.hp.process(x);
                }
                x = self.voice_fx.process(x);
                let c = &mut self.mic_chain;
                x = c.high.process(c.mid.process(c.low.process(x)));
                x * self.mic_gain
            }
            Err(_) => 0.0,
        }
    }

    fn sfx_sample<F: FnMut(Event)>(&mut self, emit: &mut F) -> (f32, f32) {
        let mut l = 0.0;
        let mut r = 0.0;
        let mut i = 0;
        while i < self.voices.len() {
            let v = &mut self.voices[i];
            let frames = v.track.frames();
            if v.pos >= frames {
                let done = self.voices.swap_remove(i);
                emit(Event::SfxEnded(done.pad));
                self.retire(done.track);
                continue;
            }
            let g = if v.pad >= CART_BASE {
                self.cart_gain
            } else {
                self.sfx_gain
            };
            l += v.track.samples[2 * v.pos] * g;
            r += v.track.samples[2 * v.pos + 1] * g;
            v.pos += 1;
            i += 1;
        }
        (l, r)
    }

    // [MIX-13]
    pub fn render<F: FnMut(Event)>(&mut self, out: &mut [f32], emit: F) {
        self.render_split(None, out, emit);
    }

    // [MIX-14]
    pub fn render_split<F: FnMut(Event)>(
        &mut self,
        mut local: Option<&mut [f32]>,
        out: &mut [f32],
        mut emit: F,
    ) {
        let frames = out.len() / 2;
        if let Some(mic) = self.mic.as_mut() {
            let max = self.sample_rate as usize * MIC_MAX_BACKLOG_MS / 1000 + frames;
            let backlog = mic.slots();
            if backlog > max {
                if let Ok(chunk) = mic.read_chunk(backlog - max / 2) {
                    chunk.commit_all();
                }
            }
        }
        let mic_step = 1.0 / (self.sample_rate as f32 * 0.02);
        let duck_depth = (1.0 - self.duck_target).max(1e-3);
        let duck_attack = duck_step(duck_depth, self.duck_attack_s, self.sample_rate);
        let duck_release = duck_step(duck_depth, self.duck_release_s, self.sample_rate);
        for f in 0..frames {
            self.check_automix(&mut emit);
            let (ga, gb) = crossfade_gains(self.crossfader);
            let (la, ra) = self.next_sample(0, &mut emit);
            let (lb, rb) = self.next_sample(1, &mut emit);
            for (p, (l, r)) in self.deck_peak.iter_mut().zip([(la, ra), (lb, rb)]) {
                p[0] = p[0].max(l.abs());
                p[1] = p[1].max(r.abs());
            }
            self.step_fade(&mut emit);

            let mic_target = if self.mic_open { 1.0 } else { 0.0 };
            if self.mic_level < mic_target {
                self.mic_level = (self.mic_level + mic_step).min(1.0);
            } else if self.mic_level > mic_target {
                self.mic_level = (self.mic_level - mic_step).max(0.0);
            }
            let duck_goal = if self.duck_enabled && self.mic_open {
                self.duck_target
            } else {
                1.0
            };
            if self.duck > duck_goal {
                self.duck = (self.duck - duck_attack).max(duck_goal);
            } else if self.duck < duck_goal {
                self.duck = (self.duck + duck_release).min(duck_goal);
            }
            let m = self.mic_sample();
            self.mic_peak = self.mic_peak.max(m.abs());
            let m = m * self.mic_level;
            let (sl, sr) = self.sfx_sample(&mut emit);
            let music = self.music_gain * self.duck;

            let bed_l = (la * ga + lb * gb) * music + sl;
            let bed_r = (ra * ga + rb * gb) * music + sr;
            let mon = if self.mic_monitor { m } else { 0.0 };
            let air = [(bed_l + m) * self.master, (bed_r + m) * self.master];
            let loc = [(bed_l + mon) * self.master, (bed_r + mon) * self.master];
            let (air, loc) = match self.master_chain.as_mut() {
                Some(c) => c.process(air, loc),
                None => (air, loc),
            };
            let l = air[0].clamp(-1.0, 1.0);
            let r = air[1].clamp(-1.0, 1.0);
            out[2 * f] = l;
            out[2 * f + 1] = r;
            let loc = [loc[0].clamp(-1.0, 1.0), loc[1].clamp(-1.0, 1.0)];
            // [MIX-22]
            let cue = self
                .decks
                .iter()
                .zip([(la, ra), (lb, rb)])
                .filter(|(d, _)| d.cue && d.track.is_some())
                .fold(None, |acc: Option<[f32; 2]>, (_, (l, r))| {
                    let [al, ar] = acc.unwrap_or_default();
                    Some([al + l, ar + r])
                })
                .map(|[l, r]| [l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0)]);
            let shown = match (self.cue_out.as_mut(), cue) {
                (Some(tx), c) => {
                    if tx.slots() >= 2 {
                        let [cl, cr] = c.unwrap_or(loc);
                        let _ = tx.push(cl);
                        let _ = tx.push(cr);
                    }
                    loc
                }
                (None, Some([cl, cr])) => [(loc[0] + loc[1]) * 0.5, (cl + cr) * 0.5],
                (None, None) => loc,
            };
            if let Some(dst) = local.as_deref_mut() {
                dst[2 * f] = shown[0];
                dst[2 * f + 1] = shown[1];
            }

            self.peak[0] = self.peak[0].max(l.abs());
            self.peak[1] = self.peak[1].max(r.abs());
            self.tick_count += 1;
            if self.tick_count >= self.tick_every {
                self.tick_count = 0;
                emit(Event::Tick {
                    decks: [self.decks[0].status(), self.decks[1].status()],
                    crossfader: self.crossfader,
                    peak: self.peak,
                    deck_peak: self.deck_peak,
                    sfx: self.sfx_mask(),
                    mic_peak: self.mic_peak,
                    mic_open: self.mic_open,
                    duck: self.duck,
                    reduction: self
                        .master_chain
                        .as_mut()
                        .map_or(0.0, |c| c.take_reduction()),
                });
                self.peak = [0.0; 2];
                self.deck_peak = [[0.0; 2]; 2];
                self.mic_peak = 0.0;
            }
        }
    }
}

// [MIX-15]
impl MicChain {
    fn build(p: &MicParams, rate: f32) -> Self {
        Self {
            hp: Biquad::highpass(90.0, 0.707, rate),
            low: Biquad::low_shelf(150.0, p.low_db, rate),
            mid: Biquad::peaking(1500.0, 0.8, p.mid_db, rate),
            high: Biquad::high_shelf(6000.0, p.high_db, rate),
        }
    }
}

// [MIX-17]
fn duck_step(depth: f32, secs: f32, sample_rate: u32) -> f32 {
    depth / (sample_rate as f32 * secs.max(0.005))
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
            format: None,
            cover: None,
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

    #[test]
    fn sfx_plays_over_music_and_ends() {
        let mut m = Mixer::new(1000);
        m.apply(Command::PlaySfx {
            pad: 3,
            track: tone(20, 0.25),
        });
        assert_eq!(m.sfx_mask(), 1 << 3);
        let (out, ev) = run(&mut m, 30);
        assert!((out[0] - 0.25).abs() < 1e-6);
        assert_eq!(out[2 * 25], 0.0);
        assert_eq!(ev, vec![Event::SfxEnded(3)]);
        assert_eq!(m.sfx_mask(), 0);
        assert!(m.take_retired().is_some());
    }

    #[test]
    fn carts_have_their_own_gain_and_stop() {
        let mut m = Mixer::new(1000);
        m.apply(Command::SetSfxGain(0.0));
        m.apply(Command::SetCartGain(0.5));
        m.apply(Command::PlaySfx {
            pad: CART_BASE,
            track: tone(50, 0.4),
        });
        m.apply(Command::PlaySfx {
            pad: 1,
            track: tone(50, 0.4),
        });
        assert_eq!(m.sfx_mask(), 1 << 1);
        let (out, _) = run(&mut m, 2);
        assert!((out[0] - 0.2).abs() < 1e-6);
        m.apply(Command::StopAllSfx);
        let (out, _) = run(&mut m, 2);
        assert!((out[0] - 0.2).abs() < 1e-6);
        m.apply(Command::StopAllCarts);
        let (out, ev) = run(&mut m, 2);
        assert_eq!(out[0], 0.0);
        assert!(ev.is_empty());
    }

    #[test]
    fn sfx_restart_and_stop_all() {
        let mut m = Mixer::new(1000);
        m.apply(Command::PlaySfx {
            pad: 1,
            track: tone(100, 0.1),
        });
        m.apply(Command::PlaySfx {
            pad: 1,
            track: tone(100, 0.1),
        });
        m.apply(Command::PlaySfx {
            pad: 2,
            track: tone(100, 0.1),
        });
        assert_eq!(m.sfx_mask(), 0b110);
        m.apply(Command::StopAllSfx);
        assert_eq!(m.sfx_mask(), 0);
    }

    #[test]
    fn mic_is_mixed_only_when_open_and_ducks_music() {
        let mut m = Mixer::new(1000);
        let (mut tx, rx) = rtrb::RingBuffer::<f32>::new(4096);
        m.attach_mic(rx);
        m.apply(Command::SetMicParams(MicParams {
            highpass: false,
            ..Default::default()
        }));
        m.apply(Command::SetDuck {
            enabled: true,
            level_db: -12.0,
            attack_s: 0.2,
            release_s: 0.8,
        });
        m.apply(Command::Load(DeckId::A, tone(3000, 0.5)));
        m.apply(Command::Play(DeckId::A));
        for _ in 0..60 {
            tx.push(0.2).unwrap();
        }
        let (out, _) = run(&mut m, 60);
        assert!(
            (out[2 * 59] - 0.5).abs() < 1e-4,
            "micro fermé : musique seule"
        );
        m.apply(Command::SetMicOpen(true));
        for _ in 0..900 {
            tx.push(0.2).unwrap();
        }
        let (out, _) = run(&mut m, 900);
        let expected = 0.5 * db_to_gain(-12.0) + 0.2;
        assert!(
            (out[2 * 899] - expected).abs() < 1e-3,
            "{} vs {}",
            out[2 * 899],
            expected
        );
        m.apply(Command::SetMicOpen(false));
        for _ in 0..1500 {
            tx.push(0.2).unwrap();
        }
        let (out, _) = run(&mut m, 1500);
        assert!(
            (out[2 * 1499] - 0.5).abs() < 1e-3,
            "retour de la musique : {}",
            out[2 * 1499]
        );
    }

    fn ramp(frames: usize) -> Arc<Track> {
        let mut samples = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let v = i as f32 * 0.001;
            samples.push(v);
            samples.push(v);
        }
        Arc::new(Track {
            source: PathBuf::from("ramp"),
            samples,
            sample_rate: 1000,
            source_rate: 1000,
            title: None,
            artist: None,
            format: None,
            cover: None,
        })
    }

    fn render_local(m: &mut Mixer, frames: usize) -> (Vec<f32>, Vec<f32>) {
        let mut out = vec![0.0; frames * 2];
        let mut local = vec![0.0; frames * 2];
        m.render_split(Some(&mut local), &mut out, |_| {});
        (out, local)
    }

    #[test]
    fn cue_splits_the_local_output_but_never_the_air() {
        let mut m = Mixer::new(1000);
        m.apply(Command::Load(DeckId::A, tone(100, 0.4)));
        m.apply(Command::Load(DeckId::B, tone(100, 0.2)));
        m.apply(Command::Play(DeckId::A));
        m.apply(Command::Play(DeckId::B));
        let (out, local) = render_local(&mut m, 4);
        assert!((local[6] - 0.4).abs() < 1e-5 && (local[7] - 0.4).abs() < 1e-5);
        m.apply(Command::SetCue(DeckId::B, true));
        let (out2, local) = render_local(&mut m, 4);
        assert_eq!(out, out2, "l'antenne ne change pas");
        assert!((local[6] - 0.4).abs() < 1e-5, "gauche : antenne");
        assert!((local[7] - 0.2).abs() < 1e-5, "droite : pré-écoute B");
        m.apply(Command::Eject(DeckId::B));
        let (_, local) = render_local(&mut m, 2);
        assert!(
            (local[3] - 0.4).abs() < 1e-5,
            "platine vide : plus de split"
        );
    }

    #[test]
    fn cue_device_gets_the_cued_deck_or_the_program() {
        let mut m = Mixer::new(1000);
        let (tx, mut rx) = rtrb::RingBuffer::<f32>::new(64);
        m.attach_cue(tx);
        m.apply(Command::Load(DeckId::A, tone(100, 0.4)));
        m.apply(Command::Load(DeckId::B, tone(100, 0.2)));
        m.apply(Command::Play(DeckId::A));
        m.apply(Command::SetCue(DeckId::B, false));
        let (_, local) = render_local(&mut m, 2);
        assert!(
            (local[1] - 0.4).abs() < 1e-5,
            "pas de split avec une sortie dédiée"
        );
        assert!(
            (rx.pop().unwrap() - 0.4).abs() < 1e-5,
            "rien en pré-écoute : le programme"
        );
        while rx.pop().is_ok() {}
        m.apply(Command::SetCue(DeckId::B, true));
        m.apply(Command::Play(DeckId::B));
        render_local(&mut m, 2);
        assert!((rx.pop().unwrap() - 0.2).abs() < 1e-5);
        render_local(&mut m, 100);
        assert_eq!(rx.slots(), 63, "file pleine : le surplus est jeté");
    }

    #[test]
    fn pitch_changes_speed_and_reported_times() {
        let mut m = Mixer::new(1000);
        m.apply(Command::Load(DeckId::A, ramp(400)));
        m.apply(Command::SetDeckRate(DeckId::A, 0.5));
        m.apply(Command::Play(DeckId::A));
        let (out, _) = run(&mut m, 5);
        for k in 0..5 {
            assert!(
                (out[2 * k] - 0.0005 * k as f32).abs() < 1e-5,
                "{k}: {}",
                out[2 * k]
            );
        }
        assert_eq!(m.decks[0].status().frames, 800);

        m.apply(Command::SetDeckRate(DeckId::A, 2.0));
        m.apply(Command::Seek(DeckId::A, 100));
        assert_eq!(m.decks[0].pos, 200);
        assert_eq!(m.decks[0].status().position, 100);
        let (_, ev) = run(&mut m, 99);
        assert!(ev.is_empty());
        let (_, ev) = run(&mut m, 1);
        assert_eq!(ev, vec![Event::Ended(DeckId::A)]);

        m.apply(Command::SetDeckRate(DeckId::B, f64::NAN));
        assert_eq!(m.decks[1].rate, 1.0);
        m.apply(Command::SetDeckRate(DeckId::B, 9.0));
        assert_eq!(m.decks[1].rate, DECK_RATE_RANGE.1);
    }

    #[test]
    fn automix_starts_on_time_with_pitch() {
        let mut m = Mixer::new(1000);
        m.apply(Command::SetAutoMix {
            enabled: true,
            fade_frames: 100,
        });
        m.apply(Command::Load(DeckId::A, tone(1000, 0.5)));
        m.apply(Command::Load(DeckId::B, tone(1000, 0.5)));
        m.apply(Command::SetDeckRate(DeckId::A, 1.25));
        m.apply(Command::Play(DeckId::A));
        let (_, ev) = run(&mut m, 699);
        assert!(ev.is_empty(), "{ev:?}");
        let (_, ev) = run(&mut m, 2);
        assert!(matches!(ev[..], [Event::AutoTransition { .. }]), "{ev:?}");
    }

    #[test]
    fn each_deck_reports_its_own_level_after_its_volume() {
        let mut m = Mixer::new(1000);
        m.apply(Command::Load(DeckId::A, tone(500, 0.5)));
        m.apply(Command::Load(DeckId::B, tone(500, 0.8)));
        m.apply(Command::SetDeckGain(DeckId::B, 0.5));
        m.apply(Command::Play(DeckId::A));
        m.apply(Command::Play(DeckId::B));
        let mut ticks = Vec::new();
        let mut out = vec![0.0; 200];
        m.render(&mut out, |e| {
            if let Event::Tick { deck_peak, .. } = e {
                ticks.push(deck_peak);
            }
        });
        let last = ticks.last().unwrap();
        assert!((last[0][0] - 0.5).abs() < 1e-6);
        assert!((last[1][1] - 0.4).abs() < 1e-6);
    }

    #[test]
    fn duck_times_set_how_long_the_music_takes_to_move() {
        let mut m = Mixer::new(1000);
        let (_tx, rx) = rtrb::RingBuffer::<f32>::new(16);
        m.attach_mic(rx);
        m.apply(Command::SetDuck {
            enabled: true,
            level_db: -20.0,
            attack_s: 1.0,
            release_s: 2.0,
        });
        m.apply(Command::SetMicOpen(true));
        run(&mut m, 500);
        let half = 1.0 - (1.0 - db_to_gain(-20.0)) / 2.0;
        assert!((m.duck - half).abs() < 0.01, "mi-attaque : {}", m.duck);
        run(&mut m, 600);
        assert!((m.duck - db_to_gain(-20.0)).abs() < 1e-4);
        m.apply(Command::SetMicOpen(false));
        run(&mut m, 1000);
        assert!((m.duck - half).abs() < 0.01, "mi-retour : {}", m.duck);
        run(&mut m, 1100);
        assert!((m.duck - 1.0).abs() < 1e-4);
    }

    #[test]
    fn mic_goes_to_air_but_not_to_local_unless_monitored() {
        let mut m = Mixer::new(1000);
        let (mut tx, rx) = rtrb::RingBuffer::<f32>::new(4096);
        m.attach_mic(rx);
        m.apply(Command::SetMicParams(MicParams {
            highpass: false,
            ..Default::default()
        }));
        m.apply(Command::SetMicOpen(true));
        for _ in 0..100 {
            tx.push(0.3).unwrap();
        }
        let mut air = vec![0.0; 200];
        let mut local = vec![0.0; 200];
        m.render_split(Some(&mut local), &mut air, |_| {});
        assert!((air[198] - 0.3).abs() < 1e-4);
        assert_eq!(local[198], 0.0);
        m.apply(Command::SetMicMonitor(true));
        for _ in 0..100 {
            tx.push(0.3).unwrap();
        }
        m.render_split(Some(&mut local), &mut air, |_| {});
        assert!((local[198] - 0.3).abs() < 1e-4);
    }
}
