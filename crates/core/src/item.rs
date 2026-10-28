//! Objects an effect shows on its timeline track (zooms, annotations, captions), described one way
//! for `devcast ls`: id, recording, source range and the rest of their fields.

use serde::Serialize;
use serde_json::{Map, Value};

use crate::id::{Id, IdKind};
use crate::time::{Range, Source};

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    /// The list of `devcast ls <list>` it belongs to, e.g. `zooms`.
    pub list: &'static str,
    /// Recording (`Media.path`) whose source time `range` is in.
    pub media: String,
    pub range: Range<Source>,
    /// The other fields of the object, as stored in the project.
    pub fields: Map<String, Value>,
}

impl Item {
    /// `object` as stored, without its `id`, `media` and `range` fields, which the item has itself.
    /// `object` is a struct, so it serializes to a JSON object.
    pub fn of<K: IdKind>(
        id: Id<K>,
        media: &str,
        range: Range<Source>,
        object: &impl Serialize,
    ) -> Self {
        let mut fields = match serde_json::to_value(object) {
            Ok(Value::Object(fields)) => fields,
            _ => Map::new(),
        };
        for key in ["id", "media", "range"] {
            fields.remove(key);
        }
        Self {
            id: id.to_string(),
            list: K::LIST,
            media: media.to_owned(),
            range,
            fields,
        }
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
        );
        assert_eq!((item.id.as_str(), item.list), ("c4", "clips"));
        assert_eq!(
            Value::Object(item.fields),
            serde_json::json!({"scale": 2.0})
        );
    }
}
