---
name: devcast
description: Запись экрана и монтаж видеопроектов DevCast (*.dcproj) через CLI devcast — записать монитор, окно или область со звуком, собрать проект из готовых видео (mp4, mov, mkv, webm), вырезать паузы и участки, скорость, зумы и автозумы по кликам, субтитры и их перевод, клики и клавиши записи, кроп и формат кадра, разрезать и переставлять клипы, стиль проекта и настройки по умолчанию, одна правка в нескольких проектах, кадр и раскадровка в PNG, план из нескольких правок одним шагом, отмена, экспорт в MP4 и GIF. Используй, когда просят смонтировать, подрезать, приблизить, поправить или экспортировать запись DevCast.
---

# Монтаж через devcast

## Главное

- Перед любой правкой — `--dry-run`: команда покажет diff и ничего не изменит. Потом та же команда без него.
- Все времена в аргументах и в выводе — по **готовому видео** (после вырезов и скорости). После выреза времена дальше сдвигаются: смотри `devcast ls` заново, не считай в уме.
- Время: `12.5` или `1:02.5`. Участки: `10.2-12.5,20-21`.
- Id короткие: клипы `c1`, зумы `z1`. Бери их из `devcast ls`.
- Проект: стой в папке `*.dcproj` (или рядом с единственной), иначе `--project путь`. `--json`, `--dry-run`, `--project` ставятся в любом месте строки.
- Коды выхода: 0 готово, 1 не получилось (текст ошибки говорит, что делать), 2 неверная команда — смотри `devcast help <команда>`.
- Аргументы любой команды: `devcast help <команда>`; схемы всех команд и проекта: `devcast help --json` (большой, ≈ 6000 токенов — только если `help <команда>` не хватило).

## Записать экран

Запись создаёт новый проект `recording-N.dcproj` в текущей папке (или `-o урок.dcproj`).
```
devcast ls sources                                  # мониторы screen:N, окна window:<id>, микрофоны --mic "…"
devcast record --source screen:1 --duration 10 --dry-run
```
Без `--dry-run` команда пишет экран и ждёт: с `--duration 10` — 10 с записи, без него — до `devcast record stop` из другого вызова (запусти `devcast record --source screen:2` в фоне). Пока идёт запись: `devcast record pause`, `devcast record resume` (паузы вырезаются), `devcast record stop` (печатает проект, длительность, размер), `devcast record cancel` (удаляет проект записи). Источники: `screen:N`, `window:<id>`, `area:x,y,w,h` (пиксели рабочего стола); `--fps 30|60|120`, `--audio off` — без системного звука, `--mic "имя"` — с микрофоном.

## Проект из готовых видео

`devcast new демо.mp4 вступление.mov -o урок.dcproj` — новый проект, по клипу на файл (mp4, mov, mkv, webm; копии ложатся в `media/` проекта). `devcast clip add демо.mp4` — добавить видео клипами в конец дорожки (один шаг отмены). У таких клипов в `ls clips` стоит `kind=imported`: телеметрии курсора и кликов у них нет.

## Осмотреться

```
devcast info          # длительность, сохранён ли, сколько объектов
devcast ls clips      # клипы с временем по готовому видео
devcast ls zooms --from 10 --to 1:20 --fields scale,mode
devcast events clicks --from 10 --to 20   # клики: время, кнопка, x y в долях кадра (как фокус зума)
devcast events keys                       # сочетания клавиш: Ctrl+S, Alt+Tab, F5
devcast events activity                   # отрезки, где был ввод; промежутки между ними — простой
```

`ls` показывает по 50 строк и пишет, как получить следующие (`--offset`). Вырезанные объекты идут в конце с пометкой «вырезан». `events` вырезанное не показывает, только считает; у импортированных видео событий нет.

Паузы в речи: `devcast silences` (по умолчанию тише −30 dB от 0,5 с, дорожка микрофона; `--min 0.8`, `--db -35`, `--track system`, `--from/--to`). Вырезать паузу целиком нельзя: оставь по 0,15 с с краёв, иначе речь склеится вплотную. Пример: пауза `0:09.250–0:11.150` → `devcast cut 9.4-11`.

Речь: `devcast transcript` — реплики с id слов (`w2–w12`), `--words` — по слову; первый вызов распознаёт записи (на процессоре примерно вдвое быстрее реального времени, результат кэшируется). Оговорку или лишнюю фразу вырезай по словам: `devcast cut w5-w9 --dry-run`, затем без `--dry-run`; границы сами сдвигаются в тишину между словами. Id слов не меняются после вырезов. Время начала слова точно примерно до 0,2 с: проверяй результат по `transcript` после выреза.

