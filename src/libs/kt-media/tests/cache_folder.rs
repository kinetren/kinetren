//! Tests t3824–t3829, t3831, t3832, t3835–t3840 (project-cache-folder): the cache folder of a project.

use kt_media::cache_folder::{self, DIR_NAME, Folder, Notice, Place, Tenant, registry};
use kt_media::index::{MediaEntryStatus, MediaStore, PeakPyramid, ReelStore};
use kt_media::old_sidecar::{self, Imported};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_time::Tick;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

/// The user's data folder of this test process.
fn data() -> &'static Path {
    static D: OnceLock<tempfile::TempDir> = OnceLock::new();
    let d = D.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        cache_folder::set_data_root(d.path());
        d
    });
    d.path()
}

fn media_file(dir: &Path, name: &str, text: &str) -> PathBuf {
    let f = dir.join(name);
    std::fs::write(&f, text).unwrap();
    f
}

fn tone() -> SyntheticSource {
    SyntheticSource::new(Tick(90_000), AudioPattern::Ramp)
}

/// Peaks, loudness and a thumb of `file` through the folder a project prefers.
fn cache_clip(preferred: &Path, file: &Path) -> MediaStore {
    let mut db = MediaStore::open(preferred).unwrap();
    let (id, _) = db.ensure_media(file).unwrap();
    db.build_audio_peaks(id, &mut tone()).unwrap();
    db.set_loudness(id, -20.0, -1.0, 5.0).unwrap();
    db.put_thumb_size(id, 320, 180).unwrap();
    db.put_thumbnail(id, 0, b"jpeg").unwrap();
    db.mark_complete(id).unwrap();
    db
}

/// A project folder no file can be made in.
fn no_files(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    }
    // root writes through the mode, Windows has none: a file stands where the folder would
    if std::fs::write(dir.join("probe"), b"x").is_ok() {
        std::fs::remove_file(dir.join("probe")).unwrap();
        std::fs::write(dir.join(DIR_NAME), b"a file").unwrap();
    }
}

fn writable(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let _ = dir;
}

/// t3824: a named project in a writable folder caches beside it, and the folder says what it is.
#[test]
fn t3824_the_folder_lies_beside_the_project_and_says_what_it_is() {
    data();
    let dir = tempfile::tempdir().unwrap();
    let clip = media_file(dir.path(), "a.bin", "media");
    let preferred = cache_folder::beside(&dir.path().join("film.kt"));
    assert_eq!(preferred, dir.path().join("kinetren-cache"));
    let db = cache_clip(&preferred, &clip);
    assert_eq!(db.folder().place(), &Place::Project);
    let tag = std::fs::read_to_string(preferred.join("CACHEDIR.TAG")).unwrap();
    // the Cache Directory Tagging signature, which backup tools look for byte for byte
    assert!(tag.starts_with("Signature: 8a477f597d28d172789f06886806bc55"), "{tag}");
    assert_eq!(std::fs::read_to_string(preferred.join(".gitignore")).unwrap(), "*\n");
    assert!(std::fs::read_to_string(preferred.join("README.txt")).unwrap().contains("delete this folder"));

    // deleted while the program runs: a miss, and the next write makes it again with its marks
    std::fs::remove_dir_all(&preferred).unwrap();
    let mut db = MediaStore::open(&preferred).unwrap();
    let (id, st) = db.ensure_media(&clip).unwrap();
    assert_eq!(st, MediaEntryStatus::Rebuilt);
    assert!(db.audio_peaks(id, 0, 0).unwrap().is_none());
    db.mark_complete(id).unwrap();
    assert!(db.is_complete(id).unwrap() && preferred.join("CACHEDIR.TAG").exists());
}

/// t3825: a project whose folder takes no files, and an untitled one, cache under the user's folder;
/// the place and the reason are known, the first is told once.
#[test]
fn t3825_a_folder_that_takes_no_files_and_an_untitled_project_cache_in_the_users_folder() {
    let data = data();
    let dir = tempfile::tempdir().unwrap();
    let clip = media_file(dir.path(), "a.bin", "media");
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    no_files(&locked);
    let preferred = cache_folder::beside(&locked.join("film.kt"));
    let db = cache_clip(&preferred, &clip);
    let f = db.folder();
    assert!(f.root().starts_with(data.join("project-caches")), "{:?}", f.root());
    assert!(matches!(f.place(), Place::NoFiles(why) if !why.is_empty()), "{:?}", f.place());
    assert!(f.root().join("CACHEDIR.TAG").exists());
    // a lane's later handle is the same folder, and nothing is told twice
    drop(MediaStore::open(&preferred).unwrap());
    let told: Vec<Notice> = cache_folder::take_notices()
        .into_iter()
        .filter(|n| matches!(n, Notice::Elsewhere { folder, .. } if *folder == locked))
        .collect();
    assert_eq!(told.len(), 1, "{told:?}");
    writable(&locked);

    let untitled = cache_folder::untitled().unwrap();
    let db = cache_clip(&untitled, &clip);
    assert_eq!(db.folder().place(), &Place::Untitled);
    assert!(untitled.starts_with(data.join("project-caches")));
    // a session that crashed left its folder; a running one's is held
    let left = data.join("project-caches/untitled-1-1");
    std::fs::create_dir_all(left.join("media")).unwrap();
    std::fs::write(left.join("owner"), b"").unwrap();
    cache_folder::sweep_untitled();
    assert!(!left.exists(), "a crashed session's folder stays");
    assert!(untitled.join("media").exists(), "the running session's folder was swept");
}

