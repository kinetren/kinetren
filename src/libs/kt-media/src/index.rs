//! Sidecar media index DB (`<project>.ktindex.db`, SQLite).
//! Schema and invalidation rules — docs/media-index.md. This is a CACHE: loss or
//! corruption is not an error, the DB is recreated. Solves defects m (storm of
//! thumbnail/waveform decodes on scroll/zoom) and j (rescanning files on
//! every launch).

use crate::{MediaSource, PEAK_BLOCK_FRAMES};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};

// v2: audio_seek/video_keyframes (one row per packet, never written)
// replaced by packet_index with i64 LE blobs — persisted packet index of
// FfmpegSource (fast opening of large files). A bump = DB recreation.
pub const SCHEMA_VERSION: i64 = 2;

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// What happens to the media record on opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaEntryStatus {
    /// Records are current (size/mtime matched, or mtime changed but the hash is the same).
    Reused,
    /// Content changed or no record existed — sections must be built anew.
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

/// Is the file not our DB at all — garbage, a damaged file, a foreign schema (`SQLITE_ERROR`
/// from our own statements)? Busy/locked, I/O, "cannot open" and the like are TRANSIENT: the
/// file is fine, someone else is using it right now.
fn is_broken_db(e: &IndexError) -> bool {
    use rusqlite::ErrorCode;
    match e {
        IndexError::Sqlite(rusqlite::Error::SqliteFailure(f, _)) => {
            matches!(f.code, ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt | ErrorCode::Unknown)
        }
        _ => false,
    }
}

/// Remove a DB that is being recreated WITH its `-wal` and `-shm`. A stale `-wal` would be
/// replayed into the new file, and a stale `-shm` is the file SQLite truncates when it believes
/// it is the first user — under a mapping another connection may still hold (t872).
fn remove_db_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    for suffix in ["-wal", "-shm"] {
        let mut side = path.as_os_str().to_owned();
        side.push(suffix);
        let _ = std::fs::remove_file(side);
    }
}

impl MediaIndexDb {
    /// Open/create. A broken or foreign DB is silently recreated (rule 3 of
    /// docs/media-index.md) — this is a cache, it must not fail because of it.
    ///
    /// Only a BROKEN one (t872): the sidecar is open in several threads of one process at once
    /// (index worker, thumbnails, the reopen of a returned file, reel previews), and under load
    /// an opening gets `database is locked` after its busy timeout. Taking that for "broken"
    /// deleted the live file under the other connections; the next opening made a new main
    /// file next to the OLD `-shm`, SQLite found no other user of it (POSIX locks of one process
    /// do not conflict) and truncated it, and a connection that still had it mapped died with
    /// SIGBUS in `walIndexRecover` (t640 under load). A transient error is returned to the
    /// caller — a cache miss, which every caller already tolerates.
    pub fn open(path: &Path) -> Result<MediaIndexDb, IndexError> {
        match Self::try_open(path) {
            Ok(db) => Ok(db),
            Err(e) if is_broken_db(&e) => {
                remove_db_files(path);
                Self::try_open(path)
            }
            Err(e) => Err(e),
        }
    }

