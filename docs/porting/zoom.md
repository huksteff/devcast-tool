# Зум: выжимка v1 (задача 0.10)

Источник — v1 в `.v1/` (ветка `main`). Ссылки `файл:строки` даны относительно `.v1/src/`. После этой выжимки код v1 читать не нужно.

Соглашения:
- время — **миллисекунды** (`ms`, float в расчётах, целые в данных);
- **[не переносить]** — баг, мёртвый код или артефакт v1; **[вопрос]** — решает владелец (§13);
- **[v2]** — рекомендация для v2, а не поведение v1;
- координаты фокуса `focus = {cx, cy}` нормированы 0..1 к **маске** — прямоугольнику видимого видео после кропа (§2.2).

Что берём (SCOPE «Зум»): рамка 1–5× (§3), режимы follow/fixed (§6), пресеты Focused/Smooth (§8), длительности, кривая и motion blur (§4, §7, §9), склейка соседних зумов (§4.3), автозумы и кнопка «Подобрать зумы» (§10).
Не берём: depth levels как данные, «Classic Animation», реакцию веб-камеры на зум (`webcam.reactToZoom`), dev-ползунки пружины камеры, `ZoomMotionBlurTuning` целиком.

## 1. Данные

`components/video-editor/types.ts:1-19`:
```ts
ZoomRegion { id; startMs; endMs; depth: 1..6; scale?: number; focus: {cx, cy}; mode?: "auto" | "manual" }
```
- `mode`: `"auto"` — **следовать за курсором**, `"manual"` — **фиксированная точка**. `undefined` ведёт себя как `"auto"` (проверка `mode !== "manual"`, `videoPlayback/sceneMotion.ts:96-100`). Подписи в UI: «Автоматически» / «Вручную».
- Масштаб: `getZoomScale(r) = isFinite(r.scale) ? clamp(r.scale, 1, 5) : ZOOM_DEPTH_SCALES[r.depth]` (`types.ts:1170-1173`).
- Legacy depth (`types.ts:1160-1167`): `1→1.25, 2→1.5, 3→1.8, 4→2.2, 5→3.5, 6→5.0`. Нужен только чтобы понять умолчания:
  - новый зум с таймлайна: depth 2 → **1.5×**, focus (0.5, 0.5), `mode:"auto"` (`hooks/useZoomRegionCommands.ts:72-84`);
  - автозум: `DEFAULT_AUTO_ZOOM_DEPTH = 2` → **1.5×**, `mode:"auto"` (`types.ts:1174`, `useZoomRegionCommands.ts:103-112`);
  - битые данные при загрузке: depth 3 → 1.8× (`projectPersistence.ts:449-451`).
- Нормализация при загрузке (`projectPersistence.ts:430-470`): `startMs = round`, `endMs ≥ startMs+1`, `scale` в [1, 5], `focus` в [0, 1] (NaN → 0.5).
- Длительность нового зума: `min(1000, totalMs)`. Ставится, только если в точке нет зума и до следующего зума и конца клипа есть ≥ этой длительности (`timeline/hooks/actions/useTimelineZoomActions.ts:40-75`). Минимальная длина элемента таймлайна 100 ms (`timeline/core/time.ts:55`).
- Перекрытие зумов UI не допускает: добавление, перетаскивание (`timeline/hooks/useTimelineDndBindings.ts:101-114`), смена скорости (`clipSpeedChange.ts:78-88`), автоподбор (§10). Рендер всё равно умеет перекрытия (§4.2).

**[v2]** `Zoom { id, start: SourceTime, end: SourceTime, scale: f32 = 1.5, focus: Vec2 = (0.5, 0.5), mode: Follow | Fixed = Follow }`. Без `depth`.

## 2. Время и координаты

### 2.1 Время
- В v1 зумы лежат во **времени таймлайна (выхода)**. При вырезе их пересчитывает `rippleRegions` (`clipSequence.ts:93-115`, §11.3), при смене скорости — масштабирует (`clipSpeedChange.ts:55-66`). Курсор семплируется во **времени исходника** (`cursorTimeMs`): `lib/exporter/modernFrameRenderer.ts:2988-2991`, `VideoPlayback.tsx:2167-2177`.
- Все длительности анимаций (§4) — ms **выхода**, от скорости клипа не зависят.
- **[v2]** Зум хранится в source time (ARCHITECTURE п.2). Перед расчётом его `[start, end]` переводится картой времени в отрезок выхода (правило — §11.3). Все формулы §4–§6 считаются во времени выхода `t`. Курсор для follow берётся в source time клипа, который виден в `t`.

### 2.2 Пространства
- **stage** — кадр вывода в пикселях `W×H`.
- **mask** `{x, y, width, height}` в px stage — где лежит видео после кропа (`videoPlayback/layoutUtils.ts:241-247`). Фон и рамка в v2 не нужны: mask — это кроп клипа, вписанный в кадр.
- `focus` нормирован к mask: `focusPx = mask.x + cx·mask.width`.
- Курсор в телеметрии нормирован ко **всему исходнику** (до кропа). В mask он переводится так: `((c.x − crop.x)/crop.w, (c.y − crop.y)/crop.h)` (`videoPlayback/cursorViewport.ts:21-57`).
- Под камерой лежат видео, курсор и аннотации. Камеру не трогают только оверлеи (субтитры, веб-камера) (`modernFrameRenderer.ts:534-562`).

