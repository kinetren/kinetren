//! Test 796b (R.67): a SINGLE PICTURE in the plan, the composite and the export.
//!
//! Owner: “still pictures do not display at all, ever.. there is an eternal
//! Loading... and not only does it not work — you will also find out about it
//! only at the moment of viewing”. Not a single fixture with stills existed, and the
//! path “picture → plan → frame → file” had never been under test.
//!
//! Requires the system FFmpeg (like conformance test 10 and export_options).

use kt_media::{MediaSource, Quality};

/// A short fingerprint of the frame: comparing arrays of 230 KB in `assert_eq!`
/// would flood the test output with a megabyte of numbers.
fn fp(bytes: &[u8]) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}
use kt_model::*;
use kt_time::Tick;

const Q: Quality = Quality { width: 320, height: 180 };

static GEN: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn fixture(name: &str) -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    // on Windows the fixtures come generated with the test archive (no bash to run gen.sh)
    let pregenerated = cfg!(windows) && dir.join("out").is_dir();
    GEN.get_or_init(|| {
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

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: Q.width,
        height: Q.height,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn clip(id: u64, media: u64, lane: u32, dur_s: f64) -> ClipEvent {
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
        kind: ClipKind::Video,
        lane,
        start: Tick::ZERO,
        duration: Tick((dur_s * 90_000.0) as i64),
        source_in: Tick::ZERO,
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

/// The composite of moment `t`, warmed exactly by what the plan requested.
fn compose(p: &Project, t: Tick) -> (Vec<u8>, bool) {
    let mut video = kt_engine::VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        std::collections::HashMap::new();
    for m in &p.media {
        let s = kt_media::ffmpeg::FfmpegSource::open(&m.path)
            .unwrap_or_else(|e| panic!("{}: {e}", m.path));
        srcs.insert(m.id, Box::new(s));
    }
    for (addr, pts, dq) in video.layers_at(t, Q) {
        let f = srcs
            .get_mut(&addr)
            .unwrap_or_else(|| panic!("address {addr:?} was requested, but there is no source under it"))
            .read_video_frame(pts, Some(dq))
            .unwrap_or_else(|e| panic!("image frame at {pts:?}: {e}"));
        cache.insert_video(
            kt_engine::source_id(addr),
            Some(dq),
            kt_engine::video::video_cache_idx(pts),
            f.into(),
        );
    }
    video.compose_from_cache(&cache, t, Q)
}

#[test]
fn t796b_a_still_is_planned_composed_and_exported() {
    // --- an event of 10 s on a PICTURE: a frame exists on any tick -------------
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), fixture("still.jpg")));
    p.clips.push(clip(1, 1, 0, 10.0));
    p.ensure_tracks(LaneKind::Video, 1);
    p.validate().expect("an event on an image is legal");

    let mut shots = Vec::new();
    for t in [Tick(0), Tick((3.7 * 90_000.0) as i64), Tick((9.96 * 90_000.0) as i64)] {
        let (bytes, complete) = compose(&p, t);
        assert!(complete, "the composite at {t:?} is not fully populated — this is the eternal “Loading…”");
        assert!(bytes.iter().any(|&b| b != 0), "image frame at {t:?} is empty");
        shots.push(bytes);
    }
    assert_eq!(fp(&shots[0]), fp(&shots[1]), "at 3.7 s a different frame arrived: an image has only one");
    assert_eq!(fp(&shots[0]), fp(&shots[2]), "at 9.96 s (almost the end of the event) the frame was gone");

    // --- determinism: the second run is byte-for-byte --------------------------------
    let (again, _) = compose(&p, Tick(0));
    assert_eq!(fp(&again), fp(&shots[0]), "the image composite is non-deterministic");

    // --- PNG WITH ALPHA over a colored background: the background shows through ----------
    let mut over = Project::new(settings());
    over.media.push(MediaRef::new(MediaId(1), fixture("still.jpg")));
    over.media.push(MediaRef::new(MediaId(2), fixture("still_alpha.png")));
    // track 0 — the TOP one: the semi-transparent PNG goes onto it, the jpeg as the background
    over.clips.push(clip(2, 2, 0, 4.0));
    over.clips.push(clip(1, 1, 1, 4.0));
    over.ensure_tracks(LaneKind::Video, 2);
    over.validate().expect("an image over an image is legal");
    let (blended, complete) = compose(&over, Tick(0));
    assert!(complete, "the composite of two images is not fully populated");
    // background (lower layer) — the same jpeg; overlaying a semi-transparent PNG
    // MUST give something in between, not the pure upper layer
    let (top, _) = compose(
        &{
            let mut only = Project::new(settings());
            only.media.push(MediaRef::new(MediaId(2), fixture("still_alpha.png")));
            only.clips.push(clip(2, 2, 0, 4.0));
            only.ensure_tracks(LaneKind::Video, 1);
            only.validate().unwrap();
            only
        },
        Tick(0),
    );
    assert_ne!(fp(&blended), fp(&top), "PNG alpha did not work: the top layer landed opaque");
    assert_ne!(fp(&blended), fp(&shots[0]), "the top layer is not visible in the composite at all");

    // --- EXPORT 1 s: the file exists and contains frames of the picture ----------
    let dir = std::env::temp_dir().join(format!("kt-t796b-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("still.mp4");
    let mut short = p.clone();
    short.clips[0].duration = Tick(90_000);
    short.validate().unwrap();
    let sources = || -> Vec<(MediaId, Box<dyn MediaSource>)> {
        short
            .media
            .iter()
            .map(|m| {
                let s = kt_media::ffmpeg::FfmpegSource::open(&m.path).expect("source");
                (m.id, Box::new(s) as Box<dyn MediaSource>)
            })
            .collect()
    };
    // audio and video — THEIR OWN sources: sharing one between the passes means
    // sharing the read position too (invariant 5)
    kt_engine::export::export_mp4(&short, sources(), sources(), out.to_str().unwrap())
        .expect("export of an event on an image");
    let size = std::fs::metadata(&out).expect("there is no export file").len();
    assert!(size > 1000, "the export file is suspiciously small: {size} bytes");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The composite of moment `t` with the facts of the opened sources (the product's path:
/// `VideoRenderer::new_with_facts`), on the CPU reference.
fn compose_facts(p: &Project, t: Tick, q: Quality) -> (Vec<u8>, bool) {
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        std::collections::HashMap::new();
    let mut facts = kt_engine::SourceFacts::default();
    for m in &p.media {
        let s: Box<dyn MediaSource> = if m.path.starts_with("gone:") {
            Box::new(kt_media::placeholder::PlaceholderSource::new(m.path.clone()))
        } else {
            Box::new(kt_media::ffmpeg::FfmpegSource::open(&m.path).unwrap_or_else(|e| panic!("{}: {e}", m.path)))
        };
        facts.observe(m.id, s.info());
        srcs.insert(m.id, s);
    }
    let mut video = kt_engine::VideoRenderer::new_with_facts(p, &facts);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    for (addr, pts, dq) in video.layers_at(t, q) {
        let f = srcs.get_mut(&addr).expect("source").read_video_frame(pts, Some(dq)).expect("frame");
        cache.insert_video(kt_engine::source_id(addr), Some(dq), kt_engine::video::video_cache_idx(pts), f.into());
    }
    video.compose_from_cache(&cache, t, q)
}

/// The columns of the frame where the picture lies — by ALPHA: the margins of fit are
/// transparent, the picture is opaque (colour would read a codec-darkened edge of the video as a
/// margin).
fn picture_columns(bytes: &[u8], q: Quality) -> (usize, usize) {
    let (w, h) = (q.width as usize, q.height as usize);
    let lit = |x: usize| (0..h).any(|y| bytes[(y * w + x) * 4 + 3] > 0);
    let first = (0..w).find(|&x| lit(x)).unwrap_or(w);
    let last = (0..w).rev().find(|&x| lit(x)).map_or(0, |x| x + 1);
    (first, last)
}

/// Test 937 (still-image-frame-rule, ktprompts 26.10.18 item 9: "a square still on a 16:9
/// timeline is squashed, and not one frame-rule option works — why does it work for video and
/// not for stills?"): under every rule a square still is placed exactly like a square video.
#[test]
fn t937_four_rules_a_square_still_equals_a_square_video() {
    let one = |path: String, rule: Conform, project_rule: ConformRule| -> Vec<u8> {
        let mut s = settings();
        s.conform = project_rule;
        let mut p = Project::new(s);
        p.media.push(MediaRef::new(MediaId(1), path));
        let mut c = clip(1, 1, 0, 0.5);
        c.geometry.rule = rule;
        p.clips.push(c);
        p.ensure_tracks(LaneKind::Video, 1);
        p.validate().unwrap();
        let (bytes, complete) = compose_facts(&p, Tick(0), Q);
        assert!(complete, "the composite is not complete");
        bytes
    };
    // the analytical columns of the picture on a 320×180 canvas
    let cases = [
        ("fit", Conform::Fit, ConformRule::Fit, (70, 250)),
        ("fill", Conform::Fill, ConformRule::Fit, (0, 320)),
        ("stretch", Conform::Stretch, ConformRule::Fit, (0, 320)),
        ("as is", Conform::AsIs, ConformRule::Fit, (72, 248)),
        ("project (fill)", Conform::Project, ConformRule::Fill, (0, 320)),
    ];
    let mut problems = Vec::new();
    let mut frames = std::collections::HashMap::new();
    for (name, rule, project_rule, want) in cases {
        let still = one(fixture("still_square.png"), rule, project_rule);
        let video = one(fixture("square.mp4"), rule, project_rule);
        let (s, v) = (picture_columns(&still, Q), picture_columns(&video, Q));
        if s.0.abs_diff(want.0) > 1 || s.1.abs_diff(want.1) > 1 {
            problems.push(format!("{name}: the still's picture spans columns {s:?}, expected {want:?}"));
        }
        if s.0.abs_diff(v.0) > 1 || s.1.abs_diff(v.1) > 1 {
            problems.push(format!("{name}: the still {s:?} and the video {v:?} are placed differently"));
        }
        frames.insert(name, still);
    }
    // fill crops, stretch squeezes: the two full-width pictures differ
    if frames["fill"] == frames["stretch"] {
        problems.push("fill and stretch gave the same still frame — the rule does nothing".into());
    }
    assert!(problems.is_empty(), "the still does not follow the frame rule: {}", problems.join("; "));
}

/// Test 941 (still-image-frame-rule): the sources drawn at the requested quality keep "frame =
/// canvas" — the missing-file placeholder fills a 4:3 canvas with no margins under fit.
#[test]
fn t941_the_placeholder_still_fills_the_canvas() {
    let mut s = settings();
    s.width = 240;
    s.height = 180;
    let mut p = Project::new(s);
    p.media.push(MediaRef::new(MediaId(1), "gone:missing.png"));
    p.clips.push(clip(1, 1, 0, 0.5));
    p.ensure_tracks(LaneKind::Video, 1);
    p.validate().unwrap();
    let q = Quality { width: 240, height: 180 };
    let (bytes, complete) = compose_facts(&p, Tick(0), q);
    assert!(complete);
    let (first, last) = picture_columns(&bytes, q);
    assert_eq!((first, last), (0, 240), "the placeholder left margins on a 4:3 canvas");
}

