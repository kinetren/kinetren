//! The STREAM source stage in the plan (datamosh-codec-path, design D1–D6).
//!
//! A clip whose chain holds a `VideoClass::Stream` slot takes as its pre-FX frame what the stage's
//! kernel produces from a stream of the event's source frames (the event before included, the
//! event on another track as a second input). The stage runs here, in the plan's common code, on
//! the CPU — both executors receive the same decoded bytes (invariant 7). The order asks exactly
//! what the kernel says it still needs (invariant 4); the bytes do not depend on the memo
//! (invariant 11).

use std::sync::{Arc, OnceLock};

use kt_cache::{BurstCache, SourceId};
use kt_media::{Quality, VideoFrame};
use kt_model::MediaId;
use kt_plugin_api::stream::{mix, Rgba, SecondFrom, Side, StreamInput, StreamKernel, StreamLayout, StreamMemo};
use kt_time::{FpsGrid, Tick};

use super::{clip_base_quality, clip_frame_keys, video_cache_idx, VideoClip};

/// The preview's ONE memo (design D3): the composer, the prefetch and the picker share it, so
/// "what is still needed" and "what was computed" see the same entries. Each export owns its own.
pub fn stream_memo() -> &'static Arc<StreamMemo> {
    static M: OnceLock<Arc<StreamMemo>> = OnceLock::new();
    M.get_or_init(|| Arc::new(StreamMemo::new(256 << 20)))
}

/// A clip's stream stage: the slot's plugin and rows, the event before on the same lane and the
/// events of the second input's lane (clones made when the renderer is built — the renderer is
/// rebuilt on every edit, so they cannot go stale).
#[derive(Clone)]
pub(super) struct StreamStage {
    pub plugin: &'static Arc<dyn kt_plugin_api::Plugin>,
    pub values: Vec<f64>,
    pub before: Option<Box<VideoClip>>,
    pub second: Vec<VideoClip>,
}

impl std::fmt::Debug for StreamStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamStage")
            .field("id", &self.plugin.manifest().id)
            .field("values", &self.values)
            .field("before", &self.before.as_ref().map(|c| c.id))
            .field("second", &self.second.len())
            .finish()
    }
}

impl StreamStage {
    /// The stage of a chain: the first active `Stream` slot whose plugin is registered.
    pub fn of(fx: &[kt_model::FxInstance]) -> Option<StreamStage> {
        fx.iter().find_map(|f| match f {
            kt_model::FxInstance::Plugin(p) if f.is_stream() => {
                p.plugin().map(|plugin| StreamStage { plugin, values: p.values.clone(), before: None, second: vec![] })
            }
            _ => None,
        })
    }

    pub fn kernel(&self) -> &'static dyn StreamKernel {
        self.plugin.stream().expect("a stream slot's plugin has a stream kernel (registry, t1240)")
    }
}

/// Fill each stage's event before and second input (after the composition sort).
pub(super) fn link_stages(clips: &mut [VideoClip]) {
    let snapshot: Vec<VideoClip> = clips.to_vec();
    let lite = |c: &VideoClip| {
        let mut c = c.clone();
        c.stream = None;
        c
    };
    for c in clips.iter_mut() {
        let Some(st) = c.stream.as_mut() else { continue };
        let k = st.kernel();
        if k.before(&st.values) > 0 {
            st.before = snapshot
                .iter()
                .find(|o| o.lane == c.lane && o.end == c.start && o.nest.is_none() && o.id != c.id)
                .map(|o| Box::new(lite(o)));
        }
        let lane = match k.second(&st.values) {
            SecondFrom::None => None,
            SecondFrom::TrackAbove => c.lane.checked_sub(1),
            SecondFrom::TrackBelow => Some(c.lane + 1),
        };
        if let Some(l) = lane {
            st.second = snapshot.iter().filter(|o| o.lane == l && o.nest.is_none()).map(lite).collect();
        }
    }
}

/// The size the stream is ordered and read at: `lines` high, the aspect of the layer's frame.
pub(super) fn stream_quality(c: &VideoClip, st: &StreamStage, quality: Quality) -> Quality {
    let base = clip_base_quality(c, quality);
    let h = st.kernel().lines(&st.values).max(16);
    let w = ((h as u64 * base.width.max(2) as u64 + base.height.max(2) as u64 / 2) / base.height.max(2) as u64) as u32;
    Quality { width: (w.max(16) + 1) & !1, height: h }
}

fn ceil_div(a: i64, b: i64) -> i64 {
    -((-a).div_euclid(b))
}

/// One clip's stream as the kernel sees it (positions = project grid frame indices).
pub(super) struct ClipStream<'a> {
    clip: &'a VideoClip,
    st: &'a StreamStage,
    grid: &'a FpsGrid,
    pub q: Quality,
    step: i64,
    first: i64,
    cut: i64,
    end: i64,
    cache: Option<&'a BurstCache>,
}

