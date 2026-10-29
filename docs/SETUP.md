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
| 5 | Vulkan SDK (распознавание речи на видеокарте, решение 010) | **Нужны права администратора.** Владелец ставит установщик Windows с https://vulkan.lunarg.com/sdk/home (версия 1.4.363.0 или новее, компоненты по умолчанию). Установщик задаёт системную переменную `VULKAN_SDK`; после установки перезапусти терминал. Тихая установка (так ставит CI): `vulkansdk-windows-X64-1.4.363.0.exe --root C:\VulkanSDK --accept-licenses --default-answer --confirm-command install`, затем задать `VULKAN_SDK=C:\VulkanSDK` | `echo %VULKAN_SDK%` показывает папку SDK, в ней есть `Bin\glslc.exe` и `Lib\vulkan-1.lib` |
| 6 | Короткая папка сборки | Пользовательская переменная `CARGO_TARGET_DIR=D:\dct` (любая папка с коротким путём; без админа: `setx CARGO_TARGET_DIR D:\dct`), затем перезапусти терминал и Claude. whisper.cpp с Vulkan собирает вспомогательную программу через MSBuild, а он не умеет пути длиннее 260 символов: из `<репозиторий>\target` путь получается длиннее. Переменная — для основной папки; у каждого worktree своя короткая папка (`CARGO_TARGET_DIR=D:\dct-<имя>` в его командах): общая папка у двух worktree портит сборку — одинаковые крейты из разных папок перезаписывают друг друга. Папку worktree удаляй вместе с ним | `echo %CARGO_TARGET_DIR%` |
| 6а | FFmpeg, libclang, CMake, загрузчик Vulkan | `cargo xtask deps`: скачивает в `.deps/`, проверяет SHA-256 и копирует DLL (FFmpeg и `vulkan-1.dll`) к exe в папку сборки | в `.deps/` появились `ffmpeg/`, `libclang/`, `cmake/`, `vulkan/` |
| 7 | Проверка, что всё работает | `cargo xtask verify --full` | `verify: OK` |

## Для проб и следующих этапов: ставь, когда дойдёшь до этапа

| Что | Зачем | Как поставить |
|---|---|---|
| CMake 4.4.5, portable zip | собирает whisper.cpp (субтитры) | zip `cmake-4.4.5-windows-x86_64.zip` с https://cmake.org/download/ распаковать в `.deps/cmake`, добавить `.deps/cmake/bin` в PATH для сборки |
| Модели Whisper | субтитры | `ggml-small.bin` и `ggml-large-v3-turbo.bin` с https://huggingface.co/ggerganov/whisper.cpp в `.deps/models/`. SHA-256 сверить с docs/decisions/010-whisper.md |
| Шрифты | текст в рендере | `cargo xtask deps` (п. 6) скачивает в `.deps/fonts/` с проверкой SHA-256: Inter, Montserrat, Roboto, PT Serif, JetBrains Mono из https://github.com/google/fonts; Noto Color Emoji (сборка CBDT) из релиза https://github.com/googlefonts/noto-emoji. См. docs/decisions/005-text.md |
| .NET SDK 10 (LTS) и vpk | установщик и обновления (Velopack) | без админа: официальный `dotnet-install.ps1 -Channel LTS` (https://dot.net/v1/dotnet-install.ps1) в `%LOCALAPPDATA%\Microsoft\dotnet`, затем `dotnet tool install -g vpk`. См. docs/decisions/013-velopack.md |
| egui-mcp | смотреть окна приложения в тестах | `cargo install egui_mcp --locked`. См. docs/decisions/012-testing.md |

## После установки

- Пробы этапа 1 лежат в `spikes/`, отдельный workspace: `cd spikes && cargo run -p spike-hud -- --exclude --seconds 30`.
- Дальше работай по CLAUDE.md: прочитай docs/PROCESS.md и текущую задачу в docs/roadmap/.
- Тестовые окна и записи экрана — только с разрешения владельца и на мониторе, который он назовёт.
