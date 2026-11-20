//! FxThumbService: effect thumbnails for the picker (R.54, item 430).
//!
//! The shape is a SERVICE modelled on `ThumbService` (`bg.rs` calls it the reference for this shape):
//! its own thread, a condvar queue, LIFO + dedup, RAM-LRU,
//! cancellation by generation. The task frame (`BgJob`)
//! does not fit here: a task is finite and has progress,
//! while thumbnails are an endless queue of small requests,
//! living as long as the picker is open.
//!
//! **A cell of an event is the real frame** (chain-everywhere, the owner 2026-11-14: "НИГДЕ НЕ
//! ДОПУСТИМО чтобы эффекты накладывались мимо цепочки"): the project the player plays at the
//! cursor's moment, the candidate appended to the event's chain (or to a morpher's patch), composed
//! by `frame_plan` on the offline device — the event's placement, the slots in front, a stream
//! stage's history, the track's stages and a reel around it run as the preview will run them after
//! the click. A still source thumbnail with "chain + candidate" laid over it showed Datamosh's
//! candidates over the un-moshed frame (found-defects 460). For that the service opens its OWN
//! short-lived sources — invariant 5's offline path: not shared, no promised pace, finite and
//! cancellable by generation — and only after the misclick guard; until a cell's composite is
//! complete it is Loading (the owner's п.1: nothing partial is shown), never the sample in its place.
//!
//! What the service does NOT have and must not have:
//! * a decoder of the live path: the drawing only ENQUEUES requests (invariant 8); the decoding of a
//!   scene cell runs on the service's lane over its own sources;
//! * persistence: the cache is only RAM and only for the session.

use eframe::egui;
use kt_model::{FxInstance, MediaId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
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

/// What to compute the thumbnail on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FxThumbBase {
    /// The built-in sample (`kt_fx::video::test_pattern`): only where no chain exists to put the
    /// effect in (chain-everywhere) — never in place of an event's frame.
    Pattern,
    /// The same, with TRANSPARENT fields: `DropShadow` on an
    /// opaque frame is an identity by construction (item 428b).
    PatternAlpha,
    /// The real frame (chain-everywhere): `rev` names the project, `place` the event or morpher the
    /// candidate joins, `t` the moment.
    Scene { rev: u64, place: u64, t: i64 },
}

/// Where a cell's candidate goes in the project (chain-everywhere).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Place {
    /// The tail of the event's chain (where the picker's click puts it); its track is soloed, so a
    /// layer above does not hide every cell alike.
    Clip(kt_model::ClipId),
    /// The tail of the morpher's video patch; the whole frame, as the morpher acts on many layers.
    Morph(kt_model::MorphId),
}

impl Place {
    fn tag(self) -> u64 {
        match self {
            Place::Clip(id) => id.0 << 1,
            Place::Morph(id) => (id.0 << 1) | 1,
        }
    }
}

/// What a scene cell renders: the project the player plays, where the candidate goes, the moment,
/// and how the service opens the project's files.
#[derive(Clone)]
pub struct Scene {
    pub project: Arc<kt_model::Project>,
    /// Names `project` in the cell keys (the model's revision and the place being edited).
    pub rev: u64,
    pub place: Place,
    pub t: kt_time::Tick,
    pub base: Option<std::path::PathBuf>,
    pub index_db: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FxThumbKey {
    pub base: FxThumbBase,
    /// Recipe hash: chain + moment + camera (see `recipe_hash`).
    pub recipe: u64,
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
    // the manifest declares it (plugin-api-barriers): no kind is named here
    fx.iter().any(|f| matches!(f, FxInstance::Plugin(p) if p.manifest().ui.alpha_sample))
}

/// A thumbnail request: on the sample, or a scene (the real frame, opened on the lane).
pub struct FxThumbJob {
    pub key: FxThumbKey,
    /// The chain with curves ALREADY resolved (values at the moment of the cursor).
    pub chain: Vec<FxInstance>,
    pub u: f64,
    pub burst_index: i64,
    pub cam3d: Option<kt_fx::geom::Cam3d>,
    pub generation: u64,
    /// A scene cell (chain-everywhere): `chain` is then the candidate alone, appended at `place`.
    pub scene: Option<Scene>,
    /// A scene cell of a KIND picker (kind-picker-previews): the candidate is a kind put in place of
    /// the event's generator or the transition's kind, and `chain` is empty.
    pub swap: Option<Swap>,
}

/// A kind candidate (kind-picker-previews): what the click on a cell of the generator or the
/// transition kind picker puts in place. The cell is the frame after that click, as an effect cell is.
#[derive(Debug, Clone)]
pub enum Swap {
    /// The generator event `ClipId` with this kind at its defaults (what `switch_kind` gives).
    Gen(kt_model::ClipId, kt_model::GeneratorKind),
    /// The transition with this kind at its defaults, its curves gone (what the picker's click gives).
    Transition(kt_model::TransitionId, kt_model::TransitionKind),
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
    queue: Mutex<JobQueue>,
    ready: Mutex<HashMap<FxThumbKey, Arc<egui::ColorImage>>>,
    order: Mutex<VecDeque<FxThumbKey>>,
    /// The UI frame each ready cell was last shown in (row 114): the cap never evicts a cell the
    /// last two frames drew, or a gallery wider than the cap orders its cells round in a circle.
    shown: Mutex<HashMap<FxThumbKey, u64>>,
    generation: AtomicU64,
    /// Scene cells whose composite could not complete (a source that cannot give the frame): not
    /// asked again until the project or the place moves — a cell asks on every UI frame, and a retry
    /// each time would keep the lane busy on a pause. They stay Loading (found-defects 581).
    failed: Mutex<HashSet<FxThumbKey>>,
    /// The scene cells' sources, cache and memos, for one project at a time.
    scene: Mutex<Option<SceneCtx>>,
    /// The windows closed: the lane drops `scene` (its open files) at its next run — the UI never
    /// takes that lock, a render may hold it.
    scene_drop: std::sync::atomic::AtomicBool,
}

impl Shared {
    fn new() -> Arc<Shared> {
        Arc::new(Shared {
            queue: Mutex::new(JobQueue::default()),
            ready: Mutex::new(HashMap::new()),
            order: Mutex::new(VecDeque::new()),
            shown: Mutex::new(HashMap::new()),
            generation: AtomicU64::new(0),
            failed: Mutex::new(HashSet::new()),
            scene: Mutex::new(None),
            scene_drop: std::sync::atomic::AtomicBool::new(false),
        })
    }

    fn publish(&self, key: FxThumbKey, img: Arc<egui::ColorImage>) {
        let mut ready = self.ready.lock().unwrap();
        let mut order = self.order.lock().unwrap();
        if ready.insert(key, img).is_none() {
            order.push_back(key);
        }
        let mut shown = self.shown.lock().unwrap();
        crate::thumbs::evict_unshown(&mut order, MAX_READY_FX, &mut shown, |old| {
            ready.remove(old);
        });
    }

