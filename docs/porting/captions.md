# Субтитры: выжимка v1 (задача 0.13)

Источник — v1 в `.v1/` (ветка `main`). Ссылки `файл:строки` даны относительно `.v1/`. После этой выжимки код v1 читать не нужно.

Короткие пути:
- `parser.ts`, `segment.ts`, `silence.ts`, `generate.ts`, `mergeSources.ts`, `whisper.ts` лежат в `electron/ipc/captions/`;
- `captionRenderer.ts` — в `src/lib/exporter/`, `squircle.ts` — в `src/lib/geometry/`;
- остальные пути без `src/` или `electron/` (`captionLayout.ts`, `types.ts`, `hooks/…`, `timeline/…`, `SettingsPanel.tsx`) — относительно `src/components/video-editor/`.

Соглашения:
- время — целые **миллисекунды** (`ms`), всегда `Math.round`; кроме мест, где явно сказано «с»;
- время реплик и слов — **source time** записи (как в ARCHITECTURE.md п. 2);
- **[не переносить]** — баг, костыль или артефакт браузерной реализации v1;
- **[вопрос]** — решение за владельцем, список в §11;
- JS `string.length` считает UTF-16 единицы. В Rust для кириллицы брать `chars().count()`, а не `len()` (байты).

Что берём в v2 (SCOPE «Субтитры»): распознавание (§1), источник (§2), сегментацию (§4), стиль (§5–6), подсветку слова (§7), анимации (§8), правку (§9).

## 1. Распознавание (Whisper)

### 1.1 Модель
- В v1 только `ggml-small.bin`: `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin`, кладётся в `<userData>/whisper/ggml-small.bin` (`electron/ipc/constants.ts:17-20`). Можно выбрать свой `.bin` (скрыто в «advanced»).
- Скачивание (`electron/ipc/captions/whisper.ts:41-151`): HTTPS GET, таймаут сокета 30 000 ms, до 5 редиректов. Пишется в `<path>.download`, при успехе `rename`, при ошибке файл удаляется. Прогресс равен `min(100, round(downloaded/contentLength*100))`, если `content-length` > 0, а в конце всегда 100.
- Хэш не проверяется, докачки нет. В v2 по SCOPE нужны модели small и large-v3-turbo, SHA-256 и докачка.

### 1.2 Подготовка звука (`electron/ipc/captions/generate.ts:144-206`)
Перебор кандидатов по порядку (§2), берётся первый, из которого ffmpeg смог извлечь звук:
```
ffmpeg -y -i <candidate> -map 0:a:0 -vn [-af adelay=<delayMs>:all=1] -ac 1 -ar 16000 -c:a pcm_s16le <tmp>.wav
```
- `adelay` ставится только для sidecar-файла (`*.mic.*`/`*.system.*`) с `startDelayMs > 0` из `<file>.json`. Это выравнивание sidecar к видео в v1 **[не переносить]**: в v2 дорожки выровнены при записи.
- Итог — **16 кГц, моно, s16**. В v2 whisper-rs принимает f32 16 кГц моно: ресэмплинг через rubato и даунмикс в моно.
- Таймауты: ffmpeg 5 мин, whisper 30 мин (`generate.ts:35,184`). Временные `.wav/.srt/.json` удаляются в `finally`.

### 1.3 Вызов (`generate.ts:272-304`)
```
whisper-cli -m <model> -f <tmp.wav> -osrt -of <base> -l <lang> -np -mc 0 -ojf
```
| флаг | смысл | в whisper-rs |
|---|---|---|
| `-l <lang>` | язык; `settings.language.trim()` или `"auto"`, если пусто | `set_language(None)` для auto, иначе код |
| `-mc 0` | max-context 0: не передавать текст прошлых окон, чтобы не тянуть галлюцинацию дальше | `n_max_text_ctx = 0` |
| `-ojf` | полный JSON с токенами и их `offsets.from/to` (ms) | токены через `full_get_token_data` |
| `-osrt` | SRT как запасной вывод | не нужен |
| `-np` | без логов | — |

