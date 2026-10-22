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

#[test]
fn t66_postfx_cached_and_no_stale() {
    // Noise: the amplitude changes the bytes even on a flat synthetic frame
    // (SyntheticSource encodes pts as a solid color — Mirror/Vhs degenerate
    // to identity on it, a stale entry cannot be told apart by bytes)
    let chain =
        vec![FxInstance::Mirror { horizontal: true, vertical: false }, FxInstance::noise(0.3)];
    let p = project_with_fx(chain.clone(), 48_000);
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000 + 3000);
    warm_pre_fx(&vr, &cache, t, q);

    let (a, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete);
    let bytes_after_first = cache.video_bytes();
    let hits_after_first = cache.stats().video_hits;

    // repeated compose: byte-identical, from the cache (the bytes do not grow — no
    // repeated insertion), the post-FX hit is registered
    let (b, complete2) = vr.compose_from_cache(&cache, t, q);
    assert!(complete2);
    assert_eq!(a, b, "a repeated compose is byte-identical");
    assert_eq!(cache.video_bytes(), bytes_after_first, "hit: without reinsertion");
    assert!(cache.stats().video_hits > hits_after_first, "post-FX was read from the cache");

    // a change of a chain PARAMETER → another fx_hash → recomputation (cache bytes
    // grow: a new key is inserted, not a stale hit) and another result
    let p2 = project_with_fx(
        vec![FxInstance::Mirror { horizontal: true, vertical: false }, FxInstance::noise(0.9)],
        48_000,
    );
    let mut vr2 = VideoRenderer::new(&p2);
    let (c, _) = vr2.compose_from_cache(&cache, t, q);
    assert_ne!(a, c, "changed FX parameters must not read stale data");
    assert!(cache.video_bytes() > bytes_after_first, "a new fx_hash → a new entry");

    // a clip shift with the same chain → another fx_hash (the u/t mapping
    // changed) → recomputation with insertion, not a stale hit
    let bytes_before_shift = cache.video_bytes();
    let p3 = project_with_fx(chain, 48_000 - 4800); // t stays inside the clip
    let mut vr3 = VideoRenderer::new(&p3);
    warm_pre_fx(&vr3, &cache, t, q);
    let bytes_warmed = cache.video_bytes();
    assert!(bytes_warmed >= bytes_before_shift);
    let (_, complete3) = vr3.compose_from_cache(&cache, t, q);
    assert!(complete3);
    assert!(
        cache.video_bytes() > bytes_warmed,
        "a shifted clip must recompute post-FX (a different fx_hash), not take a stale one"
    );
}

/// A post-FX hit makes the layer complete even when the pre-FX frame is already evicted:
/// we warm up, evict the pre-FX by overwriting the budget... simpler: a new cache
/// with only a post-FX entry cannot be assembled from outside, so we check more weakly —
/// removing the pre-FX via invalidate_source also removes the post-FX (no orphaned
/// stale entries when the file on disk changes).
#[test]
fn t66b_invalidate_source_drops_postfx() {
    let p = project_with_fx(vec![FxInstance::glow(1.0)], 48_000);
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000 + 3000);
    warm_pre_fx(&vr, &cache, t, q);
    let (_, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete);
    assert!(cache.video_bytes() > 0);
    cache.invalidate_source(kt_engine::source_id(MediaId(1)));
    assert_eq!(cache.video_bytes(), 0, "invalidate wiped both pre-FX and post-FX");
    let (_, complete_cold) = vr.compose_from_cache(&cache, t, q);
    assert!(!complete_cold, "after invalidation there are no layers");
}

/// R.31 item 4: the export compose does NOT memoize post-FX frames — on
/// multi-layer edits they evicted pre-frames of neighboring tracks not yet used from the export
/// cache, and the render lost layers. The frame bytes must still match the preview
/// compose.
#[test]
fn t66c_export_compose_skips_postfx_memoization() {
    let p = project_with_fx(
        vec![FxInstance::Mirror { horizontal: true, vertical: false }, FxInstance::noise(0.3)],
        0,
    );
    let q = kt_media::Quality { width: 640, height: 360 };
    let t = Tick(0);

    let cache_a = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut vr_a = VideoRenderer::new(&p);
    warm_pre_fx(&vr_a, &cache_a, t, q);
    let bytes_before = cache_a.video_bytes();
    let mut out_a = Vec::new();
    let complete = vr_a.compose_export_into(&cache_a, t, q, &mut out_a);
    assert!(complete);
    assert_eq!(
        cache_a.video_bytes(),
        bytes_before,
        "the export compose has no right to bloat the post-FX cache with frames"
    );

    // the preview compose on a separate cache — the bytes are the same, but memoization is present
    let cache_b = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut vr_b = VideoRenderer::new(&p);
    warm_pre_fx(&vr_b, &cache_b, t, q);
    let mut out_b = Vec::new();
    vr_b.compose_from_cache_into(&cache_b, t, q, &mut out_b);
    assert_eq!(out_a, out_b, "the export and preview compose must match byte for byte");
    assert!(cache_b.video_bytes() > bytes_before, "preview memoizes as before");
}

