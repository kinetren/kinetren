//! Test 137 (item 137, R.24): video dissolve by transitions.
//!
//! A pair of clips in the transition window is blended linearly (integer k/4096) into
//! one layer; the missing half — the full weight of the present one +
//! complete=false; the handles continue the source linearly (reversed — holds
//! the clamp frame). Superset of the t66 harness: frames — a solid color from pts.

use kt_engine::VideoRenderer;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

fn project_pair() -> Project {
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
    for id in [1u64, 2] {
        p.media.push(MediaRef::new(MediaId(id), format!("synth:{id}")));
    }
    for (id, start, src_in, media) in [(1u64, 0i64, 0i64, 1u64), (2, 90_000, 30_000, 2)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(id),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(start),
            duration: Tick(90_000),
            source_in: Tick(src_in),
            media: MediaId(media),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    p
}

fn warm(vr: &VideoRenderer, cache: &kt_cache::BurstCache, t: Tick, q: kt_media::Quality) {
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

fn synth_frame(src_pts: Tick, q: kt_media::Quality) -> Vec<u8> {
    use kt_media::MediaSource;
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    synth.read_video_frame(src_pts, Some(q)).unwrap().rgba.into_vec()
}

fn dissolve(a: &[u8], b: &[u8], k: u32) -> Vec<u8> {
    let ik = 4096 - k;
    a.iter()
        .zip(b)
        .map(|(&av, &bv)| ((av as u32 * ik + bv as u32 * k + 2048) >> 12) as u8)
        .collect()
}

#[test]
fn t137_dissolve_blends_pair_and_is_deterministic() {
    let p = project_pair();
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let (w0, w1) = (81_000i64, 99_000i64);

    for t in [78_000i64, 81_000, 90_000, 96_000, 99_000] {
        let t = Tick(t);
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "t {t:?}: all layers warmed");
        let a = synth_frame(vr.grid.align_floor(Tick(t.0)), q); // A: src = t (src_in 0)
        let b = synth_frame(vr.grid.align_floor(Tick(30_000 + (t.0 - 90_000))), q);
        let expect = if t.0 < w0 {
            a
        } else if t.0 < w1 {
            dissolve(&a, &b, (((t.0 - w0) * 4096) / (w1 - w0)).clamp(0, 4096) as u32)
        } else {
            b
        };
        assert_eq!(out, expect, "t {t:?}: dissolve math");
        // determinism: a repeated compose is byte-identical
        let (again, _) = vr.compose_from_cache(&cache, t, q);
        assert_eq!(out, again);
    }
}

#[test]
fn t137_missing_pair_half_gets_full_weight_and_incomplete() {
    use kt_media::MediaSource;
    let p = project_pair();
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000);
    // warming ONLY layer A (media 1)
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    let frame = synth.read_video_frame(vr.grid.align_floor(t), Some(q)).unwrap();
    cache.insert_video(
        kt_engine::source_id(MediaId(1)),
        Some(q),
        kt_engine::video_cache_idx(vr.grid.align_floor(t)),
        std::sync::Arc::new(frame),
    );
    let (out, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(!complete, "missing half of the pair");
    assert_eq!(out, synth_frame(vr.grid.align_floor(t), q), "present half at full weight");
}

#[test]
fn t137_layers_include_handles_in_window() {
    let p = project_pair();
    let vr = VideoRenderer::new(&p);
    let q = kt_media::Quality { width: 64, height: 36 };
    // in the transition window BOTH clips are active (the handles), outside — one
    assert_eq!(vr.layers_at(Tick(84_000), q).len(), 2, "handles active inside window");
    assert_eq!(vr.layers_at(Tick(78_000), q).len(), 1);
    assert_eq!(vr.layers_at(Tick(99_000), q).len(), 1);
    // the left handle of B reads the source BEFORE the piece: src = 30000 + (84000-90000)
    let layers = vr.layers_at(Tick(84_000), q);
    assert!(layers.contains(&(MediaId(2), vr.grid.align_floor(Tick(24_000)), q)));
}

#[test]
fn t137_dissolve_survives_fx_chain_on_pair() {
    // FX clip in the pair: post-FX cache + blend; a repeated compose from the cache
    // is byte-identical (inheritance of t66)
    let mut p = project_pair();
    p.clips[0].fx = vec![FxInstance::noise(0.3)];
    p.validate().unwrap();
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000);
    warm(&vr, &cache, t, q);
    let (first, c1) = vr.compose_from_cache(&cache, t, q);
    assert!(c1);
    let (second, c2) = vr.compose_from_cache(&cache, t, q);
    assert!(c2);
    assert_eq!(first, second, "postfx-cached dissolve is byte-stable");
}

// ---- test 156: fade at the edge/Gap (R.25) ----------------------------------------

