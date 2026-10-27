# Курсор: выжимка из v1 (задача 0.9)

Агент, который делает курсор в v2, читает только этот файл. Код v1 открывать не нужно. Ссылки `файл:строка` даны относительно `.v1/` (в `src/` и `electron/`). Метки: **v1** — как сделано; **Рек.** — предложение для v2, а не факт v1; **не переносить** — баг или костыль v1, повторять нельзя (причина рядом). Эффекты клика и облачко описаны в `clicks-callout.md`, камера и слежение за курсором — в выжимке 0.10.

## 0. Обозначения

| Символ | Смысл | Единицы |
|---|---|---|
| `t` | время исходника клипа (source time), по нему ищутся сэмплы | мс, float |
| `cx, cy` | позиция курсора в долях захваченного источника (монитор или окно), ось Y вниз | 0..1 |
| `crop` | кроп клипа `{x, y, width, height}` в долях исходника | 0..1 |
| `R = (Rx, Ry, W, Rh)` | прямоугольник контента клипа в кадре **до** камеры (`maskRect`). В v2 фона нет, поэтому `R` — кадр проекта или вписанный в него клип с полями | px кадра |
| ref-px | пиксель при `W = 1920` | — |
| `size` | «Размер курсора» | × |
| `h` | базовая высота: `h = 28 · size · W / 1920` | px |
| `k_style` | множитель набора (§8.3) | × |
| `H` | высота картинки курсора без bounce: `H = h · k_style` | px |

- **v1:** слой курсора лежит **внутри контейнера камеры**. Это видно в превью (`src/components/video-editor/VideoPlayback.tsx:1891-1893`) и экспорте (`src/lib/exporter/modernFrameRenderer.ts:544`). Поэтому зум масштабирует курсор, эффекты клика и облачко вместе с видео: при зуме 2× курсор на экране вдвое больше. Маска видео курсор не обрезает.
- **v1:** в превью единицы сцены — CSS-px (`resolution = devicePixelRatio`), в экспорте — px вывода. Всё, что выражено через `W`, совпадает по пропорции (`src/components/video-editor/videoPlayback/cursorScale.test.ts`). Абсолютные константы в px не совпадают (§13).

## 1. Телеметрия v1 (как было)

Файл `<video>.cursor.json` рядом с видео (`electron/ipc/utils.ts:67-69`):
```json
{ "version": 2, "samples": [ { "timeMs": 1234.5, "cx": 0.41, "cy": 0.77, "interactionType": "move", "cursorType": "arrow" } ] }
```
- Константы: `CURSOR_TELEMETRY_VERSION = 2`, `CURSOR_SAMPLE_INTERVAL_MS = 33` (≈30 Гц), `MAX_CURSOR_SAMPLES = 108000` (1 ч при 30 Гц) — `electron/ipc/constants.ts:27-29`.
- `interactionType`: `move | click | double-click | right-click | middle-click | mouseup`. `cursorType`: `arrow | text | pointer | crosshair | open-hand | closed-hand | resize-ew | resize-ns | not-allowed` (`src/components/video-editor/types.ts:21-43`). Колесо не пишется. Поле `pressure` есть в типе, но при загрузке отбрасывается.
- **Позиция.** Опрос рекурсивным `setTimeout` раз в 33 мс с компенсацией дрейфа (`electron/ipc/cursor/telemetry.ts:299-328`). Если таймер отстал больше чем на интервал, база сбрасывается, догоняющей серии нет. Точка берётся из `screen.getCursorScreenPoint()` (DIP Electron). Она нормируется к границам монитора источника или к `selectedWindowBounds` для окна, затем clamp 0..1 (`telemetry.ts:167-212`).
- **Время:** `timeMs = Date.now() − start − паузы` (`telemetry.ts:147-165`). `start` ставится в IPC `set-recording-state(true)` (`electron/ipc/register/recording.ts:1873-1888`), а не по первому кадру видео: задержка старта видео не компенсируется. При паузе сэмплы позже момента паузы удаляются (`telemetry.ts:125-135`).
- **Клики.** На Windows/Linux — глобальный хук `uiohook-napi` (`mousedown`/`mouseup`, `electron/ipc/cursor/interaction.ts:236-333`). На macOS — строки `INTERACTION:mousedown:<1|2|3>` и `INTERACTION:mouseup` нативного монитора (`electron/ipc/cursor/monitor.ts:31-40`). Кнопка 2 даёт `right-click`, кнопка 3 — `middle-click`. Левый клик становится `double-click`, если прошлый левый был ≤ 350 мс назад и не дальше 0.04 в нормированных координатах (`interaction.ts:190-221`). `mouseup` — отдельный сэмпл.
- **Тип курсора на Windows.** `cursor-monitor.exe` раз в 50 мс вызывает `GetCursorInfo` и сравнивает `hCursor` с `LoadCursor(NULL, IDC_*)`. При смене печатает `STATE:<тип>` (`electron/native/cursor-monitor/src/main.cpp:25-56`). Таблица соответствий:
  - `IDC_ARROW` → `arrow`, `IDC_IBEAM` → `text`, `IDC_HAND` → `pointer`, `IDC_CROSS` → `crosshair`, `IDC_NO` → `not-allowed`;
  - `IDC_SIZEWE` → `resize-ew`, `IDC_SIZENS` → `resize-ns`;
  - `IDC_SIZEALL` → `open-hand` (сомнительно);
  - `IDC_WAIT` и `IDC_APPSTARTING` → `arrow`;
  - всё остальное → `arrow`, включая `IDC_SIZENWSE`, `IDC_SIZENESW` и собственные курсоры приложений.

  Если курсор скрыт (нет флага `CURSOR_SHOWING`), тип просто не меняется: позиция пишется дальше, и курсор рисуется.
