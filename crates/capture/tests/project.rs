//! Recording into a new project on the real desktop of this computer.
//!
//! `#[ignore]`: draws a moving test picture on monitor `DEVCAST_TEST_MONITOR` and records an
//! area of it. `DEVCAST_TEST_MONITOR=<n> cargo nextest run -p dc-capture --test project --run-ignored=all --no-capture`.

#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use std::time::Duration;

use common::{TestPicture, fresh_dir, test_monitor};
use dc_capture::input::read_cursor;
use dc_capture::win::ProjectRecording;
use dc_capture::{Area, Fps, Manifest, RecordingConfig, Source};
use dc_project::{Defaults, ProjectDir};

#[test]
#[ignore = "records the screen: needs DEVCAST_TEST_MONITOR"]
fn stop_saves_a_project_and_cancel_deletes_it() {
    let monitor = test_monitor();
    let _picture = TestPicture::show(&monitor, 20);
    let b = monitor.bounds;
    let area = Area::new(b.x() + 100, b.y() + 100, 1280, 720).unwrap();
    let config = RecordingConfig {
        fps: Fps::Fps30,
        ..RecordingConfig::new(Source::Area(area))
    };

    let root = fresh_dir("Запись проекта.dcproj");
    let recording =
        ProjectRecording::start(&config, &root, "devcast test", Defaults::factory()).unwrap();
    std::thread::sleep(Duration::from_secs(3));
    let recorded = recording.stop().unwrap();
    let project = ProjectDir::open(&root).unwrap().load().unwrap();
    assert_eq!(project, recorded.project);
    let [clip] = &project.clips[..] else {
        panic!("{:?}", project.clips)
    };
    assert_eq!(clip.media.path, "media/recording-1/video.mp4");
    assert_eq!((clip.media.size.width, clip.media.size.height), (1280, 720));
    let seconds = clip.media.duration.as_secs_f64();
    assert!((seconds - 3.0).abs() < 0.3, "{seconds}");
    let media = root.join("media/recording-1");
    assert!(media.join("video.mp4").is_file());
    assert!(media.join("system.mp4").is_file());
    assert!(Manifest::read(&media).unwrap().is_finished());
    read_cursor(&media).unwrap();
    println!("{}: {project:?}", root.display());
    std::fs::remove_dir_all(&root).unwrap();

    let root = fresh_dir("Отмена.dcproj");
    let recording =
        ProjectRecording::start(&config, &root, "devcast test", Defaults::factory()).unwrap();
    std::thread::sleep(Duration::from_secs(1));
    recording.cancel().unwrap();
    assert!(!root.exists());
}
