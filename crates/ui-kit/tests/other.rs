//! Colour, gradient, collapsible group, toast, question and error panels, level indicator.

use dc_ui_kit::tokens::{S3, SURFACE};
use dc_ui_kit::widgets::{
    Answer, Confirm, ErrorAction, ErrorPanel, Kind, LevelBar, Section, ToastKind, Toasts,
    color_button, gradient_editor, panel_frame,
};
use egui::Color32;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn harness<'a, State>(
    size: egui::Vec2,
    state: State,
    mut ui_fn: impl FnMut(&mut egui::Ui, &mut State) + 'a,
) -> Harness<'a, State> {
    dc_ui_kit::testing::harness()
        .with_size(size)
        .build_ui_state(
            move |ui, state| {
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(S3)
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        ui_fn(ui, state);
                    });
            },
            state,
        )
}

#[test]
fn color_picker_opens() {
    let mut harness = harness(
        egui::vec2(420.0, 380.0),
        Color32::from_rgb(0x3b, 0x82, 0xf6),
        |ui, color| {
            color_button(ui, color, "Цвет текста");
        },
    );
    harness.snapshot("color_button");
    harness.get_by_label("Цвет текста").click();
    harness.run();
    // egui's own HSV picker is drawn with meshes that WARP does not render the same every run.
    harness.get_by_label("#RRGGBB");
}

#[test]
fn gradient_keeps_two_to_four_colors() {
    let start = vec![
        Color32::from_rgb(0x25, 0x63, 0xeb),
        Color32::from_rgb(0xf4, 0x3f, 0x5e),
    ];
    let mut harness = harness(egui::vec2(300.0, 100.0), start, |ui, colors| {
        gradient_editor(ui, colors, 240.0);
    });
    harness.snapshot("gradient");
    harness.get_by_label("Убрать цвет").click();
    harness.run();
    assert_eq!(harness.state().len(), 2, "two colours is the minimum");
    for _ in 0..3 {
        harness.get_by_label("Добавить цвет").click();
        harness.run();
    }
    assert_eq!(harness.state().len(), 4, "four colours is the maximum");
}

#[test]
fn section_remembers_state() {
    let mut harness = harness(egui::vec2(300.0, 140.0), (), |ui, ()| {
        Section::new("Курсор").show(ui, |ui| ui.label("Размер курсора"));
        Section::new("Клики")
            .default_open(false)
            .show(ui, |ui| ui.label("Эффект клика"));
    });
    harness.snapshot("section");
    assert!(harness.query_by_label("Эффект клика").is_none());
    harness.get_by_label("Клики").click();
    harness.run();
    harness.get_by_label("Эффект клика");
    harness.get_by_label("Курсор").click();
    harness.run();
    assert!(harness.query_by_label("Размер курсора").is_none());
}

#[test]
fn toast_goes_away() {
    let mut harness = harness(egui::vec2(360.0, 140.0), Toasts::default(), |ui, toasts| {
        if toasts.is_empty() && ui.input(|i| i.time) < 0.5 {
            toasts.push(ui.ctx(), ToastKind::Done, "Скопировано");
            toasts.push(ui.ctx(), ToastKind::Error, "Не удалось сохранить проект");
        }
        toasts.show(ui.ctx());
    });
    harness.run_steps(2);
    harness.snapshot("toast");
    harness.get_by_label("Скопировано").click();
    harness.run_steps(2);
    assert!(harness.query_by_label("Скопировано").is_none());
}

#[test]
fn confirm_answers() {
    let mut harness = harness(
        egui::vec2(320.0, 150.0),
        None,
        |ui, answer: &mut Option<Answer>| {
            panel_frame().show(ui, |ui| {
                let confirm = Confirm {
                    title: "Отменить запись?",
                    text: "Запись на паузе. Записанные 12:47 будут удалены. Это нельзя вернуть.",
                    cancel: "Продолжить запись",
                    confirm: "Удалить запись",
                    kind: Kind::Danger,
                };
                if let Some(given) = confirm.show(ui) {
                    *answer = Some(given);
                }
            });
        },
    );
    harness.snapshot("confirm");
    harness.get_by_label("Удалить запись").click();
    harness.run();
    assert_eq!(*harness.state(), Some(Answer::Confirm));
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert_eq!(*harness.state(), Some(Answer::Cancel));
}

#[test]
fn error_offers_copy() {
    let mut harness = harness(
        egui::vec2(420.0, 150.0),
        None,
        |ui, action: &mut Option<ErrorAction>| {
            panel_frame().show(ui, |ui| {
            let panel = ErrorPanel {
                title: "Запись не подготовлена",
                text: "Не удалось собрать видео: диск D: заполнен. Освободи место и открой проект снова.",
                extra: &["Открыть папку записи"],
            };
            if let Some(given) = panel.show(ui) {
                *action = Some(given);
            }
        });
        },
    );
    harness.snapshot("error");
    harness.get_by_label("Копировать").click();
    harness.step();
    assert!(
        harness
            .output()
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t.contains("диск D:"))),
        "the error text goes to the clipboard"
    );
    harness.get_by_label("Открыть папку записи").click();
    harness.run();
    assert_eq!(*harness.state(), Some(ErrorAction::Extra(0)));
}

#[test]
fn level_bars() {
    let mut harness = harness(egui::vec2(120.0, 90.0), (), |ui, ()| {
        for level in [0.0, 0.4, 0.78, 1.0] {
            ui.add(LevelBar::new(level, 64.0, "Уровень"));
        }
        ui.add(LevelBar::new(0.6, 18.0, "Уровень").height(3.0));
    });
    harness.run();
    harness.snapshot("level_bars");
    assert_eq!(harness.get_all_by_label("Уровень").count(), 5);
}
