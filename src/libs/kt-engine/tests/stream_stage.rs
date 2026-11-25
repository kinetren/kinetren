//! The stream source stage in the engine (datamosh-codec-path, design D3–D6): t1241 (a seek equals
//! playing up to the frame), t1242 (an export in pieces equals one in one go), t1248 (the order asks
//! what the stage reads).
//!
//! The scene is the classic cut mosh: shot A (a still texture) for one second, then shot B (another
//! texture moving right 2 px per frame) on the same track with `datamosh` — the stream starts 12
//! frames into A and the I-frame at the cut is dropped.

use std::sync::Arc;

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::VideoRenderer;
use kt_media::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_model::*;
use kt_plugin_api::generator::lowbias32;
use kt_plugin_api::stream::StreamMemo;
use kt_time::Tick;

/// The stage's encoder needs the host's compute device: skip without one, as the GPU tests do.
fn no_device() -> bool {
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    let none = kt_plugin_api::compute::host().is_none();
    if none {
        assert!(std::env::var("KT_REQUIRE_GPU").as_deref() != Ok("1"), "KT_REQUIRE_GPU=1, but no compute device");
        eprintln!("skipped: no compute device");
    }
    none
}

const STEP: i64 = 3000; // 30 fps

/// A textured source: value noise (hashed lattice every 8 px, bilinear) moving `speed` px per
/// frame, drawn at whatever size is asked (the stream asks its own).
struct Tex {
    info: MediaInfo,
    seed: u32,
    speed: i32,
}

impl Tex {
    fn new(seed: u32, speed: i32) -> Tex {
        Tex {
            info: MediaInfo {
                duration: Tick(90_000 * 10),
                has_video: true,
                has_audio: false,
                fps: Some((30, 1)),
                width: 640,
                height: 360,
                sar: (1, 1),
                still: false,
            },
            seed,
            speed,
        }
    }
}

impl MediaSource for Tex {
    fn info(&self) -> &MediaInfo {
        &self.info
    }
    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        Ok(AudioBurst::silence(idx))
    }
    fn read_video_frame(&mut self, pts: Tick, quality: Option<Quality>) -> Result<VideoFrame, MediaError> {
        let q = quality.unwrap_or(Quality { width: 64, height: 36 });
        let dx = self.speed * (pts.0 / STEP) as i32;
        let seed = self.seed;
        let lat = |x: i32, y: i32, c: u32| -> i32 {
            (lowbias32(seed.wrapping_mul(0x9e37) ^ lowbias32((x as u32).wrapping_mul(73) ^ lowbias32((y as u32).wrapping_mul(151) ^ c))) % 200) as i32 + 28
        };
        let mut rgba = vec![255u8; (q.width * q.height * 4) as usize];
        for y in 0..q.height as i32 {
            for x in 0..q.width as i32 {
                let (sx, sy) = (x - dx + 4000, y + 4000);
                let (gx, gy, fx, fy) = (sx / 8, sy / 8, sx % 8, sy % 8);
                for c in 0..3u32 {
                    let a = lat(gx, gy, c) * (8 - fx) + lat(gx + 1, gy, c) * fx;
                    let b = lat(gx, gy + 1, c) * (8 - fx) + lat(gx + 1, gy + 1, c) * fx;
                    rgba[((y as u32 * q.width + x as u32) * 4 + c) as usize] = ((a * (8 - fy) + b * fy) / 64) as u8;
                }
            }
        }
        Ok(VideoFrame { pts, width: q.width, height: q.height, rgba: rgba.into_boxed_slice(), has_alpha: false })
    }
    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        Some(Tick(pts.0.max(0)))
    }
}

fn clip(id: u64, media: u64, start: i64, dur: i64, fx: Vec<FxInstance>) -> ClipEvent {
    ClipEvent {
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
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
        masks: Default::default(),
        fx_in: Vec::new(),
    }
}

/// `mosh` — whether the second shot carries the datamosh slot.
fn project(mosh: bool) -> Project {
    project_with(mosh, None, 1.0)
}

/// The same scene; `reset_at` — a `reset` curve on the slot rising to on at that TIMELINE frame
/// (a Hold key on the beat), the mosh gate on everywhere.
fn project_with(mosh: bool, reset_at: Option<i64>, gate: f64) -> Project {
    kt_plugins_builtin::install();
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.media.push(MediaRef::new(MediaId(2), "synth:2"));
    let mut fx = vec![];
    if mosh {
        let mut f = FxInstance::plugin("datamosh").expect("datamosh is built in");
        for (name, v) in [("lines", 0.0), ("before", 12.0), ("mosh", gate), ("gop", 12.0), ("qp", 14.0)] {
            assert!(f.set_param(name, v), "{name}");
        }
        fx.push(f);
    }
    p.clips.push(clip(1, 1, 0, 30 * STEP, vec![]));
    let mut b = clip(2, 2, 30 * STEP, 30 * STEP, fx);
    if let Some(at) = reset_at {
        // the key's tick is the clip's MATERIAL: it starts at 1 s with `source_in` 0
        let m = (at - 30) * STEP;
        b.fx_auto = vec![vec![ParamCurve {
            param: "reset".into(),
            keys: vec![
                AutoKey::shaped(Tick(0), 0.0, SegShape::Hold),
                AutoKey::shaped(Tick(m), 1.0, SegShape::Hold),
                AutoKey::shaped(Tick(29 * STEP), 1.0, SegShape::Hold),
            ],
            source: CurveSource::Keys,
        }]];
    }
    p.clips.push(b);
    p.validate().unwrap();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(MediaId(1), Box::new(Tex::new(1, 0))), (MediaId(2), Box::new(Tex::new(2, 2)))]
}

const Q: Quality = Quality { width: 160, height: 90 };

