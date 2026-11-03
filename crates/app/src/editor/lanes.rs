//! Registration of the effects on the timeline (docs/ARCHITECTURE.md, «Дорожка эффекта»): the
//! lane of each effect with regions. The editor finds the lane of an item by its prefix; the
//! commands of the gestures are the lane's. The editor reaches an effect only through its seam
//! module, re-exported here.

pub use dc_fx_zoom::lane as zoom;
use dc_ui_kit::lane::{Item, Lane};

const LANES: [&Lane; 5] = [
    &dc_fx_zoom::lane::LANE,
    &dc_fx_captions::lane::LANE,
    &dc_fx_callout::lane::LANE,
    &dc_fx_annotations::lane::LANE,
    &dc_fx_audio_tracks::lane::LANE,
];

/// The lane of `item`.
pub fn of(item: Item) -> Option<&'static Lane> {
    LANES.into_iter().find(|lane| lane.prefix == item.prefix())
}

/// The item whose id `devcast` writes as `text` (`s3`).
pub fn parse(text: &str) -> Option<Item> {
    Item::parse(text, LANES.iter().map(|lane| lane.prefix))
}