- Тип попадает в сэмпл из «текущего» значения `currentCursorVisualType` (`telemetry.ts:241-259`). Смена типа видна со следующего 33-мс сэмпла.
- macOS (на этап 9): соответствия `NSCursor` → тип в `electron/native/NativeCursorMonitor.swift:10-29`:
  - `pointingHand`, `dragCopy`, `dragLink`, `contextualMenu` → `pointer`;
  - `resizeLeft`, `resizeRight`, `resizeLeftRight` → `resize-ew`;
  - `resizeUp`, `resizeDown`, `resizeUpDown` → `resize-ns`;
  - `iBeam`, `iBeamCursorForVerticalLayout` → `text`.

**Не переносить:**
- 30 Гц с линейной интерполяцией: в «точном движении» видна ломаная.
- Переполнение `MAX_CURSOR_SAMPLES` выкидывает **самые старые** сэмплы (`telemetry.ts:256-258`), и запись длиннее часа теряет начало.
- DIP и физические пиксели смешаны (`telemetry.ts:172-194`).
- Ноль времени телеметрии не привязан к PTS видео.

## 2. Что писать в v2 (Рек.)

Согласовано с ARCHITECTURE.md (WH_MOUSE_LL на своём потоке, `cursor.bin`, картинки по HCURSOR) и roadmap 3.10–3.11.
- Время каждого события — QPC, на тех же часах, что PTS кадров WGC. Хранить от PTS первого кадра клипа. Паузы вычитаются так же, как у видео.
- Позиция — каждое событие хука, без опроса. Хранить в физических px источника и в долях захваченной области (монитор, окно или область). Повторы без движения не писать.
- Кнопки: down и up раздельно, с номером кнопки. Двойной клик определять по системным `GetDoubleClickTime()` и `SM_CXDOUBLECLK`/`SM_CYDOUBLECLK`.
- Колесо: дельта по вертикали и горизонтали.
- Тип и картинка: на каждое изменение `hCursor` сравнить его со всеми стандартными `IDC_*` и записать флаг `CURSOR_SHOWING`. Картинку снять через `GetIconInfo` в PNG с хотспотом, кэшировать по HCURSOR, дедуплицировать по хэшу пикселей. В событии хранить id картинки.

## 3. Загрузка телеметрии (v1)

1. Валидация (`electron/ipc/cursor/telemetry.ts:32-81`):
   - не больше 108000 сэмплов;
   - `timeMs = max(0, timeMs)`, нечисло → 0;
   - `cx`, `cy` — clamp 0..1, нечисло → 0.5;
   - неизвестные `interactionType` и `cursorType` → `undefined`;
   - сортировка по `timeMs`.
2. В редакторе (`src/components/video-editor/timeline/zoomSuggestionUtils.ts:87-98, 114-126`): выкинуть не-конечные значения, отсортировать, `timeMs` — clamp `[0, длительность]`, `cx`, `cy` — clamp 0..1.
3. Эвристики (`zoomSuggestionUtils.ts:128-176`) ставят `text` или `closed-hand` в диапазоны времени, **только если у сэмпла нет `cursorType`**. На Windows и macOS тип есть у каждого сэмпла (по умолчанию `arrow`), поэтому эвристики не срабатывают. **Не переносить**: v2 пишет настоящий тип.

## 4. Целевая позиция в момент t (v1)

`interpolateCursorPosition` (`src/components/video-editor/videoPlayback/cursorRenderer.ts:753-791`):
- `t ≤ t₀` → первый сэмпл;
- `t ≥ t_last` → последний;
- иначе бинарным поиском найти соседей `a.t ≤ t < b.t` и линейно интерполировать `cx`, `cy`; если `b.t − a.t ≤ 0`, взять `a`.

В интерполяции участвуют все сэмплы, включая клики и `mouseup`.

