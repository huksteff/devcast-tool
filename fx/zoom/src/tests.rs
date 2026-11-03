use dc_core::project::{Media, MediaKind, SpeedRange};
use dc_core::{Clip, Command, History, Point, Project, Range, SourceTime, TimeDelta};

use crate::{
    Mode, Preset, ZoomAdd, ZoomEffect, ZoomMotion, ZoomRm, ZoomSet, ZoomSettings, ZoomState,
};

fn secs(s: f64) -> SourceTime {
    SourceTime::from_secs_f64(s)
}

fn clip(project: &mut Project, length: f64) -> &mut Clip {
    let id = project.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(length);
    project.clips.push(Clip {
        id,
        media: Media {
            path: format!("media/{id}.mp4"),
            kind: MediaKind::Recording,
            duration,
            size: dc_core::PixelSize {
                width: 1920,
                height: 1080,
            },
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: dc_core::Rect::FULL,
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

pub(crate) fn add(at: f64, duration: f64) -> ZoomAdd {
    serde_json::from_value(serde_json::json!({"at": at, "duration": duration})).unwrap()
}

fn set(args: serde_json::Value) -> ZoomSet {
    serde_json::from_value(args).unwrap()
}

fn motion(args: serde_json::Value) -> ZoomMotion {
    serde_json::from_value(args).unwrap()
}

fn rm(id: &str) -> ZoomRm {
    serde_json::from_value(serde_json::json!({ "id": id })).unwrap()
}

fn state(p: &Project) -> ZoomState {
    p.state::<ZoomEffect>().unwrap()
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
fn add_uses_defaults_and_source_time() {
    let mut p = project(&[10.0]);
    assert_eq!(run(&mut p, &add(2.0, 1.0)), "зумы +1 z1");
    let zoom = &state(&p).zooms[0];
    assert_eq!(zoom.id.to_string(), "z1");
    assert_eq!(zoom.media, p.clips[0].media.path);
    assert_eq!(zoom.range, Range::new(secs(2.0), secs(3.0)).unwrap());
    assert!((zoom.scale - 1.5).abs() < 1e-9);
    assert_eq!(zoom.mode, Mode::Follow);
    assert_eq!(zoom.focus, Point { x: 0.5, y: 0.5 });
}

#[test]
fn times_accept_clock_format() {
    let mut p = project(&[100.0]);
    let command: ZoomAdd = serde_json::from_value(serde_json::json!({"at": "1:02.5"})).unwrap();
    run(&mut p, &command);
    run(&mut p, &set(serde_json::json!({"id": "z1", "to": "1:05"})));
    assert_eq!(
        state(&p).zooms[0].range,
        Range::new(secs(62.5), secs(65.0)).unwrap()
    );
}

#[test]
fn add_duration_defaults_to_one_second() {
    let mut p = project(&[10.0]);
    let command: ZoomAdd = serde_json::from_value(serde_json::json!({"at": 4})).unwrap();
    run(&mut p, &command);
    assert_eq!(
        state(&p).zooms[0].range.duration(),
        TimeDelta::from_millis(1000)
    );
}

#[test]
fn add_maps_output_time_through_cuts_and_speed() {
    let mut p = Project::default();
    let c = clip(&mut p, 10.0);
    c.cuts.push(Range::new(secs(1.0), secs(3.0)).unwrap());
    c.speeds.push(SpeedRange {
        range: Range::new(secs(6.0), secs(10.0)).unwrap(),
        factor: 2.0,
    });
    run(&mut p, &add(2.0, 1.0));
    run(&mut p, &add(4.0, 1.0));
    let zooms = state(&p).zooms;
    assert_eq!(zooms[0].range, Range::new(secs(4.0), secs(5.0)).unwrap());
    assert_eq!(zooms[1].range, Range::new(secs(6.0), secs(8.0)).unwrap());
}

#[test]
fn add_at_start_and_up_to_end_of_video() {
    let mut p = project(&[10.0]);
    run(&mut p, &add(0.0, 1.0));
    run(&mut p, &add(9.0, 1.0));
    let zooms = state(&p).zooms;
    assert_eq!(zooms[0].range.start(), SourceTime::ZERO);
    assert_eq!(zooms[1].range.end(), secs(10.0));
}

#[test]
fn add_rejects_bad_spans() {
    let mut p = project(&[10.0]);
    assert!(fail(&mut p, &add(2.0, 0.0)).contains("больше нуля"));
    assert!(fail(&mut p, &add(-1.0, 1.0)).contains("раньше начала видео"));
    let error = fail(&mut p, &add(9.5, 1.0));
    assert!(error.contains("0:10.000"), "{error}");
    let mut empty = Project::default();
    assert!(fail(&mut empty, &add(0.0, 1.0)).contains("нет клипов"));
}

#[test]
fn add_rejects_crossing_a_clip_joint() {
    let mut p = project(&[5.0, 5.0]);
    let error = fail(&mut p, &add(4.5, 1.0));
    assert!(
        error.contains("стык") && error.contains("0:05.000"),
        "{error}"
    );
    run(&mut p, &add(5.0, 1.0));
    run(&mut p, &add(4.0, 1.0));
    let zooms = state(&p).zooms;
    assert_eq!(zooms[0].media, p.clips[1].media.path);
    assert_eq!(zooms[0].range.start(), SourceTime::ZERO);
    assert_eq!(zooms[1].media, p.clips[0].media.path);
    assert_eq!(zooms[1].range, Range::new(secs(4.0), secs(5.0)).unwrap());
}

#[test]
fn zooms_must_not_overlap_but_may_touch() {
    let mut p = project(&[10.0]);
    run(&mut p, &add(2.0, 2.0));
    let error = fail(&mut p, &add(3.5, 1.0));
    assert!(error.contains("z1"), "{error}");
    run(&mut p, &add(4.0, 1.0));
    run(&mut p, &add(1.0, 1.0));
    assert_eq!(state(&p).zooms.len(), 3);
}

#[test]
fn scale_limits() {
    let mut p = project(&[10.0]);
    for (at, scale, ok) in [
        (0.0, 1.0, true),
        (2.0, 5.0, true),
        (4.0, 0.99, false),
        (6.0, 5.01, false),
    ] {
        let command: ZoomAdd =
            serde_json::from_value(serde_json::json!({"at": at, "scale": scale})).unwrap();
        assert_eq!(command.apply(&mut p).is_ok(), ok, "scale {scale}");
    }
}

#[test]
fn set_changes_fields_and_reports_only_real_changes() {
    let mut p = project(&[10.0]);
    run(&mut p, &add(2.0, 1.0));
    let diff = run(
        &mut p,
        &set(serde_json::json!({"id": "z1", "scale": 2, "mode": "fixed", "x": 0.2})),
    );
    assert_eq!(diff, "зумы ~1 z1");
    let zoom = &state(&p).zooms[0];
    assert!((zoom.scale - 2.0).abs() < 1e-9);
    assert_eq!(zoom.mode, Mode::Fixed);
    assert_eq!(zoom.focus, Point { x: 0.2, y: 0.5 });
    assert_eq!(
        run(&mut p, &set(serde_json::json!({"id": "z1", "scale": 2}))),
        ""
    );
    assert_eq!(run(&mut p, &set(serde_json::json!({"id": "z1"}))), "");
}

#[test]
fn set_moves_one_edge_and_checks_it() {
    let mut p = project(&[10.0]);
    run(&mut p, &add(2.0, 1.0));
    run(&mut p, &add(5.0, 1.0));
    run(&mut p, &set(serde_json::json!({"id": "z1", "to": 4})));
    assert_eq!(
        state(&p).zooms[0].range,
        Range::new(secs(2.0), secs(4.0)).unwrap()
    );
    run(&mut p, &set(serde_json::json!({"id": "z1", "from": 1})));
    assert_eq!(
        state(&p).zooms[0].range,
        Range::new(secs(1.0), secs(4.0)).unwrap()
    );
    assert!(fail(&mut p, &set(serde_json::json!({"id": "z1", "from": 4}))).contains("больше нуля"));
    assert!(fail(&mut p, &set(serde_json::json!({"id": "z1", "to": 5.5}))).contains("z2"));
    assert!(fail(&mut p, &set(serde_json::json!({"id": "z1", "x": 1.5}))).contains("0..1"));
}

#[test]
fn zoom_spans_the_joint_of_clips_split_from_one_recording() {
    let mut p = project(&[10.0]);
    let mut right = p.clips[0].clone();
    right.id = p.clip_ids.allocate();
    p.clips[0].trim = Range::new(secs(0.0), secs(5.0)).unwrap();
    right.trim = Range::new(secs(5.0), secs(10.0)).unwrap();
    p.clips.push(right);
    assert_eq!(run(&mut p, &add(4.5, 1.0)), "зумы +1 z1");
    let zoom = &state(&p).zooms[0];
    assert_eq!(zoom.media, p.clips[0].media.path);
    assert_eq!(zoom.range, Range::new(secs(4.5), secs(5.5)).unwrap());
}

#[test]
fn set_edge_in_another_clip_is_rejected() {
    let mut p = project(&[5.0, 5.0]);
    run(&mut p, &add(3.0, 1.0));
    let error = fail(&mut p, &set(serde_json::json!({"id": "z1", "to": 6})));
    assert!(error.contains("стык"), "{error}");
}

#[test]
fn unknown_id_lists_existing_ones() {
    let mut p = project(&[10.0]);
    assert_eq!(
        fail(&mut p, &rm("z3")),
        "z3 не найден (нет ни одного зума); список: devcast ls zooms"
    );
    run(&mut p, &add(1.0, 1.0));
    assert_eq!(
        fail(&mut p, &set(serde_json::json!({"id": "z9", "scale": 2}))),
        "z9 не найден (есть z1); список: devcast ls zooms"
    );
}

#[test]
fn rm_removes_and_ids_are_never_reused() {
    let mut p = project(&[10.0]);
    run(&mut p, &add(1.0, 1.0));
    assert_eq!(run(&mut p, &rm("z1")), "зумы -1 z1");
    assert!(state(&p).zooms.is_empty());
    assert_eq!(run(&mut p, &add(1.0, 1.0)), "зумы +1 z2");
}

#[test]
fn motion_presets_and_fine_tuning() {
    let mut p = project(&[10.0]);
    assert_eq!(ZoomSettings::default().preset(), Some(Preset::Focused));
    let diff = run(&mut p, &motion(serde_json::json!({"preset": "smooth"})));
    assert_eq!(
        diff,
        "движение зума: вход 0.200→1.523 с, выход 0.200→1.015 с (Smooth)"
    );
    let settings = state(&p).settings;
    assert_eq!(settings.zoom_in, TimeDelta::from_micros(1_522_575));
    assert_eq!(settings.zoom_out, TimeDelta::from_micros(1_015_050));
    assert_eq!(settings.preset(), Some(Preset::Smooth));

    let diff = run(
        &mut p,
        &motion(serde_json::json!({"smoothness": 0.8, "connect": false})),
    );
    assert_eq!(
        diff,
        "движение зума: плавность 0.50→0.80, склейка вкл→выкл (Smooth)"
    );
    let diff = run(
        &mut p,
        &motion(serde_json::json!({"zoom_in": 0.5, "blur": 0})),
    );
    assert_eq!(
        diff,
        "движение зума: вход 1.523→0.500 с, motion blur 0.35→0.00"
    );
    assert_eq!(state(&p).settings.preset(), None);
    assert_eq!(
        run(&mut p, &motion(serde_json::json!({"zoom_in": 0.5}))),
        ""
    );
}

#[test]
fn motion_rejects_out_of_range_values() {
    let mut p = project(&[10.0]);
    for args in [
        serde_json::json!({"smoothness": 1.1}),
        serde_json::json!({"smoothness": -0.1}),
        serde_json::json!({"blur": 2.5}),
        serde_json::json!({"zoom_in": 0.05}),
        serde_json::json!({"zoom_out": 4.5}),
    ] {
        let error = fail(&mut p, &motion(args.clone()));
        assert!(error.contains("от "), "{args}: {error}");
    }
    run(
        &mut p,
        &motion(serde_json::json!({"smoothness": 0, "blur": 2, "zoom_in": 4, "zoom_out": 0.06})),
    );
}

#[test]
fn undo_redo_returns_the_same_state() {
    let mut p = project(&[10.0]);
    let mut history = History::new(100);
    history.apply(&mut p, &add(1.0, 1.0)).unwrap();
    let after_add = p.clone();
    history
        .apply(&mut p, &set(serde_json::json!({"id": "z1", "scale": 3})))
        .unwrap();
    history
        .apply(&mut p, &motion(serde_json::json!({"preset": "smooth"})))
        .unwrap();
    history.apply(&mut p, &rm("z1")).unwrap();
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
fn state_round_trips_and_settings_live_in_settings_field() {
    let mut p = project(&[10.0]);
    run(&mut p, &add(1.0, 1.0));
    run(&mut p, &motion(serde_json::json!({"preset": "smooth"})));
    let json = serde_json::to_string(&p).unwrap();
    let loaded: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(state(&loaded), state(&p));
    let stored = &p.effects["zoom"];
    assert!(
        stored.get(dc_core::effect::SETTINGS_FIELD).is_some(),
        "{stored}"
    );
}

#[test]
fn ops_follow_the_rules() {
    let ops = crate::ops();
    let names: Vec<&str> = ops.iter().map(|op| op.name).collect();
    assert_eq!(names, ["zoom add", "zoom set", "zoom rm", "zoom motion"]);
    for op in &ops {
        assert!(op.example.starts_with(op.name), "{}", op.example);
        let schema = serde_json::to_value((op.schema)()).unwrap();
        assert_eq!(schema["additionalProperties"], false, "{}", op.name);
    }
    assert_eq!(crate::effect().id_prefixes, ["z"]);
}
