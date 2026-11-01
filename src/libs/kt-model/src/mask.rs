//! Classic shape masks (shape-masks, kkt-26.11.01 S1).
//!
//! A mask is a closed shape in the frame of its owner — a rectangle, an ellipse or a Bézier
//! path — with feather, falloff, expansion, opacity, invert and a Boolean mode. An ORDERED list
//! combines top to bottom (the rasterizer — `kt_fx::mask`). Three owners hold the same type:
//! the event's picture and each effect slot (`ClipEvent.masks`, source space of the event), and a
//! track (`Track.masks`, canvas space).
//!
//! Coordinates are NORMALIZED to the owner's reference frame (x across its width, y down its
//! height), so a mask covers the same region at every preview quality and survives a relink
//! of another resolution. Shapes are generated in reference PIXELS (`ref_px`), so a rotation and
//! a circle are aspect-correct. Feather and expansion are reference px.
//!
//! Every number is a passport row (`Mask::specs`), its curves are `ParamCurve`s held by the mask
//! (`Mask.auto`, the owner's domain). A vertex has a stable id — its rows are `v<id>.x` … and a
//! deletion never renames another vertex (the tracker of T1 links to these names).

use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::{HasParams, ParamKind, ParamSpec, ParamUnit};
use kt_time::Tick;
use serde::{Deserialize, Serialize};

use crate::project::{AutoKey, ParamCurve, SegShape, apply_curves, curve_value, insert_key};

/// Address of a mask inside its list. Unique within the list, never reused there.
pub type MaskId = u32;

fn is_false(v: &bool) -> bool {
    !*v
}
fn is_one(v: &f64) -> bool {
    *v == 1.0
}
fn is_zero(v: &f64) -> bool {
    *v == 0.0
}
fn one() -> f64 {
    1.0
}

/// How a mask combines with the masks ABOVE it — the soft forms of the Boolean operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskMode {
    /// Union: `a + m − a·m`.
    #[default]
    Add,
    /// `a · (1 − m)`.
    Subtract,
    /// `a · m`.
    Intersect,
    /// `a + m − 2·a·m` (XOR on hard masks).
    Difference,
}

impl MaskMode {
    pub const ALL: [MaskMode; 4] =
        [MaskMode::Add, MaskMode::Subtract, MaskMode::Intersect, MaskMode::Difference];
    pub fn tag(self) -> &'static str {
        match self {
            MaskMode::Add => "add",
            MaskMode::Subtract => "subtract",
            MaskMode::Intersect => "intersect",
            MaskMode::Difference => "difference",
        }
    }
    fn is_add(&self) -> bool {
        *self == MaskMode::Add
    }
}

/// The profile of the feather across the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Falloff {
    /// Smoothstep — AE's default "Smooth".
    #[default]
    Smooth,
    Linear,
}

impl Falloff {
    pub const ALL: [Falloff; 2] = [Falloff::Smooth, Falloff::Linear];
    pub fn tag(self) -> &'static str {
        match self {
            Falloff::Smooth => "smooth",
            Falloff::Linear => "linear",
        }
    }
    fn is_smooth(&self) -> bool {
        *self == Falloff::Smooth
    }
}

/// A vertex of a Bézier path. The tangent handles are RELATIVE to the vertex: moving the
/// vertex (by hand or by a tracker driving `x/y` only) carries them along.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    /// Incoming handle (towards the previous vertex).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ix: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub iy: f64,
    /// Outgoing handle (towards the next vertex).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ox: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub oy: f64,
}

impl Vertex {
    pub fn corner(id: u32, x: f64, y: f64) -> Vertex {
        Vertex { id, x, y, ix: 0.0, iy: 0.0, ox: 0.0, oy: 0.0 }
    }
    /// A corner has no handles; a smooth vertex has at least one.
    pub fn is_corner(&self) -> bool {
        self.ix == 0.0 && self.iy == 0.0 && self.ox == 0.0 && self.oy == 0.0
    }
}