fn renderer(p: &Project) -> VideoRenderer {
    let mut vr = VideoRenderer::new(p);
    vr.set_stream_memo(Arc::new(StreamMemo::new(256 << 20)));
    vr
}

/// Warm what `layers_at(t)` names (the prefetch's job), from the same sources.
fn warm(vr: &VideoRenderer, cache: &BurstCache, t: Tick) {
    let mut a = Tex::new(1, 0);
    let mut b = Tex::new(2, 2);
    for (m, pts, q) in vr.layers_at(t, Q) {
        let s: &mut dyn MediaSource = if m == MediaId(1) { &mut a } else { &mut b };
        let f = s.read_video_frame(pts, Some(q)).unwrap();
        cache.insert_video(kt_engine::source_id(m), Some(q), kt_engine::video_cache_idx(pts), Arc::new(f));
    }
}

fn big_cache() -> BurstCache {
    BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: 1 << 30 })
}

/// t1241: a frame inside the mosh is the same bytes after playing up to it and after a cold seek.
#[test]
fn t1241_seek_equals_play() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let n = Tick(38 * STEP);
    let p = project(true);
    let (played, cache) = (renderer(&p), big_cache());
    let mut last = (vec![], false);
    for k in 0..=38 {
        let t = Tick(k * STEP);
        warm(&played, &cache, t);
        last = kt_gpu::testkit::compose(&played, &cache, t, Q);
    }
    assert!(last.1, "the played frame is complete");
    let (seek, cold) = (renderer(&p), big_cache());
    warm(&seek, &cold, n);
    let sought = kt_gpu::testkit::compose(&seek, &cold, n, Q);
    assert!(sought.1, "one warm-up of what the order names completes the frame");
    assert_eq!(sought.0, last.0, "a seek gives the bytes of playing");
    // and the slot does something: without it the frame is B itself
    let plain = project(false);
    let (vr, c) = (renderer(&plain), big_cache());
    warm(&vr, &c, n);
    assert_ne!(kt_gpu::testkit::compose(&vr, &c, n, Q).0, last.0);
}

fn export(p: &Project, range: (i64, i64)) -> Vec<u8> {
    let mut out = vec![];
    kt_gpu::testkit::render_range(
        p,
        sources(),
        Q,
        Some((Tick(range.0 * STEP), Tick(range.1 * STEP))),
        &Arc::new(std::sync::atomic::AtomicBool::new(false)),
        |_| {},
        |f| {
            out.extend_from_slice(f);
            Ok(())
        },
    )
    .unwrap();
    out
}

/// t1242: an export in pieces (each a fresh renderer, memo and cache — harder than a chunk
/// boundary, which keeps the memo) equals one in one go; a piece starting inside the mosh pre-rolls.
#[test]
fn t1242_export_in_pieces_equals_one_go() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let p = project(true);
    let whole = export(&p, (26, 46));
    let mut parts = export(&p, (26, 33));
    parts.extend(export(&p, (33, 41)));
    parts.extend(export(&p, (41, 46)));
    assert_eq!(parts.len(), whole.len());
    assert!(parts == whole, "the pieces differ from the whole");
}

/// t1248: the order of a stream layer is its stream at the input size (mosh-sharp D2): the history (the event before
/// included) with an empty memo, then only the next frame's own source.
#[test]
fn t1248_the_order_asks_what_the_stage_reads() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let p = project(true);
    let (vr, cache) = (renderer(&p), big_cache());
    let t = Tick(36 * STEP);
    let order = vr.layers_at(t, Q);
    let cq = order.iter().find(|(m, ..)| *m == MediaId(2)).expect("the mosh orders its source").2;
    // the input follows the layer (mosh-sharp D2): a new Datamosh is sharp (kkt-26.11.18b U7), its
    // carrier runs at the layer's size, and the codec's 144 lines are resampled from it
    assert_eq!((cq.width, cq.height), (Q.width, Q.height), "the stream is ordered at its input size, the layer's here");
    assert!(order.iter().all(|(_, _, q)| *q == cq), "no frame at the layer's ordinary quality: {order:?}");
    assert!(order.iter().any(|(m, ..)| *m == MediaId(1)), "the history reaches into the event before");
    assert!(order.iter().filter(|(m, ..)| *m == MediaId(2)).count() >= 7, "the history from the cut");
    warm(&vr, &cache, t);
    assert!(kt_gpu::testkit::compose(&vr, &cache, t, Q).1);
    let next = Tick(37 * STEP);
    // the source pts is the material position: the clip starts at 1 s with `source_in` 0
    let pts = Tick(next.0 - 30 * STEP);
    assert_eq!(vr.layers_at(next, Q), vec![(MediaId(2), pts, cq)], "steady state: the next frame only");
    assert_eq!(vr.video_decode_qualities(Q).iter().find(|(id, _)| *id == ClipId(2)).unwrap().1, cq);
}

/// The mean absolute difference of two RGBA frames.
fn mad(a: &[u8], b: &[u8]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).abs() as f64).sum::<f64>() / a.len().max(1) as f64
}

