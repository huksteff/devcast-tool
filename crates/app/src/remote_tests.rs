//! `devcast` asks the app: the recording through the HUD's paths with a fake recorder, the
//! export queue with a stand-in job, the status.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use dc_capture::HostTime;
use dc_ops::export::{Format, Settings, Spec};
use dc_ops::export_jobs::Finished;
use dc_ops::live::{Answer, Once, RecordAnswer, RecordControl, RecordingState, Request};
use eframe::egui;

use crate::app::Shared;
use crate::exports::{Exports, Runner};
use crate::instance::Reply;
use crate::recorder::{Event, Recorder, Request as Recording};
use crate::session::Session;

fn shared(dir: &std::path::Path) -> Shared {
    let store = crate::settings::Store::load(&dir.join("settings.json")).unwrap();
    Shared::new(egui::Context::default(), store)
}

/// Asks `request`; the answer, if it came already.
fn ask(shared: &mut Shared, request: Request) -> Receiver<Answer> {
    let (reply, answer) = Reply::fake();
    shared.call(request, reply);
    answer
}

fn control(shared: &mut Shared, control: RecordControl) -> Receiver<Answer> {
    ask(shared, Request::RecordControl(control))
}

#[test]
fn record_starts_the_countdown_and_cancel_ends_it_for_both_runs() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    let started = ask(&mut shared, Request::Record(Once::default()));
    assert!(matches!(shared.session, Session::Countdown(_)));
    assert!(started.try_recv().is_err(), "the run waits for the start");
    let again = ask(&mut shared, Request::Record(Once::default()))
        .try_recv()
        .unwrap();
    assert!(matches!(
        again,
        Answer::Record(RecordAnswer::NotNow(RecordingState::Countdown { .. }))
    ));
    let paused = control(&mut shared, RecordControl::Pause)
        .try_recv()
        .unwrap();
    assert!(matches!(paused, Answer::Record(RecordAnswer::NotNow(_))));
    let cancelled = control(&mut shared, RecordControl::Cancel);
    let done = Answer::Record(RecordAnswer::CountdownCancelled);
    assert_eq!(cancelled.try_recv().unwrap(), done);
    assert_eq!(started.try_recv().unwrap(), done);
    assert!(shared.session.is_idle());
    let idle = control(&mut shared, RecordControl::Stop)
        .try_recv()
        .unwrap();
    assert_eq!(
        idle,
        Answer::Record(RecordAnswer::NotNow(RecordingState::Idle))
    );
}

/// The start, a pause and a cancel without the question, each answered by the recorder's event.
#[test]
fn control_is_answered_once_the_recorder_has_done_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    let root = dir.path().join("Запись 1.devcast");
    let (recorder, events, requests) = Recorder::fake(root.clone());
    let started = ask(&mut shared, Request::Record(Once::default()));
    shared.session = Session::Starting {
        recorder,
        stop: false,
    };
    events
        .send(Event::Started(HostTime::from_micros(5)))
        .unwrap();
    shared.tick();
    let answer = Answer::Record(RecordAnswer::Started { root: root.clone() });
    assert_eq!(started.try_recv().unwrap(), answer);

    let paused = control(&mut shared, RecordControl::Pause);
    assert_eq!(requests.try_iter().collect::<Vec<_>>(), [Recording::Pause]);
    assert!(paused.try_recv().is_err());
    events
        .send(Event::Paused(HostTime::from_micros(2_000_005)))
        .unwrap();
    shared.tick();
    let Answer::Record(RecordAnswer::Paused { recorded }) = paused.try_recv().unwrap() else {
        panic!("pause must answer with the time recorded");
    };
    assert!((recorded - 2.0).abs() < 1e-9, "{recorded}");
    let twice = control(&mut shared, RecordControl::Pause)
        .try_recv()
        .unwrap();
    assert!(matches!(
        twice,
        Answer::Record(RecordAnswer::NotNow(RecordingState::Recording {
            paused: true,
            ..
        }))
    ));

    let cancelled = control(&mut shared, RecordControl::Cancel);
    assert!(matches!(shared.session, Session::Cancelling(_)));
    assert!(shared.popover.is_none(), "no «Отменить запись?» question");
    assert_eq!(requests.try_iter().collect::<Vec<_>>(), [Recording::Cancel]);
    events.send(Event::Cancelled).unwrap();
    shared.tick();
    assert_eq!(
        cancelled.try_recv().unwrap(),
        Answer::Record(RecordAnswer::Cancelled { root })
    );
    assert!(shared.session.is_idle());
}

