//! Design tokens of docs/design/recording-windows.html: colours, spacing, radii, text sizes and
//! control sizes. Sizes are egui points (px at 100% scale).

use egui::Color32;

pub const BG: Color32 = Color32::from_rgb(0x0f, 0x11, 0x15);
pub const SURFACE: Color32 = Color32::from_rgb(0x18, 0x1a, 0x1f);
pub const SURFACE_2: Color32 = Color32::from_rgb(0x22, 0x25, 0x2c);
pub const SURFACE_3: Color32 = Color32::from_rgb(0x2c, 0x30, 0x38);
pub const BORDER: Color32 = Color32::from_rgb(0x35, 0x3a, 0x43);
pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xea, 0xed);
pub const TEXT_2: Color32 = Color32::from_rgb(0xa3, 0xa8, 0xb3);
pub const TEXT_3: Color32 = Color32::from_rgb(0x8a, 0x90, 0x9b);
/// The one accent: selection and «on».
pub const ACCENT: Color32 = Color32::from_rgb(0x25, 0x63, 0xeb);
pub const ACCENT_HI: Color32 = Color32::from_rgb(0x3b, 0x82, 0xf6);
pub const ACCENT_SOFT: Color32 = Color32::from_rgba_unmultiplied_const(0x25, 0x63, 0xeb, 46);
/// Red is only recording and dangerous actions.
pub const REC: Color32 = Color32::from_rgb(0xf4, 0x3f, 0x5e);
pub const REC_STRONG: Color32 = Color32::from_rgb(0xe1, 0x1d, 0x48);
pub const REC_SOFT: Color32 = Color32::from_rgba_unmultiplied_const(0xf4, 0x3f, 0x5e, 41);
/// Yellow is pause and conflicts.
pub const PAUSE: Color32 = Color32::from_rgb(0xfb, 0xbf, 0x24);
pub const PAUSE_SOFT: Color32 = Color32::from_rgba_unmultiplied_const(0xfb, 0xbf, 0x24, 31);
pub const OK: Color32 = Color32::from_rgb(0x22, 0xc5, 0x5e);
/// Dimming outside a selection.
pub const SCRIM: Color32 = Color32::from_black_alpha(140);
/// Captions over a picture (`.pill`): the surface at 85 %.
pub const PILL: Color32 = Color32::from_rgba_unmultiplied_const(0x18, 0x1a, 0x1f, 217);
pub const WHITE: Color32 = Color32::WHITE;

pub const S1: f32 = 4.0;
pub const S2: f32 = 8.0;
pub const S3: f32 = 12.0;
pub const S4: f32 = 16.0;
pub const S5: f32 = 24.0;

/// Controls.
pub const R1: u8 = 6;
/// Cards.
pub const R2: u8 = 8;
/// Panels and windows.
pub const R3: u8 = 12;
/// The countdown window (mockup 4).
pub const R_COUNTDOWN: u8 = 24;
/// Fill of the countdown window: surface at 88 % (mockup 4).
pub const COUNTDOWN_FILL: Color32 = Color32::from_rgba_unmultiplied_const(0x18, 0x1a, 0x1f, 224);

/// Captions.
pub const FS_S: f32 = 12.0;
/// Body text.
pub const FS_M: f32 = 13.0;
/// Headings and the timer.
pub const FS_L: f32 = 15.0;
pub const FS_XL: f32 = 20.0;

/// Height of a control and side of an icon button.
pub const CONTROL: f32 = 32.0;
/// Height of a small button.
pub const CONTROL_S: f32 = 26.0;
/// Height of the HUD bar.
pub const HUD_H: f32 = 48.0;
pub const ICON: f32 = 16.0;

/// Colour of one kind of effect on the timeline (docs/design/editor.html «Цвета дорожек»): the
/// same on its track, on the inspector chip and on the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hue {
    /// Dark fill of a region; white text on it.
    pub fill: Color32,
    /// Light border of a region, the swatch of the track header and the inspector chip.
    pub line: Color32,
}

pub const HUE_CLIP: Hue = hue(0x2f4a75, 0x5b86cc);
pub const HUE_SPEED: Hue = hue(0x8a4210, 0xfb923c);
pub const HUE_ZOOM: Hue = hue(0x4c3696, 0x8b6cf0);
/// Cursor, clicks and the callout.
pub const HUE_CURSOR: Hue = hue(0x0f625b, 0x2dd4bf);
pub const HUE_KEYS: Hue = hue(0x38414f, 0x8a99b0);
pub const HUE_CAPTIONS: Hue = hue(0x6e5210, 0xe0b23a);
pub const HUE_ANNOTATIONS: Hue = hue(0x7e2450, 0xf0609e);
pub const HUE_AUDIO: Hue = hue(0x125a3b, 0x34d399);
/// «Сделать красиво» of the editor: running (accent) and done (green) (mockup 9).
pub const HUE_AI: Hue = hue(0x1e3a8a, 0x3b82f6);
pub const HUE_AI_DONE: Hue = hue(0x14532d, 0x22c55e);

const fn hue(fill: u32, line: u32) -> Hue {
    Hue {
        fill: rgb(fill),
        line: rgb(line),
    }
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Keyframe diamonds and changed parameters.
pub const KEYFRAME: Color32 = rgb(0x7dd3fc);
/// Snapping line.
pub const SNAP: Color32 = rgb(0xfacc15);
/// Guide of the preview: an edge or the centre of the frame a box sticks to (mockup `.guide`).
pub const GUIDE: Color32 = rgb(0xe879f9);
/// Behind the preview frame.
pub const PREVIEW_BG: Color32 = rgb(0x0b0c0f);
/// Line between timeline rows.
pub const ROW_LINE: Color32 = rgb(0x1d2026);
