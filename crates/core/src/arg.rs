//! Time arguments of ops, as an agent types them on the command line or in a plan: a moment
//! (`12.5`, `1:02.5` or a number of seconds), a range (`10.2-12.5`) or a list of ranges
//! (`10-12,1:20-1:21.5`). The project file keeps plain microseconds; these types exist only for input.

use std::borrow::Cow;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer};

use crate::time::{Range, Time, Timeline};

/// Text that is not `start-end`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "не понимаю диапазон «{text}» ({reason}): пиши начало-конец, например 10.2-12.5 или 1:02-1:05.5"
)]
pub struct ParseRangeError {
    text: String,
    reason: String,
}

impl<K: Timeline> FromStr for Range<K> {
    type Err = ParseRangeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = |reason: String| ParseRangeError {
            text: text.to_owned(),
            reason,
        };
        let (start, end) = text
            .split_once('-')
            .ok_or_else(|| error("нет дефиса между началом и концом".to_owned()))?;
        let start: Time<K> = start.parse().map_err(|e| error(format!("{e}")))?;
        let end: Time<K> = end.parse().map_err(|e| error(format!("{e}")))?;
        Range::new(start, end).map_err(|e| error(e.to_string()))
    }
}

/// A moment: `12.5`, `1:02.5`, `1:02:03` or a JSON number of seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeArg<K: Timeline>(pub Time<K>);

/// A range `start-end`, e.g. `10.2-12.5`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeArg<K: Timeline>(pub Range<K>);

/// Ranges separated by commas, e.g. `10-12,20-21`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangesArg<K: Timeline>(pub Vec<Range<K>>);

#[derive(Deserialize)]
#[serde(untagged)]
enum Raw<'a> {
    Seconds(f64),
    #[serde(borrow)]
    Text(Cow<'a, str>),
}

impl<'de, K: Timeline> Deserialize<'de> for TimeArg<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Raw::deserialize(deserializer)? {
            Raw::Seconds(secs) if secs.is_finite() => Ok(Self(Time::from_secs_f64(secs))),
            Raw::Seconds(secs) => Err(serde::de::Error::custom(format!(
                "время {secs} — не число секунд"
            ))),
            Raw::Text(text) => text.parse().map(Self).map_err(serde::de::Error::custom),
        }
    }
}

impl<'de, K: Timeline> Deserialize<'de> for RangeArg<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Cow::<str>::deserialize(deserializer)?;
        text.parse().map(Self).map_err(serde::de::Error::custom)
    }
}

impl<'de, K: Timeline> Deserialize<'de> for RangesArg<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Cow::<str>::deserialize(deserializer)?;
        text.split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

impl<K: Timeline> JsonSchema for TimeArg<K> {
    fn schema_name() -> Cow<'static, str> {
        "TimeArg".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": ["number", "string"],
            "description": "Время: секунды (12.5), M:SS (1:02.5) или H:MM:SS",
        })
    }
}

impl<K: Timeline> JsonSchema for RangeArg<K> {
    fn schema_name() -> Cow<'static, str> {
        "RangeArg".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "Диапазон начало-конец: 10.2-12.5 или 1:02-1:05.5",
        })
    }
}

impl<K: Timeline> JsonSchema for RangesArg<K> {
    fn schema_name() -> Cow<'static, str> {
        "RangesArg".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "Диапазоны через запятую: 10-12,1:20-1:21.5",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{OutputTime, Range};

    fn t(secs: f64) -> OutputTime {
        OutputTime::from_secs_f64(secs)
    }

    #[test]
    fn moments_from_text_or_seconds() {
        let parse = |json: &str| serde_json::from_str::<TimeArg<crate::time::Output>>(json);
        assert_eq!(parse("12.5").unwrap().0, t(12.5));
        assert_eq!(parse(r#""1:02.5""#).unwrap().0, t(62.5));
        let error = parse(r#""полминуты""#).unwrap_err().to_string();
        assert!(error.contains("пиши секундами"), "{error}");
    }

    #[test]
    fn ranges_and_lists() {
        let range: Range<crate::time::Output> = "1:00-1:02.5".parse().unwrap();
        assert_eq!((range.start(), range.end()), (t(60.0), t(62.5)));
        let list: RangesArg<crate::time::Output> =
            serde_json::from_str(r#""10-12,20.5-21""#).unwrap();
        assert_eq!(list.0.len(), 2);
        assert_eq!(list.0[1].start(), t(20.5));
        for bad in ["12", "12-10", "a-3", "-3", "10-12,"] {
            let error = bad.parse::<Range<crate::time::Output>>();
            let list = serde_json::from_value::<RangesArg<crate::time::Output>>(bad.into());
            assert!(error.is_err() || list.is_err(), "{bad}");
        }
        let error = "12-10".parse::<Range<crate::time::Output>>().unwrap_err();
        assert!(error.to_string().contains("пиши начало-конец"), "{error}");
    }
}
