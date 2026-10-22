//! Tests 166–168 (docs/porting-notes.md) — regression of R.28 playback,
//! complaints 8.1–8.4 (“the computer is noisy”, underruns grow, “the picture freezes”).
//!
//! Root cause: the wrap of the loop region tore the clip-local position of the FX clip, and
//! `ClipProcessor` treated it as a seek — a full `Freeverb::reset`,
//! re-creating the sinc resampler and a pre-roll of ≈1.25 s ON EVERY CYCLE (for a
//! region shorter than a burst — several times within one callback). The pre-roll at
//! that point read the source BEYOND the warmed window, and each of its misses
//! went into `underruns`.
//!
//! 166 — FX in the loop region: the wrap is a SEAM (the reverb rings on through the cycle),
//!       warm-up misses are not underruns;
//! 167 — a region shorter than a burst: no repeated warm-ups, no avalanche of reads;
//! 168 — a complex edit (overlaps, transitions, fades, FX, reverse): a linear run
//!       and a loop without a single underrun with an available source.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore, RenderStats};
use kt_media::AudioBurst;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick};
use std::sync::Arc;

const CH: usize = AUDIO_CHANNELS;
const BURST: i64 = AUDIO_BURST_FRAMES as i64;

/// Fetcher over DirectFetcher: counts the accesses and can “not have” bursts
/// below `floor` — a model of a cold cache to the left of the warmed window
/// (that is exactly where the pre-roll goes after a hard_seek).
struct Probe {
    inner: DirectFetcher,
    floor: i64,
    fetches: u64,
    misses: u64,
}

impl Probe {
    fn new(pattern: AudioPattern, floor: i64) -> Probe {
        let mut inner = DirectFetcher::new(None);
        inner.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), pattern)));
        inner.add_source(
            MediaId(2),
            Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 220.0 })),
        );
        Probe { inner, floor, fetches: 0, misses: 0 }
    }
}

impl AudioFetcher for Probe {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<AudioBurst>> {
        self.fetches += 1;
        if idx < self.floor {
            self.misses += 1;
            return None;
        }
        self.inner.audio_burst(media, idx)
    }
}

fn base_project() -> Project {
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
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.media.push(MediaRef::new(MediaId(2), "synth:sine"));
    p
}

