//! text-counter-source (`v26.10` G5's timer/counter as a Text source, the owner's G3 decision):
//! tests t3703–t3704; the contract — `openspec/specs/generators/spec.md`.

use kt_model::*;
use kt_time::Tick;

fn title(source: &str, text: &str, from: f64, format: &str) -> GeneratorKind {
    let mut k = GeneratorKind::default_text();
    assert!(k.set_value("source", &ParamValue::Choice(source.into())));
    assert!(k.set_value("count_format", &ParamValue::Choice(format.into())));
    assert!(k.set_value("text", &ParamValue::Text(text.into())));
    k.set_param("count_from", from);
    k
}

const S: i64 = 90_000;

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn gen_clip(kind: GeneratorKind) -> ClipEvent {
    ClipEvent {
        r#gen: Some(kind),
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(5 * S),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
    }
}

/// Test 3703: the running values — a timecode of the material time, a stopwatch counting up (what has
/// passed), a countdown counting down (what is left, 0 only when the time is up), the four formats,
/// and the typed text as a template with `{}`.
#[test]
fn t3703_a_title_runs_its_value_with_the_material_time() {
    let tc = title("timecode", "", 3600.0, "m_ss");
    assert_eq!(title_text_at(&tc, Tick(S + S / 2)), "01:00:01:15", "a timecode at 30 fps");
    let sw = title("stopwatch", "Lap {} s", 0.0, "s_tenths");
    assert_eq!(title_text_at(&sw, Tick(65 * S + S / 3)), "Lap 65.3 s", "the template keeps the words");
    let sw = title("stopwatch", "", 0.0, "m_ss");
    assert_eq!(title_text_at(&sw, Tick(65 * S + S - 1)), "1:05", "a stopwatch shows what has passed");
    let cd = title("countdown", "", 5.0, "m_ss");
    assert_eq!(title_text_at(&cd, Tick(0)), "0:05");
    assert_eq!(title_text_at(&cd, Tick(S / 5)), "0:05", "a countdown shows what is left, rounded up");
    assert_eq!(title_text_at(&cd, Tick(5 * S - 1)), "0:01");
    assert_eq!(title_text_at(&cd, Tick(5 * S)), "0:00");
    assert_eq!(title_text_at(&cd, Tick(9 * S)), "0:00", "a countdown stays at zero");
    let long = title("stopwatch", "", 3_725.0, "h_mm_ss");
    assert_eq!(title_text_at(&long, Tick(0)), "1:02:05");
    let tenths = title("countdown", "", 2.0, "m_ss_t");
    assert_eq!(title_text_at(&tenths, Tick(S / 2)), "0:01.5");
    let typed = title("static", "Hello", 9.0, "m_ss");
    assert_eq!(title_text_at(&typed, Tick(42 * S)), "Hello", "a typed title ignores the time");
}

/// Test 3704: the source travels through the gen path and the file; a title without a source writes
/// the former bytes and stays format 1, with a source the file is format 7.
#[test]
fn t3704_the_source_is_written_by_content() {
    kt_plugins_builtin::install();
    let cd = title("countdown", "T-{}", 10.0, "m_ss_t");
    let path = format_gen_path(&cd);
    assert_eq!(parse_gen_path(&path).as_ref(), Some(&cd), "the gen path round trip: {path}");
    let json = serde_json::to_string(&cd).unwrap();
    assert_eq!(serde_json::from_str::<GeneratorKind>(&json).unwrap(), cd, "the serde round trip");
    let plain = serde_json::to_string(&GeneratorKind::default_text()).unwrap();
    assert!(!plain.contains("source") && !plain.contains("count_"), "a typed title writes new fields: {plain}");
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "gen:text"));
    p.clips.push(gen_clip(GeneratorKind::default_text()));
    assert_eq!(project_file::version_for(&p), 1, "a typed title keeps format 1");
    p.clips[0].r#gen = Some(cd);
    assert_eq!(project_file::version_for(&p), project_file::PROJECT_FILE_VERSION_TEXT_SOURCE, "a running title is format 7");
}