- Остальные параметры (beam, threads, temperature) v1 не задаёт: берутся дефолты whisper-cli.
- **Язык.** UI v1 предлагает 10 значений: `auto, en, es, fr, de, it, pt, zh, ja, ko` (`src/components/video-editor/SettingsPanel.tsx:641-652`). **Русского нет.** При загрузке проекта принимается любая непустая строка (`projectPersistence.ts:919-923`). По умолчанию `"auto"` (`types.ts:1125`).
- В v2 нужны «авто» и любой язык Whisper, включая `ru`.
- Если whisper не знает `-ojf`, v1 повторяет запуск без него (`parser.ts:182-187`) **[не переносить]**.

### 1.4 Тайминг слов
В v1 время слов — это **таймстемпы токенов whisper.cpp из `-ojf`, не DTW**. В v2 будет DTW (ARCHITECTURE).

Сборка слов из токенов сегмента (`electron/ipc/captions/parser.ts:19-76`):
```
words = []; nextLeadingSpace = false
for token in segment.tokens:
  if token.text == "" or token.text matches /^\[_[^\]]+\]$/: continue  # [_BEG_], [_TT_123] и т.п.
  from = round(token.offsets.from); to = round(token.offsets.to)       # ms
  for part in split(token.text, /\s+|[^\s]+/):
    if part is whitespace: nextLeadingSpace = (words nonempty); continue
    prev = last(words)
    if prev and not nextLeadingSpace and part matches /^\p{P}+$/u:
       prev.text += part; continue            # пунктуация без пробела: к прошлому слову, endMs не меняется
    if from/to не числа or to <= from: return []   # !!! весь сегмент теряет слова
    if not prev or nextLeadingSpace:
       push {text: part, startMs: from, endMs: to, leadingSpace: words nonempty && nextLeadingSpace}
    else:
       prev.text += part; prev.endMs = max(prev.endMs, to)  # подслово BPE без пробела
    nextLeadingSpace = false
return words.filter(text.trim() != "")
```
- Сегмент → реплика (`parser.ts:78-128`): `startMs/endMs = round(offsets)`. Сегмент отбрасывается, если `end <= start` или текст пуст. Текст строится из слов (`buildCaptionTextFromWords`: слова склеиваются, перед словом с `leadingSpace` и индексом > 0 ставится пробел, затем `trim`), а если слов нет — `segment.text.trim()`.
- Одного токена нулевой длины хватает, чтобы весь сегмент остался без слов. Это ведёт в «смешанную» ветку §4.4 **[не переносить]**: в v2 с DTW у слова всегда есть время.
- SRT-разбор (`parser.ts:130-180`): только формат `HH:MM:SS,mmm`. В v2 не нужен.
- Заметки для v2 (это не поведение v1, проверить в пробе 1.10):
  - в whisper.cpp `t0/t1/t_dtw` токена хранятся в сотых долях секунды (×10 = ms);
  - текст токена — сырые байты, кириллица может делиться между токенами, поэтому склеивать байты до декодирования UTF-8;
  - спецтокены отсекать по id (≥ `token_eot`), а не по регэкспу.

### 1.5 Запуск из UI (`src/components/video-editor/captions/useAutoCaptionController.ts:170-245`)
- Повторный запуск во время генерации игнорируется. Без модели показывается ошибка.
- Прогресс поддельный: старт 6, каждые 900 ms `p = min(88, p + max(1, round((90-p)/7)))`, при успехе 100, сброс через 500 ms **[не переносить]**. В v2 брать реальный прогресс whisper.
- Результат **заменяет все реплики** (`setAutoCaptions(result.cues)`), ручные правки теряются. Если реплик > 0, ставится `enabled = true`.
- Пока шла генерация, источник могли сменить. Тогда результат не применяется.

## 2. Источник звука и слияние микрофона с системным звуком

**Порядок кандидатов** (`generate.ts:112-142`):
1. mic-sidecar;
2. сам файл записи (встроенная дорожка);
3. остальные sidecar (system);
4. связанная запись вебкамеры.

Дубли путей отбрасываются.

**Генерация** (`generate.ts:339-372`):
- Если микрофона нет — одна транскрипция по `[system, ...secondary]`.
- Если микрофон есть — **две независимые транскрипции**: микрофон отдельно и `[system, ...secondary]` отдельно. Так одновременные голоса не мешают распознаванию.
- Каждая проходит полный конвейер: извлечение → whisper → тишина → сегментация (§4) на **своём** wav.
- Если источник не извлёкся, он считается пустым. Если пусты оба — ошибка «нет звука».
- Затем `mergeCaptionSources(mic, system)`.