    fn get(&self, key: &FxThumbKey) -> Option<Arc<egui::ColorImage>> {
        let got = self.ready.lock().unwrap().get(key).cloned();
        if got.is_some() {
            self.shown.lock().unwrap().insert(*key, crate::thumbs::frame_now());
        }
        got
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
    /// The service's lane on the scheduler (one-scheduler): class `Visible`, so it pauses while
    /// playing or exporting by the scheduler's votes (it slept 25 ms between cells then); no thread
    /// of its own, no 1 s timer.
    lane: kt_time::sched::LaneRef,
}

impl FxThumbService {
    pub fn start() -> FxThumbService {
        let shared = Shared::new();
        let sh = shared.clone();
        let lane = kt_time::sched::Lane::service("fx-thumbs", kt_time::sched::Class::Visible, move |ctx| pump(&sh, ctx));
        FxThumbService { shared, tex: std::cell::RefCell::new(TexCache::default()), lane }
    }

    /// Enqueue a request (dedup). Drawing is never blocked.
    pub fn request(&self, job: FxThumbJob) {
        if self.shared.ready.lock().unwrap().contains_key(&job.key) {
            return;
        }
        let mut q = self.shared.queue.lock().unwrap();
        if q.push(job) {
            drop(q);
            self.lane.kick();
        }
    }

    pub fn get(&self, key: &FxThumbKey) -> Option<Arc<egui::ColorImage>> {
        self.shared.get(key)
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

    /// Whether there are unfinished requests. The redraw
    /// gate hangs on THIS, not on “the picker is open”:
    /// otherwise an open menu would hold 60 fps while idle (defect t38).
    pub fn pending(&self) -> bool {
        self.shared.queue.lock().unwrap().len() > 0
    }

    /// How many cells were asked at all: queued, ready or failed (the harness's view of a gallery).
    #[cfg(test)]
    pub fn asked(&self) -> usize {
        self.shared.queue.lock().unwrap().len() + self.shared.ready.lock().unwrap().len() + self.shared.failed.lock().unwrap().len()
    }

    /// Already queued? So that drawing does not assemble the
    /// request anew on every frame while the cell is being computed
    /// (dedup inside the queue saves from work, but not from assembling the job).
    pub fn queued(&self, key: &FxThumbKey) -> bool {
        self.shared.queue.lock().unwrap().seen_key(key)
    }

    /// A scene cell that could not complete: it is not asked again (and stays Loading).
    pub fn failed(&self, key: &FxThumbKey) -> bool {
        self.shared.failed.lock().unwrap().contains(key)
    }

    /// The windows that render scene cells closed: forget the cells that failed and close the
    /// sources (on the lane: the UI never waits for a render).
    pub fn forget_scene(&self) {
        self.shared.failed.lock().unwrap().clear();
        self.shared.scene_drop.store(true, Ordering::Relaxed);
        self.lane.kick();
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
    /// What to compute on (the real frame / the built-in sample).
    pub base: FxThumbBase,
    /// The CLIP chain with curves already resolved — what stands on the event now. The cell’s candidate
    /// is appended to the tail, so the picture shows the
    /// result of the click, not the effect “in a vacuum”.
    pub chain: Vec<FxInstance>,
    pub u: f64,
    pub burst_index: i64,
    pub cam3d: Option<kt_fx::geom::Cam3d>,
    pub generation: u64,
    /// The real frame the cells render (chain-everywhere); `None` — the sample (no chain exists).
    scene: Option<Scene>,
    /// The misclick guard has passed: a scene cell may open and decode (before it, it is Loading).
    decode_ok: bool,
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
        chain: Vec<FxInstance>,
        u: f64,
        burst_index: i64,
        cam3d: Option<kt_fx::geom::Cam3d>,
    ) -> PickerCtx<'a> {
        let generation = svc.generation();
        PickerCtx {
            svc,
            base,
            chain,
            u,
            burst_index,
            cam3d,
            generation,
            scene: None,
            decode_ok: false,
            full: Vec::new(),
        }
    }

    /// The cells of an event or a morpher: the real frame with the candidate at `scene.place`
    /// (chain-everywhere). `decode_ok` — the misclick guard has passed.
    pub fn for_scene(svc: &'a FxThumbService, scene: Scene, decode_ok: bool) -> PickerCtx<'a> {
        let t = scene.t.0;
        let mut pv = PickerCtx::new(svc, FxThumbBase::Pattern, Vec::new(), 0.0, t, None);
        pv.base = FxThumbBase::Scene { rev: scene.rev, place: scene.place.tag(), t };
        pv.scene = Some(scene);
        pv.decode_ok = decode_ok;
        pv
    }

    /// The cell texture for the chain “clip + candidate”, or `None` — then the request is queued and the cell
    /// is drawn as a placeholder until it is ready.
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        extra: &[FxInstance],
    ) -> Option<egui::TextureId> {
        if let Some(scene) = &self.scene {
            let key = FxThumbKey { base: self.base, recipe: recipe_hash(extra, 0.0, scene.t.0, None) };
            if let Some(id) = self.svc.texture_for(ctx, key) {
                return Some(id);
            }
            // before the guard nothing is opened: the cell is Loading, not the sample
            if self.decode_ok && !self.svc.queued(&key) && !self.svc.failed(&key) {
                self.svc.request(FxThumbJob {
                    key,
                    chain: extra.to_vec(),
                    u: 0.0,
                    burst_index: scene.t.0,
                    cam3d: None,
                    generation: self.generation,
                    scene: Some(scene.clone()),
                    swap: None,
                });
            }
            return None;
        }
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
                chain: self.full.clone(),
                u: self.u,
                burst_index: self.burst_index,
                cam3d: self.cam3d,
                generation: self.generation,
                scene: None,
                swap: None,
            });
        }
        None
    }
}

impl PickerCtx<'_> {
    /// The cell texture of a KIND candidate (kind-picker-previews): the scene with `swap` in place,
    /// or `None` — Loading (before the misclick guard nothing is asked; a context without a scene
    /// has no frame to show a kind on).
    pub fn swap_texture(&mut self, ctx: &egui::Context, swap: &Swap) -> Option<egui::TextureId> {
        let scene = self.scene.as_ref()?;
        let recipe = xxhash_rust::xxh3::xxh3_64(format!("{swap:?}|{}|{REF_W}x{REF_H}>{CELL_W}x{CELL_H}", scene.t.0).as_bytes());
        let key = FxThumbKey { base: self.base, recipe };
        if let Some(id) = self.svc.texture_for(ctx, key) {
            return Some(id);
        }
        if self.decode_ok && !self.svc.queued(&key) && !self.svc.failed(&key) {
            self.svc.request(FxThumbJob {
                key,
                chain: Vec::new(),
                u: 0.0,
                burst_index: scene.t.0,
                cam3d: None,
                generation: self.generation,
                scene: Some(scene.clone()),
                swap: Some(swap.clone()),
            });
        }
        None
    }
}

/// The width of a kind gallery's popup: the host window's body (as the effects gallery), never under
/// four cells a row. `host_w` is read before the popup opens: inside it the clip rect is the popup's.
pub fn gallery_width(ui: &mut egui::Ui, host_w: f32) {
    let margins = ui.spacing().menu_margin.sum().x + 4.0;
    let four = 4.0 * (crate::fx_registry::CELL_DRAW_W + ui.spacing().item_spacing.x);
    let w = (host_w - margins).max(four).floor();
    ui.set_min_width(w);
    ui.set_max_width(w);
}

