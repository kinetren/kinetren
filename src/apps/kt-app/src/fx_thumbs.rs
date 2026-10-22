//! FxThumbService: effect thumbnails for the picker (R.54, item 430).
//!
//! The shape is a SERVICE modelled on `ThumbService` (`bg.rs` calls it the reference for this shape):
//! its own thread, a condvar queue, LIFO + dedup, RAM-LRU,
//! cancellation by generation. The task frame (`BgJob`)
//! does not fit here: a task is finite and has progress,
//! while thumbnails are an endless queue of small requests,
//! living as long as the picker is open.
//!
//! What the service does NOT have and must not have:
//! * decoders and sidecar DB. The base frame is resolved by the CALLER and passed as a ready picture
//!   (or it asks for the built-in sample). Invariant No. 8 holds: drawing may only ENQUEUE requests,
//!   and the decoding is done by a foreign dedicated worker — `ThumbService`;
//! * persistence. The expensive half of the preview (the decode) already lies as a JPEG in `thumbnails`;
//!   applying the chain to a ready frame is cheaper than reading the result from sqlite,
//!   so the cache here is only RAM and only for the session.

use eframe::egui;
use kt_model::{FxInstance, MediaId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The reference size of the computation is the size of the source thumbnail (`thumbs::THUMB_W/H`).
/// Computing right at the cell size is not allowed: some parameters are set in PIXELS (`Pixelate.size`,
/// blur radii), and in a small frame they would show a different effect STRENGTH
/// (“a preview of the kind, not of the strength”, item 428).
pub const REF_W: u32 = crate::thumbs::THUMB_W;
pub const REF_H: u32 = crate::thumbs::THUMB_H;
/// The stored cell size (twice the drawn one — hidpi).
pub const CELL_W: u32 = 160;
pub const CELL_H: u32 = 90;
/// LRU of ready thumbnails: 160×90 RGBA ≈ 57 KB, that is ~14 MB at the ceiling.
pub const MAX_READY_FX: usize = 256;
/// Misclick guard (R.54, the owner’s requirement “only try to do it a second later (in case it was a misclick)”).
/// It waits NOT for the render — that is cheap and goes at
/// once — but for the decode ORDERS: asking for the thumbnail
/// of a missing frame before this deadline means paying for opening the file for a random click.
pub const DECODE_GUARD: std::time::Duration = std::time::Duration::from_millis(800);
/// The pause between renders when the system is busy (game, export): the queue is LIFO,
/// so fresh cells are still first anyway.
const BUSY_PAUSE: std::time::Duration = std::time::Duration::from_millis(25);

/// What to compute the thumbnail on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FxThumbBase {
    /// The source frame: the `ThumbService` key — the SAME as the filmstrip’s
    /// (a shared cache, it produces no new decodes).
    Thumb(crate::thumbs::ThumbKey),
    /// The built-in sample (`kt_fx::video::test_pattern`):
    /// a nested clip, a morpher without a source,
    /// a thumbnail not yet arrived — empty cells do not occur.
    Pattern,
    /// The same, with TRANSPARENT fields: `DropShadow` on an
    /// opaque frame is an identity by construction (item 428b).
    PatternAlpha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FxThumbKey {
    pub base: FxThumbBase,
    /// Recipe hash: chain + moment + camera (see `recipe_hash`).
    pub recipe: u64,
}

impl FxThumbKey {
    /// Whether the cell computes a frame of this media. Needed when reconfiguring a generator:
    /// the picture changed, but the key did not (item 412).
    pub fn is_media(&self, id: MediaId) -> bool {
        match self.base {
            FxThumbBase::Thumb(t) => t.media == id,
            FxThumbBase::Pattern | FxThumbBase::PatternAlpha => false,
        }
    }
}

/// The thumbnail recipe in one line. The `Debug` representation is hashed, not serde: in `FxInstance`
/// half the fields sit under `skip_serializing_if`, and
/// a new field with a default from JSON would drop out —
/// from `Debug` it will not drop out (the same trick as `fx_chain_hash` in kt-engine).
pub fn recipe_hash(
    chain: &[FxInstance],
    u: f64,
    burst_index: i64,
    cam3d: Option<kt_fx::geom::Cam3d>,
) -> u64 {
    let repr = format!("{chain:?}|{u:?}|{burst_index}|{cam3d:?}|{REF_W}x{REF_H}>{CELL_W}x{CELL_H}");
    xxhash_rust::xxh3::xxh3_64(repr.as_bytes())
}

/// Effects visible ONLY on transparency: the demo sample needs transparent fields for them, otherwise the cell
/// will show the source frame. `DropShadow` on an opaque
/// frame is an identity BY CONSTRUCTION — the shadow falls
/// from under the alpha, and there is no alpha (item 428b, guard t544d).
///
/// On a REAL frame of an event we do not substitute: there
/// passthrough is honest — for an opaque clip the shadow
/// really changes nothing, and one must not lie about it in the picker.
fn needs_alpha(fx: &[FxInstance]) -> bool {
    fx.iter().any(|f| matches!(f, FxInstance::DropShadow { .. }))
}

/// A thumbnail request. The base frame arrives READY: the worker goes neither to foreign caches
/// nor to decoders.
pub struct FxThumbJob {
    pub key: FxThumbKey,
    /// `None` — the base from `FxThumbBase::Pattern*`.
    pub base_img: Option<Arc<egui::ColorImage>>,
    /// The chain with curves ALREADY resolved (values at the moment of the cursor).
    pub chain: Vec<FxInstance>,
    pub u: f64,
    pub burst_index: i64,
    pub cam3d: Option<kt_fx::geom::Cam3d>,
    pub generation: u64,
}

/// The request queue: LIFO + dedup by key + cancellation by
/// generation. A separate type rather than a pair of fields —
/// modelled on `SourceLru`: this way it is checked by a unit test without a thread and without egui (t546).
#[derive(Default)]
pub(crate) struct JobQueue {
    jobs: VecDeque<FxThumbJob>,
    seen: HashSet<FxThumbKey>,
}

impl JobQueue {
    pub(crate) fn seen_key(&self, key: &FxThumbKey) -> bool {
        self.seen.contains(key)
    }
}

impl JobQueue {
    /// true — the request is accepted (new). A duplicate is dropped: the same cell asks on every frame
    /// of the interface until it is ready.
    pub(crate) fn push(&mut self, job: FxThumbJob) -> bool {
        if !self.seen.insert(job.key) {
            return false;
        }
        self.jobs.push_back(job);
        true
    }

    /// Fresh first (`pop_back`); requests of past generations are dropped silently — the picker was closed
    /// or the cursor moved away, there is no point computing them.
    pub(crate) fn pop(&mut self, generation: u64) -> Option<FxThumbJob> {
        while let Some(job) = self.jobs.pop_back() {
            self.seen.remove(&job.key);
            if job.generation >= generation {
                return Some(job);
            }
        }
        None
    }

    pub(crate) fn len(&self) -> usize {
        self.jobs.len()
    }
}

pub(crate) struct Shared {
    cond: std::sync::Condvar,
    queue: Mutex<JobQueue>,
    ready: Mutex<HashMap<FxThumbKey, Arc<egui::ColorImage>>>,
    order: Mutex<VecDeque<FxThumbKey>>,
    generation: AtomicU64,
    /// The system is busy (game/export) — “slow down” (rule 12
    /// of the checklist: background work does not get in the way).
    busy: AtomicBool,
    /// The owner has died — the thread must exit (the `bg.rs` doctrine:
    /// the task and the service are shut down together with their owner).
    stop: AtomicBool,
}

impl Shared {
    fn new() -> Arc<Shared> {
        Arc::new(Shared {
            cond: std::sync::Condvar::new(),
            queue: Mutex::new(JobQueue::default()),
            ready: Mutex::new(HashMap::new()),
            order: Mutex::new(VecDeque::new()),
            generation: AtomicU64::new(0),
            busy: AtomicBool::new(false),
            stop: AtomicBool::new(false),
        })
    }

    fn publish(&self, key: FxThumbKey, img: Arc<egui::ColorImage>) {
        let mut ready = self.ready.lock().unwrap();
        let mut order = self.order.lock().unwrap();
        if ready.insert(key, img).is_none() {
            order.push_back(key);
        }
        while order.len() > MAX_READY_FX {
            if let Some(old) = order.pop_front() {
                ready.remove(&old);
            }
        }
    }
}

/// The egui texture cache of the cells. It lives IN THE SERVICE, not in `KtApp`, for reasons of borrowing:
/// the FX and morpher windows hold `&self.model` inside the ui closures (and the morpher also calls its methods
/// through `&self`), and a `&mut` to a field of the application does not fit there any more.
/// `RefCell`, not `Mutex`: the textures are touched ONLY by the UI thread — the worker knows nothing about them,
/// it hands over a `ColorImage`.
#[derive(Default)]
struct TexCache {
    map: HashMap<FxThumbKey, egui::TextureHandle>,
    lru: VecDeque<FxThumbKey>,
}

pub struct FxThumbService {
    shared: Arc<Shared>,
    tex: std::cell::RefCell<TexCache>,
}

impl FxThumbService {
    pub fn start() -> FxThumbService {
        let shared = Shared::new();
        let sh = shared.clone();
        std::thread::Builder::new()
            .name("fxthumbs".into())
            .spawn(move || worker(sh))
            .expect("spawn fxthumbs");
        FxThumbService { shared, tex: std::cell::RefCell::new(TexCache::default()) }
    }

    /// Enqueue a request (dedup). Drawing is never blocked.
    pub fn request(&self, job: FxThumbJob) {
        if self.shared.ready.lock().unwrap().contains_key(&job.key) {
            return;
        }
        let mut q = self.shared.queue.lock().unwrap();
        if q.push(job) {
            drop(q);
            self.shared.cond.notify_one();
        }
    }

    pub fn get(&self, key: &FxThumbKey) -> Option<Arc<egui::ColorImage>> {
        self.shared.ready.lock().unwrap().get(key).cloned()
    }

    /// The current generation (carried by the requests).
    pub fn generation(&self) -> u64 {
        self.shared.generation.load(Ordering::Relaxed)
    }

    /// The picker was closed / the cursor moved: everything in the queue is no longer needed to compute.
    /// READY thumbnails stay — their LRU is limited anyway,
    /// and reopening the picker usually asks for the same cells.
    pub fn bump_generation(&self) {
        self.shared.generation.fetch_add(1, Ordering::Relaxed);
        self.shared.cond.notify_all();
    }

    /// The media stayed the same, but the PICTURE changed
    /// (an edit of the generator parameters, R.53 item 412):
    /// the cell key remembers the `ThumbKey`, which does not depend on the parameters, so this media’s ready
    /// thumbnails are stale. Called next to `ThumbService::retune_media`.
    pub fn invalidate_media(&self, id: MediaId) {
        self.shared.ready.lock().unwrap().retain(|k, _| !k.is_media(id));
        self.shared.order.lock().unwrap().retain(|k| !k.is_media(id));
        let mut tex = self.tex.borrow_mut();
        tex.map.retain(|k, _| !k.is_media(id));
        tex.lru.retain(|k| !k.is_media(id));
    }

    /// The texture of a ready cell (uploaded at the first display) or `None`.
    fn texture_for(&self, ctx: &egui::Context, key: FxThumbKey) -> Option<egui::TextureId> {
        if let Some(h) = self.tex.borrow().map.get(&key) {
            return Some(h.id());
        }
        let img = self.get(&key)?;
        let h =
            ctx.load_texture(format!("fxthumb{}", key.recipe), img, egui::TextureOptions::LINEAR);
        let id = h.id();
        let mut tex = self.tex.borrow_mut();
        tex.map.insert(key, h);
        tex.lru.push_back(key);
        while tex.lru.len() > MAX_READY_FX {
            if let Some(old) = tex.lru.pop_front() {
                tex.map.remove(&old);
            }
        }
        Some(id)
    }

    /// System busy state: playing or export in progress.
    pub fn set_busy(&self, busy: bool) {
        self.shared.busy.store(busy, Ordering::Relaxed);
    }

    /// Whether there are unfinished requests. The redraw
    /// gate hangs on THIS, not on “the picker is open”:
    /// otherwise an open menu would hold 60 fps while idle (defect t38).
    pub fn pending(&self) -> bool {
        self.shared.queue.lock().unwrap().len() > 0
    }

    /// Already queued? So that drawing does not assemble the
    /// request anew on every frame while the cell is being computed
    /// (dedup inside the queue saves from work, but not from assembling the job).
    pub fn queued(&self, key: &FxThumbKey) -> bool {
        self.shared.queue.lock().unwrap().seen_key(key)
    }
}

/// The cell context of the picker for ONE frame: the service, the texture cache and everything
/// that sets the recipe (base frame, clip chain, moment, camera).
///
/// It lives outside `Window::show`, and is passed inside by one reference: the window holds `&self.model`,
/// and these two borrows cannot get tangled — the fields are
/// different (the same trick as the copy buffers in `fx_editor`).
pub struct PickerCtx<'a> {
    pub svc: &'a FxThumbService,
    /// What to compute on (source frame / built-in sample).
    pub base: FxThumbBase,
    /// A ready base frame; for `Pattern*` — `None`.
    pub base_img: Option<Arc<egui::ColorImage>>,
    /// The CLIP chain with curves already resolved — what stands on the event now. The cell’s candidate
    /// is appended to the tail, so the picture shows the
    /// result of the click, not the effect “in a vacuum”.
    pub chain: Vec<FxInstance>,
    pub u: f64,
    pub burst_index: i64,
    pub cam3d: Option<kt_fx::geom::Cam3d>,
    pub generation: u64,
    /// A reusable buffer “chain + candidate”: there
    /// are tens of cells in a strip, and a Vec for each
    /// would be an allocation in the draw path per frame (invariant No. 12).
    full: Vec<FxInstance>,
}

