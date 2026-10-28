//! Short typed ids such as `z3` (zoom) or `c1` (clip): readable in the CLI, stable in the project.
//!
//! Each object kind implements [`IdKind`] with its own prefix. An [`IdCounter`] stored in the
//! project hands out numbers and never reuses one, even after the object is deleted.

use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A kind of object that has ids. Prefixes must be unique across kinds (checked by the registry).
pub trait IdKind:
    Copy + fmt::Debug + Eq + Ord + std::hash::Hash + Default + Send + Sync + 'static
{
    /// Lower-case ASCII letters, e.g. `"z"`.
    const PREFIX: &'static str;
    /// Russian noun in the genitive case for error messages, e.g. `"зума"`.
    const NOUN: &'static str;
    /// Russian plural in the nominative case for change summaries, e.g. `"зумы"`.
    const PLURAL: &'static str;
    /// CLI command without `devcast` that lists objects of this kind, e.g. `"ls zooms"`.
    const LIST: &'static str;
}

/// Id of an object of kind `K`, shown and stored as `<prefix><number>`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id<K: IdKind> {
    number: u32,
    kind: PhantomData<K>,
}

impl<K: IdKind> Id<K> {
    pub const fn from_number(number: u32) -> Self {
        Self {
            number,
            kind: PhantomData,
        }
    }

    pub const fn number(self) -> u32 {
        self.number
    }
}

impl<K: IdKind> fmt::Display for Id<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", K::PREFIX, self.number)
    }
}

impl<K: IdKind> fmt::Debug for Id<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Text that is not an id of the expected kind.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("«{text}» — не id {noun}: ожидается {prefix}<число>, например {prefix}3")]
pub struct ParseIdError {
    text: String,
    noun: &'static str,
    prefix: &'static str,
}

impl<K: IdKind> FromStr for Id<K> {
    type Err = ParseIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = || ParseIdError {
            text: text.to_owned(),
            noun: K::NOUN,
            prefix: K::PREFIX,
        };
        let digits = text.strip_prefix(K::PREFIX).ok_or_else(error)?;
        let canonical = !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
            && !digits.starts_with('0');
        if !canonical {
            return Err(error());
        }
        digits.parse().map(Self::from_number).map_err(|_| error())
    }
}

impl<K: IdKind> Serialize for Id<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de, K: IdKind> Deserialize<'de> for Id<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Cow::<str>::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl<K: IdKind> JsonSchema for Id<K> {
    fn schema_name() -> Cow<'static, str> {
        format!("Id_{}", K::PREFIX).into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": format!("^{}[1-9][0-9]*$", K::PREFIX),
            "description": format!("Короткий id {}: {}<число>", K::NOUN, K::PREFIX),
        })
    }
}

/// Hands out ids of kind `K`: 1, 2, 3, … Stored in the project; never goes back.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IdCounter<K: IdKind> {
    next: u32,
    #[serde(skip)]
    kind: PhantomData<K>,
}

impl<K: IdKind> Default for IdCounter<K> {
    fn default() -> Self {
        Self {
            next: 1,
            kind: PhantomData,
        }
    }
}

impl<K: IdKind> fmt::Debug for IdCounter<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IdCounter({}{})", K::PREFIX, self.next)
    }
}

impl<K: IdKind> JsonSchema for IdCounter<K> {
    fn schema_name() -> Cow<'static, str> {
        format!("IdCounter_{}", K::PREFIX).into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "integer",
            "minimum": 1,
            "description": format!("Следующий свободный номер id {}", K::NOUN),
        })
    }
}

impl<K: IdKind> IdCounter<K> {
    /// Next unused id. Saturates at `u32::MAX` (4 billion objects of one kind is not a real case).
    pub fn allocate(&mut self) -> Id<K> {
        let id = Id::from_number(self.next.max(1));
        self.next = id.number.saturating_add(1);
        id
    }

    /// The number the next [`Self::allocate`] will return.
    pub fn next_number(&self) -> u32 {
        self.next.max(1)
    }

    /// After loading a project: make sure no existing id can be handed out again.
    pub fn ensure_above(&mut self, existing: impl IntoIterator<Item = Id<K>>) {
        for id in existing {
            if id.number >= self.next {
                self.next = id.number.saturating_add(1);
            }
        }
    }
}

/// True if `prefix` is a valid [`IdKind::PREFIX`]: one or more lower-case ASCII letters.
pub fn is_valid_prefix(prefix: &str) -> bool {
    !prefix.is_empty() && prefix.bytes().all(|b| b.is_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    struct Zoom;
    impl IdKind for Zoom {
        const PREFIX: &'static str = "z";
        const NOUN: &'static str = "зума";
        const PLURAL: &'static str = "зумы";
        const LIST: &'static str = "ls zooms";
    }

    #[test]
    fn display_and_parse_round_trip() {
        let id = Id::<Zoom>::from_number(12);
        assert_eq!(id.to_string(), "z12");
        assert_eq!("z12".parse::<Id<Zoom>>().unwrap(), id);
    }

    #[test]
    fn foreign_prefix_explains_expected_form() {
        let error = "a3".parse::<Id<Zoom>>().unwrap_err().to_string();
        assert_eq!(error, "«a3» — не id зума: ожидается z<число>, например z3");
    }

    #[test]
    fn rejects_non_canonical_forms() {
        for bad in ["z", "z0", "z03", "z-1", "z1a", "Z1", " z1", "z99999999999"] {
            assert!(bad.parse::<Id<Zoom>>().is_err(), "{bad}");
        }
    }

    #[test]
    fn counter_never_repeats_and_respects_loaded_ids() {
        let mut counter = IdCounter::<Zoom>::default();
        assert_eq!(counter.allocate().to_string(), "z1");
        counter.ensure_above([Id::from_number(7), Id::from_number(3)]);
        assert_eq!(counter.allocate().to_string(), "z8");
        assert_eq!(counter.allocate().to_string(), "z9");
    }

    #[test]
    fn serializes_as_string_and_validates_on_load() {
        assert_eq!(
            serde_json::to_string(&Id::<Zoom>::from_number(4)).unwrap(),
            r#""z4""#
        );
        let error = serde_json::from_str::<Id<Zoom>>(r#""c4""#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("не id зума"), "{error}");
    }

    #[test]
    fn schema_has_prefix_pattern() {
        let schema = schemars::schema_for!(Id<Zoom>);
        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains(r#""pattern":"^z[1-9][0-9]*$""#), "{json}");
    }

    #[test]
    fn prefix_rules() {
        assert!(is_valid_prefix("z") && is_valid_prefix("cap"));
        assert!(!is_valid_prefix("") && !is_valid_prefix("Z") && !is_valid_prefix("z1"));
    }
}
