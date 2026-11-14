//! The preview's stream memo sized by the loop and the machine (stream-layout-per-build,
//! found-defects 143): t2275; through the memory ledger (memory-ledger): t2297.

use kt_engine::video::{size_stream_memo, stream_memo, stream_memo_bytes, STREAM_MEMO_BASE};
use kt_time::trace::{self, Ev};

const MB: usize = 1 << 20;

/// The tests share the preview's memo and the process's ledger.
static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// t2275: the bound is what the loop needs, within a quarter of the memory there is, never below
/// the base; sizing the preview's memo sets it, evicts at once and leaves a record.
#[test]
fn t2275_the_bound_follows_the_loop_within_the_machine() {
    let _one = ONE.lock().unwrap_or_else(|p| p.into_inner());
    let base = 256 * MB;
    let per = 8 * MB;
    let plenty = Some(64u64 << 30);
    assert_eq!(stream_memo_bytes(base, per, None, plenty), base, "no loop");
    assert_eq!(stream_memo_bytes(base, per, Some(10), plenty), base + per * 13, "a short loop: its pictures over the base");
    // 666 positions (11 s at 60 fps): every position and an eighth, over the working set
    assert_eq!(stream_memo_bytes(base, per, Some(666), plenty), per * (666 + 83 + 2) + base);
    assert_eq!(stream_memo_bytes(base, per, Some(666), Some(4u64 << 30)), 1 << 30, "a quarter of 4 GB");
    assert_eq!(stream_memo_bytes(base, per, Some(666), Some(512 << 20)), base, "never below the base");
    assert_eq!(stream_memo_bytes(base, per, Some(666), None), base, "unknown memory");
    // the preview's memo: filled, then a smaller bound drops the oldest entries now
    let memo = stream_memo();
    for k in 0..40u64 {
        memo.put(k + 1, std::sync::Arc::new(k), 8 * MB);
    }
    assert!(memo.bytes() <= STREAM_MEMO_BASE);
    let items = trace::testing::capture(|| {
        size_stream_memo(per, None);
    });
    let recs: Vec<_> = trace::testing::recs(&items).into_iter().filter(|r| r.kind() == Some(Ev::StreamMemoCap)).collect();
    assert_eq!(recs.len(), 1, "one record");
    assert_eq!(recs[0].a, STREAM_MEMO_BASE as u64);
    memo.set_cap(64 * MB);
    assert!(memo.bytes() <= 64 * MB, "a smaller bound evicts at once");
    assert!(memo.get::<u64>(40).is_some() && memo.get::<u64>(1).is_none(), "the newest stays");
}

/// t2297: the stream history is granted after the kept composites: what they leave of the growth
/// budget, never above its own want; when they want less, the next sizing grants the want.
#[test]
fn t2297_the_kept_composites_come_before_the_history() {
    use kt_time::mem::Tenant;
    let _one = ONE.lock().unwrap_or_else(|p| p.into_inner());
    kt_engine::video::set_memory_probe(|| Some(4 << 30));
    let memo = stream_memo();
    memo.clear();
    let kept = kt_time::mem::open(Tenant::Kept, 0);
    let per = 8 * MB;
    // 4 GB free, nothing grown: 2 GB of growth; the composites ask 1900 MB of it
    kept.want(1900 * MB as u64);
    let want = stream_memo_bytes(STREAM_MEMO_BASE, per, Some(666), Some(4 << 30));
    assert_eq!(want, 1024 * MB, "a quarter of the memory");
    let cap = size_stream_memo(per, Some(666));
    assert_eq!(cap, STREAM_MEMO_BASE + 148 * MB, "the base plus what the composites left");
    assert_eq!(memo.bytes_cap(), cap);
    // the composites want less: the history's next sizing gets its want, no more
    kept.want(0);
    let cap = size_stream_memo(per, Some(666));
    assert_eq!(cap, want, "its own want, not the whole budget");
    // the composites ask again: the history is taken back at once, without a sizing of its own
    kept.want(2000 * MB as u64);
    assert_eq!(memo.bytes_cap(), STREAM_MEMO_BASE + 48 * MB, "told at once");
    kept.want(0);
    size_stream_memo(per, None);
    assert_eq!(memo.bytes_cap(), STREAM_MEMO_BASE);
    kt_engine::video::set_memory_probe(|| None);
}

/// t2355 (stream-history-on-device, found-defects 394): the want covers the walk's checkpoint tier —
/// two pictures for every checkpoint of the walk from the chain's anchor to the loop's end — over
/// the loop's own pictures and the base.
#[test]
fn t2355_the_want_covers_the_walks_checkpoints() {
    use kt_engine::video::stream_memo_bytes_walk;
    let base = 256 * MB;
    let per = 2 * MB;
    let plenty = Some(64u64 << 30);
    // a loop of 40 positions whose anchor lies 400 before its end: 25 checkpoints of 16
    assert_eq!(stream_memo_bytes_walk(base, per, Some(40), 25, plenty), base + per * (40 + 5 + 2 + 2 * 25));
    assert_eq!(stream_memo_bytes_walk(base, per, Some(40), 0, plenty), stream_memo_bytes(base, per, Some(40), plenty));
    assert_eq!(stream_memo_bytes_walk(base, per, Some(40), 25, Some(1u64 << 30)), base, "still within a quarter of the memory, not below the base");
    assert_eq!(stream_memo_bytes_walk(base, per, None, 25, plenty), base, "no loop: the base");
}