## Рецепты

Вырезать участки (времена — до этого выреза, все в одной команде):
```
devcast cut 5-7,20-21.5 --dry-run
devcast cut 5-7,20-21.5
```
Ответ одной строкой: `вырез 0:05.000–0:07.000; вырез 0:20.000–0:21.500; клипы ~2 c1,c2; длительность 0:30.000→0:26.500` (`+` добавлено, `-` удалено, `~` изменено).

Ускорить или замедлить участок (0.25–16, `1` — сброс):
```
devcast speed 10-20 2
```

Зумы:
```
devcast zoom add --at 15 --duration 2 --scale 2 --mode fixed --x 0.3 --y 0.4
devcast zoom set z1 --scale 2 --from 3 --to 5
devcast zoom rm z1
devcast zoom motion --preset smooth --smoothness 0.7
```
`--mode follow` (по умолчанию) — камера идёт за курсором, `fixed` — стоит на точке `--x --y` (доли кадра 0..1). `zoom motion --auto false` — не ставить автозумы сразу после записи.

Автозумы по кликам (у записей DevCast; клики не дальше 2,5 с друг от друга — один зум, от 0,5 с до первого до 0,5 с после последнего, фокус — на главном клике):
```
devcast zoom suggest                      # предложения, ничего не меняет
devcast zoom add --at clicks --dry-run
devcast zoom add --at clicks              # все предложенные одним шагом отмены; можно --scale 2 --mode fixed
```
Где зум уже стоит, автозум не ставится. `devcast record` ставит автозумы сам сразу после стопа (настройка зума `auto`, по умолчанию включена): не нужны — `devcast undo`.

Облачко у курсора (id событий `co1`, `co2`; время — по готовому видео, печать и стирание идут с обычной скоростью и на ускоренных участках):
```
devcast callout style --text Смотри --gradient --glow 14
devcast callout add on --at 1                          # показать с текстом из style
devcast callout add text --at 3 --text Жми --duration 1.5  # напечатать за 1,5 с
devcast callout add off --at 8                         # стереть и скрыть
devcast callout set co2 --text Готово
devcast ls callouts
devcast callout rm co3
```
Текст с пробелами — в кавычках: `--text "Жми сюда"`, одна строка до 80 символов. Цвета — `#rrggbbaa` в кавычках (`--background "#14b8c4ff"`, `--colors "#14b8c4ff,#a855f7ff"` — 2–4 цвета градиента), размер `--size` 0.5–2, свечение `--glow` 0–40.

