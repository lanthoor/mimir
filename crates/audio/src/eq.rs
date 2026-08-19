//! Parametric EQ biquad cascade.
//!
//! Five peaking biquad filters at fixed frequencies (60 / 250 / 1k / 4k / 16k Hz),
//! each driven by a gain in dB. Bandwidth defaults to 1 octave.
//!
//! Standard biquad transfer function coefficients via the Audio EQ Cookbook
//! (RBJ Audio EQ Cookbook, "`PeakingEQ`"). The cascade is
//! direct-form-I on interleaved f32 PCM samples.
//!
//! ponytail: 5 bands at fixed frequencies is the cheap default. Phase 0
//! users get a no-op EQ; tier-2C's contract is "the engine has the hook".
//! Per-band parametric frequency + Q + shelf types is the next iteration.

use std::f32::consts::PI;

/// One band's settings. `gain_db == 0` is a transparent passthrough.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqBand {
    pub freq_hz: f32,
    pub gain_db: f32,
}

/// The cascade's persistent state. Direct-form-I keeps per-section
/// (`z1`, `z2`) delays so consecutive frames produce continuous output.
#[derive(Debug, Clone)]
pub struct EqState {
    coeffs: Vec<BiquadCoeffs>,
    /// `true` when the cascade is a true identity (every band's gain is
    /// zero). Cheaper than multiplying per-sample.
    bypass: bool,
    z1: Vec<f32>,
    z2: Vec<f32>,
}

#[derive(Debug, Clone, Copy)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl EqState {
    /// Build a 5-band cascade from per-band gains at the standard
    /// frequencies (60, 250, 1000, 4000, 16000 Hz). Sample rate is in Hz.
    pub fn new_5band(gains_db: [f32; 5], sample_rate: f32) -> Self {
        const FREQS: [f32; 5] = [60.0, 250.0, 1_000.0, 4_000.0, 16_000.0];
        let bypass = gains_db.iter().all(|g| g.abs() < 1e-3);
        let coeffs = FREQS
            .iter()
            .zip(gains_db.iter())
            .map(|(&f, &g)| peaking_biquad(f, g, 1.0, sample_rate))
            .collect::<Vec<_>>();
        let n = coeffs.len();
        Self {
            coeffs,
            bypass,
            z1: vec![0.0; n],
            z2: vec![0.0; n],
        }
    }

    /// True when every band is at unity (`|gain_db| < 1e-3`).
    pub fn is_passthrough(&self) -> bool {
        self.bypass
    }

    /// Process interleaved `f32` samples in place.
    pub fn process(&mut self, samples: &mut [f32]) {
        if self.is_passthrough() {
            return;
        }
        mimir_telemetry::log(
            "DEBUG",
            "audio.eq",
            &format!("process n={} bands={}", samples.len(), self.coeffs.len()),
        );
        for sample in samples.iter_mut() {
            let mut x = *sample;
            for (i, c) in self.coeffs.iter().enumerate() {
                let y = c.b0 * x + self.z1[i];
                self.z1[i] = c.b1 * x - c.a1 * y + self.z2[i];
                self.z2[i] = c.b2 * x - c.a2 * y;
                x = y;
            }
            *sample = x.clamp(-1.0, 1.0);
        }
    }
}

/// RBJ `PeakingEQ` biquad coefficients.
fn peaking_biquad(freq: f32, gain_db: f32, q: f32, sample_rate: f32) -> BiquadCoeffs {
    let a = 10f32.powf(gain_db / 40.0);
    let omega = 2.0 * PI * freq / sample_rate;
    let alpha = omega.sin() / (2.0 * q);

    let cos_w = omega.cos();
    let b0 = 1.0 + alpha * a;
    let b1 = -2.0 * cos_w;
    let b2 = 1.0 - alpha * a;
    let a0 = 1.0 + alpha / a;
    let a1 = -2.0 * cos_w;
    let a2 = 1.0 - alpha / a;

    BiquadCoeffs {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}
