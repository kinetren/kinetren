//! t973c (plugin-api-stage1, stage 3; capability `plugins`): a moved audio kind keeps its SOUND.
//!
//! t973 holds the bytes of the file, this holds the bytes of the output: every audio kind of
//! `FxInstance::all()` renders four cases — the default, every numeric row at 70 % of its range, a
//! curve on its first Smooth row, and the default reversed (pitch also on the vocoder engine) —
//! and each output's hash is the one the enum variant gave BEFORE the kind moved into a family
//! crate. The table was printed on the pre-move tree (`KT_PRINT_GOLDEN=1`) and is never edited to
//! follow a move; a red row is a node that was not moved verbatim. Completeness lock: an audio
//! kind without a row fails.

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const DUR: i64 = 48_000;

/// (tag, [default, at 70 %, a curve, reversed]) — printed on c53131f, before any audio kind moved.
#[rustfmt::skip]
const GOLDEN: &[(&str, [u64; 4])] = &[
    ("gain", [0x07c61de143e7dafd, 0x1264fdb960d2e3fe, 0xff13344c21147aa5, 0xd39bab6d08fb5c31]),
    ("slowed_reverb", [0x87076ed5c8bf9885, 0xea8ef04081c0530f, 0xe19d6bb55d90baad, 0xa3e83ba9d9e9d689]),
    ("filter", [0x10013f41744514a7, 0x62a077ae23644263, 0x1a8c50413048113a, 0xbd1c2e0c27356e9d]),
    ("eq", [0x07c61de143e7dafd, 0xdf0e8076d5a90a4a, 0x7c12aa6b75e05986, 0xd39bab6d08fb5c31]),
    ("distortion", [0xe5f2b4acb5363898, 0x5b1b12f49fd5e866, 0x68ff3b1bdb112389, 0x2dbb7834e4344924]),
    ("bitcrusher", [0x1db1a0b8cad56842, 0xe912c8b9b65aa194, 0xfd2273d67de43e83, 0xa8dee5ee872175b6]),
    ("stereo_width", [0x07c61de143e7dafd, 0x1d9e5964097d56e2, 0xc9935cc11031cd14, 0xd39bab6d08fb5c31]),
    ("tremolo", [0x0eb2076f3003c9ad, 0x06d5a9d7fcec0a44, 0xd28236247e214cd8, 0x0814e6c5f94f5725]),
    ("auto_pan", [0xd2be090ab1d7385c, 0xc42924599f4e0fe6, 0x048b8ea2f06946ef, 0x02b99a06b58f82a7]),
    ("vibrato", [0xfe49463e1cef60a5, 0xae6466151b90747d, 0x9e815f292005b929, 0x38819855630d7f58]),
    ("chorus", [0x64539531afb6609c, 0xdefb33bf1e35f068, 0x4cbc8f5c7150a5b1, 0x522a2b51030f78ba]),
    ("flanger", [0x7378db87ef053eeb, 0xf1125c7f69a7ce19, 0x063bc67dafacf0f2, 0x4f4d7a3bb6333155]),
    ("phaser", [0x73f747452688ebd5, 0x96e28bfa776a29f2, 0x61a128fa538dd933, 0x2ef4bde9df94b683]),
    ("delay", [0x3895709a9e1897a4, 0xa7d57e6d9253c226, 0x4b86d41b06b1ca90, 0xc459679f173981d3]),
    ("compressor", [0x07c61de143e7dafd, 0x300707b5ccfcaab7, 0xd1843793a101213b, 0xd39bab6d08fb5c31]),
    ("gate", [0x0937c0c533d469b7, 0xf64207d5b805d2b5, 0x6b363e06b1b945b0, 0x601abdfd08827c46]),
    ("limiter", [0xf6fa8c3ddee2e7cf, 0x195c41372d28dc06, 0xcd8f29215801aabc, 0x8bed456f023f3987]),
    // "at 70 %" re-printed by pitch-event-edges (kkt-26.11.01 item 15, the owner's request): the
    // event ends where the material goes on, and its last grains now read that material instead
    // of silence — the sound change asked for; the other three cases did not move
    ("pitch_shift", [0x07c61de143e7dafd, 0x3c85d210d0eb2c6d, 0x2058048daf9756f7, 0xd39bab6d08fb5c31]),
];

