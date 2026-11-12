//! The changelog in pieces. A merge request does not edit `CHANGELOG.md` — every one of them
//! would add its line at the same place and conflict with the others. It adds a file of its own
//! to `changelog/` instead: groups `### Добавлено` … with items `- …`, as in a section of
//! `CHANGELOG.md`. `cargo xtask changelog` joins the files into the section `## <version>` of
//! the release and deletes them; verify does not let a release go without it.

use std::path::{Path, PathBuf};

use crate::run::Outcome;

/// The folder of the pieces, in the root of the repository.
pub const DIR: &str = "changelog";
const FILE: &str = "CHANGELOG.md";
/// The groups of a section, in the order they are written.
const GROUPS: [&str; 5] = ["Главное", "Добавлено", "Изменено", "Исправлено", "Удалено"];
/// «Главное» is what the window «Что нового» shows: a few short lines.
const MAIN: &str = "Главное";
const MAIN_LINES: usize = 7;
const MAIN_LENGTH: usize = 100;

/// The lines of each group, in the order of [`GROUPS`].
type Section = [Vec<String>; 5];

/// The pieces of `root` are well-formed and the version of `Cargo.toml` has its section.
pub fn check(root: &Path) -> Outcome {
    match checked(root) {
        Ok(pieces) => Outcome::Pass(match pieces {
            0 => String::new(),
            n => format!("{n} в {DIR}/"),
        }),
        Err(problem) => Outcome::Fail(problem),
    }
}

fn checked(root: &Path) -> Result<usize, String> {
    let (section, pieces) = read(root)?;
    let text = std::fs::read_to_string(root.join(FILE))
        .map_err(|e| format!("не удалось прочитать {FILE}: {e}"))?;
    if text.lines().any(|line| line.trim() == "## Не выпущено") {
        return Err(format!(
            "в {FILE} есть раздел «Не выпущено».\n  → Перенеси его строки в свой файл {DIR}/<ветка>.md (группы `### …` и строки `- …`) и убери раздел: общий раздел конфликтует между MR."
        ));
    }
    let version = version(root)?;
    if !released(&text, &version) {
        return Err(format!(
            "в {FILE} нет раздела `## {version}`, а это `version` в Cargo.toml.\n  → Запусти `cargo xtask changelog`: он соберёт раздел из файлов {DIR}/ и удалит их."
        ));
    }
    main_fits(&section)?;
    Ok(pieces)
}

/// `cargo xtask changelog`: the section of the version of `Cargo.toml` out of the pieces.
pub fn release(root: &Path) -> Result<String, String> {
    let (section, pieces) = read(root)?;
    let version = version(root)?;
    let path = root.join(FILE);
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("не удалось прочитать {FILE}: {e}"))?;
    if released(&text, &version) {
        return Err(format!(
            "раздел `## {version}` в {FILE} уже есть: сначала подними `version` в Cargo.toml до номера нового выпуска"
        ));
    }
    if section[0].is_empty() {
        return Err(format!(
            "ни в одном файле {DIR}/ нет группы `### {MAIN}`: добавь 1–{MAIN_LINES} коротких строк о главном в выпуске — их показывает окно «Что нового»"
        ));
    }
    main_fits(&section)?;
    std::fs::write(&path, with_section(&text, &version, &section)?)
        .map_err(|e| format!("не удалось записать {FILE}: {e}"))?;
    for file in files(root)? {
        std::fs::remove_file(&file)
            .map_err(|e| format!("не удалось удалить {}: {e}", file.display()))?;
    }
    Ok(format!(
        "{FILE}: раздел `## {version}` из {pieces} файлов {DIR}/, файлы удалены"
    ))
}

/// `text` with the section of `version` above the last release.
fn with_section(text: &str, version: &str, section: &Section) -> Result<String, String> {
    let Some(at) = text.find("\n## ") else {
        return Err(format!(
            "в {FILE} нет ни одного раздела `## `: некуда вставить выпуск"
        ));
    };
    let mut written = format!("## {version}\n");
    for (title, lines) in GROUPS.iter().zip(section) {
        if lines.is_empty() {
            continue;
        }
        written.push_str(&format!("\n### {title}\n\n"));
        for line in lines {
            written.push_str(&format!("- {line}\n"));
        }
    }
    Ok(format!("{}\n{written}{}", &text[..at], &text[at..]))
}

fn main_fits(section: &Section) -> Result<(), String> {
    let main = &section[0];
    if main.len() > MAIN_LINES {
        return Err(format!(
            "в файлах {DIR}/ вместе {} строк группы `### {MAIN}`, а окно «Что нового» показывает не больше {MAIN_LINES}.\n  → Оставь в «{MAIN}» самое важное, остальное перенеси в обычные группы.",
            main.len()
        ));
    }
    match main.iter().find(|line| line.chars().count() > MAIN_LENGTH) {
        Some(line) => Err(format!(
            "строка группы `### {MAIN}` длиннее {MAIN_LENGTH} символов: {line}\n  → Сократи: её читают в окне «Что нового»."
        )),
        None => Ok(()),
    }
}

/// `version` of `[workspace.package]`: the first `version = "…"` line of the root manifest.
fn version(root: &Path) -> Result<String, String> {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|e| format!("не удалось прочитать Cargo.toml: {e}"))?;
    manifest
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.split('"').next())
        .map(str::to_owned)
        .ok_or_else(|| "в корневом Cargo.toml нет строки `version = \"…\"`".to_owned())
}

/// `text` has the section `## <version>` (`## 0.2.0` or `## 0.2.0 — 2026-11-10`).
fn released(text: &str, version: &str) -> bool {
    text.lines()
        .filter_map(|line| line.strip_prefix("## "))
        .any(|title| title.split_whitespace().next() == Some(version))
}

