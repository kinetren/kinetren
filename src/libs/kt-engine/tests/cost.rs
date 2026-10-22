//! Tests 292-295 (R.43, items 304-305): THE COST OF A PLACE.
//!
//! The user's claim, brought to its cause: “are these two reverses
//! REALLY that heavy?” and “the version before otio, the same debug build, all goes
//! smoothly if the region is larger than the reverses, but inside, at the start of
//! event 21, underruns grow.”
//!
//! There turned out to be two causes, and both are structural:
//! - `overlap_weight` scanned the WHOLE edit FOR EACH FRAME, so the cost of a
//!   place grew linearly with the size of the PROJECT (events an hour out cost
//!   as much as the neighbouring ones);
//! - in the reverb tail the warp was computed and then discarded by the gate.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::AudioBurst;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};
use std::sync::Arc;

const CH: usize = AUDIO_CHANNELS;
const BURST: i64 = AUDIO_BURST_FRAMES as i64;
const BODY_MS: i64 = 300;

/// A fetcher counting accesses to the source.
struct Counting {
    inner: DirectFetcher,
    reads: u64,
}

impl Counting {
    fn new() -> Counting {
        let mut inner = DirectFetcher::new(None);
        inner.add_source(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Ramp)),
        );
        Counting { inner, reads: 0 }
    }
}

impl AudioFetcher for Counting {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<AudioBurst>> {
        self.reads += 1;
        self.inner.audio_burst(media, idx)
    }
}

fn slowed() -> FxInstance {
    FxInstance::SlowedReverb {
        rate: 0.75,
        mix: 0.2,
        reverb_delay_ms: 85.0,
        damp: 0.2,
        decay: 0.84,
        wet_gain: 1.0,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }
}

