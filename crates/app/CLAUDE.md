# dc-app (`crates/app`)

Бинарник devcast-app: HUD, окна записи, редактор, таймлайн, библиотека, настройки. Логика в библиотеке (`run`), `main.rs` только печатает ошибку.

## Инварианты

- Один экземпляр на пользователя (`instance.rs`): первый держит канал dc-ipc `devcast-app-<пользователь>`, второй запуск просит его показаться и выходит.
- Журнал (`logging.rs`): `%APPDATA%\DevCast\logs\devcast-app.*.log`, ротация раз в сутки, хранится 7 файлов, паника пишется в журнал с backtrace. `RUST_LOG` задаёт уровень.
- Перерисовка только по событию (решение 002): безусловный `request_repaint()` запрещён, анимации — `request_repaint_after`.
- eframe собран с `inspection` + `accesskit`: `EGUI_INSPECTION=1` открывает порт для egui-mcp (`.mcp.json`, решение 012), без переменной инспекция выключена.
- Любое действие UI с проектом — команда из реестра dc-ops. Строки интерфейса — из dc-ui-kit.

## Как тестировать

- `cargo nextest run -p dc-app`: логика окон и снимки egui_kittest.
- Живой прогон: `EGUI_INSPECTION=1 cargo run -p dc-app`, затем egui-mcp (`attach`, `widget_tree`, `screenshot`).

## Куда не лезть

- Захват и запись — dc-capture, файлы проекта — dc-project, виджеты и тема — dc-ui-kit.

## Ссылки

- docs/roadmap/06-recording-ui.md, docs/decisions/001-hud-window.md, 002-multi-window.md, 012-testing.md, docs/design/recording-windows.html.
