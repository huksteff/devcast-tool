# 018. Распознавание вне окна программы

Дата: 2026-10-31. Код: `crates/models` (`dc-models`), `crates/ops/src/recognition.rs` (протокол), `crates/cli/src/worker.rs` и `speech.rs` (процесс), `crates/app/src/recognition/job.rs` (сторона редактора), правило `NEVER_LINKS` в `xtask/src/deps.rs`.

## Задача

Решение владельца 2026-10-31, вариант А: `devcast-app` не тянет whisper.cpp и Vulkan. Раньше exe импортировал `vulkan-1.dll` (и `MSVCP140.dll` из whisper.cpp): dc-app зависел от dc-transcribe ради загрузки модели в настройках и через dc-ops ради субтитров. На ПК без драйвера Vulkan программа не запустилась бы даже для записи экрана.

## Решение

- Модели (список, имена файлов, папка, докачка с SHA-256, прогресс, отмена) — базовый крейт `dc-models` без whisper. dc-app качает модели только через него; dc-transcribe использует его же (`dc_transcribe::Model` — тот же тип).
- Распознаёт редактор скрытым режимом CLI: `devcast --captions-worker`. `devcast.exe` уже собран с whisper и делает то же (`captions gen`), отдельный бинарник дал бы третий exe с той же сборкой whisper. Режим не подкоманда clap: он отвечает строками по ходу работы, а `run` CLI возвращает готовый `Outcome`; поэтому `main.rs` проверяет единственный аргумент `--captions-worker` до разбора (`worker::is_worker`), в справке его нет.
- Протокол 7.17 без изменений по сути: JSON `Request` (папка, проект, `Speech`: модель, язык, дорожки) в stdin, строки `progress <0..1>`, `silence <dBFS>` (свой порог тишины первой записи со словами, пока в проекте `auto`; с 2026-11-04 — для ползунка порога в панели субтитров), затем `cues <JSON>` или `error <текст>`. Типы — `dc_ops::recognition`, их линкуют обе стороны.
- Путь `captions gen` (распознавание записей, кэш слов, реплики, слияние дорожек) переехал из dc-ops в dc-cli (`speech.rs`): dc-ops больше не зависит от dc-transcribe. Один путь остаётся: `captions gen`, `transcript` и процесс редактора.
- Редактор ищет `devcast.exe` в папке своего exe (`job::beside`): cargo собирает оба в `target/<профиль>`, установщик должен класть их рядом: в папку пакета Velopack (`cargo xtask package`, `vpk pack --packDir`, решение 013) входят `devcast-app.exe`, `devcast.exe`, DLL FFmpeg и `vulkan-1.dll`, как в `target/release` после `xtask deps`. Нет файла — понятная ошибка «переустанови DevCast».
- Защита: `cargo xtask verify` (правило `NEVER_LINKS`) запрещает dc-app зависеть от dc-transcribe через любую цепочку обычных зависимостей; цепочка печатается в ошибке.

## Отклонено

- Отдельный маленький бинарник распознавания: ещё одна сборка whisper и ещё один exe в пакете ради того, что CLI уже умеет.
- Грузить whisper в `devcast-app` динамически (`LoadLibrary`): `unsafe` вне разрешённых модулей и свой загрузчик.

## Цена

Без `devcast.exe` рядом редактор не распознаёт речь (запись и монтаж работают). Процессы CLI и редактора должны быть одной версии: протокол не версионируется, они ставятся вместе.
