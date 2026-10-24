//! Test 17: consistency of the prefetch window with the ACTUAL
//! consumption of the source by the warp (regression of item 13 — “after random jumps
//! slowed&reverb plays silence”: the linear mapping of wanted warmed the wrong bursts).

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::{AudioFetcher, AudioRenderer, CacheFetcher, DirectFetcher};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};
use std::sync::Arc;

const CH: usize = AUDIO_CHANNELS;
const CLIP_FRAMES: i64 = 48_000 * 6; // 6 seconds

fn slowed_project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick::from_audio_frames(0),
        duration: Tick::from_audio_frames(CLIP_FRAMES),
        source_in: Tick::from_audio_frames(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![FxInstance::SlowedReverb {
            rate: 0.5,
            mix: 0.15,
            reverb_delay_ms: 85.0,
            damp: 0.32,
            decay: 0.58,
            wet_gain: 0.95,
            predelay_ms: 0.0,
            width: 1.0,
            frames: Default::default(),
            automation: vec![
                SlowedKeyframe { t: 0.0, rate: 0.9, mix: 0.15 },
                SlowedKeyframe { t: 0.5, rate: 0.4, mix: 0.15 },
                SlowedKeyframe { t: 1.0, rate: 0.8, mix: 0.15 },
            ],
        }],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn direct() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 220.0 })),
    );
    f
}

/// Logger wrapper: writes each burst read.
struct Recording {
    inner: DirectFetcher,
    log: Vec<(MediaId, i64)>,
}

impl AudioFetcher for Recording {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<kt_media::AudioBurst>> {
        self.log.push((media, idx));
        self.inner.audio_burst(media, idx)
    }
}

fn rms(buf: &[f32]) -> f64 {
    (buf.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / buf.len() as f64).sqrt()
}

/// Main invariant: wanted ⊇ consumed under any jumps.
#[test]
fn t17_wanted_bursts_cover_fx_consumption_after_random_seeks() {
    let project = slowed_project();
    let renderer = AudioRenderer::new(&project);
    let mut store = kt_engine::FxStateStore::new();
    renderer.sync_fx(&mut store);

    // deterministic “random” positions: LCG
    let mut x: u64 = 0x2545F4914F6CDD1D;
    let mut positions = Vec::new();
    for _ in 0..8 {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        positions.push(((x >> 20) as i64) % (CLIP_FRAMES - 48_000));
    }
    positions.extend([0, (CLIP_FRAMES as f64 * 0.95) as i64]);

    for &pos in &positions {
        store.reset_all(); // as player.seek: FX-state reset, hard_seek + preroll
        let mut fetcher = Recording { inner: direct(), log: Vec::new() };
        let frames = 48_000 / 2;
        let mut out = vec![0f32; frames * CH];
        renderer.render_frames_with(&mut fetcher, &mut store, pos, &mut out);

        let wanted = renderer.wanted_audio_bursts(pos..pos + frames as i64);
        for &(media, idx) in &fetcher.log {
            let covered = wanted.iter().any(|(m, r)| *m == media && r.contains(&idx));
            assert!(
                covered,
                "burst (media {:?}, idx {idx}) was read by the render, but is NOT in \
                 wanted_audio_bursts({pos}..{}) — prefetch would warm the wrong bursts",
                media,
                pos + frames as i64
            );
        }
    }
}

/// Warming the cache strictly by wanted → render without misses, byte-for-byte equal
/// to the cold reference, and it is not silence.
#[test]
fn t17_seek_into_slowed_clip_from_warm_cache_matches_cold_reference() {
    let project = slowed_project();
    let renderer = AudioRenderer::new(&project);
    let frames = 48_000usize; // window 1 s
    let positions = [
        (CLIP_FRAMES as f64 * 0.3) as i64,
        (CLIP_FRAMES as f64 * 0.6) as i64,
        (CLIP_FRAMES as f64 * 0.93) as i64,
    ];

    for &pos in &positions {
        // cold reference
        let mut cold_store = kt_engine::FxStateStore::new();
        renderer.sync_fx(&mut cold_store);
        let mut cold = vec![0f32; frames * CH];
        renderer.render_frames_with(&mut direct(), &mut cold_store, pos, &mut cold);

        // cache warm-up exactly by wanted
        let cache = Arc::new(BurstCache::new(CacheConfig::default()));
        let mut warm = DirectFetcher::new(Some(cache.clone()));
        warm.add_source(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 220.0 })),
        );
        for (media, range) in renderer.wanted_audio_bursts(pos..pos + frames as i64) {
            for idx in range {
                warm.audio_burst(media, idx);
            }
        }

        // render only from cache
        let mut store = kt_engine::FxStateStore::new();
        renderer.sync_fx(&mut store);
        let mut fetcher = CacheFetcher::new(cache);
        let mut out = vec![0f32; frames * CH];
        let stats = renderer.render_frames_with(&mut fetcher, &mut store, pos, &mut out);

        assert_eq!(fetcher.miss_count(), 0, "pos {pos}: misses after warm-up by wanted");
        assert_eq!(stats.underruns, 0, "pos {pos}: underruns after warm-up");
        assert_eq!(out, cold, "pos {pos}: the warm render diverged from the cold reference");
        assert!(rms(&out) > 0.01, "pos {pos}: the render must not be silence, rms={}", rms(&out));
    }
}

