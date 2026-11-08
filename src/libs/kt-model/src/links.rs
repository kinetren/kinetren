//! Link: a value follows a DIFFERENT property (R.61).
//!
//! **Owner's decision:** ANY event of the project may be addressed, not only
//! its own. Exactly one constraint follows from the invariants, and it does not narrow
//! the decision, but explains where the “project” edge is: **a link does not cross the reel
//! boundary**. Outward — because a reel that reads an event from outside stops
//! being self-sufficient, two of its instances would give different pictures, and law
//! E (an embedding is identical to the expanded content) would stop
//! holding. Inward — because “setting a value inside from outside” is already
//! done and is called the public parameters of a composition (R.59).
//!
//! **Time between DIFFERENT events is the TIMELINE tick.** The key's domain is the material
//! of its own event, and two events at different places in the edit have no common
//! material time. The conversion goes through `clip_timeline_at` of its own
//! event and `clip_material_at` of the source — the same pair the whole program uses;
//! we do not introduce a second conversion rule. Outside the source's body
//! its extreme value is taken.

use crate::project::*;
use crate::published::ParamOwner;
use kt_time::Tick;

/// A parsed address of the leading property.
///
/// Text form `"<event>/<owner>/<parameter>"`:
/// `"12/fx0/radius"`, `"12/gen/size"`, `"12/clip/opacity"`. The event is a number
/// `ClipId`; `self` means “this same event” and does not require knowing it.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkPath {
    /// `None` — its own event.
    pub clip: Option<ClipId>,
    pub owner: ParamOwner,
    pub param: String,
}

impl LinkPath {
    /// Parse the address text. `None` — the address does not match the form.
    pub fn parse(s: &str) -> Option<LinkPath> {
        let mut it = s.split('/');
        let who = it.next()?;
        let owner = it.next()?;
        let param = it.next()?.to_string();
        if it.next().is_some() || param.is_empty() {
            return None;
        }
        let clip = if who == "self" { None } else { Some(ClipId(who.parse().ok()?)) };
        let owner = match owner {
            "gen" => ParamOwner::Generator,
            "clip" => ParamOwner::ClipAttr,
            o if o.starts_with("mask") => ParamOwner::Mask { slot: None, mask: o[4..].parse().ok()? },
            o if o.starts_with("fx") && o.contains(".mask") => {
                let (f, m) = o[2..].split_once(".mask")?;
                ParamOwner::Mask { slot: Some(f.parse().ok()?), mask: m.parse().ok()? }
            }
            o if o.starts_with("fx") => ParamOwner::FxSlot { slot: o[2..].parse().ok()? },
            _ => return None,
        };
        Some(LinkPath { clip, owner, param })
    }

    pub fn to_text(&self) -> String {
        let who = match self.clip {
            None => "self".to_string(),
            Some(c) => c.0.to_string(),
        };
        let owner = match &self.owner {
            ParamOwner::Generator => "gen".to_string(),
            ParamOwner::ClipAttr => "clip".to_string(),
            ParamOwner::FxSlot { slot } => format!("fx{slot}"),
            ParamOwner::Mask { slot: None, mask } => format!("mask{mask}"),
            ParamOwner::Mask { slot: Some(f), mask } => format!("fx{f}.mask{mask}"),
            _ => "?".to_string(),
        };
        format!("{who}/{owner}/{}", self.param)
    }
}

/// Who asks: the consuming event and its geometry.
#[derive(Debug, Clone, Copy)]
pub struct LinkCtx {
    pub source_in: Tick,
    pub start: Tick,
    pub duration: Tick,
    pub reversed: bool,
    pub frame_step: i64,
}

impl LinkCtx {
    pub fn of(c: &ClipEvent, frame_step: i64) -> LinkCtx {
        LinkCtx {
            source_in: c.source_in,
            start: c.start,
            duration: c.duration,
            reversed: c.reversed,
            frame_step,
        }
    }
}

