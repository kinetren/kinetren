//! fx-graph-dag D4 (capability `audio-fx`): an event's sound chain runs its graph in the engine — a
//! Slowed in an arm reads the source on its own window, a dangling node does not ring, the tail is
//! the Output path's, a rewire rebuilds the processor and a value edit keeps it.

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::fx_graph::{FxIn, MergeMode, SlotInputs};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const DUR: i64 = 48_000;

fn slowed(rate: f64) -> FxInstance {
    FxInstance::plugin_with("slowed_reverb", &[rate, 0.3, 60.0, 0.4, 0.7, 1.0, 0.0, 0.0, 1.0]).unwrap()
}

fn delay(ms: f64, feedback: f64) -> FxInstance {
    FxInstance::plugin_with("delay", &[ms, feedback, 0.5]).unwrap()
}

fn sum() -> FxInstance {
    FxInstance::Merge { mode: MergeMode::Sum, mix: 1.0 }
}

fn wire(main: FxIn, second: Option<FxIn>) -> SlotInputs {
    SlotInputs { main, second, matte: None }
}

fn project(fx: Vec<FxInstance>, fx_in: Vec<SlotInputs>) -> Project {
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
    p.media.push(MediaRef::new(MediaId(1), "synth:sine"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick::from_audio_frames(DUR),
        source_in: Tick::from_audio_frames(4_800),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
        fx_in,
    });
    p.validate().unwrap();
    p
}

fn fetcher() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 330.0 })));
    f
}

/// The whole event and its tail, in bursts.
fn render(p: &Project, frames: i64) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut f = fetcher();
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; frames as usize * CH];
    for (n, chunk) in out.chunks_mut(1024 * CH).enumerate() {
        let stats = r.render_frames_with(&mut f, &mut store, n as i64 * 1024, chunk);
        assert_eq!(stats.underruns, 0, "the source is all there");
    }
    out
}

fn added(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b).map(|(x, y)| x + y).collect()
}

/// Test t3229 (fx-graph-dag D4): Slowed at 0.5× in a sound arm summed with the dry source — the arm
/// equals the event whose chain is the arm's line, sample for sample, and the dry side is the line
/// without effects; the prefetch asks the arm's own window too; two runs give the same bytes.
#[test]
fn t3229_slowed_in_a_sound_arm_reads_its_own_window() {
    kt_plugins_builtin::install();
    let frames = DUR + 48_000 * 2;
    let dry = render(&project(vec![], vec![]), frames);
    let arm = render(&project(vec![slowed(0.5)], vec![]), frames);
    let fx = vec![slowed(0.5), sum()];
    let w = vec![SlotInputs::default(), wire(FxIn::Source, Some(FxIn::Slot(0)))];
    let p = project(fx, w);
    let got = render(&p, frames);
    assert!(got == added(&dry, &arm), "the graph is not the dry line plus the arm's line");
    assert!(got == render(&p, frames), "two runs differ");
    // the arm's window is warmed: what the arm alone asks is asked by the graph
    let want = |p: &Project| AudioRenderer::new(p).wanted_audio_bursts(0..frames);
    let has = want(&p);
    for (m, b) in want(&project(vec![slowed(0.5)], vec![])) {
        for i in b {
            assert!(has.iter().any(|(hm, hb)| *hm == m && hb.contains(&i)), "burst {i} of the arm's window is not asked");
        }
    }
}

/// Test t3216 (fx-graph-dag D4): a delay in an arm and a long dangling delay — the event is the dry
/// source plus the arm alone (the dangling node does not ring), and its tail is the arm's.
#[test]
fn t3216_tails_follow_the_output_path() {
    kt_plugins_builtin::install();
    let frames = DUR + 48_000 * 3;
    let dry = render(&project(vec![], vec![]), frames);
    let arm = render(&project(vec![delay(200.0, 0.5)], vec![]), frames);
    // 0 delay (arm) ; 1 long delay (dangling, reads the Source) ; 2 Sum(Source, 0)
    let fx = vec![delay(200.0, 0.5), delay(1200.0, 0.9), sum()];
    let w = vec![SlotInputs::default(), wire(FxIn::Source, None), wire(FxIn::Source, Some(FxIn::Slot(0)))];
    let split = kt_fx::sound_split(&fx, &w);
    let none = kt_fx::AutoProfiles::default();
    assert_eq!(
        kt_fx::chain_tail(&split.main, &none),
        kt_fx::chain_tail(&[delay(200.0, 0.5)], &none),
        "the tail is not the Output path's"
    );
    let got = render(&project(fx, w), frames);
    assert!(got == added(&dry, &arm), "the dangling delay rang, or the arm is not itself");
}

/// Test t3217 (fx-graph-dag D4): a value edit on a graph keeps the processor (its state goes on), a
/// rewire is a structural change and builds a new one, as a reorder does.
#[test]
fn t3217_a_rewire_rebuilds_and_a_value_edit_keeps_the_state() {
    kt_plugins_builtin::install();
    let fx = vec![delay(200.0, 0.5), sum()];
    let w = vec![SlotInputs::default(), wire(FxIn::Source, Some(FxIn::Slot(0)))];
    let p = project(fx.clone(), w.clone());
    let r = AudioRenderer::new(&p);
    let mut f = fetcher();
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut buf = vec![0f32; 4096 * CH];
    r.render_frames_with(&mut f, &mut store, 0, &mut buf);
    let warm = |store: &mut FxStateStore| store.get_mut(ClipId(1)).and_then(|p| p.next_local());
    assert_eq!(warm(&mut store), Some(4096), "scene: the processor is warm");
    // a value: the delay's time
    let mut edited = fx.clone();
    edited[0] = delay(250.0, 0.5);
    let r2 = AudioRenderer::new(&project(edited.clone(), w.clone()));
    r2.sync_fx_since(Some(&r), &mut store);
    assert_eq!(warm(&mut store), Some(4096), "a value edit dropped the state");
    // a rewire: the Background and the Foreground swapped
    let swapped = vec![SlotInputs::default(), wire(FxIn::Prev, Some(FxIn::Source))];
    let r3 = AudioRenderer::new(&project(edited, swapped));
    r3.sync_fx_since(Some(&r2), &mut store);
    assert_eq!(warm(&mut store), None, "a rewire kept the old processor");
}

/// Test t3212 (fx-graph-dag D4): a line keeps its samples, and the graph pass computes the line's
/// samples exactly — a chain run through the graph path (a Merge at mix 0 over its own Output)
/// equals the same chain as a line, byte for byte. GOLDEN_AUDIO holds the line itself.
#[test]
fn t3212_the_graph_pass_computes_the_line() {
    kt_plugins_builtin::install();
    let frames = DUR + 48_000;
    let gain = FxInstance::plugin_with("gain", &[-3.0, 0.2]).unwrap();
    let line = render(&project(vec![delay(150.0, 0.4), gain.clone()], vec![]), frames);
    let zero = FxInstance::Merge { mode: MergeMode::Sum, mix: 0.0 };
    let w = vec![SlotInputs::default(), SlotInputs::default(), wire(FxIn::Prev, Some(FxIn::Source))];
    let graph = render(&project(vec![delay(150.0, 0.4), gain, zero], w), frames);
    assert!(graph == line, "the graph pass moved the line's samples");
}
