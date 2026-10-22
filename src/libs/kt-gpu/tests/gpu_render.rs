//! Test 184 (R.29): the GPU twin of test 67: “preview == export” on the GPU path.
//! A frame of the export stream (render_raw_video_range_with + GPU backend)
//! is byte for byte equal to the preview composite of the same moment: both the “cold”
//! (a fresh engine: a second GpuFxEngine on the same adapter) and the “warm”
//! (the same engine, ReadbackCache). Plus: determinism of the export between
//! runs on one device, and a tolerance check of the GPU export against the CPU.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use kt_engine::{ExportBackends, VideoRenderer, render_raw_video};
use kt_gpu::{FrameTol, assert_frames_close, test_engine};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;

fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 128,
        height: 72,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    for id in [1u64, 2] {
        p.media.push(MediaRef::new(MediaId(id), "synth:ramp"));
    }
    let sec = 90_000i64;
    let clip = |id: u64, lane: u32, start: i64, dur: i64, media: u64, opacity: f32| ClipEvent {
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
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![
            FxInstance::Mirror { horizontal: true, vertical: false },
            FxInstance::glow(1.0),
            FxInstance::vhs(2, 0.4),
        ],
        opacity,
        fx_off: false,
    };
    p.clips = vec![
        clip(1, 0, 0, 2 * sec, 1, 1.0),
        clip(2, 1, sec / 2, sec, 2, 0.75), // the top layer with a level
    ];
    p.normalize(); // tracks for the content (R.62)
    p.validate().unwrap();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    [1u64, 2]
        .iter()
        .map(|&id| {
            (
                MediaId(id),
                Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp))
                    as Box<dyn MediaSource>,
            )
        })
        .collect()
}

fn gpu_export_frames(backends: &ExportBackends, q: Quality, grab: &[usize]) -> Vec<Vec<u8>> {
    let p = project();
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut n = 0usize;
    kt_engine::export::render_raw_video_range_with(
        &p,
        sources(),
        q,
        None,
        backends,
        &Arc::new(AtomicBool::new(false)),
        |_| {},
        |rgba| {
            if grab.contains(&n) {
                out.push(rgba.to_vec());
            }
            n += 1;
            Ok(())
        },
    )
    .unwrap();
    out
}

#[test]
fn t184_gpu_preview_equals_gpu_export_cold_and_warm() {
    let Some(e1) = test_engine() else { return };
    let Some(e2) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let target = 17usize;
    let backends = ExportBackends { video_gpu: Some(e1.clone()) };
    let export_frame = gpu_export_frames(&backends, q, &[target]).remove(0);

    // “preview”: an independent cache, direct decoding of the layers, the same frame plan
    let p = project();
    let video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(target as i64 * 3000);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, q) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    use kt_engine::VideoGpuExec;
    // cold: a FRESH engine on the same adapter
    let plan = video.frame_plan(&cache, t, q);
    let mut cold = Vec::new();
    let complete = e2.execute_into(&plan, &mut cold).unwrap();
    assert!(complete);
    assert_eq!(cold, export_frame, "cold GPU composite == GPU export");
    // warm: the export engine, the plan is already in its ReadbackCache
    let mut warm = Vec::new();
    e1.execute_into(&plan, &mut warm).unwrap();
    assert_eq!(warm, export_frame, "warm (ReadbackCache) == GPU export");
}

#[test]
fn t184b_gpu_export_deterministic_and_close_to_cpu() {
    let Some(e1) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let grab = [0usize, 17, 40];
    let backends = ExportBackends { video_gpu: Some(e1) };
    let a = gpu_export_frames(&backends, q, &grab);
    let b = gpu_export_frames(&backends, q, &grab);
    assert_eq!(a, b, "GPU export must be deterministic on the device");

    // the CPU reference of the same frames, within the tolerance of bilinear arithmetic
    let p = project();
    let mut cpu: Vec<Vec<u8>> = Vec::new();
    let mut n = 0usize;
    render_raw_video(&p, sources(), q, |rgba| {
        if grab.contains(&n) {
            cpu.push(rgba.to_vec());
        }
        n += 1;
        Ok(())
    })
    .unwrap();
    for ((g, c), idx) in a.iter().zip(&cpu).zip(grab) {
        assert_frames_close(c, g, q.width, q.height, FrameTol::BILINEAR, &format!("frame {idx}"));
    }
}

