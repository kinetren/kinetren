# Sidecar индекс-БД медиа (`<project>.ktindex.db`)

SQLite (rusqlite, bundled). Лежит рядом с файлом проекта → переносится вместе с ним. Является **кэшем**: потеря/повреждение не является ошибкой — БД пересоздаётся фоновым воркером. В git не коммитится.

Назначение: убрать любые декоды по требованию UI (волноформы, тумбнейлы — дефект m в `porting-notes.md`) и любое сканирование файлов при открытии проекта (probe, seek-индексы — дефект j).

## Схема (schema_version = 2)

```sql
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);           -- schema_version

CREATE TABLE media (
  media_id     INTEGER PRIMARY KEY,
  path         TEXT NOT NULL,      -- как в проекте (относительный)
  size         INTEGER NOT NULL,   -- байты, быстрая проверка
  mtime_ns     INTEGER NOT NULL,   -- быстрая проверка
  xxhash       BLOB NOT NULL,      -- xxh3-128 содержимого, точная проверка
  probe_json   TEXT NOT NULL,      -- MediaInfo: длительность, потоки, fps, sr, кодеки
  built_at     INTEGER NOT NULL,
  complete     INTEGER NOT NULL    -- 0 пока воркер не закончил все секции
);

CREATE TABLE packet_index (        -- пакетный индекс FfmpegSource (тест 34):
  media_id INTEGER,                -- открытие без полного скан-прохода по файлу
  stream   TEXT,                   -- 'audio' (pts аудио-пакетов) | 'video' (pts кейфреймов)
  data     BLOB,                   -- i64 LE, отсортированы; pts в timebase потока
  PRIMARY KEY (media_id, stream)
);
-- v1 имела audio_seek/video_keyframes (строка на пакет) — не писались ни разу;
-- заменены blob-таблицей в v2.

CREATE TABLE audio_peaks (         -- peak-пирамида: level 0 = блок 256 сэмплов, level n = ×2^n
  media_id INTEGER, level INTEGER, channel INTEGER,
  data BLOB,                       -- interleaved (min:i16, max:i16) на блок
  PRIMARY KEY (media_id, level, channel)
);

CREATE TABLE thumbnails (          -- полоса по лестнице шагов THUMB_STEP_LADDER
  media_id INTEGER, t_ticks INTEGER,
  jpeg BLOB,                       -- 320×180, JPEG q80 (THUMB_W/THUMB_H/THUMB_JPEG_QUALITY)
  PRIMARY KEY (media_id, t_ticks)
);
```

### Аддитивные таблицы (создаются `IF NOT EXISTS` при каждом открытии)

Бамп `schema_version` сносит пакетный индекс, пики и тумбы всех медиа проекта —
слишком дорого ради одной таблицы, поэтому новые таблицы добавляются
аддитивно и появляются и в уже существующих БД.

```sql
CREATE TABLE poster (media_id INTEGER PRIMARY KEY, t_ticks INTEGER, score INTEGER);
CREATE TABLE loudness (media_id INTEGER PRIMARY KEY, lufs_x100, dbtp_x100, lra_x100);
CREATE TABLE sync_offset (a_media, b_media, ticks, conf_x100, PRIMARY KEY (a_media, b_media));

-- Превью ВЛОЖЕННОЙ ЛЕНТЫ (Р.58, п.463). У ленты нет файла, и `media` ей не
-- дом: ключ — СОДЕРЖИМОЕ. seq — id ленты (переживает save→load), hash —
-- редакция (kt_engine::content_hash), media_fp — отпечаток файлов дерева
-- ленты (путь, размер, mtime; генераторы — по строке пути). Оба u64 лежат
-- как i64 побитово. Одна версия на ленту: новая запись вытесняет прежнюю.
CREATE TABLE reel_strip (
  seq INTEGER PRIMARY KEY, hash INTEGER NOT NULL, media_fp INTEGER NOT NULL,
  step INTEGER NOT NULL,           -- шаг кадров полосы (тики), ступень THUMB_STEP_LADDER
  end_ticks INTEGER NOT NULL,      -- длина ленты
  n INTEGER NOT NULL               -- число слотов полосы (кадры ≤ MAX_FRAMES_PER_REEL)
);
CREATE TABLE reel_frame (          -- кадр i*step; отсутствующая строка = кадра не было
  seq INTEGER, idx INTEGER, jpeg BLOB,   -- 320×180, JPEG q80 — как thumbnails
  PRIMARY KEY (seq, idx)
);
CREATE TABLE reel_peaks (          -- та же пирамида и тот же блоб, что audio_peaks
  seq INTEGER, level INTEGER, channel INTEGER, data BLOB,
  PRIMARY KEY (seq, level, channel)
);
```

`purge_media` таблицы `reel_*` НЕ трогает: подмену файла ловит `media_fp`
(строка с другим отпечатком просто не находится, `reel_preview(seq, hash,
media_fp)` → `None`, и лента рендерится заново). Ленты, которых в документе
больше нет, убирает `prune_reels(keep)` — задача превью зовёт её после
записи. Тест 639.

## Правила инвалидации (тест 16)

1. Открытие медиа: ищем запись по path. Совпали size+mtime → доверяем. mtime сменился → пересчитываем xxhash: совпал → обновляем mtime, переиспользуем; не совпал → перестраиваем **только** записи этого media_id.
2. `schema_version` в БД ≠ текущей → полная перестройка БД.
3. Ошибка открытия/чтения БД (битый файл, чужой формат) → удалить и пересоздать, не падать.
4. Пользовательский форс («переиндексировать») → перестройка выбранного файла.
5. `complete=0` (воркер был прерван) → достроить недостающие секции.

## Кто пишет

Один фоновый index-воркер на проект; декодирует собственным короткоживущим FfmpegSource (никогда — декодерами воспроизведения). Пишет секциями в транзакциях (probe → seek-индексы → peaks → thumbnails), UI показывает плейсхолдеры до готовности секции. Blob'ы — по-уровнево (peaks) и по-кадрово (thumbnails), не одним куском.

`reel_*` пишет задача превью ленты (`kt-app::reel_preview`, поток
`reel-preview`) — после офлайн-рендера, одной транзакцией, ПОСЛЕ пиков (отменённая
на аудио задача полосы без огибающей не оставляет). Читает её она же, до любого
декода. UI-поток БД не трогает (инвариант 8/12).

`packet_index` дополнительно пишет **любой** `FfmpegSource::open_indexed` после свежего скана (первое открытие файла в проекте) — поэтому уже второе открытие того же файла (перезапуск, добавление в плеер, тумбы) обходится без скан-прохода. Соединения короткоживущие, WAL + busy_timeout 5 с покрывают конкуренцию потоков.
