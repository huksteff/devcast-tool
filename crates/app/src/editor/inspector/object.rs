//! The panel of the object picked on the timeline: a zoom, a cue, a callout event, an annotation
//! (also through one of its keys), an extra sound, a clip or a click (its own panel,
//! `dc_fx_cursor::panel::click`, with the press of its dot as [`Press`]). The frame gives each
//! its header — the colour of its row, its name, where it is — and «Удалить» (the same command
//! as Delete); a zoom gets its parameters (`dc_fx_zoom::panel::region`), a callout event its own
//! (`dc_fx_callout::panel`), a cue its text and times (`dc_fx_captions::panel::cue`), an
//! annotation its key strip and groups (`dc_fx_annotations::panel` with the section of its kind,
//! [`annotation`]) and «Дублировать» (`annot paste` of a copy where it is, on top of its row;
//! the copy gets picked), a clip its sound and an extra
//! sound its levels and place (`dc_fx_audio_tracks::panel::clip`, `extra`, with the facts of
//! [`Cx::sources`]) and «Дублировать» (`audio paste` on the first free row).

use dc_capture::input::Button;
use dc_core::Range;
use dc_core::time::Output;
use dc_fx_annotations::panel::{self as annot, Region as Shown};
use dc_fx_annotations::{AnnotId, Kind};
use dc_fx_audio_tracks::AudioId;
use dc_fx_audio_tracks::panel as sound;
use dc_fx_callout::EventId;
use dc_fx_callout::panel as callout;
use dc_fx_captions::CueId;
use dc_fx_captions::panel as cue;
use dc_fx_cursor::panel::{self as cursor, Press, PressButton};
use dc_fx_zoom::ZoomId;
use dc_fx_zoom::panel as zoom;
use dc_ui_kit::icons::Icon;
use dc_ui_kit::panel::{Header, header, header_button};
use dc_ui_kit::strings::{annotations, editor as text, inspector as names, timeline as rows_text};

use super::Cx;
use crate::editor::selection::{Item, Selected, same_moment};
use crate::editor::timeline::{self, Dot, Region, Row, Tracks, timecode};
use crate::editor::{Action, clipboard, montage, regions};
use eframe::egui::Ui;

pub fn ui(ui: &mut Ui, cx: &mut Cx<'_>, tracks: &Tracks, picked: Selected) {
    let picked = match picked {
        Selected::Key { id, .. } => Selected::Region(Item::of(id)),
        Selected::Click(at) => {
            let dot = tracks.clicks.iter().find(|d| same_moment(d.at, at));
            if let Some(dot) = dot {
                cursor::click(ui, &mut cx.base, &press(dot), &timecode(dot.at));
            }
            return;
        }
        other => other,
    };
    // The selection keeps only what the rows show, so the object is there.
    let Some((row, region)) = find(tracks, picked) else {
        return;
    };
    let mut about = describe(picked, row, region);
    let object = match picked {
        Selected::Region(item) => Object::of(item),
        _ => None,
    };
    if let Some(Object::Callout(id)) = object
        && let Some((title, subtitle)) =
            callout::about(cx.base.project, id, &timecode(region.range.start()))
    {
        about.title = title.to_owned();
        about.subtitle = subtitle;
    }
    let head = Header {
        hue: timeline::hue(row),
        title: &about.title,
        subtitle: &about.subtitle,
    };
    let copy = match (picked, object) {
        (Selected::Region(item), Some(Object::Annotation(_) | Object::Sound(_))) => {
            clipboard::duplicate(cx.base.project, item, region.range)
        }
        _ => None,
    };
    header(ui, &head, |ui| {
        if header_button(ui, Icon::Trash, names::DELETE, true)
            && let Some(action) = delete(picked)
        {
            cx.actions.push(action);
        }
        if let Some(step) = copy
            && header_button(ui, Icon::Copy, annotations::DUPLICATE, false)
        {
            cx.actions.push(Action::Paste(vec![step]));
        }
    });
    if let Selected::Clip(id) = picked {
        let sources = cx.sources(Some(id));
        sound::clip(ui, &mut cx.base, id, &sources);
    }
    match object {
        Some(Object::Zoom(id)) => zoom::region(ui, &mut cx.base, id),
        Some(Object::Callout(id)) => callout::ui(ui, &mut cx.base, id),
        Some(Object::Cue(id)) => cue::cue(ui, &mut cx.base, id, region.range),
        Some(Object::Annotation(id)) => annotation(ui, cx, id, region),
        Some(Object::Sound(id)) => {
            let sources = cx.sources(None);
            sound::extra(ui, &mut cx.base, id, &sources);
        }
        // A key opens its annotation and a click its own panel, above.
        None => {}
    }
}