/// One cell of a KIND picker (kind-picker-previews): the frame after the click (or Loading while it
/// is computed), the kind's name under it, ONE click interaction (t254). `swap` is `None` for a kind
/// with no picture (a sound generator): the cell is its name alone. `probe` publishes the cell's
/// rectangle for the live scenarios. A cell below the scroll fold is neither drawn, asked nor
/// published (rule 12: an invisible rectangle would be clicked past the popup).
pub fn kind_cell(
    ui: &mut egui::Ui,
    pv: Option<&mut PickerCtx<'_>>,
    swap: Option<&Swap>,
    name: &str,
    selected: bool,
    probe: &str,
) -> egui::Response {
    use crate::fx_registry::{CELL_DRAW_H, CELL_DRAW_W};
    let cap_h = ui.text_style_height(&egui::TextStyle::Small) + 2.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(CELL_DRAW_W, CELL_DRAW_H + cap_h), egui::Sense::click());
    // the name is the cell's in the accessibility tree even unseen (a popup's sizing pass draws nothing)
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), selected, name));
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    crate::widgets::field_probe::note(probe, rect);
    let img = egui::Rect::from_min_size(rect.min, egui::vec2(CELL_DRAW_W, CELL_DRAW_H));
    let p = ui.painter();
    p.rect_filled(img, 2.0, crate::theme::EXTREME);
    let small = egui::TextStyle::Small.resolve(ui.style());
    let weak = ui.visuals().weak_text_color();
    match (pv, swap) {
        (Some(pv), Some(swap)) => match pv.swap_texture(ui.ctx(), swap) {
            Some(tex) => {
                p.image(tex, img, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
            }
            None => {
                p.text(img.center(), egui::Align2::CENTER_CENTER, crate::i18n::tr(crate::i18n::k::perf::LOADING), small.clone(), weak);
            }
        },
        // a sound kind: no picture exists, the speaker says why the cell is empty
        (_, None) => {
            p.text(img.center(), egui::Align2::CENTER_CENTER, crate::icons::CAT_VOLUME, egui::FontId::proportional(20.0), weak);
        }
        (None, Some(_)) => {}
    }
    let stroke = if resp.hovered() || selected { crate::theme::ACCENT } else { ui.visuals().widgets.noninteractive.bg_stroke.color };
    p.rect_stroke(img, 2.0, egui::Stroke::new(if selected { 2.0 } else { 1.0 }, stroke), egui::StrokeKind::Inside);
    // the caption is truncated, never widening the cell (rule 7)
    let galley = ui.fonts_mut(|f| {
        let mut job = egui::text::LayoutJob::single_section(
            name.to_string(),
            egui::TextFormat { font_id: small, color: if selected { ui.visuals().strong_text_color() } else { weak }, ..Default::default() },
        );
        job.wrap = egui::text::TextWrapping { max_width: CELL_DRAW_W, max_rows: 1, overflow_character: Some('…'), ..Default::default() };
        f.layout_job(job)
    });
    let x = img.center().x - galley.size().x / 2.0;
    ui.painter().galley(egui::pos2(x, img.bottom() + 2.0), galley, egui::Color32::WHITE);
    resp
}

impl Drop for FxThumbService {
    fn drop(&mut self) {
        // a frame on the device when the process exits kills it: LLVM goes down under llvmpipe's
        // threads (gpu-render-hardening, D6, t1644): the lane closes and its running cell ends
        self.lane.close();
        let _join = kt_time::block::wait(kt_time::block::kind::JOIN);
        // bounded (close-while-busy): a heavy chain's cell can take seconds; at the process's exit
        // kt-gpu's exit hook and the exit's watcher bound what is left
        let since = std::time::Instant::now();
        let ended = self.lane.wait_idle(kt_time::shutdown::deadline());
        kt_time::shutdown::record("effect-cells", since, ended);
    }
}

/// Whether decode ORDERS (thumbnail/poster) are allowed — the misclick guard. A pure function:
/// the clock comes from outside, so the test does not sleep.
pub fn decode_orders_allowed(since_open: std::time::Duration) -> bool {
    since_open >= DECODE_GUARD
}

/// Rendering of one sample request. Taken out of the worker so that a test can call it directly —
/// without a thread and without waiting. With no video device the cell shows the sample.
pub(crate) fn render_job(job: &FxThumbJob) -> egui::ColorImage {
    kt_time::work::bump(kt_time::work::Work::FxThumb);
    let mut _span = kt_time::trace::span(kt_time::trace::Ev::Job);
    _span.aux(kt_time::trace::job::FX_THUMB);
    let base = kt_fx::video::test_pattern(REF_W, REF_H, job.key.base == FxThumbBase::PatternAlpha);
    // computation at the REFERENCE size, storage at the cell size (item 428)
    let mut rgba = vec![0u8; (REF_W * REF_H * 4) as usize];
    kt_fx::video::fit_into(&base, REF_W, REF_H, &mut rgba);
    let frame = kt_media::VideoFrame {
        pts: kt_time::Tick::ZERO,
        width: REF_W,
        height: REF_H,
        rgba: rgba.clone().into_boxed_slice(),
        has_alpha: base.has_alpha,
    };
    if let Some(engine) = kt_gpu::side_engine() {
        use kt_engine::VideoGpuExec as _;
        let hash = recipe_hash(&job.chain, job.u, job.burst_index, job.cam3d);
        let t = kt_time::Tick(job.burst_index);
        let plan = kt_engine::chain_plan(Arc::new(frame), &job.chain, hash, job.u, t, job.cam3d);
        if engine.execute_into(&plan, &mut rgba).is_err() {
            rgba.resize((REF_W * REF_H * 4) as usize, 0);
        }
    }
    let full = kt_fx::video::Frame { w: REF_W, h: REF_H, rgba, has_alpha: base.has_alpha };
    let mut cell = vec![0u8; (CELL_W as usize) * (CELL_H as usize) * 4];
    kt_fx::video::fit_into(&full, CELL_W, CELL_H, &mut cell);
    egui::ColorImage::from_rgba_unmultiplied([CELL_W as usize, CELL_H as usize], &cell)
}

/// The project with the candidate at `place`: appended to the event's chain (its track soloed when
/// the event is at the root), or to the morpher's video patch. `None` — the place is not in it.
pub(crate) fn with_candidate(
    project: &kt_model::Project,
    place: Place,
    extra: &[FxInstance],
    swap: Option<&Swap>,
) -> Option<kt_model::Project> {
    let mut p = project.clone();
    if let Some(swap) = swap {
        return with_swap(p, swap);
    }
    let append = |c: &mut kt_model::ClipEvent| {
        c.fx_bypass.resize(c.fx.len(), false);
        for f in extra {
            c.fx.push(f.clone());
            c.fx_bypass.push(false);
        }
    };
    match place {
        Place::Clip(id) => {
            if let Some(c) = p.clips.iter_mut().find(|c| c.id == id) {
                append(c);
                let lane = c.lane;
                solo_video(&mut p, lane);
            } else {
                // an event of a reel (an expanded band): the frame as it is, its chain extended
                append(p.sequences.iter_mut().flat_map(|s| s.clips.iter_mut()).find(|c| c.id == id)?);
            }
        }
        Place::Morph(id) => {
            let m = p
                .morphers
                .iter_mut()
                .chain(p.sequences.iter_mut().flat_map(|s| s.morphers.iter_mut()))
                .find(|m| m.id == id)?;
            m.patch.video_bypass.resize(m.patch.video_fx.len(), false);
            for f in extra {
                m.patch.video_fx.push(f.clone());
                m.patch.video_bypass.push(false);
            }
        }
    }
    Some(p)
}

