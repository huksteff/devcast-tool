# dc-app (`crates/app`)

Бинарник devcast-app: HUD, окна записи, редактор, таймлайн, настройки. Логика в библиотеке (`run`), `main.rs` только печатает ошибку.

## Инварианты

- Один экземпляр на пользователя (`instance.rs`, канал dc-ipc `devcast-app-<пользователь>`). Журнал — `logging.rs` (`%APPDATA%\DevCast\logs`, `RUST_LOG`). Дампы падений — `crash.rs`, единственный `unsafe` dc-app (процесс `--crash-server` живёт, пока открыт его stdin).
- Распознавание речи — процесс `devcast --captions-worker` рядом с exe (решение 018): dc-app не линкует whisper и Vulkan (dc-transcribe запрещён `xtask verify`), отмена — убийство процесса (`recognition/job.rs`), протокол — `dc_ops::recognition`.
- GPU одно: `dc_render::Gpu` (`gpu.rs`) создаётся до eframe, eframe получает его клоны (`WgpuSetup::Existing`); бэкенд — `dc_render::BACKEND` (DX12, решение 004).
- Перерисовка только по событию (решение 002): безусловный `request_repaint()` запрещён, анимации — `request_repaint_after`. Потоки будят только главное окно (`request_repaint_of(ROOT)`), оно передаёт перерисовку открытым окнам (`app::show_windows`): `request_repaint_of` несуществующего окна открывает пустое окно egui. Кадр другого окна идёт через `app::other_window`.
- Настройки (`settings.rs`): один `%APPDATA%\DevCast\settings.json` и один `Store` на все окна (`Shared` за `Arc<Mutex>`); изменение пишется сразу и атомарно (`Store::update`), недостающие поля — умолчания.
- Окна кроме главного — deferred viewports с общим `Shared`. Окна без рамки строит `borderless::window` (без тени, решение 019); HUD, поповеры и отсчёт — прозрачные `borderless::floating` (решение 020), панель и тень рисует egui; непрозрачные окна заливают себя `app::opaque`.
- HUD (`hud/`) — главное окно (решение 001), поверх всех; исключение из захвата и место в физических px — `dc_capture::win::hud_window` (unsafe только там); место своё на каждом мониторе (`placement.rs`). Поповеры (`hud/popover.rs`) — одно окно за раз над HUD, исключены из захвата вместе с ним (`ContentProtected`); Esc или потеря полученного фокуса закрывают. Другие окна закрываются до старта записи.
- Загрузка модели (`downloads.rs`) — одна на модель на всю программу (`Shared::downloads`), все ждущие окна ждут её; закрытие последнего ждущего отменяет.
- Запись (`session.rs`, `recorder.rs`): покой → отсчёт → `Launch` (окна закрываются за кадр до старта) → старт → запись → подготовка/отмена → покой. `ProjectRecording` живёт на потоке `devcast-recorder`; UI только шлёт `Request` и берёт `Event`, сам не ждёт. После стопа поток ставит автозумы тем же путём, что `devcast record` (`dc_ops::autozoom::after_recording`), их ошибка — тост. Таймер — часы рекордера минус паузы. Пока висит «Отменить запись?», запись на паузе. Выход (`on_exit`) останавливает и сохраняет запись. Стоп трогает только запись и отсчёт; стоп во время старта запоминается (`Session::Starting::stop`) и уходит сразу после `Event::Started`. Проблемы записи — тосты.
- Трей (`tray.rs`) и глобальные горячие клавиши (`hotkeys.rs`) живут в `App` (главный поток) и применяются только при изменении; не зарегистрированные клавиши — `Shared::taken_hotkeys`. Значок рисуется кодом (`logo.rs`), у окон один `Arc`.
- После стопа (`Event::Stopped`) открывается редактор, HUD прячется; закрытие редактора показывает HUD. Ошибка подготовки — поповер `Kind::Failed`.
- Редактор (`editor/`, docs/design/editor.html): `editor_ui` — вся раскладка, возвращает `Outcome`. Проект — `Document` (`document.rs`): редактор — единственный писатель (lock), меняет проект только `dc_ops::apply` с `Registry::devcast()` — тот же проект, что у `devcast`.
- Кнопки и клавиши только ставят `Action` (`actions.rs`); кадр выполняет их после раскладки и перестраивает дорожки (`Tracks::of` по `TimeMap`). Жест — одна команда реестра и один шаг истории; отказ команды — тост, проект не меняется.
- Сохранение: правки несохранённы до Ctrl+S; `session.json` пишет `Autosave` из кадра, события истории идут в `history.jsonl` перед ним; закрытие окна и выход дописывают (`Document::flush`). Не записалось при закрытии — окно остаётся (`CancelClose`), ошибка — тост; следующее такое закрытие спрашивает «Закрыть без сохранения последних правок?» (`closing.rs`).
- Состояние окна без истории — `View` (выбор `selection.rs`, участок, жесты, вид таймлайна `timeline::Viewport`); `View::keep` после правки выбрасывает пропавшее. Время ↔ x — `TimeScale`.
- Таймлайн (`timeline/`): главная дорожка — команды fx/cut (`montage.rs`, время — секунды с шестью знаками, как `devcast`); перетаскивание регионов — команды `editor/regions.rs`, прилипание — `snap.rs`, строки — `rows.rs`. Волна — поток `devcast-peaks` (`waves.rs`, кэш в `cache/`), нажатия — поток `devcast-presses` (`presses.rs`). Неподвижная мышь ничего не просит (иначе вечная перерисовка).
- Инспектор (`inspector/`): правка — одна команда; ползунок и цвет — черновик до отпускания, поле числа — по Enter или потере фокуса, текст — до Enter или клика мимо (`rows::flush`). Ключи аннотаций — как в CLI: `--at` только когда указатель на ключе. Шрифт — `inspector/font.rs`: встроенные и шрифты проекта (`EditorWindow::fonts`), «Добавить шрифт…» — `Action::Font` (`dc_ops::import_font` + команда панели). Дорожки `audio-N` импортированного видео для панели звука клипа читает поток `devcast-streams` (`streams.rs`).
- Превью (`preview/`): кадр рисует поток `devcast-preview` путём `devcast frame` и экспорта (`Evaluator`, `Compositor::draw`) в текстуру общего устройства, берётся только новейший запрос. Правка на превью (`stage/`) — черновик `Document::draft` без истории, отпускание — одна команда.
- Воспроизведение (`playback/`): часы — сыгранное устройством звука (`dc_audio::Line::heard`), без звука — `Instant`; звук — поток `devcast-sound` с миксом экспорта.
- Клавиши редактора: таблица `keymap.rs` — единственный источник для обработки, справки, подсказок и настроек; `None` в `keys::act` — функция ещё не готова. Пока открыто поле ввода, кроп или вопрос закрытия, клавиши молчат; при справке работает только её клавиша.

## Как тестировать

- `cargo nextest run -p dc-app`: логика окон и снимки egui_kittest (`tests/snapshots`, тесты — `*_tests.rs` рядом с кодом). Редактор — `editor/tests.rs` (проект `editor/test_project.rs` во временной `Демо-запись.devcast`); правка = проект `devcast` тех же команд (`editor/editing_tests.rs::by_cli`), undo с выросшими счётчиками id — `editing_tests::undone`.
- Превью на WARP = кадр `devcast frame` побайтно (`preview/worker_tests.rs`); распознавание — поддельные процессы (`recognition/job_tests.rs`); замер потока превью — `preview/speed_tests.rs` (`--run-ignored only`).
- Живой прогон: `EGUI_INSPECTION=1 cargo run -p dc-app`, затем egui-mcp. Из Claude desktop (MSIX) сокет дампов не работает: запускать через `explorer.exe <путь к exe>`.

## Куда не лезть

- Захват и запись — dc-capture, файлы проекта — dc-project, виджеты, тема и все строки интерфейса — dc-ui-kit.

## Ссылки

- docs/roadmap/06-recording-ui.md, 07-editor.md; docs/decisions/001, 002, 004, 012, 018, 019, 020; docs/design/recording-windows.html, editor.html.
