use std::time::{Duration, Instant};

use super::{Countdown, Session, clock};
use crate::recorder::{Event, Recorder, Request};

#[test]
fn countdown_goes_down_once_a_second() {
    let start = Instant::now();
    let mut countdown = Countdown::new(3, start);
    assert!(!countdown.tick(start + Duration::from_millis(999)));
    assert_eq!(countdown.left, 3);
    assert!(!countdown.tick(start + Duration::from_millis(1000)));
    assert_eq!(countdown.left, 2);
    assert!(countdown.tick(start + Duration::from_millis(3500)));
    assert_eq!(countdown.left, 0);
}

#[test]
fn timer_shows_hours_only_past_an_hour() {
    assert_eq!(clock(0), "00:00");
    assert_eq!(clock(767_400_000), "12:47");
    assert_eq!(clock(3_600_000_000 + 63_000_000), "1:01:03");
}

#[test]
fn a_second_stop_keeps_the_recording_being_prepared() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::settings::Store::load(&dir.path().join("settings.json")).unwrap();
    let mut shared = crate::app::Shared::new(eframe::egui::Context::default(), store);
    let (recorder, _events, _requests) = Recorder::fake(dir.path().join("запись.devcast"));
    shared.session = Session::Preparing(recorder);
    shared.stop();
    assert!(
        matches!(shared.session, Session::Preparing(_)),
        "the recorder still saves the project"
    );
    shared.session = Session::Countdown(Countdown::new(3, Instant::now()));
    shared.stop();
    assert!(shared.session.is_idle(), "stop cancels the countdown");
}

#[test]
fn stop_while_starting_stops_right_after_the_start() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::settings::Store::load(&dir.path().join("settings.json")).unwrap();
    let mut shared = crate::app::Shared::new(eframe::egui::Context::default(), store);
    let (recorder, events, requests) = Recorder::fake(dir.path().join("запись.devcast"));
    shared.session = Session::Starting {
        recorder,
        stop: false,
    };
    shared.stop();
    assert!(matches!(
        shared.session,
        Session::Starting { stop: true, .. }
    ));
    assert!(
        requests.try_recv().is_err(),
        "nothing to stop before the start"
    );
    events
        .send(Event::Started(dc_capture::HostTime::from_micros(5)))
        .unwrap();
    shared.tick();
    assert!(matches!(shared.session, Session::Preparing(_)));
    assert_eq!(requests.try_iter().collect::<Vec<_>>(), [Request::Stop]);
}
