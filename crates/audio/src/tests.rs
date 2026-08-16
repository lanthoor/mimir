//! Tests for the audio decoder.

use std::fs;
use std::path::Path;

use crate::decode_file;

/// Write a tiny mono 8kHz 16-bit PCM WAV file with `n` samples.
fn write_sine_wav(path: &Path, n: u32, sample_rate: u32) {
    let mut data = Vec::with_capacity(44 + (n as usize) * 2);
    // RIFF header.
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36u32 + n * 2).to_le_bytes());
    data.extend_from_slice(b"WAVE");
    // fmt chunk.
    data.extend_from_slice(b"fmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes()); // PCM
    data.extend_from_slice(&1u16.to_le_bytes()); // mono
    data.extend_from_slice(&sample_rate.to_le_bytes());
    data.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    data.extend_from_slice(&2u16.to_le_bytes()); // block align
    data.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    // data chunk.
    data.extend_from_slice(b"data");
    data.extend_from_slice(&(n * 2).to_le_bytes());
    for i in 0..n {
        // 200 Hz sine at the given sample rate.
        let phase = (i as f32) * 200.0 * 2.0 * std::f32::consts::PI / (sample_rate as f32);
        let sample = (phase.sin() * 16_000.0) as i16;
        data.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(path, &data).expect("write wav");
}

#[test]
fn decode_file_reads_wav_into_interleaved_pcm() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sine.wav");
    let n = 800u32;
    let sr = 8_000u32;
    write_sine_wav(&path, n, sr);

    let buf = decode_file(&path).expect("decode");
    assert_eq!(buf.sample_rate, sr);
    assert_eq!(buf.channels, 1);
    assert_eq!(buf.samples.len(), n as usize, "decoded sample count mismatch");

    // The first sample of a 200 Hz sine at 8 kHz starts near zero.
    assert!(buf.samples[0].abs() < 0.01, "first sample should be ~0");
}

#[test]
fn decode_file_rejects_unknown_extension() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("not-audio.xyz");
    fs::write(&path, b"junk").expect("write");
    assert!(decode_file(&path).is_err());
}