**`mergeCaptionSources`** (`electron/ipc/captions/mergeSources.ts:5-62`). Приоритет у микрофона:
```
isLabel(cue) = cue.text matches /^(?:\s*[[(][^\])]+[\])]\s*)+$/   # «[музыка]», «(смех)»
micSpeech = mic.filter(!isLabel)
micSpans  = flatten(micSpeech, cue -> cue.words nonempty ? cue.words : [cue])
overlapsMic(s,e) = any span: s < span.endMs && e > span.startMs
for cue in system:
  if !overlapsMic(cue): keep cue
  elif cue has no words: вычесть все micSpans из [start,end], оставить куски с ПОЛНЫМ исходным текстом   # текст дублируется
  else: разбить слова на непрерывные серии, не пересекающие mic; каждая серия -> реплика (текст из слов)
micEvents = mic.filter(isLabel && не пересекается ни с одной system-репликой)
result = sort(micSpeech + micEvents + systemCues, by (startMs, endMs)); id = "caption-<i+1>"
```
- После слияния повторной сегментации нет. Края реплик из разных источников могут перекрываться на величину отступа (80 ms).
- **v2**: дорожки выбираются, по умолчанию микрофон (SCOPE). Каждую выбранную дорожку транскрибировать отдельно и сливать этим алгоритмом с приоритетом микрофона. Если выбрано больше двух дорожек — см. **[вопрос]** 4.
- Ветка «system без слов» в v2 не нужна: с DTW слова есть всегда.

## 3. Поиск тишины (вход сегментации)
`ffmpeg -hide_banner -nostats -i <tmp.wav> -af silencedetect=noise=-30dB:d=0.5 -f null -` (`generate.ts:208-230`, константы `silence.ts:18-20`).
- Анализируется тот же wav 16 кГц моно, что и для whisper.
- Разбор вывода, семантика и крайние случаи — в `audio.md` §10. Результат — отсортированные интервалы `{startMs, endMs}` в ms, у хвостовой тишины `endMs = +∞`.
- Если поиск тишины упал, используются сырые сегменты whisper (`generate.ts:312-324`). Пустой результат сегментации — нормальный исход (все слова отброшены как галлюцинации).

## 4. Сегментация слов в реплики (`electron/ipc/captions/segment.ts`)

### 4.1 Константы (`segment.ts:26-44`, `silence.ts:23-27`)
| имя | значение | смысл |
|---|---|---|
| `pauseMs` | 700 | зазор между соседними словами ≥ этого — разрыв |
| `splitSilenceMs` | 1500 | тишина ≥ этого внутри зазора — разрыв; слова целиком внутри такой тишины удаляются |
| `edgePadMs` | 80 | отступ вокруг реплики |
| `maxPhraseMs` | 12000 | страховка: фраза без точки и паузы рвётся, когда достигает этой длины |
| `minCaptionMs` | 800 | реплики короче — кандидаты на склейку |
| `mergeGapMs` | 400 | склеивать только при зазоре ≤ этого |
| `maxMergedMs` | 2500 | склейка не длиннее |
| `maxMergedChars` | 80 | склейка не длиннее (символы, `+1` на пробел) |
| `minSpeechMs` | 150 | только для ветки без слов: регион речи короче отбрасывается |

**Лимита «слов в реплике» в v1 нет.** Строки и ширина — это раскладка при отрисовке (§6), а не сегментация. Минимальной длительности реплики тоже нет: 800 ms — только порог склейки.

### 4.2 Конец предложения (`segment.ts:72-112`)
```
TRAILING_CLOSERS = /[)\]}"'”’»」』）】］｝>]+$/u
SENTENCE_END     = /[.?!…。！？]$/
endsSentence(t): s = trim(t) без хвостовых закрывающих скобок/кавычек, снова trim
                 return SENTENCE_END(s) && !isAbbreviation(t)
isAbbreviation(t): s как выше; только если s оканчивается на "."; core = lower(s без точки)
                   core пусто -> false
                   core ~ /^[a-z]$/ или /^[a-z](\.[a-z])+$/ -> true   # "J.", "e.g.", "U.S."
                   core in {mr,mrs,ms,dr,prof,sr,jr,st,vs,etc} -> true
```
Аббревиатуры только латинские. «т.е.», «т.д.», «г.» в русском тексте рвут реплику **[вопрос]** 5.

