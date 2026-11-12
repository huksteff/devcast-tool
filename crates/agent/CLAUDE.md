# dc-agent (`crates/agent`)

«Сделать красиво» (8.11): нейронка пользователя монтирует проект командами `devcast`. Общий для вкладки «Нейронка» редактора (dc-app) и `devcast beautify` (dc-cli).

## Инварианты

- Нейронка — Claude Code (`claude -p`) или Codex CLI (`codex app-server --stdio`) на подписке пользователя; ключей не храним. Поиск (`find.rs`): `PATH`, `~\.local\bin` (установщик Claude Code), `%APPDATA%\npm` (`.cmd` npm), и Claude Code внутри Claude Desktop (`%LOCALAPPDATA%\Claude-Data\claude_*\claude-code\<версия>`; работает после своего входа); из найденных Claude Code запускается самый новый (`--version`). Codex Desktop — `%LOCALAPPDATA%\OpenAI\Codex\bin\<build>\codex.exe`; среди Codex тоже берётся новейшая версия. Выбор — `pick` (настройка `ai.agent`, при auto первая найденная; явно выбранного нет — ошибка).
- Рецепты, выбор нейронки и модели — настройки программы (`dc_settings::ai`, правка `Edit::Ai`); здесь только запуск.
- Каталог моделей Codex (`models.rs`) — штатный model/list с пагинацией, без скрытых моделей. Отдельный отменяемый процесс с теми же отключёнными инструментами, без thread/start и turn/start; UI не ждёт ответа.
- Промт (`prompt.rs`): системная часть — короткий бриф `brief.md` (`BRIEF`; нейронка перечитывает его на каждом шаге, поэтому мал; примеры проверяет `brief_examples_run` в dc-cli): правила запуска, порядок работы, частые команды. Полный `skill/SKILL.md` (`SKILL`) — только для `devcast skill install` и скилла своей нейронки. Первый запрос — отмеченные рецепты по порядку и слова пользователя (`first`), продолжение разговора — `more`.
- Запуск (`run.rs`): Claude Code — прежние `--tools Bash,PowerShell --allowedTools "Bash(devcast *)" "PowerShell(devcast *)" --strict-mcp-config`, бриф — `--append-system-prompt-file`, продолжение — `--resume`. Codex — штатный `app-server --stdio` (`codex/`, решение 021): один клиентский инструмент devcast; оболочка, сторонние инструменты и MCP отключены, модель read-only с approvalPolicy=never. До первого запроса проверяются конфигурация и MCP-инвентарь. Инструмент принимает только argv, клиент фиксирует EXE, проект и DEVCAST_GROUP; команды идут существующим CLI и IPC. Продолжение — thread/resume с новой группой. Никакого общего автоматического одобрения. Папка devcast первой в PATH процесса.
- Вход (`sign_in.rs`): `claude auth status` (JSON `loggedIn`); он говорит «вошёл» и с отозванным токеном, поэтому после отказа (`refused`: 401, «session has expired») проверка — настоящий запрос (`answers`). Вход Claude Code — `login.rs`: `claude auth login` с каналами, страницу открывает программа, код уходит в stdin; Codex — `codex login status` (stdout/stderr и код выхода), вход `codex login` через тот же `login.rs`, без окна консоли.
- Скилл для своей нейронки (`skills.rs`, 8.10): папка `agent-skill` в папке настроек — `SKILL.md`, `AGENTS.md` (без шапки skill) и `README.md` (как поставить себе: Claude Code, Codex, Gemini, Cursor, другие) и просьба `ask`, которую пользователь отдаёт нейронке.
- Ход работы: Claude stream-json разбирает stream.rs; нативный протокол Codex — codex/state.rs. Общие Event: Session, Command, Done, Said, Usage, Limits, Finished. Последнее — всегда Finished, закрытый без него поток — ошибка. Native Codex: зарегистрирован только devcast, чужие RPC-запросы отклоняются; command/exec формирует только клиент с фиксированным EXE и новой группой, дочерние команды гасятся вместе с app-server.
- `Job` читает вывод своим потоком `devcast-agent` и будит вызывающего после каждого события; остановка и drop гасят всё дерево процессов (`taskkill /T`: npm-обёртка `.cmd` запускает node под cmd.exe); окна консоли нет (`CREATE_NO_WINDOW`).

## Как тестировать

- `cargo nextest run -p dc-agent`: поиск во временной папке, командные строки обеих нейронок, разбор их вывода (`stream_tests.rs`, образец — вывод Claude Code 2.1.309), запуск и остановка поддельной нейронки (`cmd /C type`, `ping`).
- Живой прогон — с залогиненным Claude Code или Codex (за владельцем).

## Куда не лезть

- Вкладка редактора — dc-app, команды `beautify`/`recipes` — dc-cli, модель настроек — dc-settings.

## Ссылки

- docs/roadmap/08-release.md (8.11), docs/design/editor.html (раздел 9), skill/SKILL.md.
