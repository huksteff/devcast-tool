//! Which workspace packages are affected by uncommitted changes.

use std::collections::BTreeSet;

use crate::metadata::Workspace;
use crate::run;

/// Root files whose change affects every package.
const GLOBAL_FILES: [&str; 4] = [
    "Cargo.toml",
    "Cargo.lock",
    "clippy.toml",
    "rust-toolchain.toml",
];

/// Packages touched by uncommitted (incl. untracked) changes plus all packages depending on them.
pub fn packages(workspace: &Workspace) -> Result<BTreeSet<String>, String> {
    Ok(affected(workspace, &uncommitted_paths()?))
}

/// Paths (relative to the repository root) with uncommitted changes, untracked files included.
pub fn uncommitted_paths() -> Result<Vec<String>, String> {
    let status = run::git(&["status", "--porcelain", "--untracked-files=all", "-z"])?;
    Ok(changed_paths(&status))
}

/// True if any path is Rust code or build configuration, i.e. something `verify` checks.
pub fn touches_code(paths: &[String]) -> bool {
    paths
        .iter()
        .any(|path| path.ends_with(".rs") || path.ends_with("Cargo.toml") || is_global(path))
}

/// Paths from `git status --porcelain -z` output (both sides of renames and copies).
fn changed_paths(status: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut entries = status.split('\0').filter(|entry| !entry.is_empty());
    while let Some(entry) = entries.next() {
        let (code, path) = entry.split_at(entry.len().min(3));
        paths.push(path.to_owned());
        if code.contains('R') || code.contains('C') {
            paths.extend(entries.next().map(str::to_owned));
        }
    }
    paths
}

fn affected(workspace: &Workspace, paths: &[String]) -> BTreeSet<String> {
    if paths.iter().any(|path| is_global(path)) {
        return workspace.packages.iter().map(|p| p.name.clone()).collect();
    }
    let mut affected: BTreeSet<String> = paths
        .iter()
        .filter_map(|path| owner(workspace, path))
        .collect();
    loop {
        let dependents: Vec<String> = workspace
            .packages
            .iter()
            .filter(|p| !affected.contains(&p.name))
            .filter(|p| p.deps.iter().any(|dep| affected.contains(dep)))
            .map(|p| p.name.clone())
            .collect();
        if dependents.is_empty() {
            return affected;
        }
        affected.extend(dependents);
    }
}

fn is_global(path: &str) -> bool {
    GLOBAL_FILES.contains(&path) || path.starts_with(".cargo/")
}

/// The package whose folder contains `path` (the deepest one if folders nest).
fn owner(workspace: &Workspace, path: &str) -> Option<String> {
    workspace
        .packages
        .iter()
        .filter(|p| path.starts_with(&format!("{}/", p.dir)))
        .max_by_key(|p| p.dir.len())
        .map(|p| p.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Package;

    fn workspace() -> Workspace {
        let package = |name: &str, dir: &str, deps: &[&str]| Package {
            name: name.to_owned(),
            dir: dir.to_owned(),
            deps: deps.iter().map(|d| (*d).to_owned()).collect(),
        };
        Workspace {
            root: "/ws".into(),
            target: "/ws/target".into(),
            packages: vec![
                package("dc-core", "crates/core", &[]),
                package("dc-render", "crates/render", &["dc-core"]),
                package("dc-fx-zoom", "fx/zoom", &["dc-core", "dc-render"]),
                package("dc-media", "crates/media", &["dc-core"]),
            ],
        }
    }

    fn names(set: &BTreeSet<String>) -> Vec<&str> {
        set.iter().map(String::as_str).collect()
    }

    #[test]
    fn parses_porcelain_with_renames() {
        let status = " M crates/core/src/lib.rs\0R  crates/a.rs\0crates/b.rs\0?? docs/x.md\0";
        assert_eq!(
            changed_paths(status),
            vec![
                "crates/core/src/lib.rs",
                "crates/a.rs",
                "crates/b.rs",
                "docs/x.md"
            ]
        );
    }

    #[test]
    fn change_in_core_affects_dependents() {
        let set = affected(&workspace(), &["crates/core/src/time.rs".to_owned()]);
        assert_eq!(
            names(&set),
            vec!["dc-core", "dc-fx-zoom", "dc-media", "dc-render"]
        );
    }

    #[test]
    fn leaf_change_affects_only_leaf() {
        let set = affected(&workspace(), &["fx/zoom/src/lib.rs".to_owned()]);
        assert_eq!(names(&set), vec!["dc-fx-zoom"]);
    }

    #[test]
    fn docs_change_affects_nothing() {
        assert!(affected(&workspace(), &["docs/SCOPE.md".to_owned()]).is_empty());
    }

    #[test]
    fn only_code_and_build_files_count_as_code() {
        let docs = vec!["docs/SCOPE.md".to_owned(), "README.md".to_owned()];
        assert!(!touches_code(&docs));
        assert!(touches_code(&["fx/zoom/src/lib.rs".to_owned()]));
        assert!(touches_code(&["crates/core/Cargo.toml".to_owned()]));
        assert!(touches_code(&[".cargo/config.toml".to_owned()]));
    }

    #[test]
    fn lockfile_change_affects_everything() {
        assert_eq!(affected(&workspace(), &["Cargo.lock".to_owned()]).len(), 4);
    }
}
