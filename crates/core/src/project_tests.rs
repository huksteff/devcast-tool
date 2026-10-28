use super::*;
use crate::time::Source;

fn t(secs: f64) -> SourceTime {
    SourceTime::from_secs_f64(secs)
}

fn r(a: f64, b: f64) -> Range<Source> {
    Range::new(t(a), t(b)).unwrap()
}

pub fn clip(project: &mut Project, duration: f64) -> ClipId {
    let id = project.clip_ids.allocate();
    project.clips.push(Clip {
        id,
        media: Media {
            path: format!("media/{id}.mp4"),
            kind: MediaKind::Recording,
            duration: TimeDelta::from_secs_f64(duration),
            size: crate::geom::PixelSize {
                width: 1920,
                height: 1080,
            },
        },
        trim: r(0.0, duration),
        cuts: Vec::new(),
        speeds: Vec::new(),
    });
    id
}

#[test]
fn fresh_project_is_consistent() {
    let mut p = Project::default();
    clip(&mut p, 10.0);
    assert_eq!(p.problems(), Vec::<String>::new());
}

#[test]
fn empty_frame_size_is_a_problem() {
    let mut p = Project::default();
    let id = clip(&mut p, 10.0);
    p.clip_mut(id).unwrap().media.size.height = 0;
    assert_eq!(
        p.problems(),
        vec!["c1: пустой размер кадра 1920x0".to_owned()]
    );
}

#[test]
fn json_round_trip_keeps_everything() {
    let mut p = Project::default();
    let id = clip(&mut p, 10.0);
    let c = p.clip_mut(id).unwrap();
    c.cuts.push(r(2.0, 3.0));
    c.speeds.push(SpeedRange {
        range: r(5.0, 8.0),
        factor: 4.0,
    });
    p.effects
        .insert("zoom".to_owned(), serde_json::json!({"zooms": []}));
    let json = serde_json::to_string(&p).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back, p);
}

#[test]
fn reports_broken_invariants() {
    let mut p = Project::default();
    let id = clip(&mut p, 10.0);
    let c = p.clip_mut(id).unwrap();
    c.trim = r(0.0, 12.0);
    c.cuts = vec![r(5.0, 6.0), r(4.0, 5.5)];
    c.speeds = vec![SpeedRange {
        range: r(1.0, 2.0),
        factor: 32.0,
    }];
    let problems = p.problems().join("\n");
    for expected in ["выходит за длину исходника", "пересекается", "скорость 32"]
    {
        assert!(
            problems.contains(expected),
            "нет «{expected}» в:\n{problems}"
        );
    }
}

#[test]
fn duplicate_or_future_ids_are_reported() {
    let mut p = Project::default();
    let id = clip(&mut p, 1.0);
    let copy = p.clip(id).unwrap().clone();
    p.clips.push(copy);
    p.clips.push(Clip {
        id: ClipId::from_number(99),
        ..p.clips[0].clone()
    });
    let problems = p.problems().join("\n");
    assert!(problems.contains("c1: id повторяется"), "{problems}");
    assert!(
        problems.contains("c99: id не меньше счётчика"),
        "{problems}"
    );
}