/// t1246 (the owner, 2026-11-01: "reset … by keys on the curves … on the beat"): a `reset` curve key
/// on the event ends the mosh at that frame — the engine reads the slot's rows per frame with its
/// curves; a seek still equals playing.
#[test]
fn t1246_a_reset_key_ends_the_mosh_on_the_beat() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let beat = 44;
    let p = project_with(true, Some(beat), 1.0);
    // the same slot with the mosh gate off: the intact codec (the source draws its texture in output
    // pixels, so a frame at another size is another picture — compare at the same stream size)
    let plain = project_with(true, None, 0.0);
    let frame = |proj: &Project, k: i64| {
        let (vr, c) = (renderer(proj), big_cache());
        warm(&vr, &c, Tick(k * STEP));
        let (f, complete) = kt_gpu::testkit::compose(&vr, &c, Tick(k * STEP), Q);
        assert!(complete, "frame {k} is complete after one warm-up");
        f
    };
    let before = mad(&frame(&p, beat - 2), &frame(&plain, beat - 2));
    let on = mad(&frame(&p, beat), &frame(&plain, beat));
    let after = mad(&frame(&p, beat + 3), &frame(&plain, beat + 3));
    assert!(before > 3.0 * on.max(1.0), "before the beat the mosh ({before:.1}), on it shot B ({on:.1})");
    assert!(after < 12.0, "after the beat the stream is shot B again ({after:.1})");
    // seek == play with the curve
    let (played, cache) = (renderer(&p), big_cache());
    let mut last = vec![];
    for k in 28..=beat + 2 {
        warm(&played, &cache, Tick(k * STEP));
        last = kt_gpu::testkit::compose(&played, &cache, Tick(k * STEP), Q).0;
    }
    assert_eq!(frame(&p, beat + 2), last);
}

/// Shot B's chain: `posterize` at `front` levels IN FRONT of the mosh, the mosh, and `posterize` at
/// `after` levels after it (stream-input-order, found-defects 484: the owner's camera_isp → datamosh
/// → vhs_tape).
fn project_order(front: Option<f64>, after: Option<f64>) -> Project {
    let mut p = project(true);
    let poster = |levels: f64| {
        let mut f = FxInstance::plugin("posterize").expect("posterize is built in");
        assert!(f.set_param("levels", levels));
        f
    };
    let b = p.clips.iter_mut().find(|c| c.id == ClipId(2)).unwrap();
    if let Some(l) = front {
        b.fx.insert(0, poster(l));
    }
    if let Some(l) = after {
        b.fx.push(poster(l));
    }
    p.validate().unwrap();
    p
}

/// Shot B's texture with `posterize` already run over each frame, by the test engine at the size asked:
/// what the stream must read when the slot stands in front of the mosh.
struct Baked {
    tex: Tex,
    levels: f64,
}

impl MediaSource for Baked {
    fn info(&self) -> &MediaInfo {
        self.tex.info()
    }
    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        self.tex.read_audio_burst(idx)
    }
    fn read_video_frame(&mut self, pts: Tick, quality: Option<Quality>) -> Result<VideoFrame, MediaError> {
        use kt_engine::VideoGpuExec as _;
        let f = Arc::new(self.tex.read_video_frame(pts, quality)?);
        let mut slot = FxInstance::plugin("posterize").unwrap();
        slot.set_param("levels", self.levels);
        let chain = [slot];
        let plan = kt_engine::chain_plan(f.clone(), &chain, 1, 0.0, pts, None);
        let mut out = vec![];
        assert!(kt_gpu::testkit::engine().execute_once(&plan, &mut out).unwrap());
        Ok(VideoFrame { pts, width: f.width, height: f.height, rgba: out.into_boxed_slice(), has_alpha: false })
    }
    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        self.tex.keyframe_before(pts)
    }
}

/// Warm what the order names, shot A from `a`, shot B from `b`.
fn warm_from(vr: &VideoRenderer, cache: &BurstCache, t: Tick, a: &mut dyn MediaSource, b: &mut dyn MediaSource) {
    for (m, pts, q) in vr.layers_at(t, Q) {
        let s: &mut dyn MediaSource = if m == MediaId(1) { &mut *a } else { &mut *b };
        let f = s.read_video_frame(pts, Some(q)).unwrap();
        cache.insert_video(kt_engine::source_id(m), Some(q), kt_engine::video_cache_idx(pts), Arc::new(f));
    }
}

/// t2470 (found-defects 484): a slot in front of a stream stage feeds it — the frame of the chain
/// posterize → datamosh is the mosh of a posterized stream (the same bytes as sources posterized
/// before decoding: the history in the event before is read through the slot too), and posterize does not run again after the stage; the old order (the slot run
/// over the mosh's output) is another picture.
#[test]
fn t2470_a_slot_in_front_feeds_the_stream() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let t = Tick(38 * STEP);
    let frame = |p: &Project, a: &mut dyn MediaSource, b: &mut dyn MediaSource| {
        let (vr, c) = (renderer(p), big_cache());
        warm_from(&vr, &c, t, a, b);
        let (f, complete) = kt_gpu::testkit::compose(&vr, &c, t, Q);
        assert!(complete, "one warm-up of what the order names completes the frame");
        f
    };
    let runs = kt_engine::video::front_runs();
    let ordered = frame(&project_order(Some(3.0), None), &mut Tex::new(1, 0), &mut Tex::new(2, 2));
    assert!(kt_engine::video::front_runs() > runs, "the slot in front ran on the stream's frames");
    let baked = frame(
        &project_order(None, None),
        &mut Baked { tex: Tex::new(1, 0), levels: 3.0 },
        &mut Baked { tex: Tex::new(2, 2), levels: 3.0 },
    );
    assert_eq!(ordered, baked, "the stream reads shot B through the slot in front of it");
    let old = frame(&project_order(None, Some(3.0)), &mut Tex::new(1, 0), &mut Tex::new(2, 2));
    assert_ne!(ordered, old, "the slot in front is not the slot after the stage");
}

/// t2471 (found-defects 484): the history names what feeds it — an edit of a slot in front of the
/// stage makes the order ask the whole history again, an edit of a slot after it asks nothing new.
#[test]
fn t2471_an_edit_in_front_invalidates_the_history_one_after_does_not() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let t = Tick(38 * STEP);
    let memo = Arc::new(StreamMemo::new(256 << 20));
    let build = |p: &Project| {
        let mut vr = VideoRenderer::new(p);
        vr.set_stream_memo(memo.clone());
        vr
    };
    let shot_b = |vr: &VideoRenderer| vr.layers_at(t, Q).iter().filter(|(m, ..)| *m == MediaId(2)).count();
    let first = build(&project_order(Some(3.0), Some(5.0)));
    let cache = big_cache();
    warm(&first, &cache, t);
    assert!(kt_gpu::testkit::compose(&first, &cache, t, Q).1);
    assert_eq!(shot_b(&first), 0, "the memo holds the history");
    assert_eq!(shot_b(&build(&project_order(Some(3.0), Some(4.0)))), 0, "a slot after the stage feeds nothing");
    assert!(shot_b(&build(&project_order(Some(4.0), Some(5.0)))) >= 7, "a slot in front feeds the whole history");
}