/// Test 301 (R.44; formerly t187c, the opposite): export rendering WITHOUT the GPU no longer
/// refuses: Pan3d is computed on the CPU (the inverse homography in the merged
/// geometry pass), and the GpuRequired refusal is removed. The frames really carry the effect, and
/// with a live adapter the CPU and GPU agree within the GEOM tolerance.
#[test]
fn t301_export_without_gpu_renders_3d() {
    let with_fx = |fx: Vec<FxInstance>| {
        let mut p = project();
        p.clips[0].fx = fx;
        p.clips[1].fx = vec![];
        p.validate().unwrap();
        p
    };
    let pan3d = FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 15.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale: 0.8,
        camera: None,
    };
    let q = Quality { width: 128, height: 72 };
    let render = |p: &kt_model::Project, backends: &ExportBackends| -> Vec<u8> {
        let mut got = Vec::new();
        let mut n = 0usize;
        kt_engine::export::render_raw_video_range_with(
            p,
            sources(),
            q,
            None,
            backends,
            &Arc::new(AtomicBool::new(false)),
            |_| {},
            |rgba| {
                if n == 10 {
                    got = rgba.to_vec();
                }
                n += 1;
                Ok(())
            },
        )
        .expect("export render without GPU must work with 3D");
        got
    };
    let cpu_only = ExportBackends::default();
    let cpu3d = render(&with_fx(vec![pan3d.clone()]), &cpu_only);
    let cpu_plain = render(&with_fx(vec![]), &cpu_only);
    assert_ne!(cpu3d, cpu_plain, "Pan3d on CPU must change the picture");
    if let Some(e) = test_engine() {
        let gpu = ExportBackends { video_gpu: Some(e) };
        let gpu3d = render(&with_fx(vec![pan3d]), &gpu);
        assert_frames_close(&cpu3d, &gpu3d, q.width, q.height, FrameTol::GEOM, "t301-cpu-vs-gpu");
    }
}

// ---- 444-446 (R.50, B3): the subtree of an embedding on the GPU ----------------------------

/// A project with an embedding: two overlapping events inside the reel, a level and
/// a chain ON THE INSTANCE. It is exactly this configuration that distinguishes the “processing-order
/// boundary” from its absence.
fn nested_project(outer_fx: Vec<FxInstance>, outer_opacity: f32) -> Project {
    let sec = 90_000i64;
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 128,
        height: 72,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    for id in [1u64, 2] {
        p.media.push(MediaRef::new(MediaId(id), "synth:ramp"));
    }
    let ev = |id: u64, lane: u32, start: i64, dur: i64, media: u64, opacity: f32| ClipEvent {
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
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity,
        fx_off: false,
    };
    let mut s = Sequence::new(MediaId(100), "Piece");
    s.clips = vec![ev(2, 1, 0, 2 * sec, 1, 1.0), ev(3, 0, sec / 2, sec, 2, 0.5)];
    s.ensure_tracks(kt_model::LaneKind::Video, 2);
    p.sequences.push(s);
    let mut inst = ev(10, 0, 0, 2 * sec, 100, outer_opacity);
    inst.fx = outer_fx;
    p.clips = vec![inst];
    p.validate().unwrap();
    p
}

fn warm_cache(p: &Project, q: Quality, t: Tick) -> (VideoRenderer, kt_cache::BurstCache) {
    let video = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, dq) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    (video, cache)
}