/// Fade-in: there is no pair BY THE MODEL — the layer goes into transparency (under it
/// the composition's black background), and `complete` is not spoiled: there is nothing to wait for.
#[test]
fn t156_solo_fade_scales_alpha_and_stays_complete() {
    let mut p = project_pair();
    p.transitions.clear();
    p.transitions.push(TransitionEvent {
        id: TransitionId(2),
        a: None,
        b: Some(ClipId(1)),
        in_offset: Tick(0),
        out_offset: Tick(18_000), // fade-in: the first 0.2 s of the clip
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };

    for t in [0i64, 9_000, 17_999, 18_000] {
        let t = Tick(t);
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "t {t:?}: the fade is not a starvation of the frame");
        let src = synth_frame(vr.grid.align_floor(t), q);
        let expect = if t.0 < 18_000 {
            // a layer with alpha k/4096 over the composition's BLACK background
            let k = ((t.0 * 4096) / 18_000).clamp(0, 4096) as u32;
            over_black(&src, k)
        } else {
            src
        };
        assert_eq!(out, expect, "t {t:?}: fade in");
    }
}

/// Fade-out — the same, mirrored: toward the end of the clip the picture fades to black.
#[test]
fn t156_fade_out_goes_dark_at_the_edge() {
    let mut p = project_pair();
    p.clips.retain(|c| c.id == ClipId(1));
    p.transitions.clear();
    p.transitions.push(TransitionEvent {
        id: TransitionId(3),
        a: Some(ClipId(1)),
        b: None,
        in_offset: Tick(18_000),
        out_offset: Tick(0),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };

    let mid = Tick(81_000); // the middle of the window [72000, 90000)
    warm(&vr, &cache, mid, q);
    let (out, complete) = vr.compose_from_cache(&cache, mid, q);
    assert!(complete);
    let src = synth_frame(vr.grid.align_floor(mid), q);
    assert_eq!(out, over_black(&src, 4096 - 2048), "in the middle of a fade out — half brightness");

    // right at the edge of the window the frame has almost faded
    let late = Tick(89_999);
    warm(&vr, &cache, late, q);
    let (out, _) = vr.compose_from_cache(&cache, late, q);
    let mean: f64 = out.chunks_exact(4).map(|p| p[0] as f64).sum::<f64>() / (out.len() / 4) as f64;
    assert!(mean < 1.0, "near the end of the window the fade out goes dark, yet got {mean}");
}

/// A layer with alpha k/4096 over the composition's BLACK transparent background —
/// literally the arithmetic of the engine's scale_alpha_into + alpha_over.
fn over_black(src: &[u8], k: u32) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    for p in 0..src.len() / 4 {
        let i = p * 4;
        let sa = ((255 * k + 2048) >> 12).min(255);
        if sa == 255 {
            out[i..i + 4].copy_from_slice(&src[i..i + 4]);
            continue;
        }
        if sa == 0 {
            continue;
        }
        for ch in 0..3 {
            // the background is zero (dst = 0, da = 0) → only the layer's contribution remains
            out[i + ch] = ((src[i + ch] as u32 * sa) / 255).min(255) as u8;
        }
        out[i + 3] = sa as u8;
    }
    out
}

/// Test 162 (video part): when overlapping on ONE track, the one on top is the one that
/// started LATER — the Vegas rule (the user's decision after R.28:
/// “let's do it as in vegas — on top is the later one”; before that
/// the v1 rule applied: “on top is the one whose start is smaller”).
#[test]
fn t162_overlap_puts_the_later_clip_on_top() {
    let mut p = project_pair();
    p.transitions.clear();
    // clip 2 overlaps clip 1 on its own track
    for c in &mut p.clips {
        if c.id == ClipId(2) {
            c.start = Tick(45_000);
        }
    }
    p.validate().expect("overlap is legal (layering is on)");
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(60_000); // inside the zone of overlap
    warm(&vr, &cache, t, q);
    let (out, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete);
    // on top — clip 2 (it started later): the frame equals ITS source
    // (source_in 30_000, start 45_000 → pts = 30_000 + (t − 45_000))
    let expect = synth_frame(vr.grid.align_floor(Tick(30_000 + (t.0 - 45_000))), q);
    assert_eq!(out, expect, "in the push zone the clip that started later is on top");
    // and this is NOT the frame of the early clip — the rule has really changed
    assert_ne!(out, synth_frame(vr.grid.align_floor(t), q), "the earlier clip is no longer on top");
}

/// Test 174 (item 12 of R.28): the video event's level — the layer's opacity.
///
/// “in sony vegas… you can apparently change the transparency — you hover with the mouse
/// and drag up or down”. The layer is multiplied by the level: under a semi-transparent clip the lower track shows through,
/// and when it is absent — black.
#[test]
fn t174_clip_opacity_scales_the_layer() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(45_000);
    // two layers: lane 0 (top) and lane 1 (bottom), DIFFERENT sources
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
    for id in [1u64, 2] {
        p.media.push(MediaRef::new(MediaId(id), format!("synth:{id}")));
    }
    for (id, lane, media) in [(1u64, 0u32, 1u64), (2, 1, 2)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(id),
            link_group: None,
            kind: ClipKind::Video,
            lane,
            start: Tick(0),
            duration: Tick(90_000),
            source_in: Tick(if id == 1 { 0 } else { 900_000 }),
            media: MediaId(media),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.normalize(); // tracks for the content (R.62)
    p.validate().unwrap();

    let render = |p: &Project| {
        let mut vr = VideoRenderer::new(p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "the layers are warm");
        out
    };

    let opaque = render(&p);
    let mut p0 = p.clone();
    p0.clips[0].opacity = 0.0;
    let transparent = render(&p0);
    let mut p5 = p.clone();
    p5.clips[0].opacity = 0.5;
    let half = render(&p5);

    assert_ne!(opaque, transparent, "level 0 must open the bottom layer");
    // the middle between the layers — per pixel (alpha_over with straight alpha)
    for i in (0..opaque.len()).step_by(4) {
        for ch in 0..3 {
            let (a, b, m) =
                (opaque[i + ch] as i32, transparent[i + ch] as i32, half[i + ch] as i32);
            assert!(
                (m - (a + b) / 2).abs() <= 2,
                "pixel {i} channel {ch}: level 0.5 — the middle {m} between {a} and {b}"
            );
        }
    }
    // level 1.0 changes nothing — the byte-for-byte old frame (GOLDEN intact)
    let mut p1 = p.clone();
    p1.clips[0].opacity = 1.0;
    assert_eq!(render(&p1), opaque, "level 1.0 — identity");
}

