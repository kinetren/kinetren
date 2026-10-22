//! Tests 310-317 (R.46, items 320-322): DECODE SCALE — the pre-FX frame is taken
//! larger than Quality when the chain enlarges and the file has native headroom.
//!
//! The three multipliers of the scale are checked separately (t310: the formula), the
//! zoom-maximum sampler over the clip — separately (t311), and the main consequence —
//! the prefetch order and the composite read must converge TOGETHER WITH THE SIZE (t312):
//! if they diverge — a permanent cache miss, the same disease that t247d cured along the
//! frame axis.

use kt_engine::{VideoRenderer, video_cache_idx};
use kt_media::synthetic::CheckerSource;
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::{Tick, TickRange};

const SEC: i64 = 90_000;
const STEP: i64 = SEC / 30;

fn project(w: u32, h: u32, fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: w,
        height: h,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "checker"));
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
        duration: Tick(2 * SEC),
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

fn natives(w: u32, h: u32) -> std::collections::HashMap<MediaId, (u32, u32)> {
    std::collections::HashMap::from([(MediaId(1), (w, h))])
}

fn zoom(z: f64) -> FxInstance {
    FxInstance::geometry_zoom(z)
}

/// Decode quality of the clip (the only project layer) at the given Quality.
fn dq(p: &Project, nat: Option<(u32, u32)>, q: Quality) -> Quality {
    let vr = match nat {
        Some((w, h)) => VideoRenderer::new_with_natives(p, &natives(w, h)),
        None => VideoRenderer::new(p),
    };
    vr.layers_at(Tick(STEP), q).first().expect("layer is present").2
}

/// Test 310 (item 320): the formula of the scale. How much the chain asks for (tap quantum),
/// how much the file can give (native headroom, rounding UP — the owner's decision)
/// and the ceiling of intermediate buffers.
#[test]
fn t310_decode_stage_formula() {
    let q720 = Quality { width: 1280, height: 720 };
    let p2 = project(1280, 720, vec![zoom(2.0)], vec![]);
    // a FRACTIONAL headroom: 1920/1280 = 1.5 → scale 2, the remainder is covered by sws
    assert_eq!(dq(&p2, Some((1920, 1080)), q720), Quality { width: 2560, height: 1440 });
    // the full headroom is exactly enough
    assert_eq!(dq(&p2, Some((2560, 1440)), q720), Quality { width: 2560, height: 1440 });
    // no headroom: the file is not larger than Quality — decode as before
    assert_eq!(dq(&p2, Some((1280, 720)), q720), q720);
    assert_eq!(dq(&p2, Some((640, 360)), q720), q720, "finer than Quality is NEVER decoded");
    // native unknown (stub, synthetic) — no scale
    assert_eq!(dq(&p2, None, q720), q720);
    // ceiling of intermediate buffers: the 4K output does not get a scale
    let q4k = Quality { width: 3840, height: 2160 };
    let p4 = project(3840, 2160, vec![zoom(2.0)], vec![]);
    assert_eq!(dq(&p4, Some((7680, 4320)), q4k), q4k, "MAX_INTER_DIM cuts the step");
    // the chain does not enlarge — no scale at any native
    for fx in [
        vec![],
        vec![zoom(1.0)],
        vec![zoom(0.5)],
        vec![FxInstance::Mirror { horizontal: true, vertical: false }],
        vec![FxInstance::EventGrid { cols: 2, rows: 2, center: true }],
        // net unit (R.44 scenario): the grid compressed, the zoom restored it
        vec![FxInstance::EventGrid { cols: 2, rows: 2, center: true }, zoom(2.0)],
    ] {
        let p = project(1280, 720, fx.clone(), vec![]);
        assert_eq!(dq(&p, Some((3840, 2160)), q720), q720, "net zoom <= 1: {fx:?}");
    }
    // a zoom just above unity is damped by the quantum, a large one runs into the tap ceiling
    let p_small = project(1280, 720, vec![zoom(1.01)], vec![]);
    assert_eq!(dq(&p_small, Some((3840, 2160)), q720), q720, "1/32 quantum damps jitter");
    let q180 = Quality { width: 320, height: 180 };
    let p_big = project(1280, 720, vec![zoom(8.0)], vec![]);
    assert_eq!(
        dq(&p_big, Some((5120, 2880)), q180),
        Quality { width: 1280, height: 720 },
        "step ceiling is 4"
    );
}