/// t2472 (found-defects 484): an export reads the stream through the slot in front too — in pieces
/// (each a fresh renderer, memo and cache) it equals one go, and it is not the export of the old order.
#[test]
fn t2472_an_export_runs_the_slot_in_front_of_the_stream() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let p = project_order(Some(3.0), None);
    let whole = export(&p, (30, 42));
    let mut parts = export(&p, (30, 36));
    parts.extend(export(&p, (36, 42)));
    assert!(parts == whole, "the pieces differ from the whole");
    assert!(whole != export(&project_order(None, Some(3.0)), (30, 42)), "the export runs the person's order");
}

/// A rectangle mask over the left half of the frame, hard-edged.
fn left_half() -> Mask {
    Mask::new(1, "left", MaskShape::Rect { cx: 0.25, cy: 0.5, w: 0.5, h: 1.0, rot: 0.0, round: 0.0 })
}

/// Compose the frame at `t` of `p` from the textures, on a renderer of its own (or on `memo`).
fn frame_at(p: &Project, t: Tick) -> Vec<u8> {
    let (vr, c) = (renderer(p), big_cache());
    warm(&vr, &c, t);
    let (f, complete) = kt_gpu::testkit::compose(&vr, &c, t, Q);
    assert!(complete, "one warm-up of what the order names completes the frame");
    f
}

/// t2550 (chain-everywhere, found-defects 541): a slot in front of a stream stage keeps its mask in
/// the front run — the masked slot is neither the unmasked one nor no slot, and an edit of its mask
/// makes the order ask the history again (the stream's addresses name the mask).
#[test]
fn t2550_a_masked_slot_in_front_keeps_its_mask() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let t = Tick(38 * STEP);
    let masked = |m: Option<Mask>| {
        let mut p = project_order(Some(3.0), None);
        let b = p.clips.iter_mut().find(|c| c.id == ClipId(2)).unwrap();
        if let Some(m) = m {
            b.masks.list_mut(Some(0)).push(m);
        }
        p.validate().unwrap();
        p
    };
    let with_mask = frame_at(&masked(Some(left_half())), t);
    assert_ne!(with_mask, frame_at(&masked(None), t), "the mask in front acted on the whole frame");
    assert_ne!(with_mask, frame_at(&project(true), t), "the masked slot in front did nothing");

    let memo = Arc::new(StreamMemo::new(256 << 20));
    let build = |p: &Project| {
        let mut vr = VideoRenderer::new(p);
        vr.set_stream_memo(memo.clone());
        vr
    };
    let shot_b = |vr: &VideoRenderer| vr.layers_at(t, Q).iter().filter(|(m, ..)| *m == MediaId(2)).count();
    let first = build(&masked(Some(left_half())));
    let cache = big_cache();
    warm(&first, &cache, t);
    assert!(kt_gpu::testkit::compose(&first, &cache, t, Q).1);
    assert_eq!(shot_b(&first), 0, "the memo holds the history");
    let mut moved = left_half();
    moved.shape = MaskShape::Rect { cx: 0.75, cy: 0.5, w: 0.5, h: 1.0, rot: 0.0, round: 0.0 };
    assert!(shot_b(&build(&masked(Some(moved)))) >= 7, "an edit of the mask in front feeds the whole history");
}

/// t2551 (chain-everywhere, found-defects 541): a Pan3d in front of a stream stage gets the camera the
/// layer would resolve — it is no longer skipped there, and the camera's edit is seen.
#[test]
fn t2551_a_pan3d_in_front_has_its_camera() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let t = Tick(38 * STEP);
    let with_cam = |rot_y: f64| {
        let mut p = project(true);
        p.cameras.push(CameraEvent {
            ease: true,
            id: CameraId(1),
            lane: 0,
            start: Tick(0),
            duration: Tick(90 * STEP),
            name: String::new(),
            cam: CameraParams { rot_y, ..CameraParams::default() },
            keyframes: vec![],
            auto: vec![],
        });
        let b = p.clips.iter_mut().find(|c| c.id == ClipId(2)).unwrap();
        b.fx.insert(0, FxInstance::builtin("pan3d"));
        p.ensure_tracks(LaneKind::Camera, 1);
        p.validate().unwrap();
        p
    };
    let turned = frame_at(&with_cam(35.0), t);
    assert_ne!(turned, frame_at(&project(true), t), "the Pan3d in front was skipped");
    assert_ne!(turned, frame_at(&with_cam(-35.0), t), "the camera of the Pan3d in front is not read");
}

/// The cut mosh whose vectors come from the track below: shot B (lane 0) moshes with `vectors_from`
/// = below; the event on lane 1 under it (media 3, hidden by the opaque shot B) carries `below`.
fn project_second(below: Vec<FxInstance>) -> Project {
    let mut p = project(true);
    p.media.push(MediaRef::new(MediaId(3), "synth:3"));
    let b = p.clips.iter_mut().find(|c| c.id == ClipId(2)).unwrap();
    for f in b.fx.iter_mut() {
        if f.is_stream() {
            assert!(f.set_param("vectors_from", 2.0));
        }
    }
    let mut under = clip(3, 3, 0, 60 * STEP, below);
    under.lane = 1;
    p.clips.push(under);
    p.ensure_tracks(LaneKind::Video, 2);
    p.validate().unwrap();
    p
}

