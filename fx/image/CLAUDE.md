# dc-fx-image (`fx/image`)

Вид аннотации «картинка»: файл из `assets/images/` проекта в боксе аннотации, анимированный GIF/WebP проигрывается (5.34).

## Инварианты

- Зависит от dc-core, dc-render, dc-fx-annotations и dc-ui-kit (панель). Файлов не читает: копирует и декодирует картинку `devcast image set` (dc-cli через `dc_media::import_picture`), сюда приходит готовый `PictureFile`.
- Поля — `ImageBody` в `Annotation::body`: `picture` (нет — новая аннотация ничего не рисует). `PictureFile`: `path` в проекте, `size` `[w, h]` px, `frames` — кадры анимации серией `[число, мс]` (пусто у неподвижной). Всё, что нужно кадру, записано при импорте: `evaluate` чистый. Поля — текст и списки, поэтому снимок ключа между двумя картинками (`Blend for Value`) берёт одну целиком; `Blend for ImageBody` — скачком в правом ключе.
- `ImageSet` — не команда реестра (нужен файл), как `ClipAdd`: меняет только аннотации `image`; с `at` — в ключе этого момента, без него — в аннотации и во всех ключах (`edit_body`); та же картинка — пустой `Diff`.
- `evaluate(body, placement, elapsed)` → `Option<Sprite>` (`None` без картинки): «contain» — картинка вписана в бокс с сохранением пропорций по центру (v1 `annotationRenderer.ts:599-615`, как DaVinci; растяжения нет), поворот бокса, `placement.opacity` (своей прозрачности нет, как v1). Кадр анимации — `PictureFile::frame_at(elapsed)`, по кругу; `elapsed` — время выхода от первого показанного момента (`dc_fx_annotations::shown_for`, тот же отсчёт, что у анимаций входа): ускорение не торопит, вырез не съедает. В кадр — `look.rs`: `impl dc_fx_annotations::look::Body`, `READ` для таблицы видов `dc_ops`.
- SVG не поддерживается: только растр (решение владельца 2026-11-02).
- Панель (`panel.rs`): `impl dc_fx_annotations::panel::Section for ImageBody` — файл и размер, «Заменить…» — `Request::Picture` (id и ключ `Edit`): диалог, копия и `ImageSet` — у редактора (`dc_ops::import_picture`). «Сбросить» нет. Регистрация вида в редакторе — `panel::KIND` (`dc_fx_annotations::panel::Entry`).

## Как тестировать

- `cargo nextest run -p dc-fx-image`: `tests.rs` (пусто по умолчанию, `ImageSet`, чужой вид и нет id, ключ меняет картинку скачком и снимок берёт её целиком, правка во всех ключах, undo/redo и файл), `evaluate_tests.rs` (вписывание в широкий и высокий бокс, серии кадров, кадр по времени и по кругу), `panel_tests.rs` (kittest панели: правки — шаги команд вида, «Сбросить», просьбы редактору).
- Сцена — `crates/ops/src/annotations_image_tests.rs`; эталоны `crates/ops/tests/golden/image.rs` (`image/fit`, `turned`; картинки `fixtures/images`). Импорт — `crates/media/src/picture_import_tests.rs`, команда — `crates/cli/src/image_tests.rs`.

## Куда не лезть

- Общая модель и `annot …` — `fx/annotations`. Декодирование и копирование — `dc-media`, загрузка кадров в GPU — `dc_encode::Sources`, спрайты — `dc-render`.

## Ссылки

- docs/porting/annotations.md §7; docs/decisions/000-porting-questions.md («Аннотации»); задачи 5.7, 5.34.
