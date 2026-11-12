//! The common rows of an annotation panel: the key strip (mockup `.kfbar`), «Вход и выход» and
//! the box, time, row and layer of «Тонкая настройка».

use crate::panel::{Edit, Shown};
use crate::{Animation, Annotation, ROTATION_LIMIT, STAGE_DEFAULT, Stage, same_moment};
use dc_core::OpName;
use dc_core::OutputTime;
use dc_ui_kit::egui::{self, Align, Layout, Rect, Sense, Shape, Stroke, Ui, UiBuilder, pos2, vec2};
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::panel::{BUTTON, CHEVRON, Cx, Group, Request, bottom_line, padded, rows};
use dc_ui_kit::strings::timeline::timecode;
use dc_ui_kit::strings::{annotations as text, timeline as rows_text};
use dc_ui_kit::tokens::{FS_M, KEYFRAME, S2, S3, SURFACE_2, TEXT, TEXT_3};
use dc_ui_kit::widgets::{Button, IconButton, Kind as ButtonKind, dropdown, dropdown_some};
use serde_json::json;

const ANNOT_ANIM: OpName = OpName::of::<crate::AnnotAnim>();
const ANNOT_LAYER: OpName = OpName::of::<crate::AnnotLayer>();
const ANNOT_MOVE: OpName = OpName::of::<crate::AnnotMove>();
const ANNOT_SET: OpName = OpName::of::<crate::AnnotSet>();

/// Height of the key strip, points (mockup `.kfbar`).
const STRIP: f32 = 36.0;
/// Seconds of an entrance or an exit on its slider.
const STAGE_MAX: f64 = 10.0;

/// Every animation with its caption, and the caption of one the kind cannot show.
const ANIMATIONS: [(Animation, &str, &str); 9] = [
    (Animation::None, text::ANIM_NONE, text::ANIM_NONE),
    (Animation::Fade, text::ANIM_FADE, text::ANIM_FADE),
    (Animation::Pop, text::ANIM_POP, text::ANIM_POP),
    (
        Animation::SlideLeft,
        text::ANIM_SLIDE_LEFT,
        text::ANIM_SLIDE_LEFT,
    ),
    (
        Animation::SlideRight,
        text::ANIM_SLIDE_RIGHT,
        text::ANIM_SLIDE_RIGHT,
    ),
    (
        Animation::SlideTop,
        text::ANIM_SLIDE_TOP,
        text::ANIM_SLIDE_TOP,
    ),
    (
        Animation::SlideBottom,
        text::ANIM_SLIDE_BOTTOM,
        text::ANIM_SLIDE_BOTTOM,
    ),
    (Animation::Draw, text::ANIM_DRAW, text::ANIM_DRAW_ONLY),
    (Animation::Typewriter, text::ANIM_TYPE, text::ANIM_TYPE_ONLY),
];