/// The shape of a mask. Primitives keep their few numbers (they animate and track as a
/// centre and a size); a path is a list of vertices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum MaskShape {
    /// Centre and size are normalized; `rot` in degrees (clockwise on the screen);
    /// `round` 0..1 — the corner radius as a share of half the shorter side.
    Rect {
        cx: f64,
        cy: f64,
        w: f64,
        h: f64,
        #[serde(default, skip_serializing_if = "is_zero")]
        rot: f64,
        #[serde(default, skip_serializing_if = "is_zero")]
        round: f64,
    },
    Ellipse {
        cx: f64,
        cy: f64,
        w: f64,
        h: f64,
        #[serde(default, skip_serializing_if = "is_zero")]
        rot: f64,
    },
    /// A CLOSED path of cubic segments; at least three vertices.
    Path { verts: Vec<Vertex> },
}

impl MaskShape {
    pub fn kind_tag(&self) -> &'static str {
        match self {
            MaskShape::Rect { .. } => "rect",
            MaskShape::Ellipse { .. } => "ellipse",
            MaskShape::Path { .. } => "path",
        }
    }
}

/// One mask.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mask {
    pub id: MaskId,
    pub name: String,
    #[serde(flatten)]
    pub shape: MaskShape,
    #[serde(default, skip_serializing_if = "MaskMode::is_add")]
    pub mode: MaskMode,
    #[serde(default, skip_serializing_if = "is_false")]
    pub invert: bool,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f64,
    /// Width of the soft edge, reference px, centred on the contour.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub feather: f64,
    #[serde(default, skip_serializing_if = "Falloff::is_smooth")]
    pub falloff: Falloff,
    /// Moves the edge outward (positive) or inward (choke), reference px.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub expansion: f64,
    /// Curves on this mask's rows; the owner's domain (an event's material tick).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto: Vec<ParamCurve>,
}

/// The masks of an event: its picture and each effect slot (parallel to `fx`, shorter — no
/// masks on the rest, like `fx_auto`). One field on the event, so a maskless event is
/// byte for byte as before.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EventMasks {
    /// The picture: applied inside the Geometry node — crop → mask → placement.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub picture: Vec<Mask>,
    /// Per effect slot: the slot's result inside, its input outside.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<Vec<Mask>>,
}

impl EventMasks {
    pub fn is_empty(&self) -> bool {
        self.picture.is_empty() && self.slots.iter().all(Vec::is_empty)
    }
    /// The masks of slot `i` (empty when the list is shorter).
    pub fn slot(&self, i: usize) -> &[Mask] {
        self.slots.get(i).map_or(&[], Vec::as_slice)
    }
    /// The list addressed by `slot` (`None` — the picture), created on demand.
    pub fn list_mut(&mut self, slot: Option<usize>) -> &mut Vec<Mask> {
        match slot {
            None => &mut self.picture,
            Some(i) => {
                if self.slots.len() <= i {
                    self.slots.resize(i + 1, Vec::new());
                }
                &mut self.slots[i]
            }
        }
    }
    pub fn list(&self, slot: Option<usize>) -> &[Mask] {
        match slot {
            None => &self.picture,
            Some(i) => self.slot(i),
        }
    }
    /// Trailing empty slot lists are not kept (the canonical form, byte stability).
    pub fn canonicalize(&mut self) {
        while self.slots.last().is_some_and(Vec::is_empty) {
            self.slots.pop();
        }
    }
    /// Every mask with its slot (`None` — the picture).
    pub fn iter(&self) -> impl Iterator<Item = (Option<usize>, &Mask)> {
        self.picture
            .iter()
            .map(|m| (None, m))
            .chain(self.slots.iter().enumerate().flat_map(|(i, l)| l.iter().map(move |m| (Some(i), m))))
    }
}

/// Limits of the rows — the one source for the passport and the clamp.
pub const FEATHER_MAX: f64 = 1000.0;
pub const EXPANSION_MAX: f64 = 1000.0;
/// A normalized coordinate may lie past the frame (a mask rolling in), but not infinitely.
pub const COORD_LIM: f64 = 4.0;
/// A size: up to 8 frames.
pub const SIZE_MAX: f64 = 8.0;
/// A tangent handle offset.
pub const HANDLE_LIM: f64 = 4.0;

/// The shape rows that are keyed together as "the path" (all but opacity/feather/expansion).
pub fn is_shape_row(name: &str) -> bool {
    !matches!(name, "opacity" | "feather" | "expansion")
}

