//! Linear gain multiplication for `f32` PCM samples.
//!
//! ponytail: applied in-place over a single decode buffer; full-track
//! pre-amplification (or peaks limiting) would require peak/RMS analysis
//! first. For v1 we just do volume-from-RG-tags-and-user-preference.

/// Returns the multiplicative factor for `db` decibels (linear amplitude).
///
/// `db == 0.0` returns `1.0`. `db` is `f64` to match common `ReplayGain`
/// tag precision; the returned factor is `f32`.
pub fn db_to_linear(db: f64) -> f32 {
    let v = 10f64.powf(db / 20.0);
    let cap = f64::from(f32::MAX);
    let clamped = v.clamp(0.0, cap);
    #[allow(clippy::cast_possible_truncation)]
    let out = clamped as f32;
    out
}

/// Apply a gain (in dB) in-place to interleaved `f32` samples, clipping at
/// `[-1, 1]` to avoid wrap-around.
pub fn apply_gain_db_inplace(samples: &mut [f32], db: f64) {
    let g = db_to_linear(db);
    for s in samples.iter_mut() {
        let v = *s * g;
        *s = v.clamp(-1.0, 1.0);
    }
}
