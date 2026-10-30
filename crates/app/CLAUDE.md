# dc-app (`crates/app`)

Бинарник devcast-app: HUD, окна записи, редактор, таймлайн, библиотека, настройки. Логика в библиотеке (`run`), `main.rs` только печатает ошибку.

## Инварианты

- Один экземпляр на пользователя (`instance.rs`): первый держит канал dc-ipc `devcast-app-<пользователь>`, второй запуск просит его показаться и выходит.
- Журнал (`logging.rs`): `%APPDATA%\DevCast\logs\devcast-app.*.log`, ротация раз в сутки, хранится 7 файлов, паника пишется в журнал с backtrace. `RUST_LOG` задаёт уровень.
- Перерисовка только по событию (решение 002): безусловный `request_repaint()` запрещён, анимации — `request_repaint_after`.
- eframe собран с `inspection` + `accesskit`: `EGUI_INSPECTION=1` открывает порт для egui-mcp (`.mcp.json`, решение 012), без переменной инспекция выключена.
- Настройки (`settings.rs`) — один файл `%APPDATA%\DevCast\settings.json` и один `Store` на все окна (`app::Shared` за `Arc<Mutex>`): HUD и окно настроек правят одно и то же. Каждое изменение пишется сразу и атомарно (`Store::update`, без изменений — без записи); поля, которых нет в файле, берут умолчания; испорченный файл — ошибка с советом удалить его. Модель Whisper — `SpeechModel` (`fast`/`accurate`, как `--model` CLI).
- Окна кроме главного — deferred viewports (решение 002) с общим `Shared`. Окно настроек (`settings_window.rs`): разделы слева, изменения сразу, без «Сохранить»; выбор fps и отсчёта — общие функции `fps`/`countdown` с HUD.
- Любое действие UI с проектом — команда из реестра dc-ops. Строки интерфейса — из dc-ui-kit.

## Как тестировать

- `cargo nextest run -p dc-app`: логика окон и снимки egui_kittest (`tests/snapshots`, тесты окон — `*_tests.rs` рядом с кодом, настройки — во временной папке).
- Живой прогон: `EGUI_INSPECTION=1 cargo run -p dc-app`, затем egui-mcp (`attach`, `widget_tree`, `screenshot`).

## Куда не лезть

- Захват и запись — dc-capture, файлы проекта — dc-project, виджеты и тема — dc-ui-kit.

## Ссылки

- docs/roadmap/06-recording-ui.md, docs/decisions/001-hud-window.md, 002-multi-window.md, 012-testing.md, docs/design/recording-windows.html.
