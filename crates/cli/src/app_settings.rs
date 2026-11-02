//! The app settings `settings.json` for `devcast config` and `devcast hotkeys`: an edit
//! (`dc_settings::Edit`) goes to the running app, the one writer then (`Request::Settings`: its
//! windows and global hotkeys follow at once); without the app `devcast` applies the same
//! `dc_settings::apply` to the file. Under `--dry-run` the edit is applied to a copy.

use dc_ops::live::{Answer as Live, Request};
use dc_settings::{Applied, Edit, EditError, Store};
use serde_json::{Value, json};

use crate::edit::Ctx;
use crate::error::CliError;
use crate::live::{app_channel, ask_app, refused};

/// Who applied an edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum By {
    /// The running app: its windows show it already.
    App,
    /// `devcast` itself, in the file: the app reads it when it starts.
    File,
    DryRun,
}

impl By {
    /// Before the changed lines.
    pub(crate) fn prefix(self) -> &'static str {
        match self {
            Self::App => "",
            Self::File => "",
            Self::DryRun => "dry-run, настройки не изменены: ",
        }
    }

    pub(crate) fn json(self) -> Value {
        json!({
            "app": self == Self::App,
            "dry_run": self == Self::DryRun,
        })
    }
}

/// Applies `edit` where the settings live now.
pub(crate) fn apply(ctx: &Ctx<'_>, edit: Edit) -> Result<(Applied, By), CliError> {
    if ctx.globals.dry_run {
        let applied = dc_settings::apply(&ctx.app_settings()?, edit).map_err(refusal)?;
        return Ok((applied, By::DryRun));
    }
    let channel = app_channel(ctx.local.app_channel)?;
    let request = Request::Settings { edit: edit.clone() };
    match ask_app(&channel, &request)? {
        Some(Live::Configured(applied)) => return Ok((*applied, By::App)),
        Some(Live::NotConfigured(error)) => return Err(refusal(error)),
        Some(answer) => return Err(refused(answer)),
        None => {}
    }
    let mut store = Store::load(&ctx.app_settings_file()?)?;
    let applied = dc_settings::apply(store.get(), edit).map_err(refusal)?;
    let settings = applied.settings.clone();
    store.update(|s| *s = settings)?;
    Ok((applied, By::File))
}

fn refusal(error: EditError) -> CliError {
    CliError::Failed(error.to_string())
}

/// A value as written after `=`: text with spaces is quoted, as `defaults show` does.
pub(crate) fn argument(text: &str) -> String {
    match text.is_empty() || text.contains(char::is_whitespace) {
        true => format!("\"{text}\""),
        false => text.to_owned(),
    }
}

/// `on` / `off`.
pub(crate) fn switch(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// The changed lines, or that nothing changed.
pub(crate) fn changed_text(by: By, lines: Vec<String>) -> String {
    let body = match lines.is_empty() {
        true => "без изменений".to_owned(),
        false => lines.join("\n"),
    };
    let tail = match (by, lines.is_empty()) {
        (By::App, false) => "\nпрограмма DevCast применила сразу",
        _ => "",
    };
    format!("{}{body}{tail}", by.prefix())
}
