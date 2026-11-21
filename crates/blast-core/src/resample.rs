use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

const CHUNK: usize = 1024;

pub fn resample_stereo(interleaved: &[f32], from: u32, to: u32) -> Result<Vec<f32>, String> {
    if from == to || interleaved.is_empty() {
        return Ok(interleaved.to_vec());
    }
    let frames = interleaved.len() / 2;
    let ratio = to as f64 / from as f64;
    let expected = (frames as f64 * ratio).round() as usize;

    let mut left = Vec::with_capacity(frames);
    let mut right = Vec::with_capacity(frames);
    for f in 0..frames {
        left.push(interleaved[2 * f]);
        right.push(interleaved[2 * f + 1]);
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let mut rs =
        SincFixedIn::<f32>::new(ratio, 1.0, params, CHUNK, 2).map_err(|e| e.to_string())?;
    let delay = rs.output_delay();

    let mut out_l: Vec<f32> = Vec::with_capacity(expected + delay + CHUNK * 2);
    let mut out_r: Vec<f32> = Vec::with_capacity(expected + delay + CHUNK * 2);

    let mut pos = 0;
    while pos + CHUNK <= frames {
        let input = [&left[pos..pos + CHUNK], &right[pos..pos + CHUNK]];
        let o = rs.process(&input, None).map_err(|e| e.to_string())?;
        out_l.extend_from_slice(&o[0]);
        out_r.extend_from_slice(&o[1]);
        pos += CHUNK;
    }
    if pos < frames {
        let input = [&left[pos..], &right[pos..]];
        let o = rs
            .process_partial(Some(&input), None)
            .map_err(|e| e.to_string())?;
        out_l.extend_from_slice(&o[0]);
        out_r.extend_from_slice(&o[1]);
    }
    let mut guard = 0;
    while out_l.len() < expected + delay && guard < 64 {
        let o = rs
            .process_partial(None::<&[Vec<f32>]>, None)
            .map_err(|e| e.to_string())?;
        out_l.extend_from_slice(&o[0]);
        out_r.extend_from_slice(&o[1]);
        guard += 1;
    }

    let mut out = Vec::with_capacity(expected * 2);
    for i in 0..expected {
        let j = i + delay;
        out.push(out_l.get(j).copied().unwrap_or(0.0));
        out.push(out_r.get(j).copied().unwrap_or(0.0));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_follows_ratio() {
        let frames = 44_100;
        let input: Vec<f32> = (0..frames * 2)
            .map(|i| ((i / 2) as f32 * 0.05).sin() * 0.5)
            .collect();
        let out = resample_stereo(&input, 44_100, 48_000).unwrap();
        assert_eq!(out.len() / 2, 48_000);
    }

    #[test]
    fn keeps_amplitude() {
        let frames = 44_100;
        let freq = 440.0;
        let input: Vec<f32> = (0..frames)
            .flat_map(|i| {
                let s = (2.0 * std::f32::consts::PI * freq * i as f32 / 44_100.0).sin() * 0.5;
                [s, s]
            })
            .collect();
        let out = resample_stereo(&input, 44_100, 48_000).unwrap();
        let mid = &out[20_000..60_000];
        let peak = mid.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
        assert!((peak - 0.5).abs() < 0.02, "peak {peak}");
    }
}