/// «Ключ 2 из 3 · 0:33.40» or «Ключа нет», and ‹ › to the neighbour keys (`Request::Seek`);
/// `on` is the key at the playhead, `at` where edits go.
pub fn keys_strip(
    ui: &mut Ui,
    cx: &mut Cx<'_>,
    keys: &[OutputTime],
    on: Option<OutputTime>,
    at: Option<OutputTime>,
) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), STRIP), Sense::hover());
    let said = match on {
        Some(key) => {
            let number = keys.iter().position(|k| *k == key).map_or(0, |i| i + 1);
            text::key_of(number, keys.len(), &timecode(key))
        }
        None if keys.is_empty() => text::NO_KEYS.to_owned(),
        None => text::no_key(keys.len()),
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &said));
    let painter = ui.painter();
    painter.rect_filled(rect, 0, SURFACE_2);
    let centre = pos2(rect.left() + S3 + 6.0, rect.center().y);
    let diamond = Rect::from_center_size(centre, vec2(12.0, 12.0));
    if on.is_some() {
        let points = [
            diamond.center_top(),
            diamond.right_center(),
            diamond.center_bottom(),
            diamond.left_center(),
        ];
        painter.add(Shape::convex_polygon(
            points.to_vec(),
            KEYFRAME,
            Stroke::NONE,
        ));
    } else {
        Icon::Diamond.paint(painter, diamond, TEXT_3);
    }
    let galley = painter.layout_no_wrap(said, fonts::text(FS_M, Weight::Regular), TEXT);
    let place = pos2(
        diamond.right() + S2,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(place, galley, TEXT);
    let previous = keys
        .iter()
        .rev()
        .copied()
        .find(|k| *k < cx.playhead && !same_moment(*k, cx.playhead));
    let next = keys
        .iter()
        .copied()
        .find(|k| *k > cx.playhead && !same_moment(*k, cx.playhead));
    let mut cell = ui.new_child(
        UiBuilder::new()
            .max_rect(rect.shrink2(vec2(S2, 0.0)))
            .layout(Layout::right_to_left(Align::Center)),
    );
    cell.spacing_mut().item_spacing.x = 2.0;
    for (to, icon, label) in [
        (next, Icon::Right, text::KEY_NEXT),
        (previous, Icon::Left, text::KEY_PREVIOUS),
    ] {
        let button = IconButton::new(icon, label).size(BUTTON, CHEVRON);
        if cell
            .add_enabled(to.is_some(), button)
            .on_hover_text(label)
            .clicked()
            && let Some(to) = to
        {
            cx.requests.push(Request::Seek(to));
        }
    }
    bottom_line(ui, rect);
    let note = match (on, at) {
        (Some(_), _) => Some(text::KEY_ON_NOTE),
        (None, Some(_)) => Some(text::KEY_AUTO_NOTE),
        (None, None) => (!keys.is_empty()).then_some(text::KEY_OFF_NOTE),
    };
    if let Some(note) = note {
        padded(ui, |ui| rows::note(ui, note));
    }
}

/// «Вход и выход»: the animation of each stage and its length (`annot anim`); «Сбросить» brings
/// both to none and 0.3 s.
pub fn motion_group(ui: &mut Ui, cx: &mut Cx<'_>, annotation: &Annotation, edit: Edit) {
    let fields = |enter: &Stage, exit: &Stage| {
        [
            ("enter", json!(enter.animation)),
            ("enter_duration", json!(enter.duration.as_secs_f64())),
            ("exit", json!(exit.animation)),
            ("exit_duration", json!(exit.duration.as_secs_f64())),
        ]
    };
    let still = Stage {
        animation: Animation::None,
        duration: STAGE_DEFAULT,
    };
    let now = fields(&annotation.enter, &annotation.exit);
    let group = Group {
        id: "motion",
        title: text::MOTION,
        open: true,
        reset: Some(
            edit.reset(ANNOT_ANIM, &now, &fields(&still, &still))
                .into_iter()
                .collect(),
        ),
    };
    let kind = annotation.kind;
    let items: Vec<(Animation, &str, bool)> = ANIMATIONS
        .iter()
        .map(|(a, name, only)| {
            let fits = a.fits(kind);
            (*a, if fits { *name } else { *only }, fits)
        })
        .collect();
    cx.group(ui, group, |ui, cx| {
        let stages = [
            (&annotation.enter, text::ENTER, text::ENTER_TIME, "enter"),
            (&annotation.exit, text::EXIT, text::EXIT_TIME, "exit"),
        ];
        for (stage, label, time, field) in stages {
            let picked = rows::row(ui, label, |ui| {
                let mut chosen = stage.animation;
                let width = ui.available_width();
                dropdown_some(ui, label, &mut chosen, &items, width)
                    .changed()
                    .then_some(chosen)
            });
            if let Some(animation) = picked {
                edit.apply(cx, ANNOT_ANIM, [(field, json!(animation))]);
            }
            if stage.animation == Animation::None {
                continue;
            }
            let seconds = stage.duration.as_secs_f64();
            let step = (0.05, dc_ui_kit::strings::inspector::SECONDS);
            if let Some(v) = rows::slider(ui, time, seconds, 0.0..=STAGE_MAX, step) {
                let name = if field == "enter" {
                    "enter_duration"
                } else {
                    "exit_duration"
                };
                edit.apply(cx, ANNOT_ANIM, [(name, json!(v))]);
            }
        }
    });
}