    fn try_open(path: &Path) -> Result<MediaIndexDb, IndexError> {
        let conn = Connection::open(path)?;
        // The DB is opened by several threads (index-worker, thumbs, opening of
        // sources) — short writes must not fail with SQLITE_BUSY. Set before any statement,
        // so every one of them waits rather than fails at once.
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let version: Option<i64> = conn
            .query_row("SELECT value FROM meta WHERE key='schema_version'", [], |r| r.get(0))
            .optional()
            .unwrap_or(None);
        match version {
            Some(v) if v == SCHEMA_VERSION => {}
            Some(_) => {
                // schema bump → full rebuild (rule 2)
                drop(conn);
                remove_db_files(path);
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
        // Additive tables are created on EVERY opening (IF NOT EXISTS):
        // so a new table appears even in existing DBs, without requiring a
        // SCHEMA_VERSION bump (a bump wipes packet_index/peaks/thumbs of all
        // project media — too expensive for the sake of one table).
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

    /// Identity check of the file and invalidation of ONLY its records
    /// (rule 1 of docs/media-index.md). Returns (media_id, status).
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
                return Ok((id, MediaEntryStatus::Reused)); // fast path
            }
            let hash = hash_file(file)?;
            if size == ident.size && stored_hash == hash {
                // the content is the same, only mtime — we update and reuse
                self.conn.execute(
                    "UPDATE media SET mtime_ns=?1 WHERE media_id=?2",
                    params![ident.mtime_ns, id],
                )?;
                return Ok((id, MediaEntryStatus::Reused));
            }
            // content changed: we clear ONLY this media_id
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

    /// Reel tables (`reel_*`, R.58) are deliberately NOT touched here: they are
    /// keyed by content (reel, edit, media fingerprint), not by
    /// `media_id`, and a file swap is caught by the fingerprint itself — see `reel_preview`.
    fn purge_media(&self, media_id: i64) -> Result<(), IndexError> {
        for table in ["packet_index", "audio_peaks", "thumbnails", "poster", "loudness"] {
            self.conn
                .execute(&format!("DELETE FROM {table} WHERE media_id=?1"), params![media_id])?;
        }
        // The sync offset is about a PAIR, and the uniform `WHERE media_id=?1` does
        // not clear it: a file that turned out to be the wrong one invalidates
        // both sides of each of its pairs (R.49, item 369).
        self.conn
            .execute("DELETE FROM sync_offset WHERE a_media=?1 OR b_media=?1", params![media_id])?;
        Ok(())
    }

    // ---- results of offline analysis (R.49, item 369) --------------------------
    //
    // Integers ×100, not f64: the round-trip must be byte-exact, otherwise
    // “recalculated the same thing” would give a different number in the last digit, and
    // there would be no way to tell “recalculated” from “changed”.

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

    /// (LUFS, dBTP, LRA) or `None` if not measured yet
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

    /// Shift of B relative to A in ticks, and confidence 0..1. The key is an
    /// ORDERED pair: the sign of the shift depends on the order.
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

    /// Build and save a peak pyramid from an audio source.
    /// Level 0 — a block of 256 frames; level n — a block of 256·2ⁿ. Values (min,max) i16.
    pub fn build_audio_peaks(
        &mut self,
        media_id: i64,
        source: &mut dyn MediaSource,
    ) -> Result<(), IndexError> {
        self.build_audio_peaks_observed(media_id, source, &mut |_| {})
    }

    /// The same with an observer of bursts (R.49, item 373): the material's loudness
    /// is computed on the pass that runs ANYWAY — a second full decode for the sake of
    /// LUFS on an hour-long source is minutes.
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

    // ---- reel PREVIEW on disk (R.58, item 463) --------------------------------
    //
    // A reel has no file, and `media(path, size, mtime, xxhash)` is not its home:
    // the key is the CONTENT. `seq` — the reel id (survives save→load, t431),
    // `hash` — the edit (`kt_engine::content_hash`), `media_fp` — the fingerprint of the
    // reel's tree files (path, size, mtime): the file's revision does not see it, and
    // without the fingerprint a swapped source would show the old strip.
    // One version per reel: a new record evicts the previous one entirely.

    /// Save the frame strip and the reel's peak pyramid in one transaction.
    /// `frames[i]` — the JPEG of frame `i * step`, `None` — the frame was absent (no
    /// row). `hash`/`media_fp` are stored as i64 bitwise.
    #[allow(clippy::too_many_arguments)]
    /// The FRAMES of the strip — in their own transaction, RIGHT AFTER the frame stage
    /// (R.67). Previously the write was single and came only after the audio stage: a strip
    /// that had already reached the screen did not survive to disk if the task was cancelled
    /// (pause, edit, close) — and on the next project opening the reel was rendered
    /// anew (“the composition preview is AGAIN not saved”).
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

    /// The PEAKS of the same strip — in a separate transaction and LATER. They are written
    /// only to a row with the same key: if the edit diverged, the peaks are not ours.
    /// There is no flag in the schema: “peaks exist” means a non-empty `reel_peaks`, and
    /// bumping `schema_version` (which wipes the packet index, peaks and thumbs of ALL
    /// project media) for the sake of one boolean is not allowed.
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
            return Ok(()); // there is no strip with this key — the peaks have nowhere to go
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

    /// The reel preview, if the disk holds EXACTLY this edit with the SAME
    /// files; any mismatch — `None`, and the reel is rendered anew.
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
        // PEAKS MAY BE ABSENT: frames are written earlier, and a strip without an
        // envelope is a legitimate intermediate state (R.67).
        let has_peaks = !levels.is_empty();
        Ok(Some(ReelPreviewRow {
            step,
            end_ticks,
            frames,
            peaks: PeakPyramid { levels },
            has_peaks,
        }))
    }

    /// Remove the previews of reels that are no longer in the project. Returns the
    /// number of removed reels.
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

    /// Save the packet index of a stream (`stream` = “audio” | “video”);
    /// values are pts in the stream's timebase, an i64 LE blob.
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

    /// The stream's packet index, if saved. None = not built
    /// (an empty Vec is a valid index of a file without such a stream).
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

    /// JPEG thumbnail at time t (ticks), if already built.
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

    /// The nearest thumbnail within tolerance max_dist ticks (item 14, round 7): the bin
    /// scrub reuses thumbs written by the filmstrip with another step. BETWEEN
    /// on PK (media_id, t_ticks) — range scan, ABS only on the candidates.
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

    /// Media poster: the moment of the most informative frame (item 4, round 9).
    /// `t_ticks < 0` — “no informative frame” (negative cache: audio,
    /// broken video) — not searched again.
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

    /// The size at which the media's thumbs are written (R.62, t719). Sidecars built before
    /// the thumbs were conformed to the source's form have NO such row: their thumbs
    /// 320×180 are squashed, and they must not be read — they are rebuilt on the
    /// first request. The table is additive: no schema bump (and loss of the index and
    /// peaks of all media) is needed for this.
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

    /// Read a channel's pyramid level: Vec<(min, max)> by blocks.
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

/// A pyramid level blob: interleaved (min:i16, max:i16) LE per block — one
/// format for `audio_peaks` and `reel_peaks`.
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

/// Reel preview from disk (R.58): a strip of JPEG frames at step `step` and a
/// peak pyramid in the same form that `PeakAcc::finish` builds.
#[derive(Debug)]
pub struct ReelPreviewRow {
    pub step: i64,
    pub end_ticks: i64,
    /// `frames[i]` — JPEG of frame `i * step`; `None` — the frame was absent.
    pub frames: Vec<Option<Vec<u8>>>,
    pub peaks: PeakPyramid,
    /// The envelope is ALREADY on disk. Frames are written before the peaks (R.67),
    /// and a strip without them is a legitimate state: frames are served from disk, and
    /// the peaks are appended by the next task.
    pub has_peaks: bool,
}

/// In-memory peak pyramid (test 15 compares it with a naive recomputation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeakPyramid {
    /// levels[level][channel][block] = (min, max)
    pub levels: Vec<Vec<Vec<(i16, i16)>>>,
}

impl PeakPyramid {
    pub fn build(source: &mut dyn MediaSource) -> PeakPyramid {
        Self::build_observed(source, &mut |_| {})
    }

    /// The same, but EVERY burst that is read is shown to the observer
    /// (R.49, item 369).
    ///
    /// Why: the material's loudness and its envelope are computed on the pass that
    /// runs ANYWAY. A second pass over the file for LUFS would mean a second full
    /// decode — on an hour-long source that is minutes.
    ///
    /// The pyramid from the observer is NOT DEPENDENT: it only watches, and `build`
    /// remains byte-for-byte the same (t15 intact).
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

/// Peak-pyramid accumulator over a STREAM of samples (R.56, item 452).
///
/// Factored out of `build_observed` and used by it too — the arithmetic is one, and
/// t15 remains byte-for-byte. It is needed by the reel preview service: a reel has no
/// file, its envelope is taken from the ready sub-mix that `AudioRenderer` hands out
/// in chunks, not from a `MediaSource`.
#[derive(Default)]
pub struct PeakAcc {
    level0: Vec<Vec<(i16, i16)>>,
    block: Vec<(i16, i16)>,
    in_block: usize,
}

impl PeakAcc {
    /// The next chunk of f32 interleaved (a multiple of the channel count).
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
