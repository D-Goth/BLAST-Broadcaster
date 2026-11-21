#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeckId {
    A,
    B,
    C,
    D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckState {
    Empty,
    Loaded,
    Playing,
    Paused,
}

#[derive(Debug, Clone)]
pub struct Deck {
    pub id: DeckId,
    pub state: DeckState,
    pub gain: f32,
    pub cue: bool,
}

impl Deck {
    pub fn new(id: DeckId) -> Self {
        Self {
            id,
            state: DeckState::Empty,
            gain: 1.0,
            cue: false,
        }
    }
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
    fn new_deck_is_empty() {
        let d = Deck::new(DeckId::A);
        assert_eq!(d.state, DeckState::Empty);
    }
}