Проекция в кроп (`videoPlayback/cursorViewport.ts:21-57`):
```
if crop.width <= 0 or crop.height <= 0: visible = false
u = (cx − crop.x) / crop.width;   v = (cy − crop.y) / crop.height
visible = −1e−6 ≤ u ≤ 1 + 1e−6  and  −1e−6 ≤ v ≤ 1 + 1e−6
px = Rx + u·W;   py = Ry + v·Rh
```
Если курсор вне кропа, не рисуются ни курсор, ни облачко, ни эффект клика, если сам клик тоже вне кропа (`cursorRenderer.ts:1442-1470`).

## 5. Сглаживание пружиной (v1)

Сглаживаются `u` и `v` — доли `R`. На каждую ось своя пружина с одинаковыми параметрами (`SmoothedCursorState`, `cursorRenderer.ts:1081-1165`).

### 5.1. Параметры из `smoothing` (`videoPlayback/motionSmoothing.ts:219-279`)
```
s = clamp(smoothing, 0, 2)
if s <= 0:      k=1000; c=100; m=1; restDelta=0.0001; restSpeed=0.001
elif s <= 0.5:  n=s/0.5;       k=(760−420n)·1.12; c=34+24n; m=0.85+0.55n; restDelta=0.0002; restSpeed=0.01
else:           n=(s−0.5)/1.5; k=(340−180n)·1.12; c=58+22n; m=1.35+0.45n; restDelta=0.0002; restSpeed=0.01
k *= mulK; c *= mulC; m *= mulM     // mul = clamp(x, 0.25, 3), нечисло → 1  (motionSmoothing.ts:29-45)
```
`restDelta` задан в долях `R`, `restSpeed` — в долях в секунду.

### 5.2. Шаг пружины (`motionSmoothing.ts:64-70, 90-217`)
```
fn step(st, target, dt_ms, cfg):
  dt = dt_ms не конечно или <= 0 ? 1000/60 : clamp(dt_ms, 1, 80)
  if !st.init or !finite(st.x): st = {x: target, v: 0, init: true}; return target
  if |target − st.x| <= restDelta and |st.v| <= restSpeed: st.x = target; st.v = 0; return target
  w0 = sqrt(k/m); z = c / (2·sqrt(k·m)); d = target − st.x; e1 = −st.v; T = dt/1000
  pos(τ) = target − e(τ), где e(τ) — ошибка, e(0) = d, e'(0) = e1:
    z < 1:  wd = w0·sqrt(1−z²);  e = exp(−z·w0·τ)·((e1 + z·w0·d)/wd · sin(wd·τ) + d·cos(wd·τ))
    z == 1: e = exp(−w0·τ)·(d + (e1 + w0·d)·τ)
    z > 1:  wd = w0·sqrt(z²−1); a = min(wd·τ, 300)
            e = exp(−z·w0·τ)·((e1 + z·w0·d)·sinh(a) + wd·d·cosh(a)) / wd
  cur = pos(T)
  if z >= 1 and ((st.x <= target and cur > target) or (st.x >= target and cur < target)):
      st.x = target; st.v = 0; return target            // защита от перелёта
  vel = (pos(T + 0.0001) − cur) / 0.0001                // единиц в секунду
  if |vel| <= restSpeed and |target − cur| <= restDelta: st.x = target; st.v = 0
  else: st.x = cur; st.v = vel
  return st.x
```
Обновление (`cursorRenderer.ts:1101-1134`): `dt = max(1, t − lastT)` в мс исходника. Цель — спроецированная позиция из §4.

### 5.3. Когда пружины нет и курсор ставится точно в цель (snap)
- Первый кадр, после `reset()` и при `smoothing ≤ 0` (`cursorRenderer.ts:1102-1120`).
- Перемотка назад (`t < lastT`).
- Флаг `freeze`. В превью он стоит при перемотке и при одноразовой отрисовке кадра на паузе (`VideoPlayback.tsx:2269-2275`), в экспорте всегда `false`.
- Скачок времени между отрисовками больше 100 мс (`CURSOR_TIME_DISCONTINUITY_MS`, `cursorRenderer.ts:128, 1500-1516`).

Последствия, все **не переносить**: v1 сглаживает от кадра к кадру, поэтому:
1. На паузе и после перемотки виден сырой курсор, при воспроизведении — отстающий, а экспорт даёт третий вариант.
2. На ускоренных участках источник прыгает больше чем на 100 мс за кадр. Это экспорт 30 fps при ускорении ≥ 4× или тик превью 60 Гц при ≥ 7×. Тогда сглаживание, sway и blur выключаются.
3. На паузе тик продолжается с `dt = max(1, 0) = 1` мс, и курсор медленно «доползает» до цели.

### 5.4. Значения для пресетов (посчитано по §5.1)