/// Solo the video track `lane` (a layer above would hide every cell alike, as for an effect cell).
fn solo_video(p: &mut kt_model::Project, lane: u32) {
    use kt_model::LaneKind;
    if let Some(i) = kt_model::track_index(&p.tracks, LaneKind::Video, lane) {
        for (j, t) in p.tracks.iter_mut().enumerate() {
            if t.kind == LaneKind::Video {
                t.solo = j == i;
            }
        }
    }
}

/// The project with a kind candidate in place (kind-picker-previews): the generator event's kind at
/// its defaults with no curves (`fit_gen_to_media` after a kind change), or the transition's kind at
/// its defaults with no curves (the picker's click). At the root the track is soloed; an event or a
/// transition of a reel (an expanded band) is changed where it lies. `None` — it is not in the project.
fn with_swap(mut p: kt_model::Project, swap: &Swap) -> Option<kt_model::Project> {
    match swap {
        Swap::Gen(id, kind) => {
            let set = |c: &mut kt_model::ClipEvent| {
                c.r#gen = Some(kind.clone());
                c.gen_auto.clear();
            };
            if let Some(c) = p.clips.iter_mut().find(|c| c.id == *id) {
                set(c);
                let lane = c.lane;
                solo_video(&mut p, lane);
            } else {
                set(p.sequences.iter_mut().flat_map(|s| s.clips.iter_mut()).find(|c| c.id == *id)?);
            }
        }
        Swap::Transition(id, kind) => {
            let set = |t: &mut kt_model::TransitionEvent| {
                t.kind = kind.clone();
                t.auto.clear();
            };
            if let Some(t) = p.transitions.iter_mut().find(|t| t.id == *id) {
                set(t);
                let side = t.a.or(t.b);
                if let Some(lane) = side.and_then(|c| p.clips.iter().find(|e| e.id == c)).map(|c| c.lane) {
                    solo_video(&mut p, lane);
                }
            } else {
                set(p.sequences.iter_mut().flat_map(|s| s.transitions.iter_mut()).find(|t| t.id == *id)?);
            }
        }
    }
    Some(p)
}

/// The scene cells' state for one project: the sources the composites asked for (opened once, on
/// the lane — invariant 5's offline path), their facts, a cache, the stream history's memo (an
/// offline frame job's own, never the preview's) and the build memo (each candidate's renderer
/// re-derives only the event it changed).
pub(crate) struct SceneCtx {
    project: Arc<kt_model::Project>,
    gens: Vec<(MediaId, kt_model::GenState)>,
    src: HashMap<MediaId, Box<dyn kt_media::MediaSource>>,
    tried: HashSet<MediaId>,
    facts: kt_engine::SourceFacts,
    cache: kt_cache::BurstCache,
    memo: Arc<kt_plugin_api::stream::StreamMemo>,
    builds: kt_engine::incremental::BuildMemo,
}

impl SceneCtx {
    fn new(scene: &Scene) -> SceneCtx {
        SceneCtx {
            project: scene.project.clone(),
            gens: kt_engine::gen_sources(&scene.project),
            src: HashMap::new(),
            tried: HashSet::new(),
            facts: kt_engine::SourceFacts::default(),
            cache: kt_cache::BurstCache::new(kt_cache::CacheConfig { max_audio_bytes: 1, max_video_bytes: 64 << 20 }),
            memo: Arc::new(kt_plugin_api::stream::StreamMemo::new(kt_engine::export::EXPORT_STREAM_MEMO_BYTES)),
            builds: Default::default(),
        }
    }

    fn build(&mut self, p: &kt_model::Project) -> kt_engine::VideoRenderer {
        let facts = &self.facts;
        let mut video = kt_engine::incremental::with_memo(&mut self.builds, || kt_engine::VideoRenderer::new_with_facts(p, facts));
        video.set_stream_memo(self.memo.clone());
        video
    }

    /// Open the sources `video` asks at `t` that were never tried; `true` — the facts moved (the
    /// renderer must be built again).
    fn open_wanted(&mut self, scene: &Scene, video: &kt_engine::VideoRenderer, t: kt_time::Tick, q: kt_media::Quality) -> bool {
        let mut opened = false;
        for (m, _, _) in video.layers_at(t, q) {
            if !self.tried.insert(m) {
                continue;
            }
            let s = if let Some(r) = self.project.media.iter().find(|r| r.id == m) {
                let path = crate::util::resolve_media_path(scene.base.as_deref(), &r.path);
                // a missing file is its placeholder, as in the editor (catch-up-shows-loading D6)
                crate::util::open_source_or_placeholder(&path, scene.index_db.as_deref(), &crate::util::media_label(&path)).0
            } else if let Some((_, st)) = self.gens.iter().find(|(a, _)| *a == m) {
                kt_media::factory::open_gen(st)
            } else {
                continue;
            };
            self.facts.observe(m, s.info());
            self.src.insert(m, s);
            opened = true;
        }
        opened
    }
}

/// What a scene cell's render came to.
pub(crate) enum SceneOut {
    Done(egui::ColorImage),
    /// The composite cannot complete (a source that cannot give the frame): the cell stays Loading.
    Incomplete,
    /// The job was stopped (the picker moved on, the lane paused or closed).
    Stopped,
}

