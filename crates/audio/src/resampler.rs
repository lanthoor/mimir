//! Sample-rate conversion for interleaved f32 PCM via `rubato`.
//!
//! ponytail: chunked synchronous rubato — adequate for desktop audio with
//! a moderately sized chunk. Real-time resampling inside the cpal callback
//! would need a careful ring-buffer + per-callback slice; defer until
//! playback latency becomes a complaint.

use mimir_telemetry as telemetry;
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType};

/// Convert `interleaved` PCM from `in_rate` Hz to `out_rate` Hz.
///
/// `interleaved` is laid out as `[L0, R0, L1, R1, …]`. `channels` is 1
/// (mono) or 2 (stereo); other counts return a copy unchanged.
///
/// 0-length input → 0-length output. `in_rate == out_rate` → identity.
pub fn resample_interleaved(
    interleaved: &[f32],
    channels: u16,
    in_rate: u32,
    out_rate: u32,
) -> Vec<f32> {
    if interleaved.is_empty() || channels == 0 {
        return Vec::new();
    }
    if in_rate == out_rate {
        return interleaved.to_vec();
    }
    if channels != 1 && channels != 2 {
        return interleaved.to_vec();
    }

    let channels_us = channels as usize;
    let ratio = f64::from(out_rate) / f64::from(in_rate);
    let chunk_in: usize = 4_096;

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        oversampling_factor: 256,
        interpolation: SincInterpolationType::Cubic,
        window: rubato::WindowFunction::BlackmanHarris2,
    };

    let mut sinc = match SincFixedIn::<f32>::new(ratio, 1.0, params, chunk_in, channels_us) {
        Ok(s) => s,
        Err(e) => {
            telemetry::log(
                "ERROR",
                "audio.resampler",
                &format!(
                    "SincFixedIn init failed in={in_rate} out={out_rate} ch={channels} err={e:?}"
                ),
            );
            return interleaved.to_vec();
        }
    };

    let total = interleaved.len();
    let frames = total / channels_us;
    let out_capacity = (frames * out_rate as usize) / (in_rate as usize).max(1) + 64;
    let mut out: Vec<f32> = Vec::with_capacity(out_capacity);

    // Deinterleave the input.
    let mut planar_in: Vec<Vec<f32>> = (0..channels_us)
        .map(|_| Vec::with_capacity(frames))
        .collect();
    for frame in interleaved.chunks_exact(channels_us) {
        for (c, s) in frame.iter().enumerate() {
            planar_in[c].push(*s);
        }
    }
    let total_frames = planar_in[0].len();

    let input_frames_next = sinc.input_frames_next();
    let mut frame_cursor = 0usize;

    while frame_cursor < total_frames {
        let take = (total_frames - frame_cursor).min(input_frames_next);

        // Build per-channel input slices each iteration. `owned_tails` keeps
        // the zero-padded Vecs alive across `process_into_buffer`.
        let mut owned_blocks: Vec<Vec<f32>> = Vec::with_capacity(channels_us);
        for ch in 0..channels_us {
            let mut block: Vec<f32> = planar_in[ch][frame_cursor..frame_cursor + take].to_vec();
            block.resize(chunk_in, 0.0);
            owned_blocks.push(block);
        }
        let input_slices: Vec<&[f32]> =
            owned_blocks.iter().map(|b| b.as_slice()).collect();
        let in_refs: &[&[f32]] = &input_slices;

        let frames_out = sinc.output_frames_next();
        let mut output_blocks: Vec<Vec<f32>> =
            (0..channels_us).map(|_| vec![0.0_f32; frames_out]).collect();

        // `process_into_buffer` wants `&mut [Vout]` where `Vout: AsMut<[T]>`.
        let mut out_refs: Vec<&mut Vec<f32>> = output_blocks.iter_mut().collect();
        let res = sinc.process_into_buffer(in_refs, &mut out_refs, None);
        drop(out_refs);

        match res {
            Ok((n_in, n_out)) => {
                frame_cursor += n_in.min(take);
                let n = n_out.min(frames_out);
                for f in 0..n {
                    for c in 0..channels_us {
                        let v = output_blocks
                            .get(c)
                            .and_then(|b| b.get(f))
                            .copied()
                            .unwrap_or(0.0);
                        out.push(v);
                    }
                }
            }
            Err(e) => {
                telemetry::log(
                    "ERROR",
                    "audio.resampler",
                    &format!("process_into_buffer failed err={e:?}; falling back to identity"),
                );
                return interleaved.to_vec();
            }
        }
    }

    telemetry::log(
        "DEBUG",
        "audio.resampler",
        &format!(
            "resample done in={in_rate} out={out_rate} ch={channels} in_frames={total_frames} out_frames={}",
            out.len() / channels_us
        ),
    );
    out
}
