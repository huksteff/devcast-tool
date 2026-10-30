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
pub const WHITE: Color32 = Color32::WHITE;

pub const S1: f32 = 4.0;
pub const S2: f32 = 8.0;
pub const S3: f32 = 12.0;
pub const S4: f32 = 16.0;
pub const S5: f32 = 24.0;
pub const S6: f32 = 32.0;

/// Controls.
pub const R1: u8 = 6;
/// Cards.
pub const R2: u8 = 8;
/// Panels and windows.
pub const R3: u8 = 12;

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
