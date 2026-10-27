# Звук: выжимка v1 (задача 0.14)

Источник — v1 в `.v1/` (ветка `main`). Ссылки `файл:строки` даны относительно `.v1/`. После этой выжимки код v1 читать не нужно.

Короткие пути:
- `audioRoutingEngine.ts`, `audioProcessorShared.ts`, `audioProcessorBase.ts`, `offlineAudioProcessor.ts`, `audioTimelineProcessor.ts` лежат в `src/lib/exporter/`;
- `mediaTiming.ts` — в `src/lib/`, `silence.ts` и `generate.ts` — в `electron/ipc/captions/`, `audioFilters.ts` — в `electron/ipc/recording/`;
- остальные пути без `src/` или `electron/` (`audio/…`, `timeline/…`, `export/…`, `hooks/…`, `layout/…`, `state/…`, `types.ts`, `projectPersistence.ts`, `clipSequence.ts`, `SettingsPanel.tsx`) — относительно `src/components/video-editor/`.

Соглашения:
- все уровни — **линейные множители амплитуды**, не dB;
- время в ms, если не сказано «с»;
- **[не переносить]** — баг, костыль или артефакт браузерной реализации v1;
- **[вопрос]** — решение за владельцем, список в §13.

Что берём в v2 (SCOPE «Аудио», «Таймлайн», «Запись»):
- каналы записи и их уровни/mute для каждого клипа (§1–2, §4);
- громкость 0–100% и усиление до 10× (§3);
- скорость с сохранением тона (§5) и склейки (§6);
- дополнительные дорожки (§7) и громкость превью (§8);
- поиск тишины для `devcast silences` (§10).

Нормализация в v2 — EBU R128 (§3.3). Шумоподавление — nnnoiseless (§9).

## 1. Модель данных v1
```
ClipRegion { id, startMs, endMs /*timeline*/, sourceStartMs?, speed, muted?, showSourceAudio? }   # types.ts:241-258
SourceAudioTrackSetting { volume: 0..1, gain?: 0..10 (по умолчанию 1), normalize: bool }        # audio/audioTypes.ts:5-11
SourceAudioTrackSettings = Record<trackId, SourceAudioTrackSetting>; trackId ∈ {"mixed","system","mic"}
defaultSourceAudioTrackSettings: SourceAudioTrackSettings                  # «по умолчанию» проекта
sourceAudioTrackSettingsByClip: Record<clipId, SourceAudioTrackSettings>   # переопределения клипа
AudioRegion { id, startMs, endMs /*timeline*/, audioPath, volume 0..1, gain? 0..10, normalize?, trackIndex? }  # types.ts:1074-1085
audioTrackMix: Record<trackIndex, {volume 0..1, gain 0..10}>; audioTrackCount ≤ 32
```
Зажимы при загрузке проекта (`projectPersistence.ts:820-847, 1198-1210`):
- у региона: `volume` в [0,1] (по умолчанию 1), `gain` в [0,10] (по умолчанию 1), `trackIndex = max(0, floor)` (по умолчанию 0); без `endMs` — `start + 1000`;
- `audioTrackCount = max(0, min(32, floor(saved) or 1), maxTrackIndex+1)`;
- в `audioTrackMix` берутся только целые индексы ≥ 0, `volume` и `gain` зажимаются так же.

UI (`SettingsPanel.tsx:2835-2999`):
- громкость: 0–1, шаг 0.01, показ `round(v*100)%`;
- усиление («Boost»): 0–10, шаг 0.05, показ `v.toFixed(2)×`;
- «Normalize» — переключатель.

## 2. Каналы записи: маршрутизация

### 2.1 Какие дорожки есть у записи в v1 [не переносить — способ, переносить — смысл]
- В v1 каналы определяются по **именам файлов-спутников**: путь содержит `.mic.` → `mic`, `.system.` → `system`, иначе `mixed` (`src/lib/exporter/audioRoutingEngine.ts:32-37`; расширенный набор шаблонов — `audioProcessorShared.ts:64-92`).
- Список дорожек клипа для UI (`timeline/sourceAudioTracks.ts:19-67`):
  - есть sidecar system и/или mic → `[system?, mic?]`;
  - есть только mic → «system» берётся из встроенной дорожки видео;
  - sidecar нет → `[mixed]` (встроенная дорожка).
