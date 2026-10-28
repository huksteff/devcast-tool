//! What a command changed, for the one-line answer: `зумы +3 z12..z14; зумы ~1 z2`.
//! `+` added, `-` removed, `~` changed. Diffs of a batch merge per kind and sign, ids are shown
//! sorted with runs of three or more collapsed to `first..last`.

use std::fmt;

use crate::id::{Id, IdKind};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diff {
    items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
    Ids {
        sign: char,
        plural: &'static str,
        prefix: &'static str,
        numbers: Vec<u32>,
    },
    Note(String),
}

impl Diff {
    pub fn added<K: IdKind>(id: Id<K>) -> Self {
        Self::ids('+', id)
    }

    pub fn removed<K: IdKind>(id: Id<K>) -> Self {
        Self::ids('-', id)
    }

    pub fn changed<K: IdKind>(id: Id<K>) -> Self {
        Self::ids('~', id)
    }

    /// A change that has no id, e.g. `вырез 0:10.000–0:12.000` or `формат 16:9`.
    pub fn note(text: impl Into<String>) -> Self {
        Self {
            items: vec![Item::Note(text.into())],
        }
    }

    fn ids<K: IdKind>(sign: char, id: Id<K>) -> Self {
        Self {
            items: vec![Item::Ids {
                sign,
                plural: K::PLURAL,
                prefix: K::PREFIX,
                numbers: vec![id.number()],
            }],
        }
    }

    /// Appends `other`, merging ids of the same kind and sign into one item.
    #[must_use]
    pub fn and(mut self, other: Self) -> Self {
        for item in other.items {
            self.push(item);
        }
        self
    }

    fn push(&mut self, item: Item) {
        if let Item::Ids {
            sign,
            prefix,
            numbers: more,
            ..
        } = &item
        {
            for existing in &mut self.items {
                if let Item::Ids {
                    sign: s,
                    prefix: p,
                    numbers,
                    ..
                } = existing
                    && s == sign
                    && p == prefix
                {
                    numbers.extend(more);
                    return;
                }
            }
        }
        self.items.push(item);
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl fmt::Display for Diff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, item) in self.items.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            match item {
                Item::Ids {
                    sign,
                    plural,
                    prefix,
                    numbers,
                } => {
                    let mut numbers = numbers.clone();
                    numbers.sort_unstable();
                    numbers.dedup();
                    let list = id_list(prefix, &numbers);
                    write!(f, "{plural} {sign}{} {list}", numbers.len())?;
                }
                Item::Note(text) => f.write_str(text)?,
            }
        }
        Ok(())
    }
}

/// Sorted, deduplicated `numbers` as `z1,z3,z5..z9`.
pub(crate) fn id_list(prefix: &str, numbers: &[u32]) -> String {
    let mut parts = Vec::new();
    let mut start = 0;
    while start < numbers.len() {
        let mut end = start;
        while end + 1 < numbers.len() && numbers[end + 1] == numbers[end] + 1 {
            end += 1;
        }
        if end - start >= 2 {
            parts.push(format!(
                "{prefix}{}..{prefix}{}",
                numbers[start], numbers[end]
            ));
        } else {
            parts.extend((start..=end).map(|i| format!("{prefix}{}", numbers[i])));
        }
        start = end + 1;
    }
    parts.join(",")
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
        const LIST: &'static str = "zooms";
    }

    fn z(n: u32) -> Id<Zoom> {
        Id::from_number(n)
    }

    #[test]
    fn batch_merges_ids_per_kind_and_sign() {
        let diff = Diff::added(z(13))
            .and(Diff::note("вырез 0:10.000–0:12.000"))
            .and(Diff::added(z(12)))
            .and(Diff::changed(z(2)))
            .and(Diff::added(z(14)));
        assert_eq!(
            diff.to_string(),
            "зумы +3 z12..z14; вырез 0:10.000–0:12.000; зумы ~1 z2"
        );
    }

    #[test]
    fn short_runs_stay_listed() {
        assert_eq!(id_list("z", &[1, 2, 4, 6, 7, 8, 9]), "z1,z2,z4,z6..z9");
        assert_eq!(id_list("z", &[]), "");
        assert_eq!(
            id_list("z", &[u32::MAX - 1, u32::MAX]),
            "z4294967294,z4294967295"
        );
    }

    #[test]
    fn same_id_twice_is_counted_once() {
        let diff = Diff::changed(z(3)).and(Diff::changed(z(3)));
        assert_eq!(diff.to_string(), "зумы ~1 z3");
        assert!(Diff::default().is_empty() && Diff::default().to_string().is_empty());
    }
}
