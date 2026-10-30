//! Input widgets: a snapshot of each and its behaviour through AccessKit (by caption).

use dc_ui_kit::Icon;
use dc_ui_kit::tokens::{S3, SURFACE};
use dc_ui_kit::widgets::{
    Button, IconButton, Kind, Slider, Toggle, dropdown, segmented, text_field,
};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A harness that fits the widget on the panel colour.
fn harness<'a, State>(
    state: State,
    mut ui_fn: impl FnMut(&mut egui::Ui, &mut State) + 'a,
) -> Harness<'a, State> {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(480.0, 240.0))
        .build_ui_state(
            move |ui, state| {
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(S3)
                    .show(ui, |ui| ui_fn(ui, state));
            },
            state,
        );
    harness.fit_contents();
    harness
}

#[test]
fn buttons() {
    let mut harness = harness(0, |ui, clicks: &mut u32| {
        ui.horizontal(|ui| {
            if ui.add(Button::new("Выбрать…")).clicked() {
                *clicks += 1;
            }
            ui.add(Button::new("Сохранить и выйти").kind(Kind::Primary));
            ui.add(Button::new("Удалить запись").kind(Kind::Danger).small());
            ui.add(Button::new("Сбросить все").kind(Kind::Ghost).small());
            ui.add(Button::new("Папка").icon(Icon::Folder));
        });
    });
    harness.get_by_label("Выбрать…").click();
    harness.run();
    assert_eq!(*harness.state(), 1);
    harness.snapshot("button");
}

#[test]
fn icon_buttons() {
    let mut harness = harness(0, |ui, clicks: &mut u32| {
        ui.horizontal(|ui| {
            if ui
                .add(IconButton::new(Icon::Mic, "Микрофон").on(true))
                .clicked()
            {
                *clicks += 1;
            }
            ui.add(IconButton::new(Icon::Speaker, "Системный звук"));
            ui.add(IconButton::text("60", "Частота кадров"));
            ui.add(
                IconButton::new(Icon::Timer, "Обратный отсчёт")
                    .on(true)
                    .badge("3"),
            );
            ui.add(IconButton::new(Icon::MicOff, "Без микрофона").dim(true));
            ui.add(IconButton::new(Icon::Close, "Отменить запись").color(dc_ui_kit::tokens::REC));
        });
    });
    harness.get_by_label("Микрофон").click();
    harness.run();
    assert_eq!(*harness.state(), 1);
    harness.snapshot("icon_button");
}

#[test]
fn toggle() {
    let mut harness = harness([true, false], |ui, on: &mut [bool; 2]| {
        ui.horizontal(|ui| {
            ui.add(Toggle::new(&mut on[0], "Системный звук"));
            ui.add(Toggle::new(&mut on[1], "Бета-версии"));
        });
    });
    harness.snapshot("toggle");
    harness.get_by_label("Бета-версии").click();
    harness.run();
    assert_eq!(*harness.state(), [true, true]);
}

#[test]
fn segmented_choice() {
    let mut harness = harness(60, |ui, fps: &mut u32| {
        segmented(
            ui,
            fps,
            &[(30, "30", true), (60, "60", true), (120, "120", true)],
        );
        segmented(
            ui,
            &mut 0,
            &[
                (0, "Свободно", true),
                (1, "16:9", true),
                (2, "1080p", false),
            ],
        );
    });
    harness.snapshot("segmented");
    harness.get_by_label("120").click();
    harness.run();
    assert_eq!(*harness.state(), 120);
}

#[test]
fn slider_with_field() {
    let mut harness = harness(80.0, |ui, volume: &mut f64| {
        Slider::new(volume, 0.0..=200.0, "Громкость")
            .suffix("%")
            .show(ui);
    });
    harness.snapshot("slider");
}

#[test]
fn dropdown_list() {
    let mut harness = harness(1, |ui, mic: &mut usize| {
        dropdown(
            ui,
            "Микрофон",
            mic,
            &[
                (0, "Без микрофона"),
                (1, "Микрофон (Rode NT-USB)"),
                (2, "Гарнитура (HyperX Cloud II)"),
            ],
            240.0,
        );
    });
    harness.snapshot("dropdown");
    harness.get_by_label("Микрофон").click();
    harness.run();
    harness.get_by_label("Гарнитура (HyperX Cloud II)");
    harness.set_size(egui::vec2(300.0, 220.0));
    harness.run();
    harness.snapshot("dropdown_open");
}

#[test]
fn text_field_with_hint() {
    let mut harness = harness(
        (String::new(), r"D:\Видео\DevCast".to_owned()),
        |ui, (search, path): &mut (String, String)| {
            text_field(ui, search, "Найти команду или сочетание", 280.0);
            text_field(ui, path, "Папка записей", 280.0);
        },
    );
    harness.snapshot("text_field");
    harness.get_by_label("Найти команду или сочетание").click();
    harness.run();
    harness
        .get_by_label("Найти команду или сочетание")
        .type_text("пауза");
    harness.run();
    assert_eq!(harness.state().0, "пауза");
}
