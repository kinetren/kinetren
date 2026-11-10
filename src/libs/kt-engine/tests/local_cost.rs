//! The local costs of the audio kernels (local-cost-queue; capability `audio-fx`).
//!
//! t2315 — the forms t1981 does not reach keep their bytes: the limiter with a moving threshold and
//! ceiling, Pitch (WSOLA) with a curved shift, VHS Tape's sound with curves on the linear track's
//! head, its bias, its dropouts and the tracking, the camera microphone with each codec and curves on
//! its rows, Datamosh's sound predicting with losses. Each renders 4 s of an event through the real
//! renderer (curves are the event's automation), as digests printed on the tree BEFORE
//! local-cost-queue (`KT_PRINT_DIGESTS=1`), on the CPU's path and on the baseline path.

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const DUR: i64 = 48_000 * 4;

fn project(fx: FxInstance, auto: Vec<ParamCurve>, pattern: &str) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 25,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: PitchEngine::Wsola,
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
        masks: Default::default(),
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
        fx: vec![fx],
        fx_bypass: vec![],
        fx_auto: vec![auto],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().expect("a valid case");
    p
}

fn render(fx: FxInstance, auto: Vec<ParamCurve>) -> Vec<f32> {
    let pattern = AudioPattern::Sine { hz: 330.0 };
    let p = project(fx, auto, "synth:sine");
    let r = AudioRenderer::new(&p);
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), pattern)));
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; DUR as usize * CH];
    // in odd slices: a chunk boundary of the curves is not a slice boundary
    let mut pos = 0usize;
    while pos < DUR as usize {
        let n = 1500.min(DUR as usize - pos);
        r.render_frames_with(&mut f, &mut store, pos as i64, &mut out[pos * CH..(pos + n) * CH]);
        pos += n;
    }
    out
}

/// A kind at its defaults with `set` applied.
fn kind(tag: &str, set: &[(&str, f64)]) -> FxInstance {
    let mut f = FxInstance::all().iter().find(|f| f.tag() == tag).expect("a built-in kind").clone();
    for (k, v) in set {
        assert!(f.set_param(k, *v), "{tag} has no row {k}");
    }
    f
}

/// A curve over the whole event from `a` to `b`, with a key in the middle at `m`.
fn curve(param: &str, a: f64, m: f64, b: f64) -> ParamCurve {
    ParamCurve {
        param: param.to_string(),
        keys: vec![
            AutoKey::at(Tick(0), a),
            AutoKey::at(Tick::from_audio_frames(DUR / 2), m),
            AutoKey::at(Tick::from_audio_frames(DUR), b),
        ],
        source: Default::default(),
    }
}

fn cases() -> Vec<(&'static str, FxInstance, Vec<ParamCurve>)> {
    let vhs_lin = [("audio_out", 2.0), ("dropouts_per_s", 40.0)];
    vec![
        ("limiter curved", kind("limiter", &[]), vec![curve("threshold_db", -30.0, -6.0, -18.0), curve("ceiling_db", -12.0, -0.3, -3.0)]),
        ("limiter still", kind("limiter", &[("threshold_db", -20.0), ("ceiling_db", -6.0)]), vec![]),
        ("pitch wsola", kind("pitch_shift", &[("semitones", 5.0)]), vec![]),
        ("pitch wsola curved", kind("pitch_shift", &[]), vec![curve("semitones", -7.0, 3.0, 12.0)]),
        ("vhs linear", kind("vhs_tape_sound", &vhs_lin), vec![]),
        (
            "vhs head curved",
            kind("vhs_tape_sound", &vhs_lin),
            vec![curve("ac_gap", 0.6, 3.0, 1.2), curve("ac_spacing", 0.0, 1.5, 0.3), curve("bias", 0.7, 1.3, 1.0)],
        ),
        ("vhs dropouts curved", kind("vhs_tape_sound", &vhs_lin), vec![curve("dropouts_per_s", 0.5, 300.0, 20.0), curve("dropout_us", 2.0, 60.0, 10.0)]),
        ("vhs tracking curved", kind("vhs_tape_sound", &[("audio_out", 2.0)]), vec![curve("tracking_um", -40.0, 20.0, 0.0), curve("jitter_us", 0.0, 1.5, 0.2)]),
        ("vhs ep stereo", kind("vhs_tape_sound", &[("audio_out", 1.0), ("speed", 2.0), ("linear_tracks", 1.0), ("dropout_us", 60.0)]), vec![]),
        ("mic aac", kind("camera_isp_sound", &[("mic_codec", 0.0)]), vec![]),
        ("mic linear 16k", kind("camera_isp_sound", &[("mic_codec", 1.0)]), vec![]),
        ("mic mulaw", kind("camera_isp_sound", &[("mic_codec", 2.0)]), vec![]),
        ("mic mulaw stereo", kind("camera_isp_sound", &[("mic_codec", 2.0), ("mic_channels", 1.0), ("mic_scene_spl", 110.0)]), vec![]),
        (
            "mic curved",
            kind("camera_isp_sound", &[]),
            vec![curve("mic_scene_spl", 70.0, 115.0, 90.0), curve("mic_wind", 0.0, 1.0, 0.3), curve("mic_hpf", 20.0, 300.0, 80.0), curve("mic_ns_strength", 0.0, 1.0, 0.5)],
        ),
        ("datamosh predicting", kind("datamosh_sound", &[("audio_predict", 1.0), ("loss", 0.3)]), vec![]),
        ("datamosh plain", kind("datamosh_sound", &[]), vec![]),
    ]
}

