//! The costs in the audio render's inner loops (per-sample-costs; capability `audio-fx`).
//!
//! t2680 — the paths t1981, t2171 and t2315 do not reach keep their bytes: the warp under a moving
//! speed (both resampler modes), a layer strength's dry side, the tail twin and the earlier loop laps
//! of a pitched event, a track stage with a sidechain key, a nested reel's sub-mix, Pitch's vocoder,
//! Phaser, Chorus and Flanger with a curved rate, the EQ and the filter with curved bands, Freeverb
//! and the delay with moving times. Each renders through the real renderer in odd slices, as
//! digests printed on the tree BEFORE per-sample-costs (`KT_PRINT_DIGESTS=1`), on the CPU's path,
//! on the baseline path and on the general path.

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::LaneKind::Audio as A;
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const DUR: i64 = 48_000 * 3;

fn settings(engine: PitchEngine) -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: engine,
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
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

fn base(engine: PitchEngine) -> Project {
    let mut p = Project::new(settings(engine));
    for id in [1u64, 2, 3] {
        p.media.push(MediaRef::new(MediaId(id), format!("synth:{id}")));
    }
    p
}

fn fetcher() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    let pats = [(1, AudioPattern::Sine { hz: 330.0 }), (2, AudioPattern::Dc { value: 0.9 }), (3, AudioPattern::Ramp)];
    for (id, pat) in pats {
        f.add_source(MediaId(id), Box::new(SyntheticSource::new(Tick(90_000 * 600), pat)));
    }
    f
}

fn fx(id: &str, set: &[(&str, f64)]) -> FxInstance {
    kt_plugins_builtin::install();
    let mut f = FxInstance::all().iter().find(|f| f.tag() == id).expect("a built-in kind").clone();
    for (k, v) in set {
        assert!(f.set_param(k, *v), "{id} has no row {k}");
    }
    f
}

/// A curve over `len` frames from `a` to `b`, with a key in the middle at `m`.
fn curve(param: &str, len: i64, a: f64, m: f64, b: f64) -> ParamCurve {
    ParamCurve {
        param: param.to_string(),
        keys: vec![
            AutoKey::at(Tick::from_audio_frames(4_800), a),
            AutoKey::at(Tick::from_audio_frames(4_800 + len / 2), m),
            AutoKey::at(Tick::from_audio_frames(4_800 + len), b),
        ],
        source: Default::default(),
    }
}

/// One event on lane 0 with `chain` and its curves.
fn one(engine: PitchEngine, chain: Vec<(FxInstance, Vec<ParamCurve>)>) -> Project {
    let mut p = base(engine);
    let (fx, auto): (Vec<_>, Vec<_>) = chain.into_iter().unzip();
    p.clips.push(event(1, 0, 2_000, DUR - 4_000, 1, fx, auto));
    p.validate().expect("a valid case");
    p
}

fn render(p: &Project, from: i64, frames: i64) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut f = fetcher();
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; frames as usize * CH];
    let mut pos = 0i64;
    while pos < frames {
        let n = 1_500.min(frames - pos);
        r.render_frames_with(&mut f, &mut store, from + pos, &mut out[pos as usize * CH..(pos + n) as usize * CH]);
        pos += n;
    }
    out
}

fn looped(p: &Project, region: std::ops::Range<i64>, frames: i64) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut f = fetcher();
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; frames as usize * CH];
    let mut pos = 0i64;
    while pos < frames {
        let n = 1_024.min(frames - pos);
        let at = region.start + pos;
        r.render_looped_with(&mut f, &mut store, region.clone(), at, &mut out[pos as usize * CH..(pos + n) as usize * CH]);
        pos += n;
    }
    out
}