#[test]
fn t444_nested_reel_conforms_gpu_to_cpu() {
    use kt_engine::VideoGpuExec;
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let t = Tick(20 * 3000);
    // both with a chain on the embedding and without one: the chain changes the path (it is the
    // sub-composite, not the frame, that crosses the boundary), and both MUST converge
    for fx in [vec![], vec![FxInstance::Mirror { horizontal: true, vertical: false }]] {
        let p = nested_project(fx.clone(), 0.5);
        let (mut video, cache) = warm_cache(&p, q, t);
        let plan = video.frame_plan(&cache, t, q);
        let mut gpu = Vec::new();
        let complete = e.execute_into(&plan, &mut gpu).expect("the GPU must be ABLE to handle the subtree");
        assert!(complete, "a warm composite with a nested item must be complete");
        drop(plan);
        let (cpu, cpu_complete) = video.compose_from_cache(&cache, t, q);
        assert!(cpu_complete);
        assert_frames_close(
            &cpu,
            &gpu,
            q.width,
            q.height,
            FrameTol::BILINEAR,
            &format!("nested item, chain {}", fx.len()),
        );
        // ...and this is not “both black”
        assert!(gpu.iter().any(|&x| x != 0), "the GPU composite of the nested item is black");
    }
}

#[test]
fn t445_nest_boundary_is_not_twice_as_dark_on_gpu() {
    use kt_engine::VideoGpuExec;
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let t = Tick(20 * 3000);
    // Semi-transparent content: exactly where the premultiplication at the
    // boundary gives “twice as dark”. The WGSL mirror of `unpremultiply` MUST
    // match the CPU to the last digit: all the engine’s textures are Rgba8Uint.
    let p = nested_project(vec![], 1.0);
    let (mut video, cache) = warm_cache(&p, q, t);
    let plan = video.frame_plan(&cache, t, q);
    let mut gpu = Vec::new();
    e.execute_into(&plan, &mut gpu).unwrap();
    drop(plan);
    let (cpu, _) = video.compose_from_cache(&cache, t, q);
    assert_frames_close(&cpu, &gpu, q.width, q.height, FrameTol::EXACT, "boundary of the nested item");
}

#[test]
fn t446_nested_gpu_is_deterministic_on_device() {
    use kt_engine::VideoGpuExec;
    let Some(e1) = test_engine() else { return };
    let Some(e2) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let t = Tick(20 * 3000);
    let p = nested_project(vec![FxInstance::glow(1.0)], 0.75);
    let (video, cache) = warm_cache(&p, q, t);
    let plan = video.frame_plan(&cache, t, q);
    let (mut a, mut b, mut c) = (Vec::new(), Vec::new(), Vec::new());
    e1.execute_into(&plan, &mut a).unwrap();
    // a second run of THE SAME engine, through the ReadbackCache: the key MUST
    // tell the subtrees apart, otherwise a foreign sub-composite would come back
    e1.execute_into(&plan, &mut b).unwrap();
    // and a FRESH engine on the same adapter: the pool gives back dirty textures,
    // and the uncleared accumulator of the subtree shows up exactly here
    e2.execute_into(&plan, &mut c).unwrap();
    assert_eq!(a, b, "a repeated run gave another frame (ReadbackCache)");
    assert_eq!(a, c, "a fresh engine gave another frame (dirty pool of the subtree)");

    // the plan key MUST DISTINGUISH the revisions of the reel: we edit the content and
    // require a different frame from the SAME engine
    let mut p2 = nested_project(vec![FxInstance::glow(1.0)], 0.75);
    p2.sequences[0].clips[1].opacity = 0.9;
    p2.validate().unwrap();
    let (video2, cache2) = warm_cache(&p2, q, t);
    let plan2 = video2.frame_plan(&cache2, t, q);
    let mut d = Vec::new();
    e1.execute_into(&plan2, &mut d).unwrap();
    assert_ne!(a, d, "an edit INSIDE the reel did not change the plan key — ReadbackCache returned stale data");

    // A HUNGER INSIDE the reel changes the picture without changing ANY FIELD
    // of the instance: without the subtree plan in the key the engine would return the warm
    // frame on an empty cache, and would never re-request the content.
    let cold = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let plan_cold = video.frame_plan(&cold, t, q);
    let mut e = Vec::new();
    let complete = e1.execute_into(&plan_cold, &mut e).unwrap();
    assert!(!complete, "starvation inside the reel did not reach the outer composite");
    assert_ne!(a, e, "on an EMPTY cache a warm frame came back — the subtree plan is not in the key");
}

