//! Project lock (R.49, item 352).
//!
//! The task is not to forbid a second copy of the program, but to keep two copies
//! from silently writing to one project file, overwriting each other’s work.
//!
//! **Why not a single program instance.** A global lock would kill
//! live scenarios (`scripts/live-check.sh` runs kt-app in Xvfb
//! in parallel) and the legitimate scenario “opened a second time to look at
//! another piece of the edit”. The lock is named: it applies to a specific project.
//!
//! **Why a kernel lock (`flock` on Linux, `LockFileEx` on Windows — both through
//! `std::fs::File::try_lock`) and not a file with a pid and `O_EXCL`.** The kernel releases it on
//! ANY process death — a crash, SIGKILL, `terminate()` from a live
//! shell. A lock the kernel does not release by itself leaves a permanent
//! block after the first forced kill: a class that already cost
//! a round (item 232) — seven green scenarios would have turned red.
//!
//! **Why the lock is on a NEIGHBOR file, not on the project itself.** Saving is
//! atomic: `rename` replaces the inode. A lock taken on the project file would, after the
//! first Ctrl+S, remain on an orphaned inode, and the next
//! copy would open a NEW one and calmly take its own — protection would vanish exactly
//! at the moment it was needed.
//!
//! **Why the lock file lives in the data directory, not next to the project.**
//! The user’s directory is not cluttered, the lock is taken even on a project in a
//! read-only directory (on a network share, on a mounted image), and
//! abandoned empty files stay in our directory, where they bother no one.
//! The name is a hash of the absolute path (`kt_dirs::locks`, shared with kt-cli; on
//! Windows of its normalized spelling, t852); the pid and the path are written into the
//! file AND into `<name>.pid` next to it: a Windows lock is mandatory, the second copy
//! cannot read the locked file, and the message must still name the other process.

use std::path::{Path, PathBuf};

/// What ended the attempt to take the project.
pub enum LockState {
    /// The project is ours: we write to it freely.
    Owned(ProjectLock),
    /// Taken by another copy. `pid` is its process number, if the lock file
    /// could be read (if not, we work without the number — that is no reason to stay silent).
    Busy { pid: Option<u32> },
    /// The lock could not be taken at all (no data directory, no rights).
    /// Not a reason not to work: we behave as the owner, but without a guarantee.
    Unavailable,
}

pub struct ProjectLock {
    _file: std::fs::File,
}

impl Drop for ProjectLock {
    fn drop(&mut self) {
        // The file is NOT deleted: between our `unlink` and the neighbor’s `open` a third one could slip in,
        // and two processes would hold locks on different inodes of one name. An empty file
        // in its own data directory is the price of correctness, and the kernel releases the lock
        // itself in any case.
    }
}

/// Lock file name: a hash of the absolute project path. Absolute, not canonical:
/// `canonicalize` requires an existing file, and the lock is needed also for “Save as…”
/// to a file not yet created.
pub fn lock_file_for(project: &Path) -> PathBuf {
    kt_dirs::locks::lock_file(&crate::journal::data_dir(), project)
}

