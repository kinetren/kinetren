//! Test 191 (R.30): automation of parameters in the render. Curves are
//! resolved when the frame plan is built — the composite of an animated clip
//! at time t is byte for byte equal to the composite of the clip with the curve's
//! value STATICALLY set at that moment. The post-FX cache returns no stale frames across
//! different t; editing a curve changes fx_hash (the frames are recomputed).

use kt_engine::VideoRenderer;
use kt_model::*;
use kt_time::Tick;
use std::sync::Arc;

const W: u32 = 64;
const H: u32 = 36;
const SPAN: i64 = 90_000;

fn frame() -> Arc<kt_media::VideoFrame> {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            rgba[i] = (x * 4) as u8;
            rgba[i + 1] = (y * 7) as u8;
            rgba[i + 2] = ((x + y) * 3) as u8;
            rgba[i + 3] = 255;
        }
    }
    Arc::new(kt_media::VideoFrame {
        pts: Tick(0),
        width: W,
        height: H,
        rgba: rgba.into_boxed_slice(),
        has_alpha: false,
    })
}

fn project(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: W,
        height: H,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
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
        start: Tick(0),
        duration: Tick(SPAN),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: auto,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn compose(p: &Project, t: Tick, cache: &kt_cache::BurstCache) -> (Vec<u8>, bool) {
    let mut vr = VideoRenderer::new(p);
    let q = kt_media::Quality { width: W, height: H };
    for (media, src_pts, q) in vr.layers_at(t, q) {
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame(),
        );
    }
    vr.compose_from_cache(cache, t, q)
}

#[test]
fn t191_animated_equals_static_at_each_moment() {
    let base = vec![FxInstance::blur(8), FxInstance::glow(0.5)];
    let auto = vec![
        vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(SPAN), 47.0)]),
            source: Default::default(),
        }],
        vec![ParamCurve {
            param: "strength".into(),

            keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(SPAN), 2.0)]),
            source: Default::default(),
        }],
    ];
    let animated = project(base.clone(), auto);
    // moments on the 30 fps grid (step 3000 ticks): start, middle, near the end
    for t in [Tick(0), Tick(SPAN / 2), Tick(87_000)] {
        // the static equivalent: the values of the curves at that moment. The curve is
        // sampled by the SOURCE frame that is shown at that tick, so here too we take a
        // position aligned to the project grid
        let step = 90_000 / 30;
        let u = (t.0 / step * step) as f64 / SPAN as f64;
        let r = 1.0 + (47.0 - 1.0) * u;
        let s = 2.0 * u;
        let static_p =
            project(vec![FxInstance::blur(r.round() as u32), FxInstance::glow(s)], vec![]);
        let cache_a = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        let cache_s = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        let (a, ca) = compose(&animated, t, &cache_a);
        let (b, cb) = compose(&static_p, t, &cache_s);
        assert!(ca && cb);
        assert_eq!(a, b, "animated != static at t={} (u={u})", t.0);
    }
}

#[test]
fn t191_postfx_cache_no_stale_across_time_and_curve_edits() {
    let base = vec![FxInstance::blur(8)];
    let curve = |v1: f64| {
        vec![vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(SPAN), v1)]),
            source: Default::default(),
        }]]
    };
    let p = project(base.clone(), curve(47.0));
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let (t0, t1) = (Tick(0), Tick(SPAN / 2));
    let (a0, _) = compose(&p, t0, &cache);
    let (a1, _) = compose(&p, t1, &cache);
    assert_ne!(a0, a1, "different moments of animation give different frames");
    // a repeat with the same cache — byte-identical (memoization is honest by t)
    let (a0b, _) = compose(&p, t0, &cache);
    assert_eq!(a0, a0b);
    // editing a curve changes fx_hash: the same cache, but a different frame (not stale)
    let p2 = project(base, curve(20.0));
    let (a1c, _) = compose(&p2, t1, &cache);
    assert_ne!(a1, a1c, "editing a keyframe must invalidate frames");
}