Аннотации (id `a1`, `a2`; виды text, shape, image, drawing, blur; время — по готовому видео, не короче 0,1 с; анимации входа и выхода — по готовому видео, по умолчанию 0,3 с, выход — вход наоборот; бокс `--rect x,y,w,h` — доли кадра до зума, по умолчанию 0.3×0.2 в центре; поворот в градусах по часовой). В кадре рисуются все виды.
```
devcast annot add text --at 2 --duration 3 --rect 0.1,0.1,0.4,0.1
devcast annot add shape --at 3 --track 1 --rotation -15   # дорожка выше — слой выше
devcast annot set a1 --x 0.2 --to 6 --rotation 5
devcast annot move a1 --at 4 --track 1                   # длина сохраняется, на новой дорожке — сверху
devcast annot layer a1 down                              # up, down, top, bottom внутри дорожки
devcast text set a1 --text Экспорт --font Montserrat --size 48 --radius 8
devcast text set a1 --italic --underline --align left    # выключить — --italic false
devcast shape set a2 --figure arrow --from 0,1 --to 1,0 --width 8 --color "#ef4444ff"   # острие в to
devcast shape set a2 --figure highlight --color "#facc15ff"  # подсветка: заливка 28 %, без обводки
devcast shape set a2 --figure rectangle --radius 12 --gradient --glow 14   # --colors — 2–4 цвета через запятую
devcast annot anim a2 --enter draw --enter-duration 0.8   # фигура прорисовывается по контуру
devcast annot anim a1 --enter slide_left --exit fade --exit-duration 0.5   # slide_left/right/top/bottom — с той стороны
devcast annot anim a1 --enter typewriter --enter-duration 1  # pop, fade, none; typewriter — у текста, draw — у фигур и рисунков
devcast annot key add a1 --at 5                          # ключ: запомнить вид аннотации в этот момент
devcast annot set a1 --at 7 --x 0.6 --rotation 20        # правка в ключе в 7 с; без --at — во всех ключах
devcast text set a1 --at 7 --size 64 --color "#fde047ff"
devcast annot key copy a1 --at 7 --to 7.5                # --into a3 — в аннотацию того же вида
devcast annot key move a1 --at 7.5 --to 8                # перенести ключ; ключ там заменяется
devcast annot key rm a1 --at 5
devcast annot add image --at 8 --duration 2 --rect 0.7,0.05,0.25,0.2
devcast image set a3 --file логотип.png               # PNG, JPEG, WebP, GIF; копия в assets/images/, --at — в ключе
devcast annot add drawing --at 9 --duration 2 --rect 0.1,0.5,0.4,0.4
devcast drawing add a4 --points 0.1,0.8,0.5,0.2,0.9,0.8 --color "#ef4444ff" --width 6   # пары x,y — доли бокса
devcast drawing add a4 --points 0,1,1,0    # две точки — прямая, одна — точка
devcast drawing add a4 --points 0.5,0,0.5,1 --tool eraser --width 20   # ластик стирает только штрихи рисунка
devcast drawing set a4 --stroke 2 --width 8   # без --stroke — цвет и толщина всех штрихов карандаша
devcast drawing set a4 --opacity 0.8
devcast drawing rm a4 3
devcast annot anim a4 --enter draw --enter-duration 1   # штрихи прорисовываются по порядку
devcast annot add blur --at 9 --duration 2 --rect 0.6,0.6,0.3,0.15
devcast blur set a5 --strength 30 --tint "#00000080"   # альфа тонировки: 00 — чистое размытие, ff — сплошная заливка
devcast blur set a5 --tint "#000000ff" --radius 0   # «цензура»: сплошной цвет, острые углы
devcast ls annots
devcast annot rm a2
```
Ключи: время ключа — момент внутри аннотации на готовом видео; между ключами положение, размер, поворот, размер шрифта, цвета и радиус меняются плавно (smoothstep), текст, шрифт и начертание — скачком на ключе; до первого ключа держится первый, после последнего — последний. Первая правка с `--at` позже начала сама ставит ключ в начало: получается движение от исходного вида. Ключи привязаны к кадрам записи: едут с `annot move`, не сдвигаются от вырезов, на ускоренном участке движение ускоряется вместе с видео. Без последнего ключа аннотация остаётся такой, как в нём.

Текст: `--text` (перевод строки — новая строка), `--font` Inter (по умолчанию), Montserrat, Roboto, PT Serif, JetBrains Mono; `--size` 12–128 (px при 1080p); `--bold`, `--italic`, `--underline`; `--align` left, center, right; `--color "#fde047ff"` — цвет букв, `--background "#000000b0"` — плашка (альфа 00 — без плашки), `--radius` 0–64 — её углы. Текст с пробелами — в кавычках: `--text "Нажми сюда"`.

Картинки: `annot add image` создаёт пустую аннотацию, `image set a3 --file логотип.png` кладёт в неё файл (PNG, JPEG, WebP, GIF; SVG пока нет — сохрани в PNG). Картинка вписывается в бокс с сохранением пропорций по центру; анимированный GIF и WebP проигрываются по кругу от появления аннотации по готовому видео. `--at 5` меняет картинку в ключе (скачком на ключе).

Рисунки: `annot add drawing` создаёт пустой рисунок, `drawing add a4 --points x1,y1,x2,y2,…` добавляет штрих — пары чисел через запятую, доли бокса 0..1 (бокс двигает, растягивает и поворачивает рисунок); две точки — прямая, одна — точка. `--tool` pencil (по умолчанию) или eraser — ластик стирает под собой более ранние штрихи этого рисунка, не видео; `--color` (только у карандаша, по умолчанию `#2563ebff`), `--width` 1–64 (px при 1080p, 4 у карандаша, 12 у ластика). `drawing set` с `--stroke n` меняет штрих n (с 1, по порядку), без него — цвет и толщину всех штрихов карандаша; `--opacity` 0–1 — прозрачность рисунка. `drawing rm a4 n` убирает штрих, `drawing clear a4` — все. С `--at` правка идёт в ключ: прозрачность меняется плавно, штрихи — скачком на ключе. Номера штрихов — `ls annots --json`.

