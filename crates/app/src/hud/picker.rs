//! Source picker (mockup 2): a grid of previews of monitors and windows, four in a row, the
//! chosen one outlined in blue. The popover opens at once with the sources known; the worker
//! thread takes their previews one by one (one WGC frame each, as the recording takes them), then
//! lists the sources anew (asking WGC about every window takes a few hundred ms) and takes the
//! previews of the new ones; all appear as they come.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver};

use dc_capture::{Preview, Source, SourceList};
use dc_ui_kit::Icon;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::kit;
use dc_ui_kit::strings::picker as text;
use dc_ui_kit::tokens::{
    ACCENT_HI, ACCENT_SOFT, FS_S, R2, S1, S2, S3, S4, SURFACE_2, SURFACE_3, TEXT, TEXT_2, TEXT_3,
};
use dc_ui_kit::widgets::IconButton;
use eframe::egui::{
    self, Color32, ColorImage, Margin, Rect, Response, Sense, Stroke, StrokeKind, TextureHandle,
    Ui, WidgetInfo, pos2, vec2,
};

/// Width of the picker popover, points.
pub const WIDTH: f32 = 720.0;
/// Its height limit; more scrolls.
pub const MAX_HEIGHT: f32 = super::popover::MAX_HEIGHT;
/// Padding of the picker panel (mockup 2: 12 above, 16 at the sides and below the cards).
pub const PADDING: Margin = Margin {
    left: S4 as i8,
    right: S4 as i8,
    top: S3 as i8,
    bottom: S4 as i8,
};
/// Longer side of a preview, pixels.
const PREVIEW_SIDE: u32 = 320;
const COLUMNS: usize = 4;
/// Side of the icon before a card title, points (mockup: 14 px).
const TITLE_ICON: f32 = 14.0;

/// One card of the grid.
pub struct Card {
    pub source: Source,
    pub title: String,
    pub sub: String,
    /// Before the title: monitor, window or area.
    pub icon: Icon,
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
            sub: text::screen_size(m.bounds.width(), m.bounds.height(), m.scale),
            icon: Icon::Monitor,
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
            icon: Icon::Window,
        })
        .collect();
    (screens, windows)
}

/// The sources of the cards of `list`, screens first.
fn card_sources(list: &SourceList) -> Vec<Source> {
    let (screens, windows) = cards(list);
    screens
        .iter()
        .chain(&windows)
        .map(|card| card.source)
        .collect()
}

/// Takes and sends the previews of `sources` not `taken` yet; `false` once nobody listens.
fn send_previews(
    sources: &[Source],
    taken: &mut HashSet<Source>,
    sender: &mpsc::Sender<Arrival>,
    wake: &dyn Fn(),
) -> bool {
    for &source in sources {
        if !taken.insert(source) {
            continue;
        }
        let preview = match source {
            Source::Monitor(index) => dc_capture::win::preview_monitor(index, PREVIEW_SIDE),
            Source::Window(id) => dc_capture::win::preview_window(id, PREVIEW_SIDE),
            Source::Area(_) => continue,
        };
        match preview {
            Ok(preview) => {
                if sender.send(Arrival::Preview(source, preview)).is_err() {
                    return false;
                }
                wake();
            }
            Err(error) => tracing::info!("нет превью {source}: {error}"),
        }
    }
    true
}

/// What the worker sends: previews of the known sources, the fresh list, previews of new ones.
enum Arrival {
    Listed(Result<SourceList, String>),
    Preview(Source, Preview),
}

/// The fresh source list and the previews arriving from the worker, as textures by source.
pub struct Previews {
    receiver: Receiver<Arrival>,
    textures: HashMap<Source, TextureHandle>,
    /// The fresh list, until the popover takes it.
    listed: Option<Result<SourceList, String>>,
}

impl Previews {
    /// Takes the previews of the cards of `known` (the list the popover opens with), then lists
    /// the sources anew and takes the previews of the cards that list adds; each arrival wakes the
    /// UI. A failed preview leaves its card grey (a window can close meanwhile).
    pub fn start(ctx: &egui::Context, known: &SourceList) -> Self {
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        let known = card_sources(known);
        std::thread::spawn(move || {
            // The main window passes the repaint on to the popover with the cards.
            let wake = || ctx.request_repaint_of(egui::ViewportId::ROOT);
            let mut taken = HashSet::new();
            if !send_previews(&known, &mut taken, &sender, &wake) {
                return;
            }
            let listed = dc_capture::win::list_sources().map_err(|e| e.to_string());
            let fresh = listed.as_ref().map(card_sources).unwrap_or_default();
            if sender.send(Arrival::Listed(listed)).is_err() {
                return;
            }
            wake();
            send_previews(&fresh, &mut taken, &sender, &wake);
        });
        Self {
            receiver,
            textures: HashMap::new(),
            listed: None,
        }
    }

    /// The textures of the previews that came.
    pub fn textures(&self) -> &HashMap<Source, TextureHandle> {
        &self.textures
    }

