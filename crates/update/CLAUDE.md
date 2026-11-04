# dc-update (`crates/update`)

Velopack: хуки установщика, позже обновления (8.2), данные для «Что нового» и анонсы (8.3), контакты обратной связи (8.4).

## Инварианты

- `handle_installer_hooks` — первая строка `main` у `devcast-app` (решение 013): во время хука установки или удаления процесс завершается внутри.
- Версия крейта `velopack` равна версии `vpk` в `xtask/src/package.rs` (тест `vpk_version_matches_the_crate`).
- Проверки обновлений нет, пока репозиторий приватный (8.2). Когда появятся — не мешают записи и экспорту.
- Пакет собирает `cargo xtask package`, не этот крейт.
- Контакты обратной связи — `feedback.json` в корне крейта (правится без кода, `name`/`value`/`url`, `repository`, авторы `authors` — имя и тег, как в v1), вшиты в программы (`feedback()`): файл рядом с exe не годится — папку установки Velopack заменяет при обновлении. Показывают «О программе» настроек (dc-app `settings_about.rs`) и `devcast app feedback`.
- «Что нового» — `news()`: разделы `## <версия>` вшитого `CHANGELOG.md` новее последней запущенной версии (`settings.json` `version_seen`) до текущей; «Не выпущено» не выпуск. Показывают поповер HUD при первом запуске новой версии (dc-app `hud/notices.rs`) и `devcast app changelog`. Анонсов (`announcements.json`) нет: репозиторий приватный (после 8.2).

## Как тестировать

- `cargo nextest run -p dc-update` — `feedback.json` читается, разбор CHANGELOG и версий; `cargo nextest run -p xtask` — аргументы `vpk pack`, значок, версия.
- Установку проверяет владелец: `Setup.exe` из `<папка сборки>/velopack/releases`.

## Куда не лезть

- Настройки и данные программы — вне папки установки (dc-settings), она заменяется при обновлении.

## Ссылки

- docs/decisions/013-velopack.md, 018; docs/roadmap/08-release.md (8.1–8.3).
