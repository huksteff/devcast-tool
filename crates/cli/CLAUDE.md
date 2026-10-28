# dc-cli (`crates/cli`)

Бинарник `devcast`: монтаж, анализ, рендер кадров и экспорт из командной строки. Логика в библиотеке (`run`), `main.rs` только печатает `Outcome`.

## Инварианты

- `run(args, cwd, registry) -> Outcome` — чистый вход для тестов: код выхода, stdout, stderr. stdout — только результат, stderr — ошибки и предупреждения. Коды: 0 готово, 1 не получилось (`CliError::Failed`), 2 неверная командная строка (`CliError::Usage`).
- Глобальные флаги `--project`, `--json`, `--dry-run` снимаются со всей строки (`globals::split`), поэтому стоят где угодно. Поля команд с такими именами запрещает реестр `ops`.
- Встроенные команды — подкоманды clap (`Builtin`), всё остальное — команда из реестра `ops` (`external_subcommand`), её аргументы разбирает `dc_ops::parse_args`.
- Справка (`help.rs`) собирается из тех же определений: у встроенной команды `about` = строка списка, `after_help` = пример (без `devcast`), `help` у аргументов; у команды `ops` — `ABOUT`, `EXAMPLE` и JSON Schema (doc-комментарии полей = описания). `help --json` отдаёт ещё схему `Project` и состояний всех эффектов.
- Проект: `--project`, иначе папка `*.dcproj`, внутри которой стоим, иначе единственная `*.dcproj` в текущей папке.
- Без редактора CLI — единственный писатель (lock). Правит рабочее состояние: `session.json`, если редактор оставил несохранённое, иначе сразу `project.json`. Событие истории пишется до файла проекта.
- Короткий вывод по умолчанию (`Applied` одной строкой), `--json` по запросу; ошибка под `--json` — `{"error", "code"}` в stderr.
- Команды правки (`edit.rs`): команда `ops` и `apply plan.json` — одна запись истории; `undo [n]`, `redo` идут по `History`, на диск пишутся событиями `history.jsonl`; `save` переносит `session.json` в `project.json`. Под `--dry-run` всё делается в памяти без lock и записи.
- `info` и `ls <список>` (`ls.rs`): списки — `clips` и `Effect::LISTS` эффектов; время объектов — выходное (`TimeMap::output_span`), вырезанные объекты в конце с пометкой «вырезан». Фильтры `--from/--to` по пересечению, `--fields`, страницы `--limit/--offset` с подсказкой следующей. Вложенный объект в тексте — поля через точку (`focus.x=0.5`), JSON только под `--json`.
- `record` (`record.rs`, аргументы — `record_args.rs`, решение 011, «Задача 3.15»): процесс записи сам пишет проект (`dc_capture::win::ProjectRecording`) и до старта захвата занимает канал `dc_ipc` `devcast-record-<пользователь>` (`record_control::Server`); `record stop|pause|resume|cancel` из другого вызова шлёт туда `Control` и печатает ответ (`Reply`), который приходит, когда запись это сделала. `--duration` — секунды записи без пауз (таймер `recv_timeout` очереди запросов). Под `--dry-run` ничего не пишется и канал не трогается. Ctrl+C и закрытие консоли — тот же запрос стопа (`Server::stopper` → `on_console_stop`). `ls sources` (`sources.rs`) — мониторы, окна, микрофоны; проект не нужен.
- Открытие проекта (`Workspace::open`) восстанавливает записи убитого процесса: если lock свободен, собирает их и дописывает клип в рабочее состояние (под `--dry-run` только предупреждение, ничего не пишется), по строке предупреждения на запись (решение 007, «Ctrl+C и убитый процесс записи»).
- Новый эффект регистрируется в `registry.rs`: его `ops()` в список команд и `effect()`, если у него есть состояние.
- `skill install` (`skill.rs`) кладёт `skill/SKILL.md` (вшит через `include_str!`) в `~/.claude/skills/devcast/`. Строки `devcast …` в блоках кода SKILL.md и JSON-план оттуда `skill_tests` прогоняет подряд: меняешь команды или их аргументы — правь SKILL.md.

## Как тестировать

- `cargo nextest run -p dc-cli`. Тестовые проекты — `src/test_util.rs`: `Fixture` (пустой проект, тестовые команды `clip add/rm`, папка с кириллицей и пробелами) и `Recordings` (две записи, настоящий реестр). Сценарии — `src/tests.rs`; `src/edit_tests.rs` — apply/undo/redo/history/save; `src/ls_tests.rs` — info/ls на настоящем реестре; `src/skill_tests.rs` — примеры SKILL.md и установка в временную папку; `src/recover_tests.rs` — восстановление убитой записи на синтетических сегментах; `src/record_tests.rs` — аргументы записи, dry-run, канал управления без записи, `ls sources`; `tests/record_live.rs` (`#[ignore]`, `DEVCAST_TEST_MONITOR`) пишет экран через exe; `src/e2e_tests.rs` — весь сценарий агента одним insta-снимком (после намеренной смены вывода — `INSTA_UPDATE=always cargo nextest run -p dc-cli e2e` и просмотр diff снимка; размер вывода — docs/decisions/014-cli-output.md); `tests/binary.rs` проверяет настоящий exe (коды выхода, stdout/stderr).

## Куда не лезть

- Разбор аргументов команд, планы, dry-run, вывод diff — `dc-ops`. Файлы проекта, lock, история на диске — `dc-project`. Сами команды — `fx/*`.

## Ссылки

- Задачи 2.16–2.19, 2.23, 2.24 в docs/roadmap/02-core.md, 3.15 в docs/roadmap/03-capture.md, раздел «CLI» в docs/ARCHITECTURE.md.
