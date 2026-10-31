//! «Субтитры» of the settings window (mockup 5c): the Whisper model as two cards side by side.
//! The chosen card has a blue border, a soft blue fill and a filled radio; each card says
//! whether its model is downloaded, offers «Скачать» or shows the download progress.

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::settings as text;
use dc_ui_kit::tokens::{
    ACCENT_HI, ACCENT_SOFT, BORDER, FS_M, OK, R2, S1, S2, SURFACE_3, TEXT, TEXT_2, TEXT_3,
};
use dc_ui_kit::widgets::Button;
use eframe::egui::{
    self, Color32, Galley, Rect, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2,
};
use std::sync::Arc;

use crate::error::AppError;
use crate::models::{ModelState, Models};
use crate::settings::{AppSettings, SpeechModel};
use crate::settings_rows::heading;

/// Inner padding of a card, points (mockup: 8 px × 10 px).
const PAD: egui::Vec2 = vec2(10.0, 8.0);
/// The radio circle, points (mockup: 14 px).
const RADIO: f32 = 14.0;
/// Size of the grey lines of a card, points (mockup: 11 px).
const SMALL: f32 = 11.0;
/// Height of the download progress bar, points.
const BAR: f32 = 4.0;

/// What a click on a card asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardClick {
    Choose,
    Download,
}

/// The page; a download that could not start is returned.
pub fn captions(ui: &mut Ui, next: &mut AppSettings, models: &mut Models) -> Result<(), AppError> {
    heading(ui, text::CAPTIONS);
    ui.label(egui::RichText::new(text::MODEL).color(TEXT_2));
    ui.add_space(S1);
    let width = (ui.available_width() - S2) / 2.0;
    let cards = SpeechModel::ALL.map(|model| Card {
        title: title(model),
        hint: hint(model),
        chosen: next.speech_model == model,
        state: models.state(model),
        can_download: models.can_download(),
    });
    let height = cards
        .iter()
        .map(|card| card.height(ui, width))
        .fold(0.0, f32::max);
    let mut result = Ok(());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = S2;
        for (model, card) in SpeechModel::ALL.into_iter().zip(&cards) {
            match card.show(ui, vec2(width, height)) {
                Some(CardClick::Choose) => next.speech_model = model,
                Some(CardClick::Download) => {
                    let ctx = ui.ctx().clone();
                    let wake = move || ctx.request_repaint_of(egui::ViewportId::ROOT);
                    result = models.download(wake, model);
                }
                None => {}
            }
        }
    });
    result
}

fn title(model: SpeechModel) -> &'static str {
    match model {
        SpeechModel::Fast => text::MODEL_FAST,
        SpeechModel::Accurate => text::MODEL_ACCURATE,
    }
}

fn hint(model: SpeechModel) -> &'static str {
    match model {
        SpeechModel::Fast => text::MODEL_FAST_HINT,
        SpeechModel::Accurate => text::MODEL_ACCURATE_HINT,
    }
}

/// One model card.
pub struct Card {
    pub title: &'static str,
    pub hint: &'static str,
    pub chosen: bool,
    pub state: ModelState,
    pub can_download: bool,
}

impl Card {
    /// Width of the text column of a card `width` wide.
    fn text_width(width: f32) -> f32 {
        width - 2.0 * PAD.x - RADIO - S2
    }

    fn lines(&self, ui: &Ui, width: f32) -> (Arc<Galley>, Arc<Galley>) {
        let painter = ui.painter();
        let wrap = Self::text_width(width);
        let title = painter.layout(
            self.title.to_owned(),
            fonts::text(FS_M, Weight::Regular),
            TEXT,
            wrap,
        );
        let font = fonts::text(SMALL, Weight::Regular);
        let hint = painter.layout(self.hint.to_owned(), font, TEXT_3, wrap);
        (title, hint)
    }

