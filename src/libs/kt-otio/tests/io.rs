//! Tests 351-353 (R.49, item 349): atomic writing is honest about
//! power loss.
//!
//! `write + rename` protects against a process crash, but not against a loss of
//! power: the rename may reach the journal before the data, and in place of the project
//! there will be a zero-length file. The order of operations is everything the function is
//! written for — so it is checked by observation, not by kernel fault injection
//! (on ext4 its result is hidden by `auto_da_alloc`).

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};

use kt_otio::{AtomicFs, write_atomic, write_atomic_with};

/// Records the order of calls to the file system.
#[derive(Default)]
struct Recorder {
    log: RefCell<Vec<String>>,
    dir_sync_fails: bool,
}

impl AtomicFs for Recorder {
    fn write_and_sync(&self, tmp: &Path, bytes: &[u8]) -> io::Result<()> {
        self.log.borrow_mut().push(format!("write+sync {} ({} bytes)", name(tmp), bytes.len()));
        std::fs::write(tmp, bytes)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.log.borrow_mut().push(format!("rename {} -> {}", name(from), name(to)));
        std::fs::rename(from, to)
    }

    fn sync_dir(&self, _dir: &Path) -> io::Result<()> {
        self.log.borrow_mut().push("sync dir".into());
        if self.dir_sync_fails {
            // exactly what exFAT and network shares return
            return Err(io::Error::new(io::ErrorKind::Unsupported, "no directory sync here"));
        }
        Ok(())
    }
}

fn name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kt-io-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Test 351: data on disk BEFORE the rename, the directory — after.
#[test]
fn t351_atomic_write_syncs_file_then_renames_then_syncs_dir() {
    let dir = tmp_dir("order");
    let path = dir.join("a.b.otio");
    let rec = Recorder::default();
    let w = write_atomic_with(&rec, &path, "hello").expect("write");
    assert!(w.dir_synced, "on an ordinary FS the directory must be flushed");

    let log = rec.log.borrow().clone();
    // the temporary file's name carries the process number (item 352) — in expectations it
    // is substituted, not hard-coded
    let pid = std::process::id();
    assert_eq!(
        log,
        vec![
            // tmp appended to the FULL name — “a.b.otio” does not lose “.b” (t150)
            format!("write+sync a.b.otio.{pid}.tmp (5 bytes)"),
            format!("rename a.b.otio.{pid}.tmp -> a.b.otio"),
            "sync dir".to_string(),
        ],
        "the order must be: write and flush the FILE → rename → flush the DIRECTORY"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
    assert!(!kt_otio::atomic_tmp_path(&path).exists(), "temporary file does not remain");

    // and the same with a real file system — the wrapper did not diverge from the trait
    let path2 = dir.join("real.otio");
    write_atomic(&path2, "{}").expect("real write");
    assert_eq!(std::fs::read_to_string(&path2).unwrap(), "{}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 353: a failure of the DIRECTORY sync is not a write failure.
///
/// On exFAT and network shares the directory cannot be flushed that way, and turning this into an
/// error would mean lying to the user that the save did not happen: the file is in
/// place and intact. The caller learns of the degradation from `dir_synced` and says
/// so once per session.
#[test]
fn t353_dir_sync_failure_is_not_a_write_failure() {
    let dir = tmp_dir("nodirsync");
    let path = dir.join("p.otio");
    let rec = Recorder { dir_sync_fails: true, ..Default::default() };
    let w = write_atomic_with(&rec, &path, "payload").expect("the write must count as successful");
    assert!(!w.dir_synced, "but must report the degradation");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "payload", "file is in place and intact");
    let _ = std::fs::remove_dir_all(&dir);
}
