//! Interface fonts from the program's `fonts` folder (decision 005): Inter for text, JetBrains
//! Mono for the timer and shortcuts, Noto Color Emoji as the fallback. No egui built-in or
//! system fonts, so the interface looks the same on every PC.

use std::path::Path;
use std::sync::Arc;

use egui::epaint::text::{FontData, FontDefinitions, FontTweak, VariationCoords};
use egui::{FontFamily, FontId};

use crate::error::UiKitError;

const INTER: &str = "Inter[opsz,wght].ttf";
const MONO: &str = "JetBrainsMono[wght].ttf";
const EMOJI: &str = "NotoColorEmoji.ttf";

/// Inter weights as named families; the plain proportional family is 400.
const WEIGHTS: [(Weight, f32); 3] = [
    (Weight::Medium, 500.0),
    (Weight::Semibold, 600.0),
    (Weight::Bold, 700.0),
];

/// Weight of the interface font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Medium,
    Semibold,
    Bold,
}

impl Weight {
    pub fn family(self) -> FontFamily {
        match self {
            Self::Regular => FontFamily::Proportional,
            Self::Medium => FontFamily::Name("medium".into()),
            Self::Semibold => FontFamily::Name("semibold".into()),
            Self::Bold => FontFamily::Name("bold".into()),
        }
    }
}

/// Inter of `size` points and `weight`.
pub fn text(size: f32, weight: Weight) -> FontId {
    FontId::new(size, weight.family())
}

/// JetBrains Mono 500 of `size` points: the timer and key labels.
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// The interface fonts read from `dir`.
pub fn definitions(dir: &Path) -> Result<FontDefinitions, UiKitError> {
    let read = |name: &str| -> Result<Arc<FontData>, UiKitError> {
        let path = dir.join(name);
        let bytes = std::fs::read(&path).map_err(|source| UiKitError::Font { path, source })?;
        Ok(Arc::new(FontData::from_owned(bytes)))
    };
    let inter = read(INTER)?;
    let mono = read(MONO)?;
    let mut fonts = FontDefinitions::empty();
    fonts.font_data.insert(EMOJI.into(), read(EMOJI)?);
    fonts.font_data.insert(INTER.into(), Arc::clone(&inter));
    fonts
        .font_data
        .insert(MONO.into(), weighted(&mono, 500.0).into());
    fonts
        .families
        .insert(FontFamily::Proportional, vec![INTER.into(), EMOJI.into()]);
    fonts.families.insert(
        FontFamily::Monospace,
        vec![MONO.into(), INTER.into(), EMOJI.into()],
    );
    for (weight, value) in WEIGHTS {
        let name = format!("Inter {value}");
        fonts
            .font_data
            .insert(name.clone(), weighted(&inter, value).into());
        fonts
            .families
            .insert(weight.family(), vec![name, EMOJI.into()]);
    }
    Ok(fonts)
}

/// The same file at another point of its `wght` axis.
fn weighted(font: &FontData, weight: f32) -> FontData {
    FontData::from_blob(Arc::clone(&font.font), font.index).tweak(FontTweak {
        coords: VariationCoords::new([(b"wght", weight)]),
        ..FontTweak::default()
    })
}
