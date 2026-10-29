//! Reference frames (decision 012): a rendered frame against its PNG in
//! `fixtures/golden/<backend>/<name>.png`, for the tests of render and of the effects.
//!
//! The adapter the references were made on (`adapter.txt` next to them; WARP on Windows) must
//! match them byte for byte. Any other adapter rasterizes edges and rounds a little differently,
//! so there the frames are compared by ꟻLIP (nv-flip, perceptual error 0..1 per pixel) under
//! [`FLIP_MEAN_MAX`] and [`FLIP_P99_MAX`]. A failed check writes the frame and its ꟻLIP error map
//! to `target/golden/<backend>/`. `UPDATE_GOLDEN=1` (`cargo xtask golden update`) rewrites the
//! references instead of checking them.

use std::path::{Path, PathBuf};

use dc_core::PixelSize;
use dc_media::{MediaError, decode_picture, encode_png};

use crate::gpu::Gpu;
use crate::readback::Image;

/// Mean ꟻLIP error allowed on an adapter other than the references'. Measured RTX 3060 Ti
/// against WARP on the render references: 0.0014–0.0064 (blur the most).
pub const FLIP_MEAN_MAX: f32 = 0.02;
/// ꟻLIP error that 99 % of the pixels must stay under on another adapter (measured 0.015–0.038).
/// A change in less than 1 % of the frame passes it; the references' own adapter catches it.
pub const FLIP_P99_MAX: f32 = 0.1;

const UPDATE: &str = "UPDATE_GOLDEN";

/// Checks rendered frames against the references of one backend.
pub struct Golden {
    dir: PathBuf,
    report: PathBuf,
    adapter: String,
    /// The references were made on this adapter: frames must match them exactly.
    exact: bool,
    update: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum GoldenError {
    #[error("нет эталона «{}». Создай его: cargo xtask golden update \"<почему>\"", path.display())]
    Missing { path: PathBuf },
    #[error(
        "кадр «{name}» {actual:?}, а эталон {expected:?}. Если размер поменялся намеренно, обнови эталон: cargo xtask golden update \"<почему>\""
    )]
    Size {
        name: String,
        actual: PixelSize,
        expected: PixelSize,
    },
    #[error(
        "кадр «{name}» не совпал с эталоном ({how}; ꟻLIP: среднее {mean:.4}, 99 % ≤ {p99:.4}). Кадр: {}, разница: {}. Если картинка должна была измениться, обнови эталон: cargo xtask golden update \"<почему>\"",
        actual.display(), diff.display()
    )]
    Differs {
        name: String,
        how: String,
        mean: f32,
        p99: f32,
        actual: PathBuf,
        diff: PathBuf,
    },
    #[error("эталон или отчёт не записался: {0}")]
    Media(#[from] MediaError),
    #[error("«{}»: {source}. Проверь, что папка доступна для записи", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl Golden {
    /// References of the backend `gpu` runs on.
    pub fn new(gpu: &Gpu) -> Result<Self, GoldenError> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let info = gpu.info();
        let backend = info.backend.to_str();
        let dir = root.join("fixtures").join("golden").join(backend);
        let adapter = format!("{} | {} | {}\n", info.name, info.driver, info.driver_info);
        let update = std::env::var_os(UPDATE).is_some_and(|v| v == "1");
        let made_on = dir.join("adapter.txt");
        if update {
            write(&made_on, adapter.as_bytes())?;
        }
        let exact = match std::fs::read_to_string(&made_on) {
            Ok(text) => text == adapter,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(source) => {
                return Err(GoldenError::Io {
                    path: made_on,
                    source,
                });
            }
        };
        Ok(Self {
            report: root.join("target").join("golden").join(backend),
            exact,
            dir,
            adapter,
            update,
        })
    }

    /// The adapter of the checked frames, as `adapter.txt` records it.
    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    /// `image` against reference `name` (`render/shapes`), or rewrites it under `UPDATE_GOLDEN=1`.
    pub fn check(&self, name: &str, image: &Image) -> Result<(), GoldenError> {
        let path = self.dir.join(format!("{name}.png"));
        if self.update {
            return write(&path, &encode_png(image.size, &image.rgba)?);
        }
        if !path.is_file() {
            return Err(GoldenError::Missing { path });
        }
        let reference = decode_picture(&path)?;
        let expected = &reference.frames[0].rgba;
        if reference.size != image.size {
            return Err(GoldenError::Size {
                name: name.to_owned(),
                actual: image.size,
                expected: reference.size,
            });
        }
        if *expected == image.rgba {
            return Ok(());
        }
        let (error, mean, p99) = flip(image.size, expected, &image.rgba);
        if !self.exact && mean <= FLIP_MEAN_MAX && p99 <= FLIP_P99_MAX {
            return Ok(());
        }
        let how = if self.exact {
            "на адаптере эталонов кадр должен совпасть до байта".to_owned()
        } else {
            format!("порог: среднее ≤ {FLIP_MEAN_MAX}, 99 % ≤ {FLIP_P99_MAX}")
        };
        let actual = self.report.join(format!("{name}.png"));
        let diff = self.report.join(format!("{name}.diff.png"));
        write(&actual, &encode_png(image.size, &image.rgba)?)?;
        write(&diff, &encode_png(image.size, &error)?)?;
        Err(GoldenError::Differs {
            name: name.to_owned(),
            how,
            mean,
            p99,
            actual,
            diff,
        })
    }
}

