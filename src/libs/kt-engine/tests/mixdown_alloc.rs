//! t2684 (per-sample-costs, found-defects 350–352; capability `audio-render`): once warm, the audio
//! render of a burst allocates nothing — the warp under a moving speed, a layer strength's dry
//! side, a track stage with a sidechain key, a nested reel's sub-mix, plain events and a loop
//! region. Before the change every burst allocated the warp's chunk buffers, the strength's dry
//! copy, an event's chain output, a stage's sum and key buffers, the active-event lists, the reel
//! maps and the sub-mix's burst, and every loop wrap the warp map's segments.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::LaneKind::Audio as A;
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

struct Counting;

thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
    static PAUSED: Cell<bool> = const { Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if !PAUSED.try_with(|p| p.get()).unwrap_or(true) {
            let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if !PAUSED.try_with(|p| p.get()).unwrap_or(true) {
            let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        }
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static G: Counting = Counting;

fn allocs() -> u64 {
    ALLOCS.with(|c| c.get())
}

const CH: usize = AUDIO_CHANNELS;
const D: i64 = AUDIO_BURST_FRAMES as i64;

/// The source bursts made once and handed out again: the player's cache, so the count is the
/// render's own.
struct Cached {
    inner: DirectFetcher,
    kept: HashMap<(MediaId, i64), Arc<kt_media::AudioBurst>>,
}

impl AudioFetcher for Cached {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<kt_media::AudioBurst>> {
        if let Some(b) = self.kept.get(&(media, idx)) {
            return Some(b.clone());
        }
        // a source read is the decoder's, not the render's: not counted
        PAUSED.with(|p| p.set(true));
        let b = self.inner.audio_burst(media, idx);
        if let Some(b) = &b {
            self.kept.insert((media, idx), b.clone());
        }
        PAUSED.with(|p| p.set(false));
        b
    }
}

fn fetcher() -> Cached {
    let mut f = DirectFetcher::new(None);
    let pats = [(1, AudioPattern::Sine { hz: 330.0 }), (2, AudioPattern::Dc { value: 0.9 }), (3, AudioPattern::Ramp)];
    for (id, pat) in pats {
        f.add_source(MediaId(id), Box::new(SyntheticSource::new(Tick(90_000 * 600), pat)));
    }
    Cached { inner: f, kept: HashMap::with_capacity(4096) }
}

fn event(id: u64, lane: u32, start: i64, len: i64, media: u64, fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>) -> ClipEvent {
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
        kind: ClipKind::Audio,
        lane,
        start: Tick::from_audio_frames(start),
        duration: Tick::from_audio_frames(len),
        source_in: Tick::from_audio_frames(4_800),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: auto,
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
    }
}

fn fx(id: &str, set: &[(&str, f64)]) -> FxInstance {
    let mut f = FxInstance::all().iter().find(|f| f.tag() == id).expect("a built-in kind").clone();
    for (k, v) in set {
        assert!(f.set_param(k, *v), "{id} has no row {k}");
    }
    f
}

fn curve(param: &str, len: i64, a: f64, b: f64) -> ParamCurve {
    ParamCurve {
        param: param.to_string(),
        keys: vec![AutoKey::at(Tick::from_audio_frames(4_800), a), AutoKey::at(Tick::from_audio_frames(4_800 + len), b)],
        source: Default::default(),
    }
}

/// A project with each path of the mixdown: a warped event under a moving speed with a strength,
/// a plain event, a track stage keyed by another track, a nested reel.
fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: PitchEngine::Wsola,
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    for id in [1u64, 2, 3] {
        p.media.push(MediaRef::new(MediaId(id), format!("synth:{id}")));
    }
    let len = 48_000 * 20;
    let mut mix = FxInstance::LayerMix { amount: 0.4, from: 1 };
    mix.set_param("amount", 0.4);
    p.clips.push(event(
        1,
        0,
        0,
        len,
        1,
        vec![fx("slowed_reverb", &[("rate", 0.7), ("mix", 0.3)]), fx("phaser", &[]), mix],
        vec![vec![curve("rate", len, 0.6, 0.9)], vec![], vec![curve("amount", len, 0.2, 0.8)]],
    ));
    p.clips.push(event(2, 1, 0, len, 3, vec![], vec![]));
    p.clips.push(event(3, 2, 0, len, 2, vec![fx("eq", &[])], vec![]));
    let mut s = Sequence::new(MediaId(100), "Piece");
    s.clips.push(event(20, 0, 0, len, 1, vec![fx("delay", &[])], vec![]));
    p.sequences.push(s);
    let mut inst = event(10, 3, 0, len, 100, vec![fx("tremolo", &[])], vec![]);
    inst.source_in = Tick(0);
    p.clips.push(inst);
    p.ensure_tracks(A, 4);
    let key = p.track_at(A, 2).unwrap().id;
    let mut duck = FxInstance::plugin_with("compressor", &[-30.0, 12.0, 3.0, 10.0, 0.0, 0.0]).unwrap();
    duck.set_sidechain_key(Some(key));
    p.track_at_mut(A, 1).unwrap().fx = vec![fx("limiter", &[]), duck];
    p.validate().expect("a valid project");
    p
}

