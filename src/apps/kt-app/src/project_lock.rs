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
//! **Why `flock` and not a file with a pid and `O_EXCL`.** The kernel releases `flock` on
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
//! The name is a hash of the absolute path; the path itself and the pid are written INSIDE the file, so that
//! the message can name the other process.

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

/// Directory for the lock files.
fn locks_dir() -> PathBuf {
    crate::journal::data_dir().join("locks")
}

/// Lock file name: a hash of the absolute project path. Absolute, not canonical:
/// `canonicalize` requires an existing file, and the lock is needed also for “Save as…”
/// to a file not yet created.
pub fn lock_file_for(project: &Path) -> PathBuf {
    let abs = std::path::absolute(project).unwrap_or_else(|_| project.to_path_buf());
    locks_dir().join(format!("{:016x}.lock", fnv1a(abs.to_string_lossy().as_bytes())))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
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
    if try_lock(&file) {
        // The content — for the message to the neighbor and for manual inspection. We write
        // AFTER taking the lock: otherwise we would overwrite another’s record.
        let _ = std::io::Write::write_all(
            &mut (&file),
            format!("{}\n{}\n", std::process::id(), project.display()).as_bytes(),
        );
        LockState::Owned(ProjectLock { _file: file })
    } else {
        let pid = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.lines().next().and_then(|l| l.trim().parse().ok()));
        LockState::Busy { pid }
    }
}

#[cfg(unix)]
fn try_lock(file: &std::fs::File) -> bool {
    use std::os::fd::AsRawFd;
    // SAFETY: a live handle to an open file is passed; flock does not touch memory
    // and returns only an error code.
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 }
}

#[cfg(not(unix))]
fn try_lock(_file: &std::fs::File) -> bool {
    // Without flock it is more honest to consider the project free than to pretend there is protection.
    true
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
}
