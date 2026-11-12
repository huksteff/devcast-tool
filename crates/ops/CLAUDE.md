# dc-ops (`crates/ops`)

Реестр всех команд эффектов, разбор аргументов командной строки и планов, применение пачкой и dry-run, ошибки с подсказками; `evaluate` проекта.

## Инварианты

- Команда для CLI и планов — тип с трейтом `dc_core::Op`: его поля = аргументы. Один тип обслуживает командную строку, план и `help --json`.
- Шаги собираются в команды одной `plan::build_with` (ошибка называет номер шага); вызывающий может сам собрать шаг-сокращение, которое понимает только он (CLI: `cut w12-w20`; CLI и редактор: `zoom add --at clicks` — `autozoom::expand`). Имя записи истории — `step_name`.
- `Registry::new` проверяет правила каждой команды (имя, одна строка описания, пример начинается с имени, `deny_unknown_fields`, позиционные поля существуют). Нарушение = ошибка при сборке реестра, её ловит тест.
- `apply`: любое число шагов — одна запись истории, всё или ничего. Ошибка называет номер шага. Dry-run меняет копию, проект и история не трогаются. Изменение «впустую» в историю не пишется.
- `apply_commands` — то же для готовых команд (не из реестра), например `clip add` после копирования файла.
- Настройки (`settings.rs`): команда с `Op::SETTINGS` — команда настроек своего эффекта под этим ключом (ключи уникальны). `settings_steps` превращает `ключ.аргумент=значение` в шаг этой команды, поэтому `devcast set` и `defaults set` проверяет сама команда.
- Аргументы: `--min-gap` → поле `min_gap`; значение — JSON, если поле не строка по схеме.
- Ошибки по-русски, с подсказкой, что запустить (`devcast help <команда>`). `OpsError::Usage` — неверный запрос (CLI: код 2), `OpsError::Failed` — команда не выполнилась на этом проекте (код 1).
- Реестр всех команд и эффектов — `Registry::devcast()` (`devcast.rs`), один для CLI и редактора; новый эффект добавляет сюда `ops()` и `effect()`.
- `Registry::resolve` находит команду по первым словам строки (самое длинное имя). Поля `project`, `json`, `dry_run` заняты глобальными флагами (`RESERVED_FIELDS`).
- `Evaluator` (`evaluate.rs`, решение 015) — единственный источник `Scene` для превью, экспорта и `frame`/`sheet`. `new(project, cursors, keys, &mut measure)` один раз строит `TimeMap` и готовит эффекты через шов `dc_render::look` (`Prepare` → `Look`; ошибки — `EvaluateError`, своя причина эффекта — `Effect`); телеметрию читает вызывающий. Типов эффектов `evaluate.rs` не знает.
- `Evaluator::evaluate(t)` — чистая функция: `Look::draw` эффектов в порядке списка `LOOKS` (`devcast.rs`): `cut`, `zoom`, аннотации, курсор, облачко, субтитры, клавиши; облачко висит на указателе, который оставил курсор (`Frame::pointer`). Новый эффект с картинкой добавляет туда свой `look::Picture`; холст — `devcast::canvas` (кроп).
- Виды аннотаций в кадре — таблица `KINDS` (`devcast.rs`: вид → `look::READ` крейта вида), её получает `dc_fx_annotations::look::Picture`; тест — у каждого вида есть картинка.
- Экспорт (`export.rs`): `Settings` (реэкспорт `dc_settings::export`), `Spec`, `run` — одно задание для `devcast export` и редактора; сцена — `scene.rs` (телеметрия dc-capture, шрифты). Зависимости: dc-render, эффекты, dc-capture, dc-encode.
- Уровни звука (`sound.rs`): `levels(root, project)` — вход `dc_audio::Mixer`, один для экспорта (dc-cli) и редактора (dc-app).
- Импорт файлов: `import_sound` (`sound_file.rs`, в `assets/audio/`, даёт `AudioAdd`), `import_picture` (`picture_file.rs`, в `assets/images/`, даёт `ImageSet`) и `import_clips` (`clip_file.rs`: видео и проекты `*.devcast` — папка записи копируется в свободную `recording-M`, даёт `ClipAdd`); `discard` удаляет копии, если команду отвергли. Новый проект из видео — `new_project` (`devcast new`); по проекту на видео — `new_projects`, имена `project_roots` (`new --each`, «Импорт…» библиотеки); пустой — `new_empty`, имена `empty_root` («Новый проект», «Новый проект 2» …: `devcast new` без видео и кнопка редактора). Один путь для CLI и редактора.
- Шрифты (`fonts.rs`): `load(root, …)` — встроенные + `assets/fonts/` проекта для `TextMeasure` и `Compositor` (экспорт, `frame`/`sheet`, превью); `families` — список пикеров; `import_font` копирует файл (не команда: стиль, выбравший семейство, — команда с undo). Семейство встроенного шрифта отвергается.
- Автозумы (`autozoom.rs`): `click_zooms` читает телеметрию записей и отдаёт `dc_fx_zoom::suggest`, `click_commands` — их `ZoomAdd` (применять `apply_commands` одной записью), `expand` — шаг `zoom add --at clicks` (`by_clicks`) в эти команды. Один путь для CLI, автозумов после записи и редактора. `after_recording(dir, lock)` — автозумы после записи (стоп `devcast record` и записи из программы, восстановление): одна запись истории, пишется как правка `devcast`. Это не команда реестра: команда — чистая функция проекта, а клики лежат в файлах записей.
- Распознавание (`recognition.rs`, решение 018): протокол `devcast --captions-worker` (`WORKER_ARG`, `Request` JSON в stdin, ответ — строки `Message`). Само распознавание (whisper) — в dc-cli: dc-ops не зависит от dc-transcribe (правило `xtask verify`). Протокол канала программы (CLI → редактор, 7.32) — `live.rs`, события — `live::replay` (`History::push`); запросы о проекте целиком — `Discard`, `Close`, `Copied` (`save --as`), `Rename`; о программе — `Settings` (`dc_settings::Edit`, ответ `Configured`/`NotConfigured`) и `Current` (проект редактора), `Record`, `RecordControl`, `Export`, `Status`, `Quit` (`live_app.rs`); задания экспорта для обеих очередей — `export_jobs.rs`.
- Ответ на изменение — одна строка `Applied` (Display): слитый `Diff` пачки и `длительность до→после`, если она изменилась; `без изменений`; префикс dry-run.

## Как тестировать

- `cargo nextest run -p dc-ops`. Тестовая команда `note set` — в `src/tests.rs`. Юнит-тесты аннотаций — `src/annotations*_tests.rs`, сцены — `src/evaluate_tests.rs`.
- insta-снимки сцены в `src/snapshots/`; после намеренной смены — `INSTA_UPDATE=always` и просмотр diff.
- `tests/golden.rs` — один бинарник эталонов на программном адаптере, по файлу на эффект в `tests/golden/`; помощники — `tests/common`.

## Куда не лезть

- Сами команды — в крейтах `fx/*`. Undo и проверка целостности — `dc-core::History`. Файлы проекта — `dc-project`.

## Ссылки

- Задачи 2.14–2.15 в docs/roadmap/02-core.md, раздел «CLI» в docs/ARCHITECTURE.md.