/// The value of the leading property at the CONSUMER's material position.
///
/// Within ONE event no conversion is needed — the domain is shared. Between different
/// events the common denominator is computed: its own material position → timeline tick →
/// the source's material.
pub fn read_link(reel: &dyn Reel, me: &LinkCtx, path: &LinkPath, at: Tick) -> Option<f64> {
    let clips = reel.clips();
    let src = match path.clip {
        None => return read_at(reel, None, path, at),
        Some(id) => clips.iter().find(|c| crate::same_clip(c.id, id))?,
    };
    // the common denominator — the TIMELINE
    let tl = clip_timeline_at(me.source_in, me.start, me.duration, me.reversed, me.frame_step, at);
    // outside the source's body its extreme value is taken: we clamp the tick, and do not
    // stay silent — otherwise the link would “vanish” at the edges, which would look like a
    // random breakage
    // the bounds INCLUDE the end of the event: the curve's last key sits exactly there,
    // and a clamp one tick earlier would lose it
    let lo = src.start.0;
    let hi = src.start.0 + src.duration.0.max(1);
    let tl = Tick(tl.0.clamp(lo, hi));
    let mat =
        clip_material_at(src.source_in, src.start, src.duration, src.reversed, me.frame_step, tl);
    read_at(reel, Some(src), path, mat)
}

/// The value of the owner's property at ITS material position.
fn read_at(reel: &dyn Reel, src: Option<&ClipEvent>, path: &LinkPath, mat: Tick) -> Option<f64> {
    let c = match src {
        Some(c) => c,
        None => reel.clips().iter().find(|c| c.id == path.clip.unwrap_or(c.id))?,
    };
    let curves: &[ParamCurve] = owner_curves(c, &path.owner);
    // the live curve of the leading property outranks the handle — the same semantics as
    // the rest of the program
    if let Some(cur) = curves.iter().find(|x| x.param == path.param)
        && !cur.keys.is_empty()
        && cur.source.is_keys()
    {
        return Some(curve_value(&cur.keys, mat));
    }
    match &path.owner {
        ParamOwner::FxSlot { slot } => c.fx.get(*slot)?.get_param(&path.param),
        ParamOwner::Generator => c.r#gen.as_ref()?.get_param(&path.param),
        ParamOwner::ClipAttr => match path.param.as_str() {
            "opacity" => Some(c.opacity as f64),
            "gain_db" => Some(c.gain_db as f64),
            _ => None,
        },
        ParamOwner::Mask { slot, mask } => {
            c.masks.list(*slot).iter().find(|m| m.id == *mask)?.get(&path.param)
        }
        _ => None,
    }
}

/// The value of a NON-NUMERIC target by the marks of the value axis (R.61).
///
/// Between marks the PREVIOUS snapshot holds; below the first — the first: the rig
/// switches a variant as a step, there is no “half” between variants.
pub fn stop_value(stops: &[LinkStop], driver: f64) -> Option<&ParamValue> {
    if stops.is_empty() {
        return None;
    }
    let mut best = &stops[0];
    for s in stops {
        if driver >= s.at && s.at >= best.at {
            best = s;
        }
    }
    // below the first mark — the first snapshot
    let first = stops.iter().min_by(|a, b| a.at.total_cmp(&b.at))?;
    Some(if driver < first.at { &first.value } else { &best.value })
}

// ---- reel boundary and cycles (R.61) ------------------------------------------

