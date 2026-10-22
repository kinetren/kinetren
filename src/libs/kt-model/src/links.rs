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
    let curves: &[ParamCurve] = match &path.owner {
        ParamOwner::FxSlot { slot } => c.fx_auto.get(*slot).map(|v| &v[..]).unwrap_or(&[]),
        ParamOwner::Generator => &c.gen_auto,
        _ => &[],
    };
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
    let any = clips
        .iter()
        .any(|c| c.fx_auto.iter().flatten().chain(c.gen_auto.iter()).any(|x| x.needs_resolve()));
    if !any {
        return std::borrow::Cow::Borrowed(clips);
    }
    let mut out = clips.to_vec();
    // order: first the events WITHOUT derived sources (they are the leaders),
    // then the rest. A chain longer than two links is resolved by repeated
    // passes — there are no more of them than events, and the cycle has already been rejected.
    for _ in 0..=out.len() {
        let snapshot = out.clone();
        let mut moved = false;
        for i in 0..out.len() {
            let me = LinkCtx::of(&snapshot[i], frame_step);
            let id = snapshot[i].id;
            for slot in 0..out[i].fx_auto.len() {
                for j in 0..out[i].fx_auto[slot].len() {
                    if resolve_one(&snapshot, &mut out[i].fx_auto[slot][j], &me, id, frame_step) {
                        moved = true;
                    }
                }
            }
            for j in 0..out[i].gen_auto.len() {
                if resolve_one(&snapshot, &mut out[i].gen_auto[j], &me, id, frame_step) {
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
    std::borrow::Cow::Owned(out)
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
    let step = step.max(1);
    let mut out = Vec::new();
    let mut t = from.0;
    while t <= to.0 {
        // beyond the material's body — the extreme value, as for a resolved curve
        let tm = t.clamp(material.0.0, material.1.0.max(material.0.0));
        let mut vars = std::collections::BTreeMap::new();
        vars.insert("src".to_string(), 0.0);
        vars.insert(
            "k".to_string(),
            if cur.keys.is_empty() { 0.0 } else { crate::curve_value(&cur.keys, Tick(tm)) },
        );
        vars.insert("t".to_string(), (tm - source_in.0) as f64 / 90_000.0);
        match f.eval(&vars) {
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
    if !cur.needs_resolve() {
        return false;
    }
    let expr = cur.source.expr().and_then(|e| crate::formula::Formula::parse(e).ok());
    // `k` is the value of the KEYS WITH SHAPES under the layer (R.67); without keys — 0,
    // and the source formula of older files sounds as before
    let base_keys = cur.keys.clone();
    let path = cur.source.link_path().and_then(LinkPath::parse);
    let step = frame_step.max(1);
    let from = me.source_in.0;
    let to = from + me.duration.0.max(step);
    let mut keys = Vec::new();
    let mut t = from;
    while t <= to {
        let at = Tick(t);
        let src = match &path {
            Some(p) => {
                let p = LinkPath { clip: p.clip.or(Some(me_id)), ..p.clone() };
                match read_link_in(clips, me, &p, at) {
                    Some(v) => v,
                    // the leader is not yet resolved — we wait for the next pass
                    None => return false,
                }
            }
            None => 0.0,
        };
        let k = if base_keys.is_empty() { 0.0 } else { crate::curve_value(&base_keys, at) };
        let v = match &expr {
            Some(f) => {
                let mut vars = std::collections::BTreeMap::new();
                vars.insert("src".to_string(), src);
                vars.insert("k".to_string(), k);
                vars.insert("t".to_string(), (t - from) as f64 / 90_000.0);
                match f.eval(&vars) {
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

/// The same as `read_link`, but over a slice of events (the reel is already parsed).
fn read_link_in(clips: &[ClipEvent], me: &LinkCtx, path: &LinkPath, at: Tick) -> Option<f64> {
    let want = path.clip?;
    let src = clips.iter().find(|c| crate::same_clip(c.id, want))?;
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
        _ => false,
    };
    if pending {
        return None;
    }
    let tl = clip_timeline_at(me.source_in, me.start, me.duration, me.reversed, me.frame_step, at);
    let lo = src.start.0;
    let hi = src.start.0 + src.duration.0.max(1);
    let tl = Tick(tl.0.clamp(lo, hi));
    let mat =
        clip_material_at(src.source_in, src.start, src.duration, src.reversed, me.frame_step, tl);
    read_at_clip(src, path, mat)
}

fn read_at_clip(c: &ClipEvent, path: &LinkPath, mat: Tick) -> Option<f64> {
    let curves: &[ParamCurve] = match &path.owner {
        ParamOwner::FxSlot { slot } => c.fx_auto.get(*slot).map(|v| &v[..]).unwrap_or(&[]),
        ParamOwner::Generator => &c.gen_auto,
        _ => &[],
    };
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
        _ => None,
    }
}
