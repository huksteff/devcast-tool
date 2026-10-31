//! `devcast-app --crash-server`, the dump writer, through the real exe: it must end once its
//! parent is gone, even if the parent never connected (a second start of the app, a killed app).

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// Starts the dump writer with its socket and dumps in `dir`, stdin as given.
#[cfg(test)]
fn server(dir: &Path, stdin: Stdio) -> Child {
    Command::new(env!("CARGO_BIN_EXE_devcast-app"))
        .arg("--crash-server")
        .arg(dir.join("socket"))
        .arg(dir.join("crashes"))
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// Waits for the server's "ready" line.
#[cfg(test)]
fn ready(child: &mut Child) {
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line.trim(), "ready");
}

#[test]
fn ends_when_the_parent_closes_without_connecting() {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let mut child = server(dir.path(), Stdio::piped());
    ready(&mut child);
    drop(child.stdin.take());
    let status = child.wait().unwrap();
    assert!(status.success(), "{status:?}");
}

#[test]
fn ends_at_once_without_a_parent() {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let mut child = server(dir.path(), Stdio::null());
    ready(&mut child);
    let status = child.wait().unwrap();
    assert!(status.success(), "{status:?}");
}