## 3. Рамка на превью ↔ масштаб

Рамка показывается **только у выбранного зума в режиме `manual`**, а двигать её можно только на паузе (`videoPlayback/overlayUtils.ts:16-20, 64`, `VideoPlayback.tsx:1282-1288`).

Геометрия рамки (`overlayUtils.ts:37-63`):
```
Z = getZoomScale(r);  f = clampFocusToScale(r.focus, Z)
w = mask.width / Z;   h = mask.height / Z            // пропорции рамки = пропорции маски
left = clamp(mask.x + f.cx·mask.width − w/2, mask.x, mask.x + mask.width − w)
top  = clamp(mask.y + f.cy·mask.height − h/2, mask.y, mask.y + mask.height − h)
```
**Перенос рамки** (`VideoPlayback.tsx:1248-1280`). Клик или перетаскивание в любой точке оверлея ставит центр рамки под указатель, смещение захвата не сохраняется:
`focus = clampFocusToScale((clamp01((px − mask.x)/mask.w), clamp01((py − mask.y)/mask.h)), Z)`.

**Изменение размера за угол** (`videoPlayback/zoomResize.ts:6-15`, ручки `VideoPlayback.tsx:2597-2611`). Якорь — противоположный угол рамки, координаты нормированы к mask:
```
sx = corner ∈ {ne, se} ? +1 : −1;   sy = corner ∈ {sw, se} ? +1 : −1
maxSize = min(sx>0 ? 1−anchor.cx : anchor.cx,  sy>0 ? 1−anchor.cy : anchor.cy)
size    = clamp(((p.cx−anchor.cx)·sx + (p.cy−anchor.cy)·sy) / 2,  0.2, maxSize)
scale   = 1 / size                                  // size 0.2 → 5×, size 1 → 1×
focus   = (anchor.cx + sx·size/2, anchor.cy + sy·size/2)
```
Размер — среднее смещений по двум осям, поэтому рамка не искажается. Минимум 0.2 даёт предел 5×.
Если от якоря до края меньше 0.2 (`maxSize < 0.2`), `clamp` вернёт 0.2 и рамка вылезет за mask. **[не переносить]**: в v2 якорь сдвигать внутрь.

**[вопрос]** В v1 у follow-зума рамки нет, и масштаб follow-зума без переключения в manual не поменять. SCOPE говорит «рамка задаёт масштаб» и «режим у каждого зума». Предлагаю показывать рамку в обоих режимах: в follow она задаёт масштаб и стартовый фокус.

## 4. Цель камеры (target)

### 4.1 Сила региона `strength ∈ [0,1]` (`videoPlayback/zoomRegionUtils.ts:37-72`)
Константы (`videoPlayback/constants.ts:4-6`, `zoomRegionUtils.ts:11-14`):
`LEAD = 200`, `IN_OVERLAP = 1000`, `IN_WINDOW = 1015.05·1.5 = 1522.575`, `OUT_EARLY = 500`.
```
fn strength(r, t, dIn, dOut):          // dIn/dOut = max(1, настройка), §8
  a    = t − LEAD
  in0  = r.start + IN_OVERLAP − IN_WINDOW        // = r.start − 522.575, от dIn не зависит
  in1  = in0 + dIn
  out0 = r.end − OUT_EARLY
  if in1 > out0 { m = (in1 + out0)/2; in1 = m; out0 = m }
  out1 = out0 + dOut
  if a < in0 || a > out1:  return 0
  if a < in1:              return E((a − in0)/dIn)
  if a <= out0:            return 1
  return 1 − E(clamp01((a − out0)/dOut))
```
В единицах `t`: вход начинается в `start − 322.575`, выход — в `end − 300`.
- Focused (200/200): вход идёт `[start−322.575, start−122.575]`, выход `[end−300, end−100]`. Целевой зум успевает уйти раньше конца региона, остальное делает пружина (§5).
- Smooth (1522.575/1015.05): вход `[start−322.575, start+1200]`, выход `[end−300, end+715.05]`.

Короткий регион (`in1 > out0`; для Smooth это длительность < 1500 ms). Прогресс входа делится на полный `dIn`, поэтому в точке `m` сила **скачком** растёт до 1. **[не переносить]** В v2 брать пик `peak = E((m−in0)/dIn)` и выход считать как `peak·(1 − E(p))`.

### 4.2 Кривая `E` = `easeOutZoom` = `cubic-bezier(0.16, 1, 0.3, 1)` (`videoPlayback/mathUtils.ts:15-50, 61-63`)
Решатель: `x = clamp01(t)`. Сначала Ньютон, 8 итераций по `X(s) = 3·x1·(1−s)²·s + 3·x2·(1−s)·s² + s³` с выходом при `|X−x| < 1e-6` или `|X'| < 1e-6`. Потом бисекция, 10 итераций на [0,1] с тем же допуском. Ответ — `Y(s)` с `y1, y2`.
Контрольные значения: `E(0.05)=0.28106, E(0.1)=0.49439, E(0.2)=0.75213, E(0.3)=0.87717, E(0.5)=0.97178, E(0.7)=0.99568, E(0.9)=0.99988, E(1)=1`.
Это **единственная кривая**, которая реально работает в v1 (про `devcast/glide/…` см. §7).

