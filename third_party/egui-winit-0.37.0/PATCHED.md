# egui-winit 0.37.0 с правкой DevCast

Точная копия опубликованного крейта `egui-winit` 0.37.0 (crates.io) с одной правкой. Подключена в корневом `Cargo.toml` через `[patch.crates-io]`, решение [019](../../docs/decisions/019-patched-winit.md). Это чужой код: `xtask verify`, clippy и `cargo fmt` его не проверяют (папка вне workspace). Лицензия — MIT OR Apache-2.0 (поле `license` в `Cargo.toml`; файлов лицензии в опубликованном пакете нет).

## Что изменено

`src/lib.rs`, `create_winit_window_attributes`, ветка Windows: тень окна без рамки (`with_undecorated_shadow`) не включается, если в `ViewportBuilder` стоит `with_has_shadow(false)`. Без правки `has_shadow` действует только на macOS, а на Windows у каждого окна без рамки есть тень DWM. Тень поповера HUD (окно в 8 px над HUD) ложилась на HUD и затемняла его.

Обработчик `ViewportCommand::Decorations` не тронут: команде не передаётся `ViewportBuilder`, и смена рамки на лету снова включит тень. DevCast рамку на лету не меняет.

Тот же дифф — `docs/upstream/egui-winit-0.37.0-has-shadow-windows.patch`.

## Когда убрать

Когда в egui-winit появится то же поведение (PR в egui — только с согласия владельца): удалить папку и строку `egui-winit` из `[patch.crates-io]`.
