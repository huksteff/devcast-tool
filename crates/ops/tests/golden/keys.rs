//! Task 5.26: the real shortcuts of the fixture `typing.devcast` over bar videos.

use std::error::Error;
use std::path::Path;

use dc_capture::input::Telemetry;
use dc_core::{KeyTracks, Project};
use dc_fx_keys::{Keys, KeysSettings, KeysState, Position};
use dc_render::Rgba;

use crate::common::TestResult;

fn fixture(settings: KeysSettings) -> Result<(Project, KeyTracks), Box<dyn Error>> {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/typing.devcast");
    let mut project: Project =
        serde_json::from_str(&std::fs::read_to_string(root.join("project.json"))?)?;
    let media = project
        .clips
        .first()
        .ok_or("fixture without clips")?
        .media
        .path
        .clone();
    let presses = Telemetry::read(&root.join("media/recording-1"))?.key_presses();
    project.set_state::<Keys>(&KeysState { settings })?;
    Ok((project, KeyTracks::from([(media, presses)])))
}

fn check(name: &str, settings: KeysSettings, at: f64) -> TestResult {
    let (project, keys) = fixture(KeysSettings {
        show: true,
        ..settings
    })?;
    crate::common::check_keys(&format!("keys/{name}"), &project, keys, at)
}

#[test]
fn golden_shortcut_at_the_bottom() -> TestResult {
    // Ctrl+Shift+P, pressed at 3.76 s.
    check("bottom", KeysSettings::default(), 3.85)
}

#[test]
fn golden_repeat_counted_in_the_corner() -> TestResult {
    // The third Left at 4.52 s.
    let settings = KeysSettings {
        position: Position::BottomRight,
        size: 1.5,
        ..KeysSettings::default()
    };
    check("repeat", settings, 4.6)
}

#[test]
fn golden_own_colours_at_the_top_left() -> TestResult {
    // Enter at 2.19 s, faded in.
    let settings = KeysSettings {
        position: Position::TopLeft,
        color: Rgba([0x11, 0x18, 0x27, 0xff]),
        background: Rgba([0xff, 0xd6, 0x0a, 0xe6]),
        ..KeysSettings::default()
    };
    check("top_left", settings, 2.5)
}