// ---- stream-memo-budget ------------------------------------------------------------------------

use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 90_000;
const Q: kt_media::Quality = kt_media::Quality { width: 160, height: 90 };

fn clip(id: u64, media: u64, start: i64, dur: i64, fx: Vec<FxInstance>) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
        masks: Default::default(),
    }
}

/// The cut mosh of decode_storm: shot A for 2 s, then shot B for 6 s under Datamosh with every
/// I-frame dropped, so a frame late in B walks back to the cut.
fn mosh_clips() -> (Project, Vec<ClipEvent>) {
    kt_plugins_builtin::install();
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.media.push(MediaRef::new(MediaId(2), "synth:2"));
    let mut f = FxInstance::plugin("datamosh").expect("datamosh is built in");
    for (name, v) in [("lines", 0.0), ("before", 12.0), ("mosh", 1.0), ("gop", 12.0), ("qp", 14.0)] {
        assert!(f.set_param(name, v), "{name}");
    }
    let clips = vec![clip(1, 1, 0, 2 * SEC, vec![]), clip(2, 2, 2 * SEC, 6 * SEC, vec![f])];
    (p, clips)
}

/// The mosh at the root, then a reel of it at 10 s, then a plain shot to 60 s.
fn mosh_and_its_reel() -> Project {
    let (mut p, clips) = mosh_clips();
    let mut seq = Sequence::new(MediaId(100), "mosh reel");
    // ids are unique across the project: the reel's copies get their own
    seq.clips = clips.iter().cloned().map(|mut c| {
        c.id = ClipId(c.id.0 + 100);
        c
    }).collect();
    p.sequences.push(seq);
    p.clips = clips;
    p.clips.push(clip(10, 100, 10 * SEC, 8 * SEC, vec![]));
    p.clips.push(clip(11, 1, 18 * SEC, 42 * SEC, vec![]));
    p.validate().unwrap();
    p
}

/// t2491 (stream-memo-budget, found-defects 512): the checkpoint tier of a walk is counted through a
/// reel — a Datamosh inside a reel names the checkpoints its own walk keeps, as at the root.
#[test]
fn t2491_a_reels_stage_names_its_checkpoints() {
    let p = mosh_and_its_reel();
    let vr = kt_engine::VideoRenderer::new(&p);
    let root = vr.stream_checkpoints(Tick(7 * SEC), Q);
    assert!(root > 0, "the root's walk keeps checkpoints");
    assert_eq!(vr.stream_checkpoints(Tick(17 * SEC), Q), root, "the reel's same walk, 10 s later, names none or others");
}

/// t2492 (stream-memo-budget, found-defects 518): the want over a loop counts each stream stage's own
/// positions in it at its own size, a reel's included — not every moment of the region at the size of
/// the frame on screen — and does not depend on where the playhead is.
#[test]
fn t2492_the_want_counts_the_stages_own_positions() {
    let _one = ONE.lock().unwrap_or_else(|p| p.into_inner());
    let p = mosh_and_its_reel();
    let vr = kt_engine::VideoRenderer::new(&p);
    let w = vr.stream_want(Tick(0), Tick(60 * SEC), Q);
    // 6 s of the event at 30 fps at the root, the same in the reel
    assert_eq!(w.positions, 2 * 180, "{w:?}");
    let per = vr.stream_frame_bytes(Tick(5 * SEC), Q) / 2;
    assert!(per > 0);
    assert_eq!(w.per_position, per);
    assert_eq!(w.lap_bytes, 2 * 180 * per);
    let cp = vr.stream_checkpoints(Tick(8 * SEC - 3000), Q);
    assert_eq!(w.checkpoint_bytes, 2 * (2 * cp * per), "both walks' tiers");
    // a loop over the plain shot alone keeps nothing
    assert_eq!(vr.stream_want(Tick(20 * SEC), Tick(60 * SEC), Q).positions, 0);
    // the old want: the whole region's 1800 moments at the frame on screen's size
    let plenty = Some(64u64 << 30);
    let old = stream_memo_bytes(STREAM_MEMO_BASE, per, Some(1801), plenty);
    let new = kt_engine::video::stream_memo_want(STREAM_MEMO_BASE, Some(&w), plenty);
    assert_eq!(new, STREAM_MEMO_BASE + w.lap_bytes + w.lap_bytes / 8 + 2 * per + w.checkpoint_bytes);
    assert!(new < old, "the region's moments were asked: {new} against {old}");
    assert_eq!(kt_engine::video::stream_memo_want(STREAM_MEMO_BASE, None, plenty), STREAM_MEMO_BASE, "no loop");
}