impl<'a> PickerCtx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        svc: &'a FxThumbService,
        base: FxThumbBase,
        base_img: Option<Arc<egui::ColorImage>>,
        chain: Vec<FxInstance>,
        u: f64,
        burst_index: i64,
        cam3d: Option<kt_fx::geom::Cam3d>,
    ) -> PickerCtx<'a> {
        let generation = svc.generation();
        PickerCtx {
            svc,
            base,
            base_img,
            chain,
            u,
            burst_index,
            cam3d,
            generation,
            full: Vec::new(),
        }
    }

    /// The cell texture for the chain “clip + candidate”, or `None` — then the request is queued and the cell
    /// is drawn as a placeholder until it is ready.
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        extra: &[FxInstance],
    ) -> Option<egui::TextureId> {
        self.full.clear();
        self.full.extend_from_slice(&self.chain);
        self.full.extend_from_slice(extra);
        let base = match self.base {
            FxThumbBase::Pattern if needs_alpha(&self.full) => FxThumbBase::PatternAlpha,
            b => b,
        };
        let key = FxThumbKey {
            base,
            recipe: recipe_hash(&self.full, self.u, self.burst_index, self.cam3d),
        };
        if let Some(id) = self.svc.texture_for(ctx, key) {
            return Some(id);
        }
        if !self.svc.queued(&key) {
            self.svc.request(FxThumbJob {
                key,
                base_img: self.base_img.clone(),
                chain: self.full.clone(),
                u: self.u,
                burst_index: self.burst_index,
                cam3d: self.cam3d,
                generation: self.generation,
            });
        }
        None
    }
}