| Режим | smoothing | mulK / mulC / mulM | k | c | m | ζ | ω₀, рад/с |
|---|---|---|---|---|---|---|---|
| Focused (по умолчанию) | 0.67 | 1.35 / 0.79 / 1.29 | 483.2352 | 47.7897 | 1.8073 | 0.8086 | 16.352 |
| Smooth | 0.67 | 0.92 / 1.36 / 1.29 | 329.3158 | 82.2709 | 1.8073 | 1.6861 | 13.499 |
| Без множителей | 0.67 | 1 / 1 / 1 | 357.9520 | 60.4933 | 1.4010 | 1.3507 | 15.984 |

Focused недодемпфирован и даёт лёгкий перелёт. Smooth передемпфирован: сработает защита от перелёта.

### 5.5. Рек. для v2: траектория как функция времени
Посчитать траекторию для каждого клипа заранее: шагать по времени исходника с фиксированным `dt = 1000/240` мс от первого сэмпла (стартовое состояние = первый сэмпл, `v = 0`), цель — §4, шаг — §5.2. Результат кэшировать и брать линейной интерполяцией по `t`. Тогда превью, пауза, экспорт и любой fps совпадают, а резы и ускорения ничего не ломают: траектория живёт во времени исходника. Пересчитывать при смене настроек или телеметрии.

## 6. «Точное движение» (v1)

- Переключатель «Точное движение курсора» (`src/components/video-editor/SettingsPanel.tsx:3072-3089`): включение ставит `smoothing = 0`, выключение — `0.18`. Состояние переключателя равно `smoothing === 0`.
- При `smoothing = 0` курсор каждый кадр ставится в цель (§5.3), то есть идёт по линейной интерполяции сэмплов. Sway всё равно сглаживается своей пружиной (§10), blur работает.
- **Не переносить:** выключение даёт `0.18` вместо прежнего значения или 0.67 из пресета. Получается другая пружина (`k = 681.856 · mulK`, `c = 42.64 · mulC`, `m = 1.048 · mulM`), и UI теряет активный пресет. В v2 сглаживание — вкл/выкл; при «вкл» брать параметры пресета (открытый вопрос 1).
- Ползунка `smoothing` в UI v1 нет, есть только переключатель и пресеты. Множители пружины видны только в dev-сборке (`SettingsPanel.tsx:1254, 2596, 2714-2747`).

## 7. Размер (v1)

- Ползунок 0.5–10, шаг 0.05, формат `2.50×` (`SettingsPanel.tsx:3162-3170`). Clamp 0.5–10 при загрузке (`src/components/video-editor/projectPersistence.ts:1002-1004`).
- По умолчанию 2.5 из пресета Focused (`projectPersistence.ts:100`, `src/components/video-editor/cursorMotionPresets.ts:32-38`). Константа `DEFAULT_CURSOR_SIZE = 3.0` (`types.ts:171`) нигде не используется — это мёртвый код.
- Высота: `h = 28 · size · W / 1920` (`cursorRenderer.ts:147, 1444-1446`, `videoPlayback/cursorScale.ts:1-5`; `minViewportScale = 0`). Картинка рисуется высотой `H · bounceScale`, ширина `= высота · aspect`.
- Размер зависит только от ширины контента `W`, но не от высоты кадра. Зум камеры добавляет масштаб сверху (§0).
- Пример: 1080p, `W = 1920`, size 2.5 дают `h = 70` px. Для Windows 11 `H = 117.5`, для macOS `H = 120.2`.

## 8. Картинка по типу курсора (v1)

### 8.1. Выбор типа (`cursorRenderer.ts:830-868`)
```
type = cursorType последнего сэмпла с t_s ≤ t, у которого cursorType задан
       и interactionType ∉ {click, double-click, right-click, middle-click}
       // сэмплы-клики пропускаются, чтобы тип не дёргался при клике
если такого нет: cursorType последнего сэмпла с t_s ≤ t; если и его нет: "arrow"
если в наборе нет картинки для type: "arrow"
```

### 8.2. Отрисовка спрайта (`cursorRenderer.ts:1549-1622`)
- `anchor` = хотспот в долях картинки. Позиция `(px, py)`; поворот sway делается вокруг хотспота, bounce тоже масштабирует к хотспоту.
- Высота `H · bounceScale`. Прозрачность спрайта 0.95 (`dotAlpha`). `roundPixels = true`: позиция округляется до пикселя.
- Тень — второй спрайт с той же текстурой и тем же масштабом и поворотом: тонировка `#000000`, alpha 0.35, смещение `(0, +2)` px, `BlurFilter` blur 3, quality 4, padding 12 (`cursorRenderer.ts:131-137`). Это примерно CSS `drop-shadow(0 2px 3px rgba(0,0,0,.35))`.
- Все 9 типов одного набора рисуются **одной высотой `H`**. Для Windows 11 это сохраняет пропорции: у всех файлов общий бокс 32×32. Для macOS каждая картинка обрезана по альфе и растянута до `H`, поэтому I-beam и рука той же высоты, что стрелка.