Размытие: `annot add blur` размывает всё, что под ним: видео и аннотации нижних слоёв (курсор и субтитры остаются поверх); `blur set` — `--strength` 1–100 (σ в px при 1080p, по умолчанию 20), `--tint "#rrggbbaa"` — тонировка, альфа — её сила от 00 (чистое размытие, по умолчанию) до ff (сплошная «цензура»), `--radius` 0–540 — углы (по умолчанию 8). Поворот, зум, fade, pop и slide работают как у других видов; с `--at` сила, цвет и альфа тонировки и радиус меняются плавно между ключами.

Фигуры: `--figure` arrow (по умолчанию), rectangle, ellipse, circle, line, highlight; концы стрелки и линии `--from x,y --to x,y` — доли бокса 0..1 (сам бокс — `annot set --rect`, поворот бокса поворачивает фигуру); `--color` — стрелка, линия, обводка; `--width` 1–64 — толщина (px при 1080p, от размера бокса не зависит); `--outline false` — без обводки; `--fill "#2563eb40"` — заливка (альфа 00 — без заливки); `--radius` 0–540 — углы прямоугольника и подсветки; `--gradient` с `--colors` (2–4 через запятую), `--angle` (градусы) и `--cycle` (секунд на круг, 0 — градиент стоит); `--glow` 0–40; `--opacity` 0–1. Подсветка приходит с заливкой цветом `--color`, прозрачностью 28 % и без обводки. draw-in: контур рисуется по длине (прямоугольник — от левого верхнего угла по часовой, эллипс и круг — сверху по часовой, стрелка — древко, потом усы), заливка проявляется вместе с ним.

Показ нажатых клавиш (по умолчанию выключен; места top_left, top, top_right, bottom_left, bottom, bottom_right):
```
devcast keys --show --position bottom_right --size 1.5
```

Звук. Дорожки записи — `system` и `mic` (у импортированного видео — `audio-1`, `audio-2` …). Уровень дорожки: `--volume` 0–100 (%), `--gain` 1–10 (×; лимитер держит пики ниже −1 dBTP), `--mute`, `--normalize` (EBU R128 до −16 LUFS), `--denoise` (шумоподавление, запись не меняется); выключить — `--normalize false`. Без `--clip` — уровень по умолчанию для всех клипов; с `--clip` — свой у клипа: заданный параметр клипа больше не следует умолчанию, остальные следуют. Разрезанный клип сохраняет свои уровни в обеих половинах. Дополнительный звук (mp3, wav, aac, m4a, flac, ogg; id `au1`, `au2`) стоит по времени готового видео: вырезы и скорость его не сдвигают, играет в 1×.
```
devcast audio channel mic --volume 80 --denoise   # по умолчанию для всех клипов
devcast audio channel system --clip c2 --mute     # свой уровень клипа c2
devcast audio mute c1                             # весь клип; --off — вернуть звук
devcast audio reset c2                            # уровни клипа снова по умолчанию (--channel mic — одна дорожка)
devcast audio add музыка.mp3 --at 2 --duration 5  # копия в assets/audio/; --track 0–31, без него — первая свободная
devcast audio set au1 --volume 30 --normalize     # --at, --duration, --offset (секунда файла в начале), --gain, --mute
devcast ls audio
devcast audio rm au1
```

Кроп клипа и формат кадра:
```
devcast crop c2 0.1,0,0.8,1      # x,y,w,h в долях исходника; 0,0,1,1 снимает кроп
devcast aspect 16:9              # free, source, 16:9, 16:10, 4:3
```

Клипы:
```
devcast clip split --at 10       # разрезать как Blade: настройки и эффекты остаются на месте
devcast clip move c3 --to 1      # место на дорожке, с 1
devcast clip rm c1
```

