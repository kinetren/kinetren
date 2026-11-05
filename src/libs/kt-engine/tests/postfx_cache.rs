//! Test 66 (item 2 of round 7): the video post-FX cache — a repeated compose is byte-identical
//! and comes from the cache (without reapplying the chain); a change of FX parameters
//! or of clip geometry gives another fx_hash → a stale entry is not read.
//! Test 67 — in golden_export.rs (preview==export with FX, cold/warm).

use kt_engine::VideoRenderer;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

fn project_with_fx(fx: Vec<FxInstance>, start_frames: i64) -> Project {
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
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
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
        kind: ClipKind::Video,
        lane: 0,
        start: Tick::from_audio_frames(start_frames),
        duration: Tick::from_audio_frames(48_000 * 2),
        source_in: Tick::from_audio_frames(9_600),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn warm_pre_fx(vr: &VideoRenderer, cache: &kt_cache::BurstCache, t: Tick, q: kt_media::Quality) {
    use kt_media::MediaSource;
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    for (media, src_pts, q) in vr.layers_at(t, q) {
        let frame = synth.read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video_cache_idx(src_pts),
            std::sync::Arc::new(frame),
        );
    }
}

/// Test 708 (R.61): an edit of ONLY the formula text recomputes the frame.
///
/// The reason this is a separate test: the formula is resolved into numbers BEFORE
/// the engine computes the post-FX key. So the requirement “an edit of the formula
/// will not leave the old frames on screen” holds NOT because the text lies in the
/// hash, but because resolution runs before the hash and changes the values themselves.
/// The test's assertion is the observable consequence: the frame is different, no stale.
#[test]
fn t708_editing_the_formula_text_invalidates_the_frame() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    kt_plugins_builtin::install();
    // the vignette is VISIBLE on a flat frame: it darkens the edges regardless of
    // the content, unlike blur (on a uniform picture it is the identity) and unlike Pan3d
    // (without a camera the quad leaves the frame entirely)
    let with_formula = |expr: &str| {
        let mut p = project_with_fx(
            vec![kt_model::FxInstance::plugin_with("vignette", &[0.5, 0.7, 0.45]).unwrap()],
            48_000,
        );
        p.clips[0].fx_auto = vec![vec![ParamCurve {
            param: "strength".into(),
            keys: Vec::new(),
            source: CurveSource::Formula { expr: expr.into() },
        }]];
        p.validate().expect("formula is legal");
        p
    };
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000 + 3000);

    let p1 = with_formula("0.1");
    let vr1 = VideoRenderer::new(&p1);
    warm_pre_fx(&vr1, &cache, t, q);
    let vr1 = vr1;
    let (a, c1) = kt_gpu::testkit::compose(&vr1, &cache, t, q);
    assert!(c1, "the first frame did not assemble");
    assert!(a.iter().any(|&b| b != 0), "the frame is empty — the test will prove nothing");

    // only the TEXT is edited: the effect parameters are the same
    let p2 = with_formula("0.9");
    let vr2 = VideoRenderer::new(&p2);
    let (b, c2) = kt_gpu::testkit::compose(&vr2, &cache, t, q);
    assert!(c2, "the second frame did not assemble");
    assert_ne!(a, b, "an edit of the formula text gave STALE data: the values change, but the post-FX key does not");

    // the same text — the same frame: the key does not “jitter” for no reason
    let p3 = with_formula("0.1");
    let vr3 = VideoRenderer::new(&p3);
    let (c, c3) = kt_gpu::testkit::compose(&vr3, &cache, t, q);
    assert!(c3);
    assert_eq!(a, c, "the same formula gave different frames — the key is non-deterministic");
}
