//! Tests 334-336 (R.48, items 337-338): END-TO-END LOOKAHEAD of the chain.
//!
//! The point of the round in one phrase: an effect with an analysis window reads the source
//! AHEAD, and returns the output aligned — for the rest of the engine there is no delay.
//! Three assertions, one for each way of breaking this: t334 — the output did not shift;
//! t335 — the window really works (ceiling kept, reduction in time
//! before the peak, the event does not sound longer);
//! t336 — the prefetch warms what the chain actually reads.

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};
use std::sync::Arc;

const CH: usize = AUDIO_CHANNELS;
const DUR: i64 = 48_000 * 2;

/// Linear mode of the limiter: the threshold and ceiling at zero dB — the multiplier is
/// identically 1.0 on a signal below unity, and only the shift (if any) remains.
/// This property is exactly why the test exists.
fn limiter_linear() -> FxInstance {
    FxInstance::Limiter { threshold_db: 0.0, ceiling_db: 0.0, lookahead_ms: 5.0, release_ms: 80.0 }
}

fn project(fx: Vec<FxInstance>, pattern: &str) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), pattern));
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
        start: Tick(0),
        duration: Tick::from_audio_frames(DUR),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn source(pattern: AudioPattern) -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), pattern)));
    f
}

/// Render of the clip in one piece from position `from`.
fn render(fx: Vec<FxInstance>, pattern: AudioPattern, from: i64) -> Vec<f32> {
    let p = project(fx, "synth:clicks");
    let r = AudioRenderer::new(&p);
    let mut f = source(pattern);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; (DUR - from) as usize * CH];
    let stats = r.render_frames_with(&mut f, &mut store, from, &mut out);
    assert_eq!(stats.underruns, 0, "source fully available — there must be no misses");
    out
}

/// The same render, but requested in several consecutive pieces (a check that the state
/// does not depend on buffer slicing — class t325a).
fn render_chunked(fx: Vec<FxInstance>, pattern: AudioPattern, chunks: &[i64]) -> Vec<f32> {
    let p = project(fx, "synth:clicks");
    let r = AudioRenderer::new(&p);
    let mut f = source(pattern);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = Vec::new();
    let mut pos = 0i64;
    for &n in chunks {
        let mut buf = vec![0f32; n as usize * CH];
        r.render_frames_with(&mut f, &mut store, pos, &mut buf);
        out.extend_from_slice(&buf);
        pos += n;
    }
    out
}

fn peak(s: &[f32]) -> f32 {
    s.iter().fold(0f32, |m, v| m.max(v.abs()))
}

/// Test 334: LOOKAHEAD DOES NOT SHIFT THE OUTPUT.
///
/// A limiter with a 5 ms window (240 frames) in linear mode must give exactly what the
/// chain without it gives — sample for sample, not “similar”.
/// This is the PDC contract: the cost of the window is paid by reading the source ahead,
/// not by delaying the sound. (Red-proof: remove `+ la` from the input position of the
/// chunk in `produce` — the output drifts by 240 frames, the test turns red.)
#[test]
fn t334_lookahead_does_not_shift_the_output() {
    let dry = render(vec![], AudioPattern::Clicks, 0);
    let lim = render(vec![limiter_linear()], AudioPattern::Clicks, 0);
    assert_eq!(dry.len(), lim.len());
    assert_eq!(
        dry, lim,
        "the linear limiter must be an identity: lookahead is paid by the source, not by a shift"
    );

    // and the same on a warped chain: the input leads the warp, and the chunk position is
    // computed in its domain
    let warped = vec![FxInstance::slowed_reverb(0.8, 0.0, 60.0)];
    let mut with_lim = warped.clone();
    with_lim.push(limiter_linear());
    let a = render(warped, AudioPattern::Clicks, 0);
    let b = render(with_lim, AudioPattern::Clicks, 0);
    assert_eq!(a, b, "the linear limiter behind a warp must also be an identity");

    // buffer slicing does not affect the result (the lookahead lives in the node's ring,
    // not in the call counter)
    let one = render(vec![FxInstance::limiter()], AudioPattern::Clicks, 0);
    let many = render_chunked(
        vec![FxInstance::limiter()],
        AudioPattern::Clicks,
        &[1000, 17, 48_000, DUR - 49_017],
    );
    assert_eq!(one, many, "rendering in one piece must match rendering in pieces");

    // the layer strength over a lookahead node does not double the material: the dry
    // side is delayed by exactly the window the node consumed
    let mixed = render(
        vec![FxInstance::limiter(), FxInstance::LayerMix { amount: 0.5, from: 0 }],
        AudioPattern::Clicks,
        0,
    );
    let dry_c = render(vec![], AudioPattern::Clicks, 0);
    let wet = render(vec![FxInstance::limiter()], AudioPattern::Clicks, 0);
    for (i, ((m, d), w)) in mixed.iter().zip(&dry_c).zip(&wet).enumerate() {
        let want = 0.5 * d + 0.5 * w;
        assert!(
            (m - want).abs() <= 1e-6,
            "sample {i}: the mix took different moments of material ({m} instead of {want})"
        );
    }
}