### 4.3 Основной алгоритм: у всех сегментов есть слова (`segment.ts:357-475`)
```
segment(cues, silences):
  if cues empty: return []
  if not all cues have words and some cue has words: -> смешанная ветка §4.4
  if no cue has words: -> ветка без слов §4.4
  stream = flattenWords(sort(cues by start,end))
     # слова всех сегментов подряд; у 1-го слова каждого следующего сегмента leadingSpace=true,
     # у остальных leadingSpace сохраняется, если он не false; затем stable sort по (startMs, endMs)
  stream = stream.filter(w -> нет тишины S, где S.len >= 1500 и S.start <= w.start и w.end <= S.end)  # галлюцинации
  if stream empty: return []
  phrases = []; cur = []
  for i, w in stream:
    cur.push(w); next = stream[i+1]; if !next: break
    gap = next.start - w.end
    if endsSentence(w.text) or gap >= 700
       or (gap > 0 and есть S: S.len >= 1500 и S.start < next.start и S.end > w.end)
       or (w.end - cur[0].start) >= 12000:
         phrases.push(cur); cur = []
  if cur nonempty: phrases.push(cur)
  pieces = phrases -> {words (у 1-го слова убрать leadingSpace), start=first.start, end=last.end,
                       text=buildCaptionTextFromWords}; выкинуть пустой текст; sort(start,end)
  merged = mergeShort(pieces)        # ДО отступов, чтобы видеть настоящие зазоры
  padSpans(merged, 80)
  return merged с id "caption-<i+1>"
```
**`mergeShort`** (`segment.ts:311-345`), один проход слева направо:
```
group = cues[0]
for next in cues[1..]:
  if dur(group) < 800 and dur(next) < 800 and next.start - group.end <= 400
     and next.end - group.start <= 2500 and len(group.text) + len(next.text) + 1 <= 80:
       group = mergeTwo(group, next)   # слова подряд, у 1-го слова next leadingSpace=true; id левой
  else: out.push(group); group = next
out.push(group)
```
`dur(group)` берётся у уже склеенной группы. Когда группа доросла до 800 ms, склейка останавливается.

**`padSpans`** (`silence.ts:174-193`) идёт по массиву **по месту, последовательно**: левый отступ считается от уже расширенного конца предыдущей реплики.
```
for i: prevEnd = i>0 ? spans[i-1].end (уже изменённый) : null; nextStart = spans[i+1]?.start
  left  = prevEnd==null ? min(pad, rawStart) : min(pad, floor(max(0, rawStart-prevEnd)/2))
  right = nextStart==null ? pad : min(pad, floor(max(0, nextStart-rawEnd)/2))
  start = max(0, round(rawStart-left)); end = max(start+1, round(rawEnd+right))
```
Пример: зазор 100 ms даёт правый отступ 50 и левый 25. Конец последней реплики может выйти за длину звука на 80 ms, обрезки нет.

### 4.4 Ветки без слов [не переносить в v2, где слова есть всегда]
- **Смешанная** (`segment.ts:379-391`):
  - каждая реплика сегментируется отдельно;
  - результат сортируется;
  - для каждой, кроме последней: `end = max(lastWordEnd, min(end, next.start))`, где `lastWordEnd = max(start, ends of words)`;
  - отбрасываются реплики с `end <= start`, затем перенумерация.
- **Без слов (SRT)**:
  - `resegmentCuesBySilence` (`silence.ts:200-323`): регионы речи = `[первое начало, последний конец]` минус тишины ≥ 1500 ms и тишины, касающиеся краёв; регионы < 150 ms выбрасываются; текст распределяется пропорционально перекрытию;
  - затем деление по предложениям, время пропорционально числу символов (`segment.ts:207-245`);
  - затем `mergeShort` и перенумерация.

### 4.5 Проверенные тестами свойства (`segment.test.ts`)
- Предложения без паузы — разные реплики.
- Слово следующего предложения не утекает в предыдущее.
- Запятая не рвёт реплику.
- «Mr. Smith» не рвётся.
- Пауза ≥ 700 ms рвёт и без пунктуации.
- Тишина ≥ 1500 ms в зазоре рвёт, даже если зазор слов < 700.
- Короткие «Okay.» «Great.» склеиваются.
- Короткая реплика не склеивается с длинной.
- Результат отсортирован и без перекрытий.

