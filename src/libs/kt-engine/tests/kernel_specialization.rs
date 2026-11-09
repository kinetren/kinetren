//! The mixdown's "stands still" runs (kernel-specialization; capability `audio-render`).
//!
//! t2173 — the mixdown splits a block into runs of constant weight (one multiply per sample) and
//! ramp runs (the per-sample formula). An edit with every kind of weight — gains, crossfades with
//! handles in all four curves, a fade from nothing and to nothing, two and three overlapping events,
//! an overlaid transition, a reversed event, events with effects and a ringing tail — renders the
//! bytes it rendered before the runs existed (digests printed on the tree before them), in bursts
//! and in odd slices, on the specialized path AND inside `cpu::with_general`.

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;

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
    p.overlap_layers = true;
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
        masks: Default::default(),
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
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn reverb(mix: f32) -> FxInstance {
    FxInstance::plugin_with("slowed_reverb", &[1.0, mix as f64, 60.0, 0.4, 0.7, 1.0, 0.0, 0.0, 1.0]).unwrap()
}

fn transition(id: u64, a: Option<u64>, b: Option<u64>, i: i64, o: i64, curve: AudioCurve) -> TransitionEvent {
    TransitionEvent {
        id: TransitionId(id),
        a: a.map(ClipId),
        b: b.map(ClipId),
        in_offset: Tick::from_audio_frames(i),
        out_offset: Tick::from_audio_frames(o),
        kind: TransitionKind::Crossfade,
        audio: curve,
        auto: vec![],
        invert: false,
    }
}

/// (name, the edit). `fx` puts an effect on the events that otherwise play plain.
fn edits() -> Vec<(&'static str, Project)> {
    let mut out = Vec::new();
    for fx in [false, true] {
        let mut p = base_project();
        let curves = [AudioCurve::EqualPower, AudioCurve::Linear, AudioCurve::SCurve, AudioCurve::Exponential];
        // lane 0: four joints, one per curve, with handles; a fade from nothing at the start and to
        // nothing at the end; gains that are not 1
        let mut start = 4_000i64;
        for k in 0..5u64 {
            let mut c = clip(1 + k, 1 + k % 2, 0, start, 20_000, 3_000 + 500 * k as i64);
            c.gain_db = -2.0 * k as f32;
            if fx && k % 2 == 1 {
                c.fx = vec![reverb(0.3)];
            }
            p.clips.push(c);
            start += 20_000;
        }
        for k in 0..4u64 {
            p.transitions.push(transition(1 + k, Some(1 + k), Some(2 + k), 1_500, 2_500, curves[k as usize]));
        }
        p.transitions.push(transition(10, None, Some(1), 0, 3_000, AudioCurve::SCurve));
        p.transitions.push(transition(11, Some(5), None, 4_000, 0, AudioCurve::Linear));
        // lane 1: two overlapping (equal power), then three at one point (1/N), a reversed one
        let mut a = clip(20, 2, 1, 0, 30_000, 0);
        a.gain_db = 3.0;
        let b = clip(21, 1, 1, 18_000, 30_000, 1_000);
        let c = clip(22, 2, 1, 40_000, 30_000, 2_000);
        let mut d = clip(23, 1, 1, 44_000, 10_000, 5_000);
        d.reversed = true;
        if fx {
            a.fx = vec![reverb(0.5)];
        }
        p.clips.extend([a, b, c, d]);
        // lane 2: an overlaid transition (its curve instead of the built-in overlap)
        let e = clip(30, 1, 2, 10_000, 30_000, 0);
        let mut f = clip(31, 2, 2, 30_000, 30_000, 0);
        if fx {
            f.fx = vec![reverb(0.2)];
        }
        p.clips.extend([e, f]);
        p.transitions.push(transition(30, Some(30), Some(31), 0, 0, AudioCurve::Linear));
        p.normalize();
        p.validate().unwrap();
        out.push((if fx { "with effects" } else { "plain" }, p));
    }
    out
}

fn fetcher() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Ramp)));
    f.add_source(MediaId(2), Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 220.0 })));
    f
}