fn row(name: String, min: f64, max: f64, def: f64, step: f64, unit: ParamUnit) -> ParamSpec {
    ParamSpec {
        name: std::borrow::Cow::Owned(name),
        ..ParamSpec::num("", min, max, def, step, unit, ParamKind::Smooth)
    }
}

/// The six rows of one vertex, in passport order.
pub const VERTEX_ROWS: [&str; 6] = ["x", "y", "ix", "iy", "ox", "oy"];

impl Mask {
    /// A new mask with the default settings.
    pub fn new(id: MaskId, name: impl Into<String>, shape: MaskShape) -> Mask {
        Mask {
            id,
            name: name.into(),
            shape,
            mode: MaskMode::Add,
            invert: false,
            opacity: 1.0,
            feather: 0.0,
            falloff: Falloff::Smooth,
            expansion: 0.0,
            auto: Vec::new(),
        }
    }

    /// The passport of this mask, built at run time (R.59): it follows the shape and the vertices.
    pub fn specs(&self) -> Vec<ParamSpec> {
        let mut v = vec![
            row("opacity".into(), 0.0, 1.0, 1.0, 0.01, ParamUnit::Percent),
            row("feather".into(), 0.0, FEATHER_MAX, 0.0, 1.0, ParamUnit::Px),
            row("expansion".into(), -EXPANSION_MAX, EXPANSION_MAX, 0.0, 1.0, ParamUnit::Px),
        ];
        let prim = |v: &mut Vec<ParamSpec>| {
            v.push(row("cx".into(), -COORD_LIM, COORD_LIM, 0.5, 0.001, ParamUnit::Percent));
            v.push(row("cy".into(), -COORD_LIM, COORD_LIM, 0.5, 0.001, ParamUnit::Percent));
            v.push(row("w".into(), 0.0, SIZE_MAX, 0.5, 0.001, ParamUnit::Percent));
            v.push(row("h".into(), 0.0, SIZE_MAX, 0.5, 0.001, ParamUnit::Percent));
            v.push(row("rot".into(), -3600.0, 3600.0, 0.0, 0.5, ParamUnit::Deg));
        };
        match &self.shape {
            MaskShape::Rect { .. } => {
                prim(&mut v);
                v.push(row("round".into(), 0.0, 1.0, 0.0, 0.01, ParamUnit::Percent));
            }
            MaskShape::Ellipse { .. } => prim(&mut v),
            MaskShape::Path { verts } => {
                for p in verts {
                    for (k, r) in VERTEX_ROWS.iter().enumerate() {
                        let (lo, hi) = if k < 2 { (-COORD_LIM, COORD_LIM) } else { (-HANDLE_LIM, HANDLE_LIM) };
                        v.push(row(format!("v{}.{r}", p.id), lo, hi, 0.0, 0.001, ParamUnit::Percent));
                    }
                }
            }
        }
        v
    }

    /// Reads a row by name.
    pub fn get(&self, name: &str) -> Option<f64> {
        match name {
            "opacity" => return Some(self.opacity),
            "feather" => return Some(self.feather),
            "expansion" => return Some(self.expansion),
            _ => {}
        }
        match &self.shape {
            MaskShape::Rect { cx, cy, w, h, rot, round } => match name {
                "cx" => Some(*cx),
                "cy" => Some(*cy),
                "w" => Some(*w),
                "h" => Some(*h),
                "rot" => Some(*rot),
                "round" => Some(*round),
                _ => None,
            },
            MaskShape::Ellipse { cx, cy, w, h, rot } => match name {
                "cx" => Some(*cx),
                "cy" => Some(*cy),
                "w" => Some(*w),
                "h" => Some(*h),
                "rot" => Some(*rot),
                _ => None,
            },
            MaskShape::Path { verts } => {
                let (id, r) = parse_vertex_row(name)?;
                let p = verts.iter().find(|p| p.id == id)?;
                Some(match r {
                    "x" => p.x,
                    "y" => p.y,
                    "ix" => p.ix,
                    "iy" => p.iy,
                    "ox" => p.ox,
                    "oy" => p.oy,
                    _ => return None,
                })
            }
        }
    }