### 4.3 Доминирующий регион и склейка (`zoomRegionUtils.ts:78-210`)
Включатель «Связать зумы» (`connectZooms`), по умолчанию **true** (`projectPersistence.ts:1078`).

Пары склейки: регионы сортируются по `start`, берутся соседние `A → B`, если `gap = B.start − A.end ≤ 1350`. Это `CHAINED_ZOOM_PAN_GAP_MS`; отрицательный gap (перекрытие) тоже склеивается. Для пары `handoff = A.end + 200`.
```
fn region_strength(r, t, pairs):
  if let p = pair where p.A == r:                  // r — уходящий
     if t >= p.handoff:            return 0
     if t >= r.end − 500 + 200:    return 1        // выход r подавлен
  if let p = pair where p.B == r:                  // r — входящий
     if t <  p.handoff:            return 0        // свой вход r подавлен
     if t <  r.end − 500 + 200:    return 1
  return strength(r, t, dIn, dOut)

fn dominant(regions, t):
  cand = regions.map(r -> (r, region_strength)).filter(s > 0)
  sort: по s по убыванию, при равенстве — у кого больше start
  return cand[0] или None                          // None → scale 1, focus (0.5,0.5), progress 0
```
- В момент `handoff` цель **мгновенно** перескакивает с A (сила 1) на B (сила 1). Плавный переезд даёт только пружина (§5); так задумано тестом `videoPlayback/zoomAnimation.test.ts:377-388`. Камера между склеенными зумами не отъезжает до 1×.
- Есть ещё `getConnectedRegionHold` (держит B на силе 1 в `[handoff+1000, B.start)`), но это дублирует ветку «входящий». **[не переносить]**
- Тип `ConnectedPanTransition` объявлен, но не заполняется никогда (`transition` всегда `null`). **[не переносить]**
- Без склейки перекрытые регионы соревнуются по силе: камера переключается, когда кривые пересекаются.
- **[v2]** Порог 1350 ms задан в коде. Настройки `connectedZoomGapMs` (1500) и `connectedZoomDurationMs` (1000) нигде не читаются. Перенести одну константу 1350.

### 4.4 Ограничение фокуса у краёв (`videoPlayback/focusUtils.ts:42-79`)
`clampFocusToScale(f, Z)`: сначала `clamp(f, 0, 1)`, потом каждая ось в `[1/(2Z), 1 − 1/(2Z)]`. Применяется к фокусу активного региона (`zoomRegionUtils.ts:74-76`) и к фокусу follow (§6). Хранимый фокус может быть «за краем» — ограничивается при использовании.

### 4.5 Целевое преобразование (`videoPlayback/zoomTransform.ts:421-452`)
Камера — `screen = s·p + (x, y)`, где `p` — точка stage до зума (px). Для `Z` активного региона, фокуса `f` и `k = strength`:
```
fx = mask.x + f.cx·mask.width;   fy = mask.y + f.cy·mask.height
s  = 1 + (Z − 1)·k
x  = (W/2 − fx·Z)·k;             y = (H/2 − fy·Z)·k
```
При `k=1` точка фокуса стоит в центре stage, при промежуточных `k` она линейно едет от своего места к центру. Если `W,H,mask.w,mask.h ≤ 0`, возвращается `{1, 0, 0}`.
Если mask меньше stage по какой-то оси (клип с полями), края маски при зуме могут попасть в кадр: ограничение §4.4 считается в долях маски. **[вопрос]** см. §13.

## 5. Сглаживание камеры: пружина

Цель `(s, x, y)` из §4.5 проходит через три независимые пружины: `scale`, `x` и `y` (в px stage). Код: preview `VideoPlayback.tsx:2186-2240`, export `modernFrameRenderer.ts:3169-3250`.