/// Warm the order with media 3 from `c`, the rest from the textures.
fn frame_second(p: &Project, t: Tick, c: &mut dyn MediaSource) -> Vec<u8> {
    let (vr, cache) = (renderer(p), big_cache());
    let mut a = Tex::new(1, 0);
    let mut b = Tex::new(2, 2);
    for (m, pts, q) in vr.layers_at(t, Q) {
        let s: &mut dyn MediaSource = match m.0 {
            1 => &mut a,
            2 => &mut b,
            _ => &mut *c,
        };
        let f = s.read_video_frame(pts, Some(q)).unwrap();
        cache.insert_video(kt_engine::source_id(m), Some(q), kt_engine::video_cache_idx(pts), Arc::new(f));
    }
    let (f, complete) = kt_gpu::testkit::compose(&vr, &cache, t, Q);
    assert!(complete, "one warm-up of what the order names completes the frame");
    f
}

/// A source with `fx` already run over each frame, by the test engine at the size asked.
struct BakedWith {
    tex: Tex,
    fx: Vec<FxInstance>,
}

impl MediaSource for BakedWith {
    fn info(&self) -> &MediaInfo {
        self.tex.info()
    }
    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        self.tex.read_audio_burst(idx)
    }
    fn read_video_frame(&mut self, pts: Tick, quality: Option<Quality>) -> Result<VideoFrame, MediaError> {
        use kt_engine::VideoGpuExec as _;
        let f = Arc::new(self.tex.read_video_frame(pts, quality)?);
        let plan = kt_engine::chain_plan(f.clone(), &self.fx, 1, 0.0, pts, None);
        let mut out = vec![];
        assert!(kt_gpu::testkit::engine().execute_once(&plan, &mut out).unwrap());
        Ok(VideoFrame { pts, width: f.width, height: f.height, rgba: out.into_boxed_slice(), has_alpha: false })
    }
    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        self.tex.keyframe_before(pts)
    }
}

/// t2552 (chain-everywhere, found-defects 540): the stream's second input is read through its event's
/// chain — the slots of the mirrored event below run on the frames the kernel reads (the shot that
/// moshes has no slot in front, so every run counted is the second input's), and the mosh equals the
/// mosh of a source mirrored before decoding.
#[test]
fn t2552_the_second_input_runs_its_chain() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let t = Tick(38 * STEP);
    let mirror = || FxInstance::plugin_with("mirror", &[1.0, 0.0]).expect("mirror is built in");
    let runs = kt_engine::video::front_runs();
    let chained = frame_second(&project_second(vec![mirror()]), t, &mut Tex::new(3, 5));
    assert!(kt_engine::video::front_runs() > runs, "the chain below ran on the second input's frames");
    let baked = frame_second(&project_second(vec![]), t, &mut BakedWith { tex: Tex::new(3, 5), fx: vec![mirror()] });
    assert_eq!(chained, baked, "the second input reads the event below through its chain");
    // the mosh of this scene does not change with the second input's picture at all (found-defects
    // 580, the vectors from another track): the runs above are what proves the chain below is read
}

/// t2553 (chain-everywhere, found-defects 540): the stream's addresses name the second input's chain —
/// an edit of the slot on the event below makes the order ask the history again.
#[test]
fn t2553_an_edit_of_the_second_inputs_chain_invalidates_the_history() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let t = Tick(38 * STEP);
    let poster = |levels: f64| {
        let mut f = FxInstance::plugin("posterize").unwrap();
        assert!(f.set_param("levels", levels));
        f
    };
    let memo = Arc::new(StreamMemo::new(256 << 20));
    let build = |p: &Project| {
        let mut vr = VideoRenderer::new(p);
        vr.set_stream_memo(memo.clone());
        vr
    };
    let below = |vr: &VideoRenderer| vr.layers_at(t, Q).iter().filter(|(m, ..)| *m == MediaId(3)).count();
    let first = build(&project_second(vec![poster(3.0)]));
    let cache = big_cache();
    let mut a = Tex::new(1, 0);
    let mut b = Tex::new(2, 2);
    let mut c = Tex::new(3, 5);
    for (m, pts, q) in first.layers_at(t, Q) {
        let s: &mut dyn MediaSource = match m.0 {
            1 => &mut a,
            2 => &mut b,
            _ => &mut c,
        };
        let f = s.read_video_frame(pts, Some(q)).unwrap();
        cache.insert_video(kt_engine::source_id(m), Some(q), kt_engine::video_cache_idx(pts), Arc::new(f));
    }
    assert!(kt_gpu::testkit::compose(&first, &cache, t, Q).1);
    let held = below(&first);
    assert!(below(&build(&project_second(vec![poster(4.0)]))) > held, "an edit below fed no new history");
}

/// t2620 (found-defects 543): the output of the slots in front of a stream stage reaches the codec on
/// the compute device — the codec's jobs copy held frames there — and the frame is the one the bytes
/// path (readback, upload) gives.
#[test]
fn t2620_the_front_reaches_the_codec_on_the_device() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let t = Tick(38 * STEP);
    let p = project_order(Some(3.0), None);
    let frame = || {
        let mut vr = VideoRenderer::new(&p);
        vr.set_stream_memo(Arc::new(StreamMemo::new(256 << 20)));
        let c = big_cache();
        warm(&vr, &c, t);
        let (f, complete) = kt_gpu::testkit::compose(&vr, &c, t, Q);
        assert!(complete, "one warm-up of what the order names completes the frame");
        f
    };
    let reads = kt_gpu::compute::held_reads();
    let held = frame();
    assert!(kt_gpu::compute::held_reads() > reads, "the codec read the front's output where it lies");
    let bytes = kt_engine::video::with_front_bytes(frame);
    assert_eq!(held, bytes, "the held front gives the bytes of the read-back one");
}