    /// Writes a row by name, clamped by the passport. `false` — no such row.
    pub fn set(&mut self, name: &str, v: f64) -> bool {
        let Some(spec) = self.specs().into_iter().find(|s| s.name == name) else {
            return false;
        };
        let v = if v.is_finite() { v.clamp(spec.min, spec.max) } else { spec.default };
        match name {
            "opacity" => self.opacity = v,
            "feather" => self.feather = v,
            "expansion" => self.expansion = v,
            _ => match &mut self.shape {
                MaskShape::Rect { cx, cy, w, h, rot, round } => match name {
                    "cx" => *cx = v,
                    "cy" => *cy = v,
                    "w" => *w = v,
                    "h" => *h = v,
                    "rot" => *rot = v,
                    "round" => *round = v,
                    _ => return false,
                },
                MaskShape::Ellipse { cx, cy, w, h, rot } => match name {
                    "cx" => *cx = v,
                    "cy" => *cy = v,
                    "w" => *w = v,
                    "h" => *h = v,
                    "rot" => *rot = v,
                    _ => return false,
                },
                MaskShape::Path { verts } => {
                    let Some((id, r)) = parse_vertex_row(name) else { return false };
                    let Some(p) = verts.iter_mut().find(|p| p.id == id) else { return false };
                    match r {
                        "x" => p.x = v,
                        "y" => p.y = v,
                        "ix" => p.ix = v,
                        "iy" => p.iy = v,
                        "ox" => p.ox = v,
                        "oy" => p.oy = v,
                        _ => return false,
                    }
                }
            },
        }
        true
    }

