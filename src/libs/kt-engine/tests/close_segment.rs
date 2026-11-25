//! t3611 (close-while-busy, found-defects 972): the process ends while the prerender writes a segment. The
//! thread that comes back from its frame finishes nothing: no encoder's rest, no manifest, no rename; the
//! trace says what is lost, and the next open removes the file. A process of its own: what ends is the
//! process's. Without an adapter: skip.

use kt_engine::Player;
use kt_engine::render_cache::{Config, Made, RawSegments};
use kt_media::segment::{SegmentCodec, SegmentReader, SegmentWriter};
use kt_media::{MediaSource, Quality};
use kt_model::MediaId;
use kt_model::demo::{DemoSources, demo_project};
use kt_time::Tick;
use kt_time::trace::{Ev, render_cache as rc};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct Gate {
    pushed: AtomicUsize,
    finished: AtomicBool,
    /// The writers stand in their second frame until this is true (a saturated disk).
    open: Mutex<bool>,
    cv: Condvar,
}

struct Stuck(Arc<Gate>);
struct Writer(Arc<Gate>, Box<dyn SegmentWriter>);

impl SegmentWriter for Writer {
    fn push(&mut self, rgba: &[u8]) -> Result<(), String> {
        if self.0.pushed.fetch_add(1, Ordering::AcqRel) >= 1 {
            let mut g = self.0.open.lock().unwrap();
            while !*g {
                g = self.0.cv.wait(g).unwrap();
            }
        }
        self.1.push(rgba)
    }
    fn finish(self: Box<Self>) -> Result<u64, String> {
        // the probe of the encoder is finished empty
        if self.0.pushed.load(Ordering::Acquire) > 0 {
            self.0.finished.store(true, Ordering::Release);
        }
        self.1.finish()
    }
    fn encoder(&self) -> &str {
        "raw"
    }
}

impl SegmentCodec for Stuck {
    fn codec(&self) -> &str {
        "raw"
    }
    fn writer(&self, path: &Path, w: u32, h: u32, fps: u32) -> Result<Box<dyn SegmentWriter>, String> {
        Ok(Box::new(Writer(self.0.clone(), RawSegments.writer(path, w, h, fps)?)))
    }
    fn reader(&self, path: &Path, fps: u32) -> Result<Box<dyn SegmentReader>, String> {
        RawSegments.reader(path, fps)
    }
    fn extension(&self) -> &str {
        RawSegments.extension()
    }
}

fn files(root: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(root).into_iter().flatten().flatten() {
        match e.file_type() {
            Ok(t) if t.is_dir() => files(&e.path(), out),
            _ => out.push(e.path()),
        }
    }
}

fn of(root: &Path, ext: &str) -> usize {
    let mut all = Vec::new();
    files(root, &mut all);
    all.iter().filter(|p| p.extension().is_some_and(|e| e == ext)).count()
}

fn player(root: &Path, codec: Arc<dyn SegmentCodec>) -> Option<Player> {
    let gpu = kt_gpu::test_engine()?;
    let p = demo_project(DemoSources::Synth);
    let sources: Vec<(MediaId, Box<dyn MediaSource>)> =
        p.media.iter().map(|m| (m.id, kt_media::factory::open_pseudo(&m.path).unwrap().unwrap())).collect();
    let (player, _ring) = Player::new(p, sources);
    player.set_preview_quality(Quality { width: 160, height: 90 });
    player.set_video_gpu(Some(gpu));
    player.render_cache().set_heavy_for_tests(Some(1));
    player.set_render_cache(Some(Config { codec, root: root.to_path_buf(), made: Made::of(60), cap: None }));
    Some(player)
}

#[test]
fn t3611_a_process_that_ends_leaves_the_open_segment() {
    kt_plugins_builtin::install();
    let root = tempfile::tempdir().unwrap();
    let gate = Arc::new(Gate::default());
    let codec: Arc<dyn SegmentCodec> = Arc::new(Stuck(gate.clone()));
    let items = kt_time::trace::testing::capture(|| {
        let Some(p) = player(root.path(), codec.clone()) else { return };
        let grid = p.video_grid();
        let start = grid.align_floor(Tick(4 * 90_000));
        for i in 0..6 {
            p.note_preview_cost(Tick(start.0 + i * grid.frame_step()), 1_000_000_000, (160, 90));
        }
        p.render_cache().kick();
        let wait = |what: &str, ok: &dyn Fn() -> bool| {
            let deadline = kt_time::test_deadline();
            while !ok() {
                assert!(std::time::Instant::now() < deadline, "{what}");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        wait("the prerender never wrote its second frame", &|| gate.pushed.load(Ordering::Acquire) >= 2);
        // the close: the watcher is armed, the drop leaves the thread in its call
        kt_time::shutdown::ends_by(std::time::Instant::now() + std::time::Duration::from_secs(8));
        drop(p);
        *gate.open.lock().unwrap() = true;
        gate.cv.notify_all();
        wait("the prerender's thread never ended", &|| Arc::strong_count(&codec) == 1);
    });
    if gate.pushed.load(Ordering::Acquire) == 0 {
        eprintln!("skipped: no device");
        return;
    }
    assert!(!gate.finished.load(Ordering::Acquire), "the segment was finished in the process's end");
    assert_eq!((of(root.path(), "rgba"), of(root.path(), "idx")), (1, 0), "the file, and no manifest");
    let recs = kt_time::trace::testing::recs(&items);
    let left: Vec<_> = recs.iter().filter(|r| r.ev == Ev::RenderCache as u16 && r.aux == rc::ABANDON && r.b == rc::why::CLOSED && r.c > 0).collect();
    assert_eq!(left.len(), 1, "the trace says what the close left");
    assert!(left[0].c >= 1, "its moments: {}", left[0].c);
    // the next open removes it
    let Some(p) = player(root.path(), Arc::new(RawSegments)) else { return };
    let deadline = kt_time::test_deadline();
    while of(root.path(), "rgba") > 0 {
        assert!(std::time::Instant::now() < deadline, "the next open left the file");
        p.render_cache().kick();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