/// Test 225 (R.33 C4): the transition kind reaches the PICTURE, not only the file.
/// The frames of the synthetic source are a solid color, so a
/// “wipe” must give exactly two single-tone areas with a sharp boundary, while
/// a crossfade at the same moment gives one mixed color over the whole frame.
#[test]
fn t225_transition_kind_reaches_the_picture() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000); // the middle of the window 81_000..99_000
    let render = |kind: TransitionKind| {
        let mut p = project_pair();
        p.transitions[0].kind = kind.clone();
        p.validate().unwrap();
        let mut vr = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "{kind:?}: the layers are warm");
        // determinism: a repeated pass is byte-for-byte
        let (again, _) = vr.compose_from_cache(&cache, t, q);
        assert_eq!(out, again, "{kind:?}: a repeated composite must match");
        out
    };
    let px = |buf: &[u8], x: usize, y: usize| {
        let i = (y * q.width as usize + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };
    let a = synth_frame(Tick(90_000), q);
    let b = synth_frame(Tick(30_000), q);
    let (ca, cb) = (px(&a, 0, 0), px(&b, 0, 0));
    assert_ne!(ca, cb, "the colours of the halves must differ, otherwise the test is blind");

    // crossfade: a mix EVERYWHERE, neither of the source colors is in the frame
    let cross = render(TransitionKind::Crossfade);
    let mix = px(&cross, 5, 5);
    assert_eq!(mix, px(&cross, 58, 30), "the crossfade is uniform across the frame");
    assert_ne!(mix, ca);
    assert_ne!(mix, cb);

    // wipe left without blur: B opens FROM THE RIGHT, the boundary is sharp
    let wipe = render(TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 });
    assert_eq!(px(&wipe, 2, 18), ca, "on the left the frame still leaving");
    assert_eq!(px(&wipe, 61, 18), cb, "on the right the frame already entering");
    let blended = (0..q.width as usize)
        .filter(|&x| {
            let p = px(&wipe, x, 18);
            p != ca && p != cb
        })
        .count();
    assert!(blended <= 1, "a hard wipe has no blended pixels: {blended}");

    // vertical shutters: symmetric at BOTH edges, the foreign color in the center
    let barn = render(TransitionKind::Barn { vertical: false, softness: 0.0 });
    assert_eq!(px(&barn, 32, 1), ca, "at the upper edge the frame still leaving");
    assert_eq!(px(&barn, 32, 34), ca, "and at the lower one too — the shutters are symmetric");
    assert_eq!(px(&barn, 32, 18), cb, "in the centre the frame already entering");

    // a shift down: the halves swap places vertically
    let push = render(TransitionKind::Push { dir: Dir::Down });
    assert_eq!(px(&push, 32, 30), ca, "the bottom is the frame leaving, pushed down");
    assert_eq!(px(&push, 32, 4), cb, "the entering frame slid in from above");

    // through black: in the middle of the transition the frame fades out entirely
    let dip = render(TransitionKind::Dip { color_r: 0, color_g: 0, color_b: 0 });
    let mid = px(&dip, 32, 18);
    assert!(mid[0] < 4 && mid[1] < 4 && mid[2] < 4, "the middle of the dip — colour: {mid:?}");
    assert_eq!(mid[3], 255, "alpha is not lost during the dip");
}

/// Test 225b: a solo fade (the track edge) with a kind — the shape lives in the ALPHA,
/// not in uniform transparency. The crossfade-fade stays as before.
#[test]
fn t225_solo_fade_with_kind_shapes_alpha() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let render = |kind: TransitionKind, t: Tick| {
        let mut p = project_pair();
        // we remove the pair: clip 1 keeps a fade-out into the void
        p.transitions.clear();
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(1)),
            b: None,
            in_offset: Tick(18_000),
            out_offset: Tick(0),
            kind,
            audio: Default::default(),
            auto: vec![],
            invert: false,
        });
        p.clips.retain(|c| c.id == ClipId(1));
        p.validate().unwrap();
        let mut vr = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, _) = vr.compose_from_cache(&cache, t, q);
        out
    };
    let alpha = |buf: &[u8], x: usize, y: usize| buf[(y * q.width as usize + x) * 4 + 3];
    let t = Tick(81_000); // the middle of the window 72_000..90_000

    // crossfade-fade: the alpha is ONE for the whole frame (the old behavior)
    let cross = render(TransitionKind::Crossfade, t);
    assert_eq!(alpha(&cross, 2, 18), alpha(&cross, 61, 18), "the fade is uniform");

    // wipe-fade: one side is still opaque, the other is already empty
    let wipe = render(TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 }, t);
    let (l, r) = (alpha(&wipe, 2, 18), alpha(&wipe, 61, 18));
    assert!(l > 250 && r < 5, "the wipe fade must cut the frame by alpha: {l}/{r}");
}