## 5. Данные и настройки

```
CaptionCue     { id, startMs, endMs, text, words?: CaptionCueWord[] }      # types.ts:1087-1100
CaptionCueWord { text, startMs, endMs, leadingSpace? }
```
Инвариант: `end >= start + 1`. При загрузке (`projectPersistence.ts:850-903`):
- реплика без `endMs` получает `start + 1000`;
- слова зажимаются в `[cue.start, cue.end-1]` и `[wordStart+1, cue.end]`;
- **реплики с пустым текстом выбрасываются**: пустая реплика, добавленная на таймлайне (§9), исчезает после перезагрузки **[не переносить]**.

Настройки (`types.ts:1104-1137`; зажимы при загрузке `projectPersistence.ts:924-964`; UI `SettingsPanel.tsx:2155-2452`):
| поле | по умолчанию | диапазон, шаг UI | единица |
|---|---|---|---|
| `enabled` (показать/скрыть) | false | — | — |
| `language` | `"auto"` | список §1.3 | код Whisper |
| `fontFamily` | `"SF Pro Text", "SF Pro Display", "Helvetica Neue", sans-serif` | выбор из 4: `Inter, sans-serif`, `Arial, sans-serif`, `Georgia, serif`, `monospace` (`:2350-2353`) | CSS-стек |
| `fontSize` | 30 | 16–72, шаг 1 | px при эталонной ширине (§6.1) |
| `textColor` | `#FFFFFF` | цвет | — |
| `inactiveTextColor` | `#A3A3A3` | **в UI нет** | — |
| `textShadow` | true | вкл/выкл | — |
| `maxRows` | 1 | 1–4, шаг 1, `round` | строк |
| `bottomOffset` | 3 | 0–30, шаг 1 | % высоты кадра |
| `maxWidth` | 62 | 40–95, шаг 1 | % ширины кадра |
| `boxRadius` | 17.5 | 0–40, шаг 0.5 | px при `fontSize` 30 |
| `backgroundOpacity` | 0.9 | 0–1, шаг 0.01 | альфа чёрной плашки |
| `animationStyle` | `"fade"` | `none`/`fade`/`rise`/`pop` (`SettingsPanel.tsx:146-154`) | — |
| `timelineQuickAdd` | true | вкл/выкл | «призрак» на дорожке |

- Шрифт по умолчанию не входит в список из четырёх, на Windows он падает в `sans-serif` **[вопрос]** 1.
- Скрыты под «advanced»: rows, bottomOffset, maxWidth, boxRadius, backgroundOpacity, quickAdd, «свою модель».
- «Сбросить» возвращает **все** поля, включая `enabled=false` и `language`.

## 6. Раскладка и отрисовка (экспорт: `src/lib/exporter/captionRenderer.ts:19-111`, `captionStyle.ts:4-48`)

### 6.1 Размеры
`W, H` — размер кадра выхода (в превью — ширина элемента превью).
```
FONT_WEIGHT = 400; LINE_HEIGHT = 1.32
REF = 1920 * 62/100 = 1190.4
targetW  = max(1, W * maxWidth/100)
fs       = max(14, fontSize * targetW / REF)              # шрифт растёт вместе с maxWidth!
padX = 1.1*fs; padY = 0.78*fs; lineH = 1.32*fs
textMaxW = max(4*fs, targetW - 2*padX)
radius   = max(0, boxRadius * fs / 30)
```
Так как `fs ~ maxWidth`, ползунок «ширина» почти не меняет число слов в строке, а увеличивает текст **[вопрос]** 2. Шрифт задаётся строкой `"400 ${fs}px ${family}"`, ширина меряется через `measureText`.

### 6.2 Разбиение на строки: DP с минимальной «ценой» (`captionLayout.ts:332-411`)
1. Слова всех реплик выпрямляются в один список (`flattenCaptionWords`, `:155-217`):
   - у первого слова каждой реплики, кроме первой, ставится `forcedBreakBefore = true` и `leadingSpace = false`, так что строка никогда не пересекает границу реплик;
   - в репликах без слов слова берутся из текста по пробелам, перевод строки в тексте даёт `forcedBreakBefore`;
   - время таких слов равномерное: `dur = max(1, end-start)/n`, `start_i = start + dur*i`, `end_i = (i==n-1 ? end : start + dur*(i+1))`, `end_i = max(start_i+1, end_i)`.
