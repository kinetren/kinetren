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