/// What t3826's other process runs.
#[test]
#[ignore = "run by t3826 as another process"]
fn t3826_writer() {
    let root = PathBuf::from(std::env::var("KT_T3826").unwrap());
    let f = Folder::open(&root).unwrap();
    let payload = vec![std::process::id() as u8; 1 << 20];
    for _ in 0..50 {
        f.write("media/shared/both", b"TEST", 1, &payload).unwrap();
    }
}

/// t3826: a torn entry and one of a foreign kind are misses and go, every other entry hits; an entry
/// two processes write at once reads back whole whenever it is read.
#[test]
fn t3826_a_broken_entry_costs_only_itself() {
    data();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join(DIR_NAME);
    let f = Folder::open(&root).unwrap();
    for n in ["torn", "foreign", "whole", "newer"] {
        f.write(&format!("media/{n}/e"), b"TEST", 1, n.as_bytes()).unwrap();
    }
    let p = |n: &str| root.join(format!("media/{n}/e"));
    let whole = std::fs::read(p("torn")).unwrap();
    std::fs::write(p("torn"), &whole[..whole.len() - 1]).unwrap();
    f.write("media/foreign/e", b"ELSE", 1, b"foreign").unwrap();
    f.write("media/newer/e", b"TEST", 2, b"newer").unwrap();
    for n in ["torn", "foreign", "newer"] {
        assert_eq!(f.read(&format!("media/{n}/e"), b"TEST", 1), None, "{n}");
        assert!(!p(n).exists(), "{n} stays");
    }
    assert_eq!(f.read("media/whole/e", b"TEST", 1).as_deref(), Some(&b"whole"[..]));

    let mut children: Vec<_> = (0..2)
        .map(|_| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["t3826_writer", "--exact", "--ignored"])
                .env("KT_T3826", &root)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    let mut reads = 0;
    while children.iter_mut().any(|c| c.try_wait().unwrap().is_none()) {
        if let Some(b) = f.read("media/shared/both", b"TEST", 1) {
            assert!(b.len() == 1 << 20 && b.iter().all(|x| *x == b[0]), "two processes' bytes mixed");
            reads += 1;
        }
    }
    for c in &mut children {
        assert!(c.wait().unwrap().success());
    }
    assert!(f.read("media/shared/both", b"TEST", 1).is_some(), "lost ({reads} reads meanwhile)");
    let left: Vec<_> = std::fs::read_dir(root.join("media/shared")).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(left.len(), 1, "temporary files stay: {left:?}");
}

/// t3827: the registry lists every folder with its projects and size.
#[test]
fn t3827_the_registry_lists_the_folders() {
    data();
    let dir = tempfile::tempdir().unwrap();
    let clip = media_file(dir.path(), "a.bin", "media");
    let mut roots = Vec::new();
    for n in ["one", "two | tab\tname"] {
        let d = dir.path().join(n);
        if std::fs::create_dir(&d).is_err() {
            // a system whose file names take no tab
            continue;
        }
        let project = d.join("film.kt");
        let preferred = cache_folder::beside(&project);
        let db = cache_clip(&preferred, &clip);
        registry::note(&preferred, Some(&project));
        registry::note(&preferred, Some(&project));
        db.folder().flush();
        let row = registry::list().into_iter().find(|r| r.folder == preferred).expect("not listed");
        assert_eq!(row.projects, vec![project]);
        assert!(row.bytes > 0 && row.last_ms > 0 && row.cap.is_none(), "{row:?}");
        db.folder().set_cap(5_000_000_000);
        roots.push(preferred);
    }
    let rows = registry::list();
    for r in &roots {
        let row = rows.iter().find(|x| x.folder == *r).unwrap();
        assert_eq!(row.cap, Some(5_000_000_000), "a folder's cap is kept");
    }
    registry::remove(&roots[0]);
    assert!(!registry::list().iter().any(|r| r.folder == roots[0]));
}

