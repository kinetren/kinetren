//! Test 718 (R.62): the conform rule — the PROJECT with an exception on the
//! event. Changing the project's default rewrites the event that “follows
//! the project” and does not touch an event with an explicit rule (UI rule 20).

use kt_engine::{SourceFacts, VideoRenderer};
use kt_media::Quality;
use kt_model::*;
use kt_time::Tick;

fn project(rules: &[Conform], project_rule: ConformRule) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1920,
        height: 1080,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: project_rule,
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "vertical"));
    for (i, r) in rules.iter().enumerate() {
        let mut c = ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            masks: Default::default(),
            id: ClipId(1 + i as u64),
            link_group: None,
            kind: ClipKind::Video,
            lane: i as u32,
            start: Tick(0),
            duration: Tick(90_000),
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
        c.geometry.rule = *r;
        p.clips.push(c);
    }
    p.normalize(); // tracks fitted to the content (R.62)
    p.validate().unwrap();
    p
}

fn decodes(p: &Project) -> Vec<(u64, (u32, u32))> {
    let facts = SourceFacts {
        natives: std::collections::HashMap::from([(MediaId(1), (1080, 1920))]),
        procedural: Default::default(),
        views: std::collections::HashMap::from([(MediaId(1), (1080.0, 1920.0))]), ..Default::default() };
    let vr = VideoRenderer::new_with_facts(p, &facts);
    let mut v: Vec<(u64, (u32, u32))> = vr
        .video_decode_qualities(Quality { width: 1920, height: 1080 })
        .into_iter()
        .map(|(id, q)| (id.0, (q.width, q.height)))
        .collect();
    v.sort();
    v
}

#[test]
fn t718_project_rule_rewrites_followers_and_spares_explicit() {
    const FIT: (u32, u32) = (608, 1080);
    const FILL: (u32, u32) = (1920, 3412);
    const AS_IS: (u32, u32) = (1080, 1920);
    // event 1 follows the project, event 2 — an explicit “fill”
    let p = project(&[Conform::Project, Conform::Fill], ConformRule::Fit);
    assert_eq!(decodes(&p), vec![(1, FIT), (2, FILL)]);
    // the project default changed — the one that follows it changes, the explicit one does not
    let p = project(&[Conform::Project, Conform::Fill], ConformRule::AsIs);
    assert_eq!(decodes(&p), vec![(1, AS_IS), (2, FILL)]);
    // an explicit “fit” stands even when the project is “fill”
    let p = project(&[Conform::Fit, Conform::Project], ConformRule::Fill);
    assert_eq!(decodes(&p), vec![(1, FIT), (2, FILL)]);
    // “stretch” — the whole canvas; a “fit” project default is not written
    let p = project(&[Conform::Stretch], ConformRule::Fit);
    assert_eq!(decodes(&p), vec![(1, (1920, 1080))]);
    assert_eq!(p.settings.conform, ConformRule::Fit);
}

static GEN_FIX: std::sync::OnceLock<()> = std::sync::OnceLock::new();

/// A fixture of `fixtures/gen.sh` (runs it once; needs the `ffmpeg` binary, as t796b).
fn fixture(name: &str) -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let pregenerated = cfg!(windows) && dir.join("out").is_dir();
    GEN_FIX.get_or_init(|| {
        if pregenerated {
            return;
        }
        let st = std::process::Command::new("bash")
            .arg(dir.join("gen.sh"))
            .status()
            .expect("run fixtures/gen.sh (needs ffmpeg in PATH)");
        assert!(st.success(), "gen.sh failed");
    });
    dir.join("out").join(name).to_string_lossy().to_string()
}

/// Test 936 (still-image-frame-rule, ktprompts 26.10.18 item 9): ONE decision — a still has a
/// view, generators and the placeholder do not.
///
/// The facts come from the REAL `SourceFacts::observe` over the real sources' info (t718 built
/// them by hand, with `procedural` empty — that is why it never saw the defect): R.67 gave a
/// still `fps: None`, the renderer's "procedural ⇒ no view" branch took its proportions away,
/// and a square still was stretched over a 16:9 canvas under every rule.
#[test]
fn t936_a_still_has_a_view_generators_and_the_placeholder_do_not() {
    kt_plugins_builtin::install();
    use kt_media::MediaSource;
    let square = kt_media::ffmpeg::FfmpegSource::open(&fixture("still_square.png")).expect("square still");
    let video = kt_media::ffmpeg::FfmpegSource::open(&fixture("square.mp4")).expect("square video");
    let mut wide = square.info().clone();
    wide.sar = (2, 1);
    let generator = kt_media::generator::GeneratorSource::new(GeneratorKind::solid(9, 9, 9));
    let title = kt_media::generator::GeneratorSource::new(GeneratorKind::default_text());
    let placeholder = kt_media::placeholder::PlaceholderSource::new("gone.png");
    // (what, its facts, procedural, the view the renderer must see)
    type Case<'a> = (&'a str, kt_media::MediaInfo, bool, Option<(f64, f64)>);
    let cases: [Case; 6] = [
        ("square still", square.info().clone(), true, Some((176.0, 176.0))),
        ("still with SAR 2:1", wide, true, Some((352.0, 176.0))),
        ("video", video.info().clone(), false, Some((176.0, 176.0))),
        ("generator", generator.info().clone(), true, None),
        ("title", title.info().clone(), true, None),
        ("placeholder", placeholder.info().clone(), true, None),
    ];
    let mut facts = SourceFacts::default();
    let mut problems = Vec::new();
    let p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    for (i, (name, info, procedural, view)) in cases.into_iter().enumerate() {
        let m = MediaId(i as u64 + 1);
        facts.observe(m, &info);
        if facts.procedural.contains(&m) != procedural {
            problems.push(format!("{name}: procedural = {}", !procedural));
        }
        if facts.view_of(m) != view {
            problems.push(format!("{name}: view {:?}, expected {view:?}", facts.view_of(m)));
        }
        // the renderer's lead and the probe read the same value through `lead_view`
        let mut c = ClipEvent {
            r#gen: None,
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
            start: Tick::ZERO,
            duration: Tick(90_000),
            source_in: Tick::ZERO,
            media: m,
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        };
        c.media = m;
        if kt_engine::video::lead_view(&p, &facts, &c, m) != facts.view_of(m) {
            problems.push(format!("{name}: the lead's view differs from the source's"));
        }
    }
    assert!(problems.is_empty(), "the source's proportions are not one fact: {}", problems.join("; "));
}