Решатель `stepSpringValue` (`videoPlayback/motionSmoothing.ts:135-217`) — аналитическое решение осциллятора за шаг, цель на шаге постоянна:
```
dt = clamp(deltaMs, 1, 80) ms (NaN/≤0 → 1000/60);  T = dt/1000 s
if !init: value = target; vel = 0; init = true; return       // первый кадр — сразу в цель
if |target−value| ≤ restDelta && |vel| ≤ restSpeed: value = target; vel = 0; return
ω = √(k/m);  ζ = c/(2√(k·m));  Δ = target − value;  v0 = −vel
ζ<1:  ωd = ω√(1−ζ²); x(T) = target − e^(−ζωT)·( (v0+ζωΔ)/ωd·sin(ωd·T) + Δ·cos(ωd·T) )
ζ=1:  x(T) = target − e^(−ωT)·( Δ + (v0+ωΔ)·T )
ζ>1:  ωd = ω√(ζ²−1); q = min(ωd·T, 300)
      x(T) = target − e^(−ζωT)·( (v0+ζωΔ)·sinh(q) + ωd·Δ·cosh(q) ) / ωd
if ζ ≥ 1 && x(T) перешагнул target: value = target; vel = 0; return   // защита от перелёта
vel = (x(T + 0.0001) − x(T)) / 0.0001          // ед./с
if |vel| ≤ restSpeed && |target − x(T)| ≤ restDelta: value = target; vel = 0
else value = x(T)
```
Параметры (`motionSmoothing.ts:281-319`, множители `motionSmoothing.ts:29-45`):
- `zoomSmoothness` ∈ [0,1], по умолчанию 0.5. В UI его нет, оба пресета держат 0.5. При 0 пружина «жёсткая»: `{k:1000, c:100, m:1, restDelta:1e-4, restSpeed:1e-3}`.
- Иначе `sc = 2·smoothness`, `k = 100/sc`, `c = 21`, `m = sc`, `restDelta = 0.0005`, `restSpeed = 0.015`.
- Множители камеры, каждый в [0.25, 3]: k×**1**, c×**1.13**, m×**1.12** (`projectPersistence.ts:1106-1114`). Есть только в dev-панели.
- **Итог по умолчанию: k = 100, c = 23.73, m = 1.12 → ω₀ = 9.449 рад/с, ζ = 1.1211** (слегка передемпфирована). Медленный полюс `ω₀(ζ−√(ζ²−1))` = 5.804 с⁻¹, то есть τ ≈ 0.172 с и примерно 0.8 с до 1 %.
- `dt` — шаг **времени контента**, а не настенных часов: в preview это разница timeline time между кадрами, в export — длительность кадра.

Предпросмотр (`sceneMotion.ts:24-60`):
- воспроизведение — пружина;
- seek или правка на паузе — «snap» прямо в цель со сбросом пружин;
- пауза без seek — держит последний кадр;
- склейка без разрыва (contiguous cut) пружины не сбрасывает (`VideoPlayback.tsx:2022-2054`).

**Расхождения v1 [не переносить]:**
- export всегда идёт с пружиной, а preview после seek показывает цель без отставания пружины: кадры не совпадают;
- путь `modernVideoExporter.ts:2349` берёт пружину **без** множителей (c = 21, m = 1);
- результат зависит от fps: цель семплируется раз в кадр.

### 5.1 «Classic Animation» (`zoomClassicMode`, по умолчанию false)
Пружины выключены (кадр = цель), и **follow тоже выключен** (`sceneMotion.ts:96-98`). Склейка в classic превращается в жёсткий скачок камеры (§4.3).

### 5.2 Рекомендация: одна модель = пружина, детерминированная
Classic не переносить:
- ломает склейку (скачок);
- отключает follow, который SCOPE требует у каждого зума;
- в v1 это скрытый флажок под «advanced».

Пружина должна уложиться в `evaluate(project, t)` как чистая функция:
1. **Трек камеры.** `camera_track(project)` симулирует цель (§4) и follow (§6) от `t=0` до конца выхода с **фиксированным шагом** (предложение: 1000/240 ms). Сохраняет `(s, x/W, y/H)`; трек кэшируется по ревизии проекта. `evaluate(t)` берёт значение линейной интерполяцией соседних отсчётов. Тогда preview, export и `devcast frame` совпадают при любом fps и после seek.
2. Пружину держать в нормированных единицах (`x/W`, `y/H`). Решение линейное, динамика от этого не меняется. Пороги для x/y брать порядка `1e-6` — это **[v2]** предложение, в v1 они были в px.
3. Начальное состояние — покой в цели при `t = 0`, как первый кадр v1.
4. **[v2]** Итоговое преобразование ограничивать так, чтобы кадр всегда был покрыт: `s ≥ 1`, `x ∈ [W − s·W, 0]`, `y` аналогично. В v1 такого ограничения нет.

