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
- `info` и `ls <список>` (`ls.rs`): списки — `clips` и `Effect::LISTS` эффектов; время объектов — выходное (`TimeMap::output_span`), вырезанные объекты в конце с пометкой «вырезан». Фильтры `--from/--to` по пересечению, `--fields`, страницы `--limit/--offset` с подсказкой следующей.
- Новый эффект регистрируется одной строкой в `registry.rs`.
- `skill install` (`skill.rs`) кладёт `skill/SKILL.md` (вшит через `include_str!`) в `~/.claude/skills/devcast/`. Строки `devcast …` в блоках кода SKILL.md и JSON-план оттуда `skill_tests` прогоняет подряд: меняешь команды или их аргументы — правь SKILL.md.

## Как тестировать

- `cargo nextest run -p dc-cli`. Сценарии — `src/tests.rs` с тестовыми командами `clip add/rm` на временной папке с кириллицей и пробелами; `src/edit_tests.rs` — apply/undo/redo/history/save; `src/ls_tests.rs` — info/ls на настоящем реестре; `src/skill_tests.rs` — примеры SKILL.md и установка в временную папку; `tests/binary.rs` проверяет настоящий exe (коды выхода, stdout/stderr).

## Куда не лезть

- Разбор аргументов команд, планы, dry-run, вывод diff — `dc-ops`. Файлы проекта, lock, история на диске — `dc-project`. Сами команды — `fx/*`.

## Ссылки

- Задачи 2.16–2.19, 2.23 в docs/roadmap/02-core.md, раздел «CLI» в docs/ARCHITECTURE.md.