    /// The property view for the shared machinery (`apply_curves`, the fields, the curve widget).
    pub fn params(&mut self) -> MaskParams<'_> {
        let specs = self.specs();
        MaskParams { mask: self, specs }
    }

    /// The names of the shape rows (the "path" keyed as a whole).
    pub fn shape_rows(&self) -> Vec<String> {
        self.specs().into_iter().map(|s| s.name.into_owned()).filter(|n| is_shape_row(n)).collect()
    }

    /// Does any row have live keys?
    pub fn is_animated(&self) -> bool {
        self.auto.iter().any(|c| !c.keys.is_empty())
    }

    /// Is the shape keyed (the path's stopwatch is on)?
    pub fn path_animated(&self) -> bool {
        self.auto.iter().any(|c| is_shape_row(&c.param) && !c.keys.is_empty())
    }

    /// The mask at the owner's tick `at`: curves applied, curves dropped. A mask without
    /// curves is returned as it is (clone only).
    pub fn at(&self, at: Tick) -> Mask {
        let mut m = Mask { auto: Vec::new(), ..self.clone() };
        if self.auto.iter().any(|c| !c.keys.is_empty()) {
            apply_curves(&mut m.params(), &self.auto, at);
        }
        m
    }

    /// The ticks of the shape's keys (the union over the shape rows), sorted.
    pub fn path_key_ticks(&self) -> Vec<Tick> {
        let mut t: Vec<Tick> = self
            .auto
            .iter()
            .filter(|c| is_shape_row(&c.param))
            .flat_map(|c| c.keys.iter().map(|k| k.t))
            .collect();
        t.sort_by_key(|t| t.0);
        t.dedup();
        t
    }

    /// Put a key at `at` on every shape row with its CURRENT value at that tick — the
    /// stopwatch of the path, and what a gesture on an animated path does after it
    /// changed the static values (`values` — the shape the gesture produced at `at`).
    pub fn key_path(&mut self, at: Tick, values: &Mask, default: SegShape) {
        for name in self.shape_rows() {
            let v = values.get(&name).unwrap_or(0.0);
            set_key(&mut self.auto, &name, at, v, default);
        }
    }

    /// Write the result of a gesture: on an animated path every shape row gets a key at `at`
    /// with the new values (the untouched ones keep their value at `at`); a row with its own live
    /// curve (feather…) gets a key; everything else becomes the static value. Only a gesture
    /// calls this (R.67).
    pub fn write_gesture(&mut self, at: Tick, edited: &Mask, default: SegShape) {
        let path = self.path_animated();
        let before = self.at(at);
        let keyed: Vec<String> =
            self.auto.iter().filter(|c| !c.keys.is_empty()).map(|c| c.param.clone()).collect();
        // structure first: a gesture never changes the vertex SET (insert/delete have their own
        // verbs), but it may change the static flags
        self.mode = edited.mode;
        self.invert = edited.invert;
        self.falloff = edited.falloff;
        for s in edited.specs() {
            let name = s.name.as_ref();
            let v = edited.get(name).unwrap_or(s.default);
            let changed = before.get(name).is_none_or(|b| b != v);
            if (path && is_shape_row(name)) || keyed.iter().any(|k| k == name) {
                if changed || (path && is_shape_row(name)) {
                    set_key(&mut self.auto, name, at, v, default);
                }
            } else if changed {
                self.set(name, v);
            }
        }
    }

    /// Insert a vertex into segment `seg` (from vertex `seg` to the next) at the curve
    /// parameter `t`, by de Casteljau, so the shape does not change. On an animated path the
    /// new vertex and the shortened neighbour handles are keyed at EVERY key of the path with
    /// the split computed at that key — the shape at the keys stays exact. Returns the new id.
    pub fn insert_vertex(&mut self, seg: usize, t: f64, default: SegShape) -> Option<u32> {
        let MaskShape::Path { verts } = &self.shape else { return None };
        let n = verts.len();
        if n < 2 || seg >= n {
            return None;
        }
        let id = verts.iter().map(|v| v.id).max().map_or(0, |m| m + 1);
        let ticks = self.path_key_ticks();
        // split at every key first (on the evaluated shapes), then the static shape
        let mut at_keys = Vec::new();
        for &k in &ticks {
            let m = self.at(k);
            if let MaskShape::Path { verts } = &m.shape {
                at_keys.push((k, split(verts, seg, t, id)));
            }
        }
        let MaskShape::Path { verts } = &mut self.shape else { return None };
        let (a, nv, b) = split(verts, seg, t, id);
        let ia = seg;
        let ib = (seg + 1) % n;
        let (aid, bid) = (verts[ia].id, verts[ib].id);
        verts[ia] = a;
        verts[ib] = b;
        verts.insert(seg + 1, nv);
        for (k, (a, nv, b)) in at_keys {
            for (vid, p, rows) in [(aid, a, &["ox", "oy"][..]), (bid, b, &["ix", "iy"][..])] {
                for r in rows {
                    let v = vertex_row(&p, r);
                    set_key(&mut self.auto, &format!("v{vid}.{r}"), k, v, default);
                }
            }
            for r in VERTEX_ROWS {
                set_key(&mut self.auto, &format!("v{id}.{r}"), k, vertex_row(&nv, r), default);
            }
        }
        Some(id)
    }

    /// Remove a vertex and its curves. A path keeps three vertices: `false` — refused.
    pub fn remove_vertex(&mut self, id: u32) -> bool {
        let MaskShape::Path { verts } = &mut self.shape else { return false };
        if verts.len() <= 3 || !verts.iter().any(|v| v.id == id) {
            return false;
        }
        verts.retain(|v| v.id != id);
        let pre = format!("v{id}.");
        self.auto.retain(|c| !c.param.starts_with(&pre));
        true
    }

    /// The outline as closed cubic segments `[p0, c0, c1, p1]` in REFERENCE PIXELS of a
    /// `w × h` reference frame. The rasterizer and the on-screen overlay draw from it — one
    /// geometry for both.
    pub fn cubics(&self, ref_w: f64, ref_h: f64) -> Vec<[(f64, f64); 4]> {
        shape_cubics(&self.shape, ref_w, ref_h)
    }
}

fn vertex_row(p: &Vertex, r: &str) -> f64 {
    match r {
        "x" => p.x,
        "y" => p.y,
        "ix" => p.ix,
        "iy" => p.iy,
        "ox" => p.ox,
        _ => p.oy,
    }
}

/// `v12.ox` → (12, "ox").
pub fn parse_vertex_row(name: &str) -> Option<(u32, &str)> {
    let rest = name.strip_prefix('v')?;
    let (id, r) = rest.split_once('.')?;
    Some((id.parse().ok()?, r))
}

/// Set (or replace) the key at `at` on the curve of `param`, creating the curve.
pub fn set_key(auto: &mut Vec<ParamCurve>, param: &str, at: Tick, v: f64, default: SegShape) {
    match auto.iter_mut().find(|c| c.param == param) {
        Some(c) => insert_key(&mut c.keys, at, v, default),
        None => auto.push(ParamCurve {
            param: param.to_string(),
            keys: vec![AutoKey::shaped(at, v, default)],
            source: crate::project::CurveSource::Keys,
        }),
    }
}

