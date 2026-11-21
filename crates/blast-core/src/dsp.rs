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
}
