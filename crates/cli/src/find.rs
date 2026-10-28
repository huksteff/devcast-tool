//! Which project a command works on: `--project`, else the `*.dcproj` folder the current folder is
//! in, else the only `*.dcproj` folder inside the current folder.

use std::path::{Path, PathBuf};

use dc_project::ProjectDir;

use crate::error::CliError;

pub fn find_project(explicit: Option<&Path>, cwd: &Path) -> Result<ProjectDir, CliError> {
    if let Some(path) = explicit {
        return Ok(ProjectDir::open(cwd.join(path))?);
    }
    if let Some(root) = cwd.ancestors().find(|p| is_project(p)) {
        return Ok(ProjectDir::open(root)?);
    }
    let unreadable = |e: std::io::Error| {
        CliError::Failed(format!(
            "не удалось прочитать папку «{}» ({e}): укажи проект через --project <путь>",
            cwd.display()
        ))
    };
    let mut found = Vec::new();
    for entry in std::fs::read_dir(cwd).map_err(unreadable)? {
        let path = entry.map_err(unreadable)?.path();
        if is_project(&path) {
            found.push(path);
        }
    }
    found.sort();
    match found.as_slice() {
        [one] => Ok(ProjectDir::open(one)?),
        [] => Err(CliError::Failed(format!(
            "в «{}» нет проекта *.dcproj: перейди в папку проекта или укажи --project <путь>",
            cwd.display()
        ))),
        many => Err(CliError::Failed(format!(
            "в «{}» несколько проектов ({}): укажи нужный через --project <путь>",
            cwd.display(),
            names(many)
        ))),
    }
}

fn is_project(path: &Path) -> bool {
    path.is_dir()
        && path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dcproj"))
}

fn names(paths: &[PathBuf]) -> String {
    let names: Vec<_> = paths
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_string_lossy())
        .collect();
    names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_project_by_rules() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().join("Видео и звук");
        std::fs::create_dir(&cwd).unwrap();
        let error = find_project(None, &cwd).unwrap_err().to_string();
        assert!(
            error.contains("нет проекта") && error.contains("--project"),
            "{error}"
        );

        let one = ProjectDir::create(cwd.join("Первый урок.dcproj")).unwrap();
        assert_eq!(find_project(None, &cwd).unwrap(), one);
        assert_eq!(
            find_project(None, &one.media_dir()).unwrap(),
            one,
            "from inside"
        );

        let two = ProjectDir::create(cwd.join("второй.dcproj")).unwrap();
        let error = find_project(None, &cwd).unwrap_err().to_string();
        assert!(
            error.contains("Первый урок.dcproj, второй.dcproj"),
            "{error}"
        );
        let relative = Path::new("второй.dcproj");
        assert_eq!(find_project(Some(relative), &cwd).unwrap(), two);
        let error = find_project(Some(Path::new("нет.dcproj")), &cwd).unwrap_err();
        assert!(error.to_string().contains("не похоже на проект"), "{error}");
    }
}
