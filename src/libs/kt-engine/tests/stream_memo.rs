//! The preview's stream memo sized by the loop and the machine (stream-layout-per-build,
//! found-defects 143): t2275.

use kt_engine::video::{size_stream_memo, stream_memo, stream_memo_bytes, STREAM_MEMO_BASE};
use kt_time::trace::{self, Ev};

const MB: usize = 1 << 20;

/// t2275: the bound is what the loop needs, within a quarter of the memory there is, never below
/// the base; sizing the preview's memo sets it, evicts at once and leaves a record.
#[test]
fn t2275_the_bound_follows_the_loop_within_the_machine() {
    let base = 256 * MB;
    let per = 8 * MB;
    let plenty = Some(64u64 << 30);
    assert_eq!(stream_memo_bytes(base, per, None, plenty), base, "no loop");
    assert_eq!(stream_memo_bytes(base, per, Some(10), plenty), base, "a short loop keeps the base");
    // 666 positions (11 s at 60 fps): every position and an eighth
    assert_eq!(stream_memo_bytes(base, per, Some(666), plenty), per * (666 + 83 + 2));
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