### 8.3. Подготовка картинок и `k_style` (`src/components/video-editor/videoPlayback/uploadedCursorAssets.ts`, `cursorRenderer.ts:305-480`)
- **macOS:**
  1. Растеризовать с высотой не меньше 512 px: `scale = max(1, 512/naturalHeight)`.
  2. Хотспот = доли из таблицы §14 × размер холста.
  3. Обрезать по `alpha > 0` и пересчитать хотспот относительно обрезанной картинки (`cursorRenderer.ts:323-387, 409-480`).
- **Windows 11:** растеризуется так же, но **без обрезки** (`preserveCanvas`). Хотспот задан в единицах бокса 32 (`uploadedCursorAssets.ts:72-79`).
- Текстуры: линейная фильтрация и мипмапы (`cursorRenderer.ts:403-407`).
- `k_style` (`uploadedCursorAssets.ts:33-45, 121-132`):
  - macOS: `(851/958) / (386/746) = 1.716784`;
  - Windows 11: `32 / 19.0625 = 1.678689`;
  - своя картинка: 1.

  Замысел: **высота видимого тела стрелки без тени = `h`**. Для Windows 19.0625 — измеренная высота стрелки по альфе в боксе 32. Для macOS формула считает по необрезанным высотам (746 px, тело 386 px), а рисуется обрезанная картинка, поэтому в v2 надо проверить по эталону 0.16. **Рек.:** задать размер так, чтобы тело стрелки без тени было высотой `h`, и мерить это по каждому ассету.
- **Своя картинка** (`cursorRenderer.ts:203-221`):
  - одна картинка на все типы;
  - хотспот `(0, 0)` (левый верхний угол), без обрезки, `k_style = 1`;
  - тень как у наборов.

  Загрузка (`SettingsPanel.tsx:1231-1253`): `image/*`, не больше 1 МБ (`1024·1024` байт), иначе тост с ошибкой. В v1 картинка хранилась data-URL в `localStorage` (`devcast:uploaded-cursor`), а не в проекте. GIF через Pixi без плагина показывается статичным первым кадром. В v2 по SCOPE файл копируется в `assets/` проекта, а в roadmap 5.7 есть анимированный GIF.

## 9. Bounce при клике (v1)

Последний клик с `t_click ≤ t` (`cursorRenderer.ts:810-828, 1042-1061, 1530-1533`):
```
age = t − t_click;  D = bounceDuration (60–500 мс)
p = (age <= D) ? 1 − age/D : 0
bounceScale = max(0.72, 1 − sin(p·π) · 0.08 · bounce)        // bounce 0–5
```
- В начале и в конце масштаб 1, минимум `1 − 0.08·bounce` в момент `D/2`. При `bounce > 3.5` масштаб упирается в 0.72 и дно становится плоским.
- Bounce применяется к курсору и тени, но не к эффекту клика и облачку.
- Кликом считаются `click`, `double-click`, `right-click` и `middle-click`. Новый клик перезапускает bounce.
- UI: «Отскок при клике» 0–5, шаг 0.05, `2.00×`. «Скорость отскока» 60–500 мс, шаг 5, видна только в расширенном режиме (`SettingsPanel.tsx:3279-3301`).
- По умолчанию 2 и 350 мс. Clamp 0–5 и 60–500 (`projectPersistence.ts:1017-1028`, `cursorRenderer.ts:1307-1333`).

## 10. Sway — покачивание (v1)

Наклон по скорости отрисованного (уже сглаженного) курсора между кадрами (`cursorRenderer.ts:1687-1714`, `videoPlayback/cursorSway.ts:1-37`):
```
fn sway_target(dx, dy, dt_ms, sway):          // dx, dy — сдвиг px с прошлого кадра
  if sway <= 0: return 0
  d = hypot(dx, dy); if d < 0.01: return 0
  speed = d / (clampDelta(dt_ms)/1000)         // clampDelta из §5.2, px/с
  f = clamp(speed / 1400, 0, 1)
  bias = clamp((dx + 0.65·dy) / d, −1, 1)
  return bias · f · (π/18) · sway · 3          // радианы, плюс — по часовой (ось Y вниз)
target = (freeze или нет прошлой точки) ? 0 : sway_target(px − prevPx, py − prevPy, t − prevT, sway)
dt = (нет прошлого t или freeze) ? 1000/60 : max(1, t − prevT)
rot = step(swaySpring, target, dt, swayCfg); if |rot| < 0.0001 and target == 0: rot = 0
```
- `swayCfg` (`cursorRenderer.ts:870-889`):
  1. взять конфиг §5.1 от `clamp(smoothing·0.7 + 0.18, 0.15, 2)` с теми же множителями;
  2. затем `c ·= 0.9`, `m = max(0.55, m·0.8)`, `restDelta = 0.0005`, `restSpeed = 0.02`.
