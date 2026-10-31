# 019. Исправленные копии egui-winit и winit

Дата: 2026-10-31. Код: `third_party/egui-winit-0.37.0`, `third_party/winit-0.30.13` (в каждой `PATCHED.md`), `[patch.crates-io]` и `exclude` в корневом `Cargo.toml`, `crates/app/src/borderless.rs`, `crates/app/src/area/mod.rs` (`Screen::focus`). Диффы для будущего PR — `docs/upstream/*.patch`.

## Задача

Живая проверка этапа 6 нашла две ошибки слоя окон на Windows, которые не исправить из нашего кода:

1. **Тень поповера на HUD.** egui-winit включает тень winit (`with_undecorated_shadow`) у каждого окна без рамки, а `ViewportBuilder::with_has_shadow(false)` учитывает только на macOS. Поповер стоит в 8 px над HUD, и его тень DWM затемняла HUD (#e11d48 на экране становился #971330, хотя пиксели окна верные).
2. **Esc/Enter в выборе области не работали до клика.** Поповер выбора источника держал фокус и закрывался, окно области становилось передним, но фокус клавиатуры оставался пустым. `Window::focus_window` winit ничего не делает с окном переднего плана, поэтому `ViewportCommand::Focus` не помогал.

## Решение

Решение владельца 2026-10-31: точные копии опубликованных крейтов в `third_party/` с минимальными правками, подключение через `[patch.crates-io]`.

- egui-winit: при создании окна тень без рамки не включается, если `has_shadow == Some(false)`.
- winit: `focus_window` у окна переднего плана без фокуса клавиатуры (`GetFocus()` пустой) зовёт `SetFocus`.
- dc-app: все окна без рамки (HUD, поповеры, отсчёт, выбор области) строит `borderless::window` с `with_has_shadow(false)`. Ручной `set_undecorated_shadow(false)` у HUD и прямая зависимость dc-app от winit убраны. Окно выбора области на мониторе HUD один раз просит `ViewportCommand::Focus` на первом кадре.
- Папка `third_party` исключена из workspace (`exclude`): clippy, `cargo fmt` и проверки `xtask` (они смотрят только `crates`, `fx`, `xtask` и пакеты workspace) её не трогают.

PR в egui и winit — только с отдельного согласия владельца.

## Отклонено

- Свой Win32-код поверх winit (в `dc_capture::win`: снять тень, `SetFocus` по hwnd после создания окна): обход чужого состояния — winit не знает о смене, egui-winit включает тень снова при пересоздании окна, а фокус пришлось бы ловить по кадрам.
- Поповер ниже или с отступом больше тени: макет ставит его в 8 px над HUD.

## Цена

Обновление egui (и с ним egui-winit/winit) требует перенести правки в новые копии или убрать их, если upstream их принял. Смена рамки окна на лету (`ViewportCommand::Decorations`) по-прежнему включает тень: команде не передаётся `ViewportBuilder`; DevCast рамку на лету не меняет.

## Дополнение

2026-10-31: вторая правка egui-winit — прозрачное окно без битмапа перенаправления, решение [020](020-transparent-windows.md).
