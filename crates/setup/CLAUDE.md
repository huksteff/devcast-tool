# dc-setup (`crates/setup`)

`DevCast-Setup.exe` — установщик с выбором папки поверх Velopack (решение 013, решение владельца 2026-11-04). Сам Velopack ставит только без вопросов в `%LOCALAPPDATA%\DevCast`; эта программа спрашивает «Установить» / «Другая папка…» / «Отмена» и запускает вшитый установщик Velopack с `--installto <папка>`. Обновления ставятся туда же.

## Инварианты

- Окон своих нет: системные диалоги Windows (`rfd`: сообщение с тремя кнопками и выбор папки), строки — `dc_ui_kit::strings::setup`. Консоли нет (`windows_subsystem`).
- Установщик Velopack вшит при сборке: `build.rs` копирует файл из `DEVCAST_VELOPACK_SETUP` в `OUT_DIR`; без переменной (check, тесты) вшито пусто, и программа говорит «соберите `cargo xtask package`». Собирает её только `cargo xtask package` после `vpk pack` (`xtask/src/package.rs`, `setup_window`) и кладёт как `DevCast-Setup.exe` рядом с `DevCast-win-Setup.exe`.
- Выбранная папка `X` → установка в `X\DevCast` (папка с именем DevCast — как есть, `app_folder`). Вшитый установщик пишется во временную папку и удаляется после.

## Как тестировать

- `cargo nextest run -p dc-setup`: папка установки и аргументы.
- Живой прогон — запустить собранный `DevCast-Setup.exe` (за владельцем). Проверено вручную 2026-11-04: `DevCast-win-Setup.exe --silent --installto <папка>` ставит туда, `Update.exe uninstall --silent` убирает всё.

## Ссылки

- docs/decisions/013-velopack.md, docs/roadmap/08-release.md (8.1).