/// t3828: a folder inside a Dropbox root is marked as not synced; another one is not.
#[test]
fn t3828_a_synced_folder_is_marked() {
    data();
    let dir = tempfile::tempdir().unwrap();
    if !kt_dirs::mark_unsynced(dir.path()) {
        // the file system of the temporary directory takes no mark
        return;
    }
    let dropbox = dir.path().join("Dropbox/films");
    std::fs::create_dir_all(&dropbox).unwrap();
    std::fs::create_dir(dir.path().join("Dropbox/.dropbox.cache")).unwrap();
    let plain = dir.path().join("plain");
    std::fs::create_dir(&plain).unwrap();
    for (d, want) in [(&dropbox, true), (&plain, false)] {
        let f = Folder::open(&cache_folder::beside(&d.join("film.kt"))).unwrap();
        assert_eq!(kt_dirs::is_unsynced(f.root()), want, "{d:?}");
    }
    assert_eq!(cache_folder::synced(Path::new("/home/a/OneDrive - Firm/x/kinetren-cache")), Some(cache_folder::CloudTool::OneDrive));
}

/// t3829: a project folder renamed with its media in it hits every entry, and no file is hashed.
#[test]
fn t3829_a_renamed_project_folder_still_hits() {
    data();
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("before"), dir.path().join("after, renamed"));
    std::fs::create_dir_all(a.join("footage")).unwrap();
    let clip = media_file(&a.join("footage"), "clip.bin", "media");
    let outside = media_file(dir.path(), "outside.bin", "outside");
    let db = cache_clip(&cache_folder::beside(&a.join("film.kt")), &clip);
    cache_clip(&cache_folder::beside(&a.join("film.kt")), &outside);
    assert_eq!(db.folder().hashed_for_tests(), 2);
    drop(db);

    std::fs::rename(&a, &b).unwrap();
    let mut db = MediaStore::open(&cache_folder::beside(&b.join("film.kt"))).unwrap();
    for f in [b.join("footage/clip.bin"), outside] {
        let (id, st) = db.ensure_media(&f).unwrap();
        assert_eq!(st, MediaEntryStatus::Reused, "{f:?}");
        assert!(db.is_complete(id).unwrap() && db.audio_peaks(id, 0, 0).unwrap().is_some());
        assert_eq!(db.thumbnail(id, 0).unwrap().as_deref(), Some(&b"jpeg"[..]));
    }
    assert_eq!(db.folder().hashed_for_tests(), 0, "a file was hashed again");
    // a map another process died writing: its torn line costs a hash of that file, no more
    let map = b.join(DIR_NAME).join("media/paths.tsv");
    let mut text = std::fs::read_to_string(&map).unwrap();
    text.push_str("torn.bin\t12");
    std::fs::write(&map, text).unwrap();
    let more = media_file(&b, "more.bin", "more");
    db.ensure_media(&more).unwrap();
    Folder::forget(&b);
    let mut db = MediaStore::open(&cache_folder::beside(&b.join("film.kt"))).unwrap();
    assert_eq!(db.ensure_media(&more).unwrap().1, MediaEntryStatus::Reused);
    assert_eq!(db.ensure_media(&b.join("footage/clip.bin")).unwrap().1, MediaEntryStatus::Reused);
    assert_eq!(db.folder().hashed_for_tests(), 0);
}

