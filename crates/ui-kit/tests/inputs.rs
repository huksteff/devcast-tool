//! Input widgets: a snapshot of each and its behaviour through AccessKit (by caption).

use dc_ui_kit::Icon;
use dc_ui_kit::tokens::{S3, SURFACE};
use dc_ui_kit::widgets::{
    Button, Checkbox, IconButton, Kind, Slider, Toggle, anchors, dropdown, dropdown_some, palette,
    search_field, segmented, text_area, text_field, tiles,
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
        ui.horizontal(|ui| {
            ui.add(
                Button::new("Начать запись")
                    .kind(Kind::Danger)
                    .small()
                    .icon(Icon::Record),
            );
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
fn checkbox() {
    let mut harness = harness([true, false], |ui, on: &mut [bool; 2]| {
        ui.add(Checkbox::new(&mut on[0], "Убрать паузы"));
        ui.add(Checkbox::new(&mut on[1], "Показ клавиш"));
    });
    harness.snapshot("checkbox");
    harness.get_by_label("Показ клавиш").click();
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
fn anchor_grid() {
    let places = [
        "Вверху слева",
        "Вверху",
        "Вверху справа",
        "Внизу слева",
        "Внизу",
        "Внизу справа",
    ];
    let mut harness = harness(4usize, move |ui, place: &mut usize| {
        anchors(ui, place, 3, &places);
    });
    harness.snapshot("anchors");
    harness.get_by_label("Вверху справа").click();
    harness.run();
    assert_eq!(*harness.state(), 2);
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

const MICS: [(usize, &str); 3] = [
    (0, "Без микрофона"),
    (1, "Микрофон (Rode NT-USB)"),
    (2, "Гарнитура (HyperX Cloud II)"),
];

#[test]
fn dropdown_list() {
    let mut harness = harness(1, |ui, mic: &mut usize| {
        dropdown(ui, "Микрофон", mic, &MICS, 240.0);
    });
    harness.snapshot("dropdown");
}

#[test]
fn dropdown_open_shows_every_item() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(300.0, 200.0))
        .build_ui_state(
            |ui, mic: &mut usize| {
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(S3)
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        dropdown(ui, "Микрофон", mic, &MICS, 240.0);
                    });
            },
            1,
        );
    harness.get_by_label("Микрофон").click();
    harness.run();
    for (_, text) in MICS {
        harness.get_by_label(text);
    }
    harness.snapshot("dropdown_open");
}

/// A chosen caption longer than the button is cut with «…»: the button keeps its width and does
/// not push apart the fixed-width panel it stands in (the «Нейронка» tab with «Sonnet — …»).
#[test]
fn dropdown_keeps_its_width_with_a_long_choice() {
    const MODELS: [(usize, &str); 2] = [
        (0, "По умолчанию"),
        (1, "Sonnet — быстрее и экономнее, чем самая длинная подпись"),
    ];
    const WIDTH: f32 = 160.0;
    let mut harness = harness((1, 0.0), |ui, (model, width): &mut (usize, f32)| {
        *width = dropdown(ui, "Модель", model, &MODELS, WIDTH).rect.width();
    });
    harness.run();
    let width = harness.state().1;
    assert!(
        width <= WIDTH + 0.5,
        "кнопка списка шириной {width} px вместо {WIDTH}"
    );
    harness.snapshot("dropdown_long_choice");
}

#[test]
fn text_field_with_hint() {
    let mut harness = harness(
        (String::new(), r"D:\Видео\DevCast".to_owned()),
        |ui, (search, path): &mut (String, String)| {
            search_field(ui, search, "Найти команду или сочетание", 280.0);
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

#[test]
fn tiles_pick_one() {
    let options = [(0u8, "Выкл"), (1, "Ripple"), (2, "Spotlight"), (3, "Echo")];
    let mut harness = harness(1u8, move |ui, value: &mut u8| {
        tiles(ui, value, &options, |painter, rect, v| {
            let stroke = egui::Stroke::new(2.0, egui::Color32::WHITE);
            painter.circle_stroke(rect.center(), rect.width() / 2.0 - f32::from(v), stroke);
        });
    });
    harness.snapshot("tiles");
    harness.get_by_label("Echo").click();
    harness.run();
    assert_eq!(*harness.state(), 3);
}

#[test]
fn palette_with_own_colour() {
    let swatches = [
        (egui::Color32::from_rgb(0x25, 0x63, 0xeb), "Синий"),
        (egui::Color32::from_rgb(0xef, 0x44, 0x44), "Красный"),
        (egui::Color32::from_rgb(0x22, 0xc5, 0x5e), "Зелёный"),
    ];
    let mut harness = harness(swatches[0].0, move |ui, color: &mut egui::Color32| {
        palette(ui, color, &swatches, "Свой цвет");
    });
    harness.snapshot("palette");
    harness.get_by_label("Красный").click();
    harness.run();
    assert_eq!(*harness.state(), swatches[1].0);
    *harness.state_mut() = egui::Color32::from_rgb(0x14, 0xb8, 0xc4);
    harness.run();
    harness.snapshot("palette_own");
}

#[test]
fn text_area_takes_lines() {
    let mut harness = harness(
        "Шаг 2:\nоткройте настройки".to_owned(),
        |ui, said: &mut String| {
            text_area(ui, said, "Текст аннотации", 280.0, 2);
        },
    );
    harness.snapshot("text_area");
    harness.get_by_label("Текст аннотации").focus();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.get_by_label("Текст аннотации").type_text("!");
    harness.run();
    assert!(harness.state().contains("\n!"), "{}", harness.state());
}

#[test]
fn grey_dropdown_items_are_not_picked() {
    let items = [(0u8, "Нет", true), (1, "Печать · только текст", false)];
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(300.0, 160.0))
        .build_ui_state(
            move |ui, value: &mut u8| {
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(S3)
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        dropdown_some(ui, "Вход", value, &items, 240.0);
                    });
            },
            0u8,
        );
    harness.get_by_label("Вход").click();
    harness.run();
    harness.snapshot("dropdown_some");
    harness.get_by_label("Печать · только текст").click();
    harness.run();
    assert_eq!(*harness.state(), 0);
}