- Что идёт в экспорт (`audioRoutingEngine.ts:65-71`):
  - `playbackPaths = [system?, mic?]`, а при отсутствии обоих — `[mixed?]`;
  - встроенная дорожка видео берётся, только если нет sidecar `system` и `mixed`.
- **v2**: системный звук и микрофон пишутся отдельными дорожками (SCOPE «Запись»). У каждого клипа есть дорожки `system` и `mic` (у импортированного видео — его дорожки). Угадывание по именам и задержки sidecar (`estimateCompanionAudioStartDelaySeconds`, `mediaTiming.ts:54-79`) не нужны.

### 2.2 Уровни: по умолчанию и для клипа (`audio/useSourceAudioTrackSettings.ts:54-246`)
- **Эффективные настройки клипа:** `{...defaults, ...byClip[clipId]}`. Слияние идёт **по id дорожки целиком**: если у клипа есть запись для `mic`, она полностью заменяет `defaults.mic`.
- **Что правит панель:**
  - клип не выбран — правятся `defaults`;
  - клип выбран — правится `byClip[clipId]`.
- **Первая правка клипа копирует все текущие defaults** (`prevClip = byClip[id] ?? defaults`). После этого клип «замораживает» все дорожки, и последующие правки defaults на него не влияют **[вопрос]** 1.
- Зажим при вводе: `volume` в [0,1], `gain` в [0,10]. Нечисловое значение оставляет прежний `volume` и ставит `gain = 1`.
- Новая дорожка в списке получает `{volume:1, gain:1, normalize:false}`. Ключи `mixed/system/mic` из прежних настроек сохраняются, даже если у текущей записи такой дорожки нет (`:74-109`).
- **Сброс клипа** (`audio/useClipAudioReset.ts:17-41`) пишет каждой дорожке `{volume:1, normalize:false}`, а `gain` выпадает и считается 1. Явные нейтральные значения перекрывают defaults.
  - Баг: признак «есть переопределения» проверяет только `volume != 1 || normalize` и не видит `gain` **[не переносить]**.
- **Баг экспорта v1** (`export/useExportRunner.ts:286-289`): в экспорт уходит **один** набор настроек — выбранного клипа или клипа под указателем — и применяется ко всем клипам. Превью применяет настройки клипа под указателем, поэтому превью ≠ экспорт **[не переносить]**. В v2 у каждого клипа свой уровень на всём его отрезке.

## 3. Громкость, усиление, «нормализация»

### 3.1 Формулы (`audio/audioTypes.ts:27-40`, `audioRoutingEngine.ts:82-87`, `offlineAudioProcessor.ts:412-417`, `audio/audioTrackMix.ts:10-20`)
```
NORMALIZE = 1.35                         # «нормализация» v1 — фиксированный множитель
level(s)  = clamp(s.volume ?? 1, 0, 1) * clamp(s.gain ?? 1, 0, 10) * (s.normalize ? 1.35 : 1)

# канал записи (system/mic) клипа: дополнительно умножается на настройку "mixed" как на мастер
recGain(settings, id) = clamp(level(settings[id]) * (id != "mixed" ? level(settings.mixed) : 1), 0, 10)

# пользовательский регион: сначала микс дорожки
region.volume *= trackMix[trackIndex].volume;  region.gain = (region.gain ?? 1) * trackMix[trackIndex].gain
userGain = clamp(region.volume * region.gain * (region.normalize ? 1.35 : 1), 0, 10)
```
- Ключ `mixed` для `system/mic` в v1 почти всегда отсутствует или устарел, остаётся от прошлой записи **[не переносить]**. В v2 мастер клипа не нужен **[вопрос]** 2.
- Порядок в цепочке: множители одного источника перемножаются в скаляр **до** суммирования. Каждый источник — отдельный `GainNode` с этим скаляром (`audioTimelineProcessor.ts:181-207`, `offlineAudioProcessor.ts:412-423`). Затем сумма всех источников, затем мягкий лимитер (§3.2). Мастер-уровня экспорта нет.

