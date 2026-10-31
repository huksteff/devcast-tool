//! The default output device of Windows playing a [`Line`] through cpal (WASAPI). The stream
//! is opened at [`RATE`] and [`CHANNELS`] in `f32`: WASAPI output converts to the device's own
//! format (`AUTOCONVERTPCM`, cpal 0.18), so the mix needs no resampling here.

use std::sync::Arc;
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::error::AudioError;
use crate::play::{Line, Off};
use crate::{CHANNELS, RATE};

/// A playing output stream; dropping it stops the sound.
pub struct Output {
    _stream: cpal::Stream,
}

impl std::fmt::Debug for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Output").finish_non_exhaustive()
    }
}

impl Output {
    /// Plays `line` on the default output device. A failure of the running stream (the device
    /// was unplugged or changed) turns the line off with the reason; an xrun only skips sound.
    pub fn open(line: Arc<Line>) -> Result<Self, AudioError> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or(AudioError::NoOutputDevice)?;
        let name = device
            .description()
            .map_or_else(|_| device.to_string(), |d| d.name().to_owned());
        let failed = |e: cpal::Error| AudioError::Output {
            device: name.clone(),
            message: e.to_string(),
        };
        let config = cpal::StreamConfig {
            channels: CHANNELS as cpal::ChannelCount,
            sample_rate: RATE,
            buffer_size: cpal::BufferSize::Default,
        };
        let sink = Arc::clone(&line);
        let label = name.clone();
        let stream = device
            .build_output_stream(
                config,
                move |out: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    let stamp = info.timestamp();
                    let latency = stamp.playback.duration_since(stamp.callback);
                    sink.pull(out, Instant::now() + latency);
                },
                move |error: cpal::Error| {
                    if error.kind() != cpal::ErrorKind::Xrun {
                        let message = AudioError::Output {
                            device: label.clone(),
                            message: error.to_string(),
                        };
                        line.turn_off(Off::Failed(message.to_string()));
                    }
                },
                None,
            )
            .map_err(failed)?;
        stream.play().map_err(failed)?;
        Ok(Self { _stream: stream })
    }
}
