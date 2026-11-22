use std::f32::consts::PI;

#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Default for Biquad {
    fn default() -> Self {
        Self::identity()
    }
}

impl Biquad {
    pub fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn from_raw(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn highpass(freq: f32, q: f32, rate: f32) -> Self {
        let w = 2.0 * PI * freq / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q);
        Self::from_raw(
            (1.0 + c) / 2.0,
            -(1.0 + c),
            (1.0 + c) / 2.0,
            1.0 + alpha,
            -2.0 * c,
            1.0 - alpha,
        )
    }

    pub fn lowpass(freq: f32, q: f32, rate: f32) -> Self {
        let w = 2.0 * PI * freq / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q);
        Self::from_raw(
            (1.0 - c) / 2.0,
            1.0 - c,
            (1.0 - c) / 2.0,
            1.0 + alpha,
            -2.0 * c,
            1.0 - alpha,
        )
    }

    pub fn peaking(freq: f32, q: f32, gain_db: f32, rate: f32) -> Self {
        if gain_db.abs() < 0.01 {
            return Self::identity();
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * PI * freq / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q);
        Self::from_raw(
            1.0 + alpha * a,
            -2.0 * c,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * c,
            1.0 - alpha / a,
        )
    }

    fn shelf(freq: f32, gain_db: f32, rate: f32, high: bool) -> Self {
        if gain_db.abs() < 0.01 {
            return Self::identity();
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * PI * freq / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / 2.0 * 2f32.sqrt();
        let k = 2.0 * a.sqrt() * alpha;
        if high {
            Self::from_raw(
                a * ((a + 1.0) + (a - 1.0) * c + k),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
                a * ((a + 1.0) + (a - 1.0) * c - k),
                (a + 1.0) - (a - 1.0) * c + k,
                2.0 * ((a - 1.0) - (a + 1.0) * c),
                (a + 1.0) - (a - 1.0) * c - k,
            )
        } else {
            Self::from_raw(
                a * ((a + 1.0) - (a - 1.0) * c + k),
                2.0 * a * ((a - 1.0) - (a + 1.0) * c),
                a * ((a + 1.0) - (a - 1.0) * c - k),
                (a + 1.0) + (a - 1.0) * c + k,
                -2.0 * ((a - 1.0) + (a + 1.0) * c),
                (a + 1.0) + (a - 1.0) * c - k,
            )
        }
    }

    pub fn low_shelf(freq: f32, gain_db: f32, rate: f32) -> Self {
        Self::shelf(freq, gain_db, rate, false)
    }

    pub fn high_shelf(freq: f32, gain_db: f32, rate: f32) -> Self {
        Self::shelf(freq, gain_db, rate, true)
    }

    pub fn retune(&mut self, other: Biquad) {
        self.b0 = other.b0;
        self.b1 = other.b1;
        self.b2 = other.b2;
        self.a1 = other.a1;
        self.a2 = other.a2;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub fn gain_to_db(g: f32) -> f32 {
    20.0 * g.max(1e-9).log10()
}

fn coef(ms: f32, rate: f32) -> f32 {
    if ms <= 0.0 {
        0.0
    } else {
        (-1.0 / (ms * 0.001 * rate)).exp()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompParams {
    pub threshold_db: f32,
    pub ratio: f32,
    pub knee_db: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_db: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Compressor {
    p: CompParams,
    att: f32,
    rel: f32,
    gr_db: f32,
}

impl Compressor {
    pub fn new(p: CompParams, rate: f32) -> Self {
        Self {
            p,
            att: coef(p.attack_ms, rate),
            rel: coef(p.release_ms, rate),
            gr_db: 0.0,
        }
    }

    pub fn configure(&mut self, p: CompParams, rate: f32) {
        self.p = p;
        self.att = coef(p.attack_ms, rate);
        self.rel = coef(p.release_ms, rate);
    }

    fn curve(&self, x_db: f32) -> f32 {
        let over = x_db - self.p.threshold_db;
        let slope = 1.0 / self.p.ratio.max(1.0) - 1.0;
        let knee = self.p.knee_db.max(0.0);
        if knee > 0.0 && 2.0 * over.abs() <= knee {
            slope * (over + knee / 2.0).powi(2) / (2.0 * knee)
        } else if over > 0.0 {
            slope * over
        } else {
            0.0
        }
    }

    #[inline]
    pub fn gain(&mut self, level: f32) -> f32 {
        let target = self.curve(gain_to_db(level));
        let c = if target < self.gr_db {
            self.att
        } else {
            self.rel
        };
        self.gr_db = target + c * (self.gr_db - target);
        db_to_gain(self.gr_db + self.p.makeup_db)
    }

    pub fn reduction_db(&self) -> f32 {
        -self.gr_db
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgcParams {
    pub target_db: f32,
    pub max_gain_db: f32,
    pub min_gain_db: f32,
    pub gate_db: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Agc {
    p: AgcParams,
    ms_coef: f32,
    ms: f32,
    gain_db: f32,
    up: f32,
    down: f32,
}

impl Agc {
    pub fn new(p: AgcParams, rate: f32) -> Self {
        Self {
            p,
            ms_coef: coef(400.0, rate),
            ms: 0.0,
            gain_db: 0.0,
            up: 2.0 / rate,
            down: 8.0 / rate,
        }
    }

    pub fn configure(&mut self, p: AgcParams) {
        self.p = p;
    }

    #[inline]
    pub fn gain(&mut self, l: f32, r: f32) -> f32 {
        let x = (l * l + r * r) * 0.5;
        self.ms = x + self.ms_coef * (self.ms - x);
        let level_db = 10.0 * self.ms.max(1e-12).log10();
        if level_db > self.p.gate_db {
            let wanted =
                (self.p.target_db - level_db).clamp(self.p.min_gain_db, self.p.max_gain_db);
            let d = wanted - self.gain_db;
            self.gain_db += d.clamp(-self.down, self.up);
        }
        db_to_gain(self.gain_db)
    }
}

pub const LIMITER_LOOKAHEAD_MS: f32 = 3.0;

pub struct Limiter {
    ceiling: f32,
    len: usize,
    air: Vec<[f32; 2]>,
    local: Vec<[f32; 2]>,
    targets: Vec<f32>,
    pos: usize,
    g: f32,
    att: f32,
    rel: f32,
}

impl Limiter {
    pub fn new(rate: f32) -> Self {
        let len = ((LIMITER_LOOKAHEAD_MS * 0.001 * rate) as usize).max(1);
        Self {
            ceiling: db_to_gain(-1.0),
            len,
            air: vec![[0.0; 2]; len],
            local: vec![[0.0; 2]; len],
            targets: vec![1.0; len],
            pos: 0,
            g: 1.0,
            att: coef(LIMITER_LOOKAHEAD_MS / 3.0, rate),
            rel: coef(80.0, rate),
        }
    }

    pub fn configure(&mut self, ceiling_db: f32, release_ms: f32, rate: f32) {
        self.ceiling = db_to_gain(ceiling_db.min(0.0));
        self.rel = coef(release_ms, rate);
    }

    pub fn latency(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn process(&mut self, air: [f32; 2], local: [f32; 2]) -> ([f32; 2], [f32; 2]) {
        let peak = air[0]
            .abs()
            .max(air[1].abs())
            .max(local[0].abs())
            .max(local[1].abs());
        let target = if peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        let out_air = self.air[self.pos];
        let out_local = self.local[self.pos];
        self.air[self.pos] = air;
        self.local[self.pos] = local;
        self.targets[self.pos] = target;
        self.pos = (self.pos + 1) % self.len;
        let wmin = self.targets.iter().fold(1.0f32, |m, &t| m.min(t));
        if wmin < self.g {
            self.g = wmin + self.att * (self.g - wmin);
        } else {
            self.g = wmin + self.rel * (self.g - wmin);
        }
        let c = self.ceiling;
        let g = self.g;
        (
            [(out_air[0] * g).clamp(-c, c), (out_air[1] * g).clamp(-c, c)],
            [
                (out_local[0] * g).clamp(-c, c),
                (out_local[1] * g).clamp(-c, c),
            ],
        )
    }

    pub fn reduction_db(&self) -> f32 {
        -gain_to_db(self.g)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MasterParams {
    pub agc: Option<AgcParams>,
    pub comp: Option<CompParams>,
    pub ceiling_db: f32,
    pub release_ms: f32,
}

pub const MASTER_PRESETS: &[(&str, &str)] = &[
    ("aucun", "Aucun traitement"),
    ("doux", "Doux"),
    ("radio_fm", "Radio FM"),
    ("puissant", "Puissant"),
];

pub fn master_preset(id: &str) -> MasterParams {
    match id {
        "aucun" => MasterParams {
            agc: None,
            comp: None,
            ceiling_db: -0.3,
            release_ms: 50.0,
        },
        "radio_fm" => MasterParams {
            agc: Some(AgcParams {
                target_db: -17.0,
                max_gain_db: 9.0,
                min_gain_db: -9.0,
                gate_db: -50.0,
            }),
            comp: Some(CompParams {
                threshold_db: -20.0,
                ratio: 3.5,
                knee_db: 6.0,
                attack_ms: 5.0,
                release_ms: 150.0,
                makeup_db: 4.0,
            }),
            ceiling_db: -0.8,
            release_ms: 60.0,
        },
        "puissant" => MasterParams {
            agc: Some(AgcParams {
                target_db: -14.0,
                max_gain_db: 12.0,
                min_gain_db: -9.0,
                gate_db: -50.0,
            }),
            comp: Some(CompParams {
                threshold_db: -22.0,
                ratio: 5.0,
                knee_db: 4.0,
                attack_ms: 3.0,
                release_ms: 100.0,
                makeup_db: 6.0,
            }),
            ceiling_db: -0.5,
            release_ms: 40.0,
        },
        _ => MasterParams {
            agc: None,
            comp: Some(CompParams {
                threshold_db: -16.0,
                ratio: 2.0,
                knee_db: 8.0,
                attack_ms: 20.0,
                release_ms: 250.0,
                makeup_db: 1.5,
            }),
            ceiling_db: -1.0,
            release_ms: 80.0,
        },
    }
}

pub struct MasterChain {
    rate: f32,
    params: MasterParams,
    agc: Agc,
    comp: Compressor,
    limiter: Limiter,
    max_reduction: f32,
}

impl MasterChain {
    pub fn new(rate: f32) -> Self {
        let p = master_preset("doux");
        let dummy_agc = AgcParams {
            target_db: -18.0,
            max_gain_db: 0.0,
            min_gain_db: 0.0,
            gate_db: -50.0,
        };
        let dummy_comp = CompParams {
            threshold_db: 0.0,
            ratio: 1.0,
            knee_db: 0.0,
            attack_ms: 5.0,
            release_ms: 100.0,
            makeup_db: 0.0,
        };
        let mut c = Self {
            rate,
            params: p,
            agc: Agc::new(p.agc.unwrap_or(dummy_agc), rate),
            comp: Compressor::new(p.comp.unwrap_or(dummy_comp), rate),
            limiter: Limiter::new(rate),
            max_reduction: 0.0,
        };
        c.configure(p);
        c
    }

    pub fn configure(&mut self, p: MasterParams) {
        self.params = p;
        if let Some(a) = p.agc {
            self.agc.configure(a);
        }
        if let Some(cp) = p.comp {
            self.comp.configure(cp, self.rate);
        }
        self.limiter
            .configure(p.ceiling_db, p.release_ms, self.rate);
    }

    #[inline]
    pub fn process(&mut self, air: [f32; 2], local: [f32; 2]) -> ([f32; 2], [f32; 2]) {
        let mut g = 1.0;
        if self.params.agc.is_some() {
            g *= self.agc.gain(air[0], air[1]);
        }
        if self.params.comp.is_some() {
            let level = (air[0] * g).abs().max((air[1] * g).abs());
            g *= self.comp.gain(level);
        }
        let out = self
            .limiter
            .process([air[0] * g, air[1] * g], [local[0] * g, local[1] * g]);
        let red = self.limiter.reduction_db()
            + if self.params.comp.is_some() {
                self.comp.reduction_db()
            } else {
                0.0
            };
        self.max_reduction = self.max_reduction.max(red);
        out
    }

    pub fn take_reduction(&mut self) -> f32 {
        std::mem::take(&mut self.max_reduction)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceParams {
    pub gate_db: Option<f32>,
    pub hp_hz: f32,
    pub lp_hz: f32,
    pub low: (f32, f32),
    pub presence: (f32, f32, f32),
    pub high: (f32, f32),
    pub drive: f32,
    pub tube: bool,
    pub comp: Option<CompParams>,
    pub deess_db: Option<f32>,
    pub noise_db: Option<f32>,
    pub crackle_per_s: f32,
    pub output_db: f32,
}

impl Default for VoiceParams {
    fn default() -> Self {
        Self {
            gate_db: None,
            hp_hz: 0.0,
            lp_hz: 0.0,
            low: (150.0, 0.0),
            presence: (3000.0, 0.0, 0.9),
            high: (8000.0, 0.0),
            drive: 0.0,
            tube: false,
            comp: None,
            deess_db: None,
            noise_db: None,
            crackle_per_s: 0.0,
            output_db: 0.0,
        }
    }
}

pub const VOICE_PRESETS: &[(&str, &str)] = &[
    ("aucun", "Aucun effet"),
    ("naturel", "Naturel"),
    ("podcast", "Podcast chaleureux"),
    ("radio_fm", "Voix radio FM"),
    ("asmr", "ASMR"),
    ("tsf_1930", "Radio TSF 1930"),
    ("telephone", "Téléphone"),
    ("megaphone", "Mégaphone"),
];

fn comp(
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    makeup_db: f32,
) -> Option<CompParams> {
    Some(CompParams {
        threshold_db,
        ratio,
        knee_db: 6.0,
        attack_ms,
        release_ms,
        makeup_db,
    })
}

pub fn voice_preset(id: &str) -> VoiceParams {
    let d = VoiceParams::default();
    match id {
        "naturel" => VoiceParams {
            gate_db: Some(-55.0),
            comp: comp(-22.0, 2.0, 8.0, 150.0, 2.0),
            deess_db: Some(-30.0),
            ..d
        },
        "podcast" => VoiceParams {
            gate_db: Some(-52.0),
            hp_hz: 80.0,
            low: (160.0, 3.0),
            presence: (3200.0, 2.0, 0.9),
            drive: 0.6,
            comp: comp(-24.0, 3.0, 5.0, 120.0, 5.0),
            deess_db: Some(-28.0),
            ..d
        },
        "radio_fm" => VoiceParams {
            gate_db: Some(-50.0),
            hp_hz: 110.0,
            low: (130.0, 2.5),
            presence: (2500.0, 3.0, 0.8),
            high: (8000.0, 3.0),
            drive: 0.4,
            comp: comp(-28.0, 4.0, 3.0, 100.0, 7.0),
            deess_db: Some(-26.0),
            ..d
        },
        "asmr" => VoiceParams {
            hp_hz: 35.0,
            low: (200.0, 4.0),
            high: (9000.0, 5.0),
            comp: comp(-34.0, 2.0, 15.0, 300.0, 8.0),
            output_db: 2.0,
            ..d
        },
        "tsf_1930" => VoiceParams {
            gate_db: Some(-50.0),
            hp_hz: 380.0,
            lp_hz: 3000.0,
            presence: (1400.0, 4.0, 0.7),
            drive: 2.5,
            tube: true,
            comp: comp(-22.0, 3.0, 5.0, 120.0, 4.0),
            noise_db: Some(-44.0),
            crackle_per_s: 4.0,
            ..d
        },
        "telephone" => VoiceParams {
            gate_db: Some(-50.0),
            hp_hz: 420.0,
            lp_hz: 3400.0,
            presence: (1800.0, 3.0, 1.0),
            drive: 1.4,
            comp: comp(-24.0, 4.0, 3.0, 80.0, 5.0),
            ..d
        },
        "megaphone" => VoiceParams {
            gate_db: Some(-48.0),
            hp_hz: 750.0,
            lp_hz: 4500.0,
            presence: (2000.0, 6.0, 1.2),
            drive: 4.0,
            comp: comp(-26.0, 6.0, 2.0, 60.0, 4.0),
            output_db: -3.0,
            ..d
        },
        _ => d,
    }
}

pub struct VoiceFx {
    rate: f32,
    p: VoiceParams,
    hp1: Biquad,
    hp2: Biquad,
    lp1: Biquad,
    lp2: Biquad,
    low: Biquad,
    presence: Biquad,
    high: Biquad,
    deess_band: Biquad,
    deess_env: f32,
    deess_att: f32,
    deess_rel: f32,
    gate_env: f32,
    gate_gain: f32,
    gate_hold: usize,
    env_att: f32,
    env_rel: f32,
    gate_open: f32,
    gate_close: f32,
    comp: Compressor,
    noise_tone: Biquad,
    rng: u32,
    crackle: f32,
    out_gain: f32,
}

impl VoiceFx {
    pub fn new(rate: f32) -> Self {
        let mut v = Self {
            rate,
            p: VoiceParams::default(),
            hp1: Biquad::identity(),
            hp2: Biquad::identity(),
            lp1: Biquad::identity(),
            lp2: Biquad::identity(),
            low: Biquad::identity(),
            presence: Biquad::identity(),
            high: Biquad::identity(),
            deess_band: Biquad::highpass(5500.0, 0.707, rate),
            deess_env: 0.0,
            deess_att: coef(1.0, rate),
            deess_rel: coef(60.0, rate),
            gate_env: 0.0,
            gate_gain: 1.0,
            gate_hold: 0,
            env_att: coef(1.0, rate),
            env_rel: coef(80.0, rate),
            gate_open: 1.0 / (0.002 * rate),
            gate_close: 1.0 / (0.15 * rate),
            comp: Compressor::new(
                CompParams {
                    threshold_db: 0.0,
                    ratio: 1.0,
                    knee_db: 0.0,
                    attack_ms: 5.0,
                    release_ms: 100.0,
                    makeup_db: 0.0,
                },
                rate,
            ),
            noise_tone: Biquad::low_shelf(400.0, -8.0, rate),
            rng: 0x9E37_79B9,
            crackle: 0.0,
            out_gain: 1.0,
        };
        v.configure(VoiceParams::default());
        v
    }

    pub fn configure(&mut self, p: VoiceParams) {
        let r = self.rate;
        let nyq = r * 0.45;
        self.p = p;
        let hp = if p.hp_hz > 0.0 {
            Biquad::highpass(p.hp_hz.min(nyq), 0.707, r)
        } else {
            Biquad::identity()
        };
        self.hp1.retune(hp);
        self.hp2.retune(if p.hp_hz > 200.0 {
            hp
        } else {
            Biquad::identity()
        });
        let lp = if p.lp_hz > 0.0 {
            Biquad::lowpass(p.lp_hz.min(nyq), 0.707, r)
        } else {
            Biquad::identity()
        };
        self.lp1.retune(lp);
        self.lp2.retune(lp);
        self.low.retune(Biquad::low_shelf(p.low.0, p.low.1, r));
        self.presence.retune(Biquad::peaking(
            p.presence.0.min(nyq),
            p.presence.2,
            p.presence.1,
            r,
        ));
        self.high
            .retune(Biquad::high_shelf(p.high.0.min(nyq), p.high.1, r));
        if let Some(c) = p.comp {
            self.comp.configure(c, r);
        }
        self.out_gain = db_to_gain(p.output_db);
    }

    #[inline]
    fn noise(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    #[inline]
    fn saturate(&self, x: f32) -> f32 {
        let d = self.p.drive;
        if d <= 0.0 {
            return x;
        }
        let k = 1.0 + d;
        if self.p.tube {
            let bias = 0.18;
            ((k * (x + bias)).tanh() - (k * bias).tanh()) / k.tanh()
        } else {
            (k * x).tanh() / k.tanh()
        }
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let mut y = self.hp2.process(self.hp1.process(x));
        y = self.lp2.process(self.lp1.process(y));

        if let Some(th) = self.p.gate_db {
            let a = y.abs();
            let c = if a > self.gate_env {
                self.env_att
            } else {
                self.env_rel
            };
            self.gate_env = a + c * (self.gate_env - a);
            if gain_to_db(self.gate_env) > th {
                self.gate_hold = (self.rate * 0.12) as usize;
                self.gate_gain = (self.gate_gain + self.gate_open).min(1.0);
            } else if self.gate_hold > 0 {
                self.gate_hold -= 1;
            } else {
                self.gate_gain = (self.gate_gain - self.gate_close).max(0.0);
            }
            y *= self.gate_gain;
        }

        y = self
            .high
            .process(self.presence.process(self.low.process(y)));
        y = self.saturate(y);

        if self.p.comp.is_some() {
            let g = self.comp.gain(y.abs());
            y *= g;
        }

        if let Some(th) = self.p.deess_db {
            let s = self.deess_band.process(y);
            let a = s.abs();
            let c = if a > self.deess_env {
                self.deess_att
            } else {
                self.deess_rel
            };
            self.deess_env = a + c * (self.deess_env - a);
            let over = gain_to_db(self.deess_env) - th;
            if over > 0.0 {
                y *= db_to_gain(-(over * 0.6).min(10.0));
            }
        }

        if let Some(n) = self.p.noise_db {
            let raw = self.noise();
            let hiss = self.noise_tone.process(raw) * db_to_gain(n);
            y += hiss;
            if self.p.crackle_per_s > 0.0 {
                let chance = self.p.crackle_per_s / self.rate;
                if (self.noise() * 0.5 + 0.5) < chance {
                    self.crackle = self.noise() * db_to_gain(n + 18.0);
                }
                y += self.crackle;
                self.crackle *= 0.6;
            }
        }
        y * self.out_gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gain_at(mut f: Biquad, freq: f32, rate: f32) -> f32 {
        let n = rate as usize;
        let mut peak = 0.0f32;
        for i in 0..n {
            let x = (2.0 * PI * freq * i as f32 / rate).sin();
            let y = f.process(x);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        peak
    }

    #[test]
    fn identity_is_transparent() {
        assert!((gain_at(Biquad::identity(), 1000.0, 48_000.0) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn highpass_cuts_lows() {
        let r = 48_000.0;
        assert!(gain_at(Biquad::highpass(100.0, 0.707, r), 30.0, r) < 0.15);
        assert!(gain_at(Biquad::highpass(100.0, 0.707, r), 2000.0, r) > 0.97);
    }

    #[test]
    fn peaking_and_shelves_match_gain() {
        let r = 48_000.0;
        let g = gain_at(Biquad::peaking(1000.0, 0.9, 6.0, r), 1000.0, r);
        assert!((g - db_to_gain(6.0)).abs() < 0.05, "{g}");
        let g = gain_at(Biquad::low_shelf(120.0, -6.0, r), 40.0, r);
        assert!((g - db_to_gain(-6.0)).abs() < 0.08, "{g}");
        let g = gain_at(Biquad::high_shelf(6000.0, 6.0, r), 15_000.0, r);
        assert!((g - db_to_gain(6.0)).abs() < 0.1, "{g}");
    }

    fn sine(freq: f32, amp: f32, rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * PI * freq * i as f32 / rate).sin() * amp)
            .collect()
    }

    fn peak(x: &[f32]) -> f32 {
        x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    #[test]
    fn compressor_ratio() {
        let r = 48_000.0;
        let mut c = Compressor::new(
            CompParams {
                threshold_db: -20.0,
                ratio: 4.0,
                knee_db: 0.0,
                attack_ms: 1.0,
                release_ms: 50.0,
                makeup_db: 0.0,
            },
            r,
        );
        let mut g = 1.0;
        for _ in 0..10_000 {
            g = c.gain(db_to_gain(-8.0));
        }
        assert!((gain_to_db(g) - (-9.0)).abs() < 0.1, "{}", gain_to_db(g));
        for _ in 0..30_000 {
            g = c.gain(db_to_gain(-30.0));
        }
        assert!(gain_to_db(g).abs() < 0.1);
    }

    #[test]
    fn limiter_never_exceeds_ceiling() {
        let r = 48_000.0;
        let mut l = Limiter::new(r);
        l.configure(-1.0, 60.0, r);
        let x = sine(220.0, 3.0, r, 24_000);
        let mut out = Vec::new();
        for &s in &x {
            let (a, _) = l.process([s, s], [s, s]);
            out.push(a[0]);
        }
        assert!(peak(&out) <= db_to_gain(-1.0) + 1e-6);
        let mut l = Limiter::new(r);
        l.configure(-1.0, 60.0, r);
        let q = sine(220.0, 0.2, r, 4_800);
        let mut out = Vec::new();
        for &s in &q {
            out.push(l.process([s, s], [s, s]).0[0]);
        }
        let lat = l.latency();
        assert!((out[lat + 1000] - q[1000]).abs() < 1e-6);
    }

    #[test]
    fn agc_brings_quiet_music_up() {
        let r = 48_000.0;
        let mut a = Agc::new(
            AgcParams {
                target_db: -17.0,
                max_gain_db: 9.0,
                min_gain_db: -9.0,
                gate_db: -50.0,
            },
            r,
        );
        let x = sine(440.0, db_to_gain(-30.0), r, r as usize * 8);
        let mut g = 1.0;
        for &s in &x {
            g = a.gain(s, s);
        }
        assert!((gain_to_db(g) - 9.0).abs() < 0.2, "{}", gain_to_db(g));
    }

    #[test]
    fn master_chain_keeps_local_and_air_aligned() {
        let r = 48_000.0;
        let mut m = MasterChain::new(r);
        m.configure(master_preset("radio_fm"));
        let x = sine(330.0, 0.9, r, 48_000);
        let mut max = 0.0f32;
        for &s in &x {
            let (a, l) = m.process([s, s], [s * 0.5, s * 0.5]);
            max = max.max(a[0].abs());
            assert!((l[0] - a[0] * 0.5).abs() < 1e-3 || l[0].abs() <= a[0].abs());
        }
        assert!(max <= db_to_gain(-0.8) + 1e-6);
        assert!(m.take_reduction() > 1.0);
    }

    fn voice_level(id: &str, freq: f32) -> f32 {
        let r = 48_000.0;
        let mut v = VoiceFx::new(r);
        v.configure(voice_preset(id));
        let x = sine(freq, 0.2, r, 48_000);
        let y: Vec<f32> = x.iter().map(|&s| v.process(s)).collect();
        peak(&y[24_000..])
    }

    #[test]
    fn tsf_and_telephone_are_band_limited() {
        for id in ["tsf_1930", "telephone"] {
            let mid = voice_level(id, 1200.0);
            assert!(voice_level(id, 100.0) < mid * 0.15, "{id} graves");
            assert!(voice_level(id, 9000.0) < mid * 0.15, "{id} aigus");
        }
    }

    #[test]
    fn all_voice_presets_are_stable() {
        for (id, _) in VOICE_PRESETS {
            let r = 44_100.0;
            let mut v = VoiceFx::new(r);
            v.configure(voice_preset(id));
            for i in 0..44_100 {
                let s = if i % 5000 < 2500 {
                    (i as f32 * 0.07).sin() * 0.9
                } else {
                    0.0
                };
                let y = v.process(s);
                assert!(y.is_finite() && y.abs() < 4.0, "{id}");
            }
        }
    }

    #[test]
    fn tsf_adds_hiss_on_silence_and_gate_closes_for_others() {
        let r = 48_000.0;
        let mut v = VoiceFx::new(r);
        v.configure(voice_preset("tsf_1930"));
        let y: Vec<f32> = (0..48_000).map(|_| v.process(0.0)).collect();
        assert!(peak(&y[24_000..]) > 1e-4);
        let mut v = VoiceFx::new(r);
        v.configure(voice_preset("podcast"));
        let low = sine(300.0, db_to_gain(-70.0), r, 48_000);
        let y: Vec<f32> = low.iter().map(|&s| v.process(s)).collect();
        assert!(peak(&y[40_000..]) < 1e-5);
    }

    #[test]
    fn deesser_tames_sibilance() {
        let r = 48_000.0;
        let x = sine(7_000.0, 0.5, r, 48_000);
        let mut plain = VoiceFx::new(r);
        plain.configure(VoiceParams::default());
        let mut de = VoiceFx::new(r);
        de.configure(VoiceParams {
            deess_db: Some(-30.0),
            ..VoiceParams::default()
        });
        let a: Vec<f32> = x.iter().map(|&s| plain.process(s)).collect();
        let b: Vec<f32> = x.iter().map(|&s| de.process(s)).collect();
        assert!(peak(&b[24_000..]) < peak(&a[24_000..]) * 0.5);
    }
}
