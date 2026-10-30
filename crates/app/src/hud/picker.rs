//! Source picker (mockup 2): a grid of previews of monitors and windows, four in a row, the
//! chosen one outlined in blue. Previews are taken one by one on a worker thread (one WGC frame
//! each, as the recording takes them) and appear as they come.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};

use dc_capture::{Preview, Source, SourceList};
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::picker as text;
use dc_ui_kit::tokens::{
    ACCENT_HI, ACCENT_SOFT, FS_S, R2, S2, S3, SURFACE_2, SURFACE_3, TEXT, TEXT_3,
};
use eframe::egui::{
    self, Color32, ColorImage, Rect, Response, Sense, Stroke, StrokeKind, TextureHandle, Ui,
    WidgetInfo, pos2, vec2,
};

/// Width of the picker popover, points.
pub const WIDTH: f32 = 720.0;
/// Its height limit; more scrolls.
pub const MAX_HEIGHT: f32 = 620.0;
/// Longer side of a preview, pixels.
const PREVIEW_SIDE: u32 = 320;
const COLUMNS: usize = 4;

/// One card of the grid.
pub struct Card {
    pub source: Source,
    pub title: String,
    pub sub: String,
}

/// Cards of the monitors (the primary one named so) and of the windows that are not minimized.
pub fn cards(sources: &SourceList) -> (Vec<Card>, Vec<Card>) {
    let screens = sources
        .monitors
        .iter()
        .map(|m| Card {
            source: m.source(),
            title: if m.primary {
                format!("{} — {}", text::screen(m.index), text::PRIMARY)
            } else {
                text::screen(m.index)
            },
            sub: format!("{} × {}", m.bounds.width(), m.bounds.height()),
        })
        .collect();
    let windows = sources
        .windows
        .iter()
        .filter(|w| !w.minimized)
        .map(|w| Card {
            source: w.source(),
            title: w.title.clone(),
            sub: w
                .app
                .as_ref()
                .map_or_else(String::new, |app| format!("{app}.exe")),
        })
        .collect();
    (screens, windows)
}

/// Previews arriving from the worker, as textures by source.
pub struct Previews {
    receiver: Receiver<(Source, Preview)>,
    textures: HashMap<Source, TextureHandle>,
}

impl Previews {
    /// Starts taking previews of `sources`; each one wakes the UI. A failed preview leaves its
    /// card grey (a window can close meanwhile).
    pub fn start(ctx: &egui::Context, sources: Vec<Source>) -> Self {
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            for source in sources {
                let preview = match source {
                    Source::Monitor(index) => dc_capture::win::preview_monitor(index, PREVIEW_SIDE),
                    Source::Window(id) => dc_capture::win::preview_window(id, PREVIEW_SIDE),
                    Source::Area(_) => continue,
                };
                match preview {
                    Ok(preview) => {
                        if sender.send((source, preview)).is_err() {
                            return;
                        }
                        ctx.request_repaint();
                    }
                    Err(error) => tracing::info!("нет превью {source}: {error}"),
                }
            }
        });
        Self {
            receiver,
            textures: HashMap::new(),
        }
    }

    /// Takes the previews that came and returns the textures.
    pub fn textures(&mut self, ctx: &egui::Context) -> &HashMap<Source, TextureHandle> {
        for (source, preview) in self.receiver.try_iter() {
            let size = [preview.width as usize, preview.height as usize];
            let image = ColorImage::from_rgba_unmultiplied(size, &preview.rgba);
            let texture = ctx.load_texture(source.to_string(), image, egui::TextureOptions::LINEAR);
            self.textures.insert(source, texture);
        }
        &self.textures
    }
}

/// The grid; returns the source the user picked.
pub fn picker(
    ui: &mut Ui,
    sources: &SourceList,
    chosen: &Source,
    textures: &HashMap<Source, TextureHandle>,
) -> Option<Source> {
    let (screens, windows) = cards(sources);
    ui.label(
        egui::RichText::new(text::TITLE)
            .font(fonts::text(13.0, Weight::Semibold))
            .color(TEXT),
    );
    ui.add_space(S2);
    if screens.is_empty() && windows.is_empty() {
        ui.label(egui::RichText::new(text::EMPTY).color(TEXT_3));
        return None;
    }
    let mut picked = None;
    egui::ScrollArea::vertical()
        .max_height(MAX_HEIGHT - 60.0)
        .show(ui, |ui| {
            for (caption, cards) in [(text::SCREENS, &screens), (text::WINDOWS, &windows)] {
                if cards.is_empty() {
                    continue;
                }
                ui.label(egui::RichText::new(caption).size(FS_S).color(TEXT_3));
                if let Some(source) = grid(ui, cards, chosen, textures) {
                    picked = Some(source);
                }
                ui.add_space(S3);
            }
        });
    picked
}

fn grid(
    ui: &mut Ui,
    cards: &[Card],
    chosen: &Source,
    textures: &HashMap<Source, TextureHandle>,
) -> Option<Source> {
    let width = (ui.available_width() - S2 * (COLUMNS - 1) as f32) / COLUMNS as f32;
    let mut picked = None;
    for row in cards.chunks(COLUMNS) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = S2;
            for card in row {
                let response = card_ui(
                    ui,
                    card,
                    width,
                    card.source == *chosen,
                    textures.get(&card.source),
                );
                if response.clicked() {
                    picked = Some(card.source);
                }
            }
        });
    }
    picked
}

fn card_ui(
    ui: &mut Ui,
    card: &Card,
    width: f32,
    chosen: bool,
    texture: Option<&TextureHandle>,
) -> Response {
    let thumb_height = (width - 12.0) * 9.0 / 16.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, thumb_height + 12.0 + 34.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, chosen, &card.title));
    let painter = ui.painter();
    let (fill, border) = match (chosen, response.hovered()) {
        (true, _) => (ACCENT_SOFT, ACCENT_HI),
        (false, true) => (SURFACE_3, Color32::TRANSPARENT),
        (false, false) => (SURFACE_2, Color32::TRANSPARENT),
    };
    painter.rect(rect, R2, fill, Stroke::new(2.0, border), StrokeKind::Inside);
    let thumb = Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(width - 12.0, thumb_height));
    painter.rect_filled(thumb, 5, Color32::from_rgb(0x11, 0x11, 0x11));
    if let Some(texture) = texture {
        let size = texture.size_vec2();
        let scale = (thumb.width() / size.x).min(thumb.height() / size.y);
        let image = Rect::from_center_size(thumb.center(), size * scale);
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), image, uv, Color32::WHITE);
    }
    let font = fonts::text(FS_S, Weight::Regular);
    let mut job = egui::text::LayoutJob::single_section(
        card.title.clone(),
        egui::TextFormat::simple(font, TEXT),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(thumb.width());
    painter.galley(
        pos2(thumb.left(), thumb.bottom() + 6.0),
        painter.layout_job(job),
        TEXT,
    );
    painter.text(
        pos2(thumb.left(), thumb.bottom() + 22.0),
        egui::Align2::LEFT_TOP,
        &card.sub,
        fonts::text(11.0, Weight::Regular),
        TEXT_3,
    );
    response
}