/// Render a scene cell: the project with the candidate, composed at the reference size through
/// `frame_plan` on the offline device, a stream stage's history pre-rolled first. `stop` is asked
/// between the history's positions and batches.
pub(crate) fn render_scene(
    slot: &mut Option<SceneCtx>,
    scene: &Scene,
    extra: &[FxInstance],
    swap: Option<&Swap>,
    stop: impl Fn() -> bool + Clone + 'static,
) -> SceneOut {
    use kt_time::trace::stop::JOB;
    kt_time::work::bump(kt_time::work::Work::FxThumb);
    let mut _span = kt_time::trace::span(kt_time::trace::Ev::Job);
    _span.aux(kt_time::trace::job::FX_THUMB);
    // a job whose picker moved on (or whose lane paused) starts nothing
    if stop() {
        return SceneOut::Stopped;
    }
    // a generator kind is a SOURCE's state: the scene's sources and frame cache are keyed by the
    // event's address and hold its present kind, so the candidate gets a context of its own project
    // (its generator opened with the candidate's values; the track is soloed, so little else opens)
    if let Some(Swap::Gen(..)) = swap {
        let Some(p) = with_candidate(&scene.project, scene.place, extra, swap) else { return SceneOut::Incomplete };
        let own = Scene { project: Arc::new(p), ..scene.clone() };
        return render_scene(&mut None, &own, &[], None, stop);
    }
    if !matches!(slot, Some(sc) if Arc::ptr_eq(&sc.project, &scene.project)) {
        *slot = Some(SceneCtx::new(scene));
    }
    let Some(sc) = slot.as_mut() else { return SceneOut::Incomplete };
    let Some(p) = with_candidate(&scene.project, scene.place, extra, swap) else { return SceneOut::Incomplete };
    let Some(engine) = kt_gpu::side_engine() else { return SceneOut::Incomplete };
    let q = kt_media::Quality { width: REF_W, height: REF_H };
    let mut video = sc.build(&p);
    let t = video.grid.align_floor(scene.t);
    // what the composite reads: opened as it is named, the renderer built again with their facts
    for _ in 0..4 {
        if !sc.open_wanted(scene, &video, t, q) {
            break;
        }
        video = sc.build(&p);
    }
    if video.has_stream_stages() {
        // the history the stream stage reads (a history in the event before may name more files)
        for _ in 0..4 {
            let held = stop.clone();
            kt_engine::stop_when(JOB, held, || kt_engine::stream_preroll(&video, &sc.cache, &mut sc.src, t, q));
            if stop() {
                return SceneOut::Stopped;
            }
            if !sc.open_wanted(scene, &video, t, q) {
                break;
            }
            video = sc.build(&p);
        }
    }
    let mut per_src: HashMap<(MediaId, kt_media::Quality), Vec<kt_time::Tick>> = HashMap::new();
    for (media, pts, dq) in video.wanted_frames(kt_time::TickRange::new(t, kt_time::Tick(t.0 + video.grid.frame_step())), q) {
        per_src.entry((media, dq)).or_default().push(pts);
    }
    for ((media, dq), keys) in per_src {
        let Some(s) = sc.src.get_mut(&media) else { continue };
        let sid = kt_engine::source_id(media);
        let missing: Vec<kt_time::Tick> =
            keys.into_iter().filter(|k| sc.cache.get_video(sid, Some(dq), kt_engine::video_cache_idx(*k)).is_none()).collect();
        if missing.is_empty() {
            continue;
        }
        let cache = &sc.cache;
        let _ = s.read_video_span(&missing, Some(dq), &mut |k, frame| {
            cache.insert_video(sid, Some(dq), kt_engine::video_cache_idx(k), frame.into());
        });
    }
    let mut rgba = Vec::new();
    let held = stop.clone();
    let composed = kt_engine::stop_when(JOB, held, || video.compose_on(&*engine, &sc.cache, t, q, &mut rgba));
    // either the frame is there or not (the owner's п.1): an incomplete composite is never shown
    if !matches!(composed, Ok(true)) || rgba.len() != (REF_W * REF_H * 4) as usize {
        return if stop() { SceneOut::Stopped } else { SceneOut::Incomplete };
    }
    let full = kt_fx::video::Frame { w: REF_W, h: REF_H, rgba, has_alpha: false };
    let mut cell = vec![0u8; (CELL_W as usize) * (CELL_H as usize) * 4];
    kt_fx::video::fit_into(&full, CELL_W, CELL_H, &mut cell);
    SceneOut::Done(egui::ColorImage::from_rgba_unmultiplied([CELL_W as usize, CELL_H as usize], &cell))
}

