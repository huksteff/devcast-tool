# winit 0.30.13 с правкой DevCast

Точная копия опубликованного крейта `winit` 0.30.13 (crates.io) с одной правкой. Подключена в корневом `Cargo.toml` через `[patch.crates-io]`, решение [019](../../docs/decisions/019-patched-winit.md). Это чужой код: `xtask verify`, clippy и `cargo fmt` его не проверяют (папка вне workspace). Лицензия — `LICENSE` (Apache-2.0).

## Что изменено

`src/platform_impl/windows/window.rs`, `Window::focus_window`: если окно уже на переднем плане, но фокуса клавиатуры нет нигде (`GetFocus()` пустой), окно получает его через `SetFocus`. Без правки `focus_window` ничего не делает с окном переднего плана, и `ViewportCommand::Focus` не помогает. Так было в выборе области: поповер выбора источника с фокусом закрывался, окно области становилось передним, но без фокуса, и Esc/Enter не работали до клика.

Тот же дифф — `docs/upstream/winit-0.30.13-focus-without-keyboard-focus.patch`.

## Когда убрать

Когда в winit (ветка 0.30 или версия, которую берёт egui-winit) появится то же поведение: удалить папку и строку `winit` из `[patch.crates-io]`.