fn clip(id: u64, media: u64, lane: u32, start: i64, dur: i64, src_in: i64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
        start: Tick::from_audio_frames(start),
        duration: Tick::from_audio_frames(dur),
        source_in: Tick::from_audio_frames(src_in),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn slowed_reverb(rate: f64, mix: f32) -> FxInstance {
    FxInstance::SlowedReverb {
        rate,
        mix,
        reverb_delay_ms: 60.0,
        damp: 0.4,
        decay: 0.7,
        wet_gain: 1.0,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }
}

/// One output burst from position `at`, with the wrap across the region.
fn looped_burst(
    r: &AudioRenderer,
    f: &mut dyn AudioFetcher,
    store: &mut FxStateStore,
    region: std::ops::Range<i64>,
    at: i64,
    out: &mut [f32],
) -> RenderStats {
    r.render_looped_with(f, store, region, at, out)
}

// ---- Test 166: FX in the loop region -------------------------------------

#[test]
fn t166_loop_wrap_is_a_seam_not_a_seek() {
    let mut p = base_project();
    let dur = 10 * AUDIO_SAMPLE_RATE as i64;
    let mut c = clip(1, 1, 0, 0, dur, 0);
    c.fx = vec![slowed_reverb(0.8, 0.5)];
    p.clips.push(c);
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);

    // a region of ~2 s offset from the burst boundary (the start is not aligned);
    // its length is a multiple of the burst, so that the head of the second cycle falls exactly on
    // a call boundary — otherwise there is nothing to compare its head with the first one's
    let region = {
        let s = 3 * AUDIO_SAMPLE_RATE as i64 + 137;
        s..s + 94 * BURST
    };
    // warmed exactly from the burst at which the source READ begins at the start of the region
    // (rate 0.8 → the source runs slower than the output); everything to the left is “cold”: only the
    // pre-roll goes there
    let floor = (region.start as f64 * 0.8) as i64 / BURST;
    let mut f = Probe::new(AudioPattern::Sine { hz: 440.0 }, floor);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);

    let mut out = vec![0f32; BURST as usize * CH];
    // the first burst: a cold start — the pre-roll is legitimate and must miss
    let first = looped_burst(&r, &mut f, &mut store, region.clone(), region.start, &mut out);
    assert_eq!(first.underruns, 0, "PRE-ROLL misses are not underruns (8.1)");
    assert!(first.warmup_misses > 0, "pre-roll after a cold start must be present");
    let head_turn1 = out.clone();

    // we spin ~3 cycles; a wrap must not produce ANY new warm-up
    let span = region.end - region.start;
    let turns = 3;
    let bursts = (span * turns) / BURST;
    let mut head_turn2: Option<Vec<f32>> = None;
    let mut pos = region.start + BURST;
    let mut total = RenderStats::default();
    for i in 0..bursts {
        let s = looped_burst(&r, &mut f, &mut store, region.clone(), pos, &mut out);
        total.underruns += s.underruns;
        total.warmup_misses += s.warmup_misses;
        // a burst that lies entirely on the start of the second cycle
        let tl = region.start + (pos - region.start).rem_euclid(span);
        if head_turn2.is_none() && i > 4 && tl == region.start {
            head_turn2 = Some(out.clone());
        }
        pos += BURST;
    }
    assert_eq!(total.underruns, 0, "there must be no underruns at the wrap (8.1)");
    assert_eq!(
        total.warmup_misses, 0,
        "the wrap is a SEAM: not a single new pre-roll over {turns} lap(s) (8.2/8.4)"
    );

    // the FX state survived the wrap: the same position on the second cycle sounds
    // DIFFERENT from a cold start — the reverb rings on through the seam
    if let Some(h2) = head_turn2 {
        let diff =
            h2.iter().zip(head_turn1.iter()).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        assert!(diff > 1e-6, "the reverb tail must survive the wrap (a seam, not a reset)");
        assert!(h2.iter().all(|s| s.is_finite() && s.abs() <= 4.0), "the seam gives no outliers");
    } else {
        panic!("over {turns} lap(s) the second lap must have started");
    }
}

// ---- Test 167: region shorter than a burst ----------------------------------

#[test]
fn t167_short_region_does_not_re_seek_every_wrap() {
    let mut p = base_project();
    let dur = 4 * AUDIO_SAMPLE_RATE as i64;
    let mut c = clip(1, 1, 0, 0, dur, 0);
    c.fx = vec![slowed_reverb(1.0, 0.4)]; // without a warp, but with reverb
    p.clips.push(c);
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);

    // a region of 700 frames — SHORTER than a burst: previously each output burst
    // accounted for two hard_seeks with a pre-roll of 60187 frames
    let region = AUDIO_SAMPLE_RATE as i64..AUDIO_SAMPLE_RATE as i64 + 700;
    let floor = region.start / BURST;
    let mut f = Probe::new(AudioPattern::Sine { hz: 330.0 }, floor);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);

    let mut out = vec![0f32; BURST as usize * CH];
    let bursts = 20i64;
    let mut total = RenderStats::default();
    let mut first_warmup = 0u64;
    let mut energy = 0f64;
    for i in 0..bursts {
        let s = looped_burst(
            &r,
            &mut f,
            &mut store,
            region.clone(),
            region.start + i * BURST,
            &mut out,
        );
        total.underruns += s.underruns;
        // the first burst is a legitimate cold pre-roll, there are no warm-ups after it
        if i == 0 {
            first_warmup = s.warmup_misses;
        } else {
            total.warmup_misses += s.warmup_misses;
        }
        energy += out.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>();
    }
    assert!(first_warmup > 0, "a cold start must warm up");
    assert_eq!(total.underruns, 0, "a short region must not give underruns (8.4)");
    assert_eq!(total.warmup_misses, 0, "there are no repeated warm-ups at the wrap");
    assert!(energy > 1.0, "playback must not degenerate into silence (8.3)");
    // An avalanche of reads: one hard_seek is ≈59 pre-roll bursts, and for 20
    // output bursts there are ~30 wraps. The real cost is a few reads per burst
    // (the region as a whole lies in two source bursts).
    assert!(
        f.fetches < 20 * bursts as u64,
        "an avalanche of source reads: {} accesses for {bursts} bursts",
        f.fetches
    );
}

