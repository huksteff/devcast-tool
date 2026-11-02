use dc_core::project::{Media, MediaKind, SpeedRange};
use dc_core::{Clip, Command, History, PixelSize, Project, Range, Rect, SourceTime, TimeDelta};
use serde_json::{Value, json};

use crate::{
    Action, Callout, CalloutAdd, CalloutRm, CalloutSet, CalloutState, CalloutStyle, EventKind,
};

pub(crate) fn secs(s: f64) -> SourceTime {
    SourceTime::from_secs_f64(s)
}

pub(crate) fn clip(project: &mut Project, length: f64) -> &mut Clip {
    let id = project.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(length);
    project.clips.push(Clip {
        id,
        media: Media {
            path: format!("media/{id}.mp4"),
            kind: MediaKind::Recording,
            duration,
            size: PixelSize {
                width: 1920,
                height: 1080,
            },
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
        audio: Default::default(),
    });
    project.clips.last_mut().unwrap()
}

pub(crate) fn project(lengths: &[f64]) -> Project {
    let mut p = Project::default();
    for length in lengths {
        clip(&mut p, *length);
    }
    p
}

pub(crate) fn add(args: Value) -> CalloutAdd {
    serde_json::from_value(args).unwrap()
}

fn set(args: Value) -> CalloutSet {
    serde_json::from_value(args).unwrap()
}

pub(crate) fn style(args: Value) -> CalloutStyle {
    serde_json::from_value(args).unwrap()
}

fn rm(id: &str) -> CalloutRm {
    serde_json::from_value(json!({ "id": id })).unwrap()
}

fn state(p: &Project) -> CalloutState {
    p.state::<Callout>().unwrap()
}

pub(crate) fn run(p: &mut Project, command: &dyn Command) -> String {
    command.apply(p).unwrap().to_string()
}

fn fail(p: &mut Project, command: &dyn Command) -> String {
    let before = p.clone();
    let error = command.apply(p).unwrap_err().to_string();
    assert_eq!(*p, before, "failed command changed the project");
    error
}

#[test]
fn add_stores_source_time_of_the_recording() {
    let mut p = project(&[10.0]);
    let text = add(json!({"action": "text", "at": 2, "text": "  Жми сюда  ", "duration": 1.5}));
    assert_eq!(run(&mut p, &text), "события облачка +1 co1");
    let event = &state(&p).events[0];
    assert_eq!(event.media, p.clips[0].media.path);
    assert_eq!(event.at, secs(2.0));
    assert_eq!(event.duration, TimeDelta::from_millis(1500));
    assert_eq!(
        event.action,
        Action::Text {
            text: "Жми сюда".into()
        }
    );
    run(&mut p, &add(json!({"action": "on", "at": 1})));
    let on = &state(&p).events[1];
    assert_eq!(
        (on.action.clone(), on.duration),
        (Action::On, TimeDelta::from_secs_f64(1.0))
    );
}

#[test]
fn add_maps_output_time_through_cuts_speed_and_clips() {
    let mut p = project(&[10.0, 10.0]);
    let c = &mut p.clips[0];
    c.cuts.push(Range::new(secs(1.0), secs(3.0)).unwrap());
    c.speeds.push(SpeedRange {
        range: Range::new(secs(4.0), secs(8.0)).unwrap(),
        factor: 2.0,
    });
    run(&mut p, &add(json!({"action": "on", "at": 1.5})));
    run(&mut p, &add(json!({"action": "off", "at": 3.5})));
    run(&mut p, &add(json!({"action": "on", "at": 6.5})));
    let events = state(&p).events;
    assert_eq!(events[0].at, secs(3.5));
    assert_eq!(events[1].at, secs(7.0));
    assert_eq!(events[2].media, p.clips[1].media.path);
    assert_eq!(events[2].at, secs(0.5));
}

#[test]
fn region_lasts_output_time_over_speed_and_starts_after_a_cut() {
    let mut p = project(&[10.0]);
    let c = &mut p.clips[0];
    c.cuts.push(Range::new(secs(1.0), secs(3.0)).unwrap());
    c.speeds.push(SpeedRange {
        range: Range::new(secs(4.0), secs(8.0)).unwrap(),
        factor: 2.0,
    });
    let mut s = CalloutState::default();
    for at in [5.0, 2.0] {
        let id = s.ids.allocate();
        s.events.push(crate::CalloutEvent {
            id,
            media: p.clips[0].media.path.clone(),
            at: secs(at),
            duration: TimeDelta::from_secs_f64(1.0),
            action: Action::Text { text: "x".into() },
        });
    }
    p.set_state::<Callout>(&s).unwrap();
    let map = dc_core::TimeMap::build(&p);
    let spans: Vec<(f64, f64)> = (crate::effect().items)(&p)
        .unwrap()
        .iter()
        .map(|i| i.place.output(&map).unwrap())
        .map(|r| (r.start().as_secs_f64(), r.end().as_secs_f64()))
        .collect();
    assert_eq!(spans, [(2.5, 3.5), (1.0, 2.0)]);
    let shown = s.events[0].output(&map).unwrap();
    assert_eq!(shown.start().as_secs_f64(), 2.5);
}

#[test]
fn add_rejects_bad_arguments() {
    let mut p = project(&[10.0]);
    for (args, message) in [
        (json!({"action": "text", "at": 1}), "нужен текст"),
        (
            json!({"action": "on", "at": 1, "text": "x"}),
            "только у события text",
        ),
        (json!({"action": "text", "at": 1, "text": "   "}), "пустой"),
        (
            json!({"action": "text", "at": 1, "text": "a\nb"}),
            "одна строка",
        ),
        (
            json!({"action": "text", "at": 1, "text": "я".repeat(81)}),
            "длиннее 80",
        ),
        (
            json!({"action": "off", "at": 1, "duration": 0}),
            "больше нуля",
        ),
        (json!({"action": "on", "at": 10}), "не раньше конца видео"),
        (json!({"action": "on", "at": -1}), "раньше начала видео"),
    ] {
        let error = fail(&mut p, &add(args.clone()));
        assert!(error.contains(message), "{args}: {error}");
    }
    let ok = add(json!({"action": "text", "at": 1, "text": "я".repeat(80)}));
    run(&mut p, &ok);
    let error = fail(&mut project(&[]), &add(json!({"action": "on", "at": 0})));
    assert!(error.contains("нет клипов"), "{error}");
}

#[test]
fn set_changes_fields_and_switches_actions() {
    let mut p = project(&[10.0]);
    run(
        &mut p,
        &add(json!({"action": "text", "at": 1, "text": "Раз"})),
    );
    assert_eq!(
        run(&mut p, &set(json!({"id": "co1", "text": "Два", "at": 4}))),
        "события облачка ~1 co1"
    );
    let event = &state(&p).events[0];
    assert_eq!(
        (event.at, &event.action),
        (
            secs(4.0),
            &Action::Text {
                text: "Два".into()
            }
        )
    );
    run(
        &mut p,
        &set(json!({"id": "co1", "action": "off", "duration": 0.5})),
    );
    let event = &state(&p).events[0];
    assert_eq!(
        (event.duration, &event.action),
        (TimeDelta::from_millis(500), &Action::Off)
    );
    let error = fail(&mut p, &set(json!({"id": "co1", "action": "text"})));
    assert!(error.contains("нужен текст"), "{error}");
    let error = fail(&mut p, &set(json!({"id": "co1", "text": "x"})));
    assert!(error.contains("только у события text"), "{error}");
    assert_eq!(run(&mut p, &set(json!({"id": "co1", "at": 4}))), "");
    let error = fail(&mut p, &set(json!({"id": "co7", "at": 1})));
    assert!(error.contains("co7 не найден (есть co1)"), "{error}");
}

#[test]
fn rm_removes_and_ids_are_not_reused() {
    let mut p = project(&[10.0]);
    run(&mut p, &add(json!({"action": "on", "at": 1})));
    run(&mut p, &add(json!({"action": "off", "at": 2})));
    assert_eq!(run(&mut p, &rm("co2")), "события облачка -1 co2");
    run(&mut p, &add(json!({"action": "off", "at": 3})));
    assert_eq!(
        state(&p)
            .event_ids()
            .map(|i| i.to_string())
            .collect::<Vec<_>>(),
        ["co1", "co3"]
    );
    let error = fail(&mut p, &rm("co2"));
    assert!(error.contains("не найден"), "{error}");
}

#[test]
fn style_checks_ranges_and_reports_changes() {
    let mut p = project(&[10.0]);
    let summary = run(
        &mut p,
        &style(
            json!({"size": 1.5, "gradient": true, "colors": "#ff0000ff, #00ff00ff,#0000ffff", "glow": 14}),
        ),
    );
    assert_eq!(
        summary,
        "облачко: размер 1.00→1.50, градиент вкл, цвета #ff0000ff,#00ff00ff,#0000ffff, свечение 0→14"
    );
    let settings = state(&p).settings;
    assert_eq!(settings.colors.colors().len(), 3);
    run(
        &mut p,
        &style(json!({"text": "Привет", "background": "#000000ff"})),
    );
    assert_eq!(state(&p).settings.text, "Привет");
    run(&mut p, &style(json!({"text": ""})));
    assert_eq!(state(&p).settings.text, "");
    for (args, message) in [
        (json!({"size": 2.5}), "от 0.5 до 2"),
        (json!({"glow": -1}), "от 0 до 40"),
        (json!({"colors": "#ff0000ff"}), "от 2 до 4"),
        (json!({"colors": "red,blue"}), "не цвет"),
        (json!({"text": "a\tb"}), "одна строка"),
    ] {
        let error = fail(&mut p, &style(args.clone()));
        assert!(error.contains(message), "{args}: {error}");
    }
    assert_eq!(run(&mut p, &style(json!({"size": 1.5}))), "");
}

#[test]
fn undo_redo_returns_the_same_state() {
    let mut p = project(&[10.0]);
    let mut history = History::new(100);
    history
        .apply(&mut p, &add(json!({"action": "on", "at": 1})))
        .unwrap();
    let after_add = p.clone();
    history
        .apply(&mut p, &set(json!({"id": "co1", "at": 2})))
        .unwrap();
    history.apply(&mut p, &style(json!({"glow": 10}))).unwrap();
    history.apply(&mut p, &rm("co1")).unwrap();
    let last = p.clone();
    for _ in 0..3 {
        history.undo(&mut p);
    }
    assert_eq!(p, after_add);
    for _ in 0..3 {
        history.redo(&mut p);
    }
    assert_eq!(p, last);
}

#[test]
fn state_round_trips_and_lists_events() {
    let mut p = project(&[10.0]);
    run(
        &mut p,
        &add(json!({"action": "text", "at": 1, "text": "Кириллица 🎬"})),
    );
    run(&mut p, &style(json!({"gradient": true})));
    let json = serde_json::to_string(&p).unwrap();
    let loaded: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(state(&loaded), state(&p));
    let stored = &p.effects["callout"];
    assert_eq!(stored["events"][0]["action"], "text");
    assert!(
        stored.get(dc_core::effect::SETTINGS_FIELD).is_some(),
        "{stored}"
    );
    let items = (crate::effect().items)(&p).unwrap();
    assert_eq!((items[0].id.as_str(), items[0].list), ("co1", "callouts"));
    let map = dc_core::TimeMap::build(&p);
    let range = items[0].place.output(&map).unwrap();
    assert_eq!(
        (range.start().as_secs_f64(), range.end().as_secs_f64()),
        (1.0, 2.0)
    );
    assert_eq!(items[0].fields["text"], "Кириллица 🎬");
}

#[test]
fn ops_follow_the_rules() {
    let ops = crate::ops();
    let names: Vec<&str> = ops.iter().map(|op| op.name).collect();
    assert_eq!(
        names,
        ["callout add", "callout set", "callout rm", "callout style"]
    );
    for op in &ops {
        assert!(op.example.starts_with(op.name), "{}", op.example);
        let schema = serde_json::to_value((op.schema)()).unwrap();
        assert_eq!(schema["additionalProperties"], false, "{}", op.name);
    }
    assert_eq!(
        crate::effect().id_prefixes,
        [<EventKind as dc_core::IdKind>::PREFIX]
    );
}