/// Test 232 (R.34 K4): the transition kind's parameters are animated by the same
/// mechanism as effects. Keys live in ticks FROM THE START of the window;
/// the kind is resolved at every moment, so the picture must change
/// not only with the transition's progress, but also with the curve.
#[test]
fn t232_transition_params_animate() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let render = |auto: Vec<ParamCurve>, t: Tick| {
        let mut p = project_pair();
        // a wipe without blur: the edge angle is visible from the shape of the boundary
        p.transitions[0].kind = TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 };
        p.transitions[0].auto = auto;
        p.validate().unwrap();
        let mut vr = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "the layers are warm");
        out
    };
    // the transition window — [81_000, 99_000): the angle curve from 0° to 80°
    let curve = vec![ParamCurve {
        param: "angle".into(),

        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(18_000), 80.0)]),
        source: Default::default(),
    }];
    let t = Tick(96_000); // closer to the end of the window the angle is already large
    let plain = render(vec![], t);
    let animated = render(curve.clone(), t);
    assert_ne!(plain, animated, "the corner curve must change the picture");

    // at the VERY start of the window the curve is still at zero — the frame is as without it
    let t0 = Tick(81_000);
    assert_eq!(
        render(vec![], t0),
        render(curve.clone(), t0),
        "at the start of the window the curve value equals the static one"
    );

    // a key outside the window is legal (hidden by the edge edit), the load does not fail
    let mut p = project_pair();
    p.transitions[0].kind = TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 };
    p.transitions[0].auto = vec![ParamCurve {
        param: "angle".into(),

        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(900_000), 80.0)]),
        source: Default::default(),
    }];
    p.validate().expect("a key past the edge of the window is a legal hidden key");

    // a curve on a foreign parameter — a validation refusal, not silence
    p.transitions[0].auto[0].param = "no_such".into();
    assert!(p.validate().is_err(), "an orphan curve must fail the load");
}

/// Test 459 (R.51, item 396): for directional kernels the expectation is computed by a
/// FORMULA, not by a second instance of the same formula. The frames are linear
/// ramps (R along X, G along Y, B — the source marker): bilinear on a ramp at an
/// integer shift degenerates into the exact value, and the “incoming” part
/// must show the REAL rows/columns of frame B, not a blurred
/// edge. This very blindness was had by t224 (mirror against mirror) and
/// t225 (solid colors): push/whip sampled B with the shift −dir
/// instead of +dir, and the edge clamp turned B into a strip of its first
/// row/column — and both tests stayed green.
#[test]
fn t459_push_whip_bring_real_b_content() {
    use kt_engine::transitions::{TransMix, mix_into};
    const W: usize = 64;
    const H: usize = 36;
    let ramp = |marker: u8| -> kt_media::VideoFrame {
        let mut rgba = vec![0u8; W * H * 4];
        for y in 0..H {
            for x in 0..W {
                let i = (y * W + x) * 4;
                rgba[i] = (x * 4) as u8; // R: a ramp along X (0..252)
                rgba[i + 1] = (y * 7) as u8; // G: a ramp along Y (0..245)
                rgba[i + 2] = marker; // B: whose frame
                rgba[i + 3] = 255;
            }
        }
        kt_media::VideoFrame {
            pts: Tick(0),
            width: W as u32,
            height: H as u32,
            rgba: rgba.into_boxed_slice(),
            has_alpha: false,
        }
    };
    let a = ramp(50);
    let b = ramp(200);

    // The push geometry: the screen shows the reel [A|B], shifted by dir·k.
    // The texel (x,y) takes material at (x,y) − shift; outside the frame — frame B,
    // carried by a whole screen IN THE DIRECTION of motion.
    let expect = |x: i32, y: i32, dx: i32, dy: i32, shift_x: i32, shift_y: i32| -> [u8; 3] {
        let (mx, my) = (x - shift_x, y - shift_y);
        let inside = (0..W as i32).contains(&mx) && (0..H as i32).contains(&my);
        let (sx, sy, marker) =
            if inside { (mx, my, 50) } else { (mx + dx * W as i32, my + dy * H as i32, 200) };
        [(sx * 4) as u8, (sy * 7) as u8, marker]
    };

    let mut dst = Vec::new();
    for dir in [Dir::Left, Dir::Right, Dir::Up, Dir::Down] {
        let (dx, dy) = dir.vec();
        let (dx, dy) = (dx as i32, dy as i32);
        for k in [1024u32, 2048] {
            // the shift at these k is integer both in X (64·k/4096) and in Y (36·k/4096)
            let kk = k as f32 / 4096.0;
            let (shift_x, shift_y) =
                ((dx as f32 * kk * W as f32) as i32, (dy as f32 * kk * H as f32) as i32);
            let m = TransMix::of(&TransitionKind::Push { dir });
            mix_into(&mut dst, Some(&a), Some(&b), k, m, W, H);
            for y in 0..H as i32 {
                for x in 0..W as i32 {
                    let i = ((y * W as i32 + x) * 4) as usize;
                    let want = expect(x, y, dx, dy, shift_x, shift_y);
                    for c in 0..3 {
                        let got = dst[i + c] as i32;
                        assert!(
                            (got - want[c] as i32).abs() <= 1,
                            "push {dir:?} k={k}: pixel ({x},{y}) channel {c}: \
                             {got}, but the material there is {}",
                            want[c]
                        );
                    }
                }
            }
            // whip with zero smear is exactly push: 7 taps at one point
            let mw = TransMix::of(&TransitionKind::Whip { dir, blur: 0.0 });
            let mut dw = Vec::new();
            mix_into(&mut dw, Some(&a), Some(&b), k, mw, W, H);
            assert_eq!(dw, dst, "whip {dir:?} blur=0 k={k} must match push byte for byte");
        }
    }
}

