//! Waveform peaks for the timeline: the lowest and the highest sample of every [`BASE`] frames
//! of a track (all channels together), and coarser levels, each [`FACTOR`] times coarser, so a
//! view of any zoom reads a few thousand values. Only the finest level is cached, in
//! `cache/<track file>.<stream>.peaks`; the others are rebuilt on load.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use dc_project::{ProjectDir, atomic};

use crate::error::AudioError;
use crate::reader::TrackReader;
use crate::track::SourceTrack;
use crate::{CHANNELS, RATE};

/// Frames of one peak at the finest level: about 5 ms.
pub const BASE: i64 = 256;

/// Each level has this many times fewer peaks than the previous one.
pub const FACTOR: i64 = 4;

const MAGIC: &[u8; 4] = b"DCPK";
const VERSION: u32 = 1;

/// Frames decoded at a time while building.
const CHUNK: usize = 64 * BASE as usize;

/// The lowest and the highest sample of a stretch; both 0 for silence or no sound.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Peak {
    pub min: f32,
    pub max: f32,
}

impl Peak {
    fn join(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Peaks {
    /// Frames of the track from source frame 0.
    frames: i64,
    /// `levels[k]` has a peak per `BASE * FACTOR^k` frames.
    levels: Vec<Vec<Peak>>,
}

impl Peaks {
    /// Reads the whole track from source frame 0.
    pub fn build(track: &SourceTrack) -> Result<Self, AudioError> {
        let mut reader = TrackReader::open(track)?;
        let mut buffer = vec![0.0; CHUNK * CHANNELS];
        let mut finest = Vec::new();
        let mut frames = 0;
        while let Some(read) = reader.fill_until_end(frames, &mut buffer)? {
            let samples = &buffer[..read * CHANNELS];
            for bucket in samples.chunks(BASE as usize * CHANNELS) {
                let (min, max) = bucket
                    .iter()
                    .fold((0f32, 0f32), |(lo, hi), s| (lo.min(*s), hi.max(*s)));
                finest.push(Peak { min, max });
            }
            frames += read as i64;
            if read < CHUNK {
                break;
            }
        }
        Ok(Self::from_finest(frames, finest))
    }

    fn from_finest(frames: i64, finest: Vec<Peak>) -> Self {
        let mut levels = vec![finest];
        while let Some(last) = levels.last().filter(|l| l.len() > 1) {
            let coarser = last
                .chunks(FACTOR as usize)
                .map(|group| group.iter().fold(Peak::default(), |a, b| a.join(*b)))
                .collect();
            levels.push(coarser);
        }
        Self { frames, levels }
    }

    /// Frames of the track from source frame 0.
    pub fn frames(&self) -> i64 {
        self.frames
    }

    /// `count` peaks evenly covering source frames `start..end`; outside the track they are 0.
    pub fn range(&self, start: i64, end: i64, count: usize) -> Vec<Peak> {
        if count == 0 || end <= start {
            return vec![Peak::default(); count];
        }
        let per_peak = (end - start) as f64 / count as f64;
        (0..count)
            .map(|i| {
                let from = start + (i as f64 * per_peak).round() as i64;
                let to = start + ((i + 1) as f64 * per_peak).round() as i64;
                self.envelope(from, to.max(from + 1))
            })
            .collect()
    }

    /// The peak of frames `from..to`, exact to [`BASE`] frames: whole coarse peaks in the
    /// middle, finer ones at the edges, so a loud part never spills into its neighbour.
    fn envelope(&self, from: i64, to: i64) -> Peak {
        let mut first = from.max(0).div_euclid(BASE);
        let mut last = (to.min(self.frames) + BASE - 1).div_euclid(BASE);
        let mut peak = Peak::default();
        let mut take = |level: &[Peak], n: i64| {
            if let Some(p) = level.get(n as usize) {
                peak = peak.join(*p);
            }
        };
        for (index, level) in self.levels.iter().enumerate() {
            if index + 1 == self.levels.len() {
                (first..last).for_each(|n| take(level, n));
                break;
            }
            while first < last && first % FACTOR != 0 {
                take(level, first);
                first += 1;
            }
            while first < last && last % FACTOR != 0 {
                last -= 1;
                take(level, last);
            }
            (first, last) = (first / FACTOR, last / FACTOR);
        }
        peak
    }
}

/// Peaks of `track` of `project`, from the cache while the file and its start are the same.
pub fn cached_peaks(project: &ProjectDir, track: &SourceTrack) -> Result<Peaks, AudioError> {
    let cache = cache_path(project, track);
    let header = header(track)?;
    if let Some(peaks) = read(&cache, &header) {
        return Ok(peaks);
    }
    let peaks = Peaks::build(track)?;
    if let Some(folder) = cache.parent() {
        std::fs::create_dir_all(folder).map_err(|e| AudioError::cache(folder, e))?;
    }
    let mut bytes = header;
    bytes.extend_from_slice(&peaks.frames.to_le_bytes());
    for peak in &peaks.levels[0] {
        bytes.extend_from_slice(&quantize(peak.min).to_le_bytes());
        bytes.extend_from_slice(&quantize(peak.max).to_le_bytes());
    }
    atomic::write(&cache, &bytes)?;
    Ok(peaks)
}

/// `cache/<track file relative to the project>.<stream>.peaks`.
fn cache_path(project: &ProjectDir, track: &SourceTrack) -> PathBuf {
    let relative = track
        .path
        .strip_prefix(project.root())
        .unwrap_or(&track.path);
    let mut path = project.cache_dir();
    path.extend(relative.components().filter_map(|c| match c {
        std::path::Component::Normal(part) => Some(part),
        _ => None,
    }));
    let mut name = path.into_os_string();
    name.push(format!(".{}.peaks", track.stream));
    PathBuf::from(name)
}

/// What the cache was built from: format, file size and time, the track's start.
fn header(track: &SourceTrack) -> Result<Vec<u8>, AudioError> {
    let metadata = std::fs::metadata(&track.path).map_err(|e| AudioError::cache(&track.path, e))?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&metadata.len().to_le_bytes());
    bytes.extend_from_slice(&modified.to_le_bytes());
    bytes.extend_from_slice(&track.origin.to_le_bytes());
    bytes.extend_from_slice(&RATE.to_le_bytes());
    Ok(bytes)
}

fn read(cache: &Path, header: &[u8]) -> Option<Peaks> {
    let bytes = std::fs::read(cache).ok()?;
    let rest = bytes.strip_prefix(header)?;
    let (frames, data) = rest.split_first_chunk::<8>()?;
    let (pairs, tail) = data.as_chunks::<4>();
    if !tail.is_empty() {
        return None;
    }
    let finest = pairs
        .iter()
        .map(|p| Peak {
            min: dequantize(i16::from_le_bytes([p[0], p[1]])),
            max: dequantize(i16::from_le_bytes([p[2], p[3]])),
        })
        .collect();
    Some(Peaks::from_finest(i64::from_le_bytes(*frames), finest))
}

/// A sample in 1/32767 steps, rounded away from 0 so a peak never shrinks.
fn quantize(sample: f32) -> i16 {
    let scaled = (sample.clamp(-1.0, 1.0) * 32767.0)
        .abs()
        .ceil()
        .copysign(sample);
    scaled as i16
}

fn dequantize(value: i16) -> f32 {
    f32::from(value) / 32767.0
}

#[cfg(test)]
#[path = "peaks_tests.rs"]
mod tests;