/// Test 496 (R.53, item 409): a frame is recognised by its CONTENT, not by its
/// address. Editing the generator’s parameters with a STILL cursor gives the
/// same `(source, cache_idx, size)`, and up to this item the GPU returned
/// the OLD picture: `UploadCache` reused the texture, and
/// `ReadbackCache` the ready composite. The same defect comes with the return
/// of a missing file in place of a stub (t211).
///
/// The harness is blind to this (it is all CPU), so this test lives here.
#[test]
fn t496_gpu_frame_identity_follows_content() {
    use kt_engine::VideoGpuExec;
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 64, height: 36 };
    let t = Tick(0);

    // a project of a single generator clip; we change the colour “under the cursor”
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    let solid = |r: u8| GeneratorKind::Solid { color_r: r, color_g: 0, color_b: 0 };
    p.media.push(MediaRef::new(MediaId(1), kt_model::gen_kind_path(&solid(32))));
    p.clips.push(ClipEvent {
        // R.59, stage 2: the values are ON THE EVENT
        r#gen: Some(solid(32)),
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
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();

    let shot = |red: u8, cache: &kt_cache::BurstCache| -> Vec<u8> {
        let mut src = kt_media::generator::GeneratorSource::new(solid(red));
        let frame = src.read_video_frame(Tick(0), Some(q)).unwrap();
        // THE SAME address: the source, the index and the size do not change
        cache.insert_video(
            kt_engine::source_id(kt_engine::gen_source_addr(ClipId(1))),
            Some(q),
            kt_engine::video::video_cache_idx(Tick(0)),
            frame.into(),
        );
        let video = VideoRenderer::new(&p);
        let plan = video.frame_plan(cache, t, q);
        let mut out = Vec::new();
        e.execute_into(&plan, &mut out).unwrap();
        out
    };

    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let dark = shot(32, &cache);
    let bright = shot(220, &cache);
    assert_eq!(dark[0], 32, "the first frame — the set colour");
    assert_ne!(
        dark, bright,
        "an edit of parameters with a stationary cursor must change the picture on GPU"
    );
    assert_eq!(bright[0], 220, "the second frame — a NEW colour, not the old texture");
}

// ---- 586-587 (R.56, item 448): continuous reel time and blending on the GPU ---

/// A warp effect on a VIDEO instance: only the speed and the frame mode matter.
fn warp_fx(rate: f64, frames: FrameMode) -> FxInstance {
    FxInstance::SlowedReverb {
        rate,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        frames,
        width: 1.0,
        automation: vec![],
    }
}

/// The same scene as `nested_project`, but the lower child has a LIVE
/// brightness curve: without something procedural inside, a half-frame is indistinguishable from
/// a frame: the footage is pinned to the frames of the file (item 448).
fn animated_nested_project(outer_fx: Vec<FxInstance>) -> Project {
    let mut p = nested_project(outer_fx, 1.0);
    let child = &mut p.sequences[0].clips[0];
    child.fx =
        vec![FxInstance::ColorGrade { brightness: 0.0, contrast: 0.0, saturation: 1.0, hue: 0.0 }];
    child.fx_auto = vec![vec![ParamCurve {
        param: "brightness".into(),

        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(60_000), 0.9)]),
        source: Default::default(),
    }]];
    p.validate().unwrap();
    p
}