/// Test 459b: the same geometry arrives through the PLAN (VideoRenderer), not
/// just through the bare kernel — the cache is warmed with ramp frames, and the
/// incoming half must show the middle of frame B.
#[test]
fn t459b_push_through_renderer_shows_real_b() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let ramp_rgba = |marker: u8| -> Vec<u8> {
        let (w, h) = (64usize, 36usize);
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                rgba[i] = (x * 4) as u8;
                rgba[i + 1] = (y * 7) as u8;
                rgba[i + 2] = marker;
                rgba[i + 3] = 255;
            }
        }
        rgba
    };
    let mut p = project_pair();
    p.transitions[0].kind = TransitionKind::Push { dir: Dir::Down };
    p.validate().unwrap();
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(90_000); // the middle of the window: the shift is exactly half a frame
    for (media, src_pts, fq) in vr.layers_at(t, q) {
        let marker =
            if kt_engine::source_id(media) == kt_engine::source_id(MediaId(1)) { 50 } else { 200 };
        let frame = kt_media::VideoFrame {
            pts: src_pts,
            width: fq.width,
            height: fq.height,
            rgba: ramp_rgba(marker).into_boxed_slice(),
            has_alpha: false,
        };
        cache.insert_video(
            kt_engine::source_id(media),
            Some(fq),
            kt_engine::video_cache_idx(src_pts),
            std::sync::Arc::new(frame),
        );
    }
    let (out, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete, "the layers are warm");
    let px = |x: usize, y: usize| {
        let i = (y * 64 + x) * 4;
        [out[i], out[i + 1], out[i + 2]]
    };
    // push Down, k=0.5: the upper half of the screen — the LOWER half of frame B
    assert_eq!(px(32, 4), [128, (4 + 18) * 7, 200], "the real B slid in, not the edge");
    // the lower half — the top of frame A, moved down
    assert_eq!(px(32, 30), [128, (30 - 18) * 7, 50], "A moves down with its top");
}

/// Test 461 (item 398, R.51): transition inversion — “the gesture backwards” —
/// executed by the plan as a swap of sides plus the reverse progress.
/// The crossfade is the identity (the integer path is untouched); for directional kinds
/// the inversion coincides with the opposite direction; for a solo fade
/// the shape is cut from the other side.
#[test]
fn t461_transition_invert() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000); // the middle of the window, k = 0.5
    let render = |kind: TransitionKind, invert: bool| {
        let mut p = project_pair();
        p.transitions[0].kind = kind;
        p.transitions[0].invert = invert;
        p.validate().unwrap();
        let mut vr = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "the layers are warm");
        out
    };
    let px = |buf: &[u8], x: usize, y: usize| {
        let i = (y * q.width as usize + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };
    let a = synth_frame(Tick(90_000), q);
    let b = synth_frame(Tick(30_000), q);
    let (ca, cb) = (px(&a, 0, 0), px(&b, 0, 0));
    assert_ne!(ca, cb, "the colours of the sides must differ, otherwise the test is blind");

    // crossfade: the inversion is the identity, byte-for-byte (the GOLDEN path untouched)
    assert_eq!(
        render(TransitionKind::Crossfade, false),
        render(TransitionKind::Crossfade, true),
        "the inversion of the crossfade must be an identity"
    );

    // wipe: the inversion swaps the sides at the same boundary
    let wipe =
        |inv| render(TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 }, inv);
    let (w0, w1) = (wipe(false), wipe(true));
    assert_eq!(px(&w0, 2, 18), ca, "direct wipe: the frame leaving on the left");
    assert_eq!(px(&w0, 61, 18), cb, "direct wipe: the frame entering on the right");
    assert_eq!(px(&w1, 2, 18), cb, "inverted wipe: the frame already entering on the left");
    assert_eq!(px(&w1, 61, 18), ca, "inverted wipe: the frame still leaving on the right");

    // push: the inversion of a directional kind == the opposite direction
    assert_eq!(
        render(TransitionKind::Push { dir: Dir::Right }, true),
        render(TransitionKind::Push { dir: Dir::Left }, false),
        "the inverted push to the right must match the push to the left"
    );

    // solo fade: the shape is cut from the other side
    let solo = |inv: bool| {
        let mut p = project_pair();
        p.transitions.clear();
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(1)),
            b: None,
            in_offset: Tick(18_000),
            out_offset: Tick(0),
            kind: TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 },
            audio: Default::default(),
            auto: vec![],
            invert: inv,
        });
        p.clips.retain(|c| c.id == ClipId(1));
        p.validate().unwrap();
        let mut vr = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        let t = Tick(81_000); // the middle of the fade window 72_000..90_000
        warm(&vr, &cache, t, q);
        let (out, _) = vr.compose_from_cache(&cache, t, q);
        out
    };
    let alpha = |buf: &[u8], x: usize| buf[(18 * q.width as usize + x) * 4 + 3];
    let (s0, s1) = (solo(false), solo(true));
    assert!(
        alpha(&s0, 2) > 250 && alpha(&s0, 61) < 5,
        "the direct wipe fade cuts from the right: {}/{}",
        alpha(&s0, 2),
        alpha(&s0, 61)
    );
    assert!(
        alpha(&s1, 2) < 5 && alpha(&s1, 61) > 250,
        "the inverted wipe fade cuts from the left: {}/{}",
        alpha(&s1, 2),
        alpha(&s1, 61)
    );
}