/// The box in px of the frame, the rotation, the time on the output, the row and the
/// layer.
pub fn fine_rows(ui: &mut Ui, cx: &mut Cx<'_>, shown: &Shown, edit: Edit) {
    place(ui, cx, shown, edit);
    let turn = (1.0, text::DEGREES);
    let limit = -ROTATION_LIMIT..=ROTATION_LIMIT;
    if let Some(v) = rows::slider(ui, text::ROTATION, shown.rotation, limit, turn) {
        edit.apply(cx, ANNOT_SET, [("rotation", json!(v))]);
    }
    times(ui, cx, shown, edit.unkeyed());
    arrange(ui, cx, shown, edit.unkeyed());
}

/// The box in px of the output frame (`annot set` takes fractions of it).
fn place(ui: &mut Ui, cx: &mut Cx<'_>, shown: &Shown, edit: Edit) {
    let Some(canvas) = shown.canvas else {
        return;
    };
    let (w, h) = (f64::from(canvas.width), f64::from(canvas.height));
    let r = shown.rect;
    let places = [
        ("x", text::X, r.x(), w, -w..=2.0 * w),
        ("y", text::Y, r.y(), h, -h..=2.0 * h),
        ("w", text::WIDTH, r.w(), w, 1.0..=2.0 * w),
        ("h", text::HEIGHT, r.h(), h, 1.0..=2.0 * h),
    ];
    for (field, label, value, side, range) in places {
        if let Some(v) = rows::slider(ui, label, (value * side).round(), range, (1.0, "")) {
            edit.apply(cx, ANNOT_SET, [(field, json!(v / side))]);
        }
    }
    rows::note(ui, text::PLACE_NOTE);
}

/// Start and end on the output (`annot set --from/--to`), typed like the times of a cue.
fn times(ui: &mut Ui, cx: &mut Cx<'_>, shown: &Shown, edit: Edit) {
    rows::row(ui, text::TIME, |ui| {
        let half = (ui.available_width() - S2) / 2.0;
        let ends = [
            ("from", text::START, shown.range.start()),
            ("to", text::END, shown.range.end()),
        ];
        for (field, label, now) in ends {
            let apply = |typed: String| edit.step(ANNOT_SET, [(field, json!(typed))]);
            let typed = ui
                .allocate_ui(vec2(half, ui.spacing().interact_size.y), |ui| {
                    rows::text(
                        ui,
                        label,
                        ("annot-time", edit.id, field),
                        &now.to_string(),
                        apply,
                    )
                })
                .inner;
            if let Some(step) = typed {
                cx.edits.push(vec![step]);
            }
        }
    });
}

/// The row of the annotation (`annot move --track`) and its layer in the row (`annot layer`).
fn arrange(ui: &mut Ui, cx: &mut Cx<'_>, shown: &Shown, edit: Edit) {
    let names: Vec<(u32, String)> = (0..=shown.tracks)
        .map(|t| (t, rows_text::annotations(t + 1)))
        .collect();
    let items: Vec<(u32, &str)> = names.iter().map(|(t, n)| (*t, n.as_str())).collect();
    let moved = rows::row(ui, text::TRACK, |ui| {
        let mut track = shown.annotation.track;
        let width = ui.available_width();
        dropdown(ui, text::TRACK, &mut track, &items, width)
            .changed()
            .then_some(track)
    });
    if let Some(track) = moved {
        edit.apply(cx, ANNOT_MOVE, [("track", json!(track))]);
    }
    let steps = [
        ("top", text::LAYER_TOP),
        ("up", text::LAYER_UP),
        ("down", text::LAYER_DOWN),
        ("bottom", text::LAYER_BOTTOM),
    ];
    rows::caption(ui, text::LAYER);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (step, label) in steps {
            let button = Button::new(label).kind(ButtonKind::Ghost).small();
            if ui.add(button).clicked() {
                edit.apply(cx, ANNOT_LAYER, [("step", json!(step))]);
            }
        }
    });
}
