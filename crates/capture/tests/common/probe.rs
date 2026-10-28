//! What ffprobe says about a recorded file, and a frame of it as PNG.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn tool(name: &str) -> PathBuf {
    PathBuf::from(env!("FFMPEG_DIR"))
        .join("bin")
        .join(format!("{name}.exe"))
}

#[derive(Debug)]
pub struct Probe {
    pub codec: String,
    pub size: (u64, u64),
    pub time_base: String,
    pub color: [String; 4],
    pub duration: f64,
    /// Presentation times of the frames, seconds.
    pub times: Vec<f64>,
    /// Times of the key frames.
    pub keys: Vec<f64>,
}

impl Probe {
    pub fn read(path: &Path) -> Self {
        let output = Command::new(tool("ffprobe"))
            .args(["-v", "error", "-select_streams", "v:0", "-of", "json"])
            .args(["-show_entries", "stream=codec_name,width,height,time_base,color_range,color_space,color_primaries,color_transfer,duration:packet=pts_time,flags"])
            .arg(path)
            .output()
            .unwrap();
        assert!(output.status.success(), "ffprobe: {output:?}");
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        let stream = &json["streams"][0];
        let text = |key: &str| stream[key].as_str().unwrap_or("").to_owned();
        let number = |value: &Value| value.as_str().unwrap().parse::<f64>().unwrap();
        let packets = json["packets"].as_array().unwrap();
        let times: Vec<f64> = packets.iter().map(|p| number(&p["pts_time"])).collect();
        let keys = packets
            .iter()
            .filter(|p| p["flags"].as_str().unwrap().starts_with('K'))
            .map(|p| number(&p["pts_time"]))
            .collect();
        Self {
            codec: text("codec_name"),
            size: (
                stream["width"].as_u64().unwrap(),
                stream["height"].as_u64().unwrap(),
            ),
            time_base: text("time_base"),
            color: [
                text("color_range"),
                text("color_space"),
                text("color_primaries"),
                text("color_transfer"),
            ],
            duration: number(&stream["duration"]),
            times,
            keys,
        }
    }

    /// Checks what every recording must have: H.264, BT.709 tags, increasing times, a key
    /// frame at the start and then at most a second apart.
    pub fn check_common(&self) {
        assert_eq!(self.codec, "h264");
        assert_eq!(
            self.color,
            ["tv", "bt709", "bt709", "bt709"].map(String::from)
        );
        assert!(
            self.times.windows(2).all(|w| w[0] < w[1]),
            "times not increasing"
        );
        assert_eq!(self.keys.first(), Some(&0.0));
        let gaps: Vec<f64> = self.keys.windows(2).map(|w| w[1] - w[0]).collect();
        let longest = self
            .times
            .windows(2)
            .map(|w| w[1] - w[0])
            .fold(0.0, f64::max);
        // A key frame is the first frame a second or more after the previous key frame.
        assert!(
            gaps.iter().all(|g| *g <= 1.0 + longest + 1e-6),
            "key frame gaps {gaps:?}"
        );
    }

    pub fn frame_rate(&self) -> f64 {
        (self.times.len() - 1) as f64 / (self.times[self.times.len() - 1] - self.times[0])
    }
}

/// What ffprobe says about a recorded sound track.
#[derive(Debug)]
pub struct AudioProbe {
    pub codec: String,
    pub rate: u64,
    pub channels: u64,
    pub start: f64,
    pub duration: f64,
}

impl AudioProbe {
    pub fn read(path: &Path) -> Self {
        let output = Command::new(tool("ffprobe"))
            .args(["-v", "error", "-select_streams", "a:0", "-of", "json"])
            .args([
                "-show_entries",
                "stream=codec_name,sample_rate,channels,start_time,duration",
            ])
            .arg(path)
            .output()
            .unwrap();
        assert!(output.status.success(), "ffprobe: {output:?}");
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        let stream = &json["streams"][0];
        let number = |key: &str| stream[key].as_str().unwrap().parse::<f64>().unwrap();
        Self {
            codec: stream["codec_name"].as_str().unwrap().to_owned(),
            rate: number("sample_rate") as u64,
            channels: stream["channels"].as_u64().unwrap(),
            start: number("start_time"),
            duration: number("duration"),
        }
    }

    /// Every sound track: FLAC, 48 kHz stereo, from 0.
    pub fn check_common(&self) {
        assert_eq!(self.codec, "flac");
        assert_eq!((self.rate, self.channels), (48_000, 2));
        assert_eq!(self.start, 0.0);
    }
}

/// The left channel of a sound track, decoded by FFmpeg.
pub fn audio_left(path: &Path) -> Vec<f32> {
    let output = Command::new(tool("ffmpeg"))
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-af", "pan=mono|c0=c0", "-f", "f32le", "-"])
        .output()
        .unwrap();
    assert!(output.status.success());
    output
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect()
}

/// Saves the frame at `seconds` as PNG next to the file and returns its path.
pub fn frame_png(path: &Path, seconds: f64) -> PathBuf {
    let png = path.with_extension(format!("{seconds}s.png"));
    let status = Command::new(tool("ffmpeg"))
        .args(["-v", "error", "-y", "-ss", &seconds.to_string(), "-i"])
        .arg(path)
        .args(["-frames:v", "1"])
        .arg(&png)
        .status()
        .unwrap();
    assert!(status.success());
    png
}

/// Raw RGB of the frame at `seconds`, to compare frames.
pub fn frame_rgb(path: &Path, seconds: f64) -> Vec<u8> {
    let output = Command::new(tool("ffmpeg"))
        .args(["-v", "error", "-ss", &seconds.to_string(), "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .output()
        .unwrap();
    assert!(output.status.success());
    output.stdout
}
