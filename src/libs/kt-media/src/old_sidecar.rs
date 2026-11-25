//! The door for the caches of former builds (project-cache-folder D5): `<project>.ktindex.db`, one
//! SQLite file beside the project, is read once into the cache folder and deleted. Nothing else of
//! the program reads or writes SQLite; when the door goes, so does the dependency.

use crate::cache_folder::Folder;
use crate::index::{ContentId, FileIdentity, MediaStore, PeakPyramid, ReelStore, THUMB_PIXELS_VERSION, peaks_from_blob};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The former sidecar of a project file.
pub fn path_of(project: &Path) -> PathBuf {
    project.with_extension("ktindex.db")
}

/// How often an unreadable sidecar is tried before it goes: it is a cache.
const TRIES: u32 = 3;

/// What became of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Imported {
    /// There is none.
    None,
    /// Read into the folder and deleted.
    Yes,
    /// Garbage, or unreadable for the third time: deleted.
    Dropped,
    /// Locked or unreadable now: left for the next open.
    Left,
}

/// Read the project's former sidecar into its cache folder, on the index lane. Asked before every
/// table and file: `stop` leaves the sidecar for the next open.
pub fn import(project: &Path, folder: &Arc<Folder>, stop: &dyn Fn() -> bool) -> Imported {
    let file = path_of(project);
    if !file.is_file() {
        return Imported::None;
    }
    let tries = folder.root().join("media").join("import-tries");
    match read(&file, project, folder, stop) {
        Ok(true) => {
            remove(&file);
            let _ = std::fs::remove_file(&tries);
            Imported::Yes
        }
        Ok(false) => Imported::Left,
        Err(e) if is_garbage(&e) => {
            remove(&file);
            Imported::Dropped
        }
        // someone reads it now (a former build): deleting it under a connection is what a SIGBUS was
        // made of (t872)
        Err(e) if is_busy(&e) => Imported::Left,
        Err(_) => {
            let n = std::fs::read_to_string(&tries).ok().and_then(|t| t.trim().parse::<u32>().ok()).unwrap_or(0) + 1;
            if n >= TRIES {
                remove(&file);
                let _ = std::fs::remove_file(&tries);
                return Imported::Dropped;
            }
            let _ = std::fs::write(&tries, n.to_string());
            Imported::Left
        }
    }
}

/// The sidecar with its `-wal` and `-shm`.
fn remove(file: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut p = file.as_os_str().to_owned();
        p.push(suffix);
        let _ = std::fs::remove_file(p);
    }
}

type Error = Box<dyn std::error::Error>;

fn code(e: &Error) -> Option<rusqlite::ErrorCode> {
    match e.downcast_ref::<rusqlite::Error>() {
        Some(rusqlite::Error::SqliteFailure(f, _)) => Some(f.code),
        _ => None,
    }
}

fn is_garbage(e: &Error) -> bool {
    use rusqlite::ErrorCode as C;
    matches!(code(e), Some(C::NotADatabase | C::DatabaseCorrupt | C::Unknown))
}

fn is_busy(e: &Error) -> bool {
    use rusqlite::ErrorCode as C;
    matches!(code(e), Some(C::DatabaseBusy | C::DatabaseLocked))
}

