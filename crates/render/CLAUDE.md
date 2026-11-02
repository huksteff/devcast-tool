# dc-render (`crates/render`)

wgpu-компоновщик: видео, SDF-фигуры, blur/glow, текст, спрайты, кисть, motion blur, рендер без окна, эталоны, кадры для кодека.

## Инварианты

- Один рендер для превью, экспорта и CLI. Одно устройство `Gpu`: окно берёт его клоны, своего не создаёт. dc-app делает его через `Gpu::on` на instance из `instance_descriptor()` (решение 020).
- `Compositor::draw` получает готовую `Scene` и ничего не решает сам. Один render pass на кадр (`Recorder`), прерывают его только blur, glow, тень текста, кисть и слой повёрнутого текста. Пайплайны создаются один раз.
- `Scene` — данные без wgpu (serde, JSON-снимки evaluate), px кадра выхода. Кадр начинается чёрным, слои по `Band` снизу вверх, внутри полосы — в порядке `push`. Камера двигает все полосы, кроме `Overlay`.
- Видео: `Video::fitted` вписывает кроп клипа без растяжения, по центру. Кадр NV12 (`Nv12::new` проверяет плоскости) отдаётся через `Compositor::upload` до `draw`; кадр не того времени — `RenderError::NoFrame`. Текстуры кэшируются по `Media::path`. NV12 → RGB в шейдере по `VideoColor` кадра (из `dc_media::Probe::color`); совпадает с `ffmpeg scale` до ±2.
- `Rgba` (`#rrggbbaa`) имеет JSON Schema. `GradientColors` читается из `#..,#..` (`FromStr`). Общее для эффектов: `REFERENCE` (1080 — ref-px при короткой стороне кадра), `BLUE`, `DEFAULT_FAMILY`.
- Фигуры (`Layer::Shapes`): все фигуры кадра в одном storage buffer, слой — один instanced draw, камера применяется на CPU. Цвета градиента смешиваются в sRGB, сама фигура — в линейном свете; фазу `cycle` считает evaluate. `drawn` (draw-in, `shapes/trim.rs`) — доля длины пути, 1 по умолчанию (в JSON не пишется); длина дуги эллипса на CPU и в шейдере одинакова.
- Blur (`Layer::Blur`) и glow (`Shape::glow`) — раздельный гауссов в двух scratch-текстурах размера цели (`blur/`, решение 004). Тонировка смешивается по альфе цвета (0 — чистый blur, 255 — заливка). Камера масштабирует σ.
- Текст (`text/`, решение 005): glyphon, у каждого текстового слоя свой `TextRenderer`. База шрифтов пустая, шрифты — только из `Compositor::load_fonts` / `load_builtin_fonts` (папка `fonts` у exe, в тестах `.deps/fonts`; кладёт `cargo xtask deps`); нет семейства — `RenderError::NoFont`. Повторная загрузка папки добавляет только новые файлы. Шрифты, атрибуты, `families_in`/`file_families` — общий `text/fonts.rs`.
- Плашки и подчёркивание текста — фигуры шейп-прохода. Буквы с альфой 0 занимают место, но не дают чернил плашке и подчёркиванию. Текст выше верхнего края кадра ставится на целую строку, чтобы сдвиг на целые px давал те же пиксели.
- `Text::beside` (облачко у курсора): одна строка, бокс углом в `point + offset`, у края `rect` ближе `margin` (px кадра) — по другую сторону точки. `Text::shadow` (`text/shadow.rs`): тень размывается и накладывается как glow — над плашками, под буквами. Ширина строки на CPU — `TextMeasure` (решение 016): те же шрифты и разметка, что у рендера.
- Повёрнутый текст (`Text::rotation`, градусы по часовой вокруг центра `rect`; `turn/`, решение 017): рисуется прямо в прозрачный слой размера цели (`Recorder::retarget`) и ложится на кадр повёрнутым; больше цели — плитки (`turn::tiles`).
- Спрайты (`sprites/`): картинка по `ImageKey` (путь в проекте + кадр анимации). `Compositor::upload_image` берёт RGBA с прямой альфой (`Bitmap`), на GPU — premultiplied в линейном свете и mip. Кэш ограничен `IMAGE_BUDGET`; нет картинки — `RenderError::NoImage`.
- Кисть (`brush/`): lyon, свой слой размера цели, 4× MSAA, трафарет на штрих. Ластик стирает только штрихи этого рисунка. `drawn` — доля длины пути в px по порядку штрихов.
- Motion blur (`Motion`): камеры (`Scene::camera_motion`, только видео) и спрайта (`Sprite::motion`). Положение выборки считается от текущего (`step`); без движения кадр совпадает с чётким до байта.
- `AdapterChoice`: `Auto` — видеокарта, без неё программный адаптер; `Software` — только программный (эталоны CI). Нет никакого — `GpuError::NoAdapter`. Бэкенд один на ОС (`BACKEND`, решение 004): Windows — DX12, macOS — Metal, Linux — Vulkan, без переопределения. Компилятор DX12 — FXC (`Dx12Compiler::Fxc`), не DXC.
- Формат всех кадров и промежуточных текстур — `FORMAT` = `Rgba8Unorm`. Рисуют и читают через `SRGB_VIEW`: смешивание в линейном свете (решение 004).
- Эталоны (`golden.rs`, фича `golden`, решение 012): `fixtures/golden/<backend>/<имя>.png` (LFS) и `adapter.txt`. На том же адаптере и CPU — до байта, иначе ꟻLIP под `FLIP_MEAN_MAX`/`FLIP_P99_MAX`. Расхождение — в `target/golden/<backend>/`. Обновление — `cargo xtask golden update "<почему>"`.
- wgpu ровно той версии, что у egui-wgpu; задана в корневом `Cargo.toml`. Без async-рантайма: futures wgpu ждёт `block::block_on`.
- `Gpu::read` — плотный RGBA8 (`Image`). `Compositor::render` / `render_at` — сцена в память (в заданном размере). PNG и подача кадров проекта — `dc_encode::still`. `Nv12Readback` (экспорт, решение 006): цель → NV12 BT.709 limited на GPU, чтение через кольцо из 4 буферов.

## Как тестировать

- `cargo nextest run -p dc-render`. Тесты по проходам в `tests/` (`gpu`, `draw`, `video`, `video_color`, `shapes`, `blur`, `text*`, `sprites`, `brush`, `motion`, `nv12`) и `tests/golden.rs` — эталоны на программном адаптере. На CI без видеокарты идут на WARP. Шрифты для тестов ставит `cargo xtask deps`. `--no-capture` печатает выбранный адаптер.

## Куда не лезть

- Окно, surface и egui — `dc-app`. Декодирование — `dc-media`, кодирование — `dc-encode`.
- Что рисовать — `evaluate` эффектов (`fx/*`), здесь только рисование.

## Ссылки

- docs/decisions/004-overlays-wgsl.md, 012-testing.md; проба `spikes/render`. Задачи 5.1–5.12 в docs/roadmap/05-render.md; рендер эффектов — 5.13–5.36.