/// (case, digest) printed on the tree before local-cost-queue (87f6f38d).
#[rustfmt::skip]
const DIGESTS: &[(&str, u64)] = &[
    ("limiter curved", 0x1f3d47ce297b3c01),
    ("limiter still", 0x4632173eddefeda5),
    ("pitch wsola", 0xee8ce7bb7a674939),
    ("pitch wsola curved", 0x3fe6760d52e7794d),
    ("vhs linear", 0xca68073caebeba19),
    ("vhs head curved", 0xd5c2838828a63071),
    ("vhs dropouts curved", 0x0a784900a6d0d49d),
    ("vhs tracking curved", 0xc0b55f520ee6021c),
    ("vhs ep stereo", 0xb8262b85e20a45b9),
    ("mic aac", 0xf0dff3d01e8d21d5),
    ("mic linear 16k", 0xcba0760ddc1577fe),
    ("mic mulaw", 0x8a26efff4b4ad0e8),
    ("mic mulaw stereo", 0xa0044c5753bd8f33),
    ("mic curved", 0x5e918a7afdc9c902),
    ("datamosh predicting", 0x54302d4a0ab72bbc),
    ("datamosh plain", 0x09665277f9a88d3d),
];

fn digests() -> Vec<String> {
    let print = std::env::var("KT_PRINT_DIGESTS").is_ok();
    let mut bad = Vec::new();
    for (name, fx, auto) in cases() {
        let got = kt_fx::audio::cost::digest(&render(fx, auto));
        if print {
            println!("    (\"{name}\", 0x{got:016x}),");
        }
        match DIGESTS.iter().find(|(n, _)| *n == name) {
            Some((_, want)) if *want == got => {}
            Some((_, want)) => bad.push(format!("{name}: 0x{got:016x}, want 0x{want:016x}")),
            None => bad.push(format!("{name}: no row (KT_PRINT_DIGESTS=1 prints it)")),
        }
    }
    bad
}

#[test]
fn t2315_the_curved_and_switched_forms_keep_their_bytes() {
    kt_plugins_builtin::install();
    let fast = digests();
    assert!(fast.is_empty(), "the CPU's path moved bytes: {fast:#?}");
    let base = kt_plugin_api::cpu::with_baseline(digests);
    assert!(base.is_empty(), "the baseline path moved bytes: {base:#?}");
}

/// Not a check: with `KT_PRINT_COST=1` (on a release build) each case's render of 4 s against the
/// reference kernel on the same frames, minima of nine rounds, alternately — the before/after figures
/// of local-cost-queue's design.
#[test]
fn local_cost_print() {
    if std::env::var("KT_PRINT_COST").is_err() {
        return;
    }
    kt_plugins_builtin::install();
    let input = kt_fx::audio::cost::stimulus(DUR as usize);
    for (name, fx, auto) in cases() {
        let (mut best_k, mut best_r) = (f64::INFINITY, f64::INFINITY);
        for _ in 0..9 {
            let mut buf = input.clone();
            let t = std::time::Instant::now();
            kt_fx::audio::cost::Reference::default().process(&mut buf);
            best_r = best_r.min(t.elapsed().as_secs_f64());
            std::hint::black_box(&buf);
            let t = std::time::Instant::now();
            std::hint::black_box(render(fx.clone(), auto.clone()));
            best_k = best_k.min(t.elapsed().as_secs_f64());
        }
        println!("{name:<22} {:>7.2} ref  ({:.3}% of real time)", best_k / best_r, best_k / 4.0 * 100.0);
    }
}