2. Строки по DP:
```
seg[i]   = i==0 ? w(text_i) : w((leadingSpace_i ? " " : "") + text_i);  plain[i] = w(text_i)
best[n] = 0
for s = n-1 downto 0:
  width = plain[s]
  for e = s .. n-1:
    if e > s: if forcedBreakBefore[e]: break; width += seg[e]
    if width > textMaxW and e > s: break        # одно слово шире лимита остаётся строкой
    slack = max(0, textMaxW - width); full = width / textMaxW
    pen = (e == n-1) ? slack²*0.18 : slack² + (full < 0.72 ? (0.72-full)*26000 : 0)
    if pen + best[e+1] < best[s]: best[s] = …; next[s] = e+1          # строгое «<»
```
3. Ширина строки — сумма ширин сегментов, у первого слова строки нет пробела.

### 6.3 Страницы (`captionLayout.ts:255-330`)
- `totalPages = max(1, ceil(L / maxRows))`.
- Для страницы `p`: `size = min(maxRows, max(1, ceil(remainingLines / (totalPages-p))))` — строки распределяются поровну.
- `page.start` — начало первого слова страницы, `page.end` — конец последнего. Затем `page[i].end = max(start+1, page[i+1].start)`, у последней `end = max(start+1, конец последнего слова)`.
- Видимая страница — первая, у которой `start <= t <= end`. Если такой нет, ничего не рисуется.
- Страницы считаются **по всему списку строк всех реплик**. При `maxRows > 1` строки разных реплик попадают на одну страницу, и правка одной реплики сдвигает группировку дальше по видео **[не переносить]**: в v2 листать страницы внутри реплики, **[вопрос]** 3.

### 6.4 Когда субтитр виден (`captionLayout.ts:136-153, 435-505`)
- `settings.enabled` выключен или реплик нет — ничего не рисуется.
- Покрытие: реплики сортируются по `start`; если `t < cue.start`, ответ «нет».
- Иначе `effEnd = (next && next.start - cue.end < 500) ? max(cue.end, next.start) : cue.end`. Если `t <= effEnd`, ответ «да». Зазоры < 500 ms (`CAPTION_BLOCK_GAP_BREAK_MS`) перекрываются, субтитр не мигает.
- Вне покрытия ничего нет.
- В левом отступе (80 ms) реплики, которая начинает блок после паузы ≥ 500 ms, тоже ничего нет: страница начинается с первого слова.
- Между близкими репликами конец прежней страницы растянут до начала следующей, поэтому старый текст держится до первого слова новой.

### 6.5 Плашка и текст
```
boxH = lines*lineH + 2*padY
boxW = min(W*maxWidth/100 + 2*padX, maxLineWidth + 2*padX)
cx = W/2;  cy = H - H*bottomOffset/100 - boxH/2       # низ плашки на bottomOffset% от низа
transform: translate(cx, cy + translateY) · scale(s) (вокруг центра плашки); alpha = opacity
fill rgba(0,0,0,backgroundOpacity), фигура squircle(-boxW/2, -boxH/2, boxW, boxH, radius)
shadow (если textShadow): color rgba(0,0,0,0.72), blur max(2, 0.16*fs), offsetY max(1, 0.06*fs), offsetX 0
строка i: x0 = -lineW/2; y = -boxH/2 + padY + lineH*i + lineH/2 (baseline = middle)
каждое слово рисуется отдельно: текст (leadingSpace ? " " : "") + word, x += measure(этого текста)
```
- Тень включается после заливки плашки, поэтому падает только на текст.
- **Squircle** (`src/lib/geometry/squircle.ts:16-95`):
  - `r = clamp(radius, 0, min(w,h)/2)`; при `r <= 0.5` — обычный прямоугольник;
  - иначе многоугольник: старт `(x+r, y)`, затем 4 угла по 10 отрезков: верх-право (углы −π/2→0), низ-право (0→π/2), низ-лево (π/2→π), верх-лево (π→3π/2);
  - точка угла: `(cx + sgn(cosθ)·r·|cosθ|^(2/4.5), cy + sgn(sinθ)·r·|sinθ|^(2/4.5))`, центры углов отстоят на `r` от сторон.
