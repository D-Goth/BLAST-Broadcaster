use mp3lame_encoder::{Bitrate, Builder, InterleavedPcm, Mode, MonoPcm, Quality};

// [CAST-06]
pub struct Mp3Encoder {
    inner: mp3lame_encoder::Encoder,
    mono: bool,
    scratch: Vec<f32>,
    out: Vec<u8>,
}

// [CAST-07]
fn bitrate(kbps: u32) -> Bitrate {
    match kbps {
        0..=32 => Bitrate::Kbps32,
        33..=40 => Bitrate::Kbps40,
        41..=48 => Bitrate::Kbps48,
        49..=64 => Bitrate::Kbps64,
        65..=80 => Bitrate::Kbps80,
        81..=96 => Bitrate::Kbps96,
        97..=112 => Bitrate::Kbps112,
        113..=128 => Bitrate::Kbps128,
        129..=160 => Bitrate::Kbps160,
        161..=192 => Bitrate::Kbps192,
        193..=224 => Bitrate::Kbps224,
        225..=256 => Bitrate::Kbps256,
        _ => Bitrate::Kbps320,
    }
}

impl Mp3Encoder {
    pub fn new(sample_rate: u32, kbps: u32, mono: bool) -> Result<Self, String> {
        let mut b = Builder::new().ok_or("encodeur MP3 indisponible")?;
        b.set_num_channels(if mono { 1 } else { 2 })
            .map_err(|e| format!("canaux : {e:?}"))?;
        b.set_sample_rate(sample_rate)
            .map_err(|e| format!("fréquence {sample_rate} Hz : {e:?}"))?;
        b.set_brate(bitrate(kbps))
            .map_err(|e| format!("débit : {e:?}"))?;
        b.set_mode(if mono { Mode::Mono } else { Mode::JointStereo })
            .map_err(|e| format!("mode : {e:?}"))?;
        b.set_quality(Quality::Good)
            .map_err(|e| format!("qualité : {e:?}"))?;
        let inner = b.build().map_err(|e| format!("encodeur MP3 : {e:?}"))?;
        Ok(Self {
            inner,
            mono,
            scratch: Vec::new(),
            out: Vec::new(),
        })
    }

    // [CAST-08]
    pub fn encode(&mut self, interleaved: &[f32]) -> Result<&[u8], String> {
        let frames = interleaved.len() / 2;
        self.out.clear();
        self.out
            .reserve(mp3lame_encoder::max_required_buffer_size(frames.max(1)));
        let result = if self.mono {
            self.scratch.clear();
            self.scratch
                .extend(interleaved.chunks_exact(2).map(|f| (f[0] + f[1]) * 0.5));
            self.inner
                .encode_to_vec(MonoPcm(&self.scratch[..]), &mut self.out)
        } else {
            self.inner
                .encode_to_vec(InterleavedPcm(&interleaved[..frames * 2]), &mut self.out)
        };
        result.map_err(|e| format!("encodage MP3 : {e:?}"))?;
        Ok(&self.out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frames: usize, rate: u32) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let s = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / rate as f32).sin() * 0.3;
                [s, s]
            })
            .collect()
    }

    #[test]
    fn produces_mp3_frames() {
        for (rate, mono) in [(44_100, false), (48_000, false), (44_100, true)] {
            let mut enc = Mp3Encoder::new(rate, 128, mono).unwrap();
            let pcm = sine(rate as usize, rate);
            let mut total = Vec::new();
            for chunk in pcm.chunks(2048) {
                total.extend_from_slice(enc.encode(chunk).unwrap());
            }
            assert!(total.len() > 10_000, "{} octets", total.len());
            let sync = total
                .windows(2)
                .any(|w| w[0] == 0xFF && (w[1] & 0xE0) == 0xE0);
            assert!(sync, "pas d'en-tête de trame MP3");
        }
    }
}