## 6. Follow vs fixed (`videoPlayback/cursorFollowCamera.ts`)
- **Fixed** (`manual`): фокус = `clampFocusToScale(region.focus, Z)` и не меняется.
- **Follow** (`auto`, не classic, телеметрия не пуста): `computeCursorFollowFocus` (`cursorFollowCamera.ts:132-201`) с состоянием `{initialized, lastTimeMs, focus, wasZoomed, reachedFullZoom, frozenFocus}`. Курсор берётся линейной интерполяцией сырой телеметрии в source time (`videoPlayback/cursorRenderer.ts:753-791`), за краями — крайний семпл.
```
rf = clampFocusToScale(region.focus, Z)
if k < 0.01: if wasZoomed { wasZoomed = initialized = reachedFullZoom = false }; return rf
c = cursor(tSrc); if None: return initialized ? focus : rf
if k >= 0.99: reachedFullZoom = true
if reachedFullZoom && k < 0.99: return frozenFocus            // на выходе камера замирает
if !initialized || !wasZoomed || tSrc + 0.5 < lastTimeMs:     // старт или время пошло назад
   focus = frozen = rf; initialized = wasZoomed = true; lastTimeMs = tSrc; return rf
lastTimeMs = tSrc
half = 1/(2·max(1,Z)); inset = 2·half·clamp(0.25, 0, 0.49)    // safe zone = центральные 50 % видимого окна
по каждой оси: if c < focus − half + inset || c > focus + half − inset: focus_axis = c_axis
focus = clampFocusToScale(focus, Z); frozen = focus; return focus
```
- Коэффициент safe zone `SNAP_TO_EDGES_RATIO_AUTO = 0.25` (`cursorFollowCamera.ts:16`; MANUAL тоже 0.25, но не используется).
- Когда курсор выходит из зоны, камера **центрируется на курсоре** по этой оси. Комментарий в коде говорит «just enough», но это не так, проверено тестом `cursorFollowCamera.test.ts:35-76`. Цель прыгает, плавность даёт пружина §5.
- Фокус стартует из `region.focus`, а не с курсора (тест `:108-126`). Слежение идёт и во время входа. Если зум хоть раз дошёл до силы 0.99, на выходе фокус заморожен.
- Склейка A→B: сила остаётся 1, состояние не сбрасывается, и B продолжает слежение с фокуса A. `B.focus` при этом игнорируется.
- Состояние сбрасывается, только когда сила попадает в `(0, 0.01)`: при силе 0 функцию не вызывают (`sceneMotion.ts:90-92`). Если выход оборвался (конец видео, manual-зум между двумя follow-зумами), следующий зум получает старое состояние: замороженный фокус или старую точку. **[не переносить]** В v2 сбрасывать состояние явно, когда меняется активный регион. Исключение — склейка follow→follow, там состояние переносится.
- При кропе v1 сравнивает курсор в координатах исходника с фокусом в координатах маски. **[не переносить]** Курсор сначала проецировать в mask (§2.2).
- Состояние зависит от пройденного пути, поэтому в v2 follow симулируется внутри трека камеры (§5.2).

## 7. Кривые `devcast / glide / smooth / snappy / linear`
`ZoomTransitionEasing` (`types.ts:136`) и настройки по умолчанию: `zoomInEasing="devcast"`, `zoomOutEasing="devcast"`, `connectedZoomEasing="glide"` (`types.ts:203-205`). Строки локали: DevCast, «Плавно», «Мягко», «Резко», «Линейно».
**В v1 формул для них нет.** Значения сохраняются в проекте (`projectPersistence.ts:230-241`) и передаются в рендер, но `resolveSceneZoomTarget` их не принимает. Контролов в UI нет, реализации нет даже в истории git. Так же мертвы `zoomInOverlapMs` (500), `connectedZoomGapMs` (1500) и `connectedZoomDurationMs` (1000).
Реальное поведение такое: ramp `E` = cubic-bezier(0.16,1,0.3,1) на вход и выход (§4.2), поверх неё пружина. Это и есть «devcast». `linear` очевиден: `E(p) = p`.
Тоже мёртвые хелперы **[не переносить]**:
- `easeOutExpo` (`1 − 2^(−7t)`), `easeInOutCubic`, `easeOutCubic`, `smoothStep` (`mathUtils.ts:52-59, 65-86`);
- `softenFocusToScale`, `edgeSnapFocus`, `clampFocusToStage`, `stageFocusToVideoSpace` (`focusUtils.ts`), `computeFocusFromTransform`;
- `VIEWPORT_SCALE`, `ZOOM_*_DEADZONE`, `MIN_DELTA` (`constants.ts:7-10`).

**[вопрос]** Что в v2 значит «кривая плавности» из SCOPE — см. §13.

## 8. Пресеты Focused и Smooth (`components/video-editor/cursorMotionPresets.ts:32-61`)
| поле | Focused | Smooth |
|---|---|---|
| `zoomInDurationMs` | **200** | **1522.575** (= 1015.05·1.5) |
| `zoomOutDurationMs` | **200** | **1015.05** |
| `zoomSmoothness` | 0.5 (не применяется, §5) | 0.5 (не применяется) |
| `cursorSize` | 2.5 | 2.5 |
| `cursorSmoothing` | 0.67 | 0.67 |
| `cursorSpringStiffnessMultiplier` | 1.35 | 0.92 |
| `cursorSpringDampingMultiplier` | 0.79 | 1.36 |
| `cursorSpringMassMultiplier` | 1.29 | 1.29 |
| `cursorClickBounce` | 2 | 2 |
| `cursorClickBounceDuration` (ms) | 350 | 350 |

- Применение (`SettingsPanel.tsx:1591-1602`) ставит все поля, кроме `zoomSmoothness`.
- Активный пресет определяется точным равенством девяти полей (всех, кроме `zoomSmoothness`), иначе не подсвечен ни один (`cursorMotionPresets.ts:63-84`).
- Новый проект получает **Focused** (`projectPersistence.ts:100, 414-422`). Длительности ограничены [60, 4000] ms.
- Пружина курсора из этих чисел (формула `getCursorSpringConfig`, `motionSmoothing.ts:219-279`, подробно в выжимке курсора 0.9):
  - база при smoothing 0.67: k = 357.952, c = 60.493, m = 1.401;
  - Focused: k = 483.235, c = 47.790, m = 1.8073, ζ ≈ 0.809;
  - Smooth: k = 329.316, c = 82.271, m = 1.8073, ζ ≈ 1.686.
- Пресеты описаны в локали так: Focused — «Быстрая анимация для демо-роликов…», Smooth — «Мягкая анимация… в стиле Keynote».
- Сброс группы зума (`SettingsPanel.tsx:1523-1534`) возвращает длительности, множители камеры и classic = false.