/// Warm ONE cache at the moment `t` (the keys are exactly those the plan asked for).
///
/// The cache MUST be SHARED across all moments of one engine: `UploadCache`
/// is keyed by the SERIAL NUMBER of the content (item 409), while a fresh `BurstCache`
/// starts its numbering again. Two caches for one engine give it the same
/// keys for DIFFERENT frames, and a foreign texture comes back. In the program the cache is
/// one per session; the test MUST repeat the engine’s wiring.
fn warm_into(video: &VideoRenderer, cache: &kt_cache::BurstCache, q: Quality, t: Tick) {
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, dq) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
}

/// CPU and GPU execution of ONE plan of the moment `t` on a shared cache.
fn cpu_and_gpu(
    e: &kt_gpu::GpuFxEngine,
    video: &mut VideoRenderer,
    cache: &kt_cache::BurstCache,
    q: Quality,
    t: Tick,
) -> (Vec<u8>, Vec<u8>) {
    use kt_engine::VideoGpuExec;
    warm_into(video, cache, q, t);
    let mut gpu = Vec::new();
    {
        let plan = video.frame_plan(cache, t, q);
        let complete = e.execute_into(&plan, &mut gpu).expect("the GPU must be ABLE to handle the subtree");
        assert!(complete, "a warm composite with a nested item must be complete");
    }
    let (cpu, complete) = video.compose_from_cache(cache, t, q);
    assert!(complete, "the CPU composite with a nested item must be complete");
    (cpu, gpu)
}

/// Test 586 (R.56, item 448): a SLOWED reel on the GPU.
///
/// Red on the old code: the internal reel time was floored to the grid, and
/// neighbouring output frames came out identical on BOTH backends, together
/// with the animation inside the composition.
#[test]
fn t586_slowed_nest_conforms_and_is_deterministic_on_gpu() {
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let p = animated_nested_project(vec![warp_fx(0.5, FrameMode::Nearest)]);
    let mut video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());

    // an EVEN frame sits on a whole step of the material, an ODD one on a half step
    let (even, odd) = (Tick(20 * 3000), Tick(21 * 3000));
    let (cpu_e, gpu_e) = cpu_and_gpu(&e, &mut video, &cache, q, even);
    let (cpu_o, gpu_o) = cpu_and_gpu(&e, &mut video, &cache, q, odd);
    assert_frames_close(&cpu_e, &gpu_e, q.width, q.height, FrameTol::EXACT, "t586-even");
    assert_frames_close(&cpu_o, &gpu_o, q.width, q.height, FrameTol::EXACT, "t586-odd");
    assert_ne!(
        gpu_e, gpu_o,
        "the half-frame of the slowed reel matched the frame: internal time rounds \
         to the grid, and the animation inside the composition steps"
    );
    assert!(gpu_o.iter().any(|&x| x != 0), "the frame is black — nothing to compare");

    // determinism on ONE engine: the second run goes through the ReadbackCache,
    // and the plan key MUST tell the half-frames apart
    let (_, again) = cpu_and_gpu(&e, &mut video, &cache, q, odd);
    assert_eq!(gpu_o, again, "a repeated run of the same plan gave another frame");
}

