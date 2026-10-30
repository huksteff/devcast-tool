use dc_core::project::{Media, MediaKind, SpeedRange};
use dc_core::{
    Clip, Command, History, OutputTime, PixelSize, Point, Project, Range, Rect, SourceTime,
    TimeDelta,
};

use crate::classify::strength;
use crate::{Action, Mode, Pointer, Recording, ZoomEffect, suggest};

fn s(secs: f64) -> SourceTime {
    SourceTime::from_secs_f64(secs)
}

fn project(lengths: &[f64]) -> Project {
    let mut p = Project::default();
    for &length in lengths {
        let id = p.clip_ids.allocate();
        let duration = TimeDelta::from_secs_f64(length);
        p.clips.push(Clip {
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
    }
    p
}

fn press(secs: f64, x: f64, y: f64) -> Pointer {
    Pointer {
        at: s(secs),
        point: Point { x, y },
        action: Action::Press { double: false },
    }
}

fn pointer(secs: f64, x: f64, y: f64, action: Action) -> Pointer {
    Pointer {
        action,
        ..press(secs, x, y)
    }
}

fn recording(p: &Project, clip: usize, pointer: Vec<Pointer>) -> Recording {
    Recording {
        media: p.clips[clip].media.path.clone(),
        pointer,
    }
}

/// Suggested zooms as (start, end, focus x, focus y, clicks), times in output seconds.
fn zooms(p: &Project, recordings: &[Recording]) -> Vec<(f64, f64, f64, f64, usize)> {
    let secs = |t: OutputTime| (t.as_secs_f64() * 1000.0).round() / 1000.0;
    suggest(p, recordings)
        .unwrap()
        .zooms
        .iter()
        .map(|z| {
            let (a, b) = (secs(z.span.start()), secs(z.span.end()));
            (a, b, z.focus.x, z.focus.y, z.clicks)
        })
        .collect()
}

#[test]
fn close_clicks_merge_and_far_ones_do_not() {
    let p = project(&[30.0]);
    let clicks = [2.0, 3.0, 4.5, 10.0, 20.0, 22.6].map(|t| press(t, 0.5, 0.5));
    let found = zooms(&p, &[recording(&p, 0, clicks.to_vec())]);
    assert_eq!(
        found,
        [
            (1.5, 5.0, 0.5, 0.5, 3),
            (9.5, 10.5, 0.5, 0.5, 1),
            (19.5, 20.5, 0.5, 0.5, 1),
            (22.1, 23.1, 0.5, 0.5, 1),
        ],
        "2.5 s apart is one cluster, 2.6 s is not; 0.5 s of padding on both sides"
    );
}

#[test]
fn the_strongest_click_gives_the_focus() {
    let p = project(&[30.0]);
    let mut double = press(3.0, 0.7, 0.2);
    double.action = Action::Press { double: true };
    let pointer = vec![press(2.0, 0.1, 0.1), press(2.5, 0.3, 0.3), double];
    assert_eq!(
        zooms(&p, &[recording(&p, 0, pointer)]),
        [(1.5, 3.5, 0.7, 0.2, 3)]
    );
    let pointer = vec![press(2.0, 0.1, 0.1), press(2.5, 0.3, 0.3)];
    assert_eq!(
        zooms(&p, &[recording(&p, 0, pointer)]),
        [(1.5, 3.0, 0.1, 0.1, 2)],
        "equal strength: the earlier one"
    );
}

#[test]
fn zooms_stay_inside_the_video() {
    let p = project(&[10.0]);
    let pointer = vec![press(0.2, 0.5, 0.5), press(9.9, 0.5, 0.5)];
    assert_eq!(
        zooms(&p, &[recording(&p, 0, pointer)]),
        [(0.0, 0.7, 0.5, 0.5, 1), (9.4, 10.0, 0.5, 0.5, 1)]
    );
}

#[test]
fn cut_out_clicks_are_hidden_and_cuts_bring_clicks_together() {
    let mut p = project(&[30.0]);
    p.clips[0].cuts = vec![Range::new(s(6.0), s(8.0)).unwrap()];
    let pointer = vec![
        press(5.0, 0.5, 0.5),
        press(7.0, 0.5, 0.5),
        press(9.0, 0.5, 0.5),
    ];
    let recordings = [recording(&p, 0, pointer)];
    let found = suggest(&p, &recordings).unwrap();
    assert_eq!((found.clicks, found.hidden), (2, 1));
    assert_eq!(zooms(&p, &recordings), [(4.5, 7.5, 0.5, 0.5, 2)]);
    let mut history = History::default();
    let commands: Vec<_> = found
        .zooms
        .iter()
        .map(|z| z.command(1.5, Mode::Follow))
        .collect();
    let commands: Vec<&dyn Command> = commands.iter().map(|c| c as &dyn Command).collect();
    history.apply_batch(&mut p, "zoom add", &commands).unwrap();
    let zoom = &p.state::<ZoomEffect>().unwrap().zooms[0];
    assert_eq!(
        zoom.range,
        Range::new(s(4.5), s(9.5)).unwrap(),
        "the zoom spans the cut in source time"
    );
    assert!(
        suggest(&p, &recordings).unwrap().zooms.is_empty(),
        "already zoomed"
    );
    assert_eq!(suggest(&p, &recordings).unwrap().taken, 1);
}

#[test]
fn speed_changes_the_gaps_on_the_output() {
    let mut p = project(&[30.0]);
    p.clips[0].speeds = vec![SpeedRange {
        range: Range::new(s(0.0), s(10.0)).unwrap(),
        factor: 2.0,
    }];
    let pointer = vec![press(4.0, 0.5, 0.5), press(8.0, 0.5, 0.5)];
    assert_eq!(
        zooms(&p, &[recording(&p, 0, pointer)]),
        [(1.5, 4.5, 0.5, 0.5, 2)],
        "4 s apart in the source, 2 s on the output"
    );
}

#[test]
fn a_zoom_never_crosses_into_another_clip() {
    let p = project(&[10.0, 10.0]);
    let first = recording(&p, 0, vec![press(9.8, 0.5, 0.5)]);
    let second = recording(&p, 1, vec![press(0.3, 0.2, 0.8)]);
    assert_eq!(
        zooms(&p, &[first, second]),
        [(9.3, 10.0, 0.5, 0.5, 1), (10.0, 10.8, 0.2, 0.8, 1)]
    );
}

#[test]
fn focus_is_in_the_cropped_picture_and_clicks_outside_it_are_hidden() {
    let mut p = project(&[10.0]);
    p.clips[0].crop = Rect::new(0.5, 0.0, 0.5, 1.0).unwrap();
    let pointer = vec![press(2.0, 0.75, 0.5), press(6.0, 0.25, 0.5)];
    let recordings = [recording(&p, 0, pointer)];
    assert_eq!(zooms(&p, &recordings), [(1.5, 2.5, 0.5, 0.5, 1)]);
    assert_eq!(suggest(&p, &recordings).unwrap().hidden, 1);
}

#[test]
fn no_clicks_no_zooms_and_no_clips_is_an_error() {
    let p = project(&[10.0]);
    let moves = vec![pointer(1.0, 0.5, 0.5, Action::Move)];
    let found = suggest(&p, &[recording(&p, 0, moves)]).unwrap();
    assert_eq!((found.zooms.len(), found.clicks), (0, 0));
    assert!(suggest(&Project::default(), &[]).is_err());
}

#[test]
fn what_follows_a_press_sets_its_strength() {
    let at = |pointer: &[Pointer]| strength(pointer, 0);
    let mut double = press(0.0, 0.5, 0.5);
    double.action = Action::Press { double: true };
    assert_eq!(at(&[double]), 1500);
    let drag = [
        press(0.0, 0.2, 0.5),
        pointer(0.5, 0.4, 0.51, Action::Release),
    ];
    assert_eq!(at(&drag), 1300, "drag to the right selects text");
    assert_eq!(at(&[press(0.0, 0.5, 0.5)]), 1100, "no moves: a text field");
    let still: Vec<Pointer> = std::iter::once(press(0.0, 0.5, 0.5))
        .chain((1..=5).map(|i| pointer(0.2 * f64::from(i), 0.505, 0.5, Action::Move)))
        .collect();
    assert_eq!(at(&still), 1100, "the pointer stayed");
    let down: Vec<Pointer> = std::iter::once(press(0.0, 0.5, 0.2))
        .chain((1..=5).map(|i| {
            pointer(
                0.2 * f64::from(i),
                0.5,
                0.2 + 0.02 * f64::from(i),
                Action::Move,
            )
        }))
        .collect();
    assert_eq!(at(&down), 1200, "down the list: a dropdown");
    let right: Vec<Pointer> = std::iter::once(press(0.0, 0.2, 0.5))
        .chain((1..=5).map(|i| {
            pointer(
                0.2 * f64::from(i),
                0.2 + 0.02 * f64::from(i),
                0.5,
                Action::Move,
            )
        }))
        .collect();
    assert_eq!(at(&right), 1300, "along the line: a selection");
    let around: Vec<Pointer> = std::iter::once(press(0.0, 0.5, 0.5))
        .chain((1..=5).map(|i| {
            let d = 0.02 * f64::from(i);
            pointer(0.2 * f64::from(i), 0.5 + d, 0.5 - d, Action::Move)
        }))
        .collect();
    assert_eq!(at(&around), 900, "diagonal away: a plain click");
}