- Получается для Focused: `k = 487.0454`, `c = 42.7918`, `m = 1.4393`, ζ = 0.808. Для Smooth: `k = 331.9124`, `c = 73.6668`, `m = 1.4393`, ζ = 1.685.
- Максимальный угол `sway · 30°`: при 0.4 это 12°, при 2 — 60°.
- Хранимое значение 0–2, по умолчанию 0.4 (`types.ts:176`, clamp в `projectPersistence.ts:1121-1123`).
- **UI показывает значение, делённое на 2** (`cursorSway.ts:39-45`, `SettingsPanel.tsx:3302-3316`): ползунок 0–1, шаг 0.025, 0 подписан «Выкл», по умолчанию видно `0.20×`. Открытый вопрос 4.
- **Не переносить:** порог 1400 задан в px сцены. Превью меряет в CSS-px, экспорт в px вывода, и при экспорте шире превью наклон сильнее. **Рек.:** считать скорость в ref-px (`speed · 1920 / W`), а для v2 брать её из производной траектории §5.5.

## 11. Motion blur курсора (v1)

`cursorRenderer.ts:1716-1742`, `videoPlayback/zoomTransform.ts:6-8, 256-281`:
```
if freeze or blur <= 0 or нет прошлой точки: без размытия
dt_s = max(1, t − prevT) / 1000
fpsScale = (1 / max(1/240, dt_s)) / 60
vel = (px − prevPx, py − prevPy) · blur · fpsScale · 2      // CURSOR_DIRECTIONAL_BLUR_STRENGTH = 2
if |vel| < 0.01: без размытия
фильтр MotionBlurFilter(velocity = vel px, kernelSize = 13, offset = −|vel|/32)
```
- Длина смаза: `|vel| ≈ speed_px_s · blur / 30`. Это след движения за `blur · 33.3` мс, при значении по умолчанию 0.6 — за 20 мс. От fps не зависит, пока `dt ≥ 1/240` с.
- Шейдер `MotionBlurFilter` — из пакета `pixi-filters`, его исходников в `.v1` нет. Он усредняет 13 выборок текстуры вдоль `vel`, почти по центру текущей позиции.
- По умолчанию 0.6 (`types.ts:173`), clamp 0–2 (`projectPersistence.ts:1118`). **Ползунка в UI v1 нет**, в v2 он нужен.
- **Не переносить:** фильтр висит на всём контейнере курсора (`cursorRenderer.ts:1295`), поэтому смазываются и неподвижные кольца клика, и облачко. **Рек.:** размывать только спрайт и тень курсора. По roadmap 5.9 в v2 блюр многовыборочный: брать позиции траектории §5.5 на окне `blur · 33.3` мс вокруг `t`.

## 12. Loop — зацикливание (v1)

Включается, если `loopCursor` включён, сэмплов не меньше 2 и окно не пустое (`src/components/video-editor/hooks/useCursorTelemetry.ts:109-122`). Окно `[startMs, endMs]` в мс исходника — от первого до последнего видимого (не вырезанного) участка (`videoPlayback/cursorLoopTelemetry.ts:271-317`). Телеметрия переписывается (`cursorLoopTelemetry.ts:3-6, 145-265`):
```
FREEZE=670; STEPS=20; SETTLE=120; EPS=0.0015
end = round(endMs); start = clamp(round(startMs), 0, end); if end <= start: return samples
B = сэмплы с start ≤ t ≤ end; if B пусто: return samples
first = B[0]; maxF = end − first.t; if maxF <= 1: return B
F = min(FREEZE, maxF); motionEnd = end − F
S = min(SETTLE, max(0, F − 1)); ret = max(1, F − S)
srcEnd = max(first.t, tm)   // tm = t последнего сэмпла B, сдвинутого от предыдущего на hypot > EPS;
                            // если движения не было вовсе (или |B| = 1) — t последнего (первого) сэмпла B
srcDur = max(1, srcEnd − first.t); motDur = max(1, motionEnd − start)
type0 = первый cursorType в B, не на клике (иначе B[0].cursorType ?? "arrow")
out = [{...first, t: start, interactionType: none, cursorType: type0}]
for s in B:
  q = clamp((s.t − first.t)/srcDur, 0, 1); mt = round(start + motDur·q)
  if mt <= out.last.t: out.last = {...s, t: out.last.t} else out.push({...s, t: mt})
P = interpolate(out, motionEnd) ?? last(B); typeR = тип по §8.1 в out на motionEnd
for i in 0..=20: q = i/20; e = 1 − (1−q)^5                                // easeOutQuint
  out.push({t: round(motionEnd + ret·q), pos: P + (first − P)·e,
            interactionType: q>0 ? "move" : none, cursorType: q>=1 ? type0 : q<=0 ? typeR : none})
if S > 0: n = max(2, round(S/670·20)) /* = 4 */; for i in 1..=n:
  out.push({t: round(motionEnd + ret + S·i/n), pos: first, interactionType: "move", cursorType: type0})
```
- Итог: последние 670 мс курсор возвращается в стартовую точку (550 мс, easeOutQuint) и 120 мс стоит.
- Особенность: первый сэмпл в цикле `for` сразу затирает `out[0]` своими полями, и очищенные `interactionType` и `cursorType` теряются.
- **Не переносить как есть:** движение до `motionEnd` **растягивается или сжимается во времени**, и курсор с кликами расходится с видео. Если мышь замерла на 5-й секунде 10-секундного ролика, движение растянется почти вдвое. **Рек.:** время не трогать, заменить только последние 670 мс возвратом. В v2 считать окно по времени выхода проекта (открытый вопрос 5).
- UI: «Зациклить курсор», выключено по умолчанию, только в расширенном режиме (`SettingsPanel.tsx:3100-3107`).