/// A stop while the recording starts waits for the start and answers with the failure of the
/// save; a cancel while it starts is sent to the recorder at once.
#[test]
fn stop_and_cancel_while_starting() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    let (recorder, events, requests) = Recorder::fake(dir.path().join("а.devcast"));
    shared.session = Session::Starting {
        recorder,
        stop: false,
    };
    let stopped = control(&mut shared, RecordControl::Stop);
    events
        .send(Event::Started(HostTime::from_micros(5)))
        .unwrap();
    shared.tick();
    assert_eq!(requests.try_iter().collect::<Vec<_>>(), [Recording::Stop]);
    events.send(Event::Failed("диск полон".to_owned())).unwrap();
    shared.tick();
    assert_eq!(
        stopped.try_recv().unwrap(),
        Answer::Record(RecordAnswer::Failed("диск полон".to_owned()))
    );

    let (recorder, events, requests) = Recorder::fake(dir.path().join("б.devcast"));
    shared.session = Session::Starting {
        recorder,
        stop: false,
    };
    let cancelled = control(&mut shared, RecordControl::Cancel);
    assert_eq!(requests.try_iter().collect::<Vec<_>>(), [Recording::Cancel]);
    events
        .send(Event::Started(HostTime::from_micros(5)))
        .unwrap();
    events.send(Event::Cancelled).unwrap();
    shared.tick();
    assert!(matches!(
        cancelled.try_recv().unwrap(),
        Answer::Record(RecordAnswer::Cancelled { .. })
    ));
}

/// The app's queue: an export of `devcast` joins it and is answered when it has ended; status
/// and cancel see it; without a queue the run is told to export by itself.
#[test]
fn exports_join_the_app_queue() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    let spec = Spec {
        root: dir.path().to_path_buf(),
        project: dc_core::Project::default(),
        settings: Settings::default_for(Format::Mp4),
        out: dir.path().join("урок.mp4"),
    };
    let none = ask(&mut shared, Request::Export(Box::new(spec.clone())));
    assert_eq!(none.try_recv().unwrap(), Answer::NoExports);
    let runner: Runner = Arc::new(|_spec: Spec| -> dc_encode::Job {
        Box::new(|_progress| Err(dc_encode::EncodeError::NothingLeft))
    });
    shared.exports = Some(Exports::new(runner, || {}).unwrap());
    let exported = ask(&mut shared, Request::Export(Box::new(spec)));
    let Answer::Exported(Finished::Failed { id, .. }) = exported.recv().unwrap() else {
        panic!("the stand-in job fails");
    };
    let Answer::Exports { jobs } = ask(&mut shared, Request::Exports).recv().unwrap() else {
        panic!("status lists the jobs");
    };
    assert_eq!(
        (jobs.len(), jobs[0].id, jobs[0].state.as_str()),
        (1, id, "failed")
    );
    assert_eq!(shared.status().exports, jobs);
    let cancel = ask(&mut shared, Request::CancelExports { id: None });
    assert_eq!(
        cancel.recv().unwrap(),
        Answer::ExportsCancelled { ids: Vec::new() }
    );
}

#[test]
fn status_without_an_editor() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    let status = ask(&mut shared, Request::Status).recv().unwrap();
    let Answer::Status(status) = status else {
        panic!("status answers with the state");
    };
    assert_eq!(status.recording, RecordingState::Idle);
    assert_eq!(status.editor, None);
    let root = dir.path().join("нет.devcast");
    let stop = ask(&mut shared, Request::StopCaptions { root });
    assert_eq!(stop.recv().unwrap(), Answer::NotOpen);
}

/// `record --app --duration`: the recording stops by itself and the run hears only the save.
#[test]
fn a_recording_with_a_duration_stops_by_itself() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    let (recorder, events, requests) = Recorder::fake(dir.path().join("Запись 1.devcast"));
    let once = Once {
        source: None,
        duration: Some(0.5),
    };
    let asked = ask(&mut shared, Request::Record(once));
    shared.session = Session::Starting {
        recorder,
        stop: false,
    };
    // The first frame came long ago by the host clock: the duration is over.
    events
        .send(Event::Started(HostTime::from_micros(5)))
        .unwrap();
    shared.tick();
    assert!(matches!(shared.session, Session::Preparing(_)));
    assert_eq!(requests.try_iter().collect::<Vec<_>>(), [Recording::Stop]);
    assert!(asked.try_recv().is_err(), "the run waits for the save");
}