/// Test 464 (item 399, R.51): the picture on an OVERLAP. Without a transition — the old
/// hard cut of Vegas (the later one on top is opaque); with a transition the pair
/// collapses into a Dissolve, and the kind reaches the frame.
#[test]
fn t464_overlap_transition_reaches_picture() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let mk = |with: Option<TransitionKind>| {
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
        for id in [1u64, 2] {
            p.media.push(MediaRef::new(MediaId(id), format!("synth:{id}")));
        }
        // staircase: [0..120k) and [60k..180k), overlap [60k..120k)
        for (id, start, media) in [(1u64, 0i64, 1u64), (2, 60_000, 2)] {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id: ClipId(id),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(start),
                duration: Tick(120_000),
                source_in: Tick::ZERO,
                media: MediaId(media),
                gain_db: 0.0,
                reversed: false,
                fx: vec![],
                fx_bypass: vec![],
                fx_auto: vec![],
                opacity: 1.0,
                fx_off: false,
            });
        }
        if let Some(kind) = with {
            p.transitions.push(TransitionEvent {
                id: TransitionId(1),
                a: Some(ClipId(1)),
                b: Some(ClipId(2)),
                in_offset: Tick::ZERO,
                out_offset: Tick::ZERO,
                kind,
                audio: Default::default(),
                auto: vec![],
                invert: false,
            });
        }
        p.validate().unwrap();
        p
    };
    let render = |p: &Project, t: Tick| {
        let mut vr = VideoRenderer::new(p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "the layers are warm");
        out
    };
    let px = |buf: &[u8], x: usize, y: usize| {
        let i = (y * q.width as usize + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };
    let t = Tick(90_000); // the middle of the overlap
    let a = synth_frame(Tick(90_000), q);
    let b = synth_frame(Tick(30_000), q);
    let (ca, cb) = (px(&a, 0, 0), px(&b, 0, 0));
    assert_ne!(ca, cb, "the colours of the sides are distinguishable");

    // without a transition: a hard cut — the whole frame shows the LATER clip
    let bare = render(&mk(None), t);
    assert_eq!(px(&bare, 2, 18), cb, "the later one on top is opaque (the Vegas rule)");
    assert_eq!(px(&bare, 61, 18), cb);

    // with a wipe: both sides are in the frame, the boundary in the middle of the window
    let wipe =
        render(&mk(Some(TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 })), t);
    assert_eq!(px(&wipe, 2, 18), ca, "on the left the frame still leaving");
    assert_eq!(px(&wipe, 61, 18), cb, "on the right the frame already entering");

    // crossfade: a uniform mix over the frame
    let cross = render(&mk(Some(TransitionKind::Crossfade)), t);
    let mix = px(&cross, 5, 5);
    assert_eq!(mix, px(&cross, 58, 30), "the crossfade is uniform");
    assert_ne!(mix, ca);
    assert_ne!(mix, cb);
}