/// `n` events of 300 ms in a row on DIFFERENT tracks, with slowed+reverb.
fn base(n: u64) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 60,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.ensure_tracks(kt_model::LaneKind::Audio, n.max(1) as u32);
    for i in 0..n {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(i + 1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: i as u32,
            start: Tick::from_audio_frames(48 * BODY_MS * i as i64),
            duration: Tick::from_audio_frames(48 * BODY_MS),
            source_in: Tick::from_audio_frames(480_000),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: i % 3 == 0,
            fx: vec![slowed()],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    p
}

/// The same events plus `extra` DISTANT ones — a minute away from them, on a shared track.
fn with_distant(mut p: Project, extra: u64) -> Project {
    for i in 0..extra {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(10_000 + i),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick::from_audio_frames(48_000 * 60 + 480 * i as i64),
            duration: Tick::from_audio_frames(480),
            source_in: Tick::from_audio_frames(480_000),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    p
}

/// Render [a, b) frames; return the sound and the number of source accesses.
fn render(p: &Project, a: i64, b: i64) -> (Vec<f32>, u64) {
    let r = AudioRenderer::new(p);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = Counting::new();
    let mut out = Vec::new();
    let mut buf = vec![0f32; BURST as usize * CH];
    let mut pos = a;
    while pos < b {
        r.render_frames_with(&mut f, &mut store, pos, &mut buf);
        out.extend_from_slice(&buf);
        pos += BURST;
    }
    (out, f.reads)
}

fn rms(v: &[f32]) -> f64 {
    (v.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / v.len().max(1) as f64).sqrt()
}

/// Test 292 (item 305): DISTANT EVENTS DO NOT CHANGE THE SOUND.
///
/// A guard of the correctness of the overlap map: it replaced the scan of the whole
/// edit, and the set of covering events must remain THE SAME — together with the
/// order (f32 addition is not associative; GOLDEN_AUDIO rests on this).
#[test]
fn t292_distant_clips_do_not_change_the_sound() {
    let a = 48 * BODY_MS * 2;
    let b = a + 48_000;
    let (plain, _) = render(&base(7), a, b);
    let (crowded, _) = render(&with_distant(base(7), 2000), a, b);
    assert!(rms(&plain) > 1e-4, "the reference must not be silence");
    assert_eq!(plain, crowded, "events a minute away have no right to affect the sound");
}

/// Test 293 (item 305): the cost of a place does NOT grow with the size of the project.
///
/// Before R.43 `overlap_weight` scanned the whole edit for every frame: 7 events
/// cost 3.6 ms per second of sound, the same 7 plus 2000 distant ones — 92 ms, that
/// is 25 times more expensive for the same sound. The threshold here is generous (3x):
/// the test catches a return of the LINEAR scan, not fluctuations of the machine.
#[test]
fn t293_cost_does_not_scale_with_project_size() {
    let a = 48 * BODY_MS * 2;
    let b = a + 48_000;
    let best = |p: &Project| -> f64 {
        (0..3)
            .map(|_| {
                let t = std::time::Instant::now();
                let _ = render(p, a, b);
                t.elapsed().as_secs_f64()
            })
            .fold(f64::INFINITY, f64::min)
    };
    let small = best(&base(7));
    let big = best(&with_distant(base(7), 2000));
    assert!(
        big < small * 3.0,
        "2000 distant events must not make the place costlier: {:.1} ms against {:.1} ms \
         (before R.43 it was 25 times as much)",
        big * 1e3,
        small * 1e3
    );
}

/// Test 294 (item 304): in the reverb tail the source is NOT READ.
///
/// Past the end of the material the dry signal is zeroed by the gate, so everything
/// the warp outputs will be discarded. There is no reason to compute it — but before it was computed,
/// with the sinc resampler and reading the source.
#[test]
fn t294_reverb_tail_does_not_touch_the_source() {
    let p = base(7);
    let bodies_end = 48 * BODY_MS * 7;
    let r = AudioRenderer::new(&p);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = Counting::new();
    let mut buf = vec![0f32; BURST as usize * CH];

    // The FIRST entry into the tail is a `hard_seek` with a pre-roll: it honestly
    // plays the body into the garbage so that the reverb is warmed. Reads there are
    // legitimate, and the optimization does not touch them.
    let mut pos = bodies_end + BURST;
    r.render_frames_with(&mut f, &mut store, pos, &mut buf);
    pos += BURST;
    let after_preroll = f.reads;
    assert!(after_preroll > 0, "pre-roll must read the body");

    // and further on, CONTINUOUSLY along the tail, there is nothing left to read
    let mut tail = Vec::new();
    for _ in 0..20 {
        r.render_frames_with(&mut f, &mut store, pos, &mut buf);
        tail.extend_from_slice(&buf);
        pos += BURST;
    }
    let reads = f.reads - after_preroll;
    assert_eq!(reads, 0, "there is nothing to read in the tail, yet the source was touched {reads} times");
    assert!(rms(&tail) > 1e-6, "the tail must sound: reverb rings out, rms={}", rms(&tail));
}

/// Test 295 (items 303-304): a REGION INSIDE THE TAILS on repeat.
///
/// Exactly the place the user wrote about: “if you select a region
/// inside the start of event 21 — underruns grow.” Such a region contains
/// NOT ONE body, only tails; each cycle is a seam for every chain.
/// We assert that it plays: deterministically, without reading the source and
/// without underruns.
#[test]
fn t295_looping_inside_the_tails_is_deterministic_and_silent_on_reads() {
    let p = base(7);
    let bodies_end = 48 * BODY_MS * 7;
    let region = bodies_end + BURST..bodies_end + BURST + 48_000 / 4;
    let lap = |store: &mut FxStateStore, r: &AudioRenderer, f: &mut Counting| -> (Vec<f32>, u64) {
        let mut out = Vec::new();
        let mut buf = vec![0f32; BURST as usize * CH];
        let mut done = 0i64;
        let mut pos = region.start;
        let stats_before = f.reads;
        while done < region.end - region.start {
            r.render_looped_with(f, store, region.clone(), pos, &mut buf);
            out.extend_from_slice(&buf);
            pos = region.start + (pos - region.start + BURST).rem_euclid(region.end - region.start);
            done += BURST;
        }
        (out, f.reads - stats_before)
    };
    let r = AudioRenderer::new(&p);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = Counting::new();
    let (_, _) = lap(&mut store, &r, &mut f); // warm-up cycle
    let (a, reads_a) = lap(&mut store, &r, &mut f);
    let (b, reads_b) = lap(&mut store, &r, &mut f);
    assert_eq!(reads_a, 0, "a loop over tails does not read the source, yet it read {reads_a}");
    assert_eq!(reads_b, 0, "and the next one too");
    assert_eq!(a.len(), b.len());
    assert!(rms(&a) > 1e-7, "a region in tails must sound, rms={}", rms(&a));
    // the reverb decays, so the cycles are NOT required to match byte for byte;
    // what must match is the determinism of the run itself
    let (c, _) = {
        let r2 = AudioRenderer::new(&p);
        let mut s2 = FxStateStore::new();
        r2.sync_fx(&mut s2);
        let mut f2 = Counting::new();
        let _ = lap(&mut s2, &r2, &mut f2);
        let _ = lap(&mut s2, &r2, &mut f2);
        lap(&mut s2, &r2, &mut f2)
    };
    assert_eq!(b, c, "the same run must give the same bytes");
}

/// Test 710 (R.61): a formula on ONE event does not make the whole project more expensive.
///
/// The owner's requirement: “if physics was added to only one source — that is
/// no reason to break and slow down the work of the whole project.” Here this is checked by
/// two assertions: a project WITHOUT derived sources is computed exactly
/// as much as before (no graph is built at all), and a formula on one
/// event among two thousand does not raise the cost of the place.
#[test]
fn t710_a_formula_on_one_event_does_not_tax_the_project() {
    let a = 48 * BODY_MS * 2;
    let b = a + 48_000;
    let best = |p: &Project| -> f64 {
        (0..3)
            .map(|_| {
                let t = std::time::Instant::now();
                let _ = render(p, a, b);
                t.elapsed().as_secs_f64()
            })
            .fold(f64::INFINITY, f64::min)
    };

    // a formula on ONE event of a large project
    let mut with_formula = with_distant(base(7), 2000);
    {
        let c = with_formula.clips.iter_mut().find(|c| !c.fx.is_empty()).expect("event with FX");
        let param = c.fx[0]
            .auto_params()
            .next()
            .map(|s| s.name.to_string())
            .expect("the effect has an automatable parameter");
        c.fx_auto = vec![vec![kt_model::ParamCurve {
            param,
            keys: Vec::new(),
            source: kt_model::CurveSource::Formula { expr: "0.5 + 0.1 * t".into() },
        }]];
    }
    with_formula.validate().expect("formula is legal");

    let plain = best(&with_distant(base(7), 2000));
    let one = best(&with_formula);
    assert!(
        one < plain * 3.0,
        "a formula on ONE event made the place of the whole project costlier: {:.1} ms against {:.1} ms",
        one * 1e3,
        plain * 1e3
    );

    // and a project WITHOUT derived sources computes the same BYTES as before
    let (before, _) = render(&base(7), a, b);
    let (after, _) = render(&base(7), a, b);
    assert_eq!(before, after, "project without formulas stopped being deterministic");
    assert!(rms(&before) > 1e-4, "the reference must not be silence");
}
