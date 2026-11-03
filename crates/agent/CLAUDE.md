# dc-agent (`crates/agent`)

«Сделать красиво» (8.11): нейронка пользователя монтирует проект командами `devcast`. Общий для вкладки «Нейронка» редактора (dc-app) и `devcast beautify` (dc-cli).

## Инварианты

- Нейронка — Claude Code (`claude -p`) или Codex CLI (`codex exec`) на подписке пользователя; ключей не храним. Поиск (`find.rs`): `PATH`, `~\.local\bin` (установщик Claude Code), `%APPDATA%\npm` (`.cmd` npm); Claude Code внутри Claude Desktop не ищем — сам по себе он не залогинен. Выбор — `pick` (настройка `ai.agent`, иначе первая найденная).
- Рецепты, выбор нейронки и модели — настройки программы (`dc_settings::ai`, правка `Edit::Ai`); здесь только запуск.
- Промт (`prompt.rs`): системная часть — `skill/SKILL.md` (`SKILL`, его же ставит `devcast skill install`) и правила запуска (`RULES`): только команды `devcast`, без экспорта, сохранения и команд программы. Первый запрос — отмеченные рецепты по порядку и слова пользователя (`first`), продолжение разговора — `more`.
- Запуск (`run.rs`): Claude Code — `--tools Bash --allowedTools "Bash(devcast *)" --permission-prompts none --strict-mcp-config`, skill — файлом `--append-system-prompt-file` (`system_file`, в командную строку не влезает), запрос — в stdin, продолжение — `--resume <сессия>`; Codex — `exec --json --sandbox workspace-write`, skill — в первом запросе. Папка запуска — проект; `devcast` рядом с программой — первым в `PATH`; `DEVCAST_GROUP` (`GROUP_ENV`) — id запуска: правки его команд — один шаг отмены (`History::group`).
- Ход работы (`stream.rs`): строки `stream-json` Claude Code и `--json` Codex → `Event` (`Session`, `Command`, `Done`, `Said`, `Finished`); последнее — всегда `Finished`, конец вывода без него — ошибка.
- `Job` читает вывод своим потоком `devcast-agent` и будит вызывающего после каждого события; остановка и drop гасят всё дерево процессов (`taskkill /T`: npm-обёртка `.cmd` запускает node под cmd.exe); окна консоли нет (`CREATE_NO_WINDOW`).

## Как тестировать

- `cargo nextest run -p dc-agent`: поиск во временной папке, командные строки обеих нейронок, разбор их вывода (`stream_tests.rs`, образец — вывод Claude Code 2.1.309), запуск и остановка поддельной нейронки (`cmd /C type`, `ping`).
- Живой прогон — с залогиненным Claude Code или Codex (за владельцем).

## Куда не лезть

- Вкладка редактора — dc-app, команды `beautify`/`recipes` — dc-cli, модель настроек — dc-settings.

## Ссылки

- docs/roadmap/08-release.md (8.11), docs/design/editor.html (раздел 9), skill/SKILL.md.
