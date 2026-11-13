# xtask (`xtask`)

Служебные команды: `cargo xtask verify` (все проверки «готово») и `cargo xtask hook-stop` (Stop-хук Claude Code: `verify --quick` — без сборки и тестов, полную проверку агент запускает сам после порции правок). Ещё `golden update` (эталонные кадры), `deps` (FFmpeg, libclang, CMake, Vulkan, шрифты в `.deps/`, `src/native.rs`, `src/fonts.rs`) и `package` (установщик Velopack, `src/package.rs`; `--version` и `--channel` — `src/release.rs`, ими CI собирает беты; файлы `xtask/package/` (сторонние лицензии) копируются в пакет как есть, плюс лицензии FFmpeg и Vulkan из `.deps/`); справка — `cargo xtask help`.

## Инварианты

- Каждое сообщение об ошибке говорит, как её исправить.
- Вывод короткий: при успехе одна строка, при ошибке только детали упавших шагов (не больше 150 строк на шаг).
- Лимиты CLAUDE.md (`src/crates.rs`): крейт — 40 строк и 8000 символов, корень — 150 строк и 12000 символов, строка — 1000 символов; считаются символы, не байты.
- Журнал (`src/changelog.rs`, входит и в `--quick`): MR кладёт строки своим файлом `changelog/*.md` (группы `###` и строки `- `), `CHANGELOG.md` не правит; раздела «Не выпущено» в нём нет; у `version` из Cargo.toml есть раздел `## <версия>`; «Главное» — до 7 строк по 100 символов. `cargo xtask changelog` собирает раздел выпуска и удаляет файлы.
- Слои крейтов в `src/deps.rs` совпадают с картой в docs/ARCHITECTURE.md.
- Хук блокирует остановку один раз подряд; при повторной остановке показывает отчёт владельцу (не зацикливается).
- `schema_docs` (`src/schema_docs.rs`, без сборки, входит и в `--quick`): у каждого поля и варианта типа с `derive(JsonSchema)` есть `///` — из них собираются `devcast help` и JSON Schema. Тестовые типы проверяются так же (одно правило), поля с `serde(skip)`/`schemars(skip)` — нет. Разбор — `syn`.
- `seams` (`src/seams.rs`, текстовая, входит и в `--quick`): в `crates/app/src` вне тестов и комментариев `dc_fx_*` только в файлах регистрации (список `REGISTRATION`), а их `pub use dc_fx_*` ведёт только в модули швов (`lane`, `stage`, `panel`, `dialog`). Новый файл регистрации — только с решением в docs/ARCHITECTURE.md «Дорожка эффекта».
- `op_names` (`src/op_names.rs`, текстовая, входит и в `--quick`): в `fx/`, `crates/app/src` и `crates/ui-kit/src` вне тестов и комментариев нет `Step::new(` — команду называет её тип (`OpName::of::<ZoomSet>()`, `Step::of`); имя текстом остаётся строкам `devcast` и планам.

## Как тестировать

- `cargo nextest run -p xtask` — правила (deps, limits, crates, todo, seams, op_names, schema_docs, features, changed, hook) покрыты юнит-тестами.
- Живая проверка: внести нарушение каждого вида и убедиться, что `cargo xtask verify` его ловит и подсказывает исправление.

## Куда не лезть

- Логика продукта сюда не попадает.