// ---- Test 168: complex edit ----------------------------------------------

#[test]
fn t168_complex_montage_plays_without_underruns() {
    let sr = AUDIO_SAMPLE_RATE as i64;
    let mut p = base_project();
    p.overlap_layers = true;
    // 12 clips: two tracks, overlaps, FX, reverse, different gains
    for i in 0..6i64 {
        let mut c = clip(1 + i as u64, 1 + (i % 2) as u64, 0, i * sr - i * 900, sr + 400, i * 1000);
        c.gain_db = -3.0 * (i % 3) as f32;
        if i % 2 == 0 {
            c.fx = vec![slowed_reverb(if i == 0 { 0.75 } else { 1.0 }, 0.35)];
        }
        c.reversed = i == 3;
        p.clips.push(c);
    }
    for i in 0..6i64 {
        let mut c =
            clip(10 + i as u64, 2 - (i % 2) as u64, 1, i * sr + 300, sr - 200, i * 700 + 50);
        c.gain_db = -1.5;
        p.clips.push(c);
    }
    // crossfade at the junction of a pair of clips on the second track + fade on the first clip
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: None,
        b: Some(ClipId(10)),
        in_offset: Tick::ZERO,
        out_offset: Tick::from_audio_frames(2000),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.normalize(); // tracks fitted to the content (R.62)
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);

    let mut f = Probe::new(AudioPattern::Ramp, i64::MIN);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; BURST as usize * CH];

    // 1) a linear run of the whole edit
    let end = p.clips.iter().map(|c| (c.start + c.duration).to_audio_frames()).max().unwrap();
    let mut total = RenderStats::default();
    let mut pos = 0i64;
    while pos < end {
        let s = r.render_frames_with(&mut f, &mut store, pos, &mut out);
        total.underruns += s.underruns;
        pos += BURST;
    }
    assert_eq!(total.underruns, 0, "a full edit with an available source — no underruns");

    // 2) a loop across the junction with a transition and overlaps, 3 cycles
    let region = sr + 12_000..3 * sr + 5_000;
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut looped = RenderStats::default();
    let turns = 3;
    let bursts = ((region.end - region.start) * turns) / BURST;
    for i in 0..bursts {
        let s = looped_burst(
            &r,
            &mut f,
            &mut store,
            region.clone(),
            region.start + i * BURST,
            &mut out,
        );
        looped.underruns += s.underruns;
        if i > 0 {
            looped.warmup_misses += s.warmup_misses;
        }
    }
    assert_eq!(looped.underruns, 0, "a loop over a complex edit — no underruns (8.2)");
}

// ---- Prefetch lookahead window (item 167) ----------------------------------

#[test]
fn t167b_prefetch_lookahead_stays_inside_loop_region() {
    use kt_engine::lookahead_windows;
    // cursor outside the region — a linear window
    assert_eq!(lookahead_windows(10, 100, Some((500, 900))), vec![10..110]);
    assert_eq!(lookahead_windows(10, 100, None), vec![10..110]);
    // inside the region — to the end and the wrap into the head
    assert_eq!(lookahead_windows(800, 300, Some((500, 900))), vec![800..900, 500..700]);
    // a window shorter than the remainder of the region — one range
    assert_eq!(lookahead_windows(600, 100, Some((500, 900))), vec![600..700]);
    // a window longer than a cycle — no more than a cycle (otherwise dozens of identical orders)
    assert_eq!(lookahead_windows(500, 10_000, Some((500, 900))), vec![500..900]);
    // the region's end edge is inclusive (transport semantics, item 132):
    // a cursor at the end is already the head of the next cycle
    assert_eq!(lookahead_windows(900, 100, Some((500, 900))), vec![500..600]);
}

// ---- Test 175: “black box” of playback (item 10) ------------------------

