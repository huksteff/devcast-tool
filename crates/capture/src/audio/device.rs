//! Sound devices through cpal: the list of microphones and input streams that deliver stereo
//! chunks stamped on the host clock.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};

use crate::error::CaptureError;
use crate::event::HostTime;

/// Channels of every recorded track: a mono device is doubled, of a surround one the front
/// left and right are kept.
pub const CHANNELS: usize = 2;

/// What a sound track records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioSource {
    /// What the default output device plays (WASAPI loopback); follows the default device.
    System,
    /// The input device with this name.
    Microphone(String),
}

/// A microphone the user can pick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MicrophoneInfo {
    /// Name as the system shows it; [`crate::RecordingConfig::microphone`] stores it.
    pub name: String,
    /// The system's default input device.
    pub default: bool,
}

/// Input devices in system order.
pub fn list_microphones() -> Result<Vec<MicrophoneInfo>, CaptureError> {
    let host = cpal::default_host();
    let default = host.default_input_device().map(|d| name(&d));
    let devices = host
        .input_devices()
        .map_err(|e| CaptureError::audio("получить список микрофонов", e))?;
    Ok(devices
        .map(|device| {
            let name = name(&device);
            MicrophoneInfo {
                default: default.as_ref() == Some(&name),
                name,
            }
        })
        .collect())
}

/// An input device ready to stream.
pub struct Device {
    device: cpal::Device,
    config: cpal::StreamConfig,
}

impl Device {
    /// The device `source` names now: for system sound the current default output.
    pub fn find(source: &AudioSource) -> Result<Self, CaptureError> {
        let host = cpal::default_host();
        let loopback = *source == AudioSource::System;
        let device = match source {
            AudioSource::System => host
                .default_output_device()
                .ok_or(CaptureError::NoOutputDevice)?,
            AudioSource::Microphone(wanted) => host
                .input_devices()
                .map_err(|e| CaptureError::audio("получить список микрофонов", e))?
                .find(|d| name(d) == *wanted)
                .ok_or_else(|| CaptureError::MicrophoneNotFound(wanted.clone()))?,
        };
        let label = name(&device);
        let supported = if loopback {
            device.default_output_config()
        } else {
            device.default_input_config()
        }
        .map_err(|e| CaptureError::audio_device(&label, e))?;
        if supported.sample_format() != cpal::SampleFormat::F32 {
            return Err(CaptureError::AudioFormat {
                device: label,
                format: supported.sample_format().to_string(),
            });
        }
        Ok(Self {
            device,
            config: supported.config(),
        })
    }

    pub fn name(&self) -> String {
        name(&self.device)
    }

    pub fn rate(&self) -> u32 {
        self.config.sample_rate
    }

    /// Samples per frame the device delivers.
    pub fn channels(&self) -> usize {
        usize::from(self.config.channels)
    }

    /// Starts streaming: `data` gets each packet (interleaved, [`Self::channels`] per frame) with
    /// the host time of its first sample, `error` what goes wrong with the stream.
    pub fn start(
        &self,
        mut data: impl FnMut(HostTime, &[f32]) + Send + 'static,
        error: impl FnMut(cpal::Error) + Send + 'static,
    ) -> Result<cpal::Stream, CaptureError> {
        let failed = |e| CaptureError::audio_device(&self.name(), e);
        let stream = self
            .device
            .build_input_stream(
                self.config,
                move |samples: &[f32], info: &cpal::InputCallbackInfo| {
                    data(host_time(info.timestamp().capture), samples);
                },
                error,
                None,
            )
            .map_err(failed)?;
        stream.play().map_err(failed)?;
        Ok(stream)
    }
}

fn name(device: &cpal::Device) -> String {
    device
        .description()
        .map_or_else(|_| device.to_string(), |d| d.name().to_owned())
}

/// cpal stamps WASAPI packets with the QPC time of their first sample, in nanoseconds.
fn host_time(instant: cpal::StreamInstant) -> HostTime {
    HostTime::from_micros((instant.as_nanos() / 1000) as i64)
}

/// Appends `data` (`channels` per frame) to `out` as stereo.
pub fn to_stereo(data: &[f32], channels: usize, out: &mut Vec<f32>) {
    for frame in data.chunks_exact(channels) {
        match frame {
            [mono] => out.extend_from_slice(&[*mono, *mono]),
            [left, right, ..] => out.extend_from_slice(&[*left, *right]),
            [] => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_device_becomes_stereo() {
        let mut out = Vec::new();
        to_stereo(&[0.1, 0.2], 1, &mut out);
        assert_eq!(out, [0.1, 0.1, 0.2, 0.2]);
        out.clear();
        to_stereo(&[0.1, 0.2, 0.3, 0.4], 2, &mut out);
        assert_eq!(out, [0.1, 0.2, 0.3, 0.4]);
        out.clear();
        to_stereo(&[0.1, 0.2, 0.5, 0.6, 0.7, 0.8], 6, &mut out);
        assert_eq!(out, [0.1, 0.2]);
    }

    #[test]
    fn stream_instants_are_host_microseconds() {
        let instant = cpal::StreamInstant::new(12, 345_678_900);
        assert_eq!(host_time(instant), HostTime::from_micros(12_345_678));
    }
}
