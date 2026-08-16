//! Output device enumeration and a minimal `cpal` wrapper.
//!
//! Gated behind the `output` feature — CI has no audio device, so building
//! `mimir-audio` without that feature produces no system-library deps.

use cpal::traits::{DeviceTrait, HostTrait};

/// What we surface from an audio output device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDeviceInfo {
    pub name: String,
    pub is_default: bool,
}

/// List the output devices on the default host.
///
/// `Ok` even when no devices are present — an empty Vec means the host
/// is reachable but has no outputs (e.g. headless CI). `Err` only when the
/// host itself can't be queried (e.g. sandbox blocks alsa/pipewire).
pub fn list_output_devices() -> Result<Vec<OutputDeviceInfo>, cpal::HostUnavailable> {
    let host = cpal::default_host();
    let default = host
        .default_output_device()
        .map(|d| d.name().unwrap_or_default());

    let mut out = Vec::new();
    for device in host.output_devices().map_err(cpal::HostUnavailable::from)? {
        let name = device.name().unwrap_or_default();
        let is_default = Some(name.as_str()) == default.as_deref();
        out.push(OutputDeviceInfo { name, is_default });
    }
    Ok(out)
}
