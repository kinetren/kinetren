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
    ("pitch_shift", [0x07c61de143e7dafd, 0x6a8e26e7182e4946, 0x2058048daf9756f7, 0xd39bab6d08fb5c31]),
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
    for spec in f.param_specs() {
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
    kt_plugins_builtin::install();
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