## 9. Motion blur зума (`videoPlayback/zoomTransform.ts`)
Сила `zoomMotionBlur` ∈ [0, 2], по умолчанию **0.35** (`types.ts:178`, `projectPersistence.ts:371-377`). При 0 фильтры снимаются. Слайдера в v1 нет, значение приходит из проекта или пресета.

Размывается **только слой видео**, а курсор и аннотации нет (`modernFrameRenderer.ts:543-545`). Считается только в режиме `isPlaying`: в export это всегда, в preview на паузе blur сброшен.

Расчёт за кадр (`applyZoomTransform`, `zoomTransform.ts:482-577`) по **применённому** (после пружины) преобразованию:
```
первый кадр: запомнить transform, blur = 0
dt = clamp(now − last, 1, 80) ms;  fpsScale = (1 / max(1/240, dt/1000)) / 60     // нормировка к 60 fps
Q = четырёхугольник mask под transform: углы, центр, диагональ d, размеры
move = Q1.center − Q0.center (px);  sizeΔ = |ΔQ.size|
zoomVel = |ln(d1/d0)| / dt_s;   moveVel = |move| / dt_s
zoomActive = |sizeΔ| > 0.001 && zoomVel > zoomVelocityThreshold(0)
moveActive = |move|  > 0.001 && moveVel > panVelocityThreshold(0)
mode = zoomActive ? Zoom : moveActive ? Move : None      // любое изменение масштаба — радиальный blur
Move: v = move · amount · fpsScale · 4 · (maxDirectionalBlurPx/41.8)      // = 1 при умолчаниях
      если |v| < 0.01 → 0;  directional { velocity: v (px), kernelSize: 13, offset: −|v|/32 }
Zoom: strength = |1 − d1/d0| · amount · fpsScale · 4 · (maxRadialBlurStrength/1)
      radial { center: zoomCenter, strength, innerRadius: 0, radius: −1 (без границы), maxKernelSize: 13 }
```
- `zoomCenter` (`zoomTransform.ts:205-254`) — пересечение прямых, по которым двигались соответствующие углы. Углы, сдвинувшиеся меньше чем на 0.5 px, пропускаются. Если изменения нет — прежний центр, если прямые не пересеклись — центр Q1. Результат ограничивается `[−W, 2W]×[−H, 2H]`.
- Для масштаба со сдвигом без поворота это центр гомотетии: `c = (T1 − r·T0)/(1 − r)`, где `r = s1/s0`, а `T` — сдвиг `(x, y)`. **[v2]** Считать по этой формуле, при `|1 − r| < 1e-6` брать центр кадра.
- Множитель 4 (`SPATIAL_*_SHUTTER_MULTIPLIER`) имитирует длинную выдержку.
- Семантика фильтров pixi-filters 6.x. Исходника в репо нет, ниже по документации библиотеки:
  - directional — `kernelSize` выборок вдоль вектора `velocity` с небольшим сдвигом окна `offset`;
  - radial — выборки от пикселя к `center` на долю `strength` расстояния.

  Пиксельный паритет не нужен (SCOPE: эталон стиля). ARCHITECTURE: «несколько выборок вдоль скорости».
- **[v2]** Дельту брать из трека камеры: `C(t) − C(t − 1000/60)`, `fpsScale = 1`. По формуле это тот же v1-blur на 60 fps, но детерминированно, одинаково в preview и export и без условия `isPlaying`.

`ZoomMotionBlurTuning` (`types.ts:179-197`) — dev-только, в проект не сохраняется (`projectPersistence.ts:196-202`). **[не переносить]**, кроме чисел-коэффициентов выше. Используются только `panVelocityThreshold` 0, `zoomVelocityThreshold` 0, `maxDirectionalBlurPx` 41.8 и `maxRadialBlurStrength` 1 — все как отношения к умолчанию, то есть = 1. Мертвы `panResponsePerSecond` 11, `zoomResponsePerSecond` 9, `zoomSafeZoneRadiusPx` 6.

## 10. Автозумы (`timeline/zoomSuggestionUtils.ts`)

### 10.1 Нормализация телеметрии (`:87-179`)
Выкинуть неконечные значения, отсортировать по `timeMs`, `timeMs` ограничить [0, totalMs], `cx, cy` — [0, 1]. Остальная часть `normalizeCursorTelemetry` (простановка `cursorType` по эвристикам) относится к курсору, не к зуму.

### 10.2 Кандидаты из кликов (`detectInteractionCandidates`, `:227-298`)
Явные клики — семплы с `interactionType ∈ {click, double-click, right-click, middle-click}`. Для каждого `centerTimeMs = round(t)`, `focus = (cx, cy)` клика. Вид определяет `classifyPostClickBehavior` (`:458-529`):
1. `double-click` → **double-click-like**.
2. `mu` = первый `mouseup` с `0 < t_mu − t_click < 3000`. Если есть, `dur ≥ 200`, `|dx| > 0.03` и `|dx| > |dy|·1.8` → **text-selection**.
3. `moves` = семплы `move` или без типа в `(t_click+100, t_click+2000]`. Если их меньше 3 → **text-field-click**.
4. `max |s − click| < 0.02` → **text-field-click**.
5. `netDy = last.cy − click.cy > 0.03` и `Σ|dy| > Σ|dx|·1.5` → **dropdown-open**.
6. `Σ|dx| > 0.03` и `Σ|dx| > Σ|dy|·1.8` → **text-selection**.
7. Иначе **click-like**.

