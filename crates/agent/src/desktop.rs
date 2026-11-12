//! Codex Desktop keeps its CLI in a build-named folder, outside the shell's PATH.

use std::path::{Path, PathBuf};

pub(crate) fn codex(local: &Path) -> Vec<PathBuf> {
    let bin = local.join("OpenAI").join("Codex").join("bin");
    let mut found = Vec::new();
    let direct = bin.join("codex.exe");
    if direct.is_file() {
        found.push(direct);
    }
    if let Ok(builds) = std::fs::read_dir(bin) {
        found.extend(
            builds
                .flatten()
                .map(|build| build.path().join("codex.exe"))
                .filter(|exe| exe.is_file()),
        );
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_desktop_without_path_and_ignores_incomplete_updates() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("OpenAI/Codex/bin");
        let build = bin.join("f71a721e0e6193a9");
        std::fs::create_dir_all(&build).unwrap();
        std::fs::create_dir_all(bin.join("unfinished")).unwrap();
        std::fs::write(build.join("codex.exe"), "").unwrap();
        let found = crate::find(&crate::Places {
            local: Some(tmp.path().to_owned()),
            ..crate::Places::default()
        });
        assert_eq!(
            found,
            vec![crate::Found {
                agent: dc_settings::Agent::Codex,
                path: build.join("codex.exe"),
            }]
        );
        assert!(codex(&tmp.path().join("missing")).is_empty());
        assert!(crate::pick(&found, Some(dc_settings::Agent::Claude)).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn codex_version_prefix_and_prerelease_choose_the_newest_installation() {
        let tmp = tempfile::tempdir().unwrap();
        let folders: Vec<PathBuf> = ["old", "new"].map(|name| tmp.path().join(name)).into();
        for (folder, version) in folders.iter().zip(["0.99.0", "0.167.0-alpha.3.2"]) {
            std::fs::create_dir_all(folder).unwrap();
            std::fs::write(
                folder.join("codex.cmd"),
                format!("@echo codex-cli {version}\r\n"),
            )
            .unwrap();
        }
        let found = crate::find(&crate::Places {
            path: folders.clone(),
            ..crate::Places::default()
        });
        assert_eq!(found[0].path, folders[1].join("codex.cmd"));
    }
}
