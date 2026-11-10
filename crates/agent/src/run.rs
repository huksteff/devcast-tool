//! One run of the agent: its command line ([`launch`]), the process, the thread
//! `devcast-agent` that reads its output into [`Event`]s, and the stop. Every `devcast` the
//! agent runs gets [`GROUP_ENV`]: its steps join into one undo step (`History::group`).

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};

use dc_settings::Agent;

use crate::find::Found;
use crate::stream::{Event, Reader};

/// The variable that names the run a `devcast` command belongs to.
pub const GROUP_ENV: &str = "DEVCAST_GROUP";

/// The only tool calls the agent may make without asking: `devcast` commands.
/// Claude Code runs commands with Bash or, on Windows since 2.1.317, PowerShell: either may run
/// `devcast` and nothing else.
const TOOLS: &str = "Bash,PowerShell";
const ALLOWED: [&str; 2] = ["Bash(devcast *)", "PowerShell(devcast *)"];

/// What to start.
#[derive(Debug, Clone)]
pub struct Launch {
    pub agent: Agent,
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// The project folder: `devcast` finds the project there.
    pub cwd: PathBuf,
    pub env: Vec<(OsString, OsString)>,
    /// The request, written to the agent's stdin.
    pub input: String,
}

/// One request to the agent.
#[derive(Debug, Clone)]
pub struct Ask<'a> {
    pub found: &'a Found,
    /// None — the agent's own default.
    pub model: Option<&'a str>,
    pub project: &'a Path,
    /// The folder of `devcast.exe`, put first on the agent's `PATH`.
    pub devcast: &'a Path,
    /// The file with [`crate::prompt::system`]: too long for a command line.
    pub system: &'a Path,
    pub request: &'a str,
    /// The conversation to go on with (`Event::Session` of the run before).
    pub resume: Option<&'a str>,
    /// The id of this run's undo step.
    pub group: &'a str,
}