/// Test 467 (item 400, R.51): the new 2D kinds reach the picture —
/// analytic assertions of zones, not “something changed”.
#[test]
fn t467_new_kind_families_reach_picture() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000); // the middle of the window, k = 0.5
    let render = |kind: TransitionKind| {
        let mut p = project_pair();
        p.transitions[0].kind = kind;
        p.validate().unwrap();
        let mut vr = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "the layers are warm");
        out
    };
    let px = |buf: &[u8], x: usize, y: usize| {
        let i = (y * q.width as usize + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };
    let a = synth_frame(Tick(90_000), q);
    let b = synth_frame(Tick(30_000), q);
    let (ca, cb) = (px(&a, 0, 0), px(&b, 0, 0));
    assert_ne!(ca, cb);

    // chessboard 8×6 without softness: at k=0.5 the even cell has its LEFT
    // half open, the odd one its RIGHT half (counter-waves)
    let ch = render(TransitionKind::Checker { cols: 8, rows: 6, softness: 0.0 });
    // cell (0,0) even: columns 0..8, the open half 0..4
    assert_eq!(px(&ch, 1, 3), cb, "the even cell opens from the left");
    assert_eq!(px(&ch, 6, 3), ca, "the right half of the even one is still leaving");
    // cell (1,0) odd: columns 8..16, the open half 12..16
    assert_eq!(px(&ch, 14, 3), cb, "the odd cell opens from the right");
    assert_eq!(px(&ch, 9, 3), ca);

    // the split: the halves of A moved apart, in the gap [16..48) B is open
    let sp = render(TransitionKind::Split { vertical: false });
    assert_eq!(px(&sp, 8, 18), ca, "the left half of A is still on screen");
    assert_eq!(px(&sp, 32, 18), cb, "in the gap the frame already entering");
    assert_eq!(px(&sp, 56, 18), ca, "the right half of A");

    // stretching: in the band |u−0.5|<k/2 — B, outside A is untouched
    let st = render(TransitionKind::Stretch { vertical: false });
    assert_eq!(px(&st, 32, 18), cb, "in the strip — the entering frame");
    assert_eq!(px(&st, 4, 18), ca, "outside — the leaving frame as it was");
    assert_eq!(px(&st, 60, 18), ca);

    // squeeze to the right: c<k — B unfolded, beyond — A squeezed
    let sq = render(TransitionKind::Squeeze { dir: Dir::Right });
    assert_eq!(px(&sq, 8, 18), cb, "the near part — the entering frame");
    assert_eq!(px(&sq, 56, 18), ca, "the far one — the compressed leaving frame");

    // radial smear: k=0.5 — the side is already B, the ray's center is motionless
    let zb = render(TransitionKind::ZoomBlur { amount: 1.0, cx: 0.5, cy: 0.5 });
    assert_eq!(px(&zb, 32, 18), cb, "the centre of the rays is fixed and shows the side");

    // blinds, 8 strips without softness: at k=0.5 the left half of each is open
    let bl = render(TransitionKind::Blinds { count: 8, angle: 0.0, softness: 0.0 });
    assert_eq!(px(&bl, 1, 18), cb, "the start of the strip is open");
    assert_eq!(px(&bl, 6, 18), ca, "the tail of the strip is still leaving");
    assert_eq!(px(&bl, 9, 18), cb, "the second strip — the same");

    // random blocks: without softness each cell is entirely A or B
    let bk = render(TransitionKind::Blocks { cols: 16, rows: 9, softness: 0.0 });
    let mut seen_a = 0;
    let mut seen_b = 0;
    for j in 0..9 {
        for i in 0..16 {
            let c = px(&bk, i * 4 + 2, j * 4 + 2);
            if c == ca {
                seen_a += 1;
            } else if c == cb {
                seen_b += 1;
            } else {
                panic!("cell ({i},{j}) is neither A nor B: {c:?}");
            }
        }
    }
    assert!(seen_a > 20 && seen_b > 20, "both sides in the mosaic: {seen_a}/{seen_b}");

    // exchange: at the middle both sides are compressed — at the edges the VOID
    // (transparency) is visible; both pictures are in the frame with a depth shadow
    let sw = render(TransitionKind::Swap { vertical: false, depth: 0.4 });
    assert_eq!(px(&sw, 1, 1)[3], 0, "the corner is empty — the quads moved away and shrank");
    let has = |want: [u8; 4], shade: f32| {
        let dim = |c: [u8; 4]| {
            [
                (c[0] as f32 * shade) as i32,
                (c[1] as f32 * shade) as i32,
                (c[2] as f32 * shade) as i32,
            ]
        };
        let w = dim(want);
        (0..36).any(|y| {
            (0..64).any(|x| {
                let g = px(&sw, x, y);
                g[3] > 250
                    && (g[0] as i32 - w[0]).abs() <= 2
                    && (g[1] as i32 - w[1]).abs() <= 2
                    && (g[2] as i32 - w[2]).abs() <= 2
            })
        })
    };
    assert!(has(ca, 0.75) && has(cb, 0.75), "both sides of the exchange in the frame with shadow 0.75");
}