/// The whole edit rendered in slices of `step` frames from frame 0.
fn render(p: &Project, step: usize) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = fetcher();
    let total = 110_000usize;
    let mut out = vec![0f32; total * CH];
    let mut pos = 0usize;
    while pos < total {
        let n = step.min(total - pos);
        r.render_frames_with(&mut f, &mut store, pos as i64, &mut out[pos * CH..(pos + n) * CH]);
        pos += n;
    }
    out
}

fn digest(out: &[f32]) -> u64 {
    kt_fx::audio::cost::digest(out)
}

/// (edit, slice, digest) printed on the tree before the runs (d5d5b6b9).
#[rustfmt::skip]
const DIGESTS: &[(&str, usize, u64)] = &[
    ("plain", 1024, 0x372b9f52cb2328e8),
    ("plain", 777, 0x372b9f52cb2328e8),
    ("with effects", 1024, 0x7afced8703d7f06d),
    ("with effects", 777, 0x7afced8703d7f06d),
];

fn digests() -> Vec<(&'static str, usize, u64)> {
    let mut v = Vec::new();
    for (name, p) in edits() {
        for step in [1024usize, 777] {
            v.push((name, step, digest(&render(&p, step))));
        }
    }
    v
}

#[test]
fn t2173_mixdown_runs_keep_the_bytes() {
    kt_plugins_builtin::install();
    let got = digests();
    if std::env::var("KT_PRINT_DIGESTS").is_ok() {
        for (n, s, d) in &got {
            println!("    ({n:?}, {s}, 0x{d:016x}),");
        }
        return;
    }
    let general = kt_plugin_api::cpu::with_general(digests);
    for (path, got) in [("specialized", got), ("general", general)] {
        assert_eq!(got.len(), DIGESTS.len(), "a case without a digest: {got:#x?}");
        for ((n, s, d), (wn, ws, wd)) in got.iter().zip(DIGESTS) {
            assert_eq!((n, s), (wn, ws));
            assert_eq!(d, wd, "{path}: {n} in slices of {s}: 0x{d:016x}, want 0x{wd:016x}");
        }
    }
}

// ---- t2171: every audio kind, at rest and moving ------------------------------------------------

use kt_fx::audio::cost;
use kt_plugin_api::{ParamKind, ParamType};

/// Every smooth number of the kind moved to the middle between its default and an end: a person
/// turning every knob at once, which the smoothers glide through and then rest at.
fn moved(f: &FxInstance) -> Option<FxInstance> {
    let mut m = f.clone();
    let mut any = false;
    for s in f.param_specs() {
        if s.kind != ParamKind::Smooth || s.ty != ParamType::Num {
            continue;
        }
        let to = if s.max > s.default { (s.default + s.max) * 0.5 } else { (s.default + s.min) * 0.5 };
        any |= m.set_param(&s.name, to);
    }
    any.then_some(m)
}

/// The stimulus with every other quarter second silent: envelopes rise and fall through the
/// thresholds, smoothers and detectors settle in silence.
fn gapped(input: &[f32]) -> Vec<f32> {
    input.iter().enumerate().map(|(i, s)| if (i / 2 / 12_000) % 2 == 1 { 0.0 } else { *s }).collect()
}

/// (kind, scenario, digest) for every audio kind.
fn kind_digests() -> Vec<(String, &'static str, u64)> {
    kt_plugins_builtin::install();
    let input = cost::stimulus(2 * 48_000);
    let quiet: Vec<f32> = input.iter().map(|s| s * 0.01).collect();
    let gaps = gapped(&input);
    let mut v = Vec::new();
    for f in FxInstance::all().iter().filter(|f| f.is_audio()) {
        let tag = f.tag().to_string();
        let one = std::slice::from_ref(f);
        v.push((tag.clone(), "quiet", cost::digest(&cost::chain_output(one, &quiet))));
        v.push((tag.clone(), "gaps", cost::digest(&cost::chain_output(one, &gaps))));
        if let Some(m) = moved(f) {
            v.push((tag.clone(), "moved", cost::digest(&cost::switched_output(f, &m, &input))));
            v.push((tag.clone(), "back", cost::digest(&cost::switched_output(&m, f, &gaps))));
        }
        if let Some(h) = cost::heavy(f) {
            v.push((tag.clone(), "to heavy", cost::digest(&cost::switched_output(f, &h, &gaps))));
        }
        // the camera's decimating codecs (the resampler's histories run only there)
        for (codec, name) in [(1.0, "codec 1"), (2.0, "codec 2")] {
            let mut c = f.clone();
            if c.set_param("mic_codec", codec) {
                v.push((tag.clone(), name, cost::digest(&cost::chain_output(std::slice::from_ref(&c), &gaps))));
            }
        }
    }
    v
}

