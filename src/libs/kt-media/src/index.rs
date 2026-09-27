//! Sidecar индекс-БД медиа (`<project>.ktindex.db`, SQLite).
//! Схема и правила инвалидации — docs/media-index.md. Это КЭШ: потеря или
//! повреждение не ошибка, БД пересоздаётся. Решает дефекты m (шторм декодов
//! тумбнейлов/волноформ при скролле/зуме) и j (пересканирование файлов на
//! каждый запуск) — см. docs/porting-notes.md.

use crate::{MediaSource, PEAK_BLOCK_FRAMES};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};

// v2: audio_seek/video_keyframes (по строке на пакет, никогда не писались)
// заменены на packet_index с блобами i64 LE — персист пакетного индекса
// FfmpegSource (быстрое открытие больших файлов). Бамп = пересоздание БД.
pub const SCHEMA_VERSION: i64 = 2;

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Что произошло с записью медиа при открытии.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaEntryStatus {
    /// Записи актуальны (size/mtime совпали, либо mtime сменился, но хэш тот же).
    Reused,
    /// Содержимое изменилось или записи не было — секции надо строить заново.
    Rebuilt,
}

pub struct MediaIndexDb {
    conn: Connection,
    path: PathBuf,
}

struct FileIdentity {
    size: i64,
    mtime_ns: i64,
}

fn file_identity(path: &Path) -> std::io::Result<FileIdentity> {
    let md = std::fs::metadata(path)?;
    let mtime = md.modified()?;
    let ns = mtime.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as i64).unwrap_or(0);
    Ok(FileIdentity { size: md.len() as i64, mtime_ns: ns })
}

fn hash_file(path: &Path) -> std::io::Result<[u8; 16]> {
    use std::io::Read;
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.digest128().to_le_bytes())
}

impl MediaIndexDb {
    /// Открыть/создать. Битая или чужая БД молча пересоздаётся (правило 3
    /// docs/media-index.md) — это кэш, падать из-за него нельзя.
    pub fn open(path: &Path) -> Result<MediaIndexDb, IndexError> {
        match Self::try_open(path) {
            Ok(db) => Ok(db),
            Err(_) => {
                let _ = std::fs::remove_file(path);
                Self::try_open(path)
            }
        }
    }

