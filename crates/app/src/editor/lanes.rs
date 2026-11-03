//! Registration of the effects on the timeline (docs/ARCHITECTURE.md, «Дорожка эффекта»): the
//! lane of each effect with regions. The editor finds the lane of an item by its prefix; the
//! commands of the gestures are the lane's. The editor reaches an effect only through its seam
//! module, re-exported here.

use dc_capture::input::{Button, Click};
use dc_core::Project;
use dc_core::project::Media;
use dc_fx_annotations::Kind;
pub use dc_fx_annotations::lane as annotations;
pub use dc_fx_callout::lane as callout;
pub use dc_fx_captions::lane as captions;
pub use dc_fx_cursor::lane as cursor;
use dc_fx_cursor::panel::PressButton;
pub use dc_fx_keys::lane as keys;
pub use dc_fx_zoom::lane as zoom;
use dc_ui_kit::lane::{Item, Lane};
use serde_json::{Map, Value};

const LANES: [&Lane; 5] = [
    &dc_fx_zoom::lane::LANE,
    &dc_fx_captions::lane::LANE,
    &dc_fx_callout::lane::LANE,
    &dc_fx_annotations::lane::LANE,
    &dc_fx_audio_tracks::lane::LANE,
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

/// The fields of an annotation of `kind` with the defaults of its kind's crate (the key tips,
/// `annotations::tip`).
pub fn kind_fields(kind: Kind, body: &Map<String, Value>) -> Option<Map<String, Value>> {
    match kind {
        Kind::Text => annotations::fields::<dc_fx_text::TextBody>(body),
        Kind::Shape => annotations::fields::<dc_fx_shape::ShapeBody>(body),
        Kind::Image => annotations::fields::<dc_fx_image::ImageBody>(body),
        Kind::Drawing => annotations::fields::<dc_fx_drawing::DrawingBody>(body),
        Kind::Blur => annotations::fields::<dc_fx_blur::BlurBody>(body),
    }
}

/// The lane of `item`.
pub fn of(item: Item) -> Option<&'static Lane> {
    LANES.into_iter().find(|lane| lane.prefix == item.prefix())
}

/// The item whose id `devcast` writes as `text` (`s3`).
pub fn parse(text: &str) -> Option<Item> {
    Item::parse(text, LANES.iter().map(|lane| lane.prefix))
}
