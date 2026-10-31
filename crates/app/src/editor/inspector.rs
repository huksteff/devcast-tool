//! The 340 px inspector: a rail of panel tabs on the left and the open panel with the colour
//! chip of its effect and the groups «Основное» (open) and «Тонкая настройка» (closed).
//!
//! The panels are empty frames until their tasks (7.13 the frame with remembered groups and
//! «Сбросить», 7.14–7.19 the panels); the tabs and the groups already switch.

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::editor as text;
use dc_ui_kit::tokens::{
    self, BG, BORDER, FS_L, FS_M, FS_S, Hue, S2, S3, SURFACE, TEXT, TEXT_2, TEXT_3,
};
use dc_ui_kit::widgets::IconButton;
use eframe::egui::{self, Frame, Margin, Rect, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

use super::view::{Panel, View};

/// Width of the tab rail, side of a tab, height of the panel and group headers (mockup).
const RAIL: f32 = 44.0;
const TAB: f32 = 36.0;
const PANEL_HEADER: f32 = 44.0;
const GROUP_HEADER: f32 = 34.0;
/// Side of the group reset button and of the group chevron.
const RESET: f32 = 26.0;
const CHEVRON: f32 = 14.0;

const PANELS: [Panel; 5] = [
    Panel::Zoom,
    Panel::Cursor,
    Panel::Keys,
    Panel::Captions,
    Panel::Sound,
];

impl Panel {
    fn icon(self) -> Icon {
        match self {
            Self::Zoom => Icon::Frame,
            Self::Cursor => Icon::Cursor,
            Self::Keys => Icon::Keyboard,
            Self::Captions => Icon::Captions,
            Self::Sound => Icon::Speaker,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Zoom => text::ZOOM,
            Self::Cursor => text::CURSOR,
            Self::Keys => text::KEYS,
            Self::Captions => text::CAPTIONS,
            Self::Sound => text::SOUND,
        }
    }

    fn hue(self) -> Hue {
        match self {
            Self::Zoom => tokens::HUE_ZOOM,
            Self::Cursor => tokens::HUE_CURSOR,
            Self::Keys => tokens::HUE_KEYS,
            Self::Captions => tokens::HUE_CAPTIONS,
            Self::Sound => tokens::HUE_AUDIO,
        }
    }
}

pub fn ui(ui: &mut Ui, view: &mut View) {
    egui::Panel::left("inspector-rail")
        .exact_size(RAIL)
        .frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(4, 6)))
        .show(ui, |ui| rail(ui, view));
    egui::CentralPanel::no_frame()
        .frame(Frame::new().fill(SURFACE))
        .show(ui, |ui| {
            header(ui, view.panel);
            group(ui, text::BASIC, &mut view.basic_open);
            group(ui, text::FINE, &mut view.fine_open);
        });
}

fn rail(ui: &mut Ui, view: &mut View) {
    ui.spacing_mut().item_spacing.y = 2.0;
    for panel in PANELS {
        let tab = IconButton::new(panel.icon(), panel.title())
            .size(TAB, tokens::ICON)
            .on(view.panel == panel);
        if ui.add(tab).on_hover_text(panel.title()).clicked() {
            view.panel = panel;
        }
    }
}

/// The colour chip and the name of the panel's effect.
fn header(ui: &mut Ui, panel: Panel) {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), PANEL_HEADER), Sense::hover());
    let painter = ui.painter();
    let chip = Rect::from_center_size(
        pos2(rect.left() + S3 + 2.0, rect.center().y),
        vec2(4.0, 22.0),
    );
    painter.rect_filled(chip, 2, panel.hue().line);
    let title = painter.layout_no_wrap(
        panel.title().to_owned(),
        fonts::text(FS_L, Weight::Semibold),
        TEXT,
    );
    let at = pos2(chip.right() + S2, rect.center().y - title.size().y / 2.0);
    painter.galley(at, title, TEXT);
    bottom_line(ui, rect);
}

/// A group: its header toggles it; an open one shows its (still empty) body.
fn group(ui: &mut Ui, title: &str, open: &mut bool) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), GROUP_HEADER), Sense::click());
    let state = *open;
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, state, title));
    if response.clicked() {
        *open = !*open;
    }
    let chevron = Rect::from_center_size(
        pos2(rect.left() + 10.0 + CHEVRON / 2.0, rect.center().y),
        vec2(CHEVRON, CHEVRON),
    );
    let icon = if *open { Icon::Down } else { Icon::Right };
    icon.paint(ui.painter(), chevron, TEXT_3);
    let (weight, color) = if *open {
        (Weight::Semibold, TEXT)
    } else {
        (Weight::Medium, TEXT_2)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), fonts::text(FS_M, weight), color);
    let at = pos2(
        chevron.right() + 6.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, color);
    let reset = Rect::from_center_size(
        pos2(rect.right() - 6.0 - RESET / 2.0, rect.center().y),
        vec2(RESET, RESET),
    );
    let mut cell = ui.new_child(UiBuilder::new().max_rect(reset));
    cell.add_enabled(
        false,
        IconButton::new(Icon::Reset, text::RESET).size(RESET, CHEVRON),
    );
    let mut bottom = rect;
    if *open {
        ui.add_space(2.0);
        Frame::new()
            .inner_margin(Margin {
                left: S3 as i8,
                right: S3 as i8,
                top: 0,
                bottom: S3 as i8,
            })
            .show(ui, |ui| {
                ui.label(RichText::new(text::PANEL_EMPTY).size(FS_S).color(TEXT_3));
            });
        bottom = bottom.with_max_y(ui.min_rect().bottom());
    }
    bottom_line(ui, bottom);
}

fn bottom_line(ui: &Ui, rect: Rect) {
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, BORDER));
}