/// The end of the clip must not fade out silently (regression “truncated tail”:
/// the hard source window muted the sinc priming at the end).
#[test]
fn t17_slowed_clip_end_has_no_silent_tail() {
    let project = slowed_project();
    let renderer = AudioRenderer::new(&project);
    let mut store = kt_engine::FxStateStore::new();
    renderer.sync_fx(&mut store);

    let tail_frames = AUDIO_BURST_FRAMES;
    let mid_pos = CLIP_FRAMES / 2;
    let end_pos = CLIP_FRAMES - tail_frames as i64;

    let mut mid = vec![0f32; tail_frames * CH];
    renderer.render_frames_with(&mut direct(), &mut store, mid_pos, &mut mid);
    store.reset_all();
    let mut end = vec![0f32; tail_frames * CH];
    renderer.render_frames_with(&mut direct(), &mut store, end_pos, &mut end);

    let (rm, re) = (rms(&mid), rms(&end));
    assert!(rm > 0.01, "the middle of the clip is not silence, rms={rm}");
    assert!(
        re > rm * 0.5,
        "the last burst of the clip dropped by more than 6 dB against the middle: mid={rm:.4} end={re:.4}"
    );
}

/// Test 285 (R.43 item 295): on the REVERSE TRANSPORT the prefetch warms the same thing
/// that the render reads.
///
/// The user’s complaint about reverses came in the same batch as underruns.
/// The cause is separate from the mirror (item 292): the reverse transport plays ALL
/// events through `render_plain_clip` — dry and LINEAR; this path knows nothing of the warp
/// at all (“like Vegas”), while `wanted_audio_bursts` counted the window BY THE WARP.
/// On a slowed event these are two different spans of the source: the prefetch warmed
/// one set of bursts, the render asked for others, and the miss was permanent.
/// It is counted there in FRAMES, up to 1024 per skipped burst — hence the
/// growth rate of the counter.
///
/// The claim is the same as t17, but for the raw path: wanted ⊇ consumed.
#[test]
fn t285_raw_reverse_prefetch_covers_what_raw_render_reads() {
    let project = slowed_project();
    let renderer = AudioRenderer::new(&project);
    let frames = 48_000usize / 2;

    for &pos in &[0i64, CLIP_FRAMES / 3, CLIP_FRAMES / 2, (CLIP_FRAMES as f64 * 0.9) as i64] {
        let mut fetcher = Recording { inner: direct(), log: Vec::new() };
        let mut out = vec![0f32; frames * CH];
        renderer.render_frames_raw(&mut fetcher, pos, &mut out);

        let raw = renderer.wanted_audio_bursts_raw(pos..pos + frames as i64);
        for &(media, idx) in &fetcher.log {
            assert!(
                raw.iter().any(|(m, r)| *m == media && r.contains(&idx)),
                "pos {pos}: burst {idx} was read by the raw render, but is not in \
                 wanted_audio_bursts_raw — prefetch would warm the wrong bursts"
            );
        }

        // and we show WHAT exactly was broken: the warp window does not cover this burst,
        // so on reverse the miss was inevitable
        let warped = renderer.wanted_audio_bursts(pos..pos + frames as i64);
        let missed = fetcher
            .log
            .iter()
            .filter(|(m, idx)| !warped.iter().any(|(wm, r)| wm == m && r.contains(idx)))
            .count();
        if pos > 0 {
            assert!(
                missed > 0,
                "pos {pos}: if the warp window covers everything anyway, the test guards nothing"
            );
        }
    }
}
