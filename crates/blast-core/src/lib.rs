pub mod decode;
pub mod mixer;
pub mod resample;

#[cfg(feature = "output")]
pub mod engine;

pub use decode::{decode_file, is_audio_path, probe_file, DecodeError, Track, TrackInfo};
pub use mixer::{Command, DeckStatus, Event, Mixer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeckId {
    A,
    B,
}

impl DeckId {
    pub const ALL: [DeckId; 2] = [DeckId::A, DeckId::B];

    pub fn index(self) -> usize {
        match self {
            DeckId::A => 0,
            DeckId::B => 1,
        }
    }

    pub fn from_index(i: usize) -> DeckId {
        if i == 0 {
            DeckId::A
        } else {
            DeckId::B
        }
    }

    pub fn other(self) -> DeckId {
        match self {
            DeckId::A => DeckId::B,
            DeckId::B => DeckId::A,
        }
    }

    pub fn crossfader_side(self) -> f32 {
        match self {
            DeckId::A => 0.0,
            DeckId::B => 1.0,
        }
    }
}

impl std::fmt::Display for DeckId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeckId::A => write!(f, "A"),
            DeckId::B => write!(f, "B"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckState {
    Empty,
    Loaded,
    Playing,
    Paused,
    Finished,
}

pub fn crossfade_gains(position: f32) -> (f32, f32) {
    let p = position.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (p.cos(), p.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossfade_edges() {
        let (a, b) = crossfade_gains(0.0);
        assert!((a - 1.0).abs() < 1e-6 && b.abs() < 1e-6);
        let (a, b) = crossfade_gains(1.0);
        assert!(a.abs() < 1e-6 && (b - 1.0).abs() < 1e-6);
    }

    #[test]
    fn crossfade_equal_power() {
        for i in 0..=10 {
            let (a, b) = crossfade_gains(i as f32 / 10.0);
            assert!((a * a + b * b - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn deck_ids() {
        assert_eq!(DeckId::A.other(), DeckId::B);
        assert_eq!(DeckId::from_index(DeckId::B.index()), DeckId::B);
    }
}
