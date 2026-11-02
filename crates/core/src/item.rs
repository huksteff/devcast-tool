//! Objects an effect shows on its timeline track (zooms, annotations, captions, extra sound),
//! described one way for `devcast ls`: id, where it is and the rest of their fields.

use serde::Serialize;
use serde::ser::Error as _;
use serde_json::{Map, Value};

use crate::id::{Id, IdKind};
use crate::time::{Output, Range, Source, SourceTime, TimeDelta};
use crate::timemap::TimeMap;

/// Where an object of a track is.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    /// On a recording (`Media.path`), in its source time: cutting moves it with the frames.
    Source { media: String, range: Range<Source> },
    /// From a moment of a recording, lasting output time: cutting moves its start with the frames,
    /// a sped-up part does not shorten it (callout events).
    Lasting {
        media: String,
        at: SourceTime,
        duration: TimeDelta,
    },
    /// On the output timeline: cutting does not move it (extra sound tracks).
    Output(Range<Output>),
}

impl Place {
    /// Where it plays on the output; `None` if it is cut out or its recording is not on the track.
    pub fn output(&self, map: &TimeMap) -> Option<Range<Output>> {
        match self {
            Self::Source { media, range } => map.output_span(media, *range),
            Self::Lasting {
                media,
                at,
                duration,
            } => map.lasting(media, *at, *duration),
            Self::Output(range) => Some(*range),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    /// The list of `devcast ls <list>` it belongs to, e.g. `zooms`.
    pub list: &'static str,
    pub place: Place,
    /// The other fields of the object, as stored in the project.
    pub fields: Map<String, Value>,
}

impl Item {
    /// `object` as stored, without its `id`, `media` and `range` fields, which the item has itself.
    /// Fails if `object` does not serialize to a JSON object.
    pub fn of<K: IdKind>(
        id: Id<K>,
        media: &str,
        range: Range<Source>,
        object: &impl Serialize,
    ) -> Result<Self, serde_json::Error> {
        let media = media.to_owned();
        Self::placed(id, Place::Source { media, range }, object)
    }

    /// An object starting at `at` of recording `media` and lasting `duration` of output time.
    pub fn lasting<K: IdKind>(
        id: Id<K>,
        media: &str,
        at: SourceTime,
        duration: TimeDelta,
        object: &impl Serialize,
    ) -> Result<Self, serde_json::Error> {
        let media = media.to_owned();
        let place = Place::Lasting {
            media,
            at,
            duration,
        };
        Self::placed(id, place, object)
    }

    /// An object of the output timeline at `range`; its `id` and `range` fields are left out.
    pub fn on_output<K: IdKind>(
        id: Id<K>,
        range: Range<Output>,
        object: &impl Serialize,
    ) -> Result<Self, serde_json::Error> {
        Self::placed(id, Place::Output(range), object)
    }

    fn placed<K: IdKind>(
        id: Id<K>,
        place: Place,
        object: &impl Serialize,
    ) -> Result<Self, serde_json::Error> {
        let Value::Object(mut fields) = serde_json::to_value(object)? else {
            return Err(serde_json::Error::custom(format!(
                "{id}: объект дорожки должен быть JSON-объектом"
            )));
        };
        for key in ["id", "media", "range"] {
            fields.remove(key);
        }
        Ok(Self {
            id: id.to_string(),
            list: K::LIST,
            place,
            fields,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ClipKind;
    use crate::time::SourceTime;

    #[derive(Serialize)]
    struct Zoomish {
        id: String,
        media: String,
        range: Range<Source>,
        scale: f64,
    }

    #[test]
    fn item_keeps_only_the_other_fields() {
        let range = Range::new(SourceTime::ZERO, SourceTime::from_secs_f64(1.0)).unwrap();
        let object = Zoomish {
            id: "c4".to_owned(),
            media: "media/a.mp4".to_owned(),
            range,
            scale: 2.0,
        };
        let item = Item::of(
            Id::<ClipKind>::from_number(4),
            "media/a.mp4",
            range,
            &object,
        )
        .unwrap();
        assert_eq!((item.id.as_str(), item.list), ("c4", "clips"));
        assert_eq!(
            Value::Object(item.fields),
            serde_json::json!({"scale": 2.0})
        );
    }
}