/// de Casteljau split of segment `seg`. Returns the updated start vertex, the new one, the updated end.
fn split(verts: &[Vertex], seg: usize, t: f64, id: u32) -> (Vertex, Vertex, Vertex) {
    // de Casteljau is affine-invariant: the split in normalized space is exact
    let n = verts.len();
    let (a, b) = (verts[seg], verts[(seg + 1) % n]);
    let p0 = (a.x, a.y);
    let p1 = (a.x + a.ox, a.y + a.oy);
    let p2 = (b.x + b.ix, b.y + b.iy);
    let p3 = (b.x, b.y);
    let l = |p: (f64, f64), q: (f64, f64)| (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t);
    let q0 = l(p0, p1);
    let q1 = l(p1, p2);
    let q2 = l(p2, p3);
    let r0 = l(q0, q1);
    let r1 = l(q1, q2);
    let s = l(r0, r1);
    let na = Vertex { ox: q0.0 - a.x, oy: q0.1 - a.y, ..a };
    let nb = Vertex { ix: q2.0 - b.x, iy: q2.1 - b.y, ..b };
    let nv = Vertex { id, x: s.0, y: s.1, ix: r0.0 - s.0, iy: r0.1 - s.1, ox: r1.0 - s.0, oy: r1.1 - s.1 };
    (na, nv, nb)
}

/// κ — the handle length of a quarter-circle cubic.
const KAPPA: f64 = 0.552_284_749_830_793_4;

fn line(p: (f64, f64), q: (f64, f64)) -> [(f64, f64); 4] {
    let d = ((q.0 - p.0) / 3.0, (q.1 - p.1) / 3.0);
    [p, (p.0 + d.0, p.1 + d.1), (q.0 - d.0, q.1 - d.1), q]
}

/// The cubic outline of a shape in reference px.
pub fn shape_cubics(shape: &MaskShape, rw: f64, rh: f64) -> Vec<[(f64, f64); 4]> {
    match shape {
        MaskShape::Rect { cx, cy, w, h, rot, round } => {
            let (c, hw, hh) = ((cx * rw, cy * rh), (w * rw / 2.0).abs(), (h * rh / 2.0).abs());
            let r = round.clamp(0.0, 1.0) * hw.min(hh);
            let mut segs = Vec::new();
            let k = r * KAPPA;
            // clockwise from the top edge (y down)
            let corners = [(hw, -hh), (hw, hh), (-hw, hh), (-hw, -hh)];
            let dirs = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)];
            for i in 0..4 {
                let (cxr, cyr) = corners[i];
                let din = dirs[i];
                let dout = dirs[(i + 1) % 4];
                let a = (cxr - din.0 * r, cyr - din.1 * r);
                let b = (cxr + dout.0 * r, cyr + dout.1 * r);
                if r > 0.0 {
                    segs.push([a, (a.0 + din.0 * k, a.1 + din.1 * k), (b.0 - dout.0 * k, b.1 - dout.1 * k), b]);
                }
                let nxt = corners[(i + 1) % 4];
                let e = (nxt.0 - dout.0 * r, nxt.1 - dout.1 * r);
                segs.push(line(b, e));
            }
            place(segs, c, *rot)
        }
        MaskShape::Ellipse { cx, cy, w, h, rot } => {
            let (c, a, b) = ((cx * rw, cy * rh), (w * rw / 2.0).abs(), (h * rh / 2.0).abs());
            let (ka, kb) = (a * KAPPA, b * KAPPA);
            let segs = vec![
                [(a, 0.0), (a, kb), (ka, b), (0.0, b)],
                [(0.0, b), (-ka, b), (-a, kb), (-a, 0.0)],
                [(-a, 0.0), (-a, -kb), (-ka, -b), (0.0, -b)],
                [(0.0, -b), (ka, -b), (a, -kb), (a, 0.0)],
            ];
            place(segs, c, *rot)
        }
        MaskShape::Path { verts } => {
            let n = verts.len();
            let px = |x: f64, y: f64| (x * rw, y * rh);
            (0..n)
                .map(|i| {
                    let (a, b) = (verts[i], verts[(i + 1) % n]);
                    [px(a.x, a.y), px(a.x + a.ox, a.y + a.oy), px(b.x + b.ix, b.y + b.iy), px(b.x, b.y)]
                })
                .collect()
        }
    }
}