impl Drop for FxThumbService {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.cond.notify_all();
    }
}

/// Whether decode ORDERS (thumbnail/poster) are allowed — the misclick guard. A pure function:
/// the clock comes from outside, so the test does not sleep.
pub fn decode_orders_allowed(since_open: std::time::Duration) -> bool {
    since_open >= DECODE_GUARD
}

/// `ColorImage` → `Frame`. `Color32` stores the colour
/// PREMULTIPLIED, while the chain works with straight alpha
/// (the alpha rule, items 417–418): for a JPEG thumbnail the alpha is all 255 and that is an identity,
/// but for a generator-title frame it is not — and without unpacking a dark edge would get into the chain.
fn frame_from(img: &egui::ColorImage) -> kt_fx::video::Frame {
    let mut rgba = img.as_raw().to_vec();
    let has_alpha = rgba.chunks_exact(4).any(|p| p[3] != 255);
    if has_alpha {
        kt_fx::video::unpremultiply(&mut rgba);
    }
    kt_fx::video::Frame { w: img.width() as u32, h: img.height() as u32, rgba, has_alpha }
}

/// Rendering of one request. Taken out of the worker so that a test can call it directly —
/// without a thread and without waiting.
pub(crate) fn render_job(
    job: &FxThumbJob,
    scratch: &mut kt_fx::video::FxScratch,
) -> egui::ColorImage {
    let base = match (&job.base_img, job.key.base) {
        (Some(img), _) => frame_from(img),
        (None, FxThumbBase::PatternAlpha) => kt_fx::video::test_pattern(REF_W, REF_H, true),
        // the base has not arrived, but the key says “source frame”
        // — we compute on the sample: empty cells do not occur
        // (the key is then foreign, and when the frame arrives, the UI will ask for it under its own key)
        (None, _) => kt_fx::video::test_pattern(REF_W, REF_H, false),
    };
    // computation at the REFERENCE size, storage at the cell size (item 428)
    let full = kt_fx::video::render_fx_thumb(
        &base,
        &job.chain,
        REF_W,
        REF_H,
        job.u,
        job.burst_index,
        job.cam3d,
        scratch,
    );
    let mut cell = scratch.take((CELL_W as usize) * (CELL_H as usize) * 4);
    kt_fx::video::fit_into(&full, CELL_W, CELL_H, &mut cell);
    let img = egui::ColorImage::from_rgba_unmultiplied([CELL_W as usize, CELL_H as usize], &cell);
    scratch.put(cell);
    scratch.put(full.rgba);
    img
}