    /// The status line: a word, «Скачать» or the progress.
    fn status(&self) -> Status {
        match self.state {
            ModelState::Ready => Status::Text(text::MODEL_READY.to_owned(), OK),
            ModelState::Missing if self.can_download => Status::Download,
            ModelState::Missing => Status::Text(text::MODEL_MISSING.to_owned(), TEXT_3),
            ModelState::Downloading(done) => Status::Progress(done),
        }
    }

    pub fn height(&self, ui: &Ui, width: f32) -> f32 {
        let (title, hint) = self.lines(ui, width);
        let status = match self.status() {
            Status::Download => S1 + Button::new(text::DOWNLOAD).small().size(ui).y,
            Status::Text(..) => SMALL + S1,
            Status::Progress(_) => SMALL + S1 + S1 + BAR,
        };
        2.0 * PAD.y + title.size().y + hint.size().y + status
    }

    pub fn show(&self, ui: &mut Ui, size: egui::Vec2) -> Option<CardClick> {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        response.widget_info(|| {
            WidgetInfo::selected(egui::Role::RadioButton, true, self.chosen, self.title)
        });
        let (fill, border) = if self.chosen {
            (ACCENT_SOFT, ACCENT_HI)
        } else {
            (Color32::TRANSPARENT, BORDER)
        };
        let painter = ui.painter();
        painter.rect(rect, R2, fill, Stroke::new(1.0, border), StrokeKind::Inside);
        let radio = pos2(
            rect.left() + PAD.x + RADIO / 2.0,
            rect.top() + PAD.y + 2.0 + RADIO / 2.0,
        );
        if self.chosen {
            painter.circle_stroke(radio, RADIO / 2.0 - 2.0, Stroke::new(4.0, ACCENT_HI));
        } else {
            painter.circle_stroke(radio, RADIO / 2.0 - 1.0, Stroke::new(2.0, TEXT_3));
        }
        let (title, hint) = self.lines(ui, size.x);
        let left = rect.left() + PAD.x + RADIO + S2;
        let mut y = rect.top() + PAD.y;
        let title_height = title.size().y;
        painter.galley(pos2(left, y), title, TEXT);
        y += title_height;
        let hint_height = hint.size().y;
        painter.galley(pos2(left, y), hint, TEXT_3);
        y += hint_height;
        let line = Rect::from_min_size(pos2(left, y), vec2(Self::text_width(size.x), SMALL + S1));
        let mut click = response.clicked().then_some(CardClick::Choose);
        if self.status_ui(ui, line) {
            click = Some(CardClick::Download);
        }
        click
    }

    /// Draws the status line from the top left of `line`; true when «Скачать» was clicked.
    fn status_ui(&self, ui: &mut Ui, line: Rect) -> bool {
        let font = fonts::text(SMALL, Weight::Regular);
        match self.status() {
            Status::Text(word, color) => {
                ui.painter()
                    .text(line.min, egui::Align2::LEFT_TOP, word, font, color);
                false
            }
            Status::Download => {
                let button = Button::new(text::DOWNLOAD).small();
                let at = Rect::from_min_size(line.min + vec2(0.0, S1), button.size(ui));
                ui.put(at, button).clicked()
            }
            Status::Progress(done) => {
                let word = done.map_or_else(
                    || text::DOWNLOADING.to_owned(),
                    |d| text::downloading((d * 100.0).round() as u32),
                );
                let painter = ui.painter();
                painter.text(line.min, egui::Align2::LEFT_TOP, word, font, TEXT_2);
                let track = Rect::from_min_size(
                    line.min + vec2(0.0, SMALL + 2.0 * S1),
                    vec2(line.width(), BAR),
                );
                painter.rect_filled(track, BAR / 2.0, SURFACE_3);
                let share = done.unwrap_or(0.0).clamp(0.0, 1.0);
                let filled = Rect::from_min_size(track.min, vec2(track.width() * share, BAR));
                painter.rect_filled(filled, BAR / 2.0, ACCENT_HI);
                false
            }
        }
    }
}

enum Status {
    Text(String, Color32),
    Download,
    Progress(Option<f32>),
}