/// A sidecar as the former build wrote it: one clip and its every table.
fn former_sidecar(project: &Path, clip: &Path, other: &Path, peaks: &PeakPyramid) -> PathBuf {
    let path = old_sidecar::path_of(project);
    let c = rusqlite::Connection::open(&path).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    c.execute_batch(
        "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
         INSERT INTO meta VALUES('schema_version', '2');
         CREATE TABLE media (media_id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, size INTEGER NOT NULL,
            mtime_ns INTEGER NOT NULL, xxhash BLOB NOT NULL, probe_json TEXT NOT NULL DEFAULT '',
            built_at INTEGER NOT NULL DEFAULT 0, complete INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE packet_index (media_id INTEGER, stream TEXT, data BLOB, PRIMARY KEY (media_id, stream));
         CREATE TABLE audio_peaks (media_id INTEGER, level INTEGER, channel INTEGER, data BLOB, PRIMARY KEY (media_id, level, channel));
         CREATE TABLE thumbnails (media_id INTEGER, t_ticks INTEGER, jpeg BLOB, PRIMARY KEY (media_id, t_ticks));
         CREATE TABLE poster (media_id INTEGER PRIMARY KEY, t_ticks INTEGER NOT NULL, score INTEGER NOT NULL);
         CREATE TABLE thumb_meta (media_id INTEGER PRIMARY KEY, w INTEGER NOT NULL, h INTEGER NOT NULL);
         CREATE TABLE thumb_pixels (media_id INTEGER PRIMARY KEY, version INTEGER NOT NULL);
         CREATE TABLE audio_series (media_id INTEGER PRIMARY KEY, version INTEGER NOT NULL, data BLOB NOT NULL);
         CREATE TABLE loudness (media_id INTEGER PRIMARY KEY, lufs_x100 INTEGER NOT NULL, dbtp_x100 INTEGER NOT NULL, lra_x100 INTEGER NOT NULL);
         CREATE TABLE sync_offset (a_media INTEGER NOT NULL, b_media INTEGER NOT NULL, ticks INTEGER NOT NULL, conf_x100 INTEGER NOT NULL, PRIMARY KEY (a_media, b_media));
         CREATE TABLE reel_strip (seq INTEGER PRIMARY KEY, hash INTEGER NOT NULL, media_fp INTEGER NOT NULL, step INTEGER NOT NULL, end_ticks INTEGER NOT NULL, n INTEGER NOT NULL);
         CREATE TABLE reel_frame (seq INTEGER, idx INTEGER, jpeg BLOB, PRIMARY KEY (seq, idx));
         CREATE TABLE reel_peaks (seq INTEGER, level INTEGER, channel INTEGER, data BLOB, PRIMARY KEY (seq, level, channel));",
    )
    .unwrap();
    for (n, f) in [(1i64, clip), (2, other)] {
        let md = std::fs::metadata(f).unwrap();
        let ns = md.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as i64;
        let hash = xxhash_rust::xxh3::xxh3_128(&std::fs::read(f).unwrap()).to_le_bytes();
        c.execute(
            "INSERT INTO media(media_id, path, size, mtime_ns, xxhash, complete) VALUES(?1,?2,?3,?4,?5,1)",
            rusqlite::params![n, f.to_string_lossy(), md.len() as i64, ns, hash.as_slice()],
        )
        .unwrap();
    }
    let blob = |b: &[(i16, i16)]| -> Vec<u8> { b.iter().flat_map(|&(a, z)| [a.to_le_bytes(), z.to_le_bytes()].concat()).collect() };
    for (table, row) in [("audio_peaks", 1i64), ("reel_peaks", 7)] {
        for (level, chs) in peaks.levels.iter().enumerate() {
            for (ch, b) in chs.iter().enumerate() {
                c.execute(&format!("INSERT INTO {table} VALUES(?1,?2,?3,?4)"), rusqlite::params![row, level as i64, ch as i64, blob(b)]).unwrap();
            }
        }
    }
    let pts: Vec<u8> = [0i64, 512, 1024].iter().flat_map(|v| v.to_le_bytes()).collect();
    c.execute("INSERT INTO packet_index VALUES(1, 'audio', ?1), (1, 'video', x'')", [pts]).unwrap();
    c.execute_batch(
        "INSERT INTO thumbnails VALUES(1, 0, CAST('j0' AS BLOB)), (1, 9000, CAST('j1' AS BLOB)), (2, 0, CAST('stale' AS BLOB));
         INSERT INTO thumb_meta VALUES(1, 320, 180), (2, 320, 180);
         INSERT INTO thumb_pixels VALUES(1, 1), (2, 0);
         INSERT INTO poster VALUES(1, 9000, 12);
         INSERT INTO audio_series VALUES(1, 3, x'0102');
         INSERT INTO loudness VALUES(1, -2345, -120, 780);
         INSERT INTO sync_offset VALUES(1, 2, -4410, 87);
         INSERT INTO reel_strip VALUES(7, -5, 9, 90000, 270000, 3);
         INSERT INTO reel_frame VALUES(7, 0, CAST('r0' AS BLOB)), (7, 2, CAST('r2' AS BLOB));",
    )
    .unwrap();
    drop(c);
    path
}

/// t3831: a former build's sidecar is read into the folder once — peaks, packets, thumbs of the
/// current pixels, poster, loudness, series, sync offsets, reel previews — and its files go.
#[test]
fn t3831_an_old_sidecar_is_imported_once() {
    data();
    let dir = tempfile::tempdir().unwrap();
    let (clip, other) = (media_file(dir.path(), "a.bin", "media a"), media_file(dir.path(), "b.bin", "media b"));
    let project = dir.path().join("film.kt");
    let peaks = PeakPyramid::build(&mut tone());
    let old = former_sidecar(&project, &clip, &other, &peaks);
    for s in ["-wal", "-shm"] {
        std::fs::write(format!("{}{s}", old.display()), b"").unwrap();
    }
    let mut db = MediaStore::open(&cache_folder::beside(&project)).unwrap();
    // a stop leaves it for the next open
    assert_eq!(old_sidecar::import(&project, db.folder(), &|| true), Imported::Left);
    assert!(old.exists());
    assert_eq!(old_sidecar::import(&project, db.folder(), &|| false), Imported::Yes);
    for s in ["", "-wal", "-shm"] {
        assert!(!Path::new(&format!("{}{s}", old.display())).exists(), "{s} stays");
    }
    assert_eq!(old_sidecar::import(&project, db.folder(), &|| false), Imported::None);

    let (id, st) = db.ensure_media(&clip).unwrap();
    let (id2, _) = db.ensure_media(&other).unwrap();
    assert_eq!(st, MediaEntryStatus::Reused);
    assert_eq!(db.folder().hashed_for_tests(), 0, "the identities came with the sidecar");
    assert!(db.is_complete(id).unwrap());
    assert_eq!(db.pyramid(id).unwrap().as_ref(), Some(&peaks));
    assert_eq!(db.packet_index(id, "audio").unwrap(), Some(vec![0, 512, 1024]));
    assert_eq!(db.packet_index(id, "video").unwrap(), Some(Vec::new()));
    assert_eq!(db.thumb_size(id).unwrap(), Some((320, 180)));
    assert_eq!(db.thumbnail(id, 9000).unwrap().as_deref(), Some(&b"j1"[..]));
    assert_eq!(db.thumb_size(id2).unwrap(), None, "thumbs of older pixels were carried");
    assert_eq!(db.thumbnail(id2, 0).unwrap(), None);
    assert_eq!(db.poster(id).unwrap(), Some(9000));
    assert_eq!(db.audio_series(id, 3).unwrap(), Some(vec![1, 2]));
    assert_eq!(db.loudness(id).unwrap(), Some((-23.45, -1.20, 7.80)));
    assert_eq!(db.sync_offset(id, id2).unwrap(), Some((-4410, 0.87)));
    let reels = ReelStore::open(&cache_folder::beside(&project), Some(&project)).unwrap();
    let row = reels.reel_preview(7, -5i64 as u64, 9).unwrap().expect("the reel's preview");
    assert_eq!((row.step, row.end_ticks, row.has_peaks), (90_000, 270_000, true));
    assert_eq!(row.frames, vec![Some(b"r0".to_vec()), None, Some(b"r2".to_vec())]);
    assert_eq!(row.peaks, peaks);
}

/// t3832: a sidecar another program holds is left for the next open, a garbage one goes; neither
/// stands in the way of the folder.
#[test]
fn t3832_a_locked_sidecar_is_left_and_a_garbage_one_goes() {
    data();
    let dir = tempfile::tempdir().unwrap();
    let (clip, other) = (media_file(dir.path(), "a.bin", "media a"), media_file(dir.path(), "b.bin", "media b"));
    let project = dir.path().join("film.kt");
    let old = former_sidecar(&project, &clip, &other, &PeakPyramid::build(&mut tone()));
    let db = cache_clip(&cache_folder::beside(&project), &clip);
    let holder = rusqlite::Connection::open(&old).unwrap();
    holder.pragma_update(None, "locking_mode", "EXCLUSIVE").unwrap();
    holder.execute_batch("BEGIN EXCLUSIVE; INSERT INTO meta VALUES('held', 'x');").unwrap();
    for _ in 0..5 {
        assert_eq!(old_sidecar::import(&project, db.folder(), &|| false), Imported::Left);
    }
    assert!(old.exists(), "a held sidecar was deleted under its holder");
    holder.execute_batch("ROLLBACK;").unwrap();
    drop(holder);
    assert_eq!(old_sidecar::import(&project, db.folder(), &|| false), Imported::Yes);

    let project = dir.path().join("second.kt");
    std::fs::write(old_sidecar::path_of(&project), b"this is definitely not sqlite").unwrap();
    assert_eq!(old_sidecar::import(&project, db.folder(), &|| false), Imported::Dropped);
    assert!(!old_sidecar::path_of(&project).exists());
}

/// A segment or a run as its owner writes it: files under one stem, then the folder is told.
fn result(f: &Folder, rel: &str, bytes: usize) {
    let p = f.root().join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    assert!(f.ask_room(bytes as u64), "no room for {rel}");
    std::fs::write(&p, vec![0u8; bytes]).unwrap();
    f.wrote(rel, bytes as u64, 0);
}

const DAY: u64 = 86_400_000;

/// t3835: past the cap the folder drops what is worth least: results, the mix and the copies in one
/// order by value per byte (a cheap old frame before a costly one), then the reels, then the media by
/// last use; what this process used in the last minute stays.
#[test]
fn t3835_the_cycle_drops_what_is_worth_least() {
    data();
    let dir = tempfile::tempdir().unwrap();
    let f = Folder::open(&dir.path().join(DIR_NAME)).unwrap();
    f.set_free_for_tests(Some(1 << 40));
    f.set_cap(10_000);
    result(&f, "results/v3/dev/frames/cheap.mkv", 1000);
    result(&f, "results/v3/dev/frames/cheap2.mkv", 1000);
    result(&f, "results/v3/dev/frames/costly.mkv", 1000);
    result(&f, "results/v3/dev/frames/loop.mkv", 1000);
    result(&f, "mix/v1/run.raw", 1000);
    result(&f, "media/aa/copy-1.mkv", 1000);
    f.write("reels/r1/strip", b"TEST", 1, &[0; 968]).unwrap();
    f.write("media/aa/peaks", b"TEST", 1, &[0; 968]).unwrap();
    f.write("media/bb/peaks", b"TEST", 1, &[0; 968]).unwrap();
    // what each was last used at, how often it was read, what a read saves
    f.age_for_tests("results/v3/dev/frames/cheap", 10 * DAY, 4, 10);
    f.age_for_tests("results/v3/dev/frames/cheap2", 9 * DAY, 4, 10);
    f.age_for_tests("results/v3/dev/frames/costly", DAY, 4, 50_000);
    f.age_for_tests("mix/v1/run", 5 * DAY, 4, 100);
    f.age_for_tests("media/aa/copy-1", 5 * DAY, 4, 1000);
    f.age_for_tests("reels/r1", 20 * DAY, 100, 0);
    f.age_for_tests("media/aa", 2 * DAY, 100, 0);
    f.age_for_tests("media/bb", DAY, 100, 0);
    // the loop being played: the cheapest of all, but used a moment ago
    f.used("results/v3/dev/frames/loop.mkv");

    let order = [
        "results/v3/dev/frames/cheap2",
        "results/v3/dev/frames/cheap",
        "mix/v1/run",
        "media/aa/copy-1",
        "results/v3/dev/frames/costly",
        "reels/r1",
        "media/bb",
        "media/aa",
    ];
    for (i, next) in order.iter().enumerate() {
        // every write is of one entry's size, used a moment ago from then on
        result(&f, &format!("results/v3/dev/frames/new{i}.mkv"), if i == 0 { 2000 } else { 1000 });
        assert!(!f.holds_for_tests(next), "step {i}: {next} should have gone");
        for later in &order[i + 1..] {
            assert!(f.holds_for_tests(later), "step {i}: {later} went before its turn");
        }
    }
    assert!(!dir.path().join(DIR_NAME).join("media/aa").exists(), "the media's files stay");
    assert!(f.holds_for_tests("results/v3/dev/frames/loop"), "the loop being played was eaten");
    // nothing droppable is left: the write is skipped and counted, nothing fails
    assert!(!f.ask_room(1000));
    assert_eq!(f.skipped(), 1);
    assert!(f.write("media/cc/peaks", b"TEST", 1, &[0; 968]).is_err(), "no room is no write");
    let dropped = f.take_dropped("results/v3/dev/frames/");
    assert_eq!(dropped, ["cheap2", "cheap", "costly"].map(|n| format!("results/v3/dev/frames/{n}")));
    // the stamps outlive the process
    f.flush();
    let usage = std::fs::read_to_string(f.root().join("usage.tsv")).unwrap();
    assert!(usage.contains("results/v3/dev/frames/loop\t1000\t"), "{usage}");
}

/// t3836: the disk's room bounds the folder as the cap does, its reserve stays free, and a write the
/// system refuses for lack of space counts again, drops and succeeds on its retry.
#[test]
fn t3836_a_full_disk_is_the_same_event() {
    data();
    let dir = tempfile::tempdir().unwrap();
    let f = Folder::open(&dir.path().join(DIR_NAME)).unwrap();
    f.set_free_for_tests(Some(1 << 40));
    for i in 0..4 {
        result(&f, &format!("results/v3/dev/frames/s{i}.mkv"), 1000);
        f.age_for_tests(&format!("results/v3/dev/frames/s{i}"), (i + 1) * DAY, 1, 10);
    }
    // the reserve is reached: each write takes the place of the oldest
    f.set_free_for_tests(Some(2_000_000_000));
    result(&f, "results/v3/dev/frames/n0.mkv", 1000);
    assert!(!f.holds_for_tests("results/v3/dev/frames/s0"));
    assert!(f.holds_for_tests("results/v3/dev/frames/s1"));

    f.set_free_for_tests(Some(1 << 40));
    for (kind, gone) in [(ErrorKind::StorageFull, 1), (ErrorKind::QuotaExceeded, 2)] {
        let walks = f.walks_for_tests();
        f.fail_writes_for_tests(&[kind]);
        f.write("media/dd/peaks", b"TEST", 1, &[1; 900]).unwrap();
        assert!(f.walks_for_tests() > walks, "the room was not counted again");
        assert!(!f.holds_for_tests(&format!("results/v3/dev/frames/s{gone}")), "{kind:?}: nothing dropped");
        assert_eq!(f.read("media/dd/peaks", b"TEST", 1).map(|b| b.len()), Some(900));
    }
    // refused again on the retry: the write fails, the caller takes it as not kept
    f.fail_writes_for_tests(&[ErrorKind::StorageFull, ErrorKind::StorageFull]);
    assert!(f.write("media/ee/peaks", b"TEST", 1, &[1; 10]).is_err());
}

/// t3837: a folder that writes in its cycle tells it once, again after the cap moved; a folder at rest
/// walks and drops nothing.
#[test]
fn t3837_a_full_folder_says_so_once_and_rests() {
    data();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join(DIR_NAME);
    let f = Folder::open(&root).unwrap();
    f.set_free_for_tests(Some(1 << 40));
    f.set_protect_for_tests(Some(Duration::ZERO));
    f.set_cap(3000);
    let told = || cache_folder::take_notices().into_iter().filter(|n| matches!(n, Notice::Full { folder, .. } if *folder == root)).count();
    for i in 0..3 {
        result(&f, &format!("results/v3/dev/frames/s{i}.mkv"), 1000);
    }
    assert_eq!((told(), f.is_full()), (0, true), "nothing was replaced yet");
    for i in 3..8 {
        result(&f, &format!("results/v3/dev/frames/s{i}.mkv"), 1000);
    }
    assert_eq!(told(), 1, "once a session");
    let (_, _, drops) = f.counts(Tenant::Frames);
    assert_eq!(drops, 5);
    f.set_cap(2000);
    result(&f, "results/v3/dev/frames/s8.mkv", 1000);
    assert_eq!(told(), 1, "the cap moved");
    assert_eq!(f.known_sizes().get(&Tenant::Frames), Some(&(2000, 2)));

    let (walks, counts) = (f.walks_for_tests(), f.counts(Tenant::Frames));
    let before: Vec<_> = std::fs::read_dir(&root).unwrap().flatten().map(|e| (e.file_name(), e.metadata().unwrap().modified().unwrap())).collect();
    for _ in 0..3 {
        f.read("media/none/peaks", b"TEST", 1);
        drop(Folder::open(&root).unwrap());
    }
    let after: Vec<_> = std::fs::read_dir(&root).unwrap().flatten().map(|e| (e.file_name(), e.metadata().unwrap().modified().unwrap())).collect();
    assert_eq!((f.walks_for_tests(), f.counts(Tenant::Frames)), (walks, counts), "a read walked or dropped");
    assert_eq!(after, before, "a read wrote into the folder");

    // the person empties it
    assert_eq!(f.clear(Some(Tenant::Frames)), 2000);
    assert_eq!(f.known_sizes().get(&Tenant::Frames), None);
    assert_eq!(f.prune(), 0);
}

/// t3838: an untitled project's cache is in the project's folder after the first save: every entry
/// hits under the new place with nothing hashed or computed, and the untitled folder is gone.
#[test]
fn t3838_an_untitled_cache_moves_on_the_first_save() {
    data();
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("film.kt");
    let (inside, outside) = (media_file(dir.path(), "in.bin", "inside"), media_file(tempdir_kept(), "out.bin", "outside"));
    let from = cache_folder::untitled().unwrap();
    for f in [&inside, &outside] {
        cache_clip(&from, f);
    }
    let mut reels = ReelStore::open(&from, None).unwrap();
    reels.put_reel_frames(3, 5, 9, 90_000, 90_000, &[Some(b"r")]).unwrap();
    result(&Folder::open(&from).unwrap(), "results/v3/dev/frames/s0.mkv", 100);
    let to = cache_folder::beside(&project);
    cache_folder::carry(&from, &to, None, &project).unwrap();

    let mut db = MediaStore::open(&to).unwrap();
    for f in [&inside, &outside] {
        let (id, st) = db.ensure_media(f).unwrap();
        assert_eq!(st, MediaEntryStatus::Reused);
        assert!(db.is_complete(id).unwrap() && db.audio_peaks(id, 0, 0).unwrap().is_some());
    }
    assert_eq!(db.folder().hashed_for_tests(), 0);
    let reels = ReelStore::open(&to, Some(&project)).unwrap();
    assert!(reels.reel_preview(3, 5, 9).unwrap().is_some());
    assert!(!from.exists(), "the untitled folder stays");
    assert!(!registry::list().iter().any(|r| r.folder == from));
    assert!(registry::list().iter().any(|r| r.folder == to && r.projects == vec![project.clone()]));
    // the prerendered frames came along inside one file system
    if to.join("results/v3/dev/frames/s0.mkv").exists() {
        assert!(db.folder().sizes().contains_key(&Tenant::Frames));
    }
    // the next untitled project of this process begins a folder of its own
    cache_clip(&from, &inside);
    assert_eq!(Folder::open(&from).unwrap().place(), &Place::Untitled);
}

/// A directory of another place than the project's.
fn tempdir_kept() -> &'static Path {
    static D: OnceLock<tempfile::TempDir> = OnceLock::new();
    D.get_or_init(|| tempfile::tempdir().unwrap()).path()
}