/// User: “we need to think through a clever logging system that
/// will help future agents with floating playback bugs; it must be done compactly,
/// because I can take and jump over several super-small loop regions, which could
/// produce tons of logs.”
/// Verified exactly this: a thousand events do not produce a thousand lines.
#[test]
fn t175_flight_recorder_stays_compact() {
    use kt_engine::flight::{CAP, Ev, FlightRecorder, Snap};
    let f = FlightRecorder::new();

    // 1000 wraps of one loop — ONE line with a counter
    for _ in 0..1000 {
        f.push_at(0, Ev::Wrap, 48_000, 0, Snap::default());
    }
    let recs = f.records();
    assert_eq!(recs.len(), 1, "consecutive identical events are deduplicated");
    assert_eq!(recs[0].n, 1000);
    assert!(f.dump().contains("×1000"), "repeats are visible as a count: {}", f.dump());

    // jumps across DIFFERENT small regions are still limited by the ring
    for i in 0..(CAP as i64 * 4) {
        f.push_at(i as u64, Ev::Region, i * 100, i * 100 + 700, Snap::default());
    }
    assert_eq!(f.records().len(), CAP, "the ring does not grow: {} entries", f.records().len());

    // an empty snapshot is not written at all (the log is silent in silence)
    f.clear();
    f.push_snap(Snap { head_frames: 48_000, ..Default::default() });
    assert!(f.records().is_empty(), "a snapshot without a single miss is not written");
    f.push_snap(Snap { render_underruns: 3, head_frames: 48_000, ..Default::default() });
    assert_eq!(f.records().len(), 1, "a snapshot with misses is written");

    // the dump is readable by a human and contains the essence
    f.clear();
    f.push_at(0, Ev::Play, 0, 0, Snap::default());
    f.push_at(100, Ev::Region, 48_000, 96_000, Snap::default());
    f.push_at(200, Ev::Stall, 1200, 48_000, Snap::default());
    let d = f.dump();
    assert!(d.contains("PLAY"), "{d}");
    assert!(d.contains("REGION") && d.contains("1.000..2.000"), "region in seconds: {d}");
    assert!(d.contains("STALL") && d.contains("1200 ms"), "stickiness with duration: {d}");
}

// ---- Test 280: the underrun unit is one for both paths (R.43, item 289) ----------

/// The user's claim: “specifically in this place underruns broke a very long time ago,
/// we will not do a bisect.”
///
/// The bisect is not needed: this is not a regression but DIFFERENT UNITS from the very start.
/// `render_plain_clip` (a clip without FX) counted a miss in FRAMES of the body, and
/// `fill_source` (a clip with FX) — one unit per BURST miss. The same starvation gave numbers
/// that differ by `AUDIO_BURST_FRAMES` = 1024 times, and they could not be compared
/// either between events or over time.
///
/// The test asserts the CONSEQUENCE: two identical events on a fully cold source give
/// comparable numbers. Exact equality is not required — the path with FX pulls the source
/// through the warp and cuts the reads differently — but the difference must be
/// several-fold, not a thousand-fold.
#[test]
fn t280_underruns_are_counted_in_frames_on_both_paths() {
    let sr = AUDIO_SAMPLE_RATE as i64;
    let span = sr / 2; // half a second of body

    let mut plain = base_project();
    plain.clips.push(clip(1, 1, 0, 0, span, 0));
    plain.validate().unwrap();

    let mut fxp = base_project();
    let mut c = clip(1, 1, 0, 0, span, 0);
    c.fx = vec![slowed_reverb(1.0, 0.0)];
    fxp.clips.push(c);
    fxp.validate().unwrap();

    // the source is unavailable ENTIRELY: floor above any burst
    let run = |p: &Project| -> u64 {
        let r = AudioRenderer::new(p);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut f = Probe::new(AudioPattern::Ramp, i64::MAX);
        let mut out = vec![0f32; BURST as usize * CH];
        let mut total = 0u64;
        let mut pos = 0i64;
        while pos < span {
            total += r.render_frames_with(&mut f, &mut store, pos, &mut out).underruns;
            pos += BURST;
        }
        total
    };

    let a = run(&plain);
    let b = run(&fxp);
    assert!(a > 0 && b > 0, "a fully cold source must give underruns: {a} and {b}");
    // both — in frames, so both are of the same order as the length of the body
    let (lo, hi) = (a.min(b), a.max(b));
    assert!(
        hi <= lo * 4,
        "the counter unit is the same on both paths: without FX {a}, with FX {b} \
         (before R.43 it was ~{span} against ~{}, a 1024-fold difference)",
        span / BURST
    );
    assert!(
        lo >= span as u64 / 4,
        "the counter in FRAMES: over {span} frames of the body we expect hundreds, not units — {a}/{b}"
    );
}
