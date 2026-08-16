//! Tests for the audio decoder + transport.

use std::fs;
use std::path::Path;

use crate::decode_file;
use crate::transport::{PlaybackQueue, Transport, TransportState, TransportCommand};
#[cfg(feature = "output")]
use crate::{Player, PlayerCommand, PlayerSnapshot};

/// Write a tiny mono 8kHz 16-bit PCM WAV file with `n` samples.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
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
        let sample = (phase.sin() * 16_000.0).clamp(-32_768.0, 32_767.0) as i16;
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
    assert_eq!(
        buf.samples.len(),
        n as usize,
        "decoded sample count mismatch"
    );

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

#[test]
fn transport_state_transitions_match_spec() {
    // Stopped → Playing → Paused → Playing → Stopped
    assert_eq!(TransportState::Stopped, TransportState::default());

    let s = TransportState::Stopped.play();
    assert_eq!(s, TransportState::Playing);

    let s = s.pause();
    assert_eq!(s, TransportState::Paused);

    let s = s.resume();
    assert_eq!(s, TransportState::Playing);

    let s = s.stop();
    assert_eq!(s, TransportState::Stopped);
}

#[test]
fn transport_state_illegal_transitions_are_noops() {
    // Pausing when already stopped stays stopped.
    assert_eq!(TransportState::Stopped.pause(), TransportState::Stopped);
    // Resuming when not paused is a noop.
    assert_eq!(TransportState::Stopped.resume(), TransportState::Stopped);
    assert_eq!(TransportState::Playing.resume(), TransportState::Playing);
    // Stopping when already stopped is a noop.
    assert_eq!(TransportState::Stopped.stop(), TransportState::Stopped);
    // Playing when already playing is a noop.
    assert_eq!(TransportState::Playing.play(), TransportState::Playing);
}

#[test]
fn queue_push_clear_and_next_prev() {
    let mut q = PlaybackQueue::new();
    q.push(10);
    q.push(20);
    q.push(30);
    assert_eq!(q.items(), &[10, 20, 30]);
    assert_eq!(q.current(), Some(10));

    assert_eq!(q.next(), Some(20));
    assert_eq!(q.next(), Some(30));
    assert_eq!(q.next(), None, "end of queue");

    assert_eq!(q.previous(), Some(20));

    q.clear();
    assert!(q.is_empty());
    assert_eq!(q.current(), None);
}

#[test]
fn transport_dispatches_commands() {
    let mut t = Transport::default();
    assert_eq!(t.state, TransportState::Stopped);

    // Build a queue first, then play.
    t.dispatch(TransportCommand::Enqueue(10));
    t.dispatch(TransportCommand::Enqueue(20));
    t.dispatch(TransportCommand::Enqueue(30));
    assert_eq!(t.queue.items(), &[10, 20, 30]);
    assert_eq!(t.queue.current(), Some(10));

    t.dispatch(TransportCommand::Play(10));
    // Play replaces the queue with [10].
    assert_eq!(t.queue.items(), &[10]);
    assert_eq!(t.queue.current(), Some(10));
    assert_eq!(t.state, TransportState::Playing);

    // Now extend the queue from the playing position.
    t.dispatch(TransportCommand::Enqueue(20));
    t.dispatch(TransportCommand::Enqueue(30));
    assert_eq!(t.queue.items(), &[10, 20, 30]);

    t.dispatch(TransportCommand::Pause);
    assert_eq!(t.state, TransportState::Paused);

    t.dispatch(TransportCommand::Resume);
    assert_eq!(t.state, TransportState::Playing);

    t.dispatch(TransportCommand::Next);
    assert_eq!(t.queue.current(), Some(20));

    t.dispatch(TransportCommand::Previous);
    assert_eq!(t.queue.current(), Some(10));

    t.dispatch(TransportCommand::ClearQueue);
    assert!(t.queue.is_empty());
    assert_eq!(t.state, TransportState::Stopped, "clear also stops");
}

#[test]
fn transport_play_replaces_queue_with_single_track() {
    let mut t = Transport::default();
    t.dispatch(TransportCommand::Enqueue(10));
    t.dispatch(TransportCommand::Enqueue(20));
    t.dispatch(TransportCommand::Play(99));
    assert_eq!(t.queue.items(), &[99]);
    assert_eq!(t.queue.current(), Some(99));
    assert_eq!(t.state, TransportState::Playing);
}

#[test]
#[cfg(feature = "output")]
fn output_lists_default_host() {
    // The CI runner may have no audio devices (sandbox blocks alsa/pipewire,
    // or libasound2-dev is missing). We only assert the *shape* of the
    // return type — both `Ok(empty Vec)` and `Err(_)` are accepted.
    let result: Result<Vec<crate::output::OutputDeviceInfo>, String> =
        crate::output::list_output_devices();
    match result {
        Ok(devices) => {
            // The default marker should be set on at most one device, and
            // only when the list is non-empty.
            let defaults = devices.iter().filter(|d| d.is_default).count();
            if !devices.is_empty() {
                assert!(defaults <= 1, "at most one device can be default");
            }
        }
        Err(e) => {
            eprintln!("output device enumeration skipped: {e}");
        }
    }
}

#[cfg(feature = "output")]
#[test]
fn player_new_starts_in_stopped_state() {
    let p = Player::new();
    assert_eq!(p.snapshot(), PlayerSnapshot::default());
    assert_eq!(p.snapshot().state, TransportState::Stopped);
    assert!(p.snapshot().current.is_none());
}