/// Test 587 (R.56, item 448): BLENDING on an embedding, on the GPU.
///
/// The LEAVES are blended, not two sub-composites: the GPU gets the already blended
/// pre-FX frame as an ordinary `LayerInput::Frame`, and no second dissolve pass
/// appears. The plan must nonetheless differ from “nearest”, otherwise
/// ReadbackCache would return someone else’s picture.
#[test]
fn t587_blend_on_a_nest_differs_from_nearest_on_gpu() {
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let t = Tick(21 * 3000); // a half-frame: the leaves have a pair
    let blend = nested_project(vec![warp_fx(0.5, FrameMode::Blend)], 1.0);
    let near = nested_project(vec![warp_fx(0.5, FrameMode::Nearest)], 1.0);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut vb = VideoRenderer::new(&blend);
    let mut vn = VideoRenderer::new(&near);

    let (cpu_b, gpu_b) = cpu_and_gpu(&e, &mut vb, &cache, q, t);
    let (cpu_n, gpu_n) = cpu_and_gpu(&e, &mut vn, &cache, q, t);
    assert_frames_close(&cpu_b, &gpu_b, q.width, q.height, FrameTol::EXACT, "t587-blend");
    assert_frames_close(&cpu_n, &gpu_n, q.width, q.height, FrameTol::EXACT, "t587-nearest");
    assert_ne!(
        gpu_b, gpu_n,
        "“mixing” on the nested item does not differ from “nearest”: the mode \
         did not reach the leaves"
    );
    // and it is not “one of them is black”
    assert!(gpu_b.iter().any(|&x| x != 0) && gpu_n.iter().any(|&x| x != 0));

    // determinism on one engine (ReadbackCache)
    let (_, again) = cpu_and_gpu(&e, &mut vb, &cache, q, t);
    assert_eq!(gpu_b, again, "a repeated run of a mixed plan gave another frame");
}

// ---- 632 (R.58, item 459): the REEL tier ≠ the chain input of the instance --------------

/// The same project, but the reel has TWO instances: a zooming one (it raises
/// the reel tier) and a twin with its own chain. The tier is one per reel, and not every
/// chain can consume it: the sub-composite is FITTED for the second instance
/// (`NestSrc::fit_to`; on the GPU this is a `Resample` to the whole frame).
fn twin_nest_project(twin_fx: Vec<FxInstance>) -> Project {
    let sec = 90_000i64;
    let mut p = nested_project(vec![FxInstance::geometry_zoom(3.0)], 1.0);
    let mut twin = p.clips[0].clone();
    twin.id = ClipId(11);
    twin.start = Tick(10 * sec);
    twin.fx = twin_fx;
    p.clips.push(twin);
    p.validate().unwrap();
    p
}

#[test]
fn t632_reel_stage_and_chain_input_conform_gpu_to_cpu() {
    use kt_engine::VideoGpuExec;
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let sec = 90_000i64;
    let t_twin = Tick(10 * sec + 20 * 3000);
    let cases: Vec<(&str, Vec<FxInstance>)> = vec![
        // fitting: the chain of the tier does not consume it
        ("empty", vec![]),
        ("glow", vec![FxInstance::glow(1.0)]),
        (
            "leading mix",
            vec![FxInstance::geometry_zoom(2.0), FxInstance::LayerMix { amount: 0.5, from: 0 }],
        ),
        // without fitting: the instance itself requests the reel’s tier
        ("same zoom", vec![FxInstance::geometry_zoom(3.0)]),
    ];
    for (name, fx) in cases {
        let p = twin_nest_project(fx);
        let (mut video, cache) = warm_cache(&p, q, t_twin);
        let plan = video.frame_plan(&cache, t_twin, q);
        let mut gpu = Vec::new();
        let complete = e.execute_into(&plan, &mut gpu).expect("the GPU must be ABLE to handle the subtree");
        assert!(complete, "{name}: the warm composite is declared incomplete");
        drop(plan);
        let (cpu, cpu_complete) = video.compose_from_cache(&cache, t_twin, q);
        assert!(cpu_complete, "{name}: the CPU composite is declared incomplete");
        // BILINEAR tolerance: fitting is a resample, and `resample.wgsl` computes
        // in f32 and rounds to even, while `fit_into` works in f64 and rounds away from zero
        assert_frames_close(&cpu, &gpu, q.width, q.height, FrameTol::BILINEAR, name);
        assert!(gpu.iter().any(|&x| x != 0), "{name}: the GPU composite is black — the nested item dropped out");
        assert!(cpu.iter().any(|&x| x != 0), "{name}: the CPU composite is black — the nested item dropped out");
    }
}