/// The pieces of `root`, by file name.
fn files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let dir = root.join(DIR);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let unreadable = |e: std::io::Error| format!("не удалось прочитать папку {DIR}/: {e}");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(unreadable)? {
        let path = entry.map_err(unreadable)?.path();
        if path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// The lines of all pieces by group, and how many pieces there are.
fn read(root: &Path) -> Result<(Section, usize), String> {
    let mut section = Section::default();
    let files = files(root)?;
    for file in &files {
        let name = format!(
            "{DIR}/{}",
            file.file_name().unwrap_or_default().to_string_lossy()
        );
        let text = std::fs::read_to_string(file)
            .map_err(|e| format!("не удалось прочитать {name}: {e}"))?;
        parse(&name, &text, &mut section)?;
    }
    Ok((section, files.len()))
}

/// Adds the lines of the piece `name` to `section`.
fn parse(name: &str, text: &str, section: &mut Section) -> Result<(), String> {
    let shape = format!(
        "  → Файл — группы `### {}` и под каждой строки `- …`, как в разделе {FILE}.",
        GROUPS.join("`, `### ")
    );
    let mut group = None;
    let mut lines = 0;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(title) = line.strip_prefix("### ") {
            group = GROUPS.iter().position(|known| *known == title.trim());
            if group.is_none() {
                return Err(format!("{name}: нет группы «{}».\n{shape}", title.trim()));
            }
        } else if let (Some(item), Some(group)) = (line.strip_prefix("- "), group) {
            section[group].push(item.trim().to_owned());
            lines += 1;
        } else {
            return Err(format!("{name}: непонятная строка «{line}».\n{shape}"));
        }
    }
    match lines {
        0 => Err(format!(
            "{name}: в файле нет ни одной строки `- …`.\n{shape}"
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "# Changelog\n\nОписание.\n\n## 0.1.2\n\n### Главное\n\n- Запись\n";

    fn repository(version: &str, pieces: &[(&str, &str)]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let manifest = format!("[workspace.package]\nversion = \"{version}\"\n");
        std::fs::write(root.path().join("Cargo.toml"), manifest).unwrap();
        std::fs::write(root.path().join(FILE), LOG).unwrap();
        std::fs::create_dir(root.path().join(DIR)).unwrap();
        for (name, text) in pieces {
            std::fs::write(root.path().join(DIR).join(name), text).unwrap();
        }
        root
    }

    fn problem(root: &tempfile::TempDir) -> String {
        match check(root.path()) {
            Outcome::Fail(problem) => problem,
            Outcome::Pass(detail) => panic!("passed: {detail}"),
        }
    }

    #[test]
    fn pieces_wait_for_the_release_and_a_new_version_asks_for_its_section() {
        let pieces = [
            ("b-fix.md", "### Исправлено\n\n- Отсчёт\n"),
            (
                "a-add.md",
                "### Главное\n- Двойной клик\n### Добавлено\n- Двойной клик по тексту\n",
            ),
        ];
        let waiting = repository("0.1.2", &pieces);
        assert!(matches!(check(waiting.path()), Outcome::Pass(detail) if detail.contains('2')));

        let release_mr = repository("0.1.3", &pieces);
        assert!(problem(&release_mr).contains("cargo xtask changelog"));
        release(release_mr.path()).unwrap();
        let text = std::fs::read_to_string(release_mr.path().join(FILE)).unwrap();
        let section = "## 0.1.3\n\n### Главное\n\n- Двойной клик\n\n### Добавлено\n\n- Двойной клик по тексту\n\n### Исправлено\n\n- Отсчёт\n\n## 0.1.2\n";
        assert!(text.contains(section), "{text}");
        assert!(text.starts_with("# Changelog\n\nОписание.\n\n## 0.1.3\n"));
        assert!(
            files(release_mr.path()).unwrap().is_empty(),
            "pieces are deleted"
        );
        assert!(matches!(check(release_mr.path()), Outcome::Pass(detail) if detail.is_empty()));
        assert!(release(release_mr.path()).unwrap_err().contains("уже есть"));
    }

    #[test]
    fn a_release_needs_the_main_lines_and_they_are_few_and_short() {
        let plain = repository("0.1.3", &[("fix.md", "### Исправлено\n- Отсчёт\n")]);
        assert!(release(plain.path()).unwrap_err().contains("### Главное"));

        let long = format!("### Главное\n- {}\n", "я".repeat(MAIN_LENGTH + 1));
        assert!(problem(&repository("0.1.2", &[("long.md", &long)])).contains("длиннее"));
        let many = format!("### Главное\n{}", "- строка\n".repeat(MAIN_LINES + 1));
        assert!(problem(&repository("0.1.2", &[("many.md", &many)])).contains("не больше"));
    }

    #[test]
    fn a_wrong_piece_and_the_old_shared_section_say_what_to_do() {
        let unknown = repository("0.1.2", &[("x.md", "### Новое\n- строка\n")]);
        assert!(problem(&unknown).contains("нет группы «Новое»"));
        let loose = repository("0.1.2", &[("x.md", "- строка без группы\n")]);
        assert!(problem(&loose).contains("непонятная строка"));
        let empty = repository("0.1.2", &[("x.md", "### Добавлено\n")]);
        assert!(problem(&empty).contains("нет ни одной строки"));

        let old = repository("0.1.2", &[]);
        let shared = LOG.replace("## 0.1.2", "## Не выпущено\n\n- строка\n\n## 0.1.2");
        std::fs::write(old.path().join(FILE), shared).unwrap();
        assert!(problem(&old).contains("changelog/<ветка>.md"));
    }
}