/// Test 468 (item 401, R.51): the volumetric kinds reach the picture — the zones and
/// transparency behind the panels are computed analytically.
#[test]
fn t468_volume_kinds_reach_picture() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let render = |kind: TransitionKind, t: Tick| {
        let mut p = project_pair();
        p.transitions[0].kind = kind;
        p.validate().unwrap();
        let mut vr = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "the layers are warm");
        out
    };
    let px = |buf: &[u8], x: usize, y: usize| {
        let i = (y * q.width as usize + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };
    let near = |got: [u8; 4], want: [u8; 4], what: &str| {
        for c in 0..3 {
            assert!((got[c] as i32 - want[c] as i32).abs() <= 3, "{what}: {got:?} vs {want:?}");
        }
        assert!(got[3] > 250, "{what}: opaque");
    };
    let dim = |c: [u8; 4], sh: f32| {
        [
            (c[0] as f32 * sh).round() as u8,
            (c[1] as f32 * sh).round() as u8,
            (c[2] as f32 * sh).round() as u8,
            c[3],
        ]
    };
    let a = synth_frame(Tick(90_000), q);
    let b = synth_frame(Tick(30_000), q);
    let (ca, cb) = (px(&a, 0, 0), px(&b, 0, 0));
    assert_ne!(ca, cb);
    let (t0, tm, t1) = (Tick(81_300), Tick(90_000), Tick(98_700));
    // the color of the synthetics depends on pts: at the edges of the window the frames of the sides are DIFFERENT
    let ca0 = px(&synth_frame(Tick(81_000), q), 0, 0);
    let cb1 = px(&synth_frame(Tick(36_000), q), 0, 0);

    // flip: the start — almost entirely A, the middle — the panel edge-on (the frame
    // transparent at the edges), the end — almost entirely B
    let f = TransitionKind::Flip { vertical: false, persp: 0.5, shade: 0.0 };
    near(px(&render(f.clone(), t0), 32, 18), ca0, "flip@start: the centre — the frame leaving");
    let mid = render(f.clone(), tm);
    assert_eq!(px(&mid, 4, 18)[3], 0, "flip@middle: behind the edge of the panel is empty");
    assert_eq!(px(&mid, 60, 18)[3], 0);
    near(px(&render(f, t1), 32, 18), cb1, "flip@end: the centre — the frame entering");

    // door: at the middle of the leaves, the edges show the darkened A, the center — B
    let d = TransitionKind::Door { vertical: false, persp: 0.5, shade: 0.5 };
    let dm = render(d, tm);
    near(px(&dm, 32, 18), cb, "door@middle: in the opening the frame entering");
    let sh_door = 1.0 - 0.5 * (std::f32::consts::FRAC_PI_4 - 5e-4).sin();
    near(px(&dm, 2, 18), dim(ca, sh_door), "door@middle: the left leaf is darkened");

    // cube: the middle — on the left a darkened outgoing, on the right a darkened incoming
    let c = TransitionKind::Cube { dir: Dir::Left, persp: 0.5, shade: 0.5 };
    let cm = render(c, tm);
    let sh_cube = 1.0 - 0.5 * 0.5;
    near(px(&cm, 6, 18), dim(ca, sh_cube), "cube@middle: on the left the face of the leaving frame");
    near(px(&cm, 58, 18), dim(cb, sh_cube), "cube@middle: on the right the face of the entering frame");

    // peel to the left: at the start of motion (on the right of the screen) — the reverse side of the sheet,
    // at the far edge — already the clean incoming
    let p1 = TransitionKind::PagePeel { dir: Dir::Left, curl: 0.18, shadow: 0.5 };
    let pm = render(p1, tm);
    near(px(&pm, 61, 18), cb, "peel@middle: the uncovered part — the entering frame without shadow");
    near(px(&pm, 3, 18), dim(ca, 1.0 - 0.5 * 0.35), "peel@middle: the reverse side of the leaf");

    // roll to the right: the tail of the sheet still lies untouched, the start — the incoming
    let r1 = TransitionKind::PageRoll { dir: Dir::Right, curl: 0.18, shadow: 0.5 };
    let rm = render(r1, tm);
    near(px(&rm, 2, 18), cb, "roll@middle: what has passed — the entering frame");
    near(px(&rm, 61, 18), ca, "roll@middle: the tail of the leaf still lies");
}

/// Test 521 (R.53, item 417): a layer with an ALPHA under a geo chain shows through.
///
/// This is the very symptom the alpha rule was fixed for: a title (and any
/// green-screen output) with an `EventGrid` in the chain landed on the lower
/// track as an opaque rectangle. The test is independent of kt-fx: it probes
/// the COMPOSITE — what the person sees in the preview.
#[test]
fn t521_alpha_layer_under_a_chain_shows_through() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(45_000);
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
    for id in [1u64, 2] {
        p.media.push(MediaRef::new(MediaId(id), format!("synth:{id}")));
    }
    for (id, lane, media) in [(1u64, 0u32, 1u64), (2, 1, 2)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(id),
            link_group: None,
            kind: ClipKind::Video,
            lane,
            start: Tick(0),
            duration: Tick(90_000),
            source_in: Tick(if id == 1 { 0 } else { 900_000 }),
            media: MediaId(media),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    // the key — the upper layer's OWN color: the green screen knocks it out
    // through, further the grid (the synthetic frames are a solid color from pts)
    let top = synth_frame(t, q);
    p.clips[0].fx = vec![
        FxInstance::Greenscreen {
            key_r: top[0],
            key_g: top[1],
            key_b: top[2],
            tolerance: 8,
            softness: 0,
            spill: 0.0,
        },
        FxInstance::EventGrid { cols: 2, rows: 2, center: false },
    ];
    p.clips[0].fx_bypass = vec![false; 2];
    p.clips[0].fx_auto = vec![vec![]; 2];
    p.normalize(); // tracks for the content (R.62)
    p.validate().unwrap();

    let render = |p: &Project| {
        let mut vr = VideoRenderer::new(p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "the layers are warm");
        out
    };

    let with_top = render(&p);
    // the same edit without the upper layer — the reference “only the lower is visible”
    let mut only_bottom = p.clone();
    only_bottom.clips.remove(0);
    only_bottom.validate().unwrap();
    let bottom = render(&only_bottom);

    assert_eq!(
        with_top, bottom,
        "the top layer was keyed out entirely by the green screen, but landed as an OPAQUE slab: \
         the grid in the chain declared the alpha lost"
    );
}