/// t3839: a renamed project folder with two projects in it: both projects' media and reels hit, and no
/// file is hashed.
#[test]
fn t3839_a_renamed_folder_keeps_both_projects() {
    data();
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a"), dir.path().join("b"));
    std::fs::create_dir(&a).unwrap();
    let clip = media_file(&a, "clip.bin", "media");
    let root = cache_folder::beside(&a.join("one.kt"));
    assert_eq!(root, cache_folder::beside(&a.join("two.kt")), "one folder for the projects of a folder");
    cache_clip(&root, &clip);
    for (n, p) in ["one.kt", "two.kt"].iter().enumerate() {
        ReelStore::open(&root, Some(&a.join(p))).unwrap().put_reel_frames(1, n as u64, 1, 1, 1, &[Some(b"r")]).unwrap();
    }
    std::fs::rename(&a, &b).unwrap();
    let root = cache_folder::beside(&b.join("one.kt"));
    let mut db = MediaStore::open(&root).unwrap();
    assert_eq!(db.ensure_media(&b.join("clip.bin")).unwrap().1, MediaEntryStatus::Reused);
    assert_eq!(db.folder().hashed_for_tests(), 0);
    for (n, p) in ["one.kt", "two.kt"].iter().enumerate() {
        assert!(ReelStore::open(&root, Some(&b.join(p))).unwrap().reel_preview(1, n as u64, 1).unwrap().is_some(), "{p}");
    }
}

