//! t1460: a stalled disk does not lock the sidecar.
//!
//! Its own test binary: it installs a SQLite VFS as the process default, and no other test may
//! share that process.

use kt_media::index::MediaIndexDb;
use rusqlite::ffi;
use std::os::raw::{c_char, c_int};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The disk: while `true`, every fsync blocks until it is cleared — a spinning disk under the
/// write load of a whole parallel test run (one fsync took 137 s on the build machine).
static STALL: Mutex<bool> = Mutex::new(false);
static STALL_CV: Condvar = Condvar::new();
/// Syncs that met the stall.
static STALLED: AtomicUsize = AtomicUsize::new(0);

type XSync = unsafe extern "C" fn(*mut ffi::sqlite3_file, c_int) -> c_int;

struct BaseVfs(*mut ffi::sqlite3_vfs);
// SAFETY: the default VFS is a process-wide static of SQLite; only its pointer travels.
unsafe impl Send for BaseVfs {}
unsafe impl Sync for BaseVfs {}
static BASE: OnceLock<BaseVfs> = OnceLock::new();

/// The io-methods tables handed out (as addresses), with the base `xSync` each one replaces.
static METHODS: Mutex<Vec<(usize, usize, XSync)>> = Mutex::new(Vec::new());

fn set_stall(on: bool) {
    *STALL.lock().unwrap() = on;
    STALL_CV.notify_all();
}

unsafe extern "C" fn stall_sync(file: *mut ffi::sqlite3_file, flags: c_int) -> c_int {
    let table = unsafe { (*file).pMethods } as usize;
    let base = METHODS.lock().unwrap().iter().find(|m| m.1 == table).map(|m| m.2).expect("a wrapped file");
    let mut stalled = STALL.lock().unwrap();
    if *stalled {
        STALLED.fetch_add(1, Ordering::SeqCst);
        while *stalled {
            stalled = STALL_CV.wait(stalled).unwrap();
        }
    }
    drop(stalled);
    unsafe { base(file, flags) }
}

/// The base VFS opens the file in place; only its io-methods table is swapped for a copy whose
/// `xSync` passes through the stall. Every other method is the base's own and gets the same
/// file, so the shim forwards nothing else.
unsafe extern "C" fn stall_open(
    _vfs: *mut ffi::sqlite3_vfs,
    name: *const c_char,
    file: *mut ffi::sqlite3_file,
    flags: c_int,
    out_flags: *mut c_int,
) -> c_int {
    let base = BASE.get().unwrap().0;
    let rc = unsafe { ((*base).xOpen.unwrap())(base, name, file, flags, out_flags) };
    let orig = unsafe { (*file).pMethods };
    if rc == ffi::SQLITE_OK && !orig.is_null() {
        let mut m = METHODS.lock().unwrap();
        let wrapped = match m.iter().find(|e| e.0 == orig as usize) {
            Some(e) => e.1,
            None => {
                let mut copy = unsafe { *orig };
                let base_sync = copy.xSync.expect("a file without xSync");
                copy.xSync = Some(stall_sync);
                let leaked = Box::leak(Box::new(copy)) as *const ffi::sqlite3_io_methods as usize;
                m.push((orig as usize, leaked, base_sync));
                leaked
            }
        };
        unsafe { (*file).pMethods = wrapped as *const ffi::sqlite3_io_methods };
    }
    rc
}

/// Install the stalling VFS as the default of this process (before any connection).
fn install() {
    BASE.get_or_init(|| {
        let base = unsafe { ffi::sqlite3_vfs_find(std::ptr::null()) };
        assert!(!base.is_null(), "no default SQLite VFS");
        let mut vfs = unsafe { std::ptr::read(base) };
        vfs.zName = c"kt-stall".as_ptr();
        vfs.pNext = std::ptr::null_mut();
        vfs.xOpen = Some(stall_open);
        let vfs = Box::leak(Box::new(vfs));
        assert_eq!(unsafe { ffi::sqlite3_vfs_register(vfs, 1) }, ffi::SQLITE_OK);
        BaseVfs(base)
    });
}

/// t1460 (sidecar-fsync-stall): no lock of the sidecar is held across an fsync.
///
/// In the whole-workspace `finish.sh` run (all crates' tests in parallel on spinning disks) one
/// fsync took tens of seconds. A connection that held a lock of the sidecar across it — a commit
/// syncing the WAL under the write lock, the last close checkpointing and syncing under the
/// EXCLUSIVE lock every opening needs — made the others outwait their 5 s busy timeout:
/// `DatabaseBusy` in t1171, a re-rendered reel in t640, an unrounded loudness in t411 (the second
/// index service measured the file again because its opening failed). Asserted: with every fsync
/// stalled, two threads opening, writing, reading and closing a ready sidecar at once all finish
/// well inside the busy timeout, without an error and without waiting on the disk.
#[test]
fn t1460_a_stalled_disk_does_not_lock_the_sidecar() {
    install();
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("p.ktindex.db");
    let files: Vec<_> = (0..2)
        .map(|k| {
            let f = dir.path().join(format!("m{k}.bin"));
            std::fs::write(&f, format!("media {k}")).unwrap();
            f
        })
        .collect();
    // a ready sidecar, written and closed while the disk still answers
    {
        let mut db = MediaIndexDb::open(&db_path).unwrap();
        let (id, _) = db.ensure_media(&files[0]).unwrap();
        db.set_loudness(id, -20.0, -1.0, 3.0).unwrap();
    }
    assert!(STALLED.load(Ordering::SeqCst) == 0);

    set_stall(true);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(files.len()));
    let workers: Vec<_> = files
        .iter()
        .enumerate()
        .map(|(k, f)| {
            let (p, f, b) = (db_path.clone(), f.clone(), barrier.clone());
            std::thread::spawn(move || -> Result<(), String> {
                b.wait();
                for i in 0..5 {
                    let mut db = MediaIndexDb::open(&p).map_err(|e| format!("open {i}: {e:?}"))?;
                    let (id, _) = db.ensure_media(&f).map_err(|e| format!("ensure {i}: {e:?}"))?;
                    let lufs = -(k as f64) - i as f64;
                    db.set_loudness(id, lufs, -1.0, 2.0).map_err(|e| format!("write {i}: {e:?}"))?;
                    let back = db.loudness(id).map_err(|e| format!("read {i}: {e:?}"))?;
                    if back.map(|l| l.0) != Some(lufs) {
                        return Err(format!("read {i}: {back:?}, wrote {lufs}"));
                    }
                }
                Ok(())
            })
        })
        .collect();
    // well inside the busy timeout (5 s): a thread still running here waits on the disk or on a
    // lock someone holds across it
    let deadline = Instant::now() + Duration::from_secs(3);
    while workers.iter().any(|w| !w.is_finished()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let hung = workers.iter().filter(|w| !w.is_finished()).count();
    let stalled = STALLED.load(Ordering::SeqCst);
    // the disk answers again, so a thread blocked on it can finish and be joined
    set_stall(false);
    let errs: Vec<String> = workers.into_iter().filter_map(|w| w.join().unwrap().err()).collect();
    assert!(
        hung == 0 && errs.is_empty() && stalled == 0,
        "with the disk stalled: {hung} thread(s) still waiting after 3 s, {stalled} fsync(s) \
         stalled, errors {errs:?} — a lock of the sidecar is held across an fsync"
    );
}
