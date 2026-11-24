//! t1460: a lock of the cache folder another process holds stands in nobody's way.

use kt_media::index::MediaStore;

/// t1460 (sidecar-fsync-stall): nothing a lane reads or writes waits for a lock of the cache folder.
///
/// The sidecar was one SQLite file, and a connection that held a lock of it across an fsync (137 s on a
/// loaded spinning disk) made the others outwait their busy timeout: `DatabaseBusy` in t1171, a
/// re-rendered reel in t640, a loudness measured again in t411. The folder's one lock is for the cycle's
/// drops and the map's compaction, and it is only tried. Asserted: with it held, two threads opening,
/// writing and reading at once all finish, and the map is compacted by no one.
#[test]
fn t1460_a_held_lock_of_the_folder_stands_in_nobodys_way() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("kinetren-cache");
    let files: Vec<_> = (0..2)
        .map(|k| {
            let f = dir.path().join(format!("m{k}.bin"));
            std::fs::write(&f, format!("media {k}")).unwrap();
            f
        })
        .collect();
    drop(MediaStore::open(&root).unwrap());
    // another process drops or compacts, and its disk does not answer
    let held = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(root.join("lock")).unwrap();
    held.lock().unwrap();

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(files.len()));
    let (done, all) = std::sync::mpsc::channel();
    for (k, f) in files.iter().enumerate() {
        let (p, f, b, done) = (root.clone(), f.clone(), barrier.clone(), done.clone());
        std::thread::spawn(move || {
            let run = || -> Result<(), String> {
                b.wait();
                for i in 0..100 {
                    let mut db = MediaStore::open(&p).map_err(|e| format!("open {i}: {e:?}"))?;
                    // a new content each time: the map grows past what a compaction starts at
                    std::fs::write(&f, format!("media {k}, {i}")).map_err(|e| e.to_string())?;
                    let (id, _) = db.ensure_media(&f).map_err(|e| format!("ensure {i}: {e:?}"))?;
                    let lufs = -(k as f64) - i as f64;
                    db.set_loudness(id, lufs, -1.0, 2.0).map_err(|e| format!("write {i}: {e:?}"))?;
                    let back = db.loudness(id).map_err(|e| format!("read {i}: {e:?}"))?;
                    if back.map(|l| l.0) != Some(lufs) {
                        return Err(format!("read {i}: {back:?}, wrote {lufs}"));
                    }
                }
                Ok(())
            };
            let _ = done.send(run());
        });
    }
    for _ in &files {
        let r = all.recv_timeout(kt_time::test_deadline() - std::time::Instant::now()).expect("a thread waits for the folder's lock");
        assert_eq!(r, Ok(()));
    }
    let lines = std::fs::read_to_string(root.join("media/paths.tsv")).unwrap().lines().count();
    assert_eq!(lines, 200, "the map was compacted under another's lock");
    // the lock free again, the next note compacts
    drop(held);
    std::fs::write(&files[0], b"one more").unwrap();
    MediaStore::open(&root).unwrap().ensure_media(&files[0]).unwrap();
    let lines = std::fs::read_to_string(root.join("media/paths.tsv")).unwrap().lines().count();
    assert!(lines <= 2 + 1, "the map was not compacted: {lines} lines");
}