/// Check the links of ONE reel: the addresses are resolvable, the boundary is not crossed,
/// and there are no cycles.
///
/// A cycle is searched by a traversal from ALL vertices, not from the root: a pair of properties
/// that lead each other and are read by no one is still a cycle — a file with it would be saved,
/// and would open with an error. The same technique as for reel cycles.
pub fn check_links(reel: &dyn Reel) -> Result<(), LinkError> {
    let clips = reel.clips();
    // a graph vertex — a “property”: (event, owner-in-text, parameter)
    let mut edges: Vec<(String, String)> = Vec::new();
    for c in clips {
        for (slot, curves) in c.fx_auto.iter().enumerate() {
            for cur in curves {
                collect(c, &ParamOwner::FxSlot { slot }, cur, clips, &mut edges)?;
            }
        }
        for cur in &c.gen_auto {
            collect(c, &ParamOwner::Generator, cur, clips, &mut edges)?;
        }
        for (slot, m) in c.masks.iter() {
            for cur in &m.auto {
                collect(c, &ParamOwner::Mask { slot, mask: m.id }, cur, clips, &mut edges)?;
            }
        }
    }
    if let Some(path) = find_cycle(&edges) {
        return Err(LinkError::Cycle(path));
    }
    Ok(())
}

fn node_name(clip: ClipId, owner: &ParamOwner, param: &str) -> String {
    LinkPath { clip: Some(clip), owner: owner.clone(), param: param.to_string() }.to_text()
}

fn collect(
    c: &ClipEvent,
    owner: &ParamOwner,
    cur: &ParamCurve,
    clips: &[ClipEvent],
    edges: &mut Vec<(String, String)>,
) -> Result<(), LinkError> {
    let Some(text) = cur.source.link_path() else {
        // the formula of addresses does not exist: the names in it are variables, resolved by
        // whoever builds the plan (stage 4)
        return Ok(());
    };
    let path = LinkPath::parse(text).ok_or_else(|| LinkError::BadPath(text.to_string()))?;
    // the address must point to an existing event of THIS reel: crossing the boundary
    // is impossible neither outward nor inward — see the module header
    if let Some(id) = path.clip
        && !clips.iter().any(|x| crate::same_clip(x.id, id))
    {
        return Err(LinkError::OutsideReel { from: c.id, to: id });
    }
    let to = node_name(path.clip.unwrap_or(c.id), &path.owner, &path.param);
    edges.push((node_name(c.id, owner, &cur.param), to));
    Ok(())
}

/// A traversal FROM ALL vertices: a pair that leads each other and is read by no one —
/// is also a cycle.
fn find_cycle(edges: &[(String, String)]) -> Option<Vec<String>> {
    let mut stack: Vec<String> = Vec::new();
    let mut done: Vec<String> = Vec::new();
    let starts: Vec<&String> = edges.iter().map(|(a, _)| a).collect();
    for s in starts {
        if done.contains(s) {
            continue;
        }
        if let Some(p) = walk(s, edges, &mut stack, &mut done) {
            return Some(p);
        }
    }
    None
}

fn walk(
    at: &str,
    edges: &[(String, String)],
    stack: &mut Vec<String>,
    done: &mut Vec<String>,
) -> Option<Vec<String>> {
    if let Some(i) = stack.iter().position(|x| x == at) {
        let mut p = stack[i..].to_vec();
        p.push(at.to_string());
        return Some(p);
    }
    if done.iter().any(|x| x == at) {
        return None;
    }
    stack.push(at.to_string());
    for (a, b) in edges {
        if a == at
            && let Some(p) = walk(b, edges, stack, done)
        {
            return Some(p);
        }
    }
    stack.pop();
    done.push(at.to_string());
    None
}

/// What makes a link illegal.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LinkError {
    #[error("link address {0:?} is not of the form <clip>/<owner>/<param>")]
    BadPath(String),
    #[error(
        "clip {from:?} links to clip {to:?}, which is outside its reel — a reel that reads \
         events from outside stops being self-contained; publish a composition parameter instead"
    )]
    OutsideReel { from: ClipId, to: ClipId },
    #[error("properties drive each other in a circle: {}", .0.join(" -> "))]
    Cycle(Vec<String>),
}

// ---- resolution of derived sources (R.61) ------------------------------------------