## 13. Видимость, слои, абсолютные px (v1)

- Нет сэмплов (например, импортированное видео) — не рисуется ничего (`cursorRenderer.ts:1412-1431`).
- «Показать курсор» выключено: спрайта нет, но эффекты клика и облачко рисуются. Облачко тогда стоит в сырой, несглаженной позиции (`cursorRenderer.ts:1465-1528`).
- Порядок слоёв снизу вверх (`cursorRenderer.ts:1263-1271`): эффект клика, тени, спрайты курсора, плашка облачка, текст облачка.
- **Не переносить:** константы в абсолютных px не масштабируются с `W`: тень (2, 3, 12), порог скорости sway 1400, минимумы эффектов и облачка. Превью в CSS-px и экспорт в px вывода из-за этого расходятся. **Рек.:** все такие числа считать в ref-px, то есть умножать на `W/1920`.
- **Не переносить:** в v1 два пути отрисовки курсора — Pixi (`PixiCursorOverlay`) и Canvas2D (`drawCursorOnCanvas`, `cursorRenderer.ts:1781-1857`, без sway, blur и облачка), плюс атлас для нативного экспорта (`cursorRenderer.ts:690-747`). В v2 рендер один.

## 14. Ассеты наборов: точные пути и лицензия

Переносятся только macOS и Windows 11. Tahoe, Tahoe Inverted, Dot, Minimal (`figma`) и Win11 Concept исключены, а с ними `src/assets/cursors/tahoe/*` и `src/assets/cursors/custom/*`. В v1 по умолчанию стоял `"tahoe"` (`types.ts:73`): все «привычные» размеры эффектов и облачка v1 подобраны при `k_style = 1` (см. `clicks-callout.md`).

| Тип | macOS: файл в `.v1/src/assets/cursors/macos/` | хотспот (доли) | Windows 11: файл в `.v1/src/assets/cursors/windows11/` | хотспот (ед. из 32) |
|---|---|---|---|---|
| arrow | `pointer-1__34-24.svg` | 0.34, 0.24 | `arrow__31-22.svg` | 0, 0 |
| text | `ibeamstroke-1__50-50.svg` | 0.50, 0.50 | `text__50-50.svg` | 16, 16 |
| pointer | `pointinghand-1__39-26.svg` | 0.39, 0.26 | `pointer__54-35.svg` | 6, 0 |
| crosshair | `crosshair-1__50-50.svg` | 0.50, 0.50 | `cross__50-50.svg` | 16, 16 |
| open-hand | `openhand-1__50-50.svg` | 0.50, 0.50 | `grab__50-50.svg` | 16, 16 |
| closed-hand | `closedhand-1__50-50.svg` | 0.50, 0.50 | `grabbing__50-50.svg` | 16, 16 |
| resize-ew | `resizeeastwest-1__50-50.svg` | 0.50, 0.50 | `resize-horizontal__51-49.svg` | 11, 4 |
| resize-ns | `resizenorthsouth-1__50-50.svg` | 0.50, 0.50 | `resize-vertical__48-50.svg` | 4, 11 |
| not-allowed | `notallowed-1__23-0.svg` | 0.23, 0.00 | `block__50-50.svg` | 8, 8 |

