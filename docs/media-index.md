# Sidecar индекс-БД медиа (`<project>.ktindex.db`)

SQLite (rusqlite, bundled). Лежит рядом с файлом проекта → переносится вместе с ним. Является **кэшем**: потеря/повреждение не является ошибкой — БД пересоздаётся фоновым воркером. В git не коммитится.

Назначение: убрать любые декоды по требованию UI (волноформы, тумбнейлы — дефект m в `porting-notes.md`) и любое сканирование файлов при открытии проекта (probe, seek-индексы — дефект j).

## Схема (schema_version = 1)

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

CREATE TABLE audio_seek (          -- sample-accurate seek: декод вперёд от ближайшей записи
  media_id INTEGER, pkt_pos INTEGER, pkt_pts INTEGER, first_sample INTEGER,
  PRIMARY KEY (media_id, first_sample)
);

CREATE TABLE video_keyframes (     -- keyframe_before() без обращения к контейнеру
  media_id INTEGER, pts_ticks INTEGER,     -- тики 90 kHz
  PRIMARY KEY (media_id, pts_ticks)
);

CREATE TABLE audio_peaks (         -- peak-пирамида: level 0 = блок 256 сэмплов, level n = ×2^n
  media_id INTEGER, level INTEGER, channel INTEGER,
  data BLOB,                       -- interleaved (min:i16, max:i16) на блок
  PRIMARY KEY (media_id, level, channel)
);

CREATE TABLE thumbnails (          -- полоса ~1–2 c по keyframes
  media_id INTEGER, t_ticks INTEGER,
  jpeg BLOB,                       -- ~160px по ширине
  PRIMARY KEY (media_id, t_ticks)
);
```

## Правила инвалидации (тест 16)

1. Открытие медиа: ищем запись по path. Совпали size+mtime → доверяем. mtime сменился → пересчитываем xxhash: совпал → обновляем mtime, переиспользуем; не совпал → перестраиваем **только** записи этого media_id.
2. `schema_version` в БД ≠ текущей → полная перестройка БД.
3. Ошибка открытия/чтения БД (битый файл, чужой формат) → удалить и пересоздать, не падать.
4. Пользовательский форс («переиндексировать») → перестройка выбранного файла.
5. `complete=0` (воркер был прерван) → достроить недостающие секции.

## Кто пишет

Один фоновый index-воркер на проект; декодирует собственным короткоживущим FfmpegSource (никогда — декодерами воспроизведения). Пишет секциями в транзакциях (probe → seek-индексы → peaks → thumbnails), UI показывает плейсхолдеры до готовности секции. Blob'ы — по-уровнево (peaks) и по-кадрово (thumbnails), не одним куском.