/// Resolve the links and formulas of the reel into ORDINARY keyed curves.
///
/// Called ONCE when the renderer is built — the same place where curves become
/// sound profiles; no computations appear in the audio callback.
///
/// **Zero cost without use.** A reel where not a single derived source exists is
/// returned BORROWED: no dependency graph is built at all, not a single clone.
/// This is not an optimization but the owner's requirement “a simple project
/// must not lag”.
///
/// **Order is topological.** The leading property must be resolved before the led one,
/// otherwise a link on a link would read raw text. Cycles can no longer exist here:
/// validation has already rejected them.
///
/// **Discretization is the frame grid.** A derived curve turns into keys on
/// the same grid on which any other curve is read:
/// a human key cannot be placed any finer than that either.
pub fn resolve_derived<'a>(
    clips: &'a [ClipEvent],
    frame_step: i64,
) -> std::borrow::Cow<'a, [ClipEvent]> {
    // what `resolve_curve` would change: a speed curve without a link is left to
    // `rate_profile_keys`, so it alone neither clones the reel nor an event
    let pending = |x: &ParamCurve| x.needs_resolve() && !(x.param == "rate" && x.source.link_path().is_none());
    let derived = |c: &ClipEvent| {
        c.fx_auto.iter().flatten().chain(c.gen_auto.iter()).any(pending)
            || c.masks.iter().any(|(_, m)| m.auto.iter().any(pending))
    };
    if !clips.iter().any(derived) {
        return std::borrow::Cow::Borrowed(clips);
    }
    let mut out = clips.to_vec();
    // The events by id (`same_clip`: the event or a piece of it, the first), and the order
    // LEADERS FIRST (builder-cost-guard): the former passes went over every event in file order
    // and resolved one more link of a chain per pass, so a chain of n links cost n passes over
    // the reel, each with a scan per leader. In the leaders-first order one pass resolves
    // everything a curve of another event leads; the loop stays for a link between two curves
    // of one event. A leader's resolved keys do not depend on the pass that resolved them, so
    // the curves are the former ones.
    let mut by_id: std::collections::HashMap<ClipId, usize> = std::collections::HashMap::new();
    for (i, c) in out.iter().enumerate() {
        by_id.entry(c.id).or_insert(i);
        if let Some(o) = crate::slice_origin(c.id) {
            by_id.entry(o).or_insert(i);
        }
    }
    let order = leaders_first(&out, &by_id);
    for _ in 0..=out.len() {
        let mut moved = false;
        for &i in &order {
            // an event with nothing left to resolve is not cloned per pass (compiled-curves): every
            // one of its curves would return at once, the event as it was
            if !derived(&out[i]) {
                continue;
            }
            let me = LinkCtx::of(&out[i], frame_step);
            let id = out[i].id;
            // the event's curves are taken out while it resolves against the reel; a link to its
            // own other curve reads the event as it was before this step, as the former pass
            // snapshot gave it
            let own = out[i].clone();
            let mut fx_auto = std::mem::take(&mut out[i].fx_auto);
            let mut gen_auto = std::mem::take(&mut out[i].gen_auto);
            let mut masks = std::mem::take(&mut out[i].masks);
            let reel = Leaders { clips: &out, by_id: &by_id, own: (i, &own) };
            for cur in fx_auto.iter_mut().flatten().chain(gen_auto.iter_mut()) {
                moved |= resolve_with(&reel, cur, &me, id, frame_step);
            }
            for m in masks.picture.iter_mut().chain(masks.slots.iter_mut().flatten()) {
                for cur in &mut m.auto {
                    moved |= resolve_with(&reel, cur, &me, id, frame_step);
                }
            }
            out[i].fx_auto = fx_auto;
            out[i].gen_auto = gen_auto;
            out[i].masks = masks;
        }
        if !moved {
            break;
        }
    }
    std::borrow::Cow::Owned(out)
}

