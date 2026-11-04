//! Which AI agents the user has: `claude` and `codex` on `PATH`, or where their installers put
//! them (the native installer of Claude Code — `~\.local\bin`, npm — `%APPDATA%\npm`), and
//! last the Claude Code inside Claude Desktop (`%LOCALAPPDATA%\Claude-Data\claude_*\claude-code\
//! <version>\<build>\claude.exe`, the newest version): it works on its own once signed in with
//! `/login` (owner's decision 2026-11-04).

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
    /// `%LOCALAPPDATA%`: where Claude Desktop keeps its Claude Code.
    pub local: Option<PathBuf>,
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
            local: var("LOCALAPPDATA"),
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
            let installed = folders.iter().find_map(|folder| program(folder, name));
            let path = match agent {
                Agent::Claude => installed.or_else(|| desktop_claude(places.local.as_deref()?)),
                Agent::Codex => installed,
            }?;
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

/// The Claude Code of Claude Desktop under `local`, of its newest version.
fn desktop_claude(local: &Path) -> Option<PathBuf> {
    let apps = std::fs::read_dir(local.join("Claude-Data")).ok()?;
    let mut found: Vec<(Vec<u32>, PathBuf)> = Vec::new();
    for app in apps.flatten() {
        let Ok(versions) = std::fs::read_dir(app.path().join("claude-code")) else {
            continue;
        };
        for version in versions.flatten() {
            let name = version.file_name().to_string_lossy().into_owned();
            let Some(number) = version_of(&name) else {
                continue;
            };
            let Ok(builds) = std::fs::read_dir(version.path()) else {
                continue;
            };
            let exe = builds
                .flatten()
                .map(|b| b.path().join("claude.exe"))
                .find(|p| p.is_file());
            found.extend(exe.map(|exe| (number, exe)));
        }
    }
    found
        .into_iter()
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, exe)| exe)
}

/// `2.1.309` as numbers; none for a folder that is not a version.
fn version_of(name: &str) -> Option<Vec<u32>> {
    name.split('.').map(|part| part.parse().ok()).collect()
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
            local: None,
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

    #[test]
    fn claude_desktop_gives_its_newest_claude_code_last() {
        let tmp = tempfile::tempdir().unwrap();
        let code = tmp
            .path()
            .join("Claude-Data")
            .join("claude_pzs8")
            .join("claude-code");
        touch(&code.join("2.1.99").join("a1").join("claude.exe"));
        touch(&code.join("2.1.309").join("b2").join("claude.exe"));
        std::fs::create_dir_all(code.join("2.2.0").join("c3")).unwrap();
        std::fs::create_dir_all(code.join("not-a-version")).unwrap();
        let places = Places {
            local: Some(tmp.path().to_path_buf()),
            ..Places::default()
        };
        let found = find(&places);
        let newest = code.join("2.1.309").join("b2").join("claude.exe");
        assert_eq!(
            found,
            vec![Found {
                agent: Agent::Claude,
                path: newest
            }],
            "2.2.0 has no exe"
        );
        let installed = tmp.path().join("bin");
        touch(&installed.join(exe("claude")));
        let places = Places {
            path: vec![installed.clone()],
            ..places
        };
        assert_eq!(
            find(&places)[0].path,
            installed.join(exe("claude")),
            "an installed one first"
        );
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