### 3.2 Защита от клиппинга
- **Экспорт** (`audioProcessorShared.ts:18-55`): после рендера каждого 30-секундного куска применяется **поэлементный** waveshaper. Он без памяти, поэтому кусками считать можно.
  ```
  T = 0.9; C = 0.985
  f(x) = x                                             если |x| <= T
       = sign(x) * min(C, T + (1-T)*tanh((|x|-T)/(1-T))) иначе
  NaN/±Inf -> 0
  ```
  Это мягкое насыщение (искажает при больших усилениях), а не лимитер с упреждением.
- При записи WAV ещё есть жёсткий зажим `[-1,1]` и `s<0 ? s*32768 : s*32767` (`audioProcessorBase.ts:64-65`).
- **Превью** v1 лимитера не имеет: Web Audio режет на выходе, поэтому при gain > 1 превью звучит иначе, чем экспорт **[не переносить]**.
- В v2 (roadmap 4.7) нужен «лимитер без цифрового клиппинга». Константы v1 (порог 0.9, потолок 0.985) можно взять как отправную точку, но тип лимитера — решение v2.

### 3.3 Нормализация
- В v1 `normalize` — это множитель ×1.35, без анализа громкости (`audioTypes.ts:27`) **[не переносить]**.
- В продукте v1 `loudnorm`/R128 **нигде не используется**. Единственное упоминание — пример в тесте: `loudnorm=I=-16:TP=-1.5:LRA=11` (`electron/ipc/ffmpeg/filters.test.ts:119`). Это не решение по продукту.
- В v2 R128 через `ebur128` (roadmap 4.8). Целевые LUFS и true-peak — **[вопрос]** 3.
- Где в цепочке R128 в v2: логично до пользовательской громкости и усиления клипа/дорожки (как «нормализовать источник»), но это решение v2, в v1 аналога нет.

## 4. Mute
- В v1 есть только mute **клипа целиком** (все каналы записи): `ClipRegion.muted`, переключатель «Mute clip» (`SettingsPanel.tsx:2928-2937`).
- Пользовательские регионы mute клипа не затрагивает.
- **Экспорт** (`offlineAudioProcessor.ts:181-192`, `audioTimelineProcessor.ts:149-174`): интервалы выхода `[clip.start, clip.end]` замьюченных клипов вычитаются из каждого куска записи. Вырез жёсткий, без фейдов.
- **Превью** (`audio/useVideoEditorAudio.ts:86-95`, `audio/useAudioPreviewSync.ts:327-338, 452-456`):
  - если под указателем замьюченный клип **или клипа нет (дыра)**, мастер-гейн источников = 0;
  - громкость `<video>` тоже 0.
- Отдельного mute канала в v1 нет: его заменяет громкость 0. SCOPE v2: «уровни и mute для каждого клипа» **[вопрос]** 4.

## 5. Скорость с сохранением высоты тона
- Скорость задаётся на клип: `ClipRegion.speed`, `endMs = startMs + srcDur/speed`.
- Ползунок: шаг 0.25. Диапазон — то, что поддерживает браузер, от 0.25 до ≤ 30 (`videoPlayback/playbackRate.ts:20-27`). Неподдерживаемая скорость в превью **ставит звук на паузу** (`useAudioPreviewSync.ts:460-463`). В v2 по SCOPE 0.25–16×.
- Старые «speed regions» с набором `0.25…2×` (`types.ts:1139-1158`) — второй механизм скорости **[не переносить]**.
- **Превью v1:** `HTMLMediaElement.playbackRate = speed` плюс `preservesPitch = true` (`mediaTiming.ts:124-129`), то есть алгоритм браузера.
- **Экспорт v1, основной путь** — свой WSOLA (`audioTimelineProcessor.ts:216-356`), по одному вызову на каждый звучащий кусок клипа:
  ```
  если |speed-1| < 0.001: копия сэмплов без растяжения
  pad = 0.1 с с каждой стороны (на входе ×speed), рендер с запасом, затем вырезается середина
  win  = floor(sr*0.040)   окно Ханна w[i] = 0.5*(1-cos(2πi/(win-1)))
  hopOut = floor(win*0.5); hopIn = floor(hopOut*speed); search = floor(sr*0.015)
  первое окно: out[0..win) += in[start..]*w
  далее: в [inOff-search, inOff+search] с шагом 4 сэмпла выбрать сдвиг с max Σ out[outOff+i]*in[test+i]
         (i < hopOut с шагом 4, по всем каналам); out[outOff..] += in[best..]*w; outOff += hopOut; inOff += hopIn
  ```