/// The events of a reel and their index by id, for the leaders' lookups.
struct Leaders<'a> {
    clips: &'a [ClipEvent],
    by_id: &'a std::collections::HashMap<ClipId, usize>,
    /// the event being resolved, as it was before its step
    own: (usize, &'a ClipEvent),
}

/// The event indices with every link's leader before its follower (a depth-first post-order
/// over the links; a cycle, already rejected by validation, is cut where it closes).
fn leaders_first(clips: &[ClipEvent], by_id: &std::collections::HashMap<ClipId, usize>) -> Vec<usize> {
    let leads = |i: usize| -> Vec<usize> {
        let c = &clips[i];
        let curves = c
            .fx_auto
            .iter()
            .flatten()
            .chain(c.gen_auto.iter())
            .chain(c.masks.iter().flat_map(|(_, m)| m.auto.iter()));
        curves
            .filter_map(|x| x.source.link_path().and_then(LinkPath::parse))
            .filter_map(|p| p.clip.and_then(|id| by_id.get(&id).copied()))
            .filter(|&j| j != i)
            .collect()
    };
    let mut state = vec![0u8; clips.len()]; // 0 new, 1 on the stack, 2 placed
    let mut order = Vec::with_capacity(clips.len());
    for root in 0..clips.len() {
        if state[root] != 0 {
            continue;
        }
        let mut stack: Vec<(usize, Vec<usize>)> = vec![(root, leads(root))];
        state[root] = 1;
        while let Some((i, next)) = stack.last_mut() {
            match next.pop() {
                Some(j) if state[j] == 0 => {
                    state[j] = 1;
                    let l = leads(j);
                    stack.push((j, l));
                }
                Some(_) => {}
                None => {
                    let i = *i;
                    state[i] = 2;
                    order.push(i);
                    stack.pop();
                }
            }
        }
    }
    order
}

/// Values of a FORMULA curve over a window — for display (R.63 item 3, R.67).
///
/// The same evaluator and the same variables as the renderer (`resolve_one`):
/// `t` is seconds from the START OF THE MATERIAL (`source_in`), not from the window's
/// edge — otherwise the preview line would diverge from the render and drift on
/// scrolling (R.67); `k` is the value of keys with shapes under the layer;
/// `src` is the value of the leader, which a pure formula does not have (zero).
/// `material` is the body of the event's material (`source_in..end`): beyond it the render
/// holds the extreme value (the curve is resolved only within the body),
/// and the preview must hold the same. The window `from..to` is only
/// the sampling range.
///
/// It returns empty if the source is not a formula or its text is not computable: lying
/// with a line is worse than not drawing.
pub fn formula_preview(
    cur: &ParamCurve,
    material: (Tick, Tick),
    from: Tick,
    to: Tick,
    step: i64,
) -> Vec<AutoKey> {
    let source_in = material.0;
    let Some(expr) = cur.source.expr() else { return Vec::new() };
    let Ok(f) = crate::formula::Formula::parse(expr) else { return Vec::new() };
    // compiled once (compiled-curves): the slots are `LAYER_VARS` — t, src, k
    let f = f.compile(crate::LAYER_VARS);
    let step = step.max(1);
    let mut out = Vec::new();
    let mut under = crate::CurveCursor::new(&cur.keys);
    let mut t = from.0;
    while t <= to.0 {
        // beyond the material's body — the extreme value, as for a resolved curve
        let tm = t.clamp(material.0.0, material.1.0.max(material.0.0));
        let k = if cur.keys.is_empty() { 0.0 } else { under.value(Tick(tm)) };
        match f.eval(&[(tm - source_in.0) as f64 / 90_000.0, 0.0, k]) {
            Ok(v) => out.push(AutoKey::shaped(Tick(t), v, SegShape::Linear)),
            Err(_) => return Vec::new(),
        }
        t += step;
    }
    out
}