    fn try_open(path: &Path) -> Result<MediaIndexDb, IndexError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // БД открывают несколько потоков (index-worker, тумбы, открытие
        // источников) — короткие записи не должны падать по SQLITE_BUSY.
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        let version: Option<i64> = conn
            .query_row("SELECT value FROM meta WHERE key='schema_version'", [], |r| r.get(0))
            .optional()
            .unwrap_or(None);
        match version {
            Some(v) if v == SCHEMA_VERSION => {}
            Some(_) => {
                // бамп схемы → полная перестройка (правило 2)
                drop(conn);
                std::fs::remove_file(path)?;
                return Self::try_open(path);
            }
            None => {
                conn.execute_batch(
                    "BEGIN;
                     CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT);
                     CREATE TABLE IF NOT EXISTS media (
                       media_id   INTEGER PRIMARY KEY,
                       path       TEXT NOT NULL UNIQUE,
                       size       INTEGER NOT NULL,
                       mtime_ns   INTEGER NOT NULL,
                       xxhash     BLOB NOT NULL,
                       probe_json TEXT NOT NULL DEFAULT '',
                       built_at   INTEGER NOT NULL DEFAULT 0,
                       complete   INTEGER NOT NULL DEFAULT 0
                     );
                     CREATE TABLE IF NOT EXISTS packet_index (
                       media_id INTEGER, stream TEXT, data BLOB,
                       PRIMARY KEY (media_id, stream)
                     );
                     CREATE TABLE IF NOT EXISTS audio_peaks (
                       media_id INTEGER, level INTEGER, channel INTEGER, data BLOB,
                       PRIMARY KEY (media_id, level, channel)
                     );
                     CREATE TABLE IF NOT EXISTS thumbnails (
                       media_id INTEGER, t_ticks INTEGER, jpeg BLOB,
                       PRIMARY KEY (media_id, t_ticks)
                     );
                     COMMIT;",
                )?;
                conn.execute(
                    "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', ?1)",
                    params![SCHEMA_VERSION.to_string()],
                )?;
            }
        }
        // Аддитивные таблицы создаются ПРИ КАЖДОМ открытии (IF NOT EXISTS):
        // так новая таблица появляется и в уже существующих БД, не требуя
        // бампа SCHEMA_VERSION (бамп сносит packet_index/peaks/тумбы всех
        // медиа проекта — слишком дорого ради одной таблицы).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS poster (
               media_id INTEGER PRIMARY KEY,
               t_ticks  INTEGER NOT NULL,
               score    INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS thumb_meta (
               media_id INTEGER PRIMARY KEY,
               w        INTEGER NOT NULL,
               h        INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS loudness (
               media_id  INTEGER PRIMARY KEY,
               lufs_x100 INTEGER NOT NULL,
               dbtp_x100 INTEGER NOT NULL,
               lra_x100  INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS sync_offset (
               a_media  INTEGER NOT NULL,
               b_media  INTEGER NOT NULL,
               ticks    INTEGER NOT NULL,
               conf_x100 INTEGER NOT NULL,
               PRIMARY KEY (a_media, b_media)
             );
             CREATE TABLE IF NOT EXISTS reel_strip (
               seq       INTEGER PRIMARY KEY,
               hash      INTEGER NOT NULL,
               media_fp  INTEGER NOT NULL,
               step      INTEGER NOT NULL,
               end_ticks INTEGER NOT NULL,
               n         INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS reel_frame (
               seq INTEGER, idx INTEGER, jpeg BLOB,
               PRIMARY KEY (seq, idx)
             );
             CREATE TABLE IF NOT EXISTS reel_peaks (
               seq INTEGER, level INTEGER, channel INTEGER, data BLOB,
               PRIMARY KEY (seq, level, channel)
             );",
        )?;
        Ok(MediaIndexDb { conn, path: path.to_path_buf() })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Проверка идентичности файла и инвалидация ТОЛЬКО его записей
    /// (правило 1 docs/media-index.md). Возвращает (media_id, статус).
    pub fn ensure_media(&mut self, file: &Path) -> Result<(i64, MediaEntryStatus), IndexError> {
        let ident = file_identity(file)?;
        let key = file.to_string_lossy().to_string();
        let row: Option<(i64, i64, i64, Vec<u8>)> = self
            .conn
            .query_row(
                "SELECT media_id, size, mtime_ns, xxhash FROM media WHERE path=?1",
                params![key],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;

        if let Some((id, size, mtime_ns, stored_hash)) = row {
            if size == ident.size && mtime_ns == ident.mtime_ns {
                return Ok((id, MediaEntryStatus::Reused)); // быстрый путь
            }
            let hash = hash_file(file)?;
            if size == ident.size && stored_hash == hash {
                // содержимое то же, только mtime — обновляем и переиспользуем
                self.conn.execute(
                    "UPDATE media SET mtime_ns=?1 WHERE media_id=?2",
                    params![ident.mtime_ns, id],
                )?;
                return Ok((id, MediaEntryStatus::Reused));
            }
            // содержимое изменилось: чистим ТОЛЬКО этот media_id
            self.purge_media(id)?;
            self.conn.execute(
                "UPDATE media SET size=?1, mtime_ns=?2, xxhash=?3, complete=0 WHERE media_id=?4",
                params![ident.size, ident.mtime_ns, hash.as_slice(), id],
            )?;
            return Ok((id, MediaEntryStatus::Rebuilt));
        }

        let hash = hash_file(file)?;
        self.conn.execute(
            "INSERT INTO media(path, size, mtime_ns, xxhash) VALUES(?1, ?2, ?3, ?4)",
            params![key, ident.size, ident.mtime_ns, hash.as_slice()],
        )?;
        Ok((self.conn.last_insert_rowid(), MediaEntryStatus::Rebuilt))
    }

    /// Таблицы ЛЕНТ (`reel_*`, Р.58) здесь НЕ трогаются намеренно: они
    /// ключуются содержимым (лента, редакция, отпечаток медиа), а не
    /// `media_id`, и подмену файла ловит сам отпечаток — см. `reel_preview`.
    fn purge_media(&self, media_id: i64) -> Result<(), IndexError> {
        for table in ["packet_index", "audio_peaks", "thumbnails", "poster", "loudness"] {
            self.conn
                .execute(&format!("DELETE FROM {table} WHERE media_id=?1"), params![media_id])?;
        }
        // Сдвиг синхронизации — про ПАРУ, и однородное `WHERE media_id=?1`
        // его не чистит: файл, оказавшийся не тем, делает недействительными
        // обе стороны каждой своей пары (Р.49, п.369).
        self.conn
            .execute("DELETE FROM sync_offset WHERE a_media=?1 OR b_media=?1", params![media_id])?;
        Ok(())
    }

    // ---- результаты офлайн-анализа (Р.49, п.369) --------------------------
    //
    // Целые ×100, а не f64: round-trip обязан быть байт-точным, иначе
    // «пересчитали то же самое» давало бы другое число в последнем знаке, и
    // отличить «пересчитали» от «изменилось» стало бы нечем.

    pub fn set_loudness(
        &self,
        media_id: i64,
        lufs: f64,
        dbtp: f64,
        lra: f64,
    ) -> Result<(), IndexError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO loudness(media_id, lufs_x100, dbtp_x100, lra_x100) \
             VALUES(?1, ?2, ?3, ?4)",
            params![
                media_id,
                (lufs * 100.0).round() as i64,
                (dbtp * 100.0).round() as i64,
                (lra * 100.0).round() as i64
            ],
        )?;
        Ok(())
    }

    /// (LUFS, dBTP, LRA) или `None`, если ещё не мерили.
    pub fn loudness(&self, media_id: i64) -> Result<Option<(f64, f64, f64)>, IndexError> {
        let mut st = self
            .conn
            .prepare("SELECT lufs_x100, dbtp_x100, lra_x100 FROM loudness WHERE media_id=?1")?;
        let mut rows = st.query(params![media_id])?;
        match rows.next()? {
            Some(r) => {
                let (a, b, c): (i64, i64, i64) = (r.get(0)?, r.get(1)?, r.get(2)?);
                Ok(Some((a as f64 / 100.0, b as f64 / 100.0, c as f64 / 100.0)))
            }
            None => Ok(None),
        }
    }

    /// Сдвиг B относительно A в тиках и уверенность 0..1. Ключ —
    /// УПОРЯДОЧЕННАЯ пара: знак сдвига от порядка зависит.
    pub fn set_sync_offset(
        &self,
        a_media: i64,
        b_media: i64,
        ticks: i64,
        conf: f64,
    ) -> Result<(), IndexError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sync_offset(a_media, b_media, ticks, conf_x100) \
             VALUES(?1, ?2, ?3, ?4)",
            params![a_media, b_media, ticks, (conf * 100.0).round() as i64],
        )?;
        Ok(())
    }

    pub fn sync_offset(
        &self,
        a_media: i64,
        b_media: i64,
    ) -> Result<Option<(i64, f64)>, IndexError> {
        let mut st = self
            .conn
            .prepare("SELECT ticks, conf_x100 FROM sync_offset WHERE a_media=?1 AND b_media=?2")?;
        let mut rows = st.query(params![a_media, b_media])?;
        match rows.next()? {
            Some(r) => {
                let (t, c): (i64, i64) = (r.get(0)?, r.get(1)?);
                Ok(Some((t, c as f64 / 100.0)))
            }
            None => Ok(None),
        }
    }

    pub fn is_complete(&self, media_id: i64) -> Result<bool, IndexError> {
        let c: i64 = self.conn.query_row(
            "SELECT complete FROM media WHERE media_id=?1",
            params![media_id],
            |r| r.get(0),
        )?;
        Ok(c != 0)
    }

    pub fn mark_complete(&self, media_id: i64) -> Result<(), IndexError> {
        self.conn.execute(
            "UPDATE media SET complete=1, built_at=strftime('%s','now') WHERE media_id=?1",
            params![media_id],
        )?;
        Ok(())
    }

    /// Построить и сохранить peak-пирамиду из аудио источника.
    /// Level 0 — блок 256 кадров; level n — блок 256·2ⁿ. Значения (min,max) i16.
    pub fn build_audio_peaks(
        &mut self,
        media_id: i64,
        source: &mut dyn MediaSource,
    ) -> Result<(), IndexError> {
        self.build_audio_peaks_observed(media_id, source, &mut |_| {})
    }

    /// То же с наблюдателем за burst'ами (Р.49, п.373): громкость материала
    /// считается на проходе, который И ТАК ИДЁТ, — второй полный декод ради
    /// LUFS на часовом исходнике это минуты.
    pub fn build_audio_peaks_observed(
        &mut self,
        media_id: i64,
        source: &mut dyn MediaSource,
        observe: &mut dyn FnMut(&crate::AudioBurst),
    ) -> Result<(), IndexError> {
        let pyramid = PeakPyramid::build_observed(source, observe);
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM audio_peaks WHERE media_id=?1", params![media_id])?;
        for (level, channels) in pyramid.levels.iter().enumerate() {
            for (ch, blocks) in channels.iter().enumerate() {
                tx.execute(
                    "INSERT INTO audio_peaks(media_id, level, channel, data) VALUES(?1,?2,?3,?4)",
                    params![media_id, level as i64, ch as i64, peaks_blob(blocks)],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // ---- превью ЛЕНТЫ на диске (Р.58, п.463) --------------------------------
    //
    // У ленты нет файла, и `media(path, size, mtime, xxhash)` ей не дом:
    // ключ — СОДЕРЖИМОЕ. `seq` — id ленты (переживает save→load, t431),
    // `hash` — редакция (`kt_engine::content_hash`), `media_fp` — отпечаток
    // файлов дерева ленты (путь, размер, mtime): редакция файла не видит,
    // и без отпечатка подменённый исходник показывал бы старую полосу.
    // Одна версия на ленту: новая запись вытесняет прежнюю целиком.

    /// Сохранить полосу кадров и пирамиду пиков ленты одной транзакцией.
    /// `frames[i]` — JPEG кадра `i * step`, `None` — кадра не было (строки
    /// нет). `hash`/`media_fp` кладутся как i64 побитово.
    #[allow(clippy::too_many_arguments)]
    /// КАДРЫ полосы — своей транзакцией, СРАЗУ после кадровой стадии
    /// (Р.67). Раньше запись шла одна и только после аудио-стадии: полоса,
    /// уже дошедшая до экрана, до диска не доживала, если задачу отменили
    /// (пауза, правка, закрытие) — и при следующем открытии проекта лента
    /// рендерилась заново («превью композиции ОПЯТЬ не сохраняются»).
    pub fn put_reel_frames(
        &mut self,
        seq: i64,
        hash: u64,
        media_fp: u64,
        step: i64,
        end_ticks: i64,
        frames: &[Option<&[u8]>],
    ) -> Result<(), IndexError> {
        let tx = self.conn.transaction()?;
        for table in ["reel_strip", "reel_frame", "reel_peaks"] {
            tx.execute(&format!("DELETE FROM {table} WHERE seq=?1"), params![seq])?;
        }
        tx.execute(
            "INSERT INTO reel_strip(seq, hash, media_fp, step, end_ticks, n) \
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![seq, hash as i64, media_fp as i64, step, end_ticks, frames.len() as i64],
        )?;
        for (idx, jpeg) in frames.iter().enumerate() {
            if let Some(j) = jpeg {
                tx.execute(
                    "INSERT INTO reel_frame(seq, idx, jpeg) VALUES(?1,?2,?3)",
                    params![seq, idx as i64, *j],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// ПИКИ той же полосы — отдельной транзакцией и ПОЗЖЕ. Пишутся только
    /// к строке с тем же ключом: разошлась редакция — пики не наши.
    /// Флага в схеме нет: «пики есть» — это непустая `reel_peaks`, и
    /// бампать `schema_version` (он сносит пакетный индекс, пики и тумбы
    /// ВСЕХ медиа проекта) ради одного булева нельзя.
    pub fn put_reel_peaks(
        &mut self,
        seq: i64,
        hash: u64,
        media_fp: u64,
        peaks: &PeakPyramid,
    ) -> Result<(), IndexError> {
        let same: Option<i64> = self
            .conn
            .query_row(
                "SELECT n FROM reel_strip WHERE seq=?1 AND hash=?2 AND media_fp=?3",
                params![seq, hash as i64, media_fp as i64],
                |r| r.get(0),
            )
            .optional()?;
        if same.is_none() {
            return Ok(()); // полосы с этим ключом нет — пики не к чему класть
        }
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM reel_peaks WHERE seq=?1", params![seq])?;
        for (level, channels) in peaks.levels.iter().enumerate() {
            for (ch, blocks) in channels.iter().enumerate() {
                tx.execute(
                    "INSERT INTO reel_peaks(seq, level, channel, data) VALUES(?1,?2,?3,?4)",
                    params![seq, level as i64, ch as i64, peaks_blob(blocks)],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Превью ленты, если на диске лежит ИМЕННО эта редакция с ТЕМИ ЖЕ
    /// файлами; любое расхождение — `None`, и ленту рендерят заново.
    pub fn reel_preview(
        &self,
        seq: i64,
        hash: u64,
        media_fp: u64,
    ) -> Result<Option<ReelPreviewRow>, IndexError> {
        let head: Option<(i64, i64, i64)> = self
            .conn
            .query_row(
                "SELECT step, end_ticks, n FROM reel_strip WHERE seq=?1 AND hash=?2 AND media_fp=?3",
                params![seq, hash as i64, media_fp as i64],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((step, end_ticks, n)) = head else { return Ok(None) };
        let mut frames: Vec<Option<Vec<u8>>> = vec![None; n.max(0) as usize];
        let mut st = self.conn.prepare("SELECT idx, jpeg FROM reel_frame WHERE seq=?1")?;
        let mut rows = st.query(params![seq])?;
        while let Some(r) = rows.next()? {
            let idx: i64 = r.get(0)?;
            if let Some(slot) = usize::try_from(idx).ok().and_then(|i| frames.get_mut(i)) {
                *slot = Some(r.get(1)?);
            }
        }
        let mut levels: Vec<Vec<Vec<(i16, i16)>>> = Vec::new();
        let mut st = self
            .conn
            .prepare("SELECT data FROM reel_peaks WHERE seq=?1 AND level=?2 AND channel=?3")?;
        'levels: for level in 0i64.. {
            let mut channels = Vec::with_capacity(AUDIO_CHANNELS);
            for ch in 0..AUDIO_CHANNELS as i64 {
                let blob: Option<Vec<u8>> =
                    st.query_row(params![seq, level, ch], |r| r.get(0)).optional()?;
                match blob {
                    Some(b) => channels.push(peaks_from_blob(&b)),
                    None => break 'levels,
                }
            }
            levels.push(channels);
        }
        // ПИКОВ МОЖЕТ НЕ БЫТЬ: кадры пишутся раньше, и полоса без
        // огибающей — законное промежуточное состояние (Р.67)
        let has_peaks = !levels.is_empty();
        Ok(Some(ReelPreviewRow {
            step,
            end_ticks,
            frames,
            peaks: PeakPyramid { levels },
            has_peaks,
        }))
    }

    /// Убрать превью лент, которых в проекте больше нет. Возвращает число
    /// убранных лент.
    pub fn prune_reels(&mut self, keep: &[i64]) -> Result<usize, IndexError> {
        let stored: Vec<i64> = {
            let mut st = self.conn.prepare("SELECT seq FROM reel_strip")?;
            let rows = st.query_map([], |r| r.get::<_, i64>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let gone: Vec<i64> = stored.into_iter().filter(|s| !keep.contains(s)).collect();
        if gone.is_empty() {
            return Ok(0);
        }
        let tx = self.conn.transaction()?;
        for seq in &gone {
            for table in ["reel_strip", "reel_frame", "reel_peaks"] {
                tx.execute(&format!("DELETE FROM {table} WHERE seq=?1"), params![seq])?;
            }
        }
        tx.commit()?;
        Ok(gone.len())
    }

    /// Сохранить пакетный индекс потока (`stream` = "audio" | "video");
    /// значения — pts в timebase потока, блоб i64 LE.
    pub fn put_packet_index(
        &self,
        media_id: i64,
        stream: &str,
        pts: &[i64],
    ) -> Result<(), IndexError> {
        let mut blob = Vec::with_capacity(pts.len() * 8);
        for &v in pts {
            blob.extend_from_slice(&v.to_le_bytes());
        }
        self.conn.execute(
            "INSERT OR REPLACE INTO packet_index(media_id, stream, data) VALUES(?1,?2,?3)",
            params![media_id, stream, blob],
        )?;
        Ok(())
    }

    /// Пакетный индекс потока, если сохранён. None = не строился
    /// (пустой Vec — валидный индекс файла без такого потока).
    pub fn packet_index(
        &self,
        media_id: i64,
        stream: &str,
    ) -> Result<Option<Vec<i64>>, IndexError> {
        let blob: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT data FROM packet_index WHERE media_id=?1 AND stream=?2",
                params![media_id, stream],
                |r| r.get(0),
            )
            .optional()?;
        Ok(blob.map(|b| {
            b.chunks_exact(8)
                .map(|c| i64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
                .collect()
        }))
    }

    /// JPEG-тумбнейл на момент t (тики), если уже построен.
    pub fn thumbnail(&self, media_id: i64, t_ticks: i64) -> Result<Option<Vec<u8>>, IndexError> {
        Ok(self
            .conn
            .query_row(
                "SELECT jpeg FROM thumbnails WHERE media_id=?1 AND t_ticks=?2",
                params![media_id, t_ticks],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Ближайший тумбнейл в допуске max_dist тиков (п.14 р.7): скраб бина
    /// переиспользует тумбы, записанные филмстрипом с другим шагом. BETWEEN
    /// по PK (media_id, t_ticks) — range-scan, ABS только на кандидатах.
    pub fn thumbnail_near(
        &self,
        media_id: i64,
        t_ticks: i64,
        max_dist: i64,
    ) -> Result<Option<(i64, Vec<u8>)>, IndexError> {
        Ok(self
            .conn
            .query_row(
                "SELECT t_ticks, jpeg FROM thumbnails \
                 WHERE media_id=?1 AND t_ticks BETWEEN ?2-?3 AND ?2+?3 \
                 ORDER BY ABS(t_ticks-?2) LIMIT 1",
                params![media_id, t_ticks, max_dist],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    /// Постер медиа: момент самого информативного кадра (п.4 р.9).
    /// `t_ticks < 0` — «информативного кадра нет» (негативный кэш: аудио,
    /// битое видео) — повторно не ищем.
    pub fn poster(&self, media_id: i64) -> Result<Option<i64>, IndexError> {
        Ok(self
            .conn
            .query_row("SELECT t_ticks FROM poster WHERE media_id=?1", params![media_id], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn put_poster(&self, media_id: i64, t_ticks: i64, score: i64) -> Result<(), IndexError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO poster(media_id, t_ticks, score) VALUES(?1,?2,?3)",
            params![media_id, t_ticks, score],
        )?;
        Ok(())
    }

    /// Размер, в котором записаны тумбы медиа (Р.62, t719). Строки НЕТ у
    /// сайдкаров, собранных до приведения тумб к виду источника: их тумбы
    /// 320×180 раздавлены, и читать их нельзя — они перестраиваются при
    /// первом заказе. Таблица аддитивна: бампа схемы (и потери индекса и
    /// пиков всех медиа) ради этого не нужно.
    pub fn thumb_size(&self, media_id: i64) -> Result<Option<(u32, u32)>, IndexError> {
        Ok(self
            .conn
            .query_row("SELECT w, h FROM thumb_meta WHERE media_id=?1", params![media_id], |r| {
                Ok((r.get::<_, i64>(0)? as u32, r.get::<_, i64>(1)? as u32))
            })
            .optional()?)
    }

    pub fn put_thumb_size(&self, media_id: i64, w: u32, h: u32) -> Result<(), IndexError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO thumb_meta(media_id, w, h) VALUES(?1,?2,?3)",
            params![media_id, w as i64, h as i64],
        )?;
        Ok(())
    }

    pub fn put_thumbnail(
        &self,
        media_id: i64,
        t_ticks: i64,
        jpeg: &[u8],
    ) -> Result<(), IndexError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO thumbnails(media_id, t_ticks, jpeg) VALUES(?1,?2,?3)",
            params![media_id, t_ticks, jpeg],
        )?;
        Ok(())
    }

    /// Прочитать уровень пирамиды канала: Vec<(min, max)> по блокам.
    pub fn audio_peaks(
        &self,
        media_id: i64,
        level: u32,
        channel: u32,
    ) -> Result<Option<Vec<(i16, i16)>>, IndexError> {
        let blob: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT data FROM audio_peaks WHERE media_id=?1 AND level=?2 AND channel=?3",
                params![media_id, level as i64, channel as i64],
                |r| r.get(0),
            )
            .optional()?;
        Ok(blob.map(|b| peaks_from_blob(&b)))
    }
}

/// Блоб уровня пирамиды: interleaved (min:i16, max:i16) LE на блок — один
/// формат у `audio_peaks` и `reel_peaks`.
fn peaks_blob(blocks: &[(i16, i16)]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(blocks.len() * 4);
    for &(mn, mx) in blocks {
        blob.extend_from_slice(&mn.to_le_bytes());
        blob.extend_from_slice(&mx.to_le_bytes());
    }
    blob
}

fn peaks_from_blob(b: &[u8]) -> Vec<(i16, i16)> {
    b.chunks_exact(4)
        .map(|c| (i16::from_le_bytes([c[0], c[1]]), i16::from_le_bytes([c[2], c[3]])))
        .collect()
}

/// Превью ленты с диска (Р.58): полоса JPEG-кадров по шагу `step` и
/// пирамида пиков в том же виде, в каком её строит `PeakAcc::finish`.
#[derive(Debug)]
pub struct ReelPreviewRow {
    pub step: i64,
    pub end_ticks: i64,
    /// `frames[i]` — JPEG кадра `i * step`; `None` — кадра не было.
    pub frames: Vec<Option<Vec<u8>>>,
    pub peaks: PeakPyramid,
    /// Огибающая на диске УЖЕ есть. Кадры пишутся раньше пиков (Р.67),
    /// и полоса без них — законное состояние: кадры отдаются с диска, а
    /// пики дописываются следующей задачей.
    pub has_peaks: bool,
}

/// Peak-пирамида в памяти (тест 15 сравнивает её с пересчётом в лоб).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeakPyramid {
    /// levels[level][channel][block] = (min, max)
    pub levels: Vec<Vec<Vec<(i16, i16)>>>,
}

impl PeakPyramid {
    pub fn build(source: &mut dyn MediaSource) -> PeakPyramid {
        Self::build_observed(source, &mut |_| {})
    }

    /// То же, но КАЖДЫЙ прочитанный burst показывается наблюдателю
    /// (Р.49, п.369).
    ///
    /// Зачем: громкость материала и его огибающая считаются на проходе,
    /// который И ТАК ИДЁТ. Второй проход по файлу ради LUFS означал бы
    /// второй полный декод — на часовом исходнике это минуты.
    ///
    /// Пирамида от наблюдателя НЕ ЗАВИСИТ: тот только смотрит, и `build`
    /// остаётся байт-в-байт прежней (t15 цел).
    pub fn build_observed(
        source: &mut dyn MediaSource,
        observe: &mut dyn FnMut(&crate::AudioBurst),
    ) -> PeakPyramid {
        let duration_frames = source.info().duration.to_audio_frames().max(0);
        let total_bursts = duration_frames.div_euclid(AUDIO_BURST_FRAMES as i64).max(0);
        let mut acc = PeakAcc::default();
        for bidx in 0..total_bursts {
            let Ok(burst) = source.read_audio_burst(bidx) else { break };
            observe(&burst);
            acc.push(burst.data.as_slice());
        }
        acc.finish()
    }
}

/// Накопитель пирамиды по ПОТОКУ сэмплов (Р.56, п.452).
///
/// Выделен из `build_observed` и им же используется — арифметика одна, и
/// t15 остаётся байт-в-байт. Нужен службе превью ленты: у ленты нет файла,
/// её огибающая берётся с готового суб-микса, который отдаёт `AudioRenderer`
/// кусками, а не `MediaSource`.
#[derive(Default)]
pub struct PeakAcc {
    level0: Vec<Vec<(i16, i16)>>,
    block: Vec<(i16, i16)>,
    in_block: usize,
}

impl PeakAcc {
    /// Очередной кусок f32 interleaved (кратный числу каналов).
    pub fn push(&mut self, data: &[f32]) {
        if self.level0.is_empty() {
            self.level0 = vec![Vec::new(); AUDIO_CHANNELS];
            self.block = vec![(i16::MAX, i16::MIN); AUDIO_CHANNELS];
        }
        for f in 0..data.len() / AUDIO_CHANNELS {
            for (ch, b) in self.block.iter_mut().enumerate() {
                let v = (data[f * AUDIO_CHANNELS + ch].clamp(-1.0, 1.0) * 32767.0) as i16;
                b.0 = b.0.min(v);
                b.1 = b.1.max(v);
            }
            self.in_block += 1;
            if self.in_block == PEAK_BLOCK_FRAMES {
                for ch in 0..AUDIO_CHANNELS {
                    self.level0[ch].push(self.block[ch]);
                    self.block[ch] = (i16::MAX, i16::MIN);
                }
                self.in_block = 0;
            }
        }
    }

    pub fn finish(mut self) -> PeakPyramid {
        if self.level0.is_empty() {
            self.level0 = vec![Vec::new(); AUDIO_CHANNELS];
        }
        if self.in_block > 0 {
            for ch in 0..AUDIO_CHANNELS {
                self.level0[ch].push(self.block[ch]);
            }
        }
        let mut levels = vec![self.level0];
        while levels.last().unwrap()[0].len() > 1 {
            let prev = levels.last().unwrap();
            let mut next: Vec<Vec<(i16, i16)>> = vec![Vec::new(); AUDIO_CHANNELS];
            for ch in 0..AUDIO_CHANNELS {
                for pair in prev[ch].chunks(2) {
                    let mn = pair.iter().map(|p| p.0).min().unwrap();
                    let mx = pair.iter().map(|p| p.1).max().unwrap();
                    next[ch].push((mn, mx));
                }
            }
            levels.push(next);
        }
        PeakPyramid { levels }
    }
}