- **Экспорт v1, быстрый путь через ffmpeg** — только скорость 0.5–2, один источник, без регионов (`electron/ipc/nativeVideoExport.ts:9-10, 648-720`; `src/lib/exporter/editedTrackStrategy.ts:3-4`). Цепочка `atempo` (`electron/ipc/ffmpeg/filters.ts:6-30`):
  ```
  r = speed; while r < 0.5: push "atempo=0.5", r /= 0.5
  while r > 2: push "atempo=2.0", r /= 2
  if |r-1| > 0.0005: push "atempo=" + r.toFixed(6)
  ```
  Плюс `atrim=start:end, asetpts=PTS-STARTPTS` на кусок и `concat` кусков.
- В v2 (roadmap 4.6) скорость через `atempo`, один путь для превью и экспорта. Цепочку разложения брать отсюда.

## 6. Склейки, вырезы, фейды
- **В редакторе и экспорте v1 кроссфейдов и фейдов на стыках нет.** Каждый кусок клипа или звучащий отрезок между mute — отдельный `AudioBufferSourceNode`, который стартует с точной позиции и обрывается жёстко (`audioTimelineProcessor.ts:76-214`). Возможны щелчки.
- Запас 0.1 с в WSOLA спасает только от затухания окна на краях, это не фейд стыка.
- Единственный фейд в v1 — при **записи**: линейный fade-in 5 ms после паузы/продолжения и после вставленной тишины (§11).
- В v2 «короткие фейды на стыках» (roadmap 4.6) — новая функция, длительность из v1 взять нельзя **[вопрос]** 5.

## 7. Дополнительные дорожки
- **Форматы** в диалоге: `mp3, wav, aac, m4a, flac, ogg` плюс «All Files» (`electron/ipc/register/captions.ts:82-111`). Декодирует браузер. Длительность проверяется загрузкой метаданных: 0 или ошибка → «Could not read audio file».
- **Добавление** (`timeline/hooks/actions/useTimelineAudioActions.ts:89-140`, `timeline/hooks/utils/timelineAudioPlacement.ts:17-96`):
  ```
  start = clamp(playhead, 0, total); если total - start <= 0 -> ошибка
  desired = min(fileDur, total - start)
  кандидаты дорожек = [preferred] или [0 .. maxTrackIndex+1]
  gap(track) = 0, если [start, start+desired) пересекает регион дорожки,
               иначе (начало следующего региона дорожки после start или total) - start
  дорожка = первая с gap >= desired; если нет и preferred не задан — первая с gap > 0
  нет дорожки -> ошибка; dur = min(fileDur, gap, total - start)
  регион = {start, start+dur, volume 1, gain 1, normalize false, trackIndex}   # hooks/useAudioRegionCommands.ts:50-61
  ```
- **Время** — по времени выхода (таймлайна).
  - Регион всегда играет файл **с его начала**: `offset = t - region.start`. Точки входа в файле нет, поэтому сдвиг левого края сдвигает содержимое.
  - Петли нет: после конца файла тишина.
  - Скорость клипов и mute на регион не влияют.
  - Экспорт: `duration = min(region.end - region.start, fileDur - offset)`, выход удлиняется до `max(конец таймлайна, max region.end)` (`offlineAudioProcessor.ts:166-178, 377-424`).
- **Сдвиг при монтаже в v1** (`clipSequence.ts:58-90`):
  - при вырезе, перестановке или смене скорости клипов начало региона «привязано» к кадру под ним (`mapClipSequenceTime`), длительность сохраняется;
  - регион после конца видео сохраняет расстояние до конца.
  - В v2 по ARCHITECTURE доп. дорожки идут **по времени выхода и не сдвигаются**. Это расходится с v1 **[вопрос]** 6.
- **Уровни:** регион (громкость и усиление) × дорожка (громкость и усиление) по §3.1. «Сбросить» в панели ставит региону 1 / 1 / false.
- **Превью** (`useAudioPreviewSync.ts:403-435`) — подстройка `<audio>` [не переносить, в v2 общие часы]:
  - если `|cur - target| > 0.2 с`, перемотка;
  - иначе подстройка скорости `rate = max(0.1, 1 + clamp((target - cur)/2, ±0.08))`, если дрейф > 0.015 с (`mediaTiming.ts:81-116`).