impl<'a> ClipStream<'a> {
    pub fn new(clip: &'a VideoClip, st: &'a StreamStage, grid: &'a FpsGrid, quality: Quality, cache: Option<&'a BurstCache>) -> Self {
        let step = grid.frame_step().max(1);
        let cut = ceil_div(clip.start.0, step);
        let end = ceil_div(clip.end.0, step);
        // the stream starts in the event before (its last `before` frames), or at the first frame
        // of the event's own handles when it has none
        let first = match &st.before {
            Some(b) => cut - (st.kernel().before(&st.values) as i64).min(cut - ceil_div(b.start.0, step)).max(0),
            None => ceil_div(clip.start.0 - clip.head_ext.0, step).min(cut),
        };
        ClipStream { clip, st, grid, q: stream_quality(clip, st, quality), step, first, cut, end, cache }
    }

    /// The position of a frame time.
    pub fn pos(&self, t: Tick) -> i64 {
        t.0.div_euclid(self.step)
    }

    /// The clip and source pts a position reads on a side.
    pub fn at(&self, side: Side, k: i64) -> Option<(&'a VideoClip, Tick)> {
        let t = Tick(k * self.step);
        let c: &'a VideoClip = match side {
            Side::Main if k < self.cut => self.st.before.as_deref().unwrap_or(self.clip),
            Side::Main => self.clip,
            Side::Second => self.st.second.iter().find(|o| o.start <= t && t < o.end)?,
        };
        Some((c, clip_frame_keys(c, t, self.grid, false).0))
    }

    /// The order of positions: (media, pts, the stream's size).
    pub fn order(&self, needs: &[(Side, i64)], out: &mut Vec<(MediaId, Tick, Quality)>) {
        for &(side, k) in needs {
            if let Some((c, pts)) = self.at(side, k) {
                out.push((c.media, pts, self.q));
            }
        }
    }
}

impl StreamLayout for ClipStream<'_> {
    fn size(&self) -> (u32, u32) {
        (self.q.width, self.q.height)
    }
    fn first(&self) -> i64 {
        self.first
    }
    fn cut(&self) -> i64 {
        self.cut
    }
    fn end(&self) -> i64 {
        self.end
    }
    fn second_start(&self, k: i64) -> Option<i64> {
        let t = Tick(k * self.step);
        let o = self.st.second.iter().find(|o| o.start <= t && t < o.end)?;
        Some(ceil_div(o.start.0, self.step))
    }
    fn ident(&self, side: Side, k: i64) -> u64 {
        let Some((c, pts)) = self.at(side, k) else { return 0 };
        let mut h = mix(0x5754_7245_414d, c.media.0);
        h = mix(h, pts.0 as u64);
        h = mix(h, ((self.q.width as u64) << 32) | self.q.height as u64);
        mix(h, c.src_rev).max(1)
    }
}

impl StreamInput for ClipStream<'_> {
    fn with_frame(&self, side: Side, k: i64, f: &mut dyn FnMut(Rgba<'_>)) -> bool {
        let Some(cache) = self.cache else { return false };
        let Some((c, pts)) = self.at(side, k) else { return false };
        match cache.get_video(SourceId(c.media.0), Some(self.q), video_cache_idx(pts)) {
            Some(fr) => {
                f(Rgba { w: fr.width, h: fr.height, rgba: &fr.rgba });
                true
            }
            None => false,
        }
    }
}

/// Run the stage for frame time `t`: the decoded picture fitted to `dq` (opaque) and its key;
/// `(None, 0)` — a frame of the history is missing (starvation; the order asks for it).
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    clip: &VideoClip,
    st: &StreamStage,
    grid: &FpsGrid,
    cache: &BurstCache,
    memo: &StreamMemo,
    quality: Quality,
    dq: Quality,
    t: Tick,
    pts: Tick,
) -> (Option<Arc<VideoFrame>>, u64) {
    let cs = ClipStream::new(clip, st, grid, quality, Some(cache));
    let mut buf = Vec::new();
    let Some(out) = st.kernel().render(&st.values, &cs, cs.pos(t), memo, &mut buf) else {
        return (None, 0);
    };
    let src = kt_fx::video::Frame { w: out.w, h: out.h, rgba: buf, has_alpha: false };
    let mut rgba = vec![0u8; (dq.width * dq.height * 4) as usize];
    kt_fx::video::fit_into(&src, dq.width, dq.height, &mut rgba);
    let frame = VideoFrame { pts, width: dq.width, height: dq.height, rgba: rgba.into_boxed_slice(), has_alpha: false };
    (Some(Arc::new(frame)), out.key)
}
