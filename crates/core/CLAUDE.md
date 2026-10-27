# dc-core (`crates/core`)

Базовые типы: время (SourceTime/OutputTime), id, геометрия, Project, трейты Effect и Command, Diff, история, карта времени.

Пока пустой. Наполняется в задачах 2.1–2.7 (см. docs/ROADMAP.md). Когда появится код, перепиши этот файл по шаблону `docs/templates/crate-CLAUDE.md`.

## Инварианты

- Без IO, без GPU, без платформенного кода
- Ни от кого в workspace не зависит
- Время эффектов хранится в SourceTime, нарезка и скорость меняют только карту времени
