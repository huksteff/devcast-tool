# dc-capture (`crates/capture`)

Запись экрана и звука: модель (источник, настройки, время `HostTime`), платформенный захват (`win/`, позже `mac/`, `linux/`), звук через cpal (`audio/`), папка записи: сегменты fMP4, `manifest.json`, финализация в MP4.

## Инварианты

- Модель (`source`, `config`, `event`) без платформенного кода; платформа только в `win/` и т.п. Время — `HostTime`, мкс на общей шкале QPC для видео, звука и ввода.
- Источник хранится текстом: `screen:<n>` (с 1), `window:<id>` (HWND), `area:x,y,w,h` (пиксели рабочего стола, может быть отрицательным). Невозможное значение не создаётся: `Area::new` и разбор текста отказывают.
- `RecordingConfig`: fps только 30/60/120 (`Fps`), неизвестные поля JSON — ошибка; пустое имя микрофона ловит `validate`. Последний выбор хранит dc-settings (поле `recording` в `settings.json`).
- Мониторы — как в windows-capture (`Monitor::enumerate`, с 1). Окна — список windows-capture минус те, что нельзя записать; порядок — z-order. Превью и стоп-кадр (`win/preview.rs`) — GDI, не WGC: сессия WGC рисует на Windows 10 жёлтую рамку.
- Захват WGC (`win::VideoCapture`): курсор выключен всегда. Область — захват её монитора и кроп на GPU. Кадр ставится в момент прихода (`LeadFilter`, решение 007). `MIN_UPDATE_INTERVAL` — 1 мс.
- Запись видео (`win::VideoRecording`): размер по пробному кадру (вниз до чётного), кодек выбирается до старта (`dc_encode::select`). Кадры на сетке fps (`FrameGrid`, решение 007); 1 с без кадров — повтор последнего ключевым (`REPEAT`).
- Слот кадру даёт только поток кодирования (очередь 4 кадра); захват лишь отсеивает лишние до перевода в NV12. BGRA → NV12 на GPU (D3D11 Video Processor).
- Звук (`audio/`, решение 008): FLAC 48 кГц стерео. `AudioTimeline` ставит пакеты по QPC: дыра > 5 мс — тишина, нахлёст — обрезка, без пакетов > 250 мс — тишина через `fill_to`. Дрейф часов — `DriftLoop` на ресэмплере rubato. `audio/` платформы не знает: часы QPC передаёт `win::Recording`. Системный звук идёт за устройством по умолчанию (переподключение на `StreamInvalidated`), микрофон — по имени из `list_microphones`. `LevelMeter` — уровень без записи.
- `win::Recording`: звук открывается первым, все дорожки с тиком 0 в первом кадре видео; стоп сначала звуку, потом видео. Сбой до стопа запись не роняет: причина — в `RecordingSummary::problems`.
- Пауза (решение 007): `PauseClock` общий для дорожек, всё на шкале записи (QPC минус паузы), внутри паузы — отбрасывается. `manifest.pauses` — на QPC. `cancel` удаляет только файлы записи (`remove_recording`) и пустую папку.
- Папка записи (решение 007): одна `SharedManifest`, сегменты `<дорожка>-NNNNNN.mp4` (`video`, `system`, `mic`) от ключевого пакета, ≥ 2 с. `manifest.json` переписывается атомарно и перечисляет только целые сегменты.
- Финализация: `.tmp` → rename → manifest → удаление сегментов; можно повторять. Без видео — `EmptyRecording`. Удаляются только сегменты дорожек manifest и `*.tmp`. `recover` — та же финализация по manifest.
- Ввод (решение 009, `input/` — формат, `win/hooks.rs` — хуки): LL-хук на своём потоке только ставит время QPC и шлёт событие. `cursor.bin` — записи 24 байта, `keys.bin` — 12 байт; в manifest не входят. Позиции — пиксели от угла источника (`source_origin`). Курсор (решение 009): форма — по `OBJID_CURSOR` на потоке хуков; PNG и дедупликацию делает поток записи (`cursor-NNNN.png`).
- Клавиши (решение 009): `KeyFilter` вызывается в процедуре `WH_KEYBOARD_LL` до времени и очереди; пишутся только сочетания с Ctrl/Alt/Win и спецклавиши. Обычный ввод текста не пишется никогда и нигде не хранится.
- Телеметрия (`input::Telemetry::read`): события на source time (паузы уже вырезаны), позиции в пикселях. `cursor_track` — `dc_core::CursorTrack` в долях кадра (решение 015), `key_presses` — `dc_core::KeyPress`. `project::recorded` — телеметрия каждой записи дорожки один раз; `project::telemetry_tracks` — курсор и клавиши для `dc_ops::Evaluator`: одна функция для превью, экспорта и `devcast frame`.
- Проект (решение 007): `win::ProjectRecording` пишет в `media/recording-N/`; клип и при стопе, и при восстановлении добавляет одна `project::add_recording`. `unfinished_recordings` + `recover_recordings` — только под lock проекта. Дорожки и телеметрия в `Project` не перечисляются: их описывает manifest, папка записи — `project::recording_dir`.
- unsafe разрешён здесь с комментарием SAFETY. Поэтому здесь же Win32 окна HUD (`win::hud_window`): `WDA_EXCLUDEFROMCAPTURE`, место окна в физических пикселях (решение 020).

## Как тестировать

- `cargo nextest run -p dc-capture`.
- Живые тесты (`#[ignore]`) пишут экран: `DEVCAST_TEST_MONITOR=2 cargo nextest run -p dc-capture --run-ignored=all --no-capture`, затем открыть напечатанные PNG и удалить их вместе с записями. На ПК владельца записывать можно только монитор 2; `tests/input.rs` не запускать, пока владелец работает.
- Основные: `tests/live.rs`, `record.rs`, `sound.rs`, `sync.rs` (`DEVCAST_SYNC_SECONDS`), `recover.rs`, `pause.rs`, `project.rs`, `segments.rs`; помощники — `tests/common`. Фикстуры `fixtures/recordings/` пишет `examples/record_fixtures` (не тест).

## Куда не лезть

- Кодеки, их самопроверка и MP4 — `dc-encode`. Модель проекта — `dc-core`.

## Ссылки

- docs/roadmap/03-capture.md, docs/decisions/007-capture.md (WGC, пауза, проект), 008-audio.md, 009-input-hooks.md.
