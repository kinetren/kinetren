//! Tests 2062–2068 (builder-cost-guard): THE WORK OF A BUILD GROWS WITH THE PROJECT, NOT WITH ITS
//! SQUARE.
//!
//! `AudioRenderer::new` took 393–488 ms on the owner's project against 1 ms on the heavy showcase:
//! a helper that rebuilt a whole structure, called once per element of another (the speed curve's
//! segment list per sampled point). Such a mechanism is invisible on hand-made data, so each axis
//! here is GENERATED at ×1, ×4 and ×16 — events with transitions, the length of a shaped signed
//! speed curve, a reel's placements, a reel of placements of a reel, a cutting morpher layer over
//! the events, linked curves — and every builder and per-build pass is run on it.
//!
//! What is read is a COUNT, never a clock (t1671): the elements whole-structure walks touched
//! (`kt_time::cost`) and the bytes allocated on this thread (a counting allocator below). Both are
//! pure functions of the project. The bound is linear with a factor of two for sorting and
//! constant terms: ×16 may cost at most 32× the ×1 work; a quadratic walk costs ~256×.

use kt_model::*;
use kt_time::Tick;
use kt_time::cost::{Cost, Costs};
use std::cell::Cell;

// ---- bytes allocated on this thread --------------------------------------------------------

struct Counting;

thread_local! {
    static BYTES: Cell<u64> = const { Cell::new(0) };
}

unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        let _ = BYTES.try_with(|b| b.set(b.get() + l.size() as u64));
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        let _ = BYTES.try_with(|b| b.set(b.get() + n as u64));
        unsafe { std::alloc::System.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// What one run of `f` cost on this thread: the walks' elements and the bytes allocated.
#[derive(Debug, Clone, Copy)]
struct Work {
    walks: Costs,
    bytes: u64,
}

impl Work {
    fn walked(&self) -> u64 {
        self.walks.iter().sum()
    }
}

fn work<R>(f: impl FnOnce() -> R) -> Work {
    let b0 = BYTES.with(|b| b.get());
    let (r, walks) = kt_time::cost::meter(f);
    let bytes = BYTES.with(|b| b.get()) - b0;
    drop(r);
    Work { walks, bytes }
}

// ---- the scenes ------------------------------------------------------------------------------

const SEC: i64 = 90_000;

fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 25,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p
}

fn ev(id: u64, kind: ClipKind, lane: u32, start: i64, dur: i64, media: u64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: Vec::new(),
        opacity: 1.0,
        fx_off: false,
    }
}

fn slowed() -> FxInstance {
    FxInstance::plugin_with("slowed_reverb", &[0.75, 0.2, 85.0, 0.2, 0.84, 1.0, 0.0, 0.0, 1.0]).unwrap()
}

/// A speed curve over `[0, dur)` with a SHAPED segment between every two keys (sampled every
/// 5 ms by `rate_profile_keys`), swinging through zero so the address map (`sign_knots`) is built.
fn shaped_rate(dur: i64, every: i64) -> ParamCurve {
    let mut keys = Vec::new();
    let mut t = 0;
    let mut i = 0;
    while t <= dur {
        let v = if i % 2 == 0 { 1.5 } else { -0.8 };
        keys.push(AutoKey::shaped(Tick(t), v, SegShape::Smooth { tension: 1.0 }));
        t += every;
        i += 1;
    }
    ParamCurve { param: "rate".into(), keys, source: Default::default() }
}

/// A linked A/V pair at `start` with a slowed chain and a shaped speed curve.
fn pair(p: &mut Project, id: u64, lane: u32, start: i64, dur: i64, curve_every: i64) {
    for (k, kind) in [(0, ClipKind::Video), (1, ClipKind::Audio)] {
        let mut c = ev(id + k, kind, lane, start, dur, 1);
        c.link_group = Some(GroupId(id));
        c.fx = vec![slowed()];
        c.fx_auto = vec![vec![shaped_rate(dur, curve_every)]];
        p.clips.push(c);
    }
}