Сила по виду: double-click-like **1500**, text-selection **1300**, dropdown-open **1200**, text-field-click **1100**, остальное **900**.

### 10.3 Паузы курсора (dwell) — в v1 в зумы НЕ идут
`detectZoomDwellCandidates` (`:181-225`):
- серия — подряд идущие семплы, где каждый шаг `hypot ≤ 0.02` (`DWELL_MOVE_THRESHOLD`);
- серия из ≥ 2 семплов с длительностью в **[450, 2600] ms** даёт кандидата: `center = round((t0+t1)/2)`, `focus` = среднее, `strength` = длительность;
- виды: ≥1100 — text-focus-like, ≤800 — click-like, иначе dwell;
- синтетический двойной клик: две паузы обе ≤900, Δt ≤ 450, Δpos ≤ 0.035 → центр и фокус по середине, сила = сумма + 500.

Но `buildInteractionZoomSuggestions` оставляет только `source === "explicit"` (`:400-403`), то есть клики. Так было с первого коммита. **[вопрос]** SCOPE: «по паузам курсора и кластерам кликов» (§13).

### 10.4 Кластеры и регионы (`buildClickClusters :307-365`, `buildInteractionZoomSuggestions :367-447`)
```
sort clicks by centerTimeMs
кластер продолжается, пока click.t − cluster.last ≤ 2500        // CLICK_CLUSTER_MERGE_GAP_MS
focus кластера = focus клика с max strength (строго >, при равенстве — более ранний)
region = [max(0, first − 500), min(totalMs, last + 500)]        // CLICK_CLUSTER_PAD_MS
пропустить, если end ≤ start или пересекается с reserved (уже стоящие зумы и принятые предложения)
иначе добавить в reserved и в результат; результат отсортировать по start
```
- Один клик даёт зум 1000 ms. Масштаб всегда **1.5×**, `mode:"auto"` (§1).
- Параметры `defaultDurationMs` и `spacingMs` не используются.
- Статусы: `no-telemetry` (нет семплов или один семпл не-клик), `no-interactions` (кликов нет), `no-slots` (всё пересеклось или `totalMs ≤ 0`), `ok`.
- Следствие: соседние кластеры разделены > 2500 ms, после отступов между регионами > 1500 ms. Это больше порога склейки 1350, поэтому автозумы из разных кластеров **не склеиваются**.
- **[v2]** Фокус клика перед записью в зум проецировать в mask (кроп, §2.2). В v1 этого нет.

### 10.5 Кнопка «Подобрать зумы» (`useTimelineZoomActions.ts:110-188`)
Нужна телеметрия ≥ 2 семплов. `reservedSpans` — все существующие зумы. Подписи результатов: «Добавлено зумов: N» и т.п.
- В v1 кнопке передаётся source-время курсора, а регионам — время таймлайна. После вырезов или перестановки клипов зумы съезжают. **[не переносить]** В v2 всё в source time клипа.
- В v1 кнопка заблокирована, если выключен «Автозум для новых записей» (`layout/EditorShell.tsx:492`), с неверным сообщением про loop курсора. **[не переносить]** В SCOPE кнопка независима.

### 10.6 Автоприменение к свежей записи
Настройка `autoApplyFreshRecordingAutoZooms`, по умолчанию **true** (`editorPreferences.ts:185`). UI: «Автозум для новых записей».
- Открытие новой записи (`project/useInitialEditorSource.ts:100-101`, `hooks/useFreshRecordingAutoZoom.ts:52-112`). Запись помечается как ожидающая. Подбор запускается, если превью готово, `duration > 0`, **зумов ещё нет**, телеметрии ≥ 2 семплов. Затем дебаунс **500 ms**, и подбор идёт один раз на путь записи.
- Добавление записи из библиотеки на таймлайн (`library/useRecordingLibrary.ts:156-180`). Телеметрия фильтруется по source-диапазону клипа, время сдвигается к началу клипа, `reservedSpans = []`. Регионы сдвигаются на позицию клипа в последовательности.
- **Условие формата** `shouldAutoApplyFreshRecordingZoomsForSource` (`zoomSuggestionUtils.ts:43-68`):
  - на `win32` всегда true (окно на Windows бывает портретным, а клики уже нормированы к нему);
  - если размеры неконечные или ≤ 0 → true;
  - иначе `w/h ≥ 1.2`.

  При false подбор для свежей записи не запускается.
  Если подбор уже прошёл, а условие false, v1 удаляет **все** зумы с `mode:"auto"`, включая добавленные вручную (`useFreshRecordingAutoZoom.ts:130-160`). **[не переносить]**

