//! Synthetic videos for tests, made by the FFmpeg command line from `.deps` (`cargo xtask deps`).
//! `testsrc2` draws a different picture in every frame, so a frame can be told by its content.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Runs `ffmpeg <args> <output>` (`args` split at spaces) and fails the test with its log if it
/// fails.
pub(crate) fn ffmpeg(args: &str, output: &Path) {
    let exe = Path::new(env!("FFMPEG_DIR"))
        .join("bin")
        .join(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX));
    let result = Command::new(exe)
        .args(["-v", "error", "-y"])
        .args(args.split(' '))
        .arg(output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "ffmpeg {args}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

/// A 320×240 `testsrc2` video `secs` long at `fps` with a keyframe every `gop` frames, encoded by
/// `codec` (`libx264`, `libvpx`, …) into `path` (the extension picks the container).
pub(crate) fn testsrc(path: &Path, codec: &str, fps: u32, gop: u32, secs: f64) {
    ffmpeg(
        &format!(
            "-f lavfi -i testsrc2=size=320x240:rate={fps} -t {secs} -c:v {codec} -g {gop} -pix_fmt yuv420p"
        ),
        path,
    );
}

/// 30 frames at 30 fps, then 30 frames at 15 fps: 3 s with a variable rate.
pub(crate) fn variable_rate(path: &Path) {
    ffmpeg(
        "-f lavfi -i testsrc2=size=320x240:rate=30 -frames:v 60 -vf setpts='if(lt(N,30),N/30,1+(N-30)/15)/TB' -fps_mode passthrough -c:v libx264 -g 30",
        path,
    );
}

/// The recording of `fixtures/recordings/speech.dcproj` (Git LFS): 1280×720, 60 fps slots with
/// dropped frames, a keyframe every second.
pub(crate) fn speech_video() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings/speech.dcproj/media/recording-1/video.mp4")
}
