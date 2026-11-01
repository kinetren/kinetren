//! t1167 (generators-placement-and-audio, capability `generators`): a sound generator plays in the
//! offline audio render — a `tone` event on audio track 1 and a `beep` event on audio track 2
//! sound at their places and levels, silence around them, and two renders are byte-identical.

use kt_engine::{AudioRenderer, DirectFetcher};
use kt_model::*;
use kt_time::Tick;

const S: i64 = 90_000;

fn sound_kind(tag: &str, values: &[(&str, f64)]) -> GeneratorKind {
    let mut k = GeneratorKind::all().into_iter().find(|k| k.tag() == tag).expect("a sound kind");
    if let GeneratorKind::Plugin(p) = &mut k {
        for (n, v) in values {
            assert!(p.set(n, *v));
        }
    }
    k
}

fn event(id: u64, lane: u32, start: i64, dur: i64, media: u64, kind: GeneratorKind) -> ClipEvent {
    ClipEvent {
        r#gen: Some(kind),
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
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
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

fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 96,
        height: 54,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    let tone = sound_kind("tone", &[("freq", 1000.0), ("level", -20.0)]);
    let beep = sound_kind("beep", &[("freq", 2000.0), ("level", -12.0), ("period", 0.5), ("length", 20.0)]);
    p.media.push(MediaRef::new(MediaId(1), gen_kind_path(&tone)));
    p.media.push(MediaRef::new(MediaId(2), gen_kind_path(&beep)));
    p.ensure_tracks(LaneKind::Audio, 2);
    p.clips = vec![event(1, 0, 2 * S, 2 * S, 1, tone), event(2, 1, 5 * S, 2 * S, 2, beep)];
    p.validate().expect("a legal project");
    p
}

fn render(p: &Project, frames: usize) -> Vec<f32> {
    let mut f = DirectFetcher::new(None);
    for (addr, state) in kt_engine::gen_sources(p) {
        f.add_source(addr, kt_media::factory::open_gen(&state));
    }
    let mut out = vec![0f32; frames * 2];
    let st = AudioRenderer::new(p).render_frames(&mut f, 0, &mut out);
    assert_eq!(st.underruns, 0, "a generator never underruns");
    out
}

fn peak(x: &[f32], from_s: f64, to_s: f64) -> f64 {
    let (a, b) = ((from_s * 48_000.0) as usize, (to_s * 48_000.0) as usize);
    x[a * 2..b * 2].iter().fold(0f64, |m, v| m.max(v.abs() as f64))
}

#[test]
fn t1167_a_sound_generator_plays_in_a_render() {
    kt_plugins_builtin::install();
    let p = project();
    let out = render(&p, 8 * 48_000);
    let db = |v: f64| 20.0 * v.log10();
    assert!((db(peak(&out, 2.01, 3.99)) + 20.0).abs() < 0.01, "the tone at −20 dBFS: {:.3}", db(peak(&out, 2.01, 3.99)));
    assert_eq!(peak(&out, 0.0, 1.99), 0.0, "silence before the tone");
    assert_eq!(peak(&out, 4.01, 4.99), 0.0, "silence between the events");
    assert!((db(peak(&out, 5.0, 7.0)) + 12.0).abs() < 0.01, "the beep at −12 dBFS");
    // the pips: on for 20 ms at every half second of the material, off between
    for k in 0..4 {
        let t = 5.0 + 0.5 * k as f64;
        assert!(peak(&out, t + 0.001, t + 0.019) > 0.2, "pip {k} sounds");
        assert_eq!(peak(&out, t + 0.021, t + 0.49), 0.0, "silent after pip {k}");
    }
    assert_eq!(peak(&out, 7.01, 8.0), 0.0, "silence after the beep");
    assert_eq!(out, render(&p, 8 * 48_000), "two renders are byte-identical");
}