fn worker(shared: Arc<Shared>) {
    let mut scratch = kt_fx::video::FxScratch::default();
    loop {
        if shared.stop.load(Ordering::Relaxed) {
            return;
        }
        let job = {
            let mut q = shared.queue.lock().unwrap();
            loop {
                if shared.stop.load(Ordering::Relaxed) {
                    return;
                }
                let cur = shared.generation.load(Ordering::Relaxed);
                if let Some(job) = q.pop(cur) {
                    break job;
                }
                let (guard, _t) =
                    shared.cond.wait_timeout(q, std::time::Duration::from_secs(1)).unwrap();
                q = guard;
            }
        };
        if shared.ready.lock().unwrap().contains_key(&job.key) {
            continue;
        }
        let img = render_job(&job, &mut scratch);
        // the generation could change WHILE we were computing: publishing is still possible
        // (the key addresses exactly this picture), but computing
        // the next stale requests is not — pop() will drop them
        shared.publish(job.key, Arc::new(img));
        if shared.busy.load(Ordering::Relaxed) {
            std::thread::sleep(BUSY_PAUSE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(recipe: u64) -> FxThumbKey {
        FxThumbKey { base: FxThumbBase::Pattern, recipe }
    }

    fn job(recipe: u64, generation: u64) -> FxThumbJob {
        FxThumbJob {
            key: key(recipe),
            base_img: None,
            chain: vec![],
            u: 0.5,
            burst_index: 90_000,
            cam3d: None,
            generation,
        }
    }

    /// Test 546 (item 430): the thumbnail queue — LIFO, dedup and cancellation by generation.
    /// A duplicate must be dropped: the same cell asks
    /// on EVERY frame of the interface until it is ready,
    /// and without dedup the queue would grow in proportion to fps.
    #[test]
    fn t546_queue_is_lifo_dedup_and_cancels_by_generation() {
        let mut q = JobQueue::default();
        assert!(q.push(job(1, 0)));
        assert!(q.push(job(2, 0)));
        assert!(!q.push(job(1, 0)), "duplicate key was queued twice");
        assert_eq!(q.len(), 2);
        // fresh first
        assert_eq!(q.pop(0).expect("job").key.recipe, 2);
        assert_eq!(q.pop(0).expect("job").key.recipe, 1);
        assert!(q.pop(0).is_none());

        // a key removed from the queue can be ordered again (otherwise reopening the picker would not re-order
        // what had not managed to be computed)
        assert!(q.push(job(1, 0)));

        // cancellation by generation: requests of past generations are dropped
        assert!(q.push(job(3, 1)));
        assert_eq!(q.pop(1).expect("fresh job").key.recipe, 3);
        assert!(q.pop(1).is_none(), "a stale job survived the generation bump");
    }

    /// Publication and LRU: the ceiling holds, and the OLDEST is evicted.
    #[test]
    fn t546b_ready_lru_has_a_ceiling() {
        let shared = Shared::new();
        let img = || Arc::new(egui::ColorImage::new([2, 2], vec![egui::Color32::BLACK; 4]));
        for i in 0..(MAX_READY_FX as u64 + 10) {
            shared.publish(key(i), img());
        }
        assert_eq!(shared.ready.lock().unwrap().len(), MAX_READY_FX);
        assert!(shared.ready.lock().unwrap().get(&key(0)).is_none(), "oldest survived");
        assert!(shared.ready.lock().unwrap().get(&key(MAX_READY_FX as u64 + 9)).is_some());
        // republishing the same key does not grow the order queue
        let before = shared.order.lock().unwrap().len();
        shared.publish(key(MAX_READY_FX as u64 + 9), img());
        assert_eq!(shared.order.lock().unwrap().len(), before);
    }

    /// The recipe distinguishes exactly what the picture depends on.
    #[test]
    fn t546c_recipe_separates_what_changes_the_picture() {
        let a = vec![FxInstance::blur(8)];
        let b = vec![FxInstance::blur(9)];
        let base = recipe_hash(&a, 0.5, 90_000, None);
        assert_eq!(base, recipe_hash(&a, 0.5, 90_000, None), "recipe is not stable");
        assert_ne!(base, recipe_hash(&b, 0.5, 90_000, None), "chain change not seen");
        assert_ne!(base, recipe_hash(&a, 0.6, 90_000, None), "clip progress not seen");
        assert_ne!(base, recipe_hash(&a, 0.5, 91_000, None), "tick (noise seed) not seen");
        let cam = kt_fx::geom::Cam3d { cam: kt_model::CameraParams::default(), world_scale: 0.5 };
        assert_ne!(base, recipe_hash(&a, 0.5, 90_000, Some(cam)), "camera not seen");
        let mut cam2 = cam;
        cam2.world_scale = 0.25;
        assert_ne!(
            recipe_hash(&a, 0.5, 90_000, Some(cam)),
            recipe_hash(&a, 0.5, 90_000, Some(cam2)),
            "world scale not seen"
        );
        // an empty chain is also a recipe (a clean frame in the “template” cell)
        assert_ne!(base, recipe_hash(&[], 0.5, 90_000, None));
    }

    /// Rendering a request: the cell of the required size, the chain is VISIBLE, and the same recipe gives
    /// the same result (otherwise the strip would flicker).
    #[test]
    fn t546d_render_job_gives_a_cell_and_the_chain_is_visible() {
        let mut s = kt_fx::video::FxScratch::default();
        let plain = render_job(&job(0, 0), &mut s);
        assert_eq!(plain.size, [CELL_W as usize, CELL_H as usize]);
        let mut with_fx = job(1, 0);
        with_fx.chain = vec![FxInstance::Mirror { horizontal: true, vertical: false }];
        let mirrored = render_job(&with_fx, &mut s);
        assert_ne!(plain.as_raw(), mirrored.as_raw(), "the chain changed nothing");
        // a second run on the SAME pool is byte-for-byte (a dirty pool, t68)
        let again = render_job(&with_fx, &mut s);
        assert_eq!(mirrored.as_raw(), again.as_raw(), "a reused pool changed the bytes");
        // the alpha sample differs from the opaque one
        let mut alpha = job(2, 0);
        alpha.key.base = FxThumbBase::PatternAlpha;
        assert_ne!(plain.as_raw(), render_job(&alpha, &mut s).as_raw());
    }

    /// The service as a whole: a request reaches `get()`, and `invalidate_media` drops the cells
    /// of EXACTLY this media (a generator edit, item 412).
    #[test]
    fn t546e_service_delivers_and_invalidates_by_media() {
        let svc = FxThumbService::start();
        let k = key(77);
        svc.request(job(77, svc.generation()));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let img = loop {
            if let Some(img) = svc.get(&k) {
                break img;
            }
            assert!(std::time::Instant::now() < deadline, "the worker never delivered");
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(img.size, [CELL_W as usize, CELL_H as usize]);
        assert!(!svc.pending(), "queue is not empty after delivery");

        // cells on the frame of media No. 1 and on the sample
        let mine = FxThumbKey {
            base: FxThumbBase::Thumb(crate::thumbs::ThumbKey { media: MediaId(1), t: 0 }),
            recipe: 5,
        };
        let other = FxThumbKey {
            base: FxThumbBase::Thumb(crate::thumbs::ThumbKey { media: MediaId(2), t: 0 }),
            recipe: 5,
        };
        let px = Arc::new(egui::ColorImage::new([2, 2], vec![egui::Color32::BLACK; 4]));
        svc.shared.publish(mine, px.clone());
        svc.shared.publish(other, px.clone());
        svc.invalidate_media(MediaId(1));
        assert!(svc.get(&mine).is_none(), "retuned media kept its stale thumb");
        assert!(svc.get(&other).is_some(), "invalidation hit the wrong media");
        assert!(svc.get(&k).is_some(), "invalidation hit the built-in pattern");
    }

    /// The path the DRAWING takes: `PickerCtx::texture` must first enqueue the request and return `None`,
    /// and after the worker has worked, hand out the texture.
    /// Without this test “the cells are black” is visible only
    /// to a live scenario: the layout harness test finds the cell
    /// by its caption and knows nothing about the picture inside.
    #[test]
    fn t546g_picker_ctx_requests_then_gets_a_texture() {
        let svc = FxThumbService::start();
        let ctx = egui::Context::default();
        let mut pv = PickerCtx::new(&svc, FxThumbBase::Pattern, None, vec![], 0.5, 90_000, None);
        let fx = [FxInstance::blur(8)];
        assert!(pv.texture(&ctx, &fx).is_none(), "the texture was found before any render");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if pv.texture(&ctx, &fx).is_some() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the cell remained without a texture: the request did not reach the worker \
                 or the request key did not match the lookup key"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // a repeated call takes the READY texture rather than uploading it again
        let first = pv.texture(&ctx, &fx);
        let second = pv.texture(&ctx, &fx);
        assert_eq!(first, second, "the cell texture is recreated on every frame");
    }

    /// The misclick guard watches the DECODE ORDERS, not the render.
    #[test]
    fn t546f_decode_guard_waits_but_rendering_does_not() {
        use std::time::Duration;
        assert!(!decode_orders_allowed(Duration::from_millis(0)));
        assert!(!decode_orders_allowed(Duration::from_millis(799)));
        assert!(decode_orders_allowed(DECODE_GUARD));
        assert!(decode_orders_allowed(Duration::from_secs(3)));
    }
}
