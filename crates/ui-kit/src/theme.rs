//! The dark theme from the tokens: egui's own widgets (text fields, combo boxes, scroll bars)
//! look like ours. The only theme: the interface is dark only (SCOPE).

use egui::style::{Selection, WidgetVisuals, Widgets};
use egui::{CornerRadius, Margin, Shadow, Stroke, Style, TextStyle, Theme, Visuals, vec2};

use crate::fonts::{self, Weight};
use crate::tokens::{
    ACCENT, ACCENT_HI, ACCENT_SOFT, BORDER, CONTROL, FS_L, FS_M, FS_S, PAUSE, R1, R3, REC, S1, S2,
    S3, SURFACE, SURFACE_2, SURFACE_3, TEXT, TEXT_3,
};

/// Panel and popover shadow: 0 8px 24px black 45%.
pub const SHADOW: Shadow = Shadow {
    offset: [0, 8],
    blur: 24,
    spread: 0,
    color: egui::Color32::from_black_alpha(115),
};

/// Shadow of the floating windows (HUD, popovers, countdown): their transparent window takes
/// clicks in its whole rectangle, shadow included, so the shadow is smaller than [`SHADOW`]:
/// 6 px at the sides, 4 above and 8 below — the popover's shadow ends at the HUD 8 px under it
/// (decision 020).
pub const WINDOW_SHADOW: Shadow = Shadow {
    offset: [0, 2],
    blur: 12,
    spread: 0,
    color: egui::Color32::from_black_alpha(115),
};

/// Sets the theme on `ctx` for both system themes, so the OS theme changes nothing.
pub fn apply(ctx: &egui::Context) {
    for theme in [Theme::Dark, Theme::Light] {
        ctx.set_style_of(theme, style());
    }
    ctx.set_theme(Theme::Dark);
}

pub fn style() -> Style {
    let mut style = Style {
        visuals: visuals(),
        ..Style::default()
    };
    style.text_styles = [
        (TextStyle::Small, fonts::text(FS_S, Weight::Regular)),
        (TextStyle::Body, fonts::text(FS_M, Weight::Regular)),
        (TextStyle::Button, fonts::text(FS_M, Weight::Medium)),
        (TextStyle::Heading, fonts::text(FS_L, Weight::Semibold)),
        (TextStyle::Monospace, fonts::mono(FS_S)),
    ]
    .into();
    let spacing = &mut style.spacing;
    spacing.item_spacing = vec2(S2, S2);
    spacing.button_padding = vec2(S3, S1);
    spacing.interact_size = vec2(CONTROL, CONTROL);
    spacing.window_margin = Margin::same(S3 as i8);
    spacing.menu_margin = Margin::same(S2 as i8);
    spacing.icon_width = 16.0;
    spacing.icon_width_inner = 8.0;
    spacing.combo_height = 320.0;
    style
}

fn visuals() -> Visuals {
    let shadow = SHADOW;
    Visuals {
        dark_mode: true,
        override_text_color: None,
        weak_text_color: Some(TEXT_3),
        widgets: widgets(),
        selection: Selection {
            bg_fill: ACCENT_SOFT,
            stroke: Stroke::new(1.0, TEXT),
        },
        hyperlink_color: ACCENT_HI,
        faint_bg_color: SURFACE_2,
        extreme_bg_color: SURFACE_2,
        text_edit_bg_color: Some(SURFACE_2),
        code_bg_color: SURFACE_3,
        warn_fg_color: PAUSE,
        error_fg_color: REC,
        window_corner_radius: CornerRadius::same(R3),
        window_shadow: shadow,
        window_fill: SURFACE,
        window_stroke: Stroke::new(1.0, BORDER),
        menu_corner_radius: CornerRadius::same(R3),
        panel_fill: SURFACE,
        popup_shadow: shadow,
        ..Visuals::dark()
    }
}

fn widgets() -> Widgets {
    let base = WidgetVisuals {
        bg_fill: SURFACE_3,
        weak_bg_fill: SURFACE_2,
        bg_stroke: Stroke::new(1.0, BORDER),
        corner_radius: CornerRadius::same(R1),
        fg_stroke: Stroke::new(1.0, TEXT),
        expansion: 0.0,
    };
    Widgets {
        noninteractive: WidgetVisuals {
            bg_fill: SURFACE,
            weak_bg_fill: SURFACE,
            fg_stroke: Stroke::new(1.0, TEXT),
            ..base
        },
        inactive: base,
        hovered: WidgetVisuals {
            weak_bg_fill: SURFACE_3,
            ..base
        },
        active: WidgetVisuals {
            bg_fill: ACCENT,
            weak_bg_fill: SURFACE_3,
            bg_stroke: Stroke::new(1.0, ACCENT_HI),
            ..base
        },
        open: WidgetVisuals {
            bg_stroke: Stroke::new(1.0, ACCENT_HI),
            ..base
        },
    }
}