/// Resolve ONE curve in the GIVEN material (R.67): overrides of a reel instance are
/// resolved in the reel's domain before the conversion to the target's time
/// — `me` describes the reel as material (`source_in` 0, the reel's length), and
/// `clips` is its content, by which the links are read.
pub fn resolve_curve_in(
    clips: &[ClipEvent],
    cur: &mut ParamCurve,
    me: &LinkCtx,
    me_id: ClipId,
    frame_step: i64,
) -> bool {
    resolve_one(clips, cur, me, me_id, frame_step)
}

/// Resolve ONE curve. `true` — something changed.
fn resolve_one(
    clips: &[ClipEvent],
    cur: &mut ParamCurve,
    me: &LinkCtx,
    me_id: ClipId,
    frame_step: i64,
) -> bool {
    resolve_curve(cur, me, me_id, frame_step, |p| link_leader(clips, p))
}

/// [`resolve_one`] with the leader found by the reel's index.
fn resolve_with(reel: &Leaders, cur: &mut ParamCurve, me: &LinkCtx, me_id: ClipId, frame_step: i64) -> bool {
    resolve_curve(cur, me, me_id, frame_step, |p| {
        let i = *reel.by_id.get(&p.clip?)?;
        leader_ready(if i == reel.own.0 { reel.own.1 } else { &reel.clips[i] }, p)
    })
}

fn resolve_curve<'a>(
    cur: &mut ParamCurve,
    me: &LinkCtx,
    me_id: ClipId,
    frame_step: i64,
    leader_of: impl FnOnce(&LinkPath) -> Option<&'a ClipEvent>,
) -> bool {
    if !cur.needs_resolve() {
        return false;
    }
    // SPEED without a link is sampled by `rate_profile_keys` for every reader, the raw event's
    // included: resolving it here on the frame grid would give the render a profile the
    // interface does not see (slowed-reverb-is-an-event, D2; t913)
    if cur.param == "rate" && cur.source.link_path().is_none() {
        return false;
    }
    // compiled once per curve (compiled-curves): the slots are `LAYER_VARS` — t, src, k
    let expr = cur
        .source
        .expr()
        .and_then(|e| crate::formula::Formula::parse(e).ok())
        .map(|f| f.compile(crate::LAYER_VARS));
    // `k` is the value of the KEYS WITH SHAPES under the layer (R.67); without keys — 0,
    // and the source formula of older files sounds as before
    let base_keys = cur.keys.clone();
    let path = cur.source.link_path().and_then(LinkPath::parse);
    let step = frame_step.max(1);
    let from = me.source_in.0;
    let to = from + me.duration.0.max(step);
    // the leader is found ONCE per curve (builder-cost-guard): it does not depend on the
    // sample, and finding it per sample scanned the reel's events per frame of the curve
    let path = path.map(|p| LinkPath { clip: p.clip.or(Some(me_id)), ..p });
    let leader = match &path {
        Some(p) => match leader_of(p) {
            Some(c) => Some(c),
            // the leader is not yet resolved — we wait for the next pass
            None => return false,
        },
        None => None,
    };
    let mut keys = Vec::new();
    let mut under = crate::CurveCursor::new(&base_keys);
    let mut t = from;
    while t <= to {
        let at = Tick(t);
        let src = match (&path, leader) {
            (Some(p), Some(c)) => match read_link_from(c, me, p, at) {
                Some(v) => v,
                None => return false,
            },
            _ => 0.0,
        };
        let k = if base_keys.is_empty() { 0.0 } else { under.value(at) };
        let v = match &expr {
            Some(f) => {
                match f.eval(&[(t - from) as f64 / 90_000.0, src, k]) {
                    Ok(v) => v,
                    // not computable — not a silent zero: the curve stays raw,
                    // and validation has already said so loudly
                    Err(_) => return false,
                }
            }
            // a link without recomputation — the leader; keys with a segment formula —
            // their own value, expanded onto the grid
            None => {
                if path.is_some() {
                    src
                } else {
                    k
                }
            }
        };
        keys.push(AutoKey::shaped(at, v, SegShape::Linear));
        t += step;
    }
    if keys.is_empty() {
        return false;
    }
    cur.keys = keys;
    cur.source = CurveSource::Keys;
    true
}

