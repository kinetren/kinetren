//! Test 748c (R.63 item 2): the right edge of an event runs into the END OF THE MATERIAL.
//!
//! Before R.63 `trim_max_end` for a non-mirrored event gave `i64::MAX / 4`:
//! the edge could be dragged arbitrarily far to the right. That is exactly how the owner’s
//! event became 316.967 s long for a 226.268 s file — the last 11.5 s of
//! material “sounded” as silence and counted as underruns. The limit is set
//! BY THE MATERIAL: a slowed event runs into it later, a sped-up one earlier.
//! Events already longer than their material are not shortened by themselves
//! (rule 20) — they are legal, and the engine gives them honest silence.

use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 90_000;

fn scene(media_len: Option<Tick>, rate: Option<f64>) -> TimelineModel {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(match media_len {
        Some(d) => MediaRef::with_duration(MediaId(1), "synth:ramp", d),
        None => MediaRef::new(MediaId(1), "synth:ramp"),
    });
    let mut c = ClipEvent {
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
        duration: Tick(2 * SEC),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    if let Some(r) = rate {
        c.fx = vec![FxInstance::slowed_reverb(r, 0.0, 120.0)];
        c.fx_bypass = vec![false];
    }
    p.clips.push(c);
    p.normalize();
    p.validate().unwrap();
    TimelineModel::new(p)
}

fn end_of(m: &TimelineModel) -> i64 {
    let c = &m.project().clips[0];
    (c.start + c.duration).0
}

#[test]
fn t748c_trim_end_stops_at_the_material_end() {
    // 1) material 10 s, speed 1 — nothing past 10 s
    let mut m = scene(Some(Tick(10 * SEC)), None);
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(30 * SEC) }).unwrap();
    assert_eq!(
        end_of(&m),
        10 * SEC,
        "the edge did not hit the end of the material (on the previous code — 30 s)"
    );
    // inside the material, trimming is still free
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(6 * SEC) }).unwrap();
    assert_eq!(end_of(&m), 6 * SEC, "trim within the material must work");

    // 2) SLOWDOWN: 10 s of material at rate 0.5 is 20 s on the timeline
    let mut m = scene(Some(Tick(10 * SEC)), Some(0.5));
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(60 * SEC) }).unwrap();
    let got = end_of(&m);
    assert!(
        (got - 20 * SEC).abs() <= SEC / 30,
        "for a slowed event the limit is computed from the MATERIAL: expected ≈20 s, got {} s",
        got as f64 / SEC as f64
    );

    // 3) SPEED-UP: the same 10 s of material at rate 2 is 5 s
    let mut m = scene(Some(Tick(10 * SEC)), Some(2.0));
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(60 * SEC) }).unwrap();
    let got = end_of(&m);
    assert!(
        (got - 5 * SEC).abs() <= SEC / 30,
        "for a sped-up event the limit is closer: expected ≈5 s, got {} s",
        got as f64 / SEC as f64
    );

    // 4) length UNKNOWN (generator, reel, file not yet opened) —
    //    no limit, as it always was
    let mut m = scene(None, None);
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(30 * SEC) }).unwrap();
    assert_eq!(end_of(&m), 30 * SEC, "without a known length there must be no limit");

    // 5) the event is ALREADY longer than its material (the owner’s file) — legal and
    //    not shortened by itself (rule 20)
    let mut m = scene(Some(Tick(10 * SEC)), None);
    {
        let mut p = m.project().clone();
        p.clips[0].duration = Tick(30 * SEC);
        p.validate().expect("an event longer than the material is legal");
        m = TimelineModel::new(p);
    }
    assert_eq!(end_of(&m), 30 * SEC);
    // it can be TRIMMED (the limit does not drag the edge back in a jump)
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(25 * SEC) }).unwrap();
    assert_eq!(end_of(&m), 25 * SEC, "trimming an event past the material must work");
    // but growing it FURTHER is no longer possible
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(40 * SEC) }).unwrap();
    assert_eq!(end_of(&m), 25 * SEC, "an event past the material has no right to grow further");
}
