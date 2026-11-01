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
    }
}

/// `mosh` — whether the second shot carries the datamosh slot.
fn project(mosh: bool) -> Project {
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
        for (name, v) in [("lines", 0.0), ("before", 12.0), ("drop_count", 1.0), ("gop", 12.0), ("qp", 14.0)] {
            assert!(f.set_param(name, v), "{name}");
        }
        fx.push(f);
    }
    p.clips.push(clip(1, 1, 0, 30 * STEP, vec![]));
    p.clips.push(clip(2, 2, 30 * STEP, 30 * STEP, fx));
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
    let n = Tick(38 * STEP);
    let p = project(true);
    let (mut played, cache) = (renderer(&p), big_cache());
    let mut last = (vec![], false);
    for k in 0..=38 {
        let t = Tick(k * STEP);
        warm(&played, &cache, t);
        last = played.compose_from_cache(&cache, t, Q);
    }
    assert!(last.1, "the played frame is complete");
    let (mut seek, cold) = (renderer(&p), big_cache());
    warm(&seek, &cold, n);
    let sought = seek.compose_from_cache(&cold, n, Q);
    assert!(sought.1, "one warm-up of what the order names completes the frame");
    assert_eq!(sought.0, last.0, "a seek gives the bytes of playing");
    // and the slot does something: without it the frame is B itself
    let plain = project(false);
    let (mut vr, c) = (renderer(&plain), big_cache());
    warm(&vr, &c, n);
    assert_ne!(vr.compose_from_cache(&c, n, Q).0, last.0);
}

fn export(p: &Project, range: (i64, i64)) -> Vec<u8> {
    let mut out = vec![];
    kt_engine::export::render_raw_video_range(
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
    let p = project(true);
    let whole = export(&p, (26, 46));
    let mut parts = export(&p, (26, 33));
    parts.extend(export(&p, (33, 41)));
    parts.extend(export(&p, (41, 46)));
    assert_eq!(parts.len(), whole.len());
    assert!(parts == whole, "the pieces differ from the whole");
}

/// t1248: the order of a stream layer is its codec-size stream: the history (the event before
/// included) with an empty memo, then only the next frame's own source.
#[test]
fn t1248_the_order_asks_what_the_stage_reads() {
    let p = project(true);
    let (mut vr, cache) = (renderer(&p), big_cache());
    let t = Tick(36 * STEP);
    let order = vr.layers_at(t, Q);
    let cq = order.iter().find(|(m, ..)| *m == MediaId(2)).expect("the mosh orders its source").2;
    assert_eq!(cq.height, 144, "the stream is ordered at its encoded height");
    assert!(order.iter().all(|(_, _, q)| *q == cq), "no frame at the layer's ordinary quality: {order:?}");
    assert!(order.iter().any(|(m, ..)| *m == MediaId(1)), "the history reaches into the event before");
    assert!(order.iter().filter(|(m, ..)| *m == MediaId(2)).count() >= 7, "the history from the cut");
    warm(&vr, &cache, t);
    assert!(vr.compose_from_cache(&cache, t, Q).1);
    let next = Tick(37 * STEP);
    // the source pts is the material position: the clip starts at 1 s with `source_in` 0
    let pts = Tick(next.0 - 30 * STEP);
    assert_eq!(vr.layers_at(next, Q), vec![(MediaId(2), pts, cq)], "steady state: the next frame only");
    assert_eq!(vr.video_decode_qualities(Q).iter().find(|(id, _)| *id == ClipId(2)).unwrap().1, cq);
}
