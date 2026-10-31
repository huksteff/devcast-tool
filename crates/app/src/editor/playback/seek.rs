//! Where the playhead jumps (docs/porting/editor-ux.md §1.5): a frame back or forward at the
//! project's frame rate, to the previous or next keyframe or ±5 s when there is none.

use dc_core::{OutputTime, Project, TimeDelta};
use dc_project::ProjectDir;

/// Frame rate of a frame step when no clip's video can be probed (v1: the export's default).
pub const FALLBACK_FPS: f64 = 30.0;

/// Frames per second of a frame step: the highest of the clips' videos, rounded (a recording's
/// variable rate gives 59.8). A video that cannot be read is left out: the preview shows why.
pub fn frame_rate(dir: &ProjectDir, project: &Project) -> f64 {
    project
        .clips
        .iter()
        .filter_map(|clip| dc_media::cached_probe(dir, &clip.media.path).ok())
        .map(|probe| probe.fps.round())
        .filter(|fps| *fps >= 1.0)
        .reduce(f64::max)
        .unwrap_or(FALLBACK_FPS)
}

/// A key closer than this to the playhead is the one it stands on: the jump goes past it (v1).
const NEAR: TimeDelta = TimeDelta::from_micros(50_000);
/// The jump without a key that way (v1).
const LEAP: TimeDelta = TimeDelta::from_micros(5_000_000);

/// The frame `frames` frames from the one shown at `at` (`frames` < 0 is back), at `fps`
/// frames per second of output: `round(at · fps) + frames`, clamped to `0..=end`.
pub fn step(at: OutputTime, frames: i64, fps: f64, end: OutputTime) -> OutputTime {
    let index = (at.micros() as f64 * fps / 1e6).round() as i64 + frames;
    let micros = (index as f64 * 1e6 / fps).round() as i64;
    clamp(micros, end)
}

/// The nearest key of `keys` (sorted output times) before `at` (or after it when `forward`) and
/// farther than 50 ms; without one, 5 s that way; clamped to `0..=end`.
pub fn jump(at: OutputTime, forward: bool, keys: &[OutputTime], end: OutputTime) -> OutputTime {
    let micros = if forward {
        keys.iter()
            .find(|k| k.micros() > at.micros() + NEAR.micros())
            .map_or(at.micros() + LEAP.micros(), |k| k.micros())
    } else {
        keys.iter()
            .rev()
            .find(|k| k.micros() < at.micros() - NEAR.micros())
            .map_or(at.micros() - LEAP.micros(), |k| k.micros())
    };
    clamp(micros, end)
}

fn clamp(micros: i64, end: OutputTime) -> OutputTime {
    OutputTime::from_micros(micros.clamp(0, end.micros().max(0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: f64) -> OutputTime {
        OutputTime::from_secs_f64(secs)
    }

    const END: f64 = 20.0;

    #[test]
    fn a_frame_step_lands_on_the_frame_grid() {
        let end = at(END);
        assert_eq!(step(at(1.0), 1, 60.0, end).micros(), 1_016_667);
        assert_eq!(step(at(1.0), -1, 30.0, end).micros(), 966_667);
        assert_eq!(
            step(at(1.004), 1, 60.0, end).micros(),
            1_016_667,
            "from between frames to the next one of the grid"
        );
        assert_eq!(step(at(1.0), 0, 60.0, end), at(1.0));
    }

    #[test]
    fn a_frame_step_stays_inside_the_project() {
        let end = at(END);
        assert_eq!(step(at(0.0), -1, 60.0, end), at(0.0));
        assert_eq!(step(end, 1, 60.0, end), end);
    }

    #[test]
    fn a_jump_goes_to_the_nearest_key_past_50_ms() {
        let keys = [at(2.0), at(4.0), at(9.0)];
        let end = at(END);
        assert_eq!(jump(at(3.0), true, &keys, end), at(4.0));
        assert_eq!(jump(at(3.0), false, &keys, end), at(2.0));
        assert_eq!(
            jump(at(4.03), false, &keys, end),
            at(2.0),
            "standing on a key: the one before it"
        );
        assert_eq!(jump(at(3.96), true, &keys, end), at(9.0));
    }

    #[test]
    fn without_a_key_that_way_it_leaps_5_s_inside_the_project() {
        let keys = [at(2.0)];
        let end = at(END);
        assert_eq!(jump(at(10.0), true, &keys, end), at(15.0));
        assert_eq!(jump(at(17.0), true, &keys, end), end);
        assert_eq!(jump(at(1.0), false, &keys, end), at(0.0));
        assert_eq!(jump(at(9.0), false, &[], end), at(4.0));
    }
}
