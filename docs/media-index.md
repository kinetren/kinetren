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

`packet_index` дополнительно пишет **любой** `FfmpegSource::open_indexed` после свежего скана (первое открытие файла в проекте) — поэтому уже второе открытие того же файла (перезапуск, добавление в плеер, тумбы) обходится без скан-прохода. Соединения короткоживущие, WAL + busy_timeout 5 с покрывают конкуренцию потоков.
