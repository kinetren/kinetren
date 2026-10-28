//! Test 946 (morpher-event-grid-cost, D4): the owner's case, built in code.
//!
//! ktprompts 26.10.18 item 8: "an event grid in the morpher with automation 0 → 12×30 →
//! 12×10 → 0 drops fps". The scene is ours (R.67 — not the owner's project): a synthetic
//! 960×540 source, a morpher block over it with an event grid whose counts are automated
//! 1×1 → 12×30 → 12×10 → 1×1, with fades at both ends (so the strength slot is live).

use kt_engine::VideoRenderer;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

const W: u32 = 960;
const H: u32 = 540;
const SEC: i64 = 90_000;

fn curve(param: &str, vals: [f64; 4]) -> ParamCurve {
    ParamCurve {
        param: param.into(),
        keys: AutoKey::linear(
            [0, 2, 4, 6].iter().zip(vals).map(|(s, v)| AutoKey::at(Tick(s * SEC), v)).collect(),
        ),
        source: Default::default(),
    }
}

/// The owner's case: one event on the source, a morpher block with an automated grid over it.
pub fn owner_case() -> Project {
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
        duration: Tick(6 * SEC),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.ensure_tracks(LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: Tick(6 * SEC),
        targets: MorphTargets::Clips { ids: vec![ClipId(1)] },
        patch: MorphPatch {
            video_fx: vec![FxInstance::EventGrid { cols: 1, rows: 1, center: false }],
            video_auto: vec![vec![
                curve("cols", [1.0, 12.0, 12.0, 1.0]),
                curve("rows", [1.0, 30.0, 10.0, 1.0]),
            ]],
            ..Default::default()
        },
        timed: true,
        strength: 1.0,
        fade_in: Tick(SEC / 2),
        fade_out: Tick(SEC / 2),
        enabled: true,
    });
    p.validate().expect("the owner's case is a valid project");
    p
}

/// Compose a frame at `t` with ONE renderer (the cache warmed with exactly the layers the plan
/// names), and the cost the CPU executor reported for it.
fn frame(vr: &mut VideoRenderer, t: Tick) -> (Vec<u8>, kt_fx::video::FrameCost) {
    use kt_media::MediaSource;
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: W, height: H };
    let mut synth = SyntheticSource::new(Tick(SEC * 60), AudioPattern::Silence);
    for (media, src_pts, dq) in vr.layers_at(t, q) {
        let f = synth.read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video_cache_idx(src_pts),
            std::sync::Arc::new(f),
        );
    }
    let (out, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete, "frame {t:?}: all layers are warm");
    (out, vr.last_cost())
}

/// The range hash of the owner's case on the CPU reference, recorded BEFORE any executor
/// change of this change (stage 1). The tile rule and the strength-0 skip must not move it.
const RANGE_HASH: u64 = 0xc73e_46f0_e915_bf00;

/// Render the range with ONE renderer; returns the range hash and the costliest frame.
fn render_range(p: &Project) -> (u64, (Tick, kt_fx::video::FrameCost)) {
    // ONE renderer for the whole range: the automation moves per frame through the compiled
    // plan, nothing is rebuilt — and the frames equal those of a fresh renderer per frame
    let mut vr = VideoRenderer::new(p);
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    let mut peak = (Tick(0), kt_fx::video::FrameCost::default());
    for k in 0..12 {
        let t = Tick(k * SEC / 2);
        let (bytes, cost) = frame(&mut vr, t);
        if k % 4 == 1 {
            let (fresh, _) = frame(&mut VideoRenderer::new(p), t);
            assert!(fresh == bytes, "at {t:?} a fresh renderer gives other bytes");
        }
        h.update(&bytes);
        if cost.subsamples > peak.1.subsamples {
            peak = (t, cost);
        }
    }
    (h.digest(), peak)
}

#[test]
fn t946_owner_grid_case_renders_the_same() {
    let (got, peak) = render_range(&owner_case());
    eprintln!("t946 range hash: {got:#018x}; peak cost {peak:?}");
    assert_eq!(got, RANGE_HASH, "the owner's case renders other bytes than before the change");
}

/// The cost bound of the owner's case: the costliest frame reads at most a tenth of the full
/// 16-tap pass. RED until the tile rule (stage 3): "the peak frame at Tick(45000) reads 8294400
/// sub-samples, the full 16-tap pass is 8294400".
#[test]
#[ignore = "morpher-event-grid-cost stage 3 (the tile rule) turns it on"]
fn t946b_owner_grid_case_cost_is_bounded() {
    let (_, peak) = render_range(&owner_case());
    let full = W as u64 * H as u64 * 16;
    assert!(
        peak.1.subsamples * 10 <= full,
        "the peak frame at {:?} reads {} sub-samples, the full 16-tap pass is {full}",
        peak.0,
        peak.1.subsamples
    );
}

/// Test 945 (morpher-event-grid-cost, D3): a strength slot at exactly 0 skips the layer's slice.
///
/// At t = 0 the block's fade-in puts the strength at 0: the frame must be the frame WITHOUT the
/// layer (what `blend(dry, wet, 0)` always gave), and the grid must not run — no sub-samples,
/// one mix of the dry side with itself. A strength just above 0 still runs the grid.
#[test]
fn t945_strength_zero_skips_the_layer() {
    let p = owner_case();
    let mut bare = p.clone();
    bare.morphers.clear();
    let (want, _) = frame(&mut VideoRenderer::new(&bare), Tick(0));
    let mut vr = VideoRenderer::new(&p);
    let (got, cost) = frame(&mut vr, Tick(0));
    assert!(got == want, "strength 0 gave other bytes than the frame without the layer");
    assert_eq!(
        (cost.subsamples, cost.snapshots, cost.mixes),
        (0, 1, 1),
        "strength 0 still computed the grid: {cost:?}"
    );
    // one frame in (strength 1/15): the grid runs
    let (_, cost) = frame(&mut vr, Tick(SEC / 30));
    assert!(cost.subsamples > 0, "a strength above 0 must run the layer: {cost:?}");
}
