//! Contacts of the feedback page (8.4): `crates/update/feedback.json`, edited without code and
//! built into the programs. Not a file next to the exe: Velopack replaces the install folder on
//! every update, so an edit there would not live long anyway.

use serde::Deserialize;

use crate::error::UpdateError;

const FILE: &str = include_str!("../feedback.json");

/// Where to write about a bug or an idea.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feedback {
    pub contacts: Vec<Contact>,
    /// Link to the repository of DevCast.
    pub repository: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contact {
    /// What it is: «Почта», «Telegram».
    pub name: String,
    /// What the user sees: an address, a handle.
    pub value: String,
    /// The link a click opens (`mailto:`, `https:`); none — the value is only shown.
    #[serde(default)]
    pub url: Option<String>,
}

/// The contacts of `feedback.json`.
pub fn feedback() -> Result<Feedback, UpdateError> {
    parse(FILE)
}

fn parse(text: &str) -> Result<Feedback, UpdateError> {
    serde_json::from_str(text).map_err(UpdateError::Feedback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_file_has_contacts_and_the_repository() {
        let feedback = feedback().unwrap();
        assert!(!feedback.contacts.is_empty());
        assert!(feedback.repository.starts_with("https://"));
        for contact in &feedback.contacts {
            assert!(!contact.name.is_empty() && !contact.value.is_empty());
        }
    }

    #[test]
    fn a_contact_without_a_link_is_only_shown() {
        let text = r#"{"contacts": [{"name": "Telegram", "value": "@devcast"}], "repository": "https://x"}"#;
        assert_eq!(parse(text).unwrap().contacts[0].url, None);
    }

    #[test]
    fn a_broken_file_says_where_to_fix_it() {
        let error = parse(r#"{"contacts": []}"#).unwrap_err().to_string();
        assert!(error.contains("crates/update/feedback.json"), "{error}");
    }
}
