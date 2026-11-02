//! Task 5.14: the export queue runs jobs one by one in order, shows where each is, cancels a
//! queued job before it starts and a running one at its next frame. Jobs here are stand-ins
//! that a test steps through frame by frame.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use dc_core::{PixelSize, TimeDelta};
use dc_encode::export::{Exported, Progress, Stages};
use dc_encode::{EncodeError, ExportQueue, Job, JobState};

/// A job of `frames` frames: waits for `step`, reports progress, then tells `events` (so a
/// heard event means the queue already knows that frame).
fn stepped(name: &'static str, frames: u64, events: Sender<String>) -> (Job, Sender<()>) {
    let (step, steps): (Sender<()>, Receiver<()>) = channel();
    let job: Job = Box::new(move |progress| {
        for done in 1..=frames {
            steps.recv().unwrap();
            let flow = progress(Progress {
                done,
                total: frames,
            });
            events.send(format!("{name} {done}")).unwrap();
            if flow == ControlFlow::Break(()) {
                return Err(EncodeError::Cancelled);
            }
        }
        Ok(Exported {
            path: PathBuf::from(name),
            size: PixelSize {
                width: 2,
                height: 2,
            },
            fps: 30,
            frames,
            duration: TimeDelta::from_micros(1),
            codec: None,
            bytes: 1,
            stages: Stages::default(),
        })
    });
    (job, step)
}

fn states(queue: &ExportQueue) -> Vec<String> {
    queue
        .status()
        .iter()
        .map(|s| match &s.state {
            JobState::Queued => format!("{} queued", s.label),
            JobState::Running { progress, .. } => {
                format!("{} {}/{}", s.label, progress.done, progress.total)
            }
            JobState::Done { .. } => format!("{} done", s.label),
            JobState::Failed(e) => format!("{} failed: {e}", s.label),
            JobState::Cancelled => format!("{} cancelled", s.label),
        })
        .collect()
}

#[test]
fn jobs_run_one_after_another_in_order() {
    let queue = ExportQueue::new().unwrap();
    let (events, heard) = channel();
    let (a, step_a) = stepped("a", 2, events.clone());
    let (b, step_b) = stepped("b", 1, events);
    let id_a = queue.push("a".into(), a);
    let id_b = queue.push("b".into(), b);
    assert_eq!((id_a, id_b), (1, 2));
    step_a.send(()).unwrap();
    assert_eq!(heard.recv().unwrap(), "a 1");
    step_b.send(()).unwrap();
    step_a.send(()).unwrap();
    assert_eq!(heard.recv().unwrap(), "a 2", "b waits until a has finished");
    assert_eq!(heard.recv().unwrap(), "b 1");
    assert!(matches!(queue.wait(id_b), Some(JobState::Done { .. })));
    assert_eq!(states(&queue), ["a done", "b done"]);
    assert_eq!(queue.wait(7), None);
}

#[test]
fn status_shows_progress_and_the_queue() {
    let queue = ExportQueue::new().unwrap();
    let (events, heard) = channel();
    let (a, step_a) = stepped("a", 3, events.clone());
    let (b, _step_b) = stepped("b", 1, events);
    queue.push("a".into(), a);
    queue.push("b".into(), b);
    step_a.send(()).unwrap();
    heard.recv().unwrap();
    let shown = states(&queue);
    assert_eq!(shown[0], "a 1/3");
    assert_eq!(shown[1], "b queued");
    queue.cancel(None);
    step_a.send(()).unwrap();
    queue.wait_idle();
}

#[test]
fn cancel_stops_the_running_job_and_skips_a_queued_one() {
    let queue = ExportQueue::new().unwrap();
    let (events, heard) = channel();
    let (a, step_a) = stepped("a", 5, events.clone());
    let (b, _step_b) = stepped("b", 1, events);
    let id_a = queue.push("a".into(), a);
    let id_b = queue.push("b".into(), b);
    step_a.send(()).unwrap();
    assert_eq!(heard.recv().unwrap(), "a 1");
    assert_eq!(queue.cancel(Some(id_b)), [id_b]);
    assert_eq!(queue.cancel(Some(id_a)), [id_a]);
    assert_eq!(
        queue.cancel(Some(id_a)),
        Vec::<u32>::new(),
        "already cancelled"
    );
    step_a.send(()).unwrap();
    assert_eq!(queue.wait(id_a), Some(JobState::Cancelled));
    assert_eq!(queue.wait(id_b), Some(JobState::Cancelled));
    assert_eq!(heard.recv().unwrap(), "a 2");
    assert!(heard.try_recv().is_err(), "b never ran");
}

#[test]
fn a_failed_job_keeps_its_message_and_the_next_one_runs() {
    let queue = ExportQueue::new().unwrap();
    let failing: Job = Box::new(|_| Err(EncodeError::NothingLeft));
    let (events, heard) = channel();
    let (b, step_b) = stepped("b", 1, events);
    let id = queue.push("a".into(), failing);
    queue.push("b".into(), b);
    step_b.send(()).unwrap();
    heard.recv().unwrap();
    queue.wait_idle();
    let Some(JobState::Failed(message)) = queue.wait(id) else {
        panic!("{:?}", queue.status());
    };
    assert!(message.contains("всё вырезано"), "{message}");
    assert_eq!(states(&queue)[1], "b done");
}

#[test]
fn a_panicking_job_fails_and_the_next_one_runs() {
    let queue = ExportQueue::new().unwrap();
    let panicking: Job = Box::new(|_| panic!("сломано"));
    let (events, heard) = channel();
    let (b, step_b) = stepped("b", 1, events);
    let id = queue.push("a".into(), panicking);
    queue.push("b".into(), b);
    let Some(JobState::Failed(message)) = queue.wait(id) else {
        panic!("{:?}", queue.status());
    };
    assert!(message.contains("сломано"), "{message}");
    step_b.send(()).unwrap();
    heard.recv().unwrap();
    queue.wait_idle();
    assert_eq!(states(&queue)[1], "b done");
}

#[test]
fn a_watched_queue_hears_the_start_each_frame_and_the_end() {
    let (told, heard_watch) = channel::<String>();
    let told = std::sync::Mutex::new(told);
    let queue = ExportQueue::watched(Box::new(move |status| {
        let state = match &status.state {
            JobState::Running { progress, .. } => {
                format!("{}/{}", progress.done, progress.total)
            }
            JobState::Done { .. } => "done".to_owned(),
            other => format!("{other:?}"),
        };
        let line = format!("{} {state}", status.label);
        told.lock().unwrap().send(line).unwrap();
    }))
    .unwrap();
    let (events, heard) = channel();
    let (a, step_a) = stepped("a", 2, events);
    queue.push("a".into(), a);
    step_a.send(()).unwrap();
    step_a.send(()).unwrap();
    heard.recv().unwrap();
    heard.recv().unwrap();
    let seen: Vec<String> = heard_watch.iter().take(4).collect();
    assert_eq!(seen, ["a 0/0", "a 1/2", "a 2/2", "a done"]);
}
