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

    fn purge_media(&self, media_id: i64) -> Result<(), IndexError> {
        for table in ["packet_index", "audio_peaks", "thumbnails", "poster", "loudness"] {
            self.conn
                .execute(&format!("DELETE FROM {table} WHERE media_id=?1"), params![media_id])?;
        }
        // Сдвиг синхронизации — про ПАРУ, и однородное `WHERE media_id=?1`
        // его не чистит: файл, оказавшийся не тем, делает недействительными
        // обе стороны каждой своей пары (Р.49, п.369).
        self.conn.execute(
            "DELETE FROM sync_offset WHERE a_media=?1 OR b_media=?1",
            params![media_id],
        )?;
        Ok(())
    }

    // ---- результаты офлайн-анализа (Р.49, п.369) --------------------------
    //
    // Целые ×100, а не f64: round-trip обязан быть байт-точным, иначе
    // «пересчитали то же самое» давало бы другое число в последнем знаке, и
    // отличить «пересчитали» от «изменилось» стало бы нечем.

    pub fn set_loudness(&self, media_id: i64, lufs: f64, dbtp: f64, lra: f64) -> Result<(), IndexError> {
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

    pub fn sync_offset(&self, a_media: i64, b_media: i64) -> Result<Option<(i64, f64)>, IndexError> {
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
                let mut blob = Vec::with_capacity(blocks.len() * 4);
                for &(mn, mx) in blocks {
                    blob.extend_from_slice(&mn.to_le_bytes());
                    blob.extend_from_slice(&mx.to_le_bytes());
                }
                tx.execute(
                    "INSERT INTO audio_peaks(media_id, level, channel, data) VALUES(?1,?2,?3,?4)",
                    params![media_id, level as i64, ch as i64, blob],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
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
            .query_row(
                "SELECT t_ticks FROM poster WHERE media_id=?1",
                params![media_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn put_poster(&self, media_id: i64, t_ticks: i64, score: i64) -> Result<(), IndexError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO poster(media_id, t_ticks, score) VALUES(?1,?2,?3)",
            params![media_id, t_ticks, score],
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
        Ok(blob.map(|b| {
            b.chunks_exact(4)
                .map(|c| (i16::from_le_bytes([c[0], c[1]]), i16::from_le_bytes([c[2], c[3]])))
                .collect()
        }))
    }
}

/// Peak-пирамида в памяти (тест 15 сравнивает её с пересчётом в лоб).
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
        let mut level0: Vec<Vec<(i16, i16)>> = vec![Vec::new(); AUDIO_CHANNELS];
        let mut block: Vec<(i16, i16)> = vec![(i16::MAX, i16::MIN); AUDIO_CHANNELS];
        let mut in_block = 0usize;
        let total_bursts = duration_frames.div_euclid(AUDIO_BURST_FRAMES as i64).max(0);
        for bidx in 0..total_bursts {
            let Ok(burst) = source.read_audio_burst(bidx) else { break };
            observe(&burst);
            for f in 0..AUDIO_BURST_FRAMES {
                for (ch, b) in block.iter_mut().enumerate() {
                    let v = (burst.data[f * AUDIO_CHANNELS + ch].clamp(-1.0, 1.0) * 32767.0) as i16;
                    b.0 = b.0.min(v);
                    b.1 = b.1.max(v);
                }
                in_block += 1;
                if in_block == PEAK_BLOCK_FRAMES {
                    for ch in 0..AUDIO_CHANNELS {
                        level0[ch].push(block[ch]);
                        block[ch] = (i16::MAX, i16::MIN);
                    }
                    in_block = 0;
                }
            }
        }
        if in_block > 0 {
            for ch in 0..AUDIO_CHANNELS {
                level0[ch].push(block[ch]);
            }
        }

        let mut levels = vec![level0];
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