fn project(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>, reversed: bool, engine: PitchEngine) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: engine,
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:clicks"));
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
        reversed,
        fx,
        fx_bypass: vec![],
        fx_auto: auto,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn hash(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>, reversed: bool, engine: PitchEngine) -> u64 {
    let p = project(fx, auto, reversed, engine);
    let r = AudioRenderer::new(&p);
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Clicks)));
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; DUR as usize * CH];
    let stats = r.render_frames_with(&mut f, &mut store, 0, &mut out);
    assert_eq!(stats.underruns, 0, "underrun on a fully available source");
    let bytes: Vec<u8> = out.iter().flat_map(|v| v.to_le_bytes()).collect();
    xxhash_rust::xxh3::xxh3_64(&bytes)
}

fn cases(f: &FxInstance) -> [u64; 4] {
    let engine = if f.tag() == "pitch_shift" { PitchEngine::Vocoder } else { PitchEngine::Wsola };
    let mut at70 = f.clone();
    // the number rows only: an enum variant's passport had no others (a plugin's choice and flag
    // rows — filter's mode, delay's pingpong — were fields outside it), so the table compares alike
    for spec in f.param_specs().iter().filter(|s| s.ty == ParamType::Num) {
        at70.set_param(&spec.name, spec.min + 0.7 * (spec.max - spec.min));
    }
    let curve = f.param_specs().iter().find(|s| s.kind == ParamKind::Smooth).map(|spec| {
        let to = spec.min + 0.3 * (spec.max - spec.min);
        vec![vec![ParamCurve {
            param: spec.name.to_string(),
            keys: AutoKey::linear(vec![
                AutoKey::at(Tick(0), spec.default),
                AutoKey::at(Tick::from_audio_frames(DUR), to),
            ]),
            source: Default::default(),
        }]]
    });
    [
        hash(vec![f.clone()], vec![], false, PitchEngine::Wsola),
        hash(vec![at70], vec![], false, engine),
        hash(vec![f.clone()], curve.unwrap_or_default(), false, PitchEngine::Wsola),
        hash(vec![f.clone()], vec![], true, PitchEngine::Wsola),
    ]
}

#[test]
fn t973c_a_moved_audio_kind_keeps_its_sound() {
    install();
    let audio: Vec<&FxInstance> = FxInstance::all().iter().filter(|f| f.is_audio()).collect();
    if std::env::var("KT_PRINT_GOLDEN").is_ok() {
        for f in &audio {
            let h = cases(f);
            println!("    (\"{}\", [0x{:016x}, 0x{:016x}, 0x{:016x}, 0x{:016x}]),", f.tag(), h[0], h[1], h[2], h[3]);
        }
        return;
    }
    let mut problems = Vec::new();
    for f in &audio {
        let Some((_, want)) = GOLDEN.iter().find(|(t, _)| *t == f.tag()) else {
            problems.push(format!("{}: no golden row (completeness lock)", f.tag()));
            continue;
        };
        let got = cases(f);
        for (i, name) in ["default", "at 70 %", "a curve", "reversed"].iter().enumerate() {
            if got[i] != want[i] {
                problems.push(format!("{} ({name}): 0x{:016x}, the variant gave 0x{:016x}", f.tag(), got[i], want[i]));
            }
        }
    }
    assert!(problems.is_empty(), "a moved audio kind changed its sound: {problems:#?}");
}

