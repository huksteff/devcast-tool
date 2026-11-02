//! The size of an MP4 before it is exported, for the dialog's footer «1:48 · ≈ 90 МБ».
//!
//! The codecs encode at constant quality (`dc_encode::Codec::options`: CQ/CRF 21, 26, 31), not
//! at a bit rate, so the size depends on how much moves on screen and no number is exact. The
//! guess is bits per pixel per frame by quality for a screen recording (code, UI, a moving
//! cursor): not measured on our exports yet, but set to the mockup's example (1:48 of 1080p60
//! high ≈ 90 МБ, ≈ 6.6 Mbit/s) with each quality step (5 CQ) about half the bits, as for
//! H.264. A video full of motion comes out larger, a still screen smaller; hence the «≈» and
//! the «?». Real exports may correct the table; the dialog's footer is its only user.

use dc_core::{PixelSize, TimeDelta};
use dc_encode::audio::AAC_BIT_RATE;
use dc_ops::export::Quality;

/// Rough video bits per pixel per frame of a screen recording at each quality (see above).
const fn bits_per_pixel(quality: Quality) -> f64 {
    match quality {
        Quality::High => 0.053,
        Quality::Medium => 0.03,
        Quality::Low => 0.017,
    }
}

/// The estimated bytes of an MP4 of `frame` at `fps` and `quality` lasting `duration`: the
/// guessed video plus the AAC sound at its fixed bit rate.
pub fn mp4_bytes(frame: PixelSize, fps: u32, quality: Quality, duration: TimeDelta) -> u64 {
    let seconds = duration.as_secs_f64().max(0.0);
    let pixels = f64::from(frame.width) * f64::from(frame.height) * f64::from(fps);
    let video = pixels * bits_per_pixel(quality);
    let bits = (video + AAC_BIT_RATE as f64) * seconds;
    (bits / 8.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_HD: PixelSize = PixelSize {
        width: 1920,
        height: 1080,
    };

    fn megabytes(bytes: u64) -> f64 {
        bytes as f64 / 1_048_576.0
    }

    #[test]
    fn the_mockup_case_is_about_90_megabytes() {
        let length = TimeDelta::from_micros(108_000_000);
        let high = megabytes(mp4_bytes(FULL_HD, 60, Quality::High, length));
        assert!((85.0..95.0).contains(&high), "{high}");
    }

    #[test]
    fn more_pixels_frames_quality_and_time_cost_more() {
        let minute = TimeDelta::from_micros(60_000_000);
        let base = mp4_bytes(FULL_HD, 60, Quality::Medium, minute);
        let uhd = PixelSize {
            width: 3840,
            height: 2160,
        };
        assert!(mp4_bytes(uhd, 60, Quality::Medium, minute) > base);
        assert!(mp4_bytes(FULL_HD, 30, Quality::Medium, minute) < base);
        assert!(mp4_bytes(FULL_HD, 60, Quality::High, minute) > base);
        assert!(mp4_bytes(FULL_HD, 60, Quality::Low, minute) < base);
        let two = TimeDelta::from_micros(120_000_000);
        assert!(mp4_bytes(FULL_HD, 60, Quality::Medium, two).abs_diff(2 * base) <= 1);
    }

    #[test]
    fn nothing_lasts_nothing() {
        assert_eq!(mp4_bytes(FULL_HD, 60, Quality::High, TimeDelta::ZERO), 0);
    }
}
