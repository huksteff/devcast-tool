use dc_core::project::{Media, MediaKind};
use dc_core::{
    Clip, Command, History, OutputTime, PixelSize, Project, Range, Rect, SourceTime, TimeDelta,
};
use dc_fx_annotations::{AnnotAdd, AnnotKeyAdd, Annotation, Annotations, keyed};
use serde_json::{Value, json};

use crate::{ImageBody, ImageSet, PictureFile};

/// One 10 s recording with image annotation a1 and text annotation a2, both output 1–2 s.
pub(crate) fn project() -> Project {
    let mut p = Project::default();
    let id = p.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(10.0);
    p.clips.push(Clip {
        id,
        media: Media {
            path: "media/запись 1.mp4".into(),
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
    for kind in ["image", "text"] {
        let add: AnnotAdd = serde_json::from_value(json!({"kind": kind, "at": 1})).unwrap();
        add.apply(&mut p).unwrap();
    }
    p
}

pub(crate) fn picture(name: &str, width: u32, height: u32) -> PictureFile {
    PictureFile::new(
        format!("assets/images/{name}"),
        PixelSize { width, height },
        &[TimeDelta::ZERO],
    )
}

fn set(id: &str, picture: PictureFile, at: Option<f64>) -> ImageSet {
    ImageSet {
        id: id.parse().unwrap(),
        picture,
        at: at.map(OutputTime::from_secs_f64),
    }
}

fn image(p: &Project) -> Annotation {
    p.state::<Annotations>().unwrap().annotations[0].clone()
}

fn body(p: &Project) -> ImageBody {
    image(p).body().unwrap()
}

#[test]
fn a_new_image_has_no_picture() {
    assert_eq!(body(&project()), ImageBody { picture: None });
}

#[test]
fn image_set_puts_the_picture_and_says_nothing_changed_the_second_time() {
    let mut p = project();
    let logo = picture("лого 1.png", 300, 100);
    let diff = set("a1", logo.clone(), None).apply(&mut p).unwrap();
    assert_eq!(diff.to_string(), "аннотации ~1 a1");
    assert_eq!(body(&p).picture, Some(logo.clone()));
    let again = set("a1", logo, None).apply(&mut p).unwrap();
    assert!(again.is_empty());
}

#[test]
fn image_set_refuses_other_kinds_and_missing_ids_without_changes() {
    let mut p = project();
    let before = p.clone();
    let error = set("a2", picture("a.png", 1, 1), None)
        .apply(&mut p)
        .unwrap_err()
        .to_string();
    assert!(error.contains("не картинка"), "{error}");
    let error = set("a9", picture("a.png", 1, 1), None)
        .apply(&mut p)
        .unwrap_err()
        .to_string();
    assert!(error.contains("a9"), "{error}");
    assert_eq!(p, before);
}

#[test]
fn a_picture_at_a_moment_goes_into_a_key_and_steps_there() {
    let mut p = project();
    set("a1", picture("a.png", 10, 10), None)
        .apply(&mut p)
        .unwrap();
    set("a1", picture("b.gif", 20, 10), Some(1.5))
        .apply(&mut p)
        .unwrap();
    let a = image(&p);
    let times: Vec<_> = a.keys.iter().map(|k| k.at.micros()).collect();
    assert_eq!(times, [0, 500_000], "a key at the start comes with it");
    let read = |b: &serde_json::Map<String, Value>| -> ImageBody {
        serde_json::from_value(Value::Object(b.clone())).unwrap()
    };
    let keys: Vec<_> = a.keys.iter().map(|k| (k.at, read(&k.body))).collect();
    let at = |ms| keyed(&read(&a.body), &keys, TimeDelta::from_millis(ms)).picture;
    assert_eq!(at(499), Some(picture("a.png", 10, 10)));
    assert_eq!(at(500), Some(picture("b.gif", 20, 10)));
    // A snapshot between the keys takes one picture whole, size and frames with its path.
    let add = AnnotKeyAdd {
        id: a.id,
        at: dc_core::TimeArg(OutputTime::from_secs_f64(1.25)),
    };
    add.apply(&mut p).unwrap();
    let middle = read(&image(&p).keys[1].body);
    assert_eq!(middle.picture, Some(picture("a.png", 10, 10)));
}

#[test]
fn image_set_without_a_moment_changes_every_key() {
    let mut p = project();
    set("a1", picture("a.png", 10, 10), Some(1.5))
        .apply(&mut p)
        .unwrap();
    set("a1", picture("c.webp", 5, 5), None)
        .apply(&mut p)
        .unwrap();
    let a = image(&p);
    for b in std::iter::once(&a.body).chain(a.keys.iter().map(|k| &k.body)) {
        let b: ImageBody = serde_json::from_value(Value::Object(b.clone())).unwrap();
        assert_eq!(b.picture, Some(picture("c.webp", 5, 5)));
    }
}

#[test]
fn undo_and_redo_and_the_project_file_keep_the_picture() {
    let mut p = project();
    let mut history = History::new(10);
    let gif = PictureFile::new(
        "assets/images/a.gif".into(),
        PixelSize {
            width: 4,
            height: 2,
        },
        &[TimeDelta::from_millis(100); 3],
    );
    let before = p.clone();
    history
        .apply(&mut p, &set("a1", gif.clone(), None))
        .unwrap();
    let after = p.clone();
    history.undo(&mut p).unwrap();
    assert_eq!(p, before);
    history.redo(&mut p).unwrap();
    assert_eq!(p, after);
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains(r#""frames":[[3,100]]"#), "{json}");
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(body(&back).picture, Some(gif));
}
