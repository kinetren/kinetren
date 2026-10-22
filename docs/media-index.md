# Media index DB sidecar (`<project>.ktindex.db`)

SQLite (rusqlite, bundled). It lives next to the project file → it moves together with it. It is a **cache**: loss or corruption is not an error — the DB is rebuilt by the background worker. It is not committed to git.

Purpose: remove any on-demand decodes for the UI (waveforms, thumbnails — defect m in `porting-notes.md`) and any scanning of files on project open (probe, seek indexes — defect j).

## Schema (schema_version = 2)

```sql
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);           -- schema_version

CREATE TABLE media (
  media_id     INTEGER PRIMARY KEY,
  path         TEXT NOT NULL,      -- as in the project (relative)
  size         INTEGER NOT NULL,   -- bytes, quick check
  mtime_ns     INTEGER NOT NULL,   -- quick check
  xxhash       BLOB NOT NULL,      -- xxh3-128 of the content, exact check
  probe_json   TEXT NOT NULL,      -- MediaInfo: duration, streams, fps, sr, codecs
  built_at     INTEGER NOT NULL,
  complete     INTEGER NOT NULL    -- 0 until the worker has finished all sections
);

CREATE TABLE packet_index (        -- packet index of FfmpegSource (test 34):
  media_id INTEGER,                -- opening without a full scan pass over the file
  stream   TEXT,                   -- 'audio' (pts of audio packets) | 'video' (pts of keyframes)
  data     BLOB,                   -- i64 LE, sorted; pts in the stream timebase
  PRIMARY KEY (media_id, stream)
);
-- v1 had audio_seek/video_keyframes (a row per packet) — never written; replaced by the blob table in v2.

CREATE TABLE audio_peaks (         -- peak pyramid: level 0 = block of 256 samples, level n = ×2^n
  media_id INTEGER, level INTEGER, channel INTEGER,
  data BLOB,                       -- interleaved (min:i16, max:i16) per block
  PRIMARY KEY (media_id, level, channel)
);

CREATE TABLE thumbnails (          -- strip along the THUMB_STEP_LADDER steps
  media_id INTEGER, t_ticks INTEGER,
  jpeg BLOB,                       -- 320×180, JPEG q80 (THUMB_W/THUMB_H/THUMB_JPEG_QUALITY)
  PRIMARY KEY (media_id, t_ticks)
);
```

### Additive tables (created with `IF NOT EXISTS` on every open)

A bump of `schema_version` would wipe the packet index, peaks and thumbnails of all media in the project — too expensive for the sake of one table, so new tables are added additively and also appear in already existing DBs.

```sql
CREATE TABLE poster (media_id INTEGER PRIMARY KEY, t_ticks INTEGER, score INTEGER);
CREATE TABLE loudness (media_id INTEGER PRIMARY KEY, lufs_x100, dbtp_x100, lra_x100);
CREATE TABLE sync_offset (a_media, b_media, ticks, conf_x100, PRIMARY KEY (a_media, b_media));

-- Preview of a NESTED REEL (R.58, item 463). A reel has no file, and `media` is not
-- its home: the key is the CONTENT. seq — id of the reel (survives save→load), hash —
-- the revision (kt_engine::content_hash), media_fp — the fingerprint of the reel
-- tree's files (path, size, mtime; generators — by the path string). Both u64 are stored
-- as i64 bit-for-bit. One version per reel: a new record evicts the previous one.
CREATE TABLE reel_strip (
  seq INTEGER PRIMARY KEY, hash INTEGER NOT NULL, media_fp INTEGER NOT NULL,
  step INTEGER NOT NULL,           -- frame step of the strip (ticks), a step of THUMB_STEP_LADDER
  end_ticks INTEGER NOT NULL,      -- length of the reel
  n INTEGER NOT NULL               -- number of slots of the strip (frames ≤ MAX_FRAMES_PER_REEL)
);
CREATE TABLE reel_frame (          -- frame i*step; a missing row = there was no frame
  seq INTEGER, idx INTEGER, jpeg BLOB,   -- 320×180, JPEG q80 — as thumbnails
  PRIMARY KEY (seq, idx)
);
CREATE TABLE reel_peaks (          -- the same pyramid and the same blob as audio_peaks
  seq INTEGER, level INTEGER, channel INTEGER, data BLOB,
  PRIMARY KEY (seq, level, channel)
);
```

`purge_media` does NOT touch the `reel_*` tables: a file substitution is caught by `media_fp`
(a row with a different fingerprint is simply not found, `reel_preview(seq, hash,
media_fp)` → `None`, and the reel is rendered anew). Reels that are no longer in the
document are removed by `prune_reels(keep)` — the preview task calls it after
writing. Test 639.

## Invalidation rules (test 16)

1. Opening media: look up the record by path. If size+mtime match → trust it. If mtime has changed → recompute the xxhash: matches → update mtime, reuse; does not match → rebuild **only** the records of this media_id.
2. `schema_version` in the DB ≠ current → full rebuild of the DB.
3. An error opening/reading the DB (corrupt file, foreign format) → delete and recreate it, do not crash.
4. A user-forced rebuild ("reindex") → rebuild of the selected file.
5. `complete=0` (the worker was interrupted) → finish the missing sections.

## Who writes

One background index worker per project; it decodes with its own short-lived FfmpegSource (never with the playback decoders). It writes by sections in transactions (probe → seek indexes → peaks → thumbnails); the UI shows placeholders until a section is ready. Blobs are written per level (peaks) and per frame (thumbnails), not as one chunk.

The `reel_*` tables are written by the reel preview task (`kt-app::reel_preview`, thread `reel-preview`) — after the offline render, in one transaction, AFTER the peaks (a strip task cancelled on audio leaves no envelope behind). It also reads them, before any decode. The UI thread does not touch the DB (invariants 8/12).

`packet_index` is additionally written by **any** `FfmpegSource::open_indexed` after a fresh scan (the first opening of a file in the project) — therefore a second opening of the same file (restart, adding to the player, thumbnails) goes without a scan pass. The connections are short-lived; WAL + busy_timeout 5 s cover the contention of threads.