/// The time-warp machinery of one curved, signed Slowed+Reverb, as numbers: the source windows
/// and the mirror axis (kt-fx), the frame map and its span (kt-model `clip_warp`), the A/V pair's
/// sync (`sync_warp_to_link_group` through `SetFx`/`SetFxAuto`) and `clip_slice`'s anchor and keys.
/// Every number is read through names (`get_param`, curves), never through the slot's type, so
/// the same text measures the variant and the plugin.
fn warp_digest() -> Vec<String> {
    const SEC: i64 = 90_000;
    let slowed = FxInstance::slowed_reverb(0.7, 0.3, 80.0);
    // a signed curve: slow, backward, fast (R.57) — on the material axis from source_in
    let src_in = Tick(SEC);
    let rate = ParamCurve {
        param: "rate".into(),
        keys: AutoKey::linear(vec![
            AutoKey::at(src_in, 0.5),
            AutoKey::at(Tick(src_in.0 + 2 * SEC), -0.4),
            AutoKey::at(Tick(src_in.0 + 4 * SEC), 1.2),
        ]),
        source: Default::default(),
    };
    let mut out = Vec::new();
    for reversed in [false, true] {
        let mut p = project(vec![slowed.clone()], vec![vec![rate.clone()]], reversed, PitchEngine::Wsola);
        let c = &mut p.clips[0];
        c.source_in = src_in;
        c.duration = Tick(4 * SEC);
        let c = c.clone();
        // the frame map (kt-model) — the video side of the warp
        let w = kt_model::clip_warp(&c).expect("a warped clip has a map");
        out.push(format!("rev={reversed} map used={} span={:?} len={}", w.used(), w.span(), w.span_len()));
        for i in 0..=8 {
            let tl = c.duration.0 * i / 8;
            out.push(format!("  off({tl})={}", w.material_off(c.duration, reversed, tl)));
        }
        // the source windows and the mirror axis (kt-fx) — the audio side
        let len = c.duration.to_audio_frames();
        let auto = kt_model::AutoProfiles::from_curves_rev(&c.fx_auto, c.source_in.to_audio_frames(), len, reversed);
        out.push(format!("  used={} avg={:.12}", kt_fx::chain_source_used(&c.fx, &auto, len), kt_model::chain_avg_rate(&c.fx, &auto)));
        for (a, b) in [(0, 4800), (48_000, 60_000), (96_000, 120_000), (len - 4800, len)] {
            out.push(format!("  win({a}..{b})={:?}", kt_fx::chain_source_window_ext(&c.fx, &auto, len, 0..len, a..b)));
        }
        // clip_slice's anchor and the keys it moves (a cut at 1.5 s)
        let piece = kt_model::clip_slice(&c, kt_time::TickRange::new(Tick(c.start.0 + SEC * 3 / 2), Tick(c.start.0 + c.duration.0)), ClipId(9));
        let keys: Vec<(i64, f64)> = piece.fx_auto[0].iter().find(|cu| cu.param == "rate").unwrap().keys.iter().map(|k| (k.t.0, k.v)).collect();
        out.push(format!("  slice src_in={} keys={keys:?}", piece.source_in.0));
    }
    // the A/V pair: a warp set on the audio half reaches the video half (speed and its curve only)
    let mut p = project(vec![], vec![], false, PitchEngine::Wsola);
    let mut v = p.clips[0].clone();
    v.id = ClipId(2);
    v.kind = ClipKind::Video;
    p.clips.push(v);
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);
    m.apply(Command::LinkClips { ids: vec![ClipId(1), ClipId(2)], group: GroupId(1) }).unwrap();
    m.apply(Command::SetFx { id: ClipId(1), fx: vec![FxInstance::slowed_reverb(0.6, 0.4, 70.0)] }).unwrap();
    m.apply(Command::SetFxAuto { id: ClipId(1), auto: vec![vec![rate.clone()]] }).unwrap();
    let video = m.project().clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    let keys: Vec<(i64, f64)> = video.fx_auto.first().and_then(|cs| cs.iter().find(|cu| cu.param == "rate")).map(|cu| cu.keys.iter().map(|k| (k.t.0, k.v)).collect()).unwrap_or_default();
    out.push(format!(
        "pair: video slots={} rate={:?} mix={:?} keys={keys:?}",
        video.fx.len(),
        video.fx.first().and_then(|f| f.get_param("rate")),
        video.fx.first().and_then(|f| f.get_param("mix")),
    ));
    out
}

/// xxh3 of `warp_digest` — printed on the variant (before 3b.2) with `KT_PRINT_GOLDEN=1`.
const WARP_GOLDEN: u64 = 0x2258eee3ce5a93f0;

