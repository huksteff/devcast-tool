# dc-project (`crates/project`)

Папка проекта .dcproj: project.json, session.json, history.jsonl, lock, атомарная запись, версии схемы, настройки по умолчанию.

Пока пустой. Наполняется в задачах 2.8–2.13 (см. docs/ROADMAP.md). Когда появится код, перепиши этот файл по шаблону `docs/templates/crate-CLAUDE.md`.

## Инварианты

- Запись только атомарная (tmp → fsync → rename)
- Писатель один: lock-файл
- Пути с кириллицей и пробелами работают
