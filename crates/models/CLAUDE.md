# dc-models (`crates/models`)

Файлы моделей Whisper: какие есть, где лежат, загрузка с докачкой и проверкой SHA-256. Самого whisper здесь нет: окно программы качает модели через этот крейт, не линкуя whisper.cpp и Vulkan (решение 018).

## Инварианты

- Модели лежат в `models_dir()` = `%APPDATA%\DevCast\models`; источник и SHA-256 — в `Model` (решение 010), примерный размер для списков — `Model::megabytes`. В JSON `Model` — `small` / `large_v3_turbo` (запрос процесса распознавания).
- Файл под настоящим именем всегда проверен: качаем в `<файл>.part`, проверяем SHA-256, переименовываем; обрыв и отмена (`cancel`) оставляют `.part` для докачки, `.part` с чужим хэшем удаляется. HTTP — только ureq (блокирующий, rustls), докачка — `Range`, 416 — `.part` уже целый.
- Один `.part` пишет одна загрузка: две загрузки одной модели в одном процессе не запускать (в программе их объединяет `crates/app/src/models.rs`).
- Ошибки (`ModelsError`) по-русски и говорят, что делать.
- Нельзя зависеть от dc-transcribe и whisper-rs: крейт линкует `devcast-app`.

## Как тестировать

- `cargo nextest run -p dc-models` — без сети: загрузка через локальный сервер (`test_server.rs`): докачка, обрыв, отмена, хэш, 416.
- С моделями из `.deps/models` (не в CI): `cargo nextest run -p dc-models --run-ignored only` — хэши настоящих моделей.

## Куда не лезть

- Распознавание, кэш слов, реплики — `dc-transcribe`. Выбор «Быстрая»/«Точная» — `dc_settings::SpeechModel`, строки интерфейса — `dc-ui-kit`.

## Ссылки

- docs/decisions/010-whisper.md, docs/decisions/018-recognition-process.md.
