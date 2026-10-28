//! Times and ranges as command arguments, written the way a person types them: `12.5`, `1:02.5`,
//! `10.2-12.5`, `1-2,5-6`. The project file keeps microseconds; these types are only for
//! [`crate::Op`] fields, so the command line, plans and `help --json` read times the same way.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};

use crate::time::{Range, Time, Timeline};

/// A moment: seconds as a number (`12.5`) or text (`12.5`, `1:02.5`, `1:02:03`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeArg<K: Timeline>(pub Time<K>);

/// A range `start-end`, each end as in [`TimeArg`]: `10.2-12.5`, `1:02-1:05.5`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeArg<K: Timeline>(pub Range<K>);

/// Ranges separated by commas: `10.2-12.5,20-21`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeListArg<K: Timeline>(pub Vec<Range<K>>);

/// Text that is not a range `start-end`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseRangeError {
    #[error("не понимаю диапазон «{0}»: пиши начало-конец, например 10.2-12.5 или 1:02-1:05.5")]
    Format(String),
    #[error("диапазон «{text}»: {reason}")]
    Invalid { text: String, reason: String },
}

impl<K: Timeline> FromStr for Range<K> {
    type Err = ParseRangeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let format = || ParseRangeError::Format(text.to_owned());
        let (start, end) = text.split_once('-').ok_or_else(format)?;
        if start.trim().is_empty() || end.contains('-') {
            return Err(format());
        }
        let invalid = |reason: String| ParseRangeError::Invalid {
            text: text.to_owned(),
            reason,
        };
        let start: Time<K> = start.parse().map_err(|e| invalid(format!("{e}")))?;
        let end: Time<K> = end.parse().map_err(|e| invalid(format!("{e}")))?;
        Self::new(start, end).map_err(|e| invalid(e.to_string()))
    }
}

impl<'de, K: Timeline> Deserialize<'de> for TimeArg<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(TimeVisitor(std::marker::PhantomData))
    }
}

struct TimeVisitor<K>(std::marker::PhantomData<K>);

impl<K: Timeline> Visitor<'_> for TimeVisitor<K> {
    type Value = TimeArg<K>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("время: секунды (12.5) или текст (1:02.5)")
    }

    fn visit_f64<E: de::Error>(self, secs: f64) -> Result<Self::Value, E> {
        Ok(TimeArg(Time::from_secs_f64(secs)))
    }

    fn visit_i64<E: de::Error>(self, secs: i64) -> Result<Self::Value, E> {
        self.visit_f64(secs as f64)
    }

    fn visit_u64<E: de::Error>(self, secs: u64) -> Result<Self::Value, E> {
        self.visit_f64(secs as f64)
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Self::Value, E> {
        text.parse().map(TimeArg).map_err(E::custom)
    }
}

impl<'de, K: Timeline> Deserialize<'de> for RangeArg<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Cow::<str>::deserialize(deserializer)?;
        text.parse().map(RangeArg).map_err(de::Error::custom)
    }
}

impl<'de, K: Timeline> Deserialize<'de> for RangeListArg<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Cow::<str>::deserialize(deserializer)?;
        text.split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map(RangeListArg)
            .map_err(de::Error::custom)
    }
}

impl<K: Timeline> JsonSchema for TimeArg<K> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "TimeArg".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": ["string", "number"],
            "description": "Время: секунды (12.5) или M:SS (1:02.5)",
        })
    }
}

impl<K: Timeline> JsonSchema for RangeArg<K> {
    fn inline_schema() -> bool {
        true
    }

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

impl<K: Timeline> JsonSchema for RangeListArg<K> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "RangeListArg".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "Диапазоны через запятую: 10.2-12.5,20-21",
        })
    }
}

#[cfg(test)]
#[path = "arg_tests.rs"]
mod tests;