/// The frames the pipelined export hands its sink, concatenated.
struct Frames(Vec<u8>);

struct Copy;

impl kt_engine::export::ExportConverter for Copy {
    type Frame = Vec<u8>;
    fn convert(&mut self, rgba: &[u8], out: &mut Vec<u8>) -> std::io::Result<()> {
        out.clear();
        out.extend_from_slice(rgba);
        Ok(())
    }
}

impl kt_engine::export::ExportSink for Frames {
    type Frame = Vec<u8>;
    type Converter = Copy;
    fn converter(&self) -> std::io::Result<Copy> {
        Ok(Copy)
    }
    fn alloc_frame(&self) -> std::io::Result<Vec<u8>> {
        Ok(Vec::new())
    }
    fn write_video(&mut self, f: &Vec<u8>) -> std::io::Result<()> {
        self.0.extend_from_slice(f);
        Ok(())
    }
    fn write_audio(&mut self, _s: &[f32]) -> std::io::Result<()> {
        Ok(())
    }
}

/// t2707 (stream-round-trips D6, found-defects 490): the pipelined export of a project with a
/// Datamosh — on one device and on a device per frame worker, over chunks of a few frames, from a range
/// starting before the stream and from one starting inside its history — writes the serial export's
/// frames.
#[test]
fn t2707_the_pipelined_export_walks_the_stream_in_order() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let p = project(true);
    let never = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // a chunk of about four frames at `Q`, the stream's history at its own size besides
    let budget = (Q.width * Q.height * 4 * 8) as usize;
    for range in [(26, 46), (36, 52)] {
        let serial = export(&p, range);
        for (name, backends) in [("one device", kt_gpu::testkit::backends()), ("three workers", kt_gpu::testkit::backends_many(3))] {
            let mut got = Frames(Vec::new());
            let r = (Tick(range.0 * STEP), Tick(range.1 * STEP));
            let st = kt_engine::export::render_pipelined_budget(&p, sources(), sources(), Q, Some(r), &backends, &never, None, budget, &mut got).unwrap();
            assert_eq!(st.missing_video_layers, 0, "{name} {range:?}: a frame lost a layer");
            assert_eq!(got.0.len(), serial.len(), "{name} {range:?}: the frame count");
            assert!(got.0 == serial, "{name} {range:?}: the pipelined frames differ from the serial export");
        }
    }
}

/// t3814 (stream-picture-stays D3, found-defects 663): the mosh's picture stays on the compute host's
/// device as the layer's input — an engine on that device composes it with no read back, and an engine
/// on another device reads it back; both give the same bytes, and the frame is moshed.
#[test]
fn t3814_a_held_stream_picture_composes_where_it_lies() {
    use kt_engine::VideoGpuExec;
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let Some(on_host) = kt_gpu::front_on_compute() else { return };
    // the host's device composes here as the window's would (no window in a test)
    kt_gpu::compute::hold_pictures_for_tests();
    let n = Tick(38 * STEP);
    let p = project(true);
    let (vr, cache) = (renderer(&p), big_cache());
    warm(&vr, &cache, n);
    let plan = vr.frame_plan(&cache, n, Q);
    let is_held = |plan: &kt_engine::FramePlan<'_>| plan.layers.iter().any(|l| matches!(l, kt_engine::video_plan::PlanLayer::One { src, .. } if src.input.held().is_some()));
    assert!(is_held(&plan), "the mosh layer's input is held");
    let (on, back) = kt_gpu::held_pictures();
    let mut here = vec![];
    assert!(on_host.execute_once(&plan, &mut here).unwrap(), "complete on the host's device");
    assert_eq!(kt_gpu::held_pictures(), (on + 1, back), "converted where it lies");
    drop(plan);
    let (there, complete) = kt_gpu::testkit::compose(&vr, &cache, n, Q);
    assert!(complete);
    assert_eq!(kt_gpu::held_pictures().1, back + 1, "another device reads it back");
    assert_eq!(here, there, "the same bytes either way");
    let plain = project(false);
    let (pv, pc) = (renderer(&plain), big_cache());
    warm(&pv, &pc, n);
    assert_ne!(kt_gpu::testkit::compose(&pv, &pc, n, Q).0, there, "the frame is moshed");
}

/// t3823 (stream-picture-stays D3, task 2.3): the frames a walk computes ahead stay on the compute
/// device too — the prefetch's walk over a run reads no picture back, and composing every frame of it
/// on the host's device reads none back either; a frame of the run is the bytes another device gets.
#[test]
fn t3823_the_frames_walked_ahead_stay_on_the_device() {
    use kt_engine::VideoGpuExec;
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let Some(on_host) = kt_gpu::front_on_compute() else { return };
    kt_gpu::compute::hold_pictures_for_tests();
    let p = project(true);
    let memo = Arc::new(StreamMemo::new(256 << 20));
    let mut vr = VideoRenderer::new(&p);
    vr.set_stream_memo(memo.clone());
    let cache = big_cache();
    for k in 0..=40 {
        warm(&vr, &cache, Tick(k * STEP));
    }
    let read = kt_plugin_api::stream::pictures_read();
    let walked = kt_plugin_api::stream::positions_walked();
    assert!(!vr.stream_advance_run(&cache, Tick(31 * STEP), Tick(40 * STEP), Q), "the run walked whole");
    assert!(kt_plugin_api::stream::positions_walked() > walked, "the run walked");
    assert_eq!(kt_plugin_api::stream::pictures_read(), read, "the walk ahead read no picture back");
    assert!(memo.device_bytes() > 0 && memo.device_bytes() <= memo.bytes(), "the memo names its device bytes: {}", memo.device_bytes());
    let (_, back) = kt_gpu::held_pictures();
    let mut at38 = vec![];
    for k in 31..=40 {
        let plan = vr.frame_plan(&cache, Tick(k * STEP), Q);
        let mut out = vec![];
        assert!(on_host.execute_once(&plan, &mut out).unwrap(), "frame {k} complete");
        if k == 38 {
            at38 = out;
        }
    }
    assert_eq!(kt_plugin_api::stream::pictures_read(), read, "composing the run read no picture back");
    assert_eq!(kt_gpu::held_pictures().1, back, "nothing read back by the executor");
    let (there, complete) = kt_gpu::testkit::compose(&vr, &cache, Tick(38 * STEP), Q);
    assert!(complete);
    assert_eq!(at38, there, "the frame walked ahead is the bytes another device reads back");
    // a megabyte more on the device is a `stream_held` record naming the memo's device bytes
    let items = kt_time::trace::testing::capture(|| memo.put_device(u64::MAX, Arc::new(0u8), 2 << 20));
    let held: Vec<_> = kt_time::trace::testing::recs(&items).into_iter().filter(|r| r.kind() == Some(kt_time::trace::Ev::StreamHeld)).collect();
    assert_eq!(held.last().map(|r| r.a), Some(memo.device_bytes() as u64), "{held:?}");
}