- Превью v1 рисовало иначе **[не переносить]**: CSS `border-radius` (окружность) и фиксированная CSS-тень `0 2px 7px rgba(0,0,0,0.72)` (`VideoPlayback.tsx:2719-2754`). В v2 один рендер с формулами экспорта.

## 7. Активное слово (`captionLayout.ts:451-514`, `captionStyle.ts:50-57`)
```
active = первый индекс со start <= t < end
if нет: k = первый индекс с t < start; active = (k нет) ? n-1 : clamp(k-1, 0, n-1)
state[i] = i < active ? "spoken" : i == active ? "active" : "upcoming"
progress = clamp01((t - active.start) / max(1, active.end - active.start))
```
- **В v1 подсветка выключена.** `getCaptionWordVisualState` всегда возвращает `{isInactive:false, opacity:1}`, потому что тайминги слов ненадёжны. Все слова рисуются `textColor`, `inactiveTextColor` (#A3A3A3) хранится, но не используется.
- `progress` считается, но нигде не применяется.
- Код рендера готов к схеме «неактивное слово → `inactiveTextColor`». Какие состояния считать неактивными, в v1 не определено **[вопрос]** 6.
- В v2 с DTW подсветку включаем по SCOPE: цвета активного (`textColor`) и неактивного (`inactiveTextColor`) слова.

## 8. Анимации (`captionLayout.ts:61-64, 219-253, 515-552`)
```
ENTER = EXIT = 180 ms; кривая линейная (без easing)
pageStart, pageEnd — видимой страницы (§6.3)
contentEnd = max(pageStart, max end слов на странице)
willDisappear = style != none && !covered(contentEnd + 1)      # после страницы пауза ≥ 500 ms или конец
exitAnchor = willDisappear ? contentEnd : pageEnd
enter = clamp01((t - pageStart)/180); exit = clamp01((exitAnchor - t)/180); v = min(enter, exit)
none: opacity 1;              ty 0;          scale 1
fade: opacity 0.3 + 0.7v;     ty 0;          scale 1
rise: opacity 0.25 + 0.75v;   ty (1-v)*18;   scale 0.985 + 0.015v
pop:  opacity 0.35 + 0.65v;   ty (1-v)*8;    scale 0.94 + 0.06v    # и для любого неизвестного значения
if willDisappear: opacity *= exit                                  # уходит до 0, а не до «пола»
```
- `ty` — вниз, в пикселях холста.
- Смена страницы внутри непрерывной речи заново проигрывает вход: прозрачность падает к «полу» и поднимается.
- После `contentEnd` и до конца покрытия субтитр с `willDisappear` уже прозрачен.
- 18 и 8 px в v1 абсолютные, без масштаба: в 4K сдвиг визуально меньше, чем в 720p **[вопрос]** 7.

## 9. Правка реплик

Операции из списка-панели, превью и таймлайна (`captionOps.ts`, `captionEditing.ts`, `hooks/useCaptionCommands.ts`). Перед любой операцией открытая правка на превью отменяется. Список после операций сортируется по `(start, end)`.

- **Текст.**
  - Нормализация `trim` и схлопывание пробелов в один, переводы строк теряются (`captionEditing.ts:20-22`). Пустой текст — правка не применяется.
  - Реплика со словами: все её слова заменяются токенами нового текста с **равномерным** временем в `[min word.start, max word.end]` (`:24-56`). Настоящие тайминги слов теряются, даже если поправлена одна буква **[вопрос]** 8.
  - Равномерное время токена `i` из `N`: `s_i = clamp(round(S + D*i/N), S, E-1)`, `e_i = min(E, max(s_i+1, i==N-1 ? E : round(S + D*(i+1)/N)))`.
  - Реплика без слов меняет только `text`.
- **Правка на превью** (двойной клик; Enter — сохранить, Esc — отмена; перевод строки от Shift+Enter схлопывается в пробел).
  - Цель — слова видимой страницы, они могут быть из нескольких реплик.
  - Токены делятся между репликами (`captionEditing.ts:186-241`). Если токенов меньше, чем реплик, по одному токену получают самые длинные (по длительности целевых слов), остальные получают 0. Иначе каждой 1 + `floor(dur_i/Σdur * (N - k))`, а остаток раздаётся по наибольшей дробной части.
  - Слова вне цели сохраняются. Реплика может остаться с пустым текстом и не удаляется **[не переносить]**.
- **Тайминг** (`captionOps.ts:87-127`).
  - `start' = max(0, round(start))`, `end' = max(start'+1, round(end))`. Если есть слова, `end' = max(end', start' + nWords)` (каждому слову ≥ 1 ms).
  - Слова масштабируются линейно: `s = S' + (s_old - S)*k`, где `k = (E'-S')/(E-S)`. Затем `s = min(E'-1, max(cursor, round(s)))`, `e = min(E', max(s+1, round(e)))`, `cursor = e`.
  - Соседи не учитываются, перекрытия допустимы.
  - Ввод в панели — формат `m:ss.mmm`, разбор `^(?:(\d+):)?(\d{1,2})(?:\.(\d{1,3}))?$`, миллисекунды дополняются нулями справа (`5` → 500). Неверный ввод или `end <= start` откатываются (`CaptionListPanel.tsx:24-42, 96-107`).
- **Разделить** (`captionOps.ts:129-175`).
  - Нужно ≥ 2 слов. Реплика без слов получает равномерные слова из текста.
  - Точка — середина зазора `(w[i-1].end + w[i].start)/2`, ближайшая к указателю `at = clamp(playhead, cue.start, cue.end)`. При равенстве берётся более ранняя.
  - Левая часть: id прежний, `[cue.start, last left word end]`. Правая: новый id `caption-<uuid>`, `[first right word start, cue.end]`. Обе получают слова.
- **Объединить** (`:177-211`).
  - Только соседние в отсортированном списке (в UI — «с следующей»).
  - id левой; время `[left.start, right.end]`; слова — обе нормализованные последовательности, отсортированные по `start`, у всех кроме первого `leadingSpace = true`.
- **Удалить** — фильтр по id, выделение снимается.
- **Очистить все** — `cues = []`, `enabled = false` (`useCaptionCommands.ts:194-197`). Подтверждения нет.
- **Показать/скрыть** — `enabled`.
- **Добавить** (призрак или клик на дорожке, `timeline/hooks/actions/useTimelineCaptionActions.ts:8-61`).
  - Длина по умолчанию 1500 ms.
  - Запрещено, если указатель внутри существующей реплики.
  - `end = min(start+1500, конец таймлайна, начало следующей реплики)`.
  - Время переводится в source через клип под началом: `src = clipSrcStart + (clamp(t, clip.start, clip.end) - clip.start)*speed`, затем `round`.
  - Текст пустой и слов нет (`captionOps.ts:28-41`).
- **Таймлайн.**
  - Реплика показывается в каждом клипе, где есть её source-время: `tl = clip.start + (max(cue.start, srcStart) - srcStart)/speed`. Аналогично для конца, по пересечению с клипом (`captionTimeline.ts:18-39`).
  - При перетаскивании края фрагмента неизменённый край сохраняет исходное source-время реплики (`:41-51`).

## 10. Не переносить (сводка)
- Поддельный прогресс генерации.
- Повтор whisper без `-ojf`, разбор SRT и обе ветки «без слов».
- Потеря слов всего сегмента из-за одного токена нулевой длины.
- Глобальное листание страниц по всем репликам.
- CSS-отрисовка превью: круглый радиус, фиксированная тень.
- Пустые реплики, которые исчезают после перезагрузки.
- `adelay` для sidecar.
- Английский список аббревиатур как единственный.

## 11. Вопросы владельцу
1. Какие 4 шрифта с кириллицей в v2 и какой из них по умолчанию? В v1: Inter, Arial, Georgia, monospace, а по умолчанию SF Pro, которого нет в списке.
2. Оставить ли связь «размер шрифта растёт вместе с шириной» (`fs ∝ maxWidth`) или сделать ширину независимой?
3. При «строк» > 1 показывать на странице только строки одной реплики? Рекомендация: да.
4. Если для субтитров выбраны больше двух дорожек, в каком порядке приоритета их сливать? В v1 только «микрофон выше системного звука».
5. Добавить русские сокращения («т.е.», «т.д.», «т.п.», «г.»), чтобы они не рвали реплику?
6. Неактивный цвет — у ещё не сказанных слов, у уже сказанных или у всех, кроме активного?
7. Масштабировать сдвиг анимаций rise и pop (18 и 8 px) вместе со шрифтом?
8. При правке текста сохранять тайминги неизменённых слов (сопоставление по словам) или, как в v1, распределять время равномерно?