/// t996 (plugin-api-stage1, D11.5; capability `plugins`): a declared time warp drives the source
/// the way the variant did — the source windows, the mirror axis, the frame map, the A/V pair's
/// sync and `clip_slice`'s anchor equal the variant's numbers (`WARP_GOLDEN`, printed before the
/// move); the sound is t973c's slowed_reverb row and GOLDEN_AUDIO (t11).
#[test]
fn t996_a_declared_time_warp_drives_the_source_like_the_variant() {
    install();
    let d = warp_digest();
    let h = xxhash_rust::xxh3::xxh3_64(d.join("\n").as_bytes());
    if std::env::var("KT_PRINT_GOLDEN").is_ok() {
        println!("{}\nconst WARP_GOLDEN: u64 = 0x{h:016x};", d.join("\n"));
        return;
    }
    assert_eq!(h, WARP_GOLDEN, "the warp machinery moved:\n{}", d.join("\n"));

    // the DECLARATION is the door (D11.5): a time effect the host has never heard of — a rate row
    // and nothing else — consumes the source exactly as Slowed+Reverb at the same rate does
    let rate = ParamCurve {
        param: "rate".into(),
        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.5), AutoKey::at(Tick(180_000), -0.4)]),
        source: Default::default(),
    };
    let map = |f: FxInstance| {
        let mut p = project(vec![f], vec![vec![rate.clone()]], false, PitchEngine::Wsola);
        p.clips[0].duration = Tick(360_000);
        let c = p.clips[0].clone();
        let w = kt_model::clip_warp(&c).expect("a declared warp has a map");
        let len = c.duration.to_audio_frames();
        let auto = kt_model::AutoProfiles::from_curves_rev(&c.fx_auto, 0, len, false);
        let offs: Vec<i64> = (0..=8).map(|i| w.material_off(c.duration, false, c.duration.0 * i / 8)).collect();
        (w.used(), w.span(), offs, kt_fx::chain_source_used(&c.fx, &auto, len), kt_model::chain_avg_rate(&c.fx, &auto))
    };
    let mut echo = FxInstance::plugin("warp_echo").expect("the test plugin is installed");
    assert!(echo.set_warp_rate(0.7), "warp_echo has no rate row");
    assert_eq!(map(echo), map(FxInstance::slowed_reverb(0.7, 0.0, 60.0)), "the declared warp is not the variant's machinery");

    // ONE warp slot per chain: a second is refused by the command, named
    let mut m = TimelineModel::new(project(vec![], vec![], false, PitchEngine::Wsola));
    let two = vec![FxInstance::slowed_reverb(0.7, 0.0, 60.0), FxInstance::plugin("warp_echo").unwrap()];
    assert!(
        matches!(m.apply(Command::SetFx { id: ClipId(1), fx: two }), Err(CommandError::TwoTimeWarps(ClipId(1)))),
        "a second warp slot in one chain was accepted"
    );
}

/// The plugin set of this binary: the built-ins plus `warp_echo` (t996), installed once.
fn install() {
    kt_plugins_builtin::install_with(|reg| {
        reg.register_for_test(std::sync::Arc::new(WarpEcho::new())).expect("warp_echo registers");
    });
}

/// A test time effect: a rate row and a time-warp declaration, its audio a pass-through. The warp
/// machinery has no line naming it.
struct WarpEcho {
    m: kt_plugin_api::Manifest,
}

impl WarpEcho {
    fn new() -> WarpEcho {
        use kt_plugin_api::*;
        WarpEcho {
            m: Manifest {
                id: "warp_echo".into(),
                media: Media::AudioFx,
                passport: vec![ParamSpec::num("rate", -32.0, 32.0, 1.0, 0.01, ParamUnit::Ratio, ParamKind::Smooth)],
                rows: vec![RowDecl { store: Store::F64, omit_default: false }],
                decl: Decl { time_warp: Some(TimeWarp { rate_row: 0, frames_row: None }), ..Decl::default() },
                ui: UiDecl { name: "Warp echo", category_key: "fxreg.cat.timestretch", keywords_key: "fxreg.kw.slowedreverb", ..UiDecl::default() },
            },
        }
    }
}

impl kt_plugin_api::Plugin for WarpEcho {
    fn manifest(&self) -> &kt_plugin_api::Manifest {
        &self.m
    }
    fn audio(&self) -> Option<&dyn kt_plugin_api::AudioKernel> {
        Some(self)
    }
}

impl kt_plugin_api::AudioKernel for WarpEcho {
    fn build(&self, _opts: &kt_plugin_api::HostOpts) -> Box<dyn kt_plugin_api::AudioState> {
        Box::new(Pass)
    }
}

struct Pass;

impl kt_plugin_api::AudioState for Pass {
    fn apply(&mut self, _k: &[f64], _structural: bool) {}
    fn process(&mut self, _buf: &mut [f32], _pos: i64, _key: Option<&[f32]>) {}
    fn reset(&mut self) {}
}
