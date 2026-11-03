//! Which AI agents the user has: `claude` and `codex` on `PATH`, or where their installers put
//! them (the native installer of Claude Code — `~\.local\bin`, npm — `%APPDATA%\npm`). The
//! Claude Code inside Claude Desktop is not looked for: it is not signed in by itself.

use std::path::{Path, PathBuf};

use dc_settings::Agent;

/// An agent found on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub agent: Agent,
    /// The program to start: an `.exe`, or an npm `.cmd` shim.
    pub path: PathBuf,
}

/// Where to look: the folders of `PATH`, the home folder and `%APPDATA%`.
#[derive(Debug, Clone, Default)]
pub struct Places {
    pub path: Vec<PathBuf>,
    pub home: Option<PathBuf>,
    pub appdata: Option<PathBuf>,
}

impl Places {
    /// The places of this user and process.
    pub fn here() -> Self {
        let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
        Self {
            path: std::env::var_os("PATH")
                .map(|p| std::env::split_paths(&p).collect())
                .unwrap_or_default(),
            home: var("USERPROFILE").or_else(|| var("HOME")),
            appdata: var("APPDATA"),
        }
    }

    /// Folders to look in, in order: `PATH` first, as a shell would.
    fn folders(&self) -> Vec<PathBuf> {
        let mut folders = self.path.clone();
        folders.extend(self.home.iter().map(|h| h.join(".local").join("bin")));
        folders.extend(self.appdata.iter().map(|a| a.join("npm")));
        folders
    }
}

/// The agents found in `places`, Claude Code first; each once, at its first place.
pub fn find(places: &Places) -> Vec<Found> {
    let folders = places.folders();
    [(Agent::Claude, "claude"), (Agent::Codex, "codex")]
        .into_iter()
        .filter_map(|(agent, name)| {
            let path = folders.iter().find_map(|folder| program(folder, name))?;
            Some(Found { agent, path })
        })
        .collect()
}

/// The agent `wanted` among `found`, else the first one.
pub fn pick(found: &[Found], wanted: Option<Agent>) -> Option<&Found> {
    wanted
        .and_then(|agent| found.iter().find(|f| f.agent == agent))
        .or_else(|| found.first())
}

/// `name` in `folder` as Windows starts it: `.exe`, then the `.cmd` of npm.
fn program(folder: &Path, name: &str) -> Option<PathBuf> {
    let suffixes: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd"]
    } else {
        &[""]
    };
    suffixes
        .iter()
        .map(|suffix| folder.join(format!("{name}{suffix}")))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }

    fn exe(name: &str) -> String {
        if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_owned()
        }
    }

    #[test]
    fn agents_are_found_on_the_path_and_where_installers_put_them() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("Пользователь");
        let on_path = tmp.path().join("bin с пробелом");
        touch(&home.join(".local").join("bin").join(exe("claude")));
        touch(&on_path.join(exe("codex")));
        let places = Places {
            path: vec![on_path.clone()],
            home: Some(home.clone()),
            appdata: None,
        };
        let found = find(&places);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].agent, Agent::Claude, "Claude Code first");
        assert_eq!(found[1].path, on_path.join(exe("codex")));
        assert_eq!(
            pick(&found, Some(Agent::Codex)).unwrap().agent,
            Agent::Codex
        );
        assert_eq!(pick(&found, None).unwrap().agent, Agent::Claude);
        assert!(find(&Places::default()).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn an_npm_shim_is_found_too() {
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("npm").join("claude.cmd"));
        let places = Places {
            appdata: Some(tmp.path().to_path_buf()),
            ..Places::default()
        };
        let found = find(&places);
        assert_eq!(found[0].path, tmp.path().join("npm").join("claude.cmd"));
    }
}