/// The counted play: `(linear, looped)` allocations of four warm seconds each.
fn play(r: &AudioRenderer, f: &mut Cached) -> (u64, u64) {
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut buf = vec![0f32; D as usize * CH];
    // linear play: nine seconds to warm (rings, pools, a reel's sub-mix ring, which holds up to
    // its block and pre-roll — 256 + bursts), then four counted
    let mut at = 0i64;
    for _ in 0..420 {
        r.render_frames_with(f, &mut store, at, &mut buf);
        at += D;
    }
    let before = allocs();
    for _ in 0..188 {
        r.render_frames_with(f, &mut store, at, &mut buf);
        at += D;
    }
    let linear = allocs() - before;
    // a loop region of 1.5 s over the same events: two laps to warm, four counted
    let region = 48_000 * 8..48_000 * 8 + 72_000;
    let mut pos = region.start;
    for _ in 0..141 {
        r.render_looped_with(f, &mut store, region.clone(), pos, &mut buf);
        pos += D;
    }
    let before = allocs();
    for _ in 0..282 {
        r.render_looped_with(f, &mut store, region.clone(), pos, &mut buf);
        pos += D;
    }
    (linear, allocs() - before)
}

#[test]
fn t2684_a_warm_render_allocates_nothing() {
    kt_plugins_builtin::install();
    let p = project();
    let r = AudioRenderer::new(&p);
    let mut f = fetcher();
    // the first play fills the source cache, the second is counted from fresh states
    play(&r, &mut f);
    let got = play(&r, &mut f);
    assert_eq!(got, (0, 0), "allocations in 4 s of warm bursts: linear, looped");
}

/// Test t3292 (fx-graph-dag D4, capability `audio-fx`): a warm render of sound GRAPHS allocates nothing —
/// an event with a Slowed arm (a sound read), a look-ahead arm aligned before its Merge and a compressor
/// keyed by an arm, and a track stage with a branch; linear and looped.
#[test]
fn t3292_a_warm_graph_allocates_nothing() {
    use kt_model::fx_graph::{FxIn, MergeMode, SlotInputs};
    kt_plugins_builtin::install();
    let wire = |main: FxIn, second: Option<FxIn>| SlotInputs { main, second, matte: None };
    let sum = || FxInstance::Merge { mode: MergeMode::Sum, mix: 0.5 };
    let mut p = project();
    // 0 slowed (arm, a read) ; 1 limiter (arm) ; 2 compressor of the Source keyed by 1 ; 3 2+0 ; 4 3+1
    let ev = p.clips.iter_mut().find(|c| c.id == ClipId(3)).unwrap();
    ev.fx = vec![
        fx("slowed_reverb", &[("rate", 0.7), ("mix", 0.3)]),
        fx("limiter", &[]),
        FxInstance::plugin_with("compressor", &[-30.0, 12.0, 3.0, 10.0, 0.0, 0.0]).unwrap(),
        sum(),
        sum(),
    ];
    ev.fx_in = vec![
        SlotInputs::default(),
        wire(FxIn::Source, None),
        wire(FxIn::Source, Some(FxIn::Slot(1))),
        wire(FxIn::Prev, Some(FxIn::Slot(0))),
        wire(FxIn::Prev, Some(FxIn::Slot(1))),
    ];
    let track = p.track_at_mut(A, 0).unwrap();
    track.fx = vec![fx("limiter", &[]), sum()];
    track.fx_in = vec![SlotInputs::default(), wire(FxIn::Prev, Some(FxIn::Source))];
    p.validate().expect("a valid graph");
    let r = AudioRenderer::new(&p);
    let mut f = fetcher();
    play(&r, &mut f);
    let got = play(&r, &mut f);
    assert_eq!(got, (0, 0), "allocations in 4 s of warm bursts of graphs: linear, looped");
}