/// Printed at d5d5b6b9, before the "stands still" paths.
#[rustfmt::skip]
const KIND_DIGESTS: &[(&str, &str, u64)] = &[
    ("gain", "quiet", 0xd4455cd516ef2f5c),
    ("gain", "gaps", 0xab36b83e4d6f0e6b),
    ("gain", "moved", 0x0e861e2f0886dfa5),
    ("gain", "back", 0xac0f8ffd4e556a9f),
    ("slowed_reverb", "quiet", 0x2c6fa87234332faf),
    ("slowed_reverb", "gaps", 0xb928af6b5367cfd1),
    ("slowed_reverb", "moved", 0x226a688ccef69984),
    ("slowed_reverb", "back", 0x7cfafb534c07b3c0),
    ("slowed_reverb", "to heavy", 0xfbe9428d633fb3d3),
    ("filter", "quiet", 0x8be87189bd99d71f),
    ("filter", "gaps", 0x2b553a39b233a75a),
    ("filter", "moved", 0xcbb7d9f2116a43e9),
    ("filter", "back", 0x64aac5a7fccd5b2d),
    ("eq", "quiet", 0xd4455cd516ef2f5c),
    ("eq", "gaps", 0xab36b83e4d6f0e6b),
    ("eq", "moved", 0xf311c3a4d416724c),
    ("eq", "back", 0x2e090ffaeaaee758),
    ("eq", "to heavy", 0xc189368461ab9503),
    ("distortion", "quiet", 0x6acb9ba938e6092f),
    ("distortion", "gaps", 0x0cb33a0074063dd9),
    ("distortion", "moved", 0x2b18613127393fb7),
    ("distortion", "back", 0xc7780d359ad3d138),
    ("bitcrusher", "quiet", 0xe59710e85a38a525),
    ("bitcrusher", "gaps", 0x7b196c4c66cec9a5),
    ("bitcrusher", "moved", 0xd3b515c994709768),
    ("bitcrusher", "back", 0x4b59298d528a5dfb),
    ("stereo_width", "quiet", 0xd4455cd516ef2f5c),
    ("stereo_width", "gaps", 0xab36b83e4d6f0e6b),
    ("stereo_width", "moved", 0x3e281d250b6e5106),
    ("stereo_width", "back", 0x3e3e187bb7ecdc05),
    ("tremolo", "quiet", 0xb7de59b8f5c3d008),
    ("tremolo", "gaps", 0x81ea4aa23c3c16cf),
    ("tremolo", "moved", 0x0f89431e67186640),
    ("tremolo", "back", 0x20276190e51d5bf8),
    ("auto_pan", "quiet", 0x244d996742812c7b),
    ("auto_pan", "gaps", 0x90f6c31b34f72046),
    ("auto_pan", "moved", 0xc553c6938a3a2525),
    ("auto_pan", "back", 0xdbeb06e23841ed0b),
    ("vibrato", "quiet", 0xce7a6df8263967e5),
    ("vibrato", "gaps", 0xb06774b9319f9e78),
    ("vibrato", "moved", 0xe2289e7bb8567fe2),
    ("vibrato", "back", 0x198c9bf630a537bb),
    ("chorus", "quiet", 0xb87526ca4c54fe23),
    ("chorus", "gaps", 0xec4dcafb12391eea),
    ("chorus", "moved", 0x47faa897c9762684),
    ("chorus", "back", 0xd0dc7ec612a0adb9),
    ("chorus", "to heavy", 0xcfd36b836dfddda6),
    ("flanger", "quiet", 0xdbff1efce2b0db26),
    ("flanger", "gaps", 0xc3e1b99cbd1e764b),
    ("flanger", "moved", 0xf3802b29531850d4),
    ("flanger", "back", 0xad5eb5f56a9e768b),
    ("flanger", "to heavy", 0x7b4ab0f8dcc0d9cf),
    ("phaser", "quiet", 0x720af4c48788acfb),
    ("phaser", "gaps", 0x3cec02e84043ae5e),
    ("phaser", "moved", 0x47b7fa7a443e39a2),
    ("phaser", "back", 0x6ec97f8a463a176d),
    ("phaser", "to heavy", 0x0cf0426892243d53),
    ("delay", "quiet", 0xc8ab5d74995a568b),
    ("delay", "gaps", 0xcc2c4cb8bb11c007),
    ("delay", "moved", 0x7e1e4723343ceb73),
    ("delay", "back", 0x239a3322571a9904),
    ("delay", "to heavy", 0x5f3ab9eb3bc702e5),
    ("compressor", "quiet", 0xd4455cd516ef2f5c),
    ("compressor", "gaps", 0x5ec295a89d980ff4),
    ("compressor", "moved", 0x01e1b365630bf472),
    ("compressor", "back", 0xa700a1b810d8c3e3),
    ("compressor", "to heavy", 0xa0bf062303a88e34),
    ("gate", "quiet", 0x2325d58ec5438625),
    ("gate", "gaps", 0xe39c8f0d55af0aca),
    ("gate", "moved", 0xb3ae677014ace8d9),
    ("gate", "back", 0xcb37b91b9df1d259),
    ("limiter", "quiet", 0x40ee83369b7ecb24),
    ("limiter", "gaps", 0x63bed45e187343d0),
    ("limiter", "moved", 0x38bca64e0e4469b0),
    ("limiter", "back", 0x636eb26fc9d024e0),
    ("limiter", "to heavy", 0x63bed45e187343d0),
    ("pitch_shift", "quiet", 0xd4455cd516ef2f5c),
    ("pitch_shift", "gaps", 0xab36b83e4d6f0e6b),
    ("pitch_shift", "moved", 0x7b51035e9110224d),
    ("pitch_shift", "back", 0x3acc9686c372e010),
    ("pitch_shift", "to heavy", 0x3399a22a2e7db6d8),
    ("vhs_tape_sound", "quiet", 0x5859c5d98b4139ec),
    ("vhs_tape_sound", "gaps", 0x1b6c6505f14acd54),
    ("vhs_tape_sound", "moved", 0x7f65c5884ce5e3a1),
    ("vhs_tape_sound", "back", 0x4af1996707e6ee13),
    ("vhs_tape_sound", "to heavy", 0x93bae5943318c820),
    ("datamosh_sound", "quiet", 0xa57616f9e08be9cd),
    ("datamosh_sound", "gaps", 0x5a1780c265e26994),
    ("datamosh_sound", "moved", 0xd50ba03eddbccd1e),
    ("datamosh_sound", "back", 0xd8a326e98d4c96a8),
    ("datamosh_sound", "to heavy", 0x64da8ef63ed21bb5),
    ("camera_isp_sound", "quiet", 0xe78b5f37e4f75f14),
    ("camera_isp_sound", "gaps", 0x74823e67dfbc441c),
    ("camera_isp_sound", "moved", 0x5d79d0bfd854248d),
    ("camera_isp_sound", "back", 0x57ee8e0c3d18cef6),
    ("camera_isp_sound", "to heavy", 0x0ddefb06bffa4567),
    ("camera_isp_sound", "codec 1", 0x86dec5487bff7e4e),
    ("camera_isp_sound", "codec 2", 0x44e69acd2f90c751),
];

#[test]
fn t2171_every_kind_keeps_its_bytes_at_rest_and_moving() {
    let got = kind_digests();
    if std::env::var("KT_PRINT_DIGESTS").is_ok() {
        for (k, s, d) in &got {
            println!("    ({k:?}, {s:?}, 0x{d:016x}),");
        }
        return;
    }
    let check = |got: &[(String, &str, u64)], path: &str| {
        let mut bad = Vec::new();
        for (k, s, d) in got {
            match KIND_DIGESTS.iter().find(|(wk, ws, _)| wk == k && ws == s) {
                Some((_, _, w)) if w == d => {}
                w => bad.push(format!("{path}: {k} {s}: 0x{d:016x}, want {:x?}", w.map(|r| r.2))),
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    };
    check(&got, "specialized");
    check(&kt_plugin_api::cpu::with_general(kind_digests), "general");
    check(&kt_plugin_api::cpu::with_baseline(kind_digests), "baseline");
}
