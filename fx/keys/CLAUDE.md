# dc-fx-keys (`fx/keys`)

Эффект: показ нажатых клавиш записей (по умолчанию выключен); команда `keys`, вклад в сцену (`Prepared` + `evaluate`) и панель инспектора «Показ клавиш» (`panel`).

## Инварианты

- Зависит только от dc-core, dc-render и dc-ui-kit. Сочетания получает готовыми (`dc_core::KeyTracks`: имя `Ctrl+Shift+P` на source time записи) через `dc_ops::Evaluator::new`, файлов не читает (решение 015). Имена клавиш даёт `dc-capture` (`KeyEvent::name`).
- `Prepared::new` один раз ставит нажатия на выход через сегменты `TimeMap` (вырезанные выпадают, скорость сжимает), в порядке выхода. Склейка: то же сочетание, пока оно на экране, — счётчик `×N` растёт с каждым нажатием и продлевает показ; другое сочетание сразу заменяет (без затухания между ними). Показ — `duration` выходного времени после последнего нажатия, не дальше конца видео. `Prepared::shown` — те же плашки для дорожки «Клавиши» редактора (dc-app), своих мест редактор не считает.
- Плашка — один `Layer::Text` в `Band::Overlay` (камера зума её не двигает): Inter жирный, плашка `Plate` по буквам строки, без замера текста на CPU. Шрифт `36 · size · S/1080` px, `S` — короткая сторона кадра; отступ от краёв `0.04 · S`; плашка `pad` (0.5, 0.25) и радиус 0.3 шрифта. Позиция — 6 мест (верх/низ × лево/центр/право), выравнивание текста по ней. Появление 100 мс, исчезновение 250 мс.
- Состояние: `settings` (`show` false, `position` bottom, `size` 0.5–3, `color`, `background` `#rrggbbaa`, `duration` 0.3–5 с).
- Панель (`panel.rs`) — образец шва «Панель эффекта» (docs/ARCHITECTURE.md): `panel::ui(ui, &mut dc_ui_kit::panel::Cx, &Shortcuts)`, `panel::ID` = `keys`. Факты `Shortcuts::of(&Prepared, &KeyTracks)` (число разных сочетаний, самое частое — образец плашки) считает этот крейт, редактор хранит их при дорожке «Клавиши» (`Tracks::place_keys`). Каждая правка и «Сбросить» группы — `Step` команды `keys` в `cx.edits`; группы «Основное» (открыта) и «Тонкая настройка». Строки — `dc_ui_kit::strings::inspector`.

## Как тестировать

- `cargo nextest run -p dc-fx-keys`: `tests.rs` — команда, диапазоны, undo/redo, файл проекта; `evaluate_tests.rs` — insta-снимок 20 моментов через 2× и вырез до импортированного клипа (`src/snapshots/`), выключено, счётчик повторов, замена и затухание, конец видео, чужие записи, позиции, размер от короткой стороны. Данные — `src/test_util.rs`.
- `panel_tests.rs` — панель в kittest (`dc_ui_kit::testing::harness()`): факты, правка = `keys --show true`, «Сбросить». Снимки инспектора с панелью (`inspector_keys*.png`) и путь до `devcast keys` — в dc-app (`editor/inspector/tests.rs`, `shortcuts_tests.rs`).
- Эталоны кадров — `crates/ops/tests/golden/keys.rs` (настоящие сочетания `fixtures/recordings/typing.devcast`, шрифты `.deps/fonts`).

## Куда не лезть

- Чтение `keys.bin` и фильтр приватности — `dc-capture`. Порядок эффектов в кадре — `dc_ops::Evaluator`. Субтитры — `fx/captions`. Рельс вкладок, прокрутка и выполнение правок инспектора — dc-app; набор панели (заголовок, группы, строки) — `dc_ui_kit::panel`.

## Ссылки

- docs/SCOPE.md («Показ нажатых клавиш»); docs/ARCHITECTURE.md («Панель эффекта»); docs/decisions/015-project-evaluate.md, 009-input-hooks.md; задачи 5.26 (docs/roadmap/05-render.md), 7.16 и 7.35 (07-editor.md).
