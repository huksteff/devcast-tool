use dc_core::project::{Media, MediaKind};
use dc_core::{
    Clip, ClipId, Command, Diff, History, OpInfo, Project, Range, Rect, SourceTime, TimeDelta,
};
use serde_json::{Value, json};

use crate::{Aspect, Crop, CropClip, SetAspect, effect, ops};

fn project(clips: u32) -> Project {
    let mut project = Project::default();
    for _ in 0..clips {
        let id = project.clip_ids.allocate();
        let duration = TimeDelta::from_secs_f64(10.0);
        project.clips.push(Clip {
            id,
            media: Media {
                path: "media/screen.mp4".to_owned(),
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
        });
    }
    project
}

fn c(n: u32) -> ClipId {
    ClipId::from_number(n)
}

fn crop(clip: u32, x: f64, y: f64, w: f64, h: f64) -> CropClip {
    CropClip {
        clip: c(clip),
        rect: Rect::new(x, y, w, h).unwrap(),
    }
}

fn error(project: &mut Project, command: &dyn Command) -> String {
    command.apply(project).unwrap_err().to_string()
}

#[test]
fn crop_is_stored_per_clip() {
    let mut p = project(2);
    let diff = crop(2, 0.1, 0.0, 0.8, 1.0).apply(&mut p).unwrap();
    assert_eq!(diff.to_string(), "кроп c2 0.1,0,0.8,1");
    assert_eq!(p.clips[0].crop, Rect::FULL);
    assert_eq!(p.clips[1].crop, Rect::new(0.1, 0.0, 0.8, 1.0).unwrap());
    assert!(p.effects.is_empty());
    assert_eq!(
        serde_json::to_value(&p.clips[1]).unwrap()["crop"],
        json!({"x": 0.1, "y": 0.0, "w": 0.8, "h": 1.0})
    );
}

#[test]
fn full_frame_removes_the_crop() {
    let mut p = project(1);
    crop(1, 0.2, 0.2, 0.5, 0.5).apply(&mut p).unwrap();
    let diff = crop(1, 0.0, 0.0, 1.0, 1.0).apply(&mut p).unwrap();
    assert_eq!(diff.to_string(), "кроп c1 снят");
    assert_eq!(p.clips[0].crop, Rect::FULL);
    assert!(
        serde_json::to_value(&p.clips[0])
            .unwrap()
            .get("crop")
            .is_none()
    );
}

#[test]
fn unknown_or_deleted_clip_is_not_found() {
    let mut p = project(3);
    let message = error(&mut p, &crop(9, 0.0, 0.0, 0.5, 0.5));
    assert_eq!(
        message,
        "c9 не найден (есть c1..c3); список: devcast ls clips"
    );
    p.clips.retain(|clip| clip.id != c(2));
    let message = error(&mut p, &crop(2, 0.0, 0.0, 0.5, 0.5));
    assert_eq!(
        message,
        "c2 не найден (есть c1,c3); список: devcast ls clips"
    );
    let message = error(&mut project(0), &crop(1, 0.0, 0.0, 0.5, 0.5));
    assert!(message.contains("нет ни одного клипа"), "{message}");
}

#[test]
fn crop_outside_the_frame_is_rejected() {
    let mut p = project(1);
    for (x, y, w, h) in [
        (-0.1, 0.0, 0.5, 0.5),
        (0.0, -0.01, 0.5, 0.5),
        (0.6, 0.0, 0.5, 0.5),
        (0.0, 0.6, 0.5, 0.5),
        (0.0, 0.0, 1.5, 1.0),
        (2.0, 2.0, 0.5, 0.5),
    ] {
        let message = error(&mut p, &crop(1, x, y, w, h));
        assert!(
            message.contains("внутри кадра"),
            "{x},{y},{w},{h}: {message}"
        );
    }
    assert_eq!(p.clips[0].crop, Rect::FULL);
}

#[test]
fn crop_smaller_than_minimum_is_rejected() {
    let mut p = project(1);
    for (w, h) in [(0.0, 0.5), (0.5, 0.0), (0.049, 0.5), (0.5, 0.049)] {
        let message = error(&mut p, &crop(1, 0.0, 0.0, w, h));
        assert!(message.contains("не меньше 0.05"), "{w}x{h}: {message}");
    }
}

#[test]
fn extreme_crops_inside_the_frame_are_accepted() {
    let mut p = project(1);
    crop(1, 0.95, 0.95, 0.05, 0.05).apply(&mut p).unwrap();
    crop(1, 0.15, 0.3, 0.85, 0.7).apply(&mut p).unwrap();
    crop(1, 0.0, 0.0, 1.0, 0.05).apply(&mut p).unwrap();
    assert_eq!(p.clips[0].crop, Rect::new(0.0, 0.0, 1.0, 0.05).unwrap());
    assert!(p.problems().is_empty());
}

#[test]
fn aspect_is_a_setting_and_keeps_crops() {
    let mut p = project(1);
    crop(1, 0.1, 0.1, 0.5, 0.5).apply(&mut p).unwrap();
    let diff = SetAspect {
        aspect: Aspect::Wide16x10,
    }
    .apply(&mut p)
    .unwrap();
    assert_eq!(diff.to_string(), "формат 16:10");
    let state = p.state::<Crop>().unwrap();
    assert_eq!(state.settings.aspect, Aspect::Wide16x10);
    assert_eq!(p.clips[0].crop, Rect::new(0.1, 0.1, 0.5, 0.5).unwrap());
    assert_eq!(
        p.effects["crop"][dc_core::effect::SETTINGS_FIELD],
        json!({"aspect": "16:10"})
    );
}

#[test]
fn default_aspect_leaves_no_state() {
    let mut p = project(1);
    SetAspect {
        aspect: Aspect::Standard4x3,
    }
    .apply(&mut p)
    .unwrap();
    let diff = SetAspect {
        aspect: Aspect::Source,
    }
    .apply(&mut p)
    .unwrap();
    assert_eq!(diff.to_string(), "формат source");
    assert!(p.effects.is_empty());
}

#[test]
fn undo_redo_returns_the_same_state() {
    let mut p = project(2);
    let mut history = History::new(100);
    history.apply(&mut p, &crop(1, 0.1, 0.1, 0.8, 0.8)).unwrap();
    let before = p.clone();
    let commands: [&dyn Command; 2] = [
        &crop(2, 0.0, 0.25, 1.0, 0.5),
        &SetAspect {
            aspect: Aspect::Wide16x9,
        },
    ];
    let diff = history.apply_batch(&mut p, "apply", &commands).unwrap();
    assert_eq!(diff.to_string(), "кроп c2 0,0.25,1,0.5; формат 16:9");
    let after = p.clone();
    history.undo(&mut p).unwrap();
    assert_eq!(p, before);
    history.redo(&mut p).unwrap();
    assert_eq!(p, after);
}

#[test]
fn failed_step_leaves_project_untouched() {
    let mut p = project(1);
    let mut history = History::new(100);
    let commands: [&dyn Command; 2] = [&crop(1, 0.1, 0.1, 0.5, 0.5), &crop(5, 0.0, 0.0, 0.5, 0.5)];
    let message = history
        .apply_batch(&mut p, "apply", &commands)
        .unwrap_err()
        .to_string();
    assert!(
        message.starts_with("шаг 2 (crop): c5 не найден"),
        "{message}"
    );
    assert!(p.clips[0].crop == Rect::FULL && !history.can_undo());
}

fn op(name: &str) -> OpInfo {
    ops().into_iter().find(|op| op.name == name).unwrap()
}

fn parse(name: &str, args: Value) -> Result<Diff, String> {
    let command = (op(name).parse)(args)?;
    command.apply(&mut project(1)).map_err(|e| e.to_string())
}

#[test]
fn ops_parse_their_json_arguments() {
    let diff = parse("crop", json!({"clip": "c1", "rect": "0,0.5,1,0.5"})).unwrap();
    assert_eq!(diff.to_string(), "кроп c1 0,0.5,1,0.5");
    let fields = json!({"clip": "c1", "rect": {"x": 0, "y": 0.5, "w": 1, "h": 0.5}});
    assert_eq!(
        parse("crop", fields).unwrap().to_string(),
        "кроп c1 0,0.5,1,0.5"
    );
    let negative = parse("crop", json!({"clip": "c1", "rect": "0,0,-0.5,0.5"})).unwrap_err();
    assert!(negative.contains("не отрицательными"), "{negative}");
    let short = parse("crop", json!({"clip": "c1", "rect": "0,0,1"})).unwrap_err();
    assert!(short.contains("ожидается x,y,w,h"), "{short}");
    assert_eq!(
        parse("aspect", json!({"aspect": "4:3"}))
            .unwrap()
            .to_string(),
        "формат 4:3"
    );
    assert_eq!(
        parse("aspect", json!({"aspect": "free"}))
            .unwrap()
            .to_string(),
        "формат free"
    );
    let vertical = parse("aspect", json!({"aspect": "9:16"})).unwrap_err();
    assert!(
        vertical.contains("16:9") && vertical.contains("free"),
        "{vertical}"
    );
    let typo = parse("crop", json!({"clip": "c1", "rect": "0,0,1,1", "z": 1}));
    assert!(typo.unwrap_err().contains("unknown field"));
}

#[test]
fn ops_follow_the_registry_rules() {
    let list = ops();
    assert_eq!(
        list.iter().map(|op| op.name).collect::<Vec<_>>(),
        ["crop", "aspect"]
    );
    for op in list {
        let schema = serde_json::to_value((op.schema)()).unwrap();
        assert_eq!(schema["additionalProperties"], json!(false), "{}", op.name);
        assert!(op.example.starts_with(op.name) && !op.about.contains('\n'));
        for field in op.positional {
            assert!(
                schema["properties"].get(*field).is_some(),
                "{}: {field}",
                op.name
            );
        }
    }
}

#[test]
fn effect_info_describes_the_state() {
    let info = effect();
    assert_eq!((info.id, info.title), ("crop", "Кроп"));
    assert!(info.id_prefixes.is_empty());
    let schema = serde_json::to_string(&(info.schema)()).unwrap();
    assert!(
        schema.contains("16:10") && schema.contains("settings"),
        "{schema}"
    );
}

#[test]
fn copied_clip_keeps_its_crop() {
    let mut p = project(1);
    crop(1, 0.1, 0.1, 0.5, 0.5).apply(&mut p).unwrap();
    let mut half = p.clips[0].clone();
    half.id = p.clip_ids.allocate();
    p.clips.push(half);
    assert_eq!(p.clips[1].crop, Rect::new(0.1, 0.1, 0.5, 0.5).unwrap());
    assert!(p.problems().is_empty());
}