/// Test 687 (R.59, stage 3): two DIFFERENTLY overridden instances
/// of one reel converge on both backends.
///
/// Invariant 7 is read literally here: “the preview substitutes one thing, the export
/// another” is a second composition path. The public values are resolved by the
/// MODEL, before the plan, so the executors MUST have no chance to diverge;
/// the test guards that the plan of the instances really IS DIFFERENT (otherwise
/// it would be green for nothing).
#[test]
fn t687_two_differently_overridden_instances_conform_gpu_to_cpu() {
    use kt_engine::VideoGpuExec;
    let Some(e) = test_engine() else { return };
    let sec = 90_000i64;
    let q = Quality { width: 128, height: 72 };

    let mut p = nested_project(vec![], 1.0);
    // a handle: the level of the TOP event inside the reel
    let s = p.sequences.iter_mut().find(|s| s.id == MediaId(100)).unwrap();
    s.published = vec![kt_model::PublishedParam {
        id: kt_model::PubId(1),
        name: "Plate level".into(),
        target: kt_model::ParamAddr::new(ClipId(3), kt_model::ParamOwner::ClipAttr, "opacity"),
        ty: kt_model::ParamType::Num,
        range: kt_model::PubRange { min: None, max: None },
    }];
    // a second instance of the same reel, with its own value
    let mut twin = p.clips[0].clone();
    twin.id = ClipId(11);
    twin.start = Tick(4 * sec);
    twin.overrides =
        vec![kt_model::Override::value(kt_model::PubId(1), kt_model::ParamValue::Num(1.0))];
    p.clips.push(twin);
    p.validate().expect("two differently overridden placements are legal");

    let at = |k: i64| Tick(k + 20 * 3000);
    let mut frames = Vec::new();
    for t in [at(0), at(4 * sec)] {
        let (mut video, cache) = warm_cache(&p, q, t);
        let plan = video.frame_plan(&cache, t, q);
        let mut gpu = Vec::new();
        let complete = e.execute_into(&plan, &mut gpu).expect("the GPU must be ABLE to handle the subtree");
        assert!(complete, "a warm composite with a nested item must be complete");
        drop(plan);
        let (cpu, cpu_complete) = video.compose_from_cache(&cache, t, q);
        assert!(cpu_complete);
        assert_frames_close(
            &cpu,
            &gpu,
            q.width,
            q.height,
            FrameTol::EXACT,
            "instance with an override",
        );
        assert!(gpu.iter().any(|&x| x != 0), "the GPU composite of the instance is black");
        frames.push(cpu);
    }
    assert_ne!(
        frames[0], frames[1],
        "the instances drew ONE thing: the override did not reach the plan, and \
         conformance was verified on nothing"
    );
}

/// Test 724b (R.62): the reel’s own fps on the GPU: conformance EXACT with the CPU,
/// and the frame holds (the even and odd frames of the project are equal).
#[test]
fn t724b_reel_fps_holds_frames_on_gpu() {
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let mut p = animated_nested_project(vec![]);
    p.sequences[0].fps = Some(15);
    p.validate().unwrap();
    let mut video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let (even, odd, next) = (Tick(20 * 3000), Tick(21 * 3000), Tick(22 * 3000));
    let (cpu_e, gpu_e) = cpu_and_gpu(&e, &mut video, &cache, q, even);
    let (cpu_o, gpu_o) = cpu_and_gpu(&e, &mut video, &cache, q, odd);
    let (_, gpu_n) = cpu_and_gpu(&e, &mut video, &cache, q, next);
    assert_frames_close(&cpu_e, &gpu_e, q.width, q.height, FrameTol::EXACT, "t724b-even");
    assert_frames_close(&cpu_o, &gpu_o, q.width, q.height, FrameTol::EXACT, "t724b-odd");
    assert_eq!(gpu_e, gpu_o, "reel at 15 fps in a 30 project: an odd frame holds an even one");
    assert_ne!(gpu_e, gpu_n, "the next reel frame is different");
    assert!(gpu_e.iter().any(|&x| x != 0), "the frame is black — nothing to compare");
}