/// Rotate (degrees, clockwise with y down) around the origin and move to `c`.
fn place(segs: Vec<[(f64, f64); 4]>, c: (f64, f64), rot: f64) -> Vec<[(f64, f64); 4]> {
    let (s, co) = if rot == 0.0 { (0.0, 1.0) } else { (rot.to_radians().fsin(), rot.to_radians().fcos()) };
    segs.into_iter()
        .map(|seg| seg.map(|(x, y)| (c.0 + x * co - y * s, c.1 + x * s + y * co)))
        .collect()
}

/// The property view of a mask: the passport is built once per view.
pub struct MaskParams<'a> {
    pub mask: &'a mut Mask,
    specs: Vec<ParamSpec>,
}

impl HasParams for MaskParams<'_> {
    fn param_specs(&self) -> &[ParamSpec] {
        &self.specs
    }
    fn get_param(&self, name: &str) -> Option<f64> {
        self.mask.get(name)
    }
    fn set_param(&mut self, name: &str, v: f64) -> bool {
        self.mask.set(name, v)
    }
}

/// Why a mask list is not valid.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MaskError {
    #[error("two masks share the id {0}")]
    DuplicateId(MaskId),
    #[error("two masks share the name {0:?}")]
    DuplicateName(String),
    #[error("mask {0}: a path needs at least three vertices")]
    TooFewVertices(MaskId),
    #[error("mask {0}: two vertices share the id {1}")]
    DuplicateVertex(MaskId, u32),
    #[error("mask {0}: a value is not a finite number")]
    NotFinite(MaskId),
    #[error("mask {0}: a curve names a missing row {1:?}")]
    OrphanCurve(MaskId, String),
}

/// Check one list (ids, names, vertices, numbers, curves).
pub fn check_masks(list: &[Mask]) -> Result<(), MaskError> {
    for (i, m) in list.iter().enumerate() {
        if list[..i].iter().any(|o| o.id == m.id) {
            return Err(MaskError::DuplicateId(m.id));
        }
        if list[..i].iter().any(|o| o.name == m.name) {
            return Err(MaskError::DuplicateName(m.name.clone()));
        }
        if let MaskShape::Path { verts } = &m.shape {
            if verts.len() < 3 {
                return Err(MaskError::TooFewVertices(m.id));
            }
            for (j, v) in verts.iter().enumerate() {
                if verts[..j].iter().any(|o| o.id == v.id) {
                    return Err(MaskError::DuplicateVertex(m.id, v.id));
                }
            }
        }
        let specs = m.specs();
        if specs.iter().any(|s| !m.get(&s.name).is_some_and(f64::is_finite)) {
            return Err(MaskError::NotFinite(m.id));
        }
        for c in &m.auto {
            if !specs.iter().any(|s| s.name == c.param) {
                return Err(MaskError::OrphanCurve(m.id, c.param.clone()));
            }
        }
    }
    Ok(())
}

/// The next free id in a list and a name "<stem> N" not taken in it.
pub fn next_mask_id(list: &[Mask]) -> MaskId {
    list.iter().map(|m| m.id).max().map_or(1, |m| m + 1)
}

pub fn next_mask_name(list: &[Mask], stem: &str) -> String {
    (1..)
        .map(|n| format!("{stem} {n}"))
        .find(|n| !list.iter().any(|m| &m.name == n))
        .expect("an unbounded range always has a free name")
}

/// Evaluate a list at a tick (curves applied).
pub fn masks_at(list: &[Mask], at: Tick) -> Vec<Mask> {
    list.iter().map(|m| m.at(at)).collect()
}

/// The value of row `param` of a mask at `at` — reading a curve when there is one.
pub fn mask_value_at(m: &Mask, param: &str, at: Tick) -> Option<f64> {
    match m.auto.iter().find(|c| c.param == param && !c.keys.is_empty()) {
        Some(c) => m.get(param).map(|_| curve_value(&c.keys, at)),
        None => m.get(param),
    }
}
