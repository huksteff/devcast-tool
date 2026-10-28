# DevCast v2

Запись и монтаж видео с экрана, внутренний инструмент студии. Rust, egui, wgpu, FFmpeg.
Код пишет и читает только нейронка. Владелец код не читает: проверяют компилятор, тесты и ты сам.

## Прочитай перед работой

1. [docs/PROCESS.md](docs/PROCESS.md) — **обязательно**: цикл задачи, проверки, правило «без костылей», коммиты.
2. Свою задачу в [docs/roadmap/](docs/ROADMAP.md) (файл текущего этапа).
3. `CLAUDE.md` крейта, в котором работаешь.
4. При необходимости:
   - [docs/SCOPE.md](docs/SCOPE.md) — что делаем и чего не делаем;
   - [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — стек, решения, крейты, CLI;
   - `docs/porting/*.md` — алгоритмы v1.

Больше ничего без нужды не читай: контекст дорогой.

## Команды

| Что | Команда |
|---|---|
| Все проверки «готово» | `cargo xtask verify` (тесты только изменённых крейтов и их зависимых) |
| Как в CI | `cargo xtask verify --full` |
| Быстрая проверка крейта | `cargo check -p <пакет>` |
| Тесты крейта | `cargo nextest run -p <пакет>` |
| Справка xtask | `cargo xtask help` |

Stop-хук (`.claude/settings.json`) сам запускает `cargo xtask hook-stop`. Если менялся код и verify красный, сессия не закончится: чини.

## Инструменты

- Rust — версия закреплена в `rust-toolchain.toml`, rustup ставит её сам.
- cargo-nextest: `cargo install cargo-nextest --locked` или готовый бинарник с https://get.nexte.st.
- Git LFS — для `fixtures/`.
- FFmpeg (GyanD 9.0.2 shared) и libclang: `cargo xtask deps` скачивает их в `.deps/` с проверкой SHA-256 и копирует DLL к exe. Пути прописаны в `.cargo/config.toml` (`[env]`). См. docs/decisions/003-video-decode.md.

## Где что лежит

```
crates/   базовые крейты: core, project, media, audio, render, encode, transcribe, capture,
          ops, ipc, ui-kit, update, cli (бинарник devcast), app (бинарник devcast-app)
fx/       эффекты, по крейту на эффект (пакеты dc-fx-*): cut, crop, zoom, cursor, callout,
          keys, captions, annotations, text, shape, image, drawing, blur, audio-tracks
xtask/    verify и служебные команды
docs/     план, процесс, архитектура, выжимки v1 (porting/), решения (decisions/)
fixtures/ тестовые записи и эталонные кадры (Git LFS)
.v1/      старое приложение v1 только для чтения (git worktree на ветку main, в git не попадает)
```

Пакеты называются `dc-<папка>` (`crates/core` → `dc-core`, `fx/zoom` → `dc-fx-zoom`).

## Слои зависимостей (проверяет verify)

`dc-core` → базовые крейты → эффекты `dc-fx-*` → `dc-ops` → `dc-cli` / `dc-app`.

- Зависеть можно только от нижних слоёв. Базовые крейты могут зависеть друг от друга.
- Эффект может зависеть только от `dc-core`, `dc-render` и `dc-ui-kit`. Виды аннотаций ещё от `dc-fx-annotations`.
- Новый крейт нужно добавить в `xtask/src/deps.rs` и в карту в ARCHITECTURE.md.

## Правила кода (коротко; полностью — PROCESS.md и ARCHITECTURE.md)

- Файл ≤ 400 строк, функция ≤ 80 строк.
- Нет `unwrap` и `expect` вне тестов. `#[allow(..., reason = "...")]` — только с настоящей причиной.
- `unsafe` только в `capture` и `encode`, с комментарием `SAFETY:`.
- Нет пометок о недоделках, закомментированного кода, временных обходов, `sleep` против гонок.
- Ошибки через `thiserror`, текст ошибки говорит, что делать.
- Код и комментарии — по-английски. Тексты для пользователя и документация — по-русски. Строки интерфейса лежат только в `dc-ui-kit`.
- Doc-тесты не используем: примеры пишем обычными тестами.
- Любое изменение проекта — команда из реестра (`Command`). Рендер — `evaluate` (CPU, чистая функция) + `draw` (GPU).
- Время эффектов — время исходника (`SourceTime`). Нарезка и скорость — только карта времени.

## Если не получается чисто

Остановись и напиши владельцу:
- что не так;
- 2–3 варианта (вырезать фичу, изменить её, другой подход);
- свою рекомендацию.

Не пиши обход. Не меняй SCOPE и архитектуру молча.

## Коммиты

Один коммит на задачу. Сообщение начинается с `add:`, `chg:`, `fix:` или `del:`, дальше кратко по-английски. Без упоминания Claude и `Co-Authored-By`. Никогда не пушить.

## v1

Код v1 читать только если выжимки в `docs/porting/` не отвечают на вопрос. Подключить его можно так: `git worktree add --detach .v1 main`.