/// The effect object of a picked region, by the kind of its id.
#[derive(Clone, Copy)]
enum Object {
    Zoom(ZoomId),
    Callout(EventId),
    Cue(CueId),
    Annotation(AnnotId),
    Sound(AudioId),
}

impl Object {
    fn of(item: Item) -> Option<Self> {
        item.id()
            .map(Self::Zoom)
            .or_else(|| item.id().map(Self::Callout))
            .or_else(|| item.id().map(Self::Cue))
            .or_else(|| item.id().map(Self::Annotation))
            .or_else(|| item.id().map(Self::Sound))
    }
}

/// The row and the region of `picked`.
fn find(tracks: &Tracks, picked: Selected) -> Option<(Row, &Region)> {
    tracks.rows.iter().find_map(|track| {
        let region = track.regions.iter().find(|r| match picked {
            Selected::Clip(id) => r.clip == Some(id),
            Selected::Region(item) => r.item == Some(item),
            Selected::Key { .. } | Selected::Click(_) => false,
        })?;
        Some((track.row, region))
    })
}

/// What the header says of an object.
struct About {
    title: String,
    subtitle: String,
}

fn describe(picked: Selected, row: Row, region: &Region) -> About {
    let span = span(region.range);
    let length = rows_text::seconds(region.range.duration().as_secs_f64());
    let timed = |title: &str| About {
        title: title.to_owned(),
        subtitle: names::with_length(&span, &length),
    };
    let layered = || About {
        title: region.label.clone(),
        subtitle: names::on_row(&timeline::name(row), &span),
    };
    match picked {
        Selected::Clip(_) => About {
            title: region.label.clone(),
            subtitle: region.detail.clone().unwrap_or(length),
        },
        Selected::Region(item) => match Object::of(item) {
            Some(Object::Zoom(_)) => timed(text::ZOOM),
            Some(Object::Callout(_)) => timed(names::CALLOUT),
            Some(Object::Cue(_)) => timed(names::CUE),
            Some(Object::Annotation(_) | Object::Sound(_)) | None => layered(),
        },
        Selected::Key { .. } => layered(),
        Selected::Click(_) => timed(names::CLICK),
    }
}

/// «0:41.00 – 0:48.00».
fn span(range: Range<Output>) -> String {
    rows_text::span(&timecode(range.start()), &timecode(range.end()))
}

/// The command that removes `picked`, as Delete does.
fn delete(picked: Selected) -> Option<Action> {
    match picked {
        Selected::Clip(id) => montage::remove([id]),
        Selected::Region(item) => Some(Action::Apply(vec![regions::remove(item)?])),
        Selected::Key { .. } | Selected::Click(_) => None,
    }
}

/// The facts of the click panel from the dot of a press.
fn press(dot: &Dot) -> Press {
    Press {
        at: dot.at,
        media: dot.media.clone(),
        source: dot.source,
        button: match dot.button {
            Button::Left => PressButton::Left,
            Button::Right => PressButton::Right,
            Button::Middle => PressButton::Middle,
            Button::X1 => PressButton::Back,
            Button::X2 => PressButton::Forward,
        },
        double: dot.double,
    }
}

/// The panel of annotation `id` (`dc_fx_annotations::panel`) with the section of its kind's
/// crate, at the place and keys of its region and on the output frame after the crop.
fn annotation(ui: &mut Ui, cx: &mut Cx<'_>, id: AnnotId, region: &Region) {
    let project = cx.base.project;
    let Ok(crop) = project.state::<dc_fx_crop::Crop>() else {
        return;
    };
    let shown = Shown {
        range: region.range,
        keys: &region.keys,
        canvas: dc_fx_crop::canvas(project, &crop.settings),
    };
    let cx = &mut cx.base;
    match annot::kind(project, id) {
        Some(Kind::Text) => annot::ui::<dc_fx_text::TextBody>(ui, cx, id, &shown),
        Some(Kind::Shape) => annot::ui::<dc_fx_shape::ShapeBody>(ui, cx, id, &shown),
        Some(Kind::Image) => annot::ui::<dc_fx_image::ImageBody>(ui, cx, id, &shown),
        Some(Kind::Drawing) => annot::ui::<dc_fx_drawing::DrawingBody>(ui, cx, id, &shown),
        Some(Kind::Blur) => annot::ui::<dc_fx_blur::BlurBody>(ui, cx, id, &shown),
        None => {}
    }
}
