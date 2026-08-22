# Архитектура движка (src/)

Статус: проектная спецификация; поддерживать в актуальном состоянии по мере реализации этапов M1–M7 (см. `porting-notes.md`).

## Потоки и владение

```
 cpal-колбэк ◄── SPSC-кольцо (rtrb, 2–4 burst'а) ── аудио-render-поток ──► RenderGraph ──get──► BurstCache
      │ счётчик потреблённых кадров                      ▲ владеет FxStateStore                     ▲ insert
      ▼                                                  │                                          │
  Transport (атомики: playhead, state, epoch) ◄──────────┘                     MediaWorker-поток ×N (каждый владеет своим FfmpegSource)
      ▲    ▲                                                                              ▲
 UI-поток (egui): отрисовка, видео-запросы, правки модели ──► Prefetch-поток ─────────────┘
                                                              Index-воркер (peak-пирамида, тумбнейлы) — отдельный короткоживущий FfmpegSource
```

Правила владения:
- `FfmpegSource` эксклюзивно принадлежит своему `MediaWorker`; клиенты общаются только запросами (`DecodeRequest {key, prio, generation}`) через приоритетную очередь. Приоритеты: `Playback > NearPrefetch > FarPrefetch > Thumbnail`; при равных — вперёд по курсору декодера (исключает ping-pong seek). Seek транспорта бампает generation, устаревшие запросы дропаются.
- `FxStateStore` (HashMap<(ClipId, FxSlot), Box<dyn AudioFx>>) — только у аудио-render-потока. Без локов.
- `BurstCache` — общий, но это единственная разделяемая структура (внутренний мьютекс/шардинг). Ключ: `(source_id, kind{Audio | Video{quality}}, idx)`. Coverage — interval-set, поддерживаемый на insert/evict, отдаётся UI как список диапазонов.
- `Transport` — атомики; playhead продвигает только аудио-render-поток на ровно потреблённое колбэком число кадров.

## Поток данных

Воспроизведение: UI/транспорт → RenderGraph pull по playhead → BurstCache (hit) или MediaWorker (miss, но playback не ждёт: тишина/последний кадр + underrun-флаг) → per-clip FX (со state из FxStateStore) → микс (equal-power кроссфейд для 2 перекрывающихся клипов одной дорожки, 1/N для 3+; gain на клип) → SPSC → cpal. Видео: UI-поток по playhead запрашивает кадр по сетке fps, композитор alpha-over по z (индекс дорожки), egui-текстура (double-buffer).

Экспорт: тот же RenderGraph, clock = «по готовности», Quality = разрешение проекта; rgb24 → stdin ffmpeg, s16le → fifo; ffmpeg муксит на лету. Промежуточных файлов нет.

Prefetch: идемпотентный цикл — wanted-ключи (lookahead playhead ~2 c, видимый диапазон, голова loop-региона с pin в кэше) minus coverage() → submit. Состояния между тиками нет.

## Sidecar индекс-БД (`<project>.ktindex.db`, SQLite)

По media_id (ключ идентичности: path + size + mtime + xxhash + index_version): probe-данные; audio seek-индекс (позиция пакета → индекс первого сэмпла) для sample-accurate seek; video keyframe-индекс; peak-пирамида аудио (мин/макс, блок 256 сэмплов, mip-уровни); тумбнейл-полоса (~1–2 c, keyframes, ~160px JPEG). Перестройка — только затронутого файла: несовпадение size/mtime → сверка xxhash → перестройка; бамп index_version → полная; повреждённая БД → пересоздание без падения. Схема — `media-index.md`.

## Аудио-FX

- Трейт: `set_params` (сглаживание one-pole ~20 мс, буферы не трогает), `process` (f32 48k/2ch in/out), `tail() -> Tick`, `reset()` (только при seek).
- slowedreverb = rubato `SincFixedOut` (varispeed, дробный fixed-point курсор источника — burst'ы не пропускаются и не дублируются) + freeverb (8 comb + 4 allpass на канал).
- Хвосты: активное аудио-окно клипа расширяется на Σ tail цепочки — реверб дозванивает за границей клипа.
- Seek: reset всех FX + pre-roll `max(tail, 100 мс)` в мусор → детерминированный прогрев.
- Пост-FX аудио не кэшируется (FX дешевле декода на порядки; кэш — пре-FX).

## Ключевые решения и «почему так»

| Решение | Почему |
|---|---|
| 90 kHz тики | делятся на 24/25/30/60 fps и на шаг 1920 тиков аудио-burst; наследие старого движка, оно было удачным |
| f32 48k/2ch везде | одна система координат для FX/микса; источники приводятся на границе декода (swr) |
| Кэш пре-FX, не пост-FX | инвалидация по параметрам FX вернула бы лживую бухгалтерию; FX ≪ 1 мс/burst |
| CPU-композитинг v1 | один код для превью и экспорта, golden-тесты; wgpu — только если замеры M4 потребуют |
| Экспорт пайпом в ffmpeg-бинарник | кодирование через API не нужно; backpressure бесплатно; ноль temp-файлов |
| Без tokio | pull-движок на выделенных потоках с очередями; async ничего не даёт |
| rsmpeg заперт в kt-media | смена FFmpeg-мажора или бэкенда не трогает остальной код (трейт MediaSource) |