## 8. Громкость превью
- Ползунок 0–1, шаг 0.01, показ `round(v*100)%`. Иконки: ≤ 0.001 — «без звука», < 0.5 — «тихо», иначе — «громко» (`layout/EditorPreviewPanel.tsx:446-485`).
- Значение по умолчанию 1, **не сохраняется** (`state/useEditorUiState.ts:46`).
- Влияет **только на прослушивание**, не на экспорт.
- Граф превью: каналы записи → `GainNode(recGain, 0..10)` → мастер `GainNode(isMuted ? 0 : clamp(previewVolume, 0, 1))` → выход. Пользовательские регионы → `GainNode(clamp(userGain * previewVolume, 0, 10))` → выход, в обход мастера (`useAudioPreviewSync.ts:119-170, 226, 327-338`).

## 9. Микрофон: declip, шумоподавление, браузерный запасной путь [не переносить]
- `WINDOWS_NATIVE_MIC_PRE_FILTERS = ["adeclip=threshold=1"]` объявлен, но **нигде не используется** (`electron/ipc/recording/audioFilters.ts:3`). Нативный микрофон Windows пишется «сухим», без AGC и loudnorm: на бета-тестах они усиливали шум гарнитур.
- Цепочка ffmpeg **только для браузерного запасного пути** (запись через MediaRecorder, `electron/ipc/register/recording.ts:1683-1707`, `audioFilters.ts:9-33`):
  ```
  ffmpeg -y -i mic.webm -vn -ac 1 -ar 48000 -af "<filters>,aresample=async=1:first_pts=0" -c:a pcm_s16le <base>.mic.wav
  base:   adeclip=threshold=1, adeclick=w=40:o=75:t=3:b=2, highpass=f=85, lowpass=f=9500, afftdn=nr=10:nf=-45:tn=1
  профиль "no-agc": base + speechnorm=p=0.92:e=12:c=2:r=0.0005:f=0.001 + alimiter=limit=0.92:level=0
  иначе:            base + alimiter=limit=0.92:level=0
  ```
- Ограничения браузера (`src/hooks/useScreenRecorder.ts:221-238`):
  - `echoCancellation`, `noiseSuppression` и `autoGainControl` включены в профиле по умолчанию `processed`;
  - профили `no-echo`, `no-noise-suppression` и `no-agc` выключают соответствующий пункт, `raw` — все три;
  - `channelCount ideal 1`, `sampleRate ideal 48000`.
- В v2 захват один на ОС, без браузерных путей (SCOPE, «Уроки v1»).
  - Шумоподавление — переключатель на дорожке, nnnoiseless, исходник не меняется.
  - Declip, highpass/lowpass и speechnorm в SCOPE нет. Нужен ли declip в v2 — **[вопрос]** 7.

## 10. Поиск тишины (v1: только для субтитров; v2: и для `devcast silences`)
- **Вызов** (`electron/ipc/captions/generate.ts:208-230`, константы `electron/ipc/captions/silence.ts:18-20`):
  ```
  ffmpeg -hide_banner -nostats -i <src.wav> -af silencedetect=noise=-30dB:d=0.5 -f null -
  SILENCE_NOISE_DB = -30        # порог, dBFS
  SILENCE_DETECT_MIN_S = 0.5    # минимальная длительность тишины, с
  ```
- **Вход:** wav 16 кГц, моно, s16, полученный как в субтитрах (`ffmpeg … -map 0:a:0 -ac 1 -ar 16000 -c:a pcm_s16le`), то есть одна дорожка: микрофон, если есть.
- **Семантика silencedetect** (ffmpeg, проверить на фикстуре задачи 4.11 при собственной реализации):
  - сравнивается **амплитуда каждого сэмпла**, не RMS и не окно: тишина, если `|x| < 10^(-30/20) ≈ 0.0316` полной шкалы;
  - тишина начинается, когда подряд набралось ≥ `0.5 с × sr` тихих сэмплов (для моно); `silence_start` = время первого тихого сэмпла серии;
  - `silence_end` = время первого громкого сэмпла.