/// ꟻLIP of `actual` against `expected` (RGBA, alpha ignored): the error map in the magma colours
/// as RGBA, the mean error and the error 99 % of the pixels stay under.
fn flip(size: PixelSize, expected: &[u8], actual: &[u8]) -> (Vec<u8>, f32, f32) {
    let rgb = |rgba: &[u8]| -> Vec<u8> {
        rgba.as_chunks::<4>()
            .0
            .iter()
            .flat_map(|[r, g, b, _]| [*r, *g, *b])
            .collect()
    };
    let image = |rgba| nv_flip::FlipImageRgb8::with_data(size.width, size.height, &rgb(rgba));
    let error = nv_flip::flip(
        image(expected),
        image(actual),
        nv_flip::DEFAULT_PIXELS_PER_DEGREE,
    );
    let mut pool = nv_flip::FlipPool::from_image(&error);
    let colours = error.apply_color_lut(&nv_flip::magma_lut()).to_vec();
    let rgba = colours
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|[r, g, b]| [*r, *g, *b, 255])
        .collect();
    (rgba, pool.mean(), pool.get_percentile(0.99, false))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), GoldenError> {
    let io = |source| GoldenError::Io {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    std::fs::write(path, bytes).map_err(io)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(width: u32, height: u32, rgba: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .flat_map(|(x, y)| rgba(x, y))
            .collect()
    }

    #[test]
    fn flip_passes_edge_noise_and_fails_a_missing_shape() {
        let size = PixelSize {
            width: 64,
            height: 64,
        };
        let square = |x: u32, y: u32| (16..48).contains(&x) && (16..48).contains(&y);
        let reference = frame(64, 64, |x, y| {
            if square(x, y) {
                [255, 0, 0, 255]
            } else {
                [0, 0, 0, 255]
            }
        });
        // Antialiased edge pixels a few levels off, as another adapter gives them.
        let noisy = frame(64, 64, |x, y| match (square(x, y), x == 16 || x == 47) {
            (true, true) => [250, 3, 0, 255],
            (true, false) => [255, 0, 0, 255],
            _ => [0, 0, 0, 255],
        });
        let (_, mean, p99) = flip(size, &reference, &noisy);
        assert!(mean <= FLIP_MEAN_MAX && p99 <= FLIP_P99_MAX, "{mean} {p99}");
        // A 12×12 corner gone: 3.5 % of the frame.
        let small = |x: u32, y: u32| (36..48).contains(&x) && (36..48).contains(&y);
        let without = frame(64, 64, |x, y| {
            if square(x, y) && !small(x, y) {
                [255, 0, 0, 255]
            } else {
                [0, 0, 0, 255]
            }
        });
        let (map, mean, p99) = flip(size, &reference, &without);
        assert!(mean > FLIP_MEAN_MAX || p99 > FLIP_P99_MAX, "{mean} {p99}");
        assert_eq!(map.len(), 64 * 64 * 4);
    }
}
