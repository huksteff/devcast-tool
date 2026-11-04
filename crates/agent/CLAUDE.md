# dc-agent (`crates/agent`)

«Сделать красиво» (8.11): нейронка пользователя монтирует проект командами `devcast`. Общий для вкладки «Нейронка» редактора (dc-app) и `devcast beautify` (dc-cli).

## Инварианты

- Нейронка — Claude Code (`claude -p`) или Codex CLI (`codex exec`) на подписке пользователя; ключей не храним. Поиск (`find.rs`): `PATH`, `~\.local\bin` (установщик Claude Code), `%APPDATA%\npm` (`.cmd` npm), последним — Claude Code внутри Claude Desktop (`%LOCALAPPDATA%\Claude-Data\claude_*\claude-code\<версия>`, новейшая; работает после своего входа). Выбор — `pick` (настройка `ai.agent`, иначе первая найденная).
- Рецепты, выбор нейронки и модели — настройки программы (`dc_settings::ai`, правка `Edit::Ai`); здесь только запуск.
- Промт (`prompt.rs`): системная часть — короткий бриф `brief.md` (`BRIEF`; нейронка перечитывает его на каждом шаге, поэтому мал; примеры проверяет `brief_examples_run` в dc-cli): правила запуска, порядок работы, частые команды. Полный `skill/SKILL.md` (`SKILL`) — только для `devcast skill install` и скилла своей нейронки. Первый запрос — отмеченные рецепты по порядку и слова пользователя (`first`), продолжение разговора — `more`.
- Запуск (`run.rs`): Claude Code — `--tools Bash,PowerShell --allowedTools "Bash(devcast *)" "PowerShell(devcast *)" --permission-prompts none --strict-mcp-config` (на Windows с 2.1.317 команды идут через PowerShell), бриф — файлом `--append-system-prompt-file` (`--system-prompt-file` ломает вызов инструментов) (`system_file`, в командную строку не влезает), запрос — в stdin, продолжение — `--resume <сессия>`; Codex — `exec --json --sandbox workspace-write`, skill — в первом запросе. Папка запуска — проект; `devcast` рядом с программой — первым в `PATH`; `DEVCAST_GROUP` (`GROUP_ENV`) — id запуска: правки его команд — один шаг отмены (`History::group`).
- Вход (`sign_in.rs`): `claude auth status` (JSON `loggedIn`), вход — `claude auth login` / `codex login` в своём окне консоли.
- Скилл для своей нейронки (`skills.rs`, 8.10): папка `agent-skill` в папке настроек — `SKILL.md`, `AGENTS.md` (без шапки skill) и `README.md` (как поставить себе: Claude Code, Codex, Gemini, Cursor, другие) и просьба `ask`, которую пользователь отдаёт нейронке.
- Ход работы (`stream.rs`): строки `stream-json` Claude Code и `--json` Codex → `Event` (`Session`, `Command`, `Done`, `Said`, `Usage` — модель и токены, `Limits` — доли 5-часового и недельного лимита из `rate_limit_event`, `Finished`; `usage.rs`); последнее — всегда `Finished`, конец вывода без него — ошибка.
- `Job` читает вывод своим потоком `devcast-agent` и будит вызывающего после каждого события; остановка и drop гасят всё дерево процессов (`taskkill /T`: npm-обёртка `.cmd` запускает node под cmd.exe); окна консоли нет (`CREATE_NO_WINDOW`).

## Как тестировать

- `cargo nextest run -p dc-agent`: поиск во временной папке, командные строки обеих нейронок, разбор их вывода (`stream_tests.rs`, образец — вывод Claude Code 2.1.309), запуск и остановка поддельной нейронки (`cmd /C type`, `ping`).
- Живой прогон — с залогиненным Claude Code или Codex (за владельцем).

## Куда не лезть

- Вкладка редактора — dc-app, команды `beautify`/`recipes` — dc-cli, модель настроек — dc-settings.

## Ссылки

- docs/roadmap/08-release.md (8.11), docs/design/editor.html (раздел 9), skill/SKILL.md.