## 11. Граничные случаи
1. **Края кадра.** Фокус ограничивается `[1/(2Z), 1−1/(2Z)]` в долях mask (§4.4). При Z=1 диапазон [0.5, 0.5]: фокус всегда в центре. Follow упирается в ту же границу (тест `cursorFollowCamera.test.ts:78-106`: курсор (1,1) при Z=2 даёт фокус (0.75, 0.75)).
2. **Перекрытие зумов.** UI их не создаёт (§1). Рендер:
   - без склейки побеждает max strength, при равенстве — более поздний `start`;
   - со склейкой перекрытие считается парой с отрицательным gap: B не стартует до `A.end+200`, затем цель скачком переходит на B.
   - Если B кончается раньше `A.end+200`, B сразу в фазе выхода. Странно, но в UI недостижимо.
3. **Зум через вырез** (`clipSequence.ts:93-115`):
   - v1 после выреза берёт сохранившиеся куски региона на новом таймлайне и ставит `[min start, max end]`;
   - регион, полностью попавший в вырезанное, удаляется;
   - при перестановке клипов отрезок может охватить чужие клипы.

   На склейке без разрыва камера в preview не сбрасывается. Для follow время курсора прыгает: вперёд — обычное переслеживание, назад (> 0.5 ms) — повторная инициализация на `region.focus`.
   **[v2]** Видимые куски зума (source → output) объединять в один отрезок выхода `[min, max]` и анимировать по нему (§4.1). Полностью вырезанный зум не рендерить, но и не удалять из данных (source time).
4. **Начало видео.** Первый кадр пружины ставится сразу в цель.
   - Зум со `start = 0` при Focused уже в силе 1: `a = −200 > in1 = −322.575`.
   - При Smooth: `p = 322.575/1522.575 = 0.2119`, `E = 0.772`, то есть кадр 0 уже на 77 % зума.
   - **[v2]** Так же, для этого `t < 0` не обрабатывается отдельно.
5. **Конец видео.** Выход начинается в `end − 300`. При Smooth целевой выход кончается в `end + 715.05`: если зум стоит до конца видео, ролик обрывается на середине отдаления. При Focused цель на 1× к `end − 100`, но пружина ещё ~0.8 с догоняет. v1 это не исправляет. **[вопрос]** нужен ли принудительный выход к концу.
6. **Кроп.** Фокус живёт в долях кропнутой области. Если поменять кроп, зум сместится относительно содержимого. Follow и автоподбор в v1 кроп игнорируют (§6, §10.4).
7. **Скорость клипа.** Длительности анимаций в ms выхода не масштабируются; в v1 масштабируется только сам регион (`clipSpeedChange.ts:55-66`).
8. **Нулевые размеры stage или mask** → `{s:1, x:0, y:0}`, blur сброшен (`zoomTransform.ts:499-509`).
9. **Нет телеметрии** у follow-зума → он ведёт себя как fixed (`sceneMotion.ts:99-100`). Курсор вне диапазона телеметрии → крайний семпл.

## 12. Сводка «не переносить»
- `depth` и `ZOOM_DEPTH_SCALES` как данные;
- `zoomClassicMode`;
- мёртвые настройки easing/overlap/gap/connectedDuration (§7);
- `ZoomMotionBlurTuning`;
- dev-множители пружины камеры (зашить итоговые k=100, c=23.73, m=1.12);
- `getConnectedRegionHold`, `ConnectedPanTransition`;
- мёртвые хелперы фокуса и кривых;
- скачок силы у коротких регионов (§4.1);
- рамка, вылезающая за край (§3);
- несброс follow-состояния (§6) и игнорирование кропа follow-ом и автоподбором;
- разные пружины в разных путях экспорта и snap-кадр preview vs пружина в export (§5);
- удаление всех `auto`-зумов по условию формата и блокировка кнопки подбора настройкой (§10).

## 13. Вопросы владельцу
1. **Кривая плавности.** В v1 у `devcast/glide/smooth/snappy/linear` нет формул, работает только cubic-bezier(0.16,1,0.3,1) плюс пружина. Варианты:
   - (а) оставить «DevCast» (эта кривая) и «Линейно», остальные не делать;
   - (б) заменить выбор кривой ползунком «Плавность камеры» (`zoomSmoothness` 0–1, §5);
   - (в) задать новые кривые с нуля.

   Нужен выбор.
2. **Автозумы по паузам.** v1 ставит зумы только по кликам (§10.3). Включать паузы 450–2600 ms в кластеры как вторичный источник или оставить только клики?
3. **Условие формата ≥ 1.2.** На Windows в v1 оно фактически выключено. Убрать его совсем или применять везде?
4. **Пресеты меняют размер курсора** (2.5) — в v1 так. Оставить или пресеты трогают только движение?
5. **Фокус при клипах с полями и разном кропе.** Хранить его в долях кадра выхода (проще, зум переживает смену кропа и подходит для нескольких клипов) или, как в v1, в долях кропнутого клипа?
6. **Рамка у follow-зума** (§3). Показывать ли её, чтобы задавать масштаб?
7. **Масштаб по умолчанию.** В v1 это 1.5×, а в примере CLI в ARCHITECTURE `--scale 1.8`. Какой брать?
8. **Выход в конце видео** (§11.5). Нужно ли укорачивать выход, чтобы он завершился в кадре?
