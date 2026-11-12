//! Is there a voice on a track: its noise floor and its speech level ([`silence::measure`]) lie
//! apart. A track that only hisses, a muted microphone and digital silence have both at one
//! level: there every moment is «a pause», and cutting the pauses would cut the whole video.
//! The two levels are cached in `cache/<track file>.<stream>.voice`: `devcast info` asks for
//! them on every call.

use std::path::PathBuf;

use dc_project::{FileStamp, ProjectDir, atomic};

use crate::error::AudioError;
use crate::silence;
use crate::track::SourceTrack;

/// The least distance between the noise floor and the speech level of a track with a voice, dB.
/// The quietest voice checked is 10 dB over its noise (owner's noisy microphone: −45 and −35
/// dBFS); steady noise stays within 3 dB of itself.
pub const SPREAD_DB: f64 = 6.0;

const MAGIC: &[u8; 4] = b"DCVC";
const VERSION: u32 = 1;

/// The noise floor and the speech level of a track, dBFS.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Voice {
    pub noise: f64,
    pub speech: f64,
}

impl Voice {
    /// The silence threshold: halfway in dB between the noise and the speech.
    pub fn threshold(self) -> f64 {
        (self.noise + self.speech) / 2.0
    }

    pub fn is_heard(self) -> bool {
        self.speech - self.noise >= SPREAD_DB
    }
}

/// [`silence::measure`] of `track`, from the cache of `project` when the track file is the same.
pub fn cached(project: &ProjectDir, track: &SourceTrack) -> Result<Voice, AudioError> {
    let cache = cache_path(project, track);
    let header = header(track)?;
    if let Some(voice) = read(&cache, &header) {
        return Ok(voice);
    }
    let voice = silence::measure(track)?;
    if let Some(folder) = cache.parent() {
        std::fs::create_dir_all(folder).map_err(|e| AudioError::cache(folder, e))?;
    }
    let mut bytes = header;
    bytes.extend_from_slice(&voice.noise.to_le_bytes());
    bytes.extend_from_slice(&voice.speech.to_le_bytes());
    atomic::write(&cache, &bytes)?;
    Ok(voice)
}

fn cache_path(project: &ProjectDir, track: &SourceTrack) -> PathBuf {
    project.cache_file(&track.path, &format!(".{}.voice", track.stream))
}

/// What the cache was measured from: format, file size and time.
fn header(track: &SourceTrack) -> Result<Vec<u8>, AudioError> {
    let stamp = FileStamp::of(&track.path)?;
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&stamp.to_le_bytes());
    Ok(bytes)
}

fn read(cache: &std::path::Path, header: &[u8]) -> Option<Voice> {
    let bytes = std::fs::read(cache).ok()?;
    let (noise, speech) = bytes.strip_prefix(header)?.split_first_chunk::<8>()?;
    Some(Voice {
        noise: f64::from_le_bytes(*noise),
        speech: f64::from_le_bytes(speech.try_into().ok()?),
    })
}
