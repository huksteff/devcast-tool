//! Workspace layout from `cargo metadata`: packages, their folders and workspace dependencies.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

pub struct Package {
    pub name: String,
    /// Package folder relative to the workspace root, with `/` separators.
    pub dir: String,
    /// Names of other workspace packages this one depends on (any dependency kind).
    pub deps: Vec<String>,
}

pub struct Workspace {
    pub root: PathBuf,
    /// Cargo's build folder: `target/` or `CARGO_TARGET_DIR`.
    pub target: PathBuf,
    pub packages: Vec<Package>,
}

#[derive(Deserialize)]
struct RawMetadata {
    workspace_root: PathBuf,
    target_directory: PathBuf,
    packages: Vec<RawPackage>,
}

#[derive(Deserialize)]
struct RawPackage {
    name: String,
    manifest_path: PathBuf,
    dependencies: Vec<RawDependency>,
}

#[derive(Deserialize)]
struct RawDependency {
    name: String,
}

impl Workspace {
    pub fn load() -> Result<Self, String> {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let output = Command::new(cargo)
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .output()
            .map_err(|error| format!("не удалось запустить `cargo metadata`: {error}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("`cargo metadata` завершился с ошибкой:\n{stderr}"));
        }
        let raw: RawMetadata = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("не удалось разобрать вывод `cargo metadata`: {error}"))?;
        Ok(Self::from_raw(raw))
    }

    fn from_raw(raw: RawMetadata) -> Self {
        let names: BTreeSet<String> = raw.packages.iter().map(|p| p.name.clone()).collect();
        let packages = raw
            .packages
            .into_iter()
            .map(|package| Package {
                dir: relative_dir(&raw.workspace_root, &package.manifest_path),
                deps: package
                    .dependencies
                    .into_iter()
                    .map(|dep| dep.name)
                    .filter(|dep| names.contains(dep))
                    .collect(),
                name: package.name,
            })
            .collect();
        Self {
            root: raw.workspace_root,
            target: raw.target_directory,
            packages,
        }
    }
}

fn relative_dir(root: &Path, manifest: &Path) -> String {
    let dir = manifest.parent().unwrap_or(manifest);
    let relative = dir.strip_prefix(root).unwrap_or(dir);
    relative.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_workspace_dependencies() {
        let raw: RawMetadata = serde_json::from_str(
            r#"{"workspace_root": "/ws", "target_directory": "/ws/target", "packages": [
                {"name": "dc-core", "manifest_path": "/ws/crates/core/Cargo.toml", "dependencies": []},
                {"name": "dc-ops", "manifest_path": "/ws/crates/ops/Cargo.toml",
                 "dependencies": [{"name": "dc-core"}, {"name": "serde"}]}
            ]}"#,
        )
        .unwrap();
        let workspace = Workspace::from_raw(raw);
        let ops = workspace
            .packages
            .iter()
            .find(|p| p.name == "dc-ops")
            .unwrap();
        assert_eq!(ops.dir, "crates/ops");
        assert_eq!(ops.deps, vec!["dc-core".to_owned()]);
    }
}
