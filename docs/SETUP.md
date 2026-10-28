# Установка на новый компьютер

Инструкция для нейронки. Владелец разрешил поставить всё из этого списка, только из официальных источников. Что требует прав администратора, не ставь сам: попроси владельца и дай ему готовую команду или ссылку.

Делай по порядку и после каждого шага проверяй командой из колонки «Проверка». Скачанное клади в `.deps/` (папка в `.gitignore`).

## Обязательно: без этого не собирается

| # | Что | Как поставить | Проверка |
|---|---|---|---|
| 1 | Visual Studio Build Tools 2022, нагрузка «Разработка классических приложений на C++» (MSVC и Windows SDK) | **Нужны права администратора.** Владелец ставит с https://visualstudio.microsoft.com/visual-cpp-build-tools/ | `cargo build` в п. 7 проходит без ошибок линковки |
| 2 | Rust через rustup | https://rustup.rs (`rustup-init.exe`, ставится в профиль пользователя). Версию из `rust-toolchain.toml` rustup поставит сам при первой сборке | `cargo --version` |
| 3 | Git LFS | https://git-lfs.com, затем `git lfs install` и `git lfs pull` в репозитории | `git lfs ls-files` |
| 4 | cargo-nextest | `cargo install cargo-nextest --locked` | `cargo nextest --version` |
| 5 | FFmpeg и libclang | `cargo xtask deps`: скачивает в `.deps/`, проверяет SHA-256 и копирует DLL к exe | в `.deps/` появились `ffmpeg/` и `libclang/` |
| 6 | Проверка, что всё работает | `cargo xtask verify --full` | `verify: OK` |

## Для проб и следующих этапов: ставь, когда дойдёшь до этапа

| Что | Зачем | Как поставить |
|---|---|---|
| CMake 4.4.5, portable zip | собирает whisper.cpp (субтитры) | zip `cmake-4.4.5-windows-x86_64.zip` с https://cmake.org/download/ распаковать в `.deps/cmake`, добавить `.deps/cmake/bin` в PATH для сборки |
| Модели Whisper | субтитры | `ggml-small.bin` и `ggml-large-v3-turbo.bin` с https://huggingface.co/ggerganov/whisper.cpp в `.deps/models/`. SHA-256 сверить с docs/decisions/010-whisper.md |
| Шрифты | текст в рендере | Inter, Montserrat, Roboto, PT Serif, JetBrains Mono из https://github.com/google/fonts в `.deps/fonts/`; Noto Color Emoji оттуда же. См. docs/decisions/005-text.md |
| .NET SDK 10 (LTS) и vpk | установщик и обновления (Velopack) | без админа: официальный `dotnet-install.ps1 -Channel LTS` (https://dot.net/v1/dotnet-install.ps1) в `%LOCALAPPDATA%\Microsoft\dotnet`, затем `dotnet tool install -g vpk`. См. docs/decisions/013-velopack.md |
| egui-mcp | смотреть окна приложения в тестах | `cargo install egui_mcp --locked`. См. docs/decisions/012-testing.md |
| Vulkan SDK | GPU для Whisper | **Нужны права администратора**, вопрос открыт в docs/QUESTIONS.md. Владелец ставит с https://vulkan.lunarg.com |

## После установки

- Пробы этапа 1 лежат в `spikes/`, отдельный workspace: `cd spikes && cargo run -p spike-hud -- --exclude --seconds 30`.
- Дальше работай по CLAUDE.md: прочитай docs/PROCESS.md и текущую задачу в docs/roadmap/.
- Тестовые окна и записи экрана — только с разрешения владельца и на мониторе, который он назовёт.
