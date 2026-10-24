//! Reading/writing of the .otio document.
//!
//! Writing is deterministic: `OTIO_SCHEMA` as the first key, known fields in the
//! declaration order of the structs, passthrough — in the order of the source document;
//! indent of 4 spaces (the style of the reference library), a final line break
//! (house style). Idempotence `write(parse(write(x))) == write(x)` is
//! pinned by test 144.

use serde::Serialize;
use serde_json::Value;

use crate::schema::{decode_timeline, encode_timeline};
use crate::{Error, Timeline};

pub fn parse_document(text: &str) -> Result<Timeline, Error> {
    let v: Value = serde_json::from_str(text)?;
    decode_timeline(v)
}

pub fn write_document(t: &Timeline) -> String {
    let v = encode_timeline(t);
    let mut out = Vec::with_capacity(16 * 1024);
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
    v.serialize(&mut ser).expect("Value serialization is infallible");
    out.push(b'\n');
    String::from_utf8(out).expect("serde_json emits valid UTF-8")
}

// ---- atomic write of the project file (R.49, item 349) -----------------------
//
// One home for the whole program: both the GUI (Ctrl+S, background
// recovery, exit) and kt-cli come here. Previously the atomic write lived in
// kt-app, while kt-cli wrote with a bare `fs::write` — an interruption in the middle left
// a stub right at the project’s place.
//
// The order of operations is not cosmetic: `write + rename` WITHOUT fsync protects against
// a process crash, but not against power loss — the rename can land
// in the journal before the data itself, and a zero-length file would end up
// in the project’s place. Therefore: write → **flush the file to disk** → rename →
// **flush the directory**.

use std::io;
use std::path::{Path, PathBuf};

/// How the write ended. `dir_synced == false` — the data and the rename
/// are on disk, but the directory could not be flushed (exFAT, a network share,
/// a non-unix system): on power loss the rename ITSELF may be lost.
/// This is NOT a write error — the file is in place and intact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Written {
    pub dir_synced: bool,
}

/// The points where the atomic write touches the filesystem. Pulled out into a
/// trait for the observation of ORDER (fault injection by the kernel cannot check the
/// order — on ext4 `auto_da_alloc` hides it) and of the Windows retry (t853).
pub trait AtomicFs {
    /// Create a temporary file, write the bytes and flush THEM to disk.
    fn write_and_sync(&self, tmp: &Path, bytes: &[u8]) -> io::Result<()>;
    /// Replace `to` with `from`.
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    /// Flush the directory entry (so that the rename survives a power cut).
    fn sync_dir(&self, dir: &Path) -> io::Result<()>;
    /// The rename itself is durable (Windows: `MOVEFILE_WRITE_THROUGH`), so it counts as
    /// the directory sync.
    fn rename_is_durable(&self) -> bool {
        false
    }
    /// The rename failed because someone briefly holds the target (Windows: an antivirus,
    /// a sync client, an editor) — worth another attempt.
    fn is_transient(&self, _e: &io::Error) -> bool {
        false
    }
    fn pause(&self, d: std::time::Duration) {
        std::thread::sleep(d)
    }
}

/// Pauses between attempts of a replace that met a briefly held target: about 0.63 s of
/// waiting, about a second in all (windows-build, design D6). Bounded on purpose — a stuck
/// editor must not hang the exit save.
pub const REPLACE_PAUSES_MS: [u64; 6] = [10, 20, 40, 80, 160, 320];

/// `fs.rename` with the retry policy: transient failures are retried after each pause,
/// any other failure (or the last transient one) is returned.
fn replace_with_retry(fs: &dyn AtomicFs, from: &Path, to: &Path) -> io::Result<()> {
    let mut pauses = REPLACE_PAUSES_MS.iter();
    loop {
        match fs.rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if fs.is_transient(&e) => match pauses.next() {
                Some(ms) => fs.pause(std::time::Duration::from_millis(*ms)),
                None => return Err(e),
            },
            Err(e) => return Err(e),
        }
    }
}

/// Replace `to` with `from` the way the project file is replaced (the journal rotation and
/// the user store use it): on Windows durable and retried while the target is briefly held.
pub fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    replace_with_retry(&RealFs, from, to)
}

/// The real filesystem.
pub struct RealFs;

impl AtomicFs for RealFs {
    fn write_and_sync(&self, tmp: &Path, bytes: &[u8]) -> io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(tmp)?;
        f.write_all(bytes)?;
        // the same descriptor that was written to: the data is guaranteed to be ours
        f.sync_all()
    }

    #[cfg(not(windows))]
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    /// `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)`: the replace returns only after it is
    /// on disk — Windows' durable rename, since a directory cannot be opened for a sync.
    #[cfg(windows)]
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        let wide = |p: &Path| -> Vec<u16> { p.as_os_str().encode_wide().chain([0]).collect() };
        let (f, t) = (wide(from), wide(to));
        let ok = unsafe {
            MoveFileExW(f.as_ptr(), t.as_ptr(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)
        };
        if ok != 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
    }

    #[cfg(unix)]
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        std::fs::File::open(dir)?.sync_all()
    }

    #[cfg(not(unix))]
    fn sync_dir(&self, _dir: &Path) -> io::Result<()> {
        // on Windows the directory cannot be opened that way; the write-through rename
        // stands in for it (`rename_is_durable`)
        Err(io::Error::new(io::ErrorKind::Unsupported, "directory sync is unix-only"))
    }

    fn rename_is_durable(&self) -> bool {
        cfg!(windows)
    }

    /// ERROR_ACCESS_DENIED (5), ERROR_SHARING_VIOLATION (32), ERROR_LOCK_VIOLATION (33):
    /// what a replace meets while someone holds the target without delete sharing.
    fn is_transient(&self, e: &io::Error) -> bool {
        cfg!(windows) && matches!(e.raw_os_error(), Some(5 | 32 | 33))
    }
}

/// Write text to a file atomically and as durably as
/// the filesystem allows. The directory is created if needed.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<Written> {
    write_atomic_with(&RealFs, path, text)
}

pub fn write_atomic_with(fs: &dyn AtomicFs, path: &Path, text: &str) -> io::Result<Written> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty());
    if let Some(dir) = dir {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = atomic_tmp_path(path);
    fs.write_and_sync(&tmp, text.as_bytes())?;
    if let Err(e) = replace_with_retry(fs, &tmp, path) {
        // the project is unchanged; the half-step must not stay next to it
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if fs.rename_is_durable() {
        return Ok(Written { dir_synced: true });
    }
    // A failure to sync the directory is NOT a write failure: the file is already in place and intact.
    // On exFAT and network shares this is a legitimate `Unsupported`, and turning
    // it into an error would mean lying to the user that nothing was saved.
    let dir_synced = match dir {
        Some(d) => fs.sync_dir(d).is_ok(),
        None => false,
    };
    Ok(Written { dir_synced })
}

/// The temporary file name is APPENDED to the full name (test 150): the old
/// `with_extension("ktproj.tmp")` for “a.b.otio” ate part of the name.
///
/// The name includes the process number (R.49, item 352): two copies of the program that opened
/// one project would write INTO ONE temporary file — and the `rename` of one would replace
/// the project with the half-written contents of the other. The lock on the project (see
/// `kt-app/src/project_lock.rs`) does not forbid a second copy,
/// so one lock is not enough here.
pub fn atomic_tmp_path(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!("{name}.{}.tmp", std::process::id()))
}