fn cases() -> Vec<(&'static str, Box<dyn Fn() -> Vec<f32>>)> {
    use PitchEngine::{Vocoder, Wsola};
    let len = DUR - 4_000;
    let slowed = |rate: f64| fx("slowed_reverb", &[("rate", rate), ("mix", 0.3)]);
    let mut v: Vec<(&'static str, Box<dyn Fn() -> Vec<f32>>)> = Vec::new();
    v.push((
        "warp curved",
        Box::new(move || render(&one(Wsola, vec![(slowed(0.7), vec![curve("rate", len, 0.5, 0.9, 0.6)])]), 0, DUR)),
    ));
    v.push((
        "warp extended",
        Box::new(move || render(&one(Wsola, vec![(slowed(0.7), vec![curve("rate", len, 0.15, 1.4, 0.4)])]), 0, DUR)),
    ));
    v.push((
        "strength",
        Box::new(move || {
            let mut mix = FxInstance::LayerMix { amount: 0.4, from: 1 };
            mix.set_param("amount", 0.4);
            let chain = vec![
                (fx("delay", &[]), vec![]),
                (fx("phaser", &[]), vec![]),
                (mix, vec![curve("amount", len, 0.1, 0.9, 0.3)]),
            ];
            render(&one(Wsola, chain), 0, DUR)
        }),
    ));
    for (name, engine) in [("twin and laps wsola", Wsola), ("twin and laps vocoder", Vocoder)] {
        v.push((
            name,
            Box::new(move || {
                let mut pitch = fx("pitch_shift", &[("semitones", 7.0)]);
                assert!(pitch.set_param("echo_before", 1.0));
                let mut p = base(engine);
                p.clips.push(event(1, 0, 9_600, 24_000, 1, vec![fx("delay", &[("feedback", 0.6)]), pitch], vec![]));
                p.validate().unwrap();
                let mut out = render(&p, 0, 48_000);
                out.extend(looped(&p, 9_600..9_600 + 26_400, 26_400 * 5));
                out
            }),
        ));
    }
    v.push((
        "vocoder curved",
        Box::new(move || render(&one(Vocoder, vec![(fx("pitch_shift", &[]), vec![curve("semitones", len, -7.0, 3.0, 12.0)])]), 0, DUR)),
    ));
    v.push((
        "track key",
        Box::new(move || {
            let mut p = base(Wsola);
            p.clips.push(event(1, 0, 0, DUR, 3, vec![fx("chorus", &[])], vec![]));
            p.clips.push(event(2, 1, 9_600, 24_000, 2, vec![], vec![]));
            p.clips.push(event(3, 1, 72_000, 24_000, 2, vec![fx("eq", &[])], vec![]));
            p.ensure_tracks(A, 2);
            let speech = p.track_at(A, 1).unwrap().id;
            let mut duck = FxInstance::plugin_with("compressor", &[-30.0, 12.0, 3.0, 10.0, 0.0, 0.0]).unwrap();
            duck.set_sidechain_key(Some(speech));
            p.track_at_mut(A, 0).unwrap().fx = vec![fx("limiter", &[]), duck];
            p.validate().unwrap();
            render(&p, 0, DUR)
        }),
    ));
    v.push((
        "nested reel",
        Box::new(move || {
            let mut p = base(Wsola);
            let mut s = Sequence::new(MediaId(100), "Piece");
            s.clips.push(event(2, 0, 0, 24_000, 1, vec![fx("slowed_reverb", &[])], vec![]));
            s.clips.push(event(3, 0, 24_000, 24_000, 3, vec![fx("delay", &[])], vec![]));
            p.sequences.push(s);
            p.clips.push(event(1, 0, 0, 24_000, 3, vec![], vec![]));
            let mut inst = event(10, 0, 24_000, 48_000, 100, vec![fx("phaser", &[])], vec![]);
            inst.source_in = Tick(0);
            p.clips.push(inst);
            p.validate().unwrap();
            render(&p, 0, DUR)
        }),
    ));
    for (name, tag, row, a, m, b) in [
        ("phaser rate curved", "phaser", "rate_hz", 0.05, 4.0, 1.0),
        ("phaser depth curved", "phaser", "depth", 0.0, 1.0, 0.3),
        ("chorus rate curved", "chorus", "rate_hz", 0.1, 5.0, 1.0),
        ("flanger rate curved", "flanger", "rate_hz", 0.05, 2.0, 0.5),
        ("tremolo rate curved", "tremolo", "rate_hz", 0.1, 20.0, 3.0),
        ("filter curved", "filter", "cutoff_hz", 80.0, 8_000.0, 500.0),
        ("delay time curved", "delay", "time_ms", 20.0, 600.0, 150.0),
        ("compressor curved", "compressor", "threshold_db", -50.0, -5.0, -20.0),
    ] {
        v.push((name, Box::new(move || render(&one(Wsola, vec![(fx(tag, &[]), vec![curve(row, len, a, m, b)])]), 0, DUR))));
    }
    v.push((
        "eq curved",
        Box::new(move || {
            let eq = fx("eq", &[("low_gain_db", 6.0), ("mid_gain_db", -4.0), ("high_gain_db", 3.0)]);
            render(&one(Wsola, vec![(eq, vec![curve("mid_freq_hz", len, 200.0, 4_000.0, 800.0), curve("high_gain_db", len, -12.0, 12.0, 0.0)])]), 0, DUR)
        }),
    ));
    v.push((
        "freeverb moving",
        Box::new(move || {
            let sr = fx("slowed_reverb", &[("rate", 1.0), ("mix", 0.5)]);
            render(&one(Wsola, vec![(sr, vec![curve("mix", len, 0.0, 1.0, 0.4), curve("decay", len, 0.2, 0.95, 0.5)])]), 0, DUR)
        }),
    ));
    v
}

/// (case, digest) printed on the tree before per-sample-costs (8cc06ecf).
#[rustfmt::skip]
const DIGESTS: &[(&str, u64)] = &[
    ("warp curved", 0x81e7de07ceb3ae49),
    ("warp extended", 0xa7f21d20a6a00ebb),
    ("strength", 0x1582082b7b2d4f99),
    ("twin and laps wsola", 0x14018a798afb000d),
    ("twin and laps vocoder", 0x77c54984054a9605),
    ("vocoder curved", 0x142ff2ef743e216d),
    ("track key", 0x66e16dfeecc1134b),
    ("nested reel", 0x1537c5126e3e777b),
    ("phaser rate curved", 0x74533d2774beaebd),
    ("phaser depth curved", 0xe79b0a39ce6ef0f1),
    ("chorus rate curved", 0x16cf37b52a572dc2),
    ("flanger rate curved", 0x49ad6d8de1666435),
    ("tremolo rate curved", 0xb85faf259e0ad55d),
    ("filter curved", 0xbf28395e0bbc2279),
    ("delay time curved", 0x451dad66f6d6ac79),
    ("compressor curved", 0x220219e5b2a3b259),
    ("eq curved", 0x6f6b8b8cc7427545),
    ("freeverb moving", 0x9886817b505845d0),
];

fn digests() -> Vec<String> {
    let print = std::env::var("KT_PRINT_DIGESTS").is_ok();
    let mut bad = Vec::new();
    for (name, run) in cases() {
        let got = kt_fx::audio::cost::digest(&run());
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
fn t2680_the_inner_loops_keep_their_bytes() {
    kt_plugins_builtin::install();
    let fast = digests();
    if std::env::var("KT_PRINT_DIGESTS").is_ok() {
        return;
    }
    assert!(fast.is_empty(), "the CPU's path moved bytes: {fast:#?}");
    let base = kt_plugin_api::cpu::with_baseline(digests);
    assert!(base.is_empty(), "the baseline path moved bytes: {base:#?}");
    let general = kt_plugin_api::cpu::with_general(digests);
    assert!(general.is_empty(), "the general path moved bytes: {general:#?}");
}