/// t3840: Save As into another folder copies the media index and the reel previews and no prerender;
/// the first folder keeps everything. Save As beside the first file shares the folder, and the copy's
/// reels are its own.
#[test]
fn t3840_save_as_copies_what_is_small() {
    data();
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a"), dir.path().join("b"));
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let clip = media_file(&a, "clip.bin", "media");
    let (was, now) = (a.join("film.kt"), b.join("film.kt"));
    let (from, to) = (cache_folder::beside(&was), cache_folder::beside(&now));
    cache_clip(&from, &clip);
    result(&Folder::open(&from).unwrap(), "results/v3/dev/frames/s0.mkv", 100);
    result(&Folder::open(&from).unwrap(), "media/aa/copy-1.mkv", 100);
    ReelStore::open(&from, Some(&was)).unwrap().put_reel_frames(3, 5, 9, 1, 1, &[Some(b"r")]).unwrap();
    cache_folder::carry(&from, &to, Some(&was), &now).unwrap();
    for root in [&from, &to] {
        let mut db = MediaStore::open(root).unwrap();
        let hashed = db.folder().hashed_for_tests();
        let (id, st) = db.ensure_media(&clip).unwrap();
        assert!(st == MediaEntryStatus::Reused && db.is_complete(id).unwrap() && db.thumbnail(id, 0).unwrap().is_some());
        assert_eq!(db.folder().hashed_for_tests(), hashed, "hashed again in {root:?}");
        assert!(ReelStore::open(root, Some(&now)).unwrap().reel_preview(3, 5, 9).unwrap().is_some());
    }
    assert!(from.join("results/v3/dev/frames/s0.mkv").exists() && from.join("media/aa/copy-1.mkv").exists());
    assert!(!to.join("results").exists() && !to.join("media/aa").exists(), "a large result was copied");

    let beside = a.join("film 2.kt");
    cache_folder::carry(&from, &cache_folder::beside(&beside), Some(&was), &beside).unwrap();
    let mut copy = ReelStore::open(&from, Some(&beside)).unwrap();
    assert_eq!(copy.prune_reels(&[]).unwrap(), 1);
    assert!(ReelStore::open(&from, Some(&was)).unwrap().reel_preview(3, 5, 9).unwrap().is_some(), "the copy's prune took the first's");
}