/// Test 230 (R.34 K2, revised in R.62): CamMotion is converted into a “Geometry”
/// node with curves and draws the SAME within the resampling tolerance. Byte-for-byte is
/// unreachable by construction: the node stores the POSITION (`pan_to_pos(pan, zoom)`)
/// and places the layer with a continuous affine, while the legacy interpolated pan and zoom
/// separately, multiplied them and cut an INTEGER frame. The owner: “backward
/// compatibility is not needed”, Pan is abolished. Measured on 64×36: mean
/// divergence < 3 (a single edge pixel up to 90) — tolerance `mean < 4`;
/// at zero (pan 0 or zoom 1) — byte for byte. It is checked by rendering: the migration
/// touches the key domain, the storage form and `time_scale`.
#[test]
fn t230_cammotion_migrates_to_pan_curves() {
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let keys = vec![
        CamKeyframe { t: 0.0, pan_x: -0.2, pan_y: 0.1, zoom: 1.0 },
        CamKeyframe { t: 0.5, pan_x: 0.0, pan_y: 0.0, zoom: 1.3 },
        CamKeyframe { t: 1.0, pan_x: 0.25, pan_y: -0.15, zoom: 1.6 },
    ];
    for (label, scale) in [("scale=1", 1.0f64), ("scale=2", 2.0)] {
        let cam = FxInstance::CamMotion {
            ease: true,
            mode: PanMode::Fit,
            time_scale: scale,
            keyframes: keys.clone(),
        };
        let before = project(vec![cam], vec![]);
        let mut after = before.clone();
        assert!(
            kt_model::migrate_legacy_cammotion(&mut after.clips[0]),
            "{label}: the migration must fire"
        );
        assert!(
            matches!(after.clips[0].fx[0], FxInstance::Geometry { .. }),
            "{label}: CamMotion must become a “Geometry” node"
        );
        assert_eq!(after.clips[0].fx_auto[0].len(), 3, "{label}: three curves");
        after.validate().unwrap();
        // times ON the frame GRID (step 3000): the old path read the raw
        // progress of the clip, the new one — the aligned position of the material
        for t in [Tick(0), Tick(21_000), Tick(45_000), Tick(69_000)] {
            let (a, ca) = compose(&before, t, &cache);
            let (b, cb) = compose(&after, t, &cache);
            assert!(ca && cb, "{label}: frames are warm");
            if t.0 == 0 {
                assert_eq!(a, b, "{label}: at zero (zoom 1) the frame must match byte for byte");
            } else {
                let sum: u64 = a.iter().zip(b.iter()).map(|(x, y)| x.abs_diff(*y) as u64).sum();
                let mean = sum as f64 / a.len() as f64;
                assert!(mean < 4.0, "{label}: the frame at {t:?} diverges by {mean:.2} > 4");
            }
        }
    }

    // EMPTY keyframes on CamMotion meant not a static but the v1 preset
    // “zoom 1.0 → 1.35”: the motion must survive the migration
    let empty = FxInstance::CamMotion {
        ease: true,
        mode: PanMode::Fit,
        time_scale: 1.0,
        keyframes: vec![],
    };
    let before = project(vec![empty], vec![]);
    let mut after = before.clone();
    assert!(kt_model::migrate_legacy_cammotion(&mut after.clips[0]));
    let zoom = after.clips[0].fx_auto[0].iter().find(|c| c.param == "zoom").expect("push-in curve");
    assert_eq!(zoom.keys.len(), 2);
    assert!((zoom.keys[1].v - 1.35).abs() < 1e-9, "v1 preset saved: {:?}", zoom.keys);
    for t in [Tick(0), Tick(SPAN / 2), Tick(69_000)] {
        let (a, _) = compose(&before, t, &cache);
        let (b, _) = compose(&after, t, &cache);
        assert_eq!(a, b, "preset without keyframes: the frame at {t:?} matches");
    }

    // the curve of the NEW domain takes precedence over the legacy keyframes
    let cam = FxInstance::CamMotion {
        ease: true,
        mode: PanMode::Fit,
        time_scale: 1.0,
        keyframes: keys.clone(),
    };
    let mut c = project(vec![cam], vec![]).clips[0].clone();
    c.fx_auto = vec![vec![ParamCurve {
        param: "zoom".into(),

        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 4.0)]),
        source: Default::default(),
    }]];
    kt_model::migrate_legacy_cammotion(&mut c);
    let zoom = c.fx_auto[0].iter().find(|c| c.param == "zoom").unwrap();
    assert_eq!(zoom.keys.len(), 1, "the migration does not touch an existing curve");
    assert_eq!(zoom.keys[0].v, 4.0);
}
