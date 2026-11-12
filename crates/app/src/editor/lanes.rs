//! Registration of the effects on the timeline and the preview (docs/ARCHITECTURE.md, «Дорожка
//! эффекта»): the lane of each effect with regions, the preview gestures, the kinds of
//! annotations. The editor finds the lane of an item by its prefix; the commands of the gestures
//! are the lane's. The editor reaches an effect only through its seam modules, re-exported here.

use dc_capture::input::{Button, Click};
use dc_core::Project;
use dc_core::project::Media;
pub use dc_fx_annotations::lane as annotations;
use dc_fx_annotations::lane::Kind;
use dc_fx_annotations::panel::Entry;
pub use dc_fx_audio_tracks::lane as audio;
pub use dc_fx_callout::lane as callout;
pub use dc_fx_captions::lane as captions;
pub use dc_fx_crop::stage as frame;
pub use dc_fx_cursor::lane as cursor;
use dc_fx_cursor::panel::PressButton;
pub use dc_fx_cut::lane as clips;
pub use dc_fx_keys::lane as keys;
pub use dc_fx_zoom::lane as zoom;
use dc_ui_kit::lane::{Item, Lane};
use dc_ui_kit::stage::Stage;
use serde_json::{Map, Value};

const LANES: [&Lane; 5] = [
    &dc_fx_zoom::lane::LANE,
    &dc_fx_captions::lane::LANE,
    &dc_fx_callout::lane::LANE,
    &dc_fx_annotations::lane::LANE,
    &dc_fx_audio_tracks::lane::LANE,
];

/// The kinds of annotations: the panel section and the fields of each.
static KINDS: [&Entry; 5] = [
    &dc_fx_text::panel::KIND,
    &dc_fx_shape::panel::KIND,
    &dc_fx_image::panel::KIND,
    &dc_fx_drawing::panel::KIND,
    &dc_fx_blur::panel::KIND,
];

/// The gestures on the preview, in the order the pointer is offered to them: the picked zoom's
/// frame takes it first, the brush in a picked drawing before the boxes of the annotations.
pub static STAGES: [&dyn Stage; 3] = [
    &dc_fx_zoom::stage::ZoomFrame,
    &dc_fx_drawing::stage::Strokes,
    &dc_fx_annotations::stage::Boxes,
];

/// The dots of the presses `of` gives for each recording of `project`, in time order.
pub fn click_dots<'a>(
    project: &Project,
    of: impl Fn(&Media) -> Option<&'a Vec<Click>>,
) -> Vec<cursor::Dot> {
    let recordings = super::presses::recordings(project)
        .into_iter()
        .map(|media| {
            let presses = of(&media)
                .into_iter()
                .flatten()
                .map(|click| cursor::Pressed {
                    at: click.at,
                    button: match click.button {
                        Button::Left => PressButton::Left,
                        Button::Right => PressButton::Right,
                        Button::Middle => PressButton::Middle,
                        Button::X1 => PressButton::Back,
                        Button::X2 => PressButton::Forward,
                    },
                    double: click.double,
                });
            (media.path.clone(), presses.collect())
        });
    cursor::dots(project, recordings.collect())
}

/// The registration of annotation kind `kind`.
pub fn kind(kind: Kind) -> Option<&'static Entry> {
    KINDS.into_iter().find(|entry| entry.kind == kind)
}

/// The fields of an annotation of `kind` with the defaults of its kind's crate (the key tips,
/// `annotations::tip`).
pub fn kind_fields(kind: Kind, body: &Map<String, Value>) -> Option<Map<String, Value>> {
    (self::kind(kind)?.fields)(body)
}

/// The lane of `item`.
pub fn of(item: Item) -> Option<&'static Lane> {
    LANES.into_iter().find(|lane| lane.prefix == item.prefix())
}

/// The item whose id `devcast` writes as `text` (`s3`).
pub fn parse(text: &str) -> Option<Item> {
    Item::parse(text, LANES.iter().map(|lane| lane.prefix))
}
