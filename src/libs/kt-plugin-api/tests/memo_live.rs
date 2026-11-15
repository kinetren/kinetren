//! t2709 (stream-round-trips D1, capability `plugins`): the stream memo's live tier — what a walk
//! continues from, kept on the compute device — is bounded on its own, outside the pictures' LRU.

use std::sync::Arc;

use kt_plugin_api::stream::{StreamMemo, LIVE_BYTES};

/// Test t2709: pictures filling the memo's bound evict no live entry; the live tier evicts its own
/// least recently used past `LIVE_BYTES`; `holds` sees both tiers; `drop_unpinned` keeps the live tier
/// and `clear` empties it.
#[test]
fn t2709_the_live_tier_is_bounded_apart_from_the_pictures() {
    let memo = StreamMemo::new(1 << 20);
    memo.put_live(1, Arc::new(1u32), 100);
    for k in 10..100u64 {
        memo.put(k, Arc::new(k), 64 << 10);
    }
    assert!(memo.bytes() <= 1 << 20, "the pictures keep their bound");
    assert_eq!(memo.get_live::<u32>(1).as_deref(), Some(&1), "the pictures evicted the live state");
    assert!(memo.holds(1) && !memo.contains(1), "the live key is held, not a picture");
    // the live tier's own bound: the least recently used goes
    let big = LIVE_BYTES / 2 - 1000;
    memo.put_live(2, Arc::new(2u32), big);
    memo.put_live(3, Arc::new(3u32), big);
    let _ = memo.get_live::<u32>(2);
    memo.put_live(4, Arc::new(4u32), big);
    assert!(memo.live_bytes() <= LIVE_BYTES);
    assert!(memo.get_live::<u32>(3).is_none(), "the least recently used live entry stays past the bound");
    assert!(memo.get_live::<u32>(2).is_some() && memo.get_live::<u32>(4).is_some());
    memo.drop_unpinned();
    assert!(memo.get_live::<u32>(4).is_some(), "the LRU's loss is not the live tier's");
    memo.clear();
    assert_eq!(memo.live_bytes(), 0);
    assert!(!memo.holds(4));
}