fn crossfade(p: &mut Project, id: u64, a: u64, b: u64) {
    p.transitions.push(TransitionEvent {
        id: TransitionId(id),
        a: Some(ClipId(a)),
        b: Some(ClipId(b)),
        in_offset: Tick(SEC / 10),
        out_offset: Tick(SEC / 10),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
}

/// `n` pairs butted on two lanes, a crossfade at every joint of each kind.
fn events(n: u64) -> Project {
    let mut p = project();
    p.ensure_tracks(LaneKind::Video, 1);
    p.ensure_tracks(LaneKind::Audio, 1);
    for i in 0..n {
        pair(&mut p, 10 * i + 10, 0, i as i64 * SEC, SEC, SEC / 2);
        if i > 0 {
            for k in 0..2 {
                crossfade(&mut p, 2 * i + k, 10 * (i - 1) + 10 + k, 10 * i + 10 + k);
            }
        }
    }
    p
}

/// One pair whose shaped speed curve runs `secs` seconds.
fn long_curve(secs: u64) -> Project {
    let secs = secs as i64;
    let mut p = project();
    p.ensure_tracks(LaneKind::Video, 1);
    p.ensure_tracks(LaneKind::Audio, 1);
    pair(&mut p, 10, 0, 0, secs * SEC, 2 * SEC);
    p
}

/// A reel of `n` A/V pairs placed `n` times: the project grows as placements AND reel content,
/// and a cost per placement × per inner event grows with its square.
fn placements(n: u64) -> Project {
    let mut p = project();
    p.sequences.push(reel(100, n));
    p.ensure_tracks(LaneKind::Video, 1);
    p.ensure_tracks(LaneKind::Audio, 1);
    let len = n as i64 * SEC;
    for i in 0..n {
        p.clips.push(ev(10 + 2 * i, ClipKind::Video, 0, i as i64 * len, len, 100));
        p.clips.push(ev(11 + 2 * i, ClipKind::Audio, 0, i as i64 * len, len, 100));
    }
    p
}

fn reel(id: u64, n: u64) -> Sequence {
    let mut s = Sequence::new(MediaId(id), "Reel");
    s.ensure_tracks(LaneKind::Video, 1);
    s.ensure_tracks(LaneKind::Audio, 1);
    for i in 0..n {
        s.clips.push(ev(100_000 + 2 * i, ClipKind::Video, 0, i as i64 * SEC, SEC, 1));
        s.clips.push(ev(100_001 + 2 * i, ClipKind::Audio, 0, i as i64 * SEC, SEC, 1));
    }
    s
}

/// A reel B holding `n` placements of a reel A of `n` pairs, B placed once.
fn nested(n: u64) -> Project {
    let mut p = project();
    p.sequences.push(reel(100, n));
    let mut b = Sequence::new(MediaId(200), "Outer");
    b.ensure_tracks(LaneKind::Video, 1);
    let len = n as i64 * SEC;
    for i in 0..n {
        b.clips.push(ev(5000 + i, ClipKind::Video, 0, i as i64 * len, len, 100));
    }
    p.sequences.push(b);
    p.ensure_tracks(LaneKind::Video, 1);
    p.clips.push(ev(10, ClipKind::Video, 0, 0, n as i64 * len, 200));
    p
}

/// `n` butted video events with crossfades, under one morpher layer that CUTS (a warp in its
/// patch) — the cut-point pass asks the forbidden zones of every target.
fn layered(n: u64) -> Project {
    let mut p = project();
    p.ensure_tracks(LaneKind::Video, 1);
    for i in 0..n {
        p.clips.push(ev(10 + i, ClipKind::Video, 0, i as i64 * SEC, SEC, 1));
        if i > 0 {
            crossfade(&mut p, i, 9 + i, 10 + i);
        }
    }
    p.ensure_tracks(LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(SEC / 3),
        duration: Tick(n as i64 * SEC - SEC),
        targets: MorphTargets::Clips { ids: (0..n).map(|i| ClipId(10 + i)).collect() },
        patch: MorphPatch { audio_fx: vec![slowed()], audio_auto: vec![vec![]], ..Default::default() },
        timed: true,
        strength: 1.0,
        fade_in: Tick(0),
        fade_out: Tick(0),
        enabled: true,
    });
    p
}

/// `n` audio events, each with a gain whose curve is LINKED to its predecessor's.
fn linked(n: u64) -> Project {
    let mut p = project();
    p.ensure_tracks(LaneKind::Audio, 1);
    for i in 0..n {
        let mut c = ev(10 + i, ClipKind::Audio, 0, i as i64 * 4 * SEC, 4 * SEC, 1);
        c.fx = vec![FxInstance::gain(0.0)];
        let curve = if i == 0 {
            ParamCurve {
                param: "db".into(),
                keys: AutoKey::linear(vec![AutoKey::at(Tick(0), -6.0), AutoKey::at(Tick(4 * SEC), 0.0)]),
                source: Default::default(),
            }
        } else {
            ParamCurve {
                param: "db".into(),
                keys: Vec::new(),
                source: CurveSource::Link { path: format!("{}/fx0/db", 9 + i), expr: None, stops: Vec::new() },
            }
        };
        c.fx_auto = vec![vec![curve]];
        p.clips.push(c);
    }
    p
}

// ---- the passes ------------------------------------------------------------------------------

/// Every builder and per-build pass a project goes through on an edit and on a load. Each returns
/// the size of its OUTPUT where that may legitimately grow faster than the project: the
/// flattened clip list of `effective_clips` holds every placement's events (placements × reel
/// events), and its declared bound is linear in that list, not in the project.
type Pass = fn(&Project) -> Option<usize>;

fn passes() -> Vec<(&'static str, Pass)> {
    vec![
        ("validate", |p| {
            p.validate().expect("the scene is a valid project");
            None
        }),
        ("effective_clips", |p| Some(p.effective_clips().len())),
        ("AudioRenderer::new", |p| {
            drop(kt_engine::AudioRenderer::new(p));
            None
        }),
        ("VideoRenderer::new", |p| {
            drop(kt_engine::VideoRenderer::new(p));
            None
        }),
        ("gen_sources", |p| {
            drop(kt_engine::gen_sources(p));
            None
        }),
        ("project file round trip", |p| {
            let text = kt_model::project_file::to_text(p, None);
            drop(kt_model::project_file::from_text(&text).expect("the scene reads back"));
            None
        }),
    ]
}

/// Runs every pass on the scene at ×1, ×4, ×16 and holds each one's growth to the linear bound.
fn holds_linear(axis: &str, scene: fn(u64) -> Project, unit: u64) {
    kt_plugins_builtin::install();
    let ks = [1u64, 4, 16];
    let scenes: Vec<Project> = ks.iter().map(|k| scene(unit * k)).collect();
    for (name, pass) in passes() {
        let mut out = [None; 3];
        let w: Vec<Work> = scenes
            .iter()
            .enumerate()
            .map(|(i, p)| work(|| out[i] = pass(p)))
            .collect();
        // the growth allowed at ×4 and ×16: the scene's factor, or the output's where it is larger
        let grow = |i: usize| -> u64 {
            let by_out = match (out[0], out[i]) {
                (Some(a), Some(b)) => (b as u64).div_ceil(a.max(1) as u64),
                _ => 0,
            };
            ks[i].max(by_out)
        };
        let per = |f: &dyn Fn(&Work) -> u64| -> [u64; 3] { [f(&w[0]), f(&w[1]), f(&w[2])] };
        for (what, n) in [("elements walked", per(&|w| w.walked())), ("bytes allocated", per(&|w| w.bytes))] {
            // a pass that walks nothing at ×1 is held at the constant it has
            let base = n[0].max(64);
            assert!(
                n[1] <= 2 * grow(1) * base && n[2] <= 2 * grow(2) * base,
                "{axis}: {name} is super-linear in {what}: ×1 {}, ×4 {}, ×16 {} (rows at ×16: {})",
                n[0],
                n[1],
                n[2],
                rows(&w[2].walks)
            );
        }
    }
}

fn rows(c: &Costs) -> String {
    Cost::ALL.iter().map(|r| format!("{} {}", r.name(), c[*r as usize])).collect::<Vec<_>>().join(", ")
}

/// Test 2062: events with transitions — every transition's ends were scanned among all the events,
/// in each builder and in validation.
#[test]
fn t2062_events_and_transitions() {
    holds_linear("events", events, 24);
}

/// Test 2063: the length of a shaped speed curve that crosses zero (row 151: the address map rebuilt
/// the segment list per sampled point; the profile sampling walked the keys from the first per
/// sample).
#[test]
fn t2063_shaped_speed_curve_length() {
    holds_linear("curve", long_curve, 8);
}

/// Test 2064: placements of one reel, the reel growing with them — every placement hashed the
/// whole reel's text, twice in the audio build and four times in the video one.
#[test]
fn t2064_reel_placements() {
    holds_linear("placements", placements, 8);
}

/// Test 2065: a reel of placements of a reel — the revision hash fed the inner reel's text once per
/// placement, and every level of nesting multiplied it.
#[test]
fn t2065_nested_placements() {
    holds_linear("nested", nested, 8);
}

/// Test 2066: a cutting morpher layer over events with transitions.
#[test]
fn t2066_cutting_layer() {
    kt_plugins_builtin::install();
    // the scene's guard: the layer really cuts its targets, or the cut-point pass is not run
    let p = layered(24);
    assert!(p.effective_clips().len() > p.clips.len(), "the layer cut nothing");
    holds_linear("layer", layered, 24);
}

/// Test 2067: linked curves — the leader was looked up among the events per sample of the curve.
#[test]
fn t2067_linked_curves() {
    holds_linear("links", linked, 12);
}