/// Try to take the project. Does not block for even a millisecond:
/// taken means taken, and the program keeps working,
/// only honestly reporting it.
pub fn acquire(project: &Path) -> LockState {
    let path = lock_file_for(project);
    if let Some(d) = path.parent()
        && std::fs::create_dir_all(d).is_err()
    {
        return LockState::Unavailable;
    }
    // truncate(false): the lock file survives a change of holder, and another’s record
    // must not be zeroed before the lock is taken
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
    {
        Ok(f) => f,
        Err(_) => return LockState::Unavailable,
    };
    match file.try_lock() {
        Ok(()) => {
            // The content — for the message to the neighbor and for manual inspection. We
            // write AFTER taking the lock: otherwise we would overwrite another’s record.
            let record = format!("{}\n{}\n", std::process::id(), project.display());
            let _ = std::io::Write::write_all(&mut (&file), record.as_bytes());
            let _ = std::fs::write(kt_dirs::locks::pid_file(&path), &record);
            LockState::Owned(ProjectLock { _file: file })
        }
        Err(std::fs::TryLockError::WouldBlock) => {
            LockState::Busy { pid: kt_dirs::locks::read_holder(&path) }
        }
        // the file system cannot lock (some network shares): work as the owner, without
        // a guarantee — the same as having no data directory
        Err(std::fs::TryLockError::Error(_)) => LockState::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t362: the lock is named and durable. A second attempt to take the SAME project is refused
    /// and names the process; another project is taken freely; atomic rewriting of the project
    /// itself does NOT break the lock (which is why it is taken on a neighboring file, not on the
    /// project’s inode).
    #[test]
    fn t362_project_lock_is_per_project_and_survives_atomic_save() {
        let dir = std::env::temp_dir().join(format!("kt-t362-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.kt");
        let b = dir.join("b.kt");
        std::fs::write(&a, "{}").unwrap();

        let first = match acquire(&a) {
            LockState::Owned(l) => l,
            _ => panic!("a free project must be claimed"),
        };
        // same process, same project — flock is recursive per descriptor,
        // so we take a NEW descriptor,
        // as a second copy of the program would
        match acquire(&a) {
            LockState::Busy { pid } => {
                assert_eq!(pid, Some(std::process::id()), "the lock did not name its holder")
            }
            _ => panic!("the busy project was handed to a second copy"),
        }
        // a neighboring project is not affected
        assert!(matches!(acquire(&b), LockState::Owned(_)), "the lock turned out to be global");

        // saving the project atomically replaces the inode — the lock must survive
        kt_otio::write_atomic(&a, "{\"x\":1}").unwrap();
        assert!(matches!(acquire(&a), LockState::Busy { .. }), "Ctrl+S cleared the lock");

        drop(first);
        assert!(matches!(acquire(&a), LockState::Owned(_)), "the lock was not released");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Different paths — different lock files; one path — one lock file
    /// (including one written differently).
    #[test]
    fn t362b_lock_file_is_keyed_by_path() {
        let a = lock_file_for(Path::new("/w/film.kt"));
        let b = lock_file_for(Path::new("/w/film.mp4.kt"));
        assert_ne!(a, b);
        assert_eq!(a, lock_file_for(Path::new("/w/./film.kt")));
        assert!(a.to_string_lossy().ends_with(".lock"));
    }

    /// t852 (windows-build): the Windows key of one file written four ways is one key, the
    /// Linux key keeps case; a busy project names its holder through the pid file (the only
    /// way on Windows, where the locked file cannot be read).
    #[test]
    fn t852_lock_key_normalized_and_pid_read_while_locked() {
        use kt_dirs::Os;
        use kt_dirs::locks::{key, stem};
        let forms = [r"C:\Proj\a.kt", r"c:\proj\A.kt", r"\\?\C:\Proj\a.kt", "C:/Proj/a.kt"];
        let keys: Vec<String> = forms.iter().map(|f| key(f, Os::Windows)).collect();
        assert!(keys.iter().all(|k| k == r"c:\proj\a.kt"), "{keys:?}");
        assert_eq!(key(r"\\?\UNC\nas\v\a.kt", Os::Windows), r"\\nas\v\a.kt");
        assert_ne!(stem("/p/a.kt", Os::Linux), stem("/p/A.kt", Os::Linux));
        // the Linux name of an existing lock did not change (FNV-1a of the path as is)
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in "/w/film.kt".as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        assert_eq!(stem("/w/film.kt", Os::Linux), format!("{h:016x}"));

        let dir = std::env::temp_dir().join(format!("kt-t852-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("p.kt");
        let _held = match acquire(&p) {
            LockState::Owned(l) => l,
            _ => panic!("a free project must be claimed"),
        };
        let lock = lock_file_for(&p);
        assert!(kt_dirs::locks::pid_file(&lock).exists(), "the pid file is written next to the lock");
        match acquire(&p) {
            LockState::Busy { pid } => assert_eq!(pid, Some(std::process::id())),
            _ => panic!("the busy project was handed to a second copy"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