fn read(file: &Path, project: &Path, folder: &Arc<Folder>, stop: &dyn Fn() -> bool) -> Result<bool, Error> {
    let conn = Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    // a held sidecar is left at once: the lane has other work
    conn.busy_timeout(std::time::Duration::ZERO)?;
    let tables: Vec<String> = {
        let mut st = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'")?;
        let rows = st.query_map([], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let has = |t: &str| tables.iter().any(|x| x == t);
    let store = MediaStore::of(folder.clone());
    let mut ids: HashMap<i64, ContentId> = HashMap::new();
    if has("media") {
        let mut st = conn.prepare("SELECT media_id, path, size, mtime_ns, xxhash, complete FROM media")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let (row, path, size, mtime_ns): (i64, String, i64, i64) = (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?);
            let Ok(hash) = <[u8; 16]>::try_from(r.get::<_, Vec<u8>>(4)?) else { continue };
            let id = ContentId(hash);
            store.note_path(Path::new(&path), FileIdentity { size: size as u64, mtime_ns }, id)?;
            if r.get::<_, i64>(5)? != 0 {
                store.mark_complete(id)?;
            }
            ids.insert(row, id);
        }
    }
    for (&row, &id) in &ids {
        if stop() {
            return Ok(false);
        }
        if has("audio_peaks") && !store.has(id, "peaks") {
            let levels = levels(&conn, "audio_peaks", "media_id", row)?;
            if !levels.is_empty() {
                store.put_pyramid(id, &PeakPyramid { levels })?;
            }
        }
        if has("packet_index") {
            let mut st = conn.prepare("SELECT stream, data FROM packet_index WHERE media_id=?1")?;
            let mut rows = st.query(params![row])?;
            while let Some(r) = rows.next()? {
                let (stream, data): (String, Vec<u8>) = (r.get(0)?, r.get(1)?);
                let pts: Vec<i64> = data.chunks_exact(8).map(|c| i64::from_le_bytes(c.try_into().unwrap())).collect();
                store.put_packet_index(id, &stream, &pts)?;
            }
        }
        if has("thumbnails") && has("thumb_meta") && has("thumb_pixels") && !store.has(id, "thumbs") {
            import_thumbs(&conn, &store, row, id)?;
        }
        if has("poster") {
            let p: Option<(i64, i64)> = conn
                .query_row("SELECT t_ticks, score FROM poster WHERE media_id=?1", params![row], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;
            if let Some((t, score)) = p {
                store.put_poster(id, t, score)?;
            }
        }
        if has("loudness") {
            let l: Option<(i64, i64, i64)> = conn
                .query_row("SELECT lufs_x100, dbtp_x100, lra_x100 FROM loudness WHERE media_id=?1", params![row], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .optional()?;
            if let Some((a, b, c)) = l {
                store.set_loudness(id, a as f64 / 100.0, b as f64 / 100.0, c as f64 / 100.0)?;
            }
        }
        if has("audio_series") {
            let s: Option<(i64, Vec<u8>)> = conn
                .query_row("SELECT version, data FROM audio_series WHERE media_id=?1", params![row], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;
            if let Some((v, data)) = s {
                store.put_audio_series(id, v as u32, &data)?;
            }
        }
    }
    if has("sync_offset") {
        let mut st = conn.prepare("SELECT a_media, b_media, ticks, conf_x100 FROM sync_offset")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let (a, b, ticks, conf): (i64, i64, i64, i64) = (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?);
            if let (Some(&a), Some(&b)) = (ids.get(&a), ids.get(&b)) {
                store.set_sync_offset(a, b, ticks, conf as f64 / 100.0)?;
            }
        }
    }
    if has("reel_strip") && has("reel_frame") && has("reel_peaks") {
        import_reels(&conn, &mut ReelStore::of(folder.clone(), Some(project)), stop)?;
    }
    Ok(!stop())
}

fn import_thumbs(conn: &Connection, store: &MediaStore, row: i64, id: ContentId) -> Result<(), Error> {
    let of = |q: &str| conn.query_row(q, params![row], |r| r.get::<_, i64>(0)).optional();
    // thumbs of older pixels are not carried (t2894)
    if of("SELECT version FROM thumb_pixels WHERE media_id=?1")? != Some(THUMB_PIXELS_VERSION as i64) {
        return Ok(());
    }
    let (w, h) = (of("SELECT w FROM thumb_meta WHERE media_id=?1")?, of("SELECT h FROM thumb_meta WHERE media_id=?1")?);
    let (Some(w), Some(h)) = (w, h) else { return Ok(()) };
    let mut st = conn.prepare("SELECT t_ticks, jpeg FROM thumbnails WHERE media_id=?1 ORDER BY t_ticks")?;
    let rows = st.query_map(params![row], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let thumbs: Vec<(i64, Vec<u8>)> = rows.collect::<Result<_, _>>()?;
    if !thumbs.is_empty() {
        let mut store = MediaStore::of(store.folder().clone());
        store.put_thumb_size(id, w as u32, h as u32)?;
        store.put_thumbs(id, &thumbs)?;
    }
    Ok(())
}

fn import_reels(conn: &Connection, reels: &mut ReelStore, stop: &dyn Fn() -> bool) -> Result<(), Error> {
    let mut st = conn.prepare("SELECT seq, hash, media_fp, step, end_ticks, n FROM reel_strip")?;
    let heads = st.query_map([], |r| {
        Ok([r.get::<_, i64>(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?])
    })?;
    for head in heads {
        let [seq, hash, fp, step, end, n] = head?;
        if stop() {
            return Ok(());
        }
        let mut frames: Vec<Option<Vec<u8>>> = vec![None; n.clamp(0, 4096) as usize];
        let mut st = conn.prepare("SELECT idx, jpeg FROM reel_frame WHERE seq=?1")?;
        let mut rows = st.query(params![seq])?;
        while let Some(r) = rows.next()? {
            let idx: i64 = r.get(0)?;
            if let Some(slot) = usize::try_from(idx).ok().and_then(|i| frames.get_mut(i)) {
                *slot = Some(r.get(1)?);
            }
        }
        let refs: Vec<Option<&[u8]>> = frames.iter().map(|f| f.as_deref()).collect();
        reels.put_reel_frames(seq, hash as u64, fp as u64, step, end, &refs)?;
        let levels = levels(conn, "reel_peaks", "seq", seq)?;
        if !levels.is_empty() {
            reels.put_reel_peaks(seq, hash as u64, fp as u64, &PeakPyramid { levels })?;
        }
    }
    Ok(())
}

type Levels = Vec<Vec<Vec<(i16, i16)>>>;

fn levels(conn: &Connection, table: &str, key: &str, row: i64) -> Result<Levels, Error> {
    let mut st = conn.prepare(&format!("SELECT level, data FROM {table} WHERE {key}=?1 ORDER BY level, channel"))?;
    let mut rows = st.query(params![row])?;
    let mut out: Levels = Vec::new();
    while let Some(r) = rows.next()? {
        let (level, data): (i64, Vec<u8>) = (r.get(0)?, r.get(1)?);
        if level as usize == out.len() {
            out.push(Vec::new());
        }
        // a hole in the levels: nothing after it is a pyramid's
        let Some(l) = out.get_mut(level as usize) else { break };
        l.push(peaks_from_blob(&data));
    }
    Ok(out)
}
