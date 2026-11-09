//! Updates without a network: a build of the repository is no installation, so the states
//! the threads would bring are set by hand and the events are sent as the threads send them.

use std::sync::mpsc::Receiver;

use dc_ops::live::{Answer, Request, UpdateState};
use eframe::egui;

use super::*;
use crate::session::Session;

fn shared(dir: &std::path::Path) -> Shared {
    let store = crate::settings::Store::load(&dir.join("settings.json")).unwrap();
    Shared::new(egui::Context::default(), store)
}

fn ask(shared: &mut Shared, install: bool) -> Receiver<Answer> {
    let (reply, answer) = Reply::fake();
    shared.call(Request::Update { install }, reply);
    answer
}

fn available(version: &str) -> UpdateState {
    UpdateState::Available {
        version: version.to_owned(),
    }
}

#[test]
fn a_build_of_the_repository_has_no_updates() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    shared.updates_tick();
    assert_eq!(shared.updates.state, UpdateState::NotInstalled);
    let answer = ask(&mut shared, false).try_recv().unwrap();
    assert_eq!(answer, Answer::Update(UpdateState::NotInstalled));
    assert!(shared.popover.is_none());
}

#[test]
fn a_found_version_is_asked_about_once_until_the_snooze_ends() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    shared.updates.state = available("9.9.9");
    shared.ask_about_update();
    assert_eq!(shared.popover.map(|o| o.kind), Some(Kind::Update));
    shared.popover = None;
    shared.ask_about_update();
    assert!(
        shared.popover.is_none(),
        "«Не сейчас» or a lost focus: 3 hours"
    );
    shared.updates.snoozed_until = Some(0);
    shared.ask_about_update();
    assert!(shared.popover.is_some());
}

#[test]
fn a_skipped_version_is_not_asked_about_but_a_newer_one_is() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    shared.updates.state = available("9.9.9");
    shared.skip_update();
    let reread = crate::settings::Store::load(&dir.path().join("settings.json")).unwrap();
    assert_eq!(reread.get().updates.skipped.as_deref(), Some("9.9.9"));
    shared.ask_about_update();
    assert!(shared.popover.is_none());
    shared.updates.state = available("9.9.10");
    shared.ask_about_update();
    assert!(shared.popover.is_some());
}

#[test]
fn nothing_is_asked_while_recording_and_install_waits_for_its_end() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    shared.record();
    assert!(matches!(shared.session, Session::Countdown(_)));
    shared.updates.state = available("9.9.9");
    shared.ask_about_update();
    assert!(shared.popover.is_none());
    let answer = ask(&mut shared, true).try_recv().unwrap();
    assert!(matches!(answer, Answer::Refused(text) if text.contains("запись")));
}

#[test]
fn a_check_is_remembered_and_answers_the_waiting_runs() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    let (reply, answer) = Reply::fake();
    shared.updates.waiters.push((false, reply));
    shared.updates_heard(Event::Checked(Ok(None)));
    let up_to_date = UpdateState::UpToDate {
        version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    assert_eq!(answer.try_recv().unwrap(), Answer::Update(up_to_date));
    assert!(shared.store.get().updates.checked_at.is_some());
}

#[test]
fn a_failed_check_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    let (reply, answer) = Reply::fake();
    shared.updates.waiters.push((true, reply));
    let error = UpdateError::Network("нет сети".to_owned());
    shared.updates_heard(Event::Checked(Err(error)));
    let Answer::Update(UpdateState::Failed { error }) = answer.try_recv().unwrap() else {
        panic!("not a failure");
    };
    assert!(error.contains("нет сети") && error.contains("Проверь интернет"));
    assert!(shared.store.get().updates.checked_at.is_none());
}

#[test]
fn a_downloaded_version_is_told_once_and_restarts_on_its_button() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = shared(dir.path());
    shared.updates.state = UpdateState::Ready {
        version: "9.9.9".to_owned(),
    };
    shared.ask_about_update();
    assert_eq!(shared.popover.map(|o| o.kind), Some(Kind::Update));
    shared.popover = None;
    shared.ask_about_update();
    assert!(shared.popover.is_none(), "told once");
    shared.restart_into_update();
    assert!(shared.updates.restart);
}