/// Test 311 (item 320): the sampler of the zoom maximum over the clip. The scale is taken over the WHOLE
/// clip (the owner's decision), so an animated zoom must be caught
/// at its maximum, not at the moment of the first frame.
#[test]
fn t311_head_zoom_samples_the_whole_clip() {
    let q = Quality { width: 320, height: 180 };
    let nat = Some((1280, 720));
    let key = |t_s: f64, v: f64| AutoKey::at(Tick((t_s * SEC as f64) as i64), v);
    // zoom 1 → 4 by the end of the clip: at the first frame the chain is still identity
    let curve = ParamCurve {
        param: "zoom".into(),
        keys: vec![key(0.0, 1.0), key(2.0, 4.0)],
        source: Default::default(),
    };
    let p = project(320, 180, vec![zoom(1.0)], vec![vec![curve]]);
    assert_eq!(
        dq(&p, nat, q),
        Quality { width: 1280, height: 720 },
        "the step is taken from the curve maximum, not from the first frame"
    );
    // reverse curve (zoom at the start) — the same maximum
    let back = ParamCurve {
        param: "zoom".into(),
        keys: vec![key(0.0, 4.0), key(2.0, 1.0)],
        source: Default::default(),
    };
    let pb = project(320, 180, vec![zoom(1.0)], vec![vec![back]]);
    assert_eq!(dq(&pb, nat, q), Quality { width: 1280, height: 720 });
    // a layer strength stop at zero zeroes the zoom of the prefix: no scale
    let mixed =
        project(320, 180, vec![zoom(2.0), FxInstance::LayerMix { amount: 0.5, from: 0 }], vec![]);
    assert_eq!(dq(&mixed, nat, q), q, "mix at zero — prefix chain is empty");
    // reversing the clip does not break the axis: the maximum is the same
    let mut pr = project(
        320,
        180,
        vec![zoom(1.0)],
        vec![vec![ParamCurve {
            param: "zoom".into(),
            keys: vec![key(0.0, 1.0), key(2.0, 4.0)],
            source: Default::default(),
        }]],
    );
    pr.clips[0].reversed = true;
    assert_eq!(dq(&pr, nat, q), Quality { width: 1280, height: 720 });
}