/// t3812 (stream-picture-stays D1): every mosh picture that crosses the bus to be composed leaves a
/// `stream_cross` record — the bytes read back, the bytes handed up, the conversion's span — where it
/// crosses: on two devices the kernel's bytes path for each frame composed; a project with no stream
/// stage crosses nothing.
#[test]
fn t3812_a_crossing_is_in_the_trace() {
    use kt_time::trace::{self, cross, Ev};
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let crossings = |p: &Project| {
        let (vr, cache) = (renderer(p), big_cache());
        for k in 0..=34 {
            warm(&vr, &cache, Tick(k * STEP));
        }
        let items = trace::testing::capture(|| {
            for k in 31..=34 {
                assert!(kt_gpu::testkit::compose(&vr, &cache, Tick(k * STEP), Q).1, "frame {k} complete");
            }
        });
        trace::testing::recs(&items).into_iter().filter(|r| r.kind() == Some(Ev::StreamCross)).collect::<Vec<_>>()
    };
    let moshed = crossings(&project(true));
    assert_eq!(moshed.len(), 4, "one crossing a frame composed: {moshed:?}");
    for r in &moshed {
        assert_eq!(r.aux, cross::KERNEL, "the kernel's bytes path");
        assert!(r.a > 0, "bytes read back");
        assert_eq!(r.b, (Q.width * Q.height * 4) as u64, "the layer's RGBA handed up");
    }
    assert!(crossings(&project(false)).is_empty(), "no stream stage, no crossing");
}

/// t3820 (stream-picture-stays D4): an export composed on the compute host's device takes the mosh's
/// pictures as they lie — no stream picture crosses the bus — and writes the frames an export on a
/// device of its own writes (which reads each held picture back).
#[test]
fn t3820_an_export_on_the_hosts_device_crosses_nothing() {
    use kt_time::trace::{self, cross, Ev};
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    kt_gpu::compute::hold_pictures_for_tests();
    let host = kt_gpu::compute::host_device().expect("this crate's host");
    let on_host = kt_gpu::GpuFxEngine::new_for_export(host.context()).unwrap();
    let p = project(true);
    let run = |backends: &kt_engine::export::ExportBackends| {
        let mut out = vec![];
        let items = trace::testing::capture(|| {
            kt_engine::export::render_raw_video_range_with(
                &p,
                sources(),
                Q,
                Some((Tick(26 * STEP), Tick(40 * STEP))),
                backends,
                &Arc::new(std::sync::atomic::AtomicBool::new(false)),
                |_| {},
                |f| {
                    out.extend_from_slice(f);
                    Ok(())
                },
            )
            .unwrap();
        });
        let crossed: Vec<_> = trace::testing::recs(&items).into_iter().filter(|r| r.kind() == Some(Ev::StreamCross)).collect();
        (out, crossed)
    };
    let here = kt_engine::export::ExportBackends { video_gpu: Some(on_host as Arc<dyn kt_engine::VideoGpuExec>), ..Default::default() };
    let (a, crossed_a) = run(&here);
    assert!(crossed_a.is_empty(), "the host's device crosses nothing: {crossed_a:?}");
    let (b, crossed_b) = run(&kt_gpu::testkit::backends());
    assert!(!crossed_b.is_empty() && crossed_b.iter().all(|r| r.aux == cross::EXECUTOR), "a device of its own reads the pictures back");
    assert_eq!(a.len(), b.len());
    assert!(a == b, "the two exports write the same frames");
}

/// The scene of t3228: shot B merges its blurred trunk with a posterized Datamosh arm (`arm`), or is
/// the arm's path alone as a line.
fn project_arm(arm: bool) -> Project {
    use kt_model::fx_graph::{FxIn, SlotInputs};
    let mut p = project_order(Some(3.0), None);
    let b = p.clips.iter_mut().find(|c| c.id == ClipId(2)).unwrap();
    if arm {
        // blur(0: source)  posterize(1: source) datamosh(2)  merge(3: bg blur, fg datamosh, Normal)
        let line = std::mem::take(&mut b.fx);
        b.fx = vec![FxInstance::blur(6)];
        b.fx.extend(line);
        b.fx.push(FxInstance::Merge { mode: MergeMode::default(), mix: 1.0 });
        b.fx_in = vec![
            SlotInputs::default(),
            SlotInputs { main: FxIn::Source, ..Default::default() },
            SlotInputs::default(),
            SlotInputs { main: FxIn::Slot(0), second: Some(FxIn::Prev), matte: None },
        ];
    }
    p.validate().unwrap();
    p
}