Источник таблицы — `uploadedCursorAssets.ts:1-27, 85-118`.
- У macOS суффикс имени `__X-Y` — это хотспот в процентах, и код берёт именно его.
- У Windows 11 суффикс имени **не** хотспот: код берёт точки из `.cur`-файлов Windows. Имена не парсить.
- **macOS-файлы** — SVG-обёртки со встроенным base64-PNG: снятые растром системные курсоры Apple (стрелка 767.31×746).
- **Windows 11** — векторная перерисовка: бокс `viewBox 0 0 32 32`, заливка `#FFFFFE`, обводка `#000001` толщиной 1. Координаты снизу вверх через `translate(0 32) scale(1 -1)`, растеризатор должен учесть этот transform (resvg учитывает).
- **Хотспоты Windows не совпадают с геометрией SVG на 1–2 единицы.** Вершина стрелки — `(1.26, 1.43)` против `(0, 0)`. Центр resize-horizontal — `(12, 6)` против `(11, 4)`, resize-vertical — `(5, 12)` против `(4, 11)`, круг block — `(9, 9)` против `(8, 8)`. У text совпадает: `(16, 16)`. При `size = 2.5` на 1080p одна единица ≈ 3.7 px. **Рек.:** хотспот брать по геометрии SVG и проверить на эталоне 0.16.
- В папках лежат и неиспользуемые файлы: в `macos/` 43 файла, в `windows11/` 22. Из `windows11/` в v2 могут пригодиться `resize-diagonal-backslash__50-50.svg`, `resize-diagonal-slash__50-50.svg`, `move__51-51.svg`, `busy__50-50.svg`, `wait__50-50.svg`, если расширить типы (открытый вопрос 2).
- **Лицензия.** В v1 нет ни лицензии, ни атрибуции на курсоры: `THIRD_PARTY_NOTICES.md` упоминает только Voom. Картинки macOS — графика Apple из системы, Windows 11 — перерисовка дизайна Microsoft. Для внутреннего инструмента риск низкий, но при публичном репозитории (AGPL) распространять их сомнительно. Решение за владельцем (вопрос 3). Альтернатива: v2 и так снимает настоящие картинки системного курсора при записи (§2) — набор «Системный» без чужих ассетов в репозитории.

## 15. Сводка настроек v1

| Настройка (RU в v1) | Ключ | По умолчанию | Диапазон и UI |
|---|---|---|---|
| Показать курсор | `showCursor` | true | вкл/выкл (расширенный режим) |
| Стиль курсора | `cursorStyle` | `"tahoe"` (в v2 нет) | сетка карточек + «Upload cursor» |
| Размер курсора | `cursorSize` | 2.5 | 0.5–10, шаг 0.05 |
| Точное движение | `cursorSmoothing` | 0.67 | данные 0–2; UI: 0 ↔ 0.18 |
| Пружина (dev) | `cursorSpring{Stiffness,Damping,Mass}Multiplier` | 1.35 / 0.79 / 1.29 | 0.25–3, только dev |
| Размытие в движении | `cursorMotionBlur` | 0.6 | 0–2, в UI нет |
| Отскок при клике | `cursorClickBounce` | 2 | 0–5, шаг 0.05 |
| Скорость отскока | `cursorClickBounceDuration` | 350 мс | 60–500, шаг 5 (расширенный) |
| Покачивание | `cursorSway` | 0.4 | хранится 0–2, UI показывает /2 |
| Зациклить курсор | `loopCursor` | false | вкл/выкл (расширенный) |

- Пресеты движения (`cursorMotionPresets.ts:32-61`). Общее у обоих: `cursorSize 2.5`, `smoothing 0.67`, `mulM 1.29`, `bounce 2`, `bounceDuration 350`, `zoomSmoothness 0.5`. Различия:
  - Focused: `mulK 1.35`, `mulC 0.79`, зум 200/200 мс;
  - Smooth: `mulK 0.92`, `mulC 1.36`, зум 1522.575/1015.05 мс.
- Применение пресета (`SettingsPanel.tsx:1591-1602`) пишет длительности зума и все курсорные поля. Активный пресет определяется точным равенством 9 полей (`cursorMotionPresets.ts:63-84`). **Не переносить:** выбор пресета зума не должен менять размер курсора.
- Кнопка «Сбросить» группы курсора возвращает всё, включая эффекты клика и облачко (`SettingsPanel.tsx:1536-1565`).

## 16. Открытые вопросы владельцу

1. Сглаживание в v2 — только вкл/выкл. При «вкл» брать пружину из активного пресета Focused/Smooth (как в v1) или фиксированную Focused?
2. Типы курсора: SCOPE называет пять форм (стрелка, I-beam, рука, ресайз, «нельзя»), а в v1 их девять плюс есть ассеты для диагоналей, move и wait. Какой список?
3. Лицензия ассетов Apple и Microsoft при публичном репозитории: оставить, заменить своими или сделать набор «Системный» из снятых при записи картинок?
4. Sway: в UI показывать 0–2 (как в SCOPE) или 0–1, как в v1?
5. Loop: оставить растяжение времени v1 или (рекомендация) только возврат в последние 670 мс? Считать по времени выхода всего проекта?
6. Набор по умолчанию вместо Tahoe: Windows 11 на Windows и macOS на macOS?
7. Свою картинку рисовать с хотспотом (0, 0), как в v1, или дать задать хотспот?
