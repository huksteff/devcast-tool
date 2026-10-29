//! A recording's telemetry as the cursor effect reads it (`dc_core::CursorTrack`, decision 015).

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use dc_core::project::Media;
use dc_core::{CursorPicture, CursorPoint, CursorShape, CursorTrack, PixelSize, Point};

use super::cursor::{CursorEvent, image_file};
use super::telemetry::Telemetry;
use crate::error::CaptureError;

/// PNG signature and the IHDR chunk up to the height: width at 16..20, height at 20..24.
const PNG_HEAD: usize = 24;
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

impl Telemetry {
    /// The cursor of recording `media` whose folder is `dir`: positions as fractions of its
    /// picture, pictures as paths in the project next to its video.
    pub fn cursor_track(&self, dir: &Path, media: &Media) -> Result<CursorTrack, CaptureError> {
        let folder = media
            .path
            .rsplit_once('/')
            .map_or(String::new(), |(f, _)| format!("{f}/"));
        let (w, h) = (f64::from(media.size.width), f64::from(media.size.height));
        let at_pixel = |x: i32, y: i32| Point {
            x: f64::from(x) / w,
            y: f64::from(y) / h,
        };
        let mut track = CursorTrack::default();
        let mut pictures: HashMap<u32, CursorPicture> = HashMap::new();
        for event in &self.cursor {
            let at = event.at;
            match event.event {
                CursorEvent::Image { id, x, y } => {
                    let name = image_file(id);
                    let picture = CursorPicture {
                        path: format!("{folder}{name}"),
                        size: png_size(&dir.join(&name))?,
                        hotspot: Point {
                            x: f64::from(x),
                            y: f64::from(y),
                        },
                    };
                    pictures.insert(id, picture);
                }
                CursorEvent::Shape { image, .. } => {
                    let picture = image.map(|id| picture(&pictures, id, dir)).transpose()?;
                    track.shapes.push(CursorShape { at, picture });
                }
                other => {
                    let Some((x, y)) = other.position() else {
                        continue;
                    };
                    let point = CursorPoint {
                        at,
                        pos: at_pixel(x, y),
                    };
                    track.moves.push(point);
                    if matches!(other, CursorEvent::Down { .. }) {
                        track.clicks.push(point);
                    }
                }
            }
        }
        Ok(track)
    }
}

fn picture(
    pictures: &HashMap<u32, CursorPicture>,
    id: u32,
    dir: &Path,
) -> Result<CursorPicture, CaptureError> {
    pictures
        .get(&id)
        .cloned()
        .ok_or_else(|| CaptureError::BadTelemetry {
            path: dir.join(super::CURSOR_FILE),
            message: format!("форма курсора ссылается на картинку {id} до её записи"),
        })
}

/// Width and height from the PNG header.
fn png_size(path: &Path) -> Result<PixelSize, CaptureError> {
    let bad = |message: String| CaptureError::BadTelemetry {
        path: path.to_path_buf(),
        message,
    };
    let mut head = [0u8; PNG_HEAD];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut head))
        .map_err(|e| bad(e.to_string()))?;
    let number = |at: usize| {
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&head[at..at + 4]);
        u32::from_be_bytes(bytes)
    };
    let size = PixelSize {
        width: number(16),
        height: number(20),
    };
    if head[..8] != PNG_SIGNATURE || &head[12..16] != b"IHDR" || size.width == 0 || size.height == 0
    {
        return Err(bad("это не картинка PNG".to_owned()));
    }
    Ok(size)
}

#[cfg(test)]
#[path = "track_tests.rs"]
mod tests;