Субтитры (id реплик `s1`, `s2`; время — по готовому видео):
- `devcast captions gen` — реплики из речи всех записей, заменяет все прежние; распознаёт, как `transcript` (кэш общий). `--tracks mic,system` — несколько дорожек, где речь перекрывается, главнее первая; `--language ru`, `--model accurate`.
- `devcast ls captions` — реплики с временем и текстом.
- `devcast captions set s3 --text "Новый текст"` — неизменённые слова сохраняют время; `--from 12.5 --to 15` — границы.
- `devcast captions split s3 --at 14.2` (в паузе между словами у этого момента), `devcast captions merge s3` (со следующей), `devcast captions rm s3`, `devcast captions clear`.
- Вид: `devcast captions style --size 36 --rows 2 --animation pop` — шрифт (Inter, Montserrat, Roboto, PT Serif, JetBrains Mono), размер в px при 1080p, `--color` сказанных слов, `--highlight` текущего, `--inactive` несказанных (#rrggbbaa), `--shadow`, `--bottom` и `--width` в % кадра, `--radius`, `--background` (прозрачность плашки 0–1), `--show false` — скрыть.
- Перевод: `devcast captions export -o субтитры.txt` → в файле строка `s3 текст` на реплику; переведи текст, id не трогай → `devcast captions import субтитры.txt`. Время реплик не меняется, реплики не из файла остаются как были.

## План: много правок одним шагом отмены

`plan.json` — массив шагов; `op` — имя команды, остальное — её аргументы (имена как в `devcast help <команда>`, без `--`). Шаги идут по порядку и видят результат предыдущих, поэтому вырезы ставь последними. Ошибка в любом шаге — не меняется ничего.

```json
[
  {"op": "zoom add", "at": 2, "scale": 2},
  {"op": "crop", "clip": "c2", "rect": "0.1,0,0.8,1"},
  {"op": "aspect", "aspect": "16:9"},
  {"op": "clip move", "id": "c2", "to": 1},
  {"op": "cut", "ranges": "0-1"}
]
```
```
devcast apply plan.json --dry-run
devcast apply plan.json
```

## Стиль, настройки по умолчанию, несколько проектов

`set` меняет настройки эффектов всего проекта одной записью истории: `настройка.параметр=значение`, параметр — аргумент команды этих настроек без `--` (`help set` покажет, какие команды: `zoom` — `zoom motion`, `captions` — `captions style`, `cursor`, `clicks`, `callout` — `callout style`, `keys`, `crop` — `aspect`). Значения проверяет та же команда.
```
devcast set zoom.preset=smooth captions.size=36 cursor.size=3 --dry-run
devcast set captions.rows=2 clicks.style=ripple crop.aspect=16:9
devcast defaults show                       # с чем начинаются новые проекты и записи: что не заводское
devcast defaults set captions.size=36       # как set, но для новых проектов
devcast defaults from-project               # настройки этого проекта — по умолчанию
devcast defaults reset                      # заводские
devcast style copy --from . --to ../*.dcproj   # стиль (не содержимое) в другие проекты
devcast batch ../*.dcproj -- set cursor.size=2 # одна команда в каждом проекте
```
Стиль — только настройки эффектов: зумы, реплики, аннотации и вырезы не копируются. У `style copy` и `batch` каждый проект получает свою запись истории; проект с ошибкой не останавливает остальные (код выхода 1, ошибка в stderr). В `batch` нельзя команды, которые пишут файл `-o` (export, frame, sheet, captions export), создают проект или сами перечисляют проекты.

## Посмотреть кадр

`devcast frame --at 12.5 -o кадр.png` — кадр готового видео со всеми эффектами, как в экспорте; `--grid` добавляет сетку через 0.1 с подписями — по ней читаются доли кадра для `--rect`, `--x --y` и точек рисунка (вне зума). `devcast sheet --from 10 --to 20 -n 6 -o раскадровка.png` — 6 кадров в одном PNG (начала шести равных частей отрезка; время кадров — в ответе). Открой PNG и посмотри глазами.

## Проверить и откатить

```
devcast ls clips                 # что получилось
devcast history                  # шаги, новые сверху
devcast undo                     # отменить последний шаг; undo 3 — три
devcast redo                     # вернуть отменённый
```
Если в редакторе остались несохранённые изменения, правки идут туда же, а `devcast save` переносит их в `project.json`.

## Экспорт

Формат — по расширению `-o` (`.mp4` или `.gif`). MP4: `--res 720p|1080p|1440p|4k|source` (по короткой стороне, по умолчанию 1080p), `--fps 30|60` (120 — только для записей 120; по умолчанию 60), `--quality low|medium|high` (high). GIF: `--res 720p|1080p|source`, `--fps 15|20|25|30` (15), `--loop on|off` (on).
```
devcast export -o урок.mp4 --res 720p --fps 30 --dry-run
devcast export status            # очередь: e1 идёт 42% (300/720 кадров), осталось ~5 с …
```
Без `--dry-run` команда ждёт, пока файл будет готов, и печатает путь, размер, кодек и время (1080p60 — примерно впятеро быстрее реального времени). Долгий экспорт запускай в фоне; несколько `devcast export` встают в одну очередь и идут по очереди. Пока идёт: `devcast export status`, `devcast export cancel e2` (без номера — отменить всё). Отменённый или упавший экспорт файла не оставляет. В файл попадает проект на момент вызова: монтировать дальше можно.
