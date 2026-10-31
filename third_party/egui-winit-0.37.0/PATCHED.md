# egui-winit 0.37.0 с правками DevCast

Точная копия опубликованного крейта `egui-winit` 0.37.0 (crates.io) с двумя правками. Подключена в корневом `Cargo.toml` через `[patch.crates-io]`, решения [019](../../docs/decisions/019-patched-winit.md) и [020](../../docs/decisions/020-transparent-windows.md). Это чужой код: `xtask verify`, clippy и `cargo fmt` его не проверяют (папка вне workspace). Лицензия — MIT OR Apache-2.0 (поле `license` в `Cargo.toml`; файлов лицензии в опубликованном пакете нет).

## Что изменено

Обе правки — в `src/lib.rs`, `create_winit_window_attributes`, ветка Windows.

1. Тень окна без рамки (`with_undecorated_shadow`) не включается, если в `ViewportBuilder` стоит `with_has_shadow(false)`. Без правки `has_shadow` действует только на macOS, а на Windows у каждого окна без рамки есть тень DWM. Тень поповера HUD (окно в 8 px над HUD) ложилась на HUD и затемняла его. Дифф — `docs/upstream/egui-winit-0.37.0-has-shadow-windows.patch`.
2. Прозрачное окно (`with_transparent(true)`) создаётся с `WS_EX_NOREDIRECTIONBITMAP` (`with_no_redirection_bitmap` winit). Флаг действует только при создании окна. Без него у окна есть битмап перенаправления GDI, и прозрачные пиксели swapchain из DirectComposition (`Dx12SwapchainKind::DxgiFromVisual`) показывают его — белый фон (решение 001). Дифф поверх первой правки — `docs/upstream/egui-winit-0.37.0-no-redirection-bitmap-windows.patch`.

Обработчики `ViewportCommand::Decorations` и `ViewportCommand::Transparent` не тронуты: команде не передаётся `ViewportBuilder`, а флаг перенаправления на живом окне не меняется. DevCast рамку и прозрачность на лету не меняет.

## Когда убрать

Когда в egui-winit появится то же поведение (PR в egui — только с согласия владельца): удалить папку и строку `egui-winit` из `[patch.crates-io]`.