- **Разбор stderr** (`silence.ts:60-87`):
  ```
  for line in stderr.lines():
    if m = /silence_start:\s*(-?[\d.]+)/: pending = max(0, round(float(m)*1000)); continue
    if m = /silence_end:\s*(-?[\d.]+)/ and pending != null:
       end = round(float(m)*1000); if end > pending: push {pending, end}; pending = null
  if pending != null: push {pending, +∞}       # тишина до конца файла
  sort by start
  ```
- **Крайние случаи:**
  - отрицательный `silence_start` → 0;
  - `silence_end` без начала игнорируется;
  - интервалы с `end <= start` отбрасываются;
  - хвостовая тишина получает `endMs = +∞`. В v2 заменить на длительность дорожки.
- **Ошибка поиска** в субтитрах не фатальна: используются сырые сегменты.
- **Расхождение:** в ARCHITECTURE для CLI указано `devcast silences [--min 0.5] [--db -40]`, а в v1 −30 dB **[вопрос]** 8. Какую дорожку анализировать в v2 (микрофон, микс, выбранные) — **[вопрос]** 9.
- **Пороги, которыми субтитры пользуются поверх этого списка:**
  - разрыв реплики на тишине ≥ 1500 ms;
  - удаление слов внутри такой тишины.
  
  См. `captions.md` §4.

## 11. Смежное: захват на Windows (для задачи 3.7)
`electron/native/wgc-capture/src/wasapi_loopback.cpp:11-19, 355-410, 515-545`:
- **Ожидаемый кадр пакета:**
  - `expected = round((qpc - firstQpc - paused) * sr / 1e7)`, если таймстемп надёжен (нет `TIMESTAMP_ERROR`);
  - если `expected - written` больше порога, вставляется столько нулей.
- **Порог:**
  - 50 ms (`kMinimumGapFillThresholdMs`);
  - при флаге `DATA_DISCONTINUITY` 140 ms (`kDiscontinuityGapFillThresholdMs`), а меньший разрыв «схлопывается» без тишины.
- **Тихие пакеты:** с флагом `SILENT` пишутся нулями. Если при этом есть discontinuity и длина ≤ 40 ms (`kSilentDiscontinuityCompactThresholdMs`), пакет отбрасывается.
- **Fade-in:** линейный 5 ms (`kBoundaryFadeInMs`) после старта/продолжения и после вставки тишины. `scale = (k+1)/N` на кадр `k` из `N = sr*5/1000`.
- Нули пишутся блоками по 4096 кадров.

## 12. Не переносить (сводка)
- Множитель «нормализации» ×1.35.
- Один набор уровней на весь экспорт.
- Каналы по именам файлов и мастер `mixed`.
- Разные пути экспорта: passthrough, trim-only, offline, ffmpeg-atempo (`src/lib/exporter/audioEncoder.ts:104-230`).
- Превью без лимитера.
- Подстройка `<audio>` по дрейфу.
- Браузерная цепочка фильтров микрофона.
- Неиспользуемый `adeclip` для Windows.
- Старые speed regions.
- Невидимый сброс `gain` в «Reset clip audio».

## 13. Вопросы владельцу
1. Клип с правками: наследует ли он дальнейшие изменения «по умолчанию» для дорожек, которые в нём не трогали? В v1 первая правка копирует и замораживает все дорожки.
2. Нужен ли общий уровень клипа поверх уровней системного звука и микрофона? В v1 был скрытый мастер `mixed`.
3. Нормализация R128: целевая громкость (например −16 или −14 LUFS) и true-peak? В v1 данных нет, в тесте встречается только −16 LUFS / TP −1.5 / LRA 11.
4. Mute в v2: по каналу клипа (отдельно системный звук и микрофон), только всего клипа, или и то и другое? В v1 только весь клип.
5. Длительность фейда на стыках вырезов и клипов? В v1 фейдов нет, есть только 5 ms fade-in при записи.
6. Доп. аудиорегионы при монтаже основной дорожки: стоят на месте (ARCHITECTURE) или едут за кадром под их началом (v1)?
7. Нужен ли в v2 declip микрофона (в v1 он объявлен, но не применялся) или хватит переключателя шумоподавления?
8. Порог тишины по умолчанию для `devcast silences`: −30 dB (v1) или −40 dB (ARCHITECTURE)? Минимальная длительность 0.5 с совпадает.
9. По какой дорожке искать тишину: микрофон (как в v1), микс клипа или выбранные дорожки?