/// Test 335: THE WINDOW WORKS.
///
/// The ceiling is kept strictly (the maximum window includes the output frame itself),
/// the reduction starts BEFORE the peak — the lookahead is paid for exactly this —
/// and the event does not start sounding longer: the lookahead does not enter
/// the tail.
#[test]
fn t335_limiter_ceiling_and_no_tail_growth() {
    let lim = FxInstance::Limiter {
        threshold_db: -12.0,
        ceiling_db: -6.0,
        lookahead_ms: 5.0,
        release_ms: 200.0,
    };
    let la = (5.0 / 1000.0 * 48_000.0) as usize;
    let out = render(vec![lim.clone()], AudioPattern::Clicks, 0);
    let ceiling = 10f32.powf(-6.0 / 20.0);
    let p = peak(&out);
    println!("t335 peak {p:.4} ceiling {ceiling:.4}");
    assert!(p <= ceiling + 1e-4, "ceiling breached: peak {p}, ceiling {ceiling}");

    // The reduction is in time BEFORE the peak — that is what the lookahead pays for.
    // Measured on the BACKGROUND (ClicksBed: clicks over a quiet sine): on a zero background
    // the gain is not visible, there would be nothing to divide by.
    let dry = render(vec![], AudioPattern::ClicksBed, 0);
    let wet = render(vec![lim.clone()], AudioPattern::ClicksBed, 0);
    let click = dry
        .chunks_exact(CH)
        .enumerate()
        .skip(48_000 / 2)
        .find(|(_, fr)| fr[0].abs() > 0.3 || fr[1].abs() > 0.3)
        .map(|(i, _)| i)
        .expect("the stimulus must contain a click");
    // window [peak − la, peak): the gain here must be lower than far from the
    // peak (there the limiter has released and holds the makeup)
    // The gain is measured by the ENERGY of a 64-frame window: the 200 Hz background every 120
    // frames passes through zero, and the ratio at a single point there is
    // undefined (the first version of the test caught exactly these zeros).
    let gain_at = |center: usize| -> f64 {
        let (a, b) = (center.saturating_sub(32), (center + 32).min(DUR as usize));
        let en = |s: &[f32]| -> f64 {
            s[a * CH..b * CH].iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>()
        };
        (en(&wet) / en(&dry).max(1e-30)).sqrt()
    };
    let near = gain_at(click - la / 2);
    let far = gain_at(click - la * 4);
    println!("t335 gain near {near:.3} (over {} frames), far {far:.3}", la / 2);
    assert!(
        near < far * 0.9,
        "the limiter must lower the volume BEFORE the peak: {near:.3} against {far:.3} far away"
    );

    // FOREIGN MATERIAL BEHIND THE END OF AN EVENT does not affect its sound: the gate
    // is computed in the INPUT domain, so past the end of the material the chain
    // gets silence, not what lies further in the file. Otherwise a click right past the
    // boundary would dampen the last frames of the event — the limiter's window would
    // “see” it. (Stimulus: clicks every 4800 frames, and DUR is a multiple of 4800 —
    // a click sits exactly on the boundary.)
    let end_gain = gain_at(DUR as usize - la / 2);
    println!("t335 gain at the very end {end_gain:.3}");
    assert!(
        end_gain > far * 0.9,
        "material past the end of the event dulled its tail: {end_gain:.3} against {far:.3}"
    );

    // tail: lookahead does NOT extend the event — past its end there is silence
    let auto = kt_fx::AutoProfiles::from_curves_rev(&[vec![]], 0, DUR, false);
    assert_eq!(
        kt_fx::chain_tail(std::slice::from_ref(&lim), &auto),
        Tick::ZERO,
        "the limiter has no tail: the analysis window is not ringing"
    );
    assert!(
        kt_fx::chain_lookahead(&[lim], &auto) > 0,
        "but it does have lookahead — otherwise the test checks nothing"
    );
}

/// Logger wrapper: writes every burst read.
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

/// Test 336: THE PREFETCH WARMS WHAT THE CHAIN READS.
///
/// The source window is widened forward by the lookahead in ONE function —
/// the one that both the render and the prefetch ask. If one widened only for the
/// render, the cache would warm the wrong bursts, and the mismatch would produce no error
/// and no underrun: past the window the source silently gives silence. So the test
/// checks that what is read is included in wanted, not just the sound.
/// (Red-proof: remove `+ chain_lookahead` from `chain_source_window_ext`.)
#[test]
fn t336_prefetch_window_covers_the_lookahead() {
    let p = project(vec![FxInstance::limiter()], "synth:clicks");
    let r = AudioRenderer::new(&p);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);

    for &pos in &[0i64, 4096, DUR / 2, DUR - 48_000] {
        store.reset_all();
        let frames = 48_000usize.min((DUR - pos) as usize);
        let mut fetcher = Recording { inner: source(AudioPattern::Clicks), log: Vec::new() };
        let mut out = vec![0f32; frames * CH];
        r.render_frames_with(&mut fetcher, &mut store, pos, &mut out);

        let wanted = r.wanted_audio_bursts(pos..pos + frames as i64);
        for &(media, idx) in &fetcher.log {
            let covered = wanted.iter().any(|(m, range)| *m == media && range.contains(&idx));
            assert!(
                covered,
                "burst {idx} was read by the chain with lookahead, but is not in wanted({pos}..{})",
                pos + frames as i64
            );
        }
    }

    // and vice versa: warming strictly by wanted gives a render with no misses that
    // is byte for byte equal to a cold one
    let pos = DUR / 3;
    store.reset_all();
    let mut cold = vec![0f32; 48_000 * CH];
    r.render_frames_with(&mut source(AudioPattern::Clicks), &mut store, pos, &mut cold);

    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let mut warm = DirectFetcher::new(Some(cache.clone()));
    warm.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Clicks)),
    );
    for (media, range) in r.wanted_audio_bursts(pos..pos + 48_000) {
        for idx in range {
            warm.audio_burst(media, idx);
        }
    }
    let mut only_cache = kt_engine::CacheFetcher::new(cache);
    store.reset_all();
    let mut hot = vec![0f32; 48_000 * CH];
    let stats = r.render_frames_with(&mut only_cache, &mut store, pos, &mut hot);
    assert_eq!(stats.underruns, 0, "warm-up by wanted must cover the read of the chain");
    assert_eq!(hot, cold, "the same piece from the warm cache must be byte-for-byte");
}
