//! Signing Claude Code in from the app, without its console window: `claude auth login` runs
//! with pipes, tells the page to open («If the browser didn't open, visit: https://…») and
//! waits on its stdin for the code that page shows. The app opens the page in the browser
//! itself and hands the code over.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::find::Found;

/// A sign-in that runs; dropping it ends it.
#[derive(Debug)]
pub struct Login {
    child: Child,
    input: Option<ChildStdin>,
    lines: Receiver<String>,
}

impl Login {
    /// Starts the sign-in of `found`; `wake` is called when it said something or ended.
    pub fn start(found: &Found, wake: impl Fn() + Send + Clone + 'static) -> std::io::Result<Self> {
        let mut command = Command::new(&found.path);
        command.args(crate::sign_in::login_args(found.agent));
        Self::spawn(command, wake)
    }

    fn spawn(
        mut command: Command,
        wake: impl Fn() + Send + Clone + 'static,
    ) -> std::io::Result<Self> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        crate::run::no_window(&mut command);
        let mut child = command.spawn()?;
        let (sender, lines) = channel();
        if let Some(output) = child.stdout.take() {
            listen(output, sender.clone(), wake.clone())?;
        }
        if let Some(errors) = child.stderr.take() {
            listen(errors, sender, wake)?;
        }
        Ok(Self {
            input: child.stdin.take(),
            child,
            lines,
        })
    }

    /// The page to open in the browser, once: none until the agent tells it and after it was
    /// taken.
    pub fn page(&mut self) -> Option<String> {
        self.lines.try_iter().find_map(|line| page(&line))
    }

    /// Hands over the code the page showed.
    pub fn send_code(&mut self, code: &str) -> std::io::Result<()> {
        let Some(input) = self.input.as_mut() else {
            return Err(std::io::Error::other("вход уже закончился"));
        };
        writeln!(input, "{}", code.trim())?;
        input.flush()
    }

    /// Whether the sign-in ended, and well; none while it runs.
    pub fn ended(&mut self) -> Option<bool> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(status.success()),
            Ok(None) => None,
            Err(_) => Some(false),
        }
    }
}

impl Drop for Login {
    fn drop(&mut self) {
        if self.ended().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Sends the lines of `output` on a thread of its own; wakes after each and at the end.
fn listen(
    output: impl Read + Send + 'static,
    sender: Sender<String>,
    wake: impl Fn() + Send + 'static,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("devcast-agent-login".to_owned())
        .spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    return;
                }
                wake();
            }
            wake();
        })
        .map(drop)
}

/// The address in `line`, from `https://` to the next space.
fn page(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let address = line[start..].split_whitespace().next()?;
    Some(address.to_owned())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn the_page_is_the_address_of_the_line() {
        let line = "If the browser didn't open, visit: https://claude.com/cai/oauth/authorize?code=true&state=A-1";
        assert_eq!(
            page(line).as_deref(),
            Some("https://claude.com/cai/oauth/authorize?code=true&state=A-1")
        );
        assert_eq!(page("Opening browser to sign in…"), None);
    }

    fn wait<T>(mut ready: impl FnMut() -> Option<T>) -> T {
        let started = Instant::now();
        loop {
            if let Some(value) = ready() {
                return value;
            }
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "the pretend agent hangs"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// A pretend agent: tells a page, then ends well only with the code 4242.
    #[cfg(windows)]
    #[test]
    fn the_page_comes_out_and_the_code_goes_in() {
        let run = || {
            let mut command = Command::new("cmd");
            command.args(["/V:ON", "/C", "echo visit: https://example.test/login?code=true& set /p code=& if !code!==4242 (exit /b 0) else (exit /b 1)"]);
            Login::spawn(command, || {}).unwrap()
        };
        let mut login = run();
        let address = wait(|| login.page());
        assert_eq!(address, "https://example.test/login?code=true");
        assert_eq!(login.page(), None, "told once");
        assert_eq!(login.ended(), None, "waits for the code");
        login.send_code(" 4242 ").unwrap();
        assert!(wait(|| login.ended()), "the right code signs in");

        let mut wrong = run();
        wait(|| wrong.page());
        wrong.send_code("1").unwrap();
        assert!(!wait(|| wrong.ended()));
    }
}