/// Test 312 (item 321): ORDER == READ together with the size. Warming strictly by
/// `wanted_frames` gives a full composite; warming “as before”, at Quality,
/// gives nothing — the layer does not find its frame. This is t247d along the size
/// axis: if they diverge — a permanent cache miss.
#[test]
fn t312_order_equals_read_with_quality() {
    let q = Quality { width: 128, height: 72 };
    let p = project(128, 72, vec![zoom(2.0)], vec![]);
    let mut vr = VideoRenderer::new_with_natives(&p, &natives(512, 288));
    let range = TickRange::new(Tick(0), Tick(10 * STEP));
    let wanted = vr.wanted_frames(range, q);
    assert!(!wanted.is_empty());
    assert!(
        wanted.iter().all(|(_, _, dq)| *dq == Quality { width: 256, height: 144 }),
        "the order must carry decode quality of the clip"
    );

    // warming EXACTLY by the order
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut src = CheckerSource::new(Tick(10 * SEC), 512, 288);
    for (media, pts, dq) in &wanted {
        let f = src.read_video_frame(*pts, Some(*dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(*media),
            Some(*dq),
            video_cache_idx(*pts),
            f.into(),
        );
    }
    for i in 0..10 {
        let (_, complete) = vr.compose_from_cache(&cache, Tick(i * STEP), q);
        assert!(complete, "frame {i}: composite reads what prefetch warmed");
    }

    // warming at Quality (as before R.46) — the layer does not find its frame
    let cache_q = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    for (media, pts, _) in &wanted {
        let f = src.read_video_frame(*pts, Some(q)).unwrap();
        cache_q.insert_video(
            kt_engine::source_id(*media),
            Some(q),
            video_cache_idx(*pts),
            f.into(),
        );
    }
    let (_, complete) = vr.compose_from_cache(&cache_q, Tick(0), q);
    assert!(!complete, "a frame of another quality does not count as warm");
}

/// Test 313 (item 321): the scale IN THE KEY of post-FX memoization. The same
/// clip at the same Quality from input 1× and input 2× gives DIFFERENT bytes — without
/// the scale in the key the cache would return a stale one.
#[test]
fn t313_postfx_key_includes_the_stage() {
    let q = Quality { width: 128, height: 72 };
    let p = project(128, 72, vec![zoom(2.0)], vec![]);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut src = CheckerSource::new(Tick(10 * SEC), 512, 288);
    let t = Tick(0);

    // frames of both qualities are in the cache at once (the keys differ)
    let mut vr1 = VideoRenderer::new(&p); // no natives → scale 1
    let mut vr2 = VideoRenderer::new_with_natives(&p, &natives(512, 288));
    for vr in [&vr1, &vr2] {
        for (media, pts, dq) in vr.layers_at(t, q) {
            let f = src.read_video_frame(pts, Some(dq)).unwrap();
            cache.insert_video(
                kt_engine::source_id(media),
                Some(dq),
                video_cache_idx(pts),
                f.into(),
            );
        }
    }
    let (a, ca) = vr1.compose_from_cache(&cache, t, q);
    let (b, cb) = vr2.compose_from_cache(&cache, t, q);
    assert!(ca && cb, "both composites full");
    assert_ne!(a, b, "a large input must give a different picture, not a stale one from the cache");

    // a repeated compose of each — from memoization, byte for byte its own
    assert_eq!(vr1.compose_from_cache(&cache, t, q).0, a);
    assert_eq!(vr2.compose_from_cache(&cache, t, q).0, b);
}

/// Test 314 (item 321): the export chunk budget is computed from the DECODE quality.
/// A frame of scale S takes S² times more space in the cache, and a chunk computed by
/// Quality would not survive to the composite: LRU would evict the batch before
/// it was assembled — exactly the regression of item 15 (“black render”).
#[test]
fn t314_export_chunk_respects_the_decode_stage() {
    // the sizes are chosen so that by Quality the chunk fits the budget, but by the
    // decode quality (×2 per side = ×4 memory) it no longer does
    let q = Quality { width: 960, height: 540 };
    let dq = Quality { width: 1920, height: 1080 };
    let by_quality = kt_engine::export::export_chunk_frames(
        kt_engine::export::EXPORT_VIDEO_CACHE_BYTES,
        q.width,
        q.height,
        1,
    );
    let by_decode = kt_engine::export::export_chunk_frames(
        kt_engine::export::EXPORT_VIDEO_CACHE_BYTES,
        dq.width,
        dq.height,
        1,
    );
    assert!(by_decode * 4 <= by_quality, "a chunk must shrink fourfold at step 2");

    // and the live render with a scale gets through without missing layers
    let p = project(128, 72, vec![zoom(2.0)], vec![]);
    let sources: Vec<(MediaId, Box<dyn MediaSource>)> =
        vec![(MediaId(1), Box::new(CheckerSource::new(Tick(10 * SEC), 512, 288)))];
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut frames = 0u64;
    let stats = kt_engine::export::render_raw_video_range_with(
        &p,
        sources,
        Quality { width: 128, height: 72 },
        Some((Tick(0), Tick(30 * STEP))),
        &Default::default(),
        &cancel,
        |_| {},
        |_| {
            frames += 1;
            Ok(())
        },
    )
    .expect("render");
    assert_eq!(frames, 30, "all frames of the range are rendered");
    assert_eq!(stats.missing_video_layers, 0, "no layer was lost at the step");
}

/// Test 317 (item 322): the coverage BARS ask for the DECODE quality.
/// The cache is quality-exact (t8), so a clip with a scale, warmed
/// properly, would look entirely cold when polled by Quality —
/// a violation of invariant No. 4 in the user's eyes.
#[test]
fn t317_coverage_is_queried_at_the_decode_quality() {
    let q = Quality { width: 128, height: 72 };
    let p = project(128, 72, vec![zoom(2.0)], vec![]);
    let vr = VideoRenderer::new_with_natives(&p, &natives(512, 288));
    let dq_clip = vr.video_decode_qualities(q);
    assert_eq!(dq_clip, vec![(ClipId(1), Quality { width: 256, height: 144 })]);

    // the prefetch puts frames strictly by the order
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut src = CheckerSource::new(Tick(10 * SEC), 512, 288);
    for (media, pts, dq) in vr.wanted_frames(TickRange::new(Tick(0), Tick(10 * STEP)), q) {
        let f = src.read_video_frame(pts, Some(dq)).unwrap();
        cache.insert_video(kt_engine::source_id(media), Some(dq), video_cache_idx(pts), f.into());
    }
    let sid = kt_engine::source_id(MediaId(1));
    let range = 0..(10 * STEP);
    let warm = cache.coverage(sid, kt_cache::BurstKind::video(Some(dq_clip[0].1)), range.clone());
    let cold = cache.coverage(sid, kt_cache::BurstKind::video(Some(q)), range);
    assert!(!warm.is_empty(), "the clip is warm for its decode quality");
    assert!(cold.is_empty(), "there are no frames at the Quality of the same clip — the bar would lie");
}

/// Test 484b (R.52): the generator's native (`GEN_NATIVE` = 4096) is the honest
/// “detail headroom” of the vector source: a title clip with a zooming chain
/// orders S×Quality (a sharp zoom), a bare generator — exactly Quality,
/// and without a native, scale 1 even under zoom.
#[test]
fn t484b_generator_native_feeds_decode_stage() {
    use kt_media::generator::GEN_NATIVE;
    let q = Quality { width: 640, height: 360 };
    let zoomed = project(640, 360, vec![zoom(2.0)], vec![]);
    assert_eq!(
        dq(&zoomed, Some((GEN_NATIVE, GEN_NATIVE)), q),
        Quality { width: 1280, height: 720 },
        "title zoom must order a frame at a larger size"
    );
    let plain = project(640, 360, vec![], vec![]);
    assert_eq!(dq(&plain, Some((GEN_NATIVE, GEN_NATIVE)), q), q, "there is no step without FX");
    assert_eq!(dq(&zoomed, None, q), q, "native size unknown — step 1");
}

/// Test 633 (R.58, item 459): a layer of a FOREIGN SIZE never passes silently.
///
/// Before R.58 the composite, meeting a frame of the wrong size, simply skipped the layer —
/// and declared the frame COMPLETE. The export wrote such a frame as good, and an
/// attachment from it disappeared. Rule: if the size does not match — the frame is not complete,
/// and it is re-ordered.
#[test]
fn t633_a_layer_of_a_foreign_size_is_never_silent() {
    let q = Quality { width: 128, height: 72 };
    let p = project(128, 72, vec![], vec![]);
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    // we put a frame of HALF size under the key of full quality: exactly what
    // the scale mismatch inside the program does
    let mut src = CheckerSource::new(Tick(10 * SEC), 512, 288);
    let half = Quality { width: q.width / 2, height: q.height / 2 };
    for (media, pts, dq) in vr.layers_at(Tick(0), q) {
        assert_eq!(dq, q, "fixture: a clip without a chain has step 1");
        let f = src.read_video_frame(pts, Some(half)).unwrap();
        cache.insert_video(kt_engine::source_id(media), Some(dq), video_cache_idx(pts), f.into());
    }
    let (out, complete) = vr.compose_from_cache(&cache, Tick(0), q);
    assert!(
        !complete,
        "the composite declared the frame complete by dropping a layer of a foreign size: such a frame \
         goes into the export as good, and the event is simply absent from it"
    );
    assert!(out.chunks(4).all(|px| px[3] == 0), "a layer of a foreign size still got into the frame");
}

/// Test 714 (R.62): the order equals the read for EVERY conform rule.
/// A vertical source 1080×1920 with four events with four
/// rules in a 1920×1080 project: the decode size is the rule's frame
/// (“fit” 608×1080, “fill” 1920×3414, “as is” 1080×1920,
/// “stretch” 1920×1080), the scale is read from the conform size, and the
/// composite warmed strictly by `wanted_frames` is complete.
#[test]
fn t714_order_equals_read_on_every_conform_rule() {
    let q = Quality { width: 1920, height: 1080 };
    let rules = [Conform::Fit, Conform::Fill, Conform::AsIs, Conform::Stretch];
    let mut p = project(1920, 1080, vec![], vec![]);
    p.clips.clear();
    for (i, r) in rules.iter().enumerate() {
        let mut c = ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(10 + i as u64),
            link_group: None,
            kind: ClipKind::Video,
            lane: i as u32,
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
        c.geometry.rule = *r;
        p.clips.push(c);
    }
    p.normalize(); // tracks fitted to the content (R.62)
    p.validate().unwrap();
    let facts = kt_engine::SourceFacts {
        natives: natives(1080, 1920),
        procedural: Default::default(),
        views: std::collections::HashMap::from([(MediaId(1), (1080.0, 1920.0))]), ..Default::default() };
    let mut vr = kt_engine::VideoRenderer::new_with_facts(&p, &facts);
    assert_eq!(vr.layers_at(Tick(STEP), q).len(), 4);
    let want = [(608, 1080), (1920, 3412), (1080, 1920), (1920, 1080)];
    for (id, dq) in vr.video_decode_qualities(q) {
        let i = (id.0 - 10) as usize;
        assert_eq!((dq.width, dq.height), want[i], "rule {:?}", rules[i]);
    }
    // order == read: warm strictly by wanted_frames and composite
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut src = CheckerSource::new(Tick(2 * SEC), 1080, 1920);
    for (media, pts, dq) in vr.wanted_frames(TickRange::new(Tick(0), Tick(2 * STEP)), q) {
        let f = src.read_video_frame(pts, Some(dq)).unwrap();
        assert_eq!((f.width, f.height), (dq.width, dq.height));
        cache.insert_video(kt_engine::source_id(media), Some(dq), video_cache_idx(pts), f.into());
    }
    let (frame, complete) = vr.compose_from_cache(&cache, Tick(STEP), q);
    assert!(complete, "every layer found its own decode size");
    assert_eq!(frame.len(), (q.width * q.height * 4) as usize);
}