/// The command line of `ask`: Claude Code with only `Bash(devcast …)` allowed and nothing to
/// ask about; Codex in its sandbox writing only the project folder.
pub fn launch(ask: &Ask<'_>) -> Launch {
    let mut args: Vec<OsString> = Vec::new();
    let mut input = ask.request.to_owned();
    match ask.found.agent {
        Agent::Claude => {
            add(
                &mut args,
                &["-p", "--output-format", "stream-json", "--verbose"],
            );
            add(&mut args, &["--tools", TOOLS, "--allowedTools"]);
            add(&mut args, &ALLOWED);
            add(
                &mut args,
                &["--permission-prompts", "none", "--strict-mcp-config"],
            );
            add(
                &mut args,
                &["--disable-slash-commands", "--append-system-prompt-file"],
            );
            args.push(ask.system.as_os_str().to_owned());
            if let Some(model) = ask.model {
                add(&mut args, &["--model", model]);
            }
            if let Some(session) = ask.resume {
                add(&mut args, &["--resume", session]);
            }
        }
        Agent::Codex => {
            add(&mut args, &["exec"]);
            if let Some(session) = ask.resume {
                add(&mut args, &["resume", session]);
            }
            add(
                &mut args,
                &[
                    "--json",
                    "--skip-git-repo-check",
                    "--sandbox",
                    "workspace-write",
                ],
            );
            if let Some(model) = ask.model {
                add(&mut args, &["--model", model]);
            }
            add(&mut args, &["-"]);
            // Codex has no flag for more system text: the first request carries it.
            if ask.resume.is_none() {
                input = format!("{}\n\n{input}", crate::prompt::system());
            }
        }
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut folders = vec![ask.devcast.to_path_buf()];
    folders.extend(std::env::split_paths(&path));
    let path = std::env::join_paths(folders).unwrap_or(path);
    Launch {
        agent: ask.found.agent,
        program: ask.found.path.clone(),
        args,
        cwd: ask.project.to_path_buf(),
        env: vec![("PATH".into(), path), (GROUP_ENV.into(), ask.group.into())],
        input,
    }
}

/// A running agent; dropping it stops the agent with its children.
#[derive(Debug)]
pub struct Job {
    child: Child,
    events: Receiver<Event>,
}

impl Job {
    /// Starts `launch` and reads its output on a thread that calls `wake` after every event.
    pub fn start(launch: Launch, wake: impl Fn() + Send + 'static) -> std::io::Result<Self> {
        let mut command = Command::new(&launch.program);
        command
            .args(&launch.args)
            .current_dir(&launch.cwd)
            .envs(launch.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        no_window(&mut command);
        let mut child = command.spawn()?;
        let (Some(mut input), Some(output), Some(errors)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            return Err(std::io::Error::other("нет каналов процесса нейронки"));
        };
        let (sender, events) = channel();
        let job = Self { child, events };
        let agent = launch.agent;
        let text = launch.input;
        std::thread::Builder::new()
            .name("devcast-agent".to_owned())
            .spawn(move || {
                // Read on its own thread: a full pipe would stall the agent.
                let complaint = std::thread::spawn(move || last_line(BufReader::new(errors)));
                // A failed write shows as the agent's own answer or as no answer.
                let _ = input.write_all(text.as_bytes());
                drop(input);
                let complaint = || complaint.join().unwrap_or_default();
                listen(agent, BufReader::new(output), &sender, &wake, complaint);
            })?;
        Ok(job)
    }

    /// The events that came since the last call.
    pub fn receive(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }

    /// Waits for the next event; `None` once the run has ended and all were taken.
    pub fn next(&self) -> Option<Event> {
        self.events.recv().ok()
    }

    /// Stops the agent and the commands it runs.
    pub fn stop(&mut self) {
        if let Ok(Some(_)) = self.child.try_wait() {
            return;
        }
        kill_tree(&mut self.child);
        let _ = self.child.wait();
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Sends the events of `output` until the end of the run; output that ends without it is a
/// failure that tells the agent's `complaint` (the last line of its stderr). Stops when nobody
/// listens any more.
fn listen(
    agent: Agent,
    output: impl BufRead,
    sender: &Sender<Event>,
    wake: &dyn Fn(),
    complaint: impl FnOnce() -> String,
) {
    let mut reader = Reader::new(agent);
    for line in output.lines() {
        let Ok(line) = line else { break };
        for event in reader.line(&line) {
            let last = matches!(event, Event::Finished(_));
            if sender.send(event).is_err() {
                return;
            }
            wake();
            if last {
                return;
            }
        }
    }
    if sender
        .send(Event::Finished(Err(no_answer(&complaint()))))
        .is_ok()
    {
        wake();
    }
}

/// The last line with text, at most 200 characters of it.
fn last_line(errors: impl BufRead) -> String {
    let last = errors
        .lines()
        .map_while(Result::ok)
        .filter(|line| !line.trim().is_empty())
        .last();
    last.map_or(String::new(), |line| {
        line.trim().chars().take(200).collect()
    })
}

/// Why a run ended without an answer, and what to do.
fn no_answer(complaint: &str) -> String {
    if complaint.contains("unknown option") {
        format!(
            "нейронка устарела ({complaint}): обнови её (Claude Code — `claude update`) и попробуй ещё раз"
        )
    } else if complaint.is_empty() {
        "нейронка закончила работу без ответа: попробуй ещё раз".to_owned()
    } else {
        format!("нейронка закончила работу без ответа ({complaint}): попробуй ещё раз")
    }
}

/// An npm `.cmd` shim runs node under cmd.exe: killing only it leaves the agent running, so
/// Windows stops the whole tree.
fn kill_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        let mut taskkill = Command::new("taskkill");
        taskkill
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        no_window(&mut taskkill);
        if taskkill.status().is_ok_and(|s| s.success()) {
            return;
        }
    }
    let _ = child.kill();
}

/// A console program started from the app opens no console window.
pub(crate) fn no_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;

fn add(args: &mut Vec<OsString>, items: &[&str]) {
    args.extend(items.iter().map(OsString::from));
}