    /// Takes what came: the previews become textures; the fresh source list, once it came, is
    /// returned (once).
    pub fn receive(&mut self, ctx: &egui::Context) -> Option<Result<SourceList, String>> {
        for arrival in self.receiver.try_iter() {
            let (source, preview) = match arrival {
                Arrival::Listed(listed) => {
                    self.listed = Some(listed);
                    continue;
                }
                Arrival::Preview(source, preview) => (source, preview),
            };
            let size = [preview.width as usize, preview.height as usize];
            let image = ColorImage::from_rgba_unmultiplied(size, &preview.rgba);
            let texture = ctx.load_texture(source.to_string(), image, egui::TextureOptions::LINEAR);
            self.textures.insert(source, texture);
        }
        self.listed.take()
    }
}

/// What the user picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Source(Source),
    /// «Область…»: select an area on the screen.
    Area,
    /// The ✕ of the header: close without a choice.
    Close,
}

/// The grid; `last_area` is the size of the last area for its card.
pub fn picker(
    ui: &mut Ui,
    sources: &SourceList,
    chosen: &Source,
    textures: &HashMap<Source, TextureHandle>,
    last_area: Option<(u32, u32)>,
) -> Option<Pick> {
    let (screens, windows) = cards(sources);
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(text::TITLE)
                .font(fonts::text(13.0, Weight::Semibold))
                .color(TEXT),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(IconButton::new(Icon::Close, kit::CLOSE)).clicked() {
                picked = Some(Pick::Close);
            }
        });
    });
    ui.add_space(S1);
    if screens.is_empty() && windows.is_empty() {
        ui.label(egui::RichText::new(text::EMPTY).color(TEXT_3));
        return picked;
    }
    egui::ScrollArea::vertical()
        .max_height(MAX_HEIGHT - 76.0)
        .show(ui, |ui| {
            let groups = [(text::SCREENS, &screens), (text::WINDOWS, &windows)];
            let shown = groups.into_iter().filter(|(_, cards)| !cards.is_empty());
            for (index, (caption, cards)) in shown.enumerate() {
                if index > 0 {
                    ui.add_space(S3);
                }
                ui.label(egui::RichText::new(caption).size(FS_S).color(TEXT_3));
                let area = (caption == text::SCREENS).then_some(last_area);
                if let Some(pick) = grid(ui, cards, chosen, textures, area) {
                    picked = Some(pick);
                }
            }
        });
    picked
}

/// Cards in rows of four; `area` adds the «Область…» card (with the last area's size).
fn grid(
    ui: &mut Ui,
    cards: &[Card],
    chosen: &Source,
    textures: &HashMap<Source, TextureHandle>,
    area: Option<Option<(u32, u32)>>,
) -> Option<Pick> {
    let width = (ui.available_width() - S2 * (COLUMNS - 1) as f32) / COLUMNS as f32;
    let area_card = area.map(|last| Card {
        source: *chosen,
        title: text::AREA.to_owned(),
        sub: last.map_or_else(|| text::NO_AREA.to_owned(), |(w, h)| text::last_area(w, h)),
        icon: Icon::Area,
    });
    let all: Vec<&Card> = cards.iter().chain(area_card.as_ref()).collect();
    let mut picked = None;
    for row in all.chunks(COLUMNS) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = S2;
            for card in row {
                let is_area = card.title == text::AREA;
                let selected = if is_area {
                    matches!(chosen, Source::Area(_))
                } else {
                    card.source == *chosen
                };
                let texture = if is_area {
                    None
                } else {
                    textures.get(&card.source)
                };
                if card_ui(ui, card, width, selected, texture, is_area).clicked() {
                    picked = Some(if is_area {
                        Pick::Area
                    } else {
                        Pick::Source(card.source)
                    });
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
    area: bool,
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
    let thumb_fill = if area {
        SURFACE_3
    } else {
        Color32::from_rgb(0x11, 0x11, 0x11)
    };
    painter.rect_filled(thumb, 5, thumb_fill);
    if area {
        let icon = Rect::from_center_size(thumb.center(), vec2(28.0, 28.0));
        Icon::Area.paint(painter, icon, TEXT_2);
    }
    if let Some(texture) = texture {
        let size = texture.size_vec2();
        let scale = (thumb.width() / size.x).min(thumb.height() / size.y);
        let image = Rect::from_center_size(thumb.center(), size * scale);
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), image, uv, Color32::WHITE);
    }
    let line = thumb.bottom() + 6.0;
    let icon = Rect::from_min_size(pos2(thumb.left(), line), vec2(TITLE_ICON, TITLE_ICON));
    card.icon.paint(painter, icon, TEXT_2);
    let title_left = icon.right() + 6.0;
    let font = fonts::text(FS_S, Weight::Regular);
    let mut job = egui::text::LayoutJob::single_section(
        card.title.clone(),
        egui::TextFormat::simple(font, TEXT),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(thumb.right() - title_left);
    painter.galley(pos2(title_left, line), painter.layout_job(job), TEXT);
    painter.text(
        pos2(thumb.left(), thumb.bottom() + 22.0),
        egui::Align2::LEFT_TOP,
        &card.sub,
        fonts::text(11.0, Weight::Regular),
        TEXT_3,
    );
    response
}