/// One run of the service: render cells newest first until the queue is empty (`Idle`) or the
/// class pauses (`More`: run again on resume).
fn pump(shared: &Arc<Shared>, ctx: &kt_time::sched::Ctx) -> kt_time::sched::Pump {
    use kt_time::sched::Pump;
    loop {
        if shared.scene_drop.swap(false, Ordering::Relaxed) {
            *shared.scene.lock().unwrap() = None;
        }
        if ctx.cancelled() {
            return Pump::Idle;
        }
        if ctx.paused() {
            return Pump::More;
        }
        let cur = shared.generation.load(Ordering::Relaxed);
        let Some(job) = shared.queue.lock().unwrap().pop(cur) else { return Pump::Idle };
        if shared.ready.lock().unwrap().contains_key(&job.key) {
            continue;
        }
        let img = match &job.scene {
            None => render_job(&job),
            Some(scene) => {
                let (lane, own) = ctx.tokens();
                let class = ctx.class();
                let sh = shared.clone();
                let generation = job.generation;
                let stop = move || {
                    lane.is_cancelled()
                        || own.is_cancelled()
                        || sh.generation.load(Ordering::Relaxed) > generation
                        || (class.pausable() && kt_time::sched::paused() != 0)
                };
                let out = render_scene(&mut shared.scene.lock().unwrap(), scene, &job.chain, job.swap.as_ref(), stop);
                match out {
                    SceneOut::Done(img) => img,
                    SceneOut::Incomplete => {
                        shared.failed.lock().unwrap().insert(job.key);
                        crate::wake::soon();
                        continue;
                    }
                    // a paused lane asks again on resume; a job of a past generation is dropped
                    SceneOut::Stopped => {
                        if job.generation >= shared.generation.load(Ordering::Relaxed) && !ctx.cancelled() {
                            shared.queue.lock().unwrap().push(job);
                            return Pump::More;
                        }
                        continue;
                    }
                }
            }
        };
        // the generation could change WHILE we were computing: publishing is still possible
        // (the key addresses exactly this picture), but computing
        // the next stale requests is not — pop() will drop them
        shared.publish(job.key, Arc::new(img));
        crate::wake::soon();
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
            chain: vec![],
            u: 0.5,
            burst_index: 90_000,
            cam3d: None,
            generation,
            scene: None,
            swap: None,
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
        crate::plugins::install();
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
        crate::plugins::install();
        if kt_gpu::offline_engine().is_none() {
            return;
        }
        let plain = render_job(&job(0, 0));
        assert_eq!(plain.size, [CELL_W as usize, CELL_H as usize]);
        let mut with_fx = job(1, 0);
        with_fx.chain = vec![kt_model::FxInstance::plugin_with("mirror", &[1.0, 0.0]).unwrap()];
        let mirrored = render_job(&with_fx);
        assert_ne!(plain.as_raw(), mirrored.as_raw(), "the chain changed nothing");
        // a second run is byte-for-byte
        let again = render_job(&with_fx);
        assert_eq!(mirrored.as_raw(), again.as_raw(), "a second run changed the bytes");
        // the alpha sample differs from the opaque one
        let mut alpha = job(2, 0);
        alpha.key.base = FxThumbBase::PatternAlpha;
        assert_ne!(plain.as_raw(), render_job(&alpha).as_raw());
    }

    /// t544e: a thumbnail is the fit to the reference size plus the chain, then the fit to the cell,
    /// and nothing else; a second run gives the same bytes.
    #[test]
    fn t544e_thumb_is_fit_plus_chain_and_nothing_else() {
        if kt_gpu::testkit::no_gpu(crate::plugins::install) {
            return;
        }
        let mut j = job(1, 0);
        j.chain = vec![FxInstance::blur(6), FxInstance::vhs(4, 0.4)];
        let got = render_job(&j);
        let base = kt_fx::video::test_pattern(REF_W, REF_H, false);
        let ctx = kt_fx::video::VideoFxCtx {
            out_w: REF_W,
            out_h: REF_H,
            u: j.u,
            burst_index: j.burst_index,
            cam3d: None,
            in_scale: 1,
            lead: None,
        };
        let full = kt_gpu::testkit::chain(&j.chain, base, &ctx);
        let mut cell = vec![0u8; (CELL_W * CELL_H * 4) as usize];
        kt_fx::video::fit_into(&full, CELL_W, CELL_H, &mut cell);
        let want = egui::ColorImage::from_rgba_unmultiplied([CELL_W as usize, CELL_H as usize], &cell);
        assert_eq!(got.as_raw(), want.as_raw(), "render_job is not fit + chain + fit");
        assert_eq!(render_job(&j).as_raw(), want.as_raw(), "a second run changed the bytes");
    }

    /// t544f: a thumbnail's grain is pinned to the tick: the same tick gives the same cell, another
    /// tick another one.
    #[test]
    fn t544f_noise_is_pinned_to_the_tick() {
        if kt_gpu::testkit::no_gpu(crate::plugins::install) {
            return;
        }
        let at = |t: i64| {
            let mut j = job(1, 0);
            j.chain = vec![FxInstance::noise(0.25)];
            j.burst_index = t;
            render_job(&j)
        };
        assert_eq!(at(90_000).as_raw(), at(90_000).as_raw(), "one tick gave two different thumbs");
        assert_ne!(at(90_000).as_raw(), at(270_000).as_raw(), "noise ignores the tick");
    }

    /// t1644 (gpu-render-hardening, D6): dropping the service waits for its worker. A frame left on
    /// the device at exit killed the process: t547 ended in SIGSEGV one run in three. Since
    /// close-while-busy the wait is bounded (`kt_time::shutdown::WAIT`) and a cell still rendering past
    /// it is left behind BY NAME and ends by itself (found-defects 760: a full parallel run at load ~57
    /// rendered the one cell past two seconds, and the drop's bound was read as "the worker outlived").
    #[test]
    fn t1644_dropping_the_service_waits_for_its_worker() {
        crate::plugins::install();
        let svc = FxThumbService::start();
        let shared = svc.shared.clone();
        svc.request(job(5, svc.generation()));
        let since = std::time::Instant::now();
        drop(svc);
        let waited = since.elapsed();
        // which branch the DROP took is its own record, not the count of `shared`: the lane's
        // thread lets its copy go after it says idle, so a drop that waited the cell out still saw
        // the count above one for a moment under load (found-defects 781)
        if kt_time::shutdown::left_behind().contains(&"effect-cells") {
            eprintln!("t1644: the cell outlived the drop's bound ({waited:?}), left behind by name");
            assert!(waited >= kt_time::shutdown::WAIT, "the drop returned before its bound ({waited:?}) with the worker running");
        }
        let deadline = kt_time::test_deadline();
        while Arc::strong_count(&shared) > 1 {
            assert!(std::time::Instant::now() < deadline, "the worker never ended");
            std::thread::yield_now();
        }
    }

    /// The service as a whole: a request reaches `get()`. A generator edit no longer needs a
    /// media invalidation (item 412): a cell of an event is keyed by the project's revision, which
    /// the edit moves (chain-everywhere).
    #[test]
    fn t546e_service_delivers() {
        crate::plugins::install();
        let svc = FxThumbService::start();
        let k = key(77);
        svc.request(job(77, svc.generation()));
        // the first request opens the offline device
        let deadline = kt_time::test_deadline();
        let img = loop {
            if let Some(img) = svc.get(&k) {
                break img;
            }
            assert!(std::time::Instant::now() < deadline, "the worker never delivered");
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(img.size, [CELL_W as usize, CELL_H as usize]);
        assert!(!svc.pending(), "queue is not empty after delivery");
    }

    /// The path the DRAWING takes: `PickerCtx::texture` must first enqueue the request and return `None`,
    /// and after the worker has worked, hand out the texture.
    /// Without this test “the cells are black” is visible only
    /// to a live scenario: the layout harness test finds the cell
    /// by its caption and knows nothing about the picture inside.
    #[test]
    fn t546g_picker_ctx_requests_then_gets_a_texture() {
        crate::plugins::install();
        let svc = FxThumbService::start();
        let ctx = egui::Context::default();
        let mut pv = PickerCtx::new(&svc, FxThumbBase::Pattern, vec![], 0.5, 90_000, None);
        let fx = [FxInstance::blur(8)];
        assert!(pv.texture(&ctx, &fx).is_none(), "the texture was found before any render");
        let deadline = kt_time::test_deadline();
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

    // ---- scene cells (chain-everywhere) ------------------------------------------------------

    fn clip_at(id: u64, media: u64, lane: u32, start: i64, dur: i64, fx: Vec<FxInstance>) -> kt_model::ClipEvent {
        kt_model::ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            masks: Default::default(),
            id: kt_model::ClipId(id),
            link_group: None,
            kind: kt_model::ClipKind::Video,
            lane,
            start: kt_time::Tick(start),
            duration: kt_time::Tick(dur),
            source_in: kt_time::Tick(0),
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

    fn scene_project() -> kt_model::Project {
        let mut p = kt_model::Project::new(kt_model::ProjectSettings {
            fps: 30,
            width: 320,
            height: 180,
            sample_rate: 48000,
            pitch_engine: Default::default(),
            default_shape: Default::default(),
            conform: Default::default(),
            settings_explicit: true,
        });
        p.media.push(kt_model::MediaRef::new(MediaId(1), "synth:ramp"));
        p.media.push(kt_model::MediaRef::new(MediaId(2), "synth:impulse"));
        p
    }

    fn scene(p: kt_model::Project, place: Place, t: i64) -> Scene {
        Scene { project: Arc::new(p), rev: 1, place, t: kt_time::Tick(t), base: None, index_db: None }
    }

    fn mirror() -> FxInstance {
        FxInstance::plugin_with("mirror", &[1.0, 0.0]).expect("mirror is built in")
    }

    fn poster(levels: f64) -> FxInstance {
        let mut f = FxInstance::plugin("posterize").expect("posterize is built in");
        assert!(f.set_param("levels", levels));
        f
    }

    /// The frame the preview composes after the click, by the offline path the strip and the
    /// poster take (`reel_preview::compose_offline_frame`, its own sources), fitted into a cell.
    fn after_click(p: &kt_model::Project, t: i64) -> Vec<u8> {
        let ctl = crate::bg::BgCtl::from_flags(kt_time::sched::Token::new(), Default::default());
        let mut src = HashMap::new();
        let mut facts = kt_engine::SourceFacts::default();
        for r in &p.media {
            let s = kt_media::factory::open_pseudo(&r.path).expect("pseudo").expect("opens");
            facts.observe(r.id, s.info());
            src.insert(r.id, s);
        }
        let mut video = crate::reel_preview::offline_renderer(p, &facts);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig { max_audio_bytes: 1, max_video_bytes: 64 << 20 });
        let q = kt_media::Quality { width: REF_W, height: REF_H };
        let t = video.grid.align_floor(kt_time::Tick(t));
        let mut buf = Vec::new();
        crate::reel_preview::compose_offline_frame(&ctl, &mut video, &mut src, &cache, t, q, &mut buf);
        assert_eq!(buf.len(), (REF_W * REF_H * 4) as usize, "the reference frame is incomplete");
        let full = kt_fx::video::Frame { w: REF_W, h: REF_H, rgba: buf, has_alpha: false };
        let mut cell = vec![0u8; (CELL_W * CELL_H * 4) as usize];
        kt_fx::video::fit_into(&full, CELL_W, CELL_H, &mut cell);
        egui::ColorImage::from_rgba_unmultiplied([CELL_W as usize, CELL_H as usize], &cell).as_raw().to_vec()
    }

    fn cell(sc: &Scene, extra: &[FxInstance]) -> Vec<u8> {
        let mut slot = None;
        match render_scene(&mut slot, sc, extra, None, || false) {
            SceneOut::Done(img) => img.as_raw().to_vec(),
            SceneOut::Incomplete => panic!("the cell is incomplete"),
            SceneOut::Stopped => panic!("the cell stopped"),
        }
    }

    fn cut_project(kind: kt_model::TransitionKind) -> kt_model::Project {
        let mut p = scene_project();
        p.ensure_tracks(kt_model::LaneKind::Video, 2);
        p.clips.push(clip_at(1, 1, 0, 0, 2 * 90_000, vec![]));
        let mut b = clip_at(2, 2, 0, 2 * 90_000, 2 * 90_000, vec![]);
        b.source_in = kt_time::Tick(90_000);
        p.clips.push(b);
        // a layer above the cut: the cell solos the cut's track, as an effect cell does
        p.clips.push(clip_at(3, 1, 1, 0, 4 * 90_000, vec![]));
        p.transitions.push(kt_model::TransitionEvent {
            id: kt_model::TransitionId(1),
            a: Some(kt_model::ClipId(1)),
            b: Some(kt_model::ClipId(2)),
            in_offset: kt_time::Tick(30_000),
            out_offset: kt_time::Tick(30_000),
            kind,
            audio: Default::default(),
            auto: Vec::new(),
            invert: false,
        });
        p
    }

    /// t3108 (kind-picker-previews, `v26.10` G6): a kind candidate goes where the picker's click puts
    /// it — the generator event's kind at its defaults with no curves, the transition's kind with no
    /// curves — on the track soloed; a place the project does not hold gives no scene.
    #[test]
    fn t3108_a_kind_candidate_is_what_the_click_puts() {
        crate::plugins::install();
        let mut p = cut_project(kt_model::TransitionKind::Crossfade);
        p.transitions[0].auto.push(kt_model::ParamCurve::constant("x", kt_time::Tick(0), kt_time::Tick(90_000), 0.5, Default::default()));
        let iris = kt_model::TransitionKind::builtin("iris");
        let got = with_candidate(&p, Place::Clip(kt_model::ClipId(1)), &[], Some(&Swap::Transition(kt_model::TransitionId(1), iris.clone())))
            .expect("a scene");
        assert_eq!(got.transitions[0].kind, iris, "the transition's kind is not the candidate");
        assert!(got.transitions[0].auto.is_empty(), "the former kind's curves stayed");
        let solo: Vec<bool> = got.tracks.iter().filter(|t| t.kind == kt_model::LaneKind::Video).map(|t| t.solo).collect();
        assert_eq!(solo, vec![true, false], "the cut's track alone is soloed");
        assert_eq!(got.clips, p.clips, "a kind candidate touched the events");

        let mut g = scene_project();
        let mut c = clip_at(1, 1, 0, 0, 90_000, vec![]);
        c.r#gen = Some(kt_model::GeneratorKind::solid(10, 20, 30));
        c.gen_auto.push(kt_model::ParamCurve::constant("x", kt_time::Tick(0), kt_time::Tick(90_000), 0.5, Default::default()));
        g.clips.push(c);
        let checker = kt_model::GeneratorKind::default_for_tag("checker").expect("checker is built in");
        let got = with_candidate(&g, Place::Clip(kt_model::ClipId(1)), &[], Some(&Swap::Gen(kt_model::ClipId(1), checker.clone())))
            .expect("a scene");
        assert_eq!(got.clips[0].r#gen, Some(checker), "the generator's kind is not the candidate at its defaults");
        assert!(got.clips[0].gen_auto.is_empty(), "the former kind's curves stayed");
        assert!(with_candidate(&g, Place::Clip(kt_model::ClipId(1)), &[], Some(&Swap::Gen(kt_model::ClipId(9), kt_model::GeneratorKind::solid(0, 0, 0)))).is_none());
        assert!(with_candidate(&p, Place::Clip(kt_model::ClipId(1)), &[], Some(&Swap::Transition(kt_model::TransitionId(9), iris))).is_none());
    }

    /// t3109 (kind-picker-previews): a transition kind's cell is the frame after the click — the cut
    /// composed with that kind at the middle of its window — byte for byte, and two kinds differ there.
    #[test]
    fn t3109_a_transition_cell_is_the_cut_after_the_click() {
        crate::plugins::install();
        if kt_gpu::offline_engine().is_none() {
            eprintln!("skipped: no offline device");
            return;
        }
        let p = cut_project(kt_model::TransitionKind::Crossfade);
        p.validate().unwrap();
        let mid = 2 * 90_000;
        let sc = scene(p.clone(), Place::Clip(kt_model::ClipId(1)), mid);
        let shot = |kind: &kt_model::TransitionKind| {
            let swap = Swap::Transition(kt_model::TransitionId(1), kind.clone());
            let got = match render_scene(&mut None, &sc, &[], Some(&swap), || false) {
                SceneOut::Done(img) => img.as_raw().to_vec(),
                _ => panic!("the kind cell is incomplete"),
            };
            let want = after_click(&with_candidate(&p, sc.place, &[], Some(&swap)).unwrap(), mid);
            assert_eq!(got, want, "the {} cell is not the cut after the click", kind.tag());
            got
        };
        let iris = shot(&kt_model::TransitionKind::builtin("iris"));
        let fade = shot(&kt_model::TransitionKind::Crossfade);
        assert_ne!(iris, fade, "two kinds give one cell: the candidate did not reach the frame");

        // a generator kind: the event's SOURCE takes the candidate's values (live kind-picker-previews
        // found every cell the present kind's frame — the scene's source opened by the present state)
        let mut g = scene_project();
        let black = kt_model::GeneratorKind::solid(0, 0, 0);
        g.media.push(kt_model::MediaRef::new(MediaId(3), kt_model::format_gen_path(&black)));
        let mut c = clip_at(1, 3, 0, 0, 90_000, vec![]);
        c.r#gen = Some(black);
        g.clips.push(c);
        g.validate().unwrap();
        let sc = scene(g, Place::Clip(kt_model::ClipId(1)), 45_000);
        let gen_cell = |kind: kt_model::GeneratorKind| match render_scene(&mut None, &sc, &[], Some(&Swap::Gen(kt_model::ClipId(1), kind)), || false) {
            SceneOut::Done(img) => img.as_raw().to_vec(),
            _ => panic!("the generator kind cell is incomplete"),
        };
        let white = gen_cell(kt_model::GeneratorKind::solid(255, 255, 255));
        let checker = gen_cell(kt_model::GeneratorKind::default_for_tag("checker").unwrap());
        assert!(white.chunks(4).all(|px| px[0] > 200), "the white candidate's cell is not white: the present kind's source drew it");
        assert_ne!(white, checker, "two generator kinds give one cell");
    }

    /// t2554 (chain-everywhere): the candidate goes where the click puts it — the tail of the event's
    /// chain (its track soloed), or the tail of the morpher's video patch; a place the project does
    /// not hold gives no scene.
    #[test]
    fn t2554_the_candidate_goes_where_the_click_puts_it() {
        crate::plugins::install();
        let mut p = scene_project();
        p.ensure_tracks(kt_model::LaneKind::Video, 2);
        p.clips.push(clip_at(1, 1, 0, 0, 90_000, vec![]));
        p.clips.push(clip_at(2, 2, 1, 0, 90_000, vec![poster(4.0)]));
        p.clips[1].fx_bypass = vec![true];
        let got = with_candidate(&p, Place::Clip(kt_model::ClipId(2)), &[mirror()], None).expect("a scene");
        let c = got.clips.iter().find(|c| c.id == kt_model::ClipId(2)).unwrap();
        assert_eq!(c.fx, vec![poster(4.0), mirror()], "the candidate is not at the tail");
        assert_eq!(c.fx_bypass, vec![true, false], "the candidate is bypassed or the bypass moved");
        let videos: Vec<bool> = got.tracks.iter().filter(|t| t.kind == kt_model::LaneKind::Video).map(|t| t.solo).collect();
        assert_eq!(videos, vec![false, true], "the event's track alone is soloed");
        assert!(with_candidate(&p, Place::Clip(kt_model::ClipId(9)), &[mirror()], None).is_none());
        assert!(with_candidate(&p, Place::Morph(kt_model::MorphId(9)), &[mirror()], None).is_none());
    }

    /// t2555 (chain-everywhere, found-defects 460): an effect cell is the frame the preview shows after
    /// the click — the event's placement, its chain with the candidate at the tail and the track's
    /// stage after it, composed by `frame_plan` — byte for byte, and not the candidate over a still
    /// sample.
    #[test]
    fn t2555_a_cell_is_the_frame_after_the_click() {
        crate::plugins::install();
        if kt_gpu::offline_engine().is_none() {
            eprintln!("skipped: no offline device");
            return;
        }
        let mut p = scene_project();
        p.clips.push(clip_at(1, 1, 0, 0, 2 * 90_000, vec![poster(5.0)]));
        let vt = p.tracks.iter().position(|t| t.kind == kt_model::LaneKind::Video).unwrap();
        p.tracks[vt].fx = vec![poster(3.0)];
        p.validate().unwrap();
        let t = 45_000;
        let sc = scene(p.clone(), Place::Clip(kt_model::ClipId(1)), t);
        let got = cell(&sc, &[mirror()]);
        let want = after_click(&with_candidate(&p, Place::Clip(kt_model::ClipId(1)), &[mirror()], None).unwrap(), t);
        assert_eq!(got, want, "the cell is not the frame after the click");
        let mut on_sample = job(1, 0);
        on_sample.chain = vec![poster(5.0), mirror()];
        assert_ne!(got, render_job(&on_sample).as_raw().to_vec(), "the cell is the candidate over the sample");
    }

    /// t2556 (chain-everywhere, found-defects 460 — the owner's example): a cell of an event whose
    /// chain holds Datamosh shows the candidate over the MOSH — the stream's history pre-rolled on the
    /// service's lane — not over the un-moshed frame.
    #[test]
    fn t2556_a_cell_after_datamosh_shows_the_candidate_over_the_mosh() {
        crate::plugins::install();
        if kt_gpu::offline_engine().is_none() {
            eprintln!("skipped: no offline device");
            return;
        }
        kt_plugin_api::compute::install(kt_gpu::compute::open);
        if kt_plugin_api::compute::host().is_none() {
            eprintln!("skipped: no compute device");
            return;
        }
        let mosh = || {
            let mut f = FxInstance::plugin("datamosh").expect("datamosh is built in");
            for (name, v) in [("lines", 0.0), ("before", 12.0), ("mosh", 1.0), ("gop", 12.0), ("qp", 14.0)] {
                assert!(f.set_param(name, v), "{name}");
            }
            f
        };
        let shots = |b: Vec<FxInstance>| {
            let mut p = scene_project();
            p.clips.push(clip_at(1, 1, 0, 0, 30 * 3000, vec![]));
            p.clips.push(clip_at(2, 2, 0, 30 * 3000, 30 * 3000, b));
            p.validate().unwrap();
            p
        };
        let t = 34 * 3000;
        let moshed = shots(vec![mosh()]);
        let got = cell(&scene(moshed.clone(), Place::Clip(kt_model::ClipId(2)), t), &[mirror()]);
        let want = after_click(&with_candidate(&moshed, Place::Clip(kt_model::ClipId(2)), &[mirror()], None).unwrap(), t);
        assert_eq!(got, want, "the cell is not the frame after the click");
        let plain = shots(vec![]);
        let unmoshed = after_click(&with_candidate(&plain, Place::Clip(kt_model::ClipId(2)), &[mirror()], None).unwrap(), t);
        assert_ne!(got, unmoshed, "the candidate was laid over the un-moshed frame");
    }

    /// t2557 (chain-everywhere, item 430 kept): before the misclick guard a scene cell is Loading and
    /// asks nothing; after it, the cell is asked; a cell that cannot complete is not asked again until
    /// the windows close.
    #[test]
    fn t2557_a_scene_cell_waits_for_the_guard_and_a_failed_one_is_not_retried() {
        crate::plugins::install();
        let svc = FxThumbService::start();
        // the lane must not take the job while the test looks at the queue
        svc.bump_generation();
        let ctx = egui::Context::default();
        let mut p = scene_project();
        p.clips.push(clip_at(1, 1, 0, 0, 90_000, vec![]));
        let sc = scene(p, Place::Clip(kt_model::ClipId(1)), 0);
        let fx = [mirror()];
        let mut early = PickerCtx::for_scene(&svc, sc.clone(), false);
        assert!(early.texture(&ctx, &fx).is_none());
        assert!(!svc.pending(), "a cell asked before the misclick guard");
        let key = FxThumbKey { base: early.base, recipe: recipe_hash(&fx, 0.0, 0, None) };
        svc.shared.failed.lock().unwrap().insert(key);
        let mut late = PickerCtx::for_scene(&svc, sc.clone(), true);
        assert!(late.texture(&ctx, &fx).is_none());
        assert!(!svc.queued(&key), "a cell that could not complete was asked again");
        svc.forget_scene();
        assert!(!svc.failed(&key), "closing the windows kept the failed cells");
    }

    /// t2558 (chain-everywhere): a scene cell's job whose picker moved on starts nothing (a stream
    /// history's pre-roll obeys the same stop between positions) and comes out as stopped.
    #[test]
    fn t2558_a_stopped_scene_cell_publishes_nothing() {
        crate::plugins::install();
        let mut p = scene_project();
        p.clips.push(clip_at(1, 1, 0, 0, 90_000, vec![]));
        let sc = scene(p, Place::Clip(kt_model::ClipId(1)), 45_000);
        let mut slot = None;
        let out = render_scene(&mut slot, &sc, &[mirror()], None, || true);
        assert!(matches!(out, SceneOut::Stopped), "a stopped cell came out");
    }
}
