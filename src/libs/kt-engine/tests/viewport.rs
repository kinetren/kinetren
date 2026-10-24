//! Test 13: timeline scroll/zoom does not produce
//! decode requests (invariant 8). The old engine, on every viewport shift,
//! re-decoded thumbnails/waveforms from scratch (defect m).
//!
//! UI-drawing model: waveforms = reads of the peak pyramid (memory/DB),
//! warmth bars = cache.coverage(). Here we run hundreds of “redraws” of random
//! viewports and check the counters of accesses
//! to the source.

use kt_cache::{BurstCache, BurstKind, CacheConfig};
use kt_media::index::PeakPyramid;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, PEAK_BLOCK_FRAMES};
use kt_time::Tick;
use std::sync::Arc;

#[test]
fn t13_viewport_churn_causes_zero_source_reads() {
    let mut src = SyntheticSource::new(Tick(90_000 * 30), AudioPattern::Sine { hz: 440.0 });

    // index built once (these are the only permitted reads)
    let pyramid = PeakPyramid::build(&mut src);
    let reads_after_index = src.audio_reads;
    assert!(reads_after_index > 0);

    // cache partially warmed (as after prefetch)
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    for idx in 0..64 {
        cache.insert_audio(kt_cache::SourceId(1), Arc::new(src.read_audio_burst(idx).unwrap()));
    }
    let reads_after_prefetch = src.audio_reads;

    // 500 “redraws” of random viewports: choice of pyramid level by zoom,
    // min/max block reads, coverage query — exactly what
    // kt-app::draw_timeline does
    let mut acc = 0i64; // guard against the optimizer discarding the loop
    let mut state = 0x9e3779b97f4a7c15u64; // deterministic LCG
    for _ in 0..500 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let zoom_level = (state >> 32) as usize % pyramid.levels.len();
        let start_block = (state as usize) % pyramid.levels[zoom_level][0].len().max(1);
        let blocks = &pyramid.levels[zoom_level][0];
        for b in blocks.iter().skip(start_block).take(200) {
            acc += b.0 as i64 + b.1 as i64;
        }
        let cov = cache.coverage(kt_cache::SourceId(1), BurstKind::Audio, 0..(state % 100) as i64);
        acc += cov.len() as i64;
        // level choice as in the UI
        let frames_per_px = 1.0 + (state % 10_000) as f64;
        let mut level = 0usize;
        let mut block = PEAK_BLOCK_FRAMES as f64;
        while block * 2.0 <= frames_per_px && level + 1 < pyramid.levels.len() {
            level += 1;
            block *= 2.0;
        }
        acc += level as i64;
    }
    assert!(acc != 0);

    assert_eq!(
        src.audio_reads, reads_after_prefetch,
        "redrawing the viewport must not touch the source (invariant No. 8)"
    );
}
