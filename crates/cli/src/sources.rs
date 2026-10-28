//! `devcast ls sources`: what `devcast record --source` and `--mic` can take right now.

use dc_capture::audio::{MicrophoneInfo, list_microphones};
use dc_capture::win::list_sources;
use dc_capture::{MonitorInfo, SourceList, WindowInfo};
use serde_json::{Value, json};

use crate::error::CliError;
use crate::run::Answer;

/// Name of the list in `devcast ls`.
pub(crate) const SOURCES: &str = "sources";

pub(crate) fn ls() -> Result<Answer, CliError> {
    let failed = |e: dc_capture::CaptureError| CliError::Failed(e.to_string());
    let sources = list_sources().map_err(failed)?;
    let microphones = list_microphones().map_err(failed)?;
    Ok(answer(&sources, &microphones))
}

/// One line per source: monitors, windows (topmost first), microphones.
pub(crate) fn answer(sources: &SourceList, microphones: &[MicrophoneInfo]) -> Answer {
    let lines = sources
        .monitors
        .iter()
        .map(monitor_line)
        .chain(sources.windows.iter().map(window_line))
        .chain(microphones.iter().map(microphone_line));
    let mut text: Vec<String> = lines.collect();
    if text.is_empty() {
        text.push("источников нет: подключи монитор".to_owned());
    }
    let json = json!({
        "monitors": sources.monitors.iter().map(monitor_json).collect::<Vec<_>>(),
        "windows": sources.windows.iter().map(window_json).collect::<Vec<_>>(),
        "microphones": microphones,
    });
    Answer::new(text.join("\n"), json)
}

fn monitor_line(m: &MonitorInfo) -> String {
    let b = m.bounds;
    let mut line = format!(
        "{}  {}x{} в {},{}",
        m.source(),
        b.width(),
        b.height(),
        b.x(),
        b.y()
    );
    if m.primary {
        line.push_str("  основной");
    }
    if let Some(name) = &m.name {
        line.push_str(&format!("  {name}"));
    }
    line
}

fn window_line(w: &WindowInfo) -> String {
    let mut line = format!("{}  ", w.source());
    if let Some(app) = &w.app {
        line.push_str(&format!("{app} — "));
    }
    line.push_str(&w.title);
    if w.minimized {
        line.push_str("  (свёрнуто: разверни перед записью)");
    }
    line
}

fn microphone_line(m: &MicrophoneInfo) -> String {
    let default = if m.default {
        "  по умолчанию"
    } else {
        ""
    };
    format!("--mic \"{}\"{default}", m.name)
}

fn monitor_json(m: &MonitorInfo) -> Value {
    let b = m.bounds;
    json!({
        "source": m.source(),
        "name": m.name,
        "x": b.x(),
        "y": b.y(),
        "width": b.width(),
        "height": b.height(),
        "primary": m.primary,
    })
}

fn window_json(w: &WindowInfo) -> Value {
    json!({
        "source": w.source(),
        "title": w.title,
        "app": w.app,
        "minimized": w.minimized,
    })
}