/// The leader of a link in a slice of events (the reel is already parsed), or `None` while it
/// is itself not yet resolved.
fn link_leader<'a>(clips: &'a [ClipEvent], path: &LinkPath) -> Option<&'a ClipEvent> {
    let want = path.clip?;
    let at = clips.iter().position(|c| crate::same_clip(c.id, want));
    kt_time::cost::add(kt_time::cost::Cost::ClipSearch, at.map_or(clips.len(), |i| i + 1) as u64);
    leader_ready(&clips[at?], path)
}

/// The leader itself, or `None` while its own curve is not yet resolved.
fn leader_ready<'a>(src: &'a ClipEvent, path: &LinkPath) -> Option<&'a ClipEvent> {
    // the leading property, itself not yet resolved, cannot be read: one would get
    // a value “before the link”, that is, not what a person will see
    let pending = match &path.owner {
        ParamOwner::FxSlot { slot } => src
            .fx_auto
            .get(*slot)
            .is_some_and(|v| v.iter().any(|c| c.param == path.param && !c.source.is_keys())),
        ParamOwner::Generator => {
            src.gen_auto.iter().any(|c| c.param == path.param && !c.source.is_keys())
        }
        ParamOwner::Mask { .. } => owner_curves(src, &path.owner)
            .iter()
            .any(|c| c.param == path.param && !c.source.is_keys()),
        _ => false,
    };
    if pending {
        return None;
    }
    Some(src)
}

/// The leader's value at the follower's material moment `at`.
fn read_link_from(src: &ClipEvent, me: &LinkCtx, path: &LinkPath, at: Tick) -> Option<f64> {
    let tl = clip_timeline_at(me.source_in, me.start, me.duration, me.reversed, me.frame_step, at);
    let lo = src.start.0;
    let hi = src.start.0 + src.duration.0.max(1);
    let tl = Tick(tl.0.clamp(lo, hi));
    let mat =
        clip_material_at(src.source_in, src.start, src.duration, src.reversed, me.frame_step, tl);
    read_at_clip(src, path, mat)
}

fn read_at_clip(c: &ClipEvent, path: &LinkPath, mat: Tick) -> Option<f64> {
    let curves: &[ParamCurve] = owner_curves(c, &path.owner);
    if let Some(cur) = curves.iter().find(|x| x.param == path.param)
        && !cur.keys.is_empty()
        && cur.source.is_keys()
    {
        return Some(curve_value(&cur.keys, mat));
    }
    match &path.owner {
        ParamOwner::FxSlot { slot } => c.fx.get(*slot)?.get_param(&path.param),
        ParamOwner::Generator => c.r#gen.as_ref()?.get_param(&path.param),
        ParamOwner::ClipAttr => match path.param.as_str() {
            "opacity" => Some(c.opacity as f64),
            "gain_db" => Some(c.gain_db as f64),
            _ => None,
        },
        ParamOwner::Mask { slot, mask } => {
            c.masks.list(*slot).iter().find(|m| m.id == *mask)?.get(&path.param)
        }
        _ => None,
    }
}

/// The curves an owner holds on an event (the mask's own, the slot's, the generator's).
fn owner_curves<'a>(c: &'a ClipEvent, owner: &ParamOwner) -> &'a [ParamCurve] {
    match owner {
        ParamOwner::FxSlot { slot } => c.fx_auto.get(*slot).map(|v| &v[..]).unwrap_or(&[]),
        ParamOwner::Generator => &c.gen_auto,
        ParamOwner::Mask { slot, mask } => {
            c.masks.list(*slot).iter().find(|m| m.id == *mask).map_or(&[][..], |m| &m.auto[..])
        }
        _ => &[],
    }
}
