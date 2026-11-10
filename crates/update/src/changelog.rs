//! «Что нового» (8.3): the releases of `CHANGELOG.md`, built into the programs. A release is a
//! section `## <версия>` (`## 0.2.0` or `## 0.2.0 — 2026-11-10`) with groups `### Добавлено` …
//! of items `- …`; «Не выпущено» and other sections without a version are not releases.
//! The window «Что нового» shows only the group `### Главное`: a few short lines a user reads
//! in ten seconds; the other groups are the full log (`devcast app changelog`).

use crate::error::UpdateError;

const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// One section `## <версия>` of the changelog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub groups: Vec<Group>,
}

/// The group the window «Что нового» shows.
pub const MAIN: &str = "Главное";

impl Release {
    /// The lines of «Главное»; none when the release has no such group.
    pub fn main(&self) -> &[String] {
        self.groups
            .iter()
            .find(|group| group.title == MAIN)
            .map_or(&[], |group| group.items.as_slice())
    }
}

/// `### Добавлено` and its items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub title: String,
    pub items: Vec<String>,
}

/// The releases newer than `seen` up to `current`, newest first; `seen` none — all up to
/// `current`.
pub fn news(seen: Option<&str>, current: &str) -> Result<Vec<Release>, UpdateError> {
    between(CHANGELOG, seen, current)
}

fn between(text: &str, seen: Option<&str>, current: &str) -> Result<Vec<Release>, UpdateError> {
    let seen = seen.map(version).transpose()?;
    let current = version(current)?;
    let mut news = Vec::new();
    for release in releases(text) {
        let at = version(&release.version)?;
        if at <= current && seen.is_none_or(|seen| at > seen) {
            news.push((at, release));
        }
    }
    news.sort_by_key(|a| std::cmp::Reverse(a.0));
    Ok(news.into_iter().map(|(_, release)| release).collect())
}

fn releases(text: &str) -> Vec<Release> {
    let mut releases: Vec<Release> = Vec::new();
    let mut open = false;
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            let first = title.split_whitespace().next().unwrap_or_default();
            open = first.starts_with(|c: char| c.is_ascii_digit());
            if open {
                releases.push(Release {
                    version: first.to_owned(),
                    groups: Vec::new(),
                });
            }
            continue;
        }
        let Some(release) = releases.last_mut().filter(|_| open) else {
            continue;
        };
        if let Some(title) = line.strip_prefix("### ") {
            release.groups.push(Group {
                title: title.trim().to_owned(),
                items: Vec::new(),
            });
        } else if let (Some(item), Some(group)) =
            (line.strip_prefix("- "), release.groups.last_mut())
        {
            group.items.push(item.trim().to_owned());
        }
    }
    releases
}

/// `major.minor.patch` as numbers, compared in that order.
fn version(text: &str) -> Result<(u32, u32, u32), UpdateError> {
    let bad = || UpdateError::Version(text.to_owned());
    let mut parts = text.split('.').map(|p| p.parse::<u32>().map_err(|_| bad()));
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(major), Some(minor), Some(patch), None) => Ok((major?, minor?, patch?)),
        _ => Err(bad()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "# Changelog\n\nОписание.\n\n## Не выпущено\n\n### Добавлено\n\n- ещё не вышло\n\n\
        ## 0.10.0 — 2026-12-01\n\n### Добавлено\n\n- Лезвие (B)\n- Шумоподавление\n\n### Исправлено\n\n- Субтитры\n\n\
        ## 0.9.1\n\n### Исправлено\n\n- Экспорт\n\n## 0.9.0\n\n### Добавлено\n\n- Запись\n";

    fn versions(news: &[Release]) -> Vec<&str> {
        news.iter().map(|r| r.version.as_str()).collect()
    }

    #[test]
    fn news_are_the_releases_after_the_seen_one_newest_first() {
        let news = between(TEXT, Some("0.9.0"), "0.10.0").unwrap();
        assert_eq!(versions(&news), ["0.10.0", "0.9.1"]);
        assert_eq!(news[0].groups[0].title, "Добавлено");
        assert_eq!(news[0].groups[0].items, ["Лезвие (B)", "Шумоподавление"]);
        assert_eq!(news[0].groups[1].items, ["Субтитры"]);
    }

    #[test]
    fn unreleased_and_future_sections_are_not_news() {
        assert_eq!(
            versions(&between(TEXT, Some("0.9.0"), "0.9.1").unwrap()),
            ["0.9.1"]
        );
        assert_eq!(versions(&between(TEXT, None, "0.9.0").unwrap()), ["0.9.0"]);
        assert!(between(TEXT, Some("0.10.0"), "0.10.0").unwrap().is_empty());
    }

    #[test]
    fn a_wrong_version_says_what_is_expected() {
        let error = between(TEXT, Some("бета"), "0.10.0")
            .unwrap_err()
            .to_string();
        assert!(error.contains("бета") && error.contains("1.2.3"), "{error}");
    }

    #[test]
    fn the_built_in_changelog_reads() {
        news(None, env!("CARGO_PKG_VERSION")).unwrap();
    }

    /// The most lines of [`MAIN`] in a release and the most characters in a line.
    const MAIN_LINES: usize = 7;
    const MAIN_LENGTH: usize = 100;

    /// «Не выпущено» is the next release: it is held to the same rule.
    #[test]
    fn every_release_tells_the_main_things_in_a_few_short_lines() {
        let text = CHANGELOG.replace("## Не выпущено", "## 9999.0.0");
        let all = releases(&text);
        assert!(all.len() >= 2);
        for release in all {
            let main = release.main();
            assert!(
                (1..=MAIN_LINES).contains(&main.len()),
                "{}: «### Главное» has {} lines, 1–{MAIN_LINES} wanted",
                release.version,
                main.len()
            );
            for line in main {
                assert!(
                    line.chars().count() <= MAIN_LENGTH,
                    "{}: longer than {MAIN_LENGTH} characters: {line}",
                    release.version
                );
            }
        }
        assert_eq!(releases(TEXT)[0].main(), [] as [String; 0]);
    }
}
