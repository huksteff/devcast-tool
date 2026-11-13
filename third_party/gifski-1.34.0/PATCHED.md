# gifski 1.34.0 с правкой DevCast

Копия опубликованного пакета `gifski` 1.34.0 (crates.io), upstream commit `1060eab4500a20f27e2fa3ab7e85473d0e921cbd`. Лицензия — AGPL-3.0-or-later (`Cargo.toml`; отдельного текста лицензии в опубликованном пакете нет). Подключена через `[patch.crates-io]`, вне workspace, как остальные копии в third_party.

## Что изменено

`src/gifsicle.rs`: рекурсивный поиск кодового слова в lossy LZW (`Lookup::lossy_node` / `try_node`) заменён поиском в глубину с явной очередью посещений в Vec. Буфер переиспользуется между поисками, дети кладутся в обратном порядке, поэтому порядок посещений и разрешение равных ошибок цвета совпадают с upstream. Качество, квантование, палитра и формат LZW не меняются.

Причина: нативный stack overflow `0xc00000fd` в потоке `lzw0` при экспорте GIF с длинными одноцветными участками. Дамп тестовой сборки DevCast показал повторяющиеся `lossy_node` / `try_node`; тест с однотонным кадром 2560×1440 воспроизвёл тот же код падения. Стек потока не увеличивается, lossy-сжатие не выключается.

`src/gifsicle_tests.rs` сравнивает выбор узла, длину и суммарную ошибку с исходным рекурсивным порядком на разветвлённых деревьях, включая равные цвета, прозрачность и interlace. Запуск отдельно от workspace:

```powershell
cargo test --manifest-path third_party/gifski-1.34.0/Cargo.toml --lib gifsicle::tests --no-default-features --features gifsicle
```

Регрессия реального кодера и проверка декодированного кадра — `crates/encode/tests/gif_lossy.rs`, входит в обычный verify. Дифф upstream — `docs/upstream/gifski-1.34.0-iterative-lossy-lookup.patch`. Решение — [022](../../docs/decisions/022-gif-lossy-stack.md).

## Когда убрать

Когда upstream выпустит поиск без глубокой рекурсии: обновить зависимость, удалить копию и строку patch, сохранить тест в dc-encode. PR в upstream — только по отдельному поручению владельца.