/// Test 66d (R.44): the CAMERA — in the post-FX memoization key. The CPU computes
/// Pan3d, and the camera is resolved per frame (the active-camera rule,
/// animation) — a static fx_hash does not see it: without the camera in the key, an edit
/// or a move of the camera would return a stale frame (the GPU is protected from this by
/// put_cam3d in plan_key since R.29).
#[test]
fn t66d_camera_edit_invalidates_postfx() {
    let pan3d = FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 0.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale: 0.5, // transparent margins: the quad's shift is visible on a flat frame too
        camera: None,
    };
    let cam_at = |pos_x: f64, start: i64| CameraEvent {
        ease: true,
        id: CameraId(1),
        lane: 0,
        start: Tick(start),
        duration: Tick(90_000 * 4),
        name: String::new(),
        cam: CameraParams { pos_x, ..CameraParams::default() },
        keyframes: vec![],
        auto: vec![],
    };
    let with_cam = |cam: CameraEvent| {
        let mut p = project_with_fx(vec![pan3d.clone()], 48_000);
        p.ensure_tracks(kt_model::LaneKind::Camera, 1);
        p.cameras = vec![cam];
        p.validate().unwrap();
        p
    };
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000 + 3000);

    let p1 = with_cam(cam_at(0.0, 0));
    let mut vr1 = VideoRenderer::new(&p1);
    warm_pre_fx(&vr1, &cache, t, q);
    let (a, c1) = vr1.compose_from_cache(&cache, t, q);
    assert!(c1);

    // an edit of a camera PARAMETER (the clip's fx_hash does NOT change, the cache survived
    // the renderer re-creation) → the frame must be recomputed
    let p2 = with_cam(cam_at(20.0, 0));
    let mut vr2 = VideoRenderer::new(&p2);
    let (b, c2) = vr2.compose_from_cache(&cache, t, q);
    assert!(c2);
    assert_ne!(a, b, "an edit of the camera must not read stale post-FX");

    // a MOVE of a camera event beyond t: there is no active camera —
    // the default “50 mm”; the frame is different again (not stale from p2)
    let p3 = with_cam(cam_at(20.0, 90_000 * 10));
    let mut vr3 = VideoRenderer::new(&p3);
    let (c, c3) = vr3.compose_from_cache(&cache, t, q);
    assert!(c3);
    assert_ne!(b, c, "moving the camera (switching the active one) must not read stale data");
    // the default camera == camera p1 (pos_x 0) → the frames are equal, AND this is a memo hit
    // (same key) — the camera in the key does not hinder reuse
    assert_eq!(a, c, "the default camera reproduces p1");
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
    // the vignette is VISIBLE on a flat frame: it darkens the edges regardless of
    // the content, unlike blur (on a uniform picture it is the identity) and unlike Pan3d
    // (without a camera the quad leaves the frame entirely)
    let with_formula = |expr: &str| {
        let mut p = project_with_fx(
            vec![FxInstance::Vignette { strength: 0.5, radius: 0.7, softness: 0.45 }],
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
    let mut vr1 = vr1;
    let (a, c1) = vr1.compose_from_cache(&cache, t, q);
    assert!(c1, "the first frame did not assemble");
    assert!(a.iter().any(|&b| b != 0), "the frame is empty — the test will prove nothing");

    // only the TEXT is edited: the effect parameters are the same
    let p2 = with_formula("0.9");
    let mut vr2 = VideoRenderer::new(&p2);
    let (b, c2) = vr2.compose_from_cache(&cache, t, q);
    assert!(c2, "the second frame did not assemble");
    assert_ne!(a, b, "an edit of the formula text gave STALE data: the values change, but the post-FX key does not");

    // the same text — the same frame: the key does not “jitter” for no reason
    let p3 = with_formula("0.1");
    let mut vr3 = VideoRenderer::new(&p3);
    let (c, c3) = vr3.compose_from_cache(&cache, t, q);
    assert!(c3);
    assert_eq!(a, c, "the same formula gave different frames — the key is non-deterministic");
}