/// Test t3228 (fx-graph-dag D2; capability `video-plan`): Datamosh in an arm keeps a history of its own,
/// keyed as its line's — its frames are those of the event whose chain is the arm's path (the event
/// before the cut, the slot in front and its masks included), after playing and after a cold seek; a
/// change of the trunk beside it asks no history again; and everything that sizes or walks the streams
/// (the run ahead, the checkpoints, the memo's want, the bytes of a position) counts the read's stage.
#[test]
fn t3228_datamosh_in_an_arm_keeps_its_own_history() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    if no_device() {
        return;
    }
    let t = Tick(38 * STEP);
    let (arm, line) = (project_arm(true), project_arm(false));
    // a Normal merge at full level over an opaque trunk is its Foreground: the arm's own picture
    let want = frame_at(&line, t);
    assert!(want != frame_at(&project(false), t), "scene: the mosh does something");
    assert!(frame_at(&arm, t) == want, "the arm is not the event with that line");
    // played up to it
    let (vr, cache) = (renderer(&arm), big_cache());
    let mut last = (vec![], false);
    for k in 0..=38 {
        warm(&vr, &cache, Tick(k * STEP));
        last = kt_gpu::testkit::compose(&vr, &cache, Tick(k * STEP), Q);
    }
    assert!(last.1 && last.0 == want, "playing up to the moment gives other bytes than the seek");

    // the trunk's edit leaves the arm's history where it is; an edit of the slot in front moves it
    let memo = Arc::new(StreamMemo::new(256 << 20));
    let build = |p: &Project| {
        let mut vr = VideoRenderer::new(p);
        vr.set_stream_memo(memo.clone());
        vr
    };
    let of_history = |vr: &VideoRenderer| vr.layers_at(t, Q).iter().filter(|(m, ..)| *m == MediaId(1)).count();
    let first = build(&arm);
    assert!(of_history(&first) > 0, "scene: the cold history reads the event before");
    let cache = big_cache();
    warm(&first, &cache, t);
    assert!(kt_gpu::testkit::compose(&first, &cache, t, Q).1);
    assert_eq!(of_history(&first), 0, "the memo holds the arm's history");
    let edited = |i: usize, f: FxInstance| {
        let mut p = arm.clone();
        p.clips.iter_mut().find(|c| c.id == ClipId(2)).unwrap().fx[i] = f;
        build(&p)
    };
    assert_eq!(of_history(&edited(0, FxInstance::blur(12))), 0, "an edit of the trunk threw the arm's history away");
    let mut post = FxInstance::plugin("posterize").unwrap();
    assert!(post.set_param("levels", 6.0));
    assert!(of_history(&edited(1, post)) > 0, "an edit in front of the arm's stream kept its history");

    // what sizes and walks the streams sees the read's stage as the line's
    let (a, l) = (renderer(&arm), renderer(&line));
    assert_eq!(a.stream_frame_bytes(t, Q), l.stream_frame_bytes(t, Q), "the bytes of a position");
    assert_eq!(a.stream_checkpoints(t, Q), l.stream_checkpoints(t, Q), "the checkpoints");
    let (from, to) = (Tick(30 * STEP), Tick(45 * STEP));
    let (wa, wl) = (a.stream_want(from, to, Q), l.stream_want(from, to, Q));
    assert!(wl.positions > 0, "scene: the line's stage wants positions");
    assert_eq!((wa.positions, wa.lap_bytes, wa.per_position, wa.checkpoint_bytes), (wl.positions, wl.lap_bytes, wl.per_position, wl.checkpoint_bytes), "the memo's want");
    // the run ahead walks it: the moments of the run are in the memo after it
    let cache = big_cache();
    for k in 30..=40 {
        warm(&a, &cache, Tick(k * STEP));
    }
    assert!(!a.stream_advance_run(&cache, Tick(36 * STEP), Tick(40 * STEP), Q), "the run ahead left the arm's stage short");
    assert_eq!(of_history(&a), 0, "the run ahead did not walk the arm's history");
    // both on the trunk and in an arm: two histories, both wanted
    let mut both = arm.clone();
    let c = both.clips.iter_mut().find(|c| c.id == ClipId(2)).unwrap();
    let mosh = c.fx[2].clone();
    c.fx[0] = mosh;
    both.validate().unwrap();
    assert_eq!(renderer(&both).stream_want(from, to, Q).positions, 2 * wl.positions, "two stages of one event count as one");
}

/// Test t3263 (found-defects 843): the slots in front of a stream stage run on its input, so their zoom asks
/// nothing of this layer's decode step — a Datamosh after a zooming Geometry got its picture at the step's
/// size with nothing left in the chain to bring it back: a panic in a debug build, in a release one a layer
/// that never came. The frame composes; a zoom AFTER the stage asks its step as before.
#[test]
fn t3263_a_zoom_in_front_of_a_stream_asks_no_decode_step() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) || no_device() {
        return;
    }
    let zoomed = |front: bool| {
        let mut p = project(true);
        let b = p.clips.iter_mut().find(|c| c.id == ClipId(2)).unwrap();
        let at = if front { 0 } else { b.fx.len() };
        b.fx.insert(at, FxInstance::geometry_zoom(2.0));
        p.validate().unwrap();
        let big = [MediaId(1), MediaId(2)].map(|m| (m, (Q.width * 4, Q.height * 4)));
        let mut vr = VideoRenderer::new_with_natives(&p, &std::collections::HashMap::from(big));
        vr.set_stream_memo(Arc::new(StreamMemo::new(256 << 20)));
        vr
    };
    let t = Tick(38 * STEP);
    for front in [true, false] {
        let (vr, c) = (zoomed(front), big_cache());
        warm_from(&vr, &c, t, &mut Tex::new(1, 0), &mut Tex::new(2, 2));
        let (_, complete) = kt_gpu::testkit::compose(&vr, &c, t, Q);
        assert!(complete, "front {front}: the moshed layer has no picture");
    }
}
