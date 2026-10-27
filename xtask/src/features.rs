//! `docs/features.json` (the acceptance list) must stay consistent with the roadmap.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use crate::run::Outcome;

const FEATURES: &str = "docs/features.json";
const ROADMAP_DIR: &str = "docs/roadmap";
const STATUSES: [&str; 3] = ["todo", "pass", "fail"];
const VERIFY_KINDS: [&str; 6] = ["test", "golden", "visual", "ui", "cli", "manual"];

#[derive(Deserialize)]
struct FeatureList {
    features: Vec<Feature>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Feature {
    id: String,
    area: String,
    title: String,
    acceptance: String,
    verify: Vec<String>,
    tasks: Vec<String>,
    status: String,
}

pub fn check(root: &Path) -> Outcome {
    let text = match std::fs::read_to_string(root.join(FEATURES)) {
        Ok(text) => text,
        Err(error) => return Outcome::Fail(format!("не удалось прочитать {FEATURES}: {error}")),
    };
    let list: FeatureList = match serde_json::from_str(&text) {
        Ok(list) => list,
        Err(error) => return Outcome::Fail(format!("{FEATURES}: неверный JSON: {error}")),
    };
    let tasks = match roadmap_tasks(&root.join(ROADMAP_DIR)) {
        Ok(tasks) => tasks,
        Err(error) => return Outcome::Fail(error),
    };
    let problems = problems(&list.features, &tasks);
    if problems.is_empty() {
        let done = list.features.iter().filter(|f| f.status == "pass").count();
        Outcome::Pass(format!("{done}/{} pass", list.features.len()))
    } else {
        Outcome::Fail(format!("{FEATURES}:\n{}", problems.join("\n")))
    }
}

fn problems(features: &[Feature], tasks: &BTreeSet<String>) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for feature in features {
        let id = &feature.id;
        if !seen.insert(id.as_str()) {
            problems.push(format!("{id}: id повторяется — сделай его уникальным"));
        }
        let empty = [&feature.area, &feature.title, &feature.acceptance]
            .iter()
            .any(|field| field.trim().is_empty());
        if empty {
            problems.push(format!("{id}: пустые area/title/acceptance — заполни"));
        }
        if !STATUSES.contains(&feature.status.as_str()) {
            problems.push(format!(
                "{id}: статус `{}`, допустимы {STATUSES:?}",
                feature.status
            ));
        }
        if feature.verify.is_empty() {
            problems.push(format!("{id}: не указан способ проверки (verify)"));
        }
        for kind in &feature.verify {
            if !VERIFY_KINDS.contains(&kind.as_str()) {
                problems.push(format!(
                    "{id}: способ проверки `{kind}`, допустимы {VERIFY_KINDS:?}"
                ));
            }
        }
        for task in &feature.tasks {
            if !tasks.contains(task) {
                problems.push(format!(
                    "{id}: задачи {task} нет в {ROADMAP_DIR} — исправь номер"
                ));
            }
        }
    }
    problems
}

/// Task numbers declared in roadmap files as `**N.M**`.
fn roadmap_tasks(dir: &Path) -> Result<BTreeSet<String>, String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|error| format!("не удалось прочитать {}: {error}", dir.display()))?;
    let mut tasks = BTreeSet::new();
    for entry in entries {
        let path = entry
            .map_err(|error| format!("ошибка чтения {}: {error}", dir.display()))?
            .path();
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("не удалось прочитать {}: {error}", path.display()))?;
        tasks.extend(task_ids(&text));
    }
    Ok(tasks)
}

fn task_ids(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split("**").skip(1).step_by(2).filter_map(|chunk| {
        let is_task = chunk.contains('.')
            && chunk
                .split('.')
                .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
        is_task.then(|| chunk.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature(id: &str, status: &str, verify: &[&str], tasks: &[&str]) -> Feature {
        Feature {
            id: id.to_owned(),
            area: "A".to_owned(),
            title: "T".to_owned(),
            acceptance: "X".to_owned(),
            verify: verify.iter().map(|v| (*v).to_owned()).collect(),
            tasks: tasks.iter().map(|t| (*t).to_owned()).collect(),
            status: status.to_owned(),
        }
    }

    fn known() -> BTreeSet<String> {
        task_ids("- [ ] **2.1** a\n- [x] **5.17** b **bold** text").collect()
    }

    #[test]
    fn reads_task_ids_only() {
        assert_eq!(known().into_iter().collect::<Vec<_>>(), vec!["2.1", "5.17"]);
    }

    #[test]
    fn consistent_list_passes() {
        let features = [feature("a", "todo", &["test"], &["2.1"])];
        assert!(problems(&features, &known()).is_empty());
    }

    #[test]
    fn reports_every_kind_of_problem() {
        let features = [
            feature("a", "done", &["eyes"], &["9.99"]),
            feature("a", "todo", &[], &[]),
        ];
        let found = problems(&features, &known()).join("\n");
        for expected in [
            "статус `done`",
            "способ проверки `eyes`",
            "задачи 9.99",
            "id повторяется",
            "не указан способ",
        ] {
            assert!(found.contains(expected), "нет «{expected}» в:\n{found}");
        }
    }
}
