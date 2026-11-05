//! The broken stream (datamosh-codec-path, design D3): the GOP schedule, the breakages, the
//! decoder's dependency chain, the content keys and the replay through the host's memo.
//!
//! Frame N is the decoder's loop from its ANCHOR (the last self-contained node of its dependency
//! chain) to N — a pure function of the rows (their curves included, read per position through
//! `StreamLayout::rows_at`) and of the source frames the chain reads. The memo only skips work:
//! every entry is exactly what the replay computes from the same inputs.
//!
//! The breakages are GATES drawn by curves (the owner, 2026-11-01: "reset/inactivity marks by hand
//! on the beat"): `mosh` drops the I-frames while on, a `reset` key rising to on forces an intact
//! I-frame there, `bloom` repeats the motion of its run's first P-frame, `freeze` stops the
//! reference from updating.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use kt_plugin_api::generator::lowbias32;
use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::stream::{mix, SecondFrom, Side, StreamInput, StreamLayout, StreamMemo};

use crate::codec::{self, Cfg, Clean, Planes};

/// The decoder's intra refresh (design D3): a broken chain meets a forced intact I-frame every
/// `MAX_CHAIN` frames counted from its anchor — a fixed grid, so consecutive frames continue each
/// other and the memo is reused. It bounds the cold cost of a frame; the person's own resets
/// (the `reset` curve) normally end a mosh long before it.
pub const MAX_CHAIN: usize = 600;

/// The encoder's longest GOP: `gop` 0 means "keyframes only at the stream start, the cut and the
/// resets", and the encoder still puts one every `KEYINT_MAX` frames, as x264's `keyint` does. While
/// `mosh` is on such a keyframe is dropped like any other; while off it is intact and invisible.
pub const KEYINT_MAX: i64 = 600;

/// A walk longer than this is cut (only reachable by rows outside their passport).
const MAX_WALK: usize = 8192;

pub const LINES: &[&str] = &["144", "240", "360", "480", "720", "1080"];
pub const LINES_PX: [u32; 6] = [144, 240, 360, 480, 720, 1080];
pub const BLOCKS: &[&str] = &["16", "8"];
pub const FROM: &[&str] = &["own", "track_above", "track_below"];

/// The rows, read for one position.
#[derive(Debug, Clone, Copy)]
pub struct Rows {
    /// The GOP length; 0 — only the stream start, the cut, the resets (and `KEYINT_MAX`).
    pub gop: i64,
    pub cfg: Cfg,
    pub lines: u32,
    pub before: u32,
    pub mosh: bool,
    pub reset: bool,
    pub bloom: bool,
    pub freeze: bool,
    pub scale: f64,
    pub rotate: f64,
    pub swap: bool,
    pub second: SecondFrom,
    pub gain_q8: i32,
    pub loss: f64,
    pub seed: u32,
}

fn row(p: &[f64], i: usize, d: f64) -> f64 {
    p.get(i).copied().filter(|v| v.is_finite()).unwrap_or(d)
}

impl Rows {
    pub fn from(p: &[f64]) -> Rows {
        use crate::rows::*;
        let n = |i, d: f64, lo: f64, hi: f64| row(p, i, d).round().clamp(lo, hi);
        let gate = |i| row(p, i, 0.0) >= 0.5;
        Rows {
            gop: n(GOP, 24.0, 0.0, 600.0) as i64,
            cfg: Cfg {
                qp: row(p, QP, 24.0).clamp(0.0, 51.0),
                bs: if n(BLOCK, 0.0, 0.0, 1.0) == 0.0 { 16 } else { 8 },
                search: n(SEARCH, 16.0, 1.0, 64.0) as i32,
            },
            lines: LINES_PX[n(LINES_ROW, 2.0, 0.0, 5.0) as usize],
            before: n(BEFORE, 0.0, 0.0, 300.0) as u32,
            mosh: gate(MOSH),
            reset: gate(RESET),
            bloom: gate(BLOOM),
            freeze: gate(FREEZE),
            scale: row(p, MV_SCALE, 1.0).clamp(-16.0, 16.0),
            rotate: row(p, MV_ROTATE, 0.0).clamp(-180.0, 180.0),
            swap: row(p, MV_SWAP, 0.0) >= 0.5,
            second: match n(VECTORS_FROM, 0.0, 0.0, 2.0) as u32 {
                1 => SecondFrom::TrackAbove,
                2 => SecondFrom::TrackBelow,
                _ => SecondFrom::None,
            },
            gain_q8: (row(p, RESIDUAL, 1.0).clamp(0.0, 1.0) * 256.0).round() as i32,
            loss: row(p, LOSS, 0.0).clamp(0.0, 1.0),
            seed: n(SEED, 0.0, 0.0, 9999.0) as u32,
        }
    }

    /// What the ENCODER's output depends on besides the source addresses and the per-frame
    /// quantiser: the structural rows.
    fn enc_hash(&self, size: (u32, u32)) -> u64 {
        let mut h = mix(0x00c0_dec0, self.gop as u64);
        h = mix(h, self.cfg.bs as u64);
        h = mix(h, self.cfg.search as u64);
        h = mix(h, ((size.0 as u64) << 32) | size.1 as u64);
        mix(h, self.before as u64)
    }

    /// What the DECODER's output depends on besides the packets and the per-frame rows a node
    /// mixes in itself.
    fn break_hash(&self, enc: u64) -> u64 {
        let mut h = mix(enc, 0x0b0e_a4ed);
        h = mix(h, self.swap as u64);
        h = mix(h, self.second as u64);
        mix(h, self.seed as u64)
    }
}

/// What the decoder does at one position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node {
    /// Before the stream: a decoder with no picture holds flat grey.
    Base,
    /// An intra packet (the I-frame at `p`): self-contained.
    Intra { p: i64 },
    /// A forced intact I-frame of the intra refresh, coded from the source at `p`.
    Refresh { p: i64 },
    /// A dropped I-frame: the decoder shows its reference again.
    Hold,
    /// A P-frame: the packet at `p` applied to the reference at `dep`.
    Inter { p: i64, dep: i64 },
}

/// The schedule of one stream: the static rows, the layout and the rows per position.
pub struct Sched<'a> {
    pub r: Rows,
    pub lay: &'a dyn StreamLayout,
    first: i64,
    cut: i64,
    at: RefCell<HashMap<i64, Rows>>,
}

impl<'a> Sched<'a> {
    pub fn new(r: Rows, lay: &'a dyn StreamLayout) -> Sched<'a> {
        let cut = lay.cut();
        Sched { r, lay, first: lay.first().min(cut), cut, at: RefCell::new(HashMap::new()) }
    }

    /// The rows at position `k` (its curves applied).
    pub fn at(&self, k: i64) -> Rows {
        if let Some(r) = self.at.borrow().get(&k) {
            return *r;
        }
        let r = self.lay.rows_at(k.max(self.first)).map_or(self.r, |v| Rows::from(&v));
        self.at.borrow_mut().insert(k, r);
        r
    }

    fn g(&self) -> i64 {
        if self.r.gop == 0 { KEYINT_MAX } else { self.r.gop }
    }

    /// A `reset` key rising to on at `k`: an intact I-frame there.
    pub fn reset_edge(&self, k: i64) -> bool {
        k > self.first && self.at(k).reset && !self.at(k - 1).reset
    }

    /// The GOP grid (counted from the cut, and from the stream start inside the event before). A
    /// reset adds a keyframe; it does not move the grid.
    fn on_grid(&self, k: i64) -> bool {
        let g = self.g();
        (k > self.cut && (k - self.cut) % g == 0) || (k > self.first && k < self.cut && (k - self.first) % g == 0)
    }

    /// An I-frame of the clean encoder: the stream start, the cut, the grid, a reset.
    pub fn sched_i(&self, k: i64) -> bool {
        k == self.first || k == self.cut || self.on_grid(k) || self.reset_edge(k)
    }

    /// The clean encoder's GOP start of `k` on the main side: the latest I-frame at or before it.
    pub fn gop_start(&self, k: i64) -> i64 {
        let g = self.g();
        let grid = if k >= self.cut {
            self.cut + (k - self.cut) / g * g
        } else {
            self.first + (k - self.first).max(0) / g * g
        };
        let mut j = k;
        while j > grid {
            if self.reset_edge(j) {
                return j;
            }
            j -= 1;
        }
        grid
    }

    fn second_gop_start(&self, k: i64) -> Option<i64> {
        if self.r.second == SecondFrom::None {
            return None;
        }
        let s = self.lay.second_start(k)?;
        Some(s + (k - s).max(0) / self.g() * self.g())
    }

    /// The start of the run of a gate that is on at `k`.
    fn run_start(&self, k: i64, gate: impl Fn(&Rows) -> bool) -> i64 {
        let mut j = k;
        let mut n = 0;
        while j > self.first && gate(&self.at(j - 1)) && n < MAX_WALK {
            j -= 1;
            n += 1;
        }
        j
    }

    /// The packet the decoder receives at `k`: while `bloom` is on, the run's first P-frame is
    /// repeated (the frames before it in the run decode their own).
    pub fn pkt_pos(&self, k: i64) -> i64 {
        if !self.at(k).bloom {
            return k;
        }
        let r0 = self.run_start(k, |r| r.bloom);
        (r0..k).find(|&j| !self.sched_i(j)).unwrap_or(k)
    }

    /// While `mosh` is on, every I-frame is deleted from the stream but the stream's first one (a
    /// decoder needs a first picture) and a reset (the person's own keyframe).
    pub fn dropped(&self, k: i64) -> bool {
        self.sched_i(k) && k != self.first && !self.reset_edge(k) && self.at(k).mosh
    }

    fn self_contained(&self, k: i64) -> bool {
        let p = self.pkt_pos(k);
        self.sched_i(p) && !self.dropped(p)
    }

    /// The frozen reference of `k`: while `freeze` is on, every P-frame predicts from the run's
    /// first picture (or from the last self-contained one inside the run).
    fn frozen_ref(&self, k: i64) -> Option<i64> {
        if !self.at(k).freeze {
            return None;
        }
        let r0 = self.run_start(k, |r| r.freeze);
        if r0 == k {
            return None;
        }
        Some(((r0 + 1)..k).rev().find(|&j| self.self_contained(j)).unwrap_or(r0))
    }

    /// The node at `k` before the refresh rule.
    fn node_raw(&self, k: i64) -> Node {
        if k < self.first {
            return Node::Base;
        }
        let p = self.pkt_pos(k);
        if self.sched_i(p) {
            return if self.dropped(p) { Node::Hold } else { Node::Intra { p } };
        }
        Node::Inter { p, dep: self.frozen_ref(k).unwrap_or(k - 1) }
    }

    /// The dependency chain of `n`, anchor first: positions and nodes, the intra refresh applied.
    pub fn chain(&self, n: i64) -> Vec<(i64, Node)> {
        let mut back = vec![];
        let mut k = n;
        loop {
            let node = self.node_raw(k);
            back.push((k, node));
            let dep = match node {
                Node::Base | Node::Intra { .. } | Node::Refresh { .. } => break,
                Node::Hold => k - 1,
                Node::Inter { dep, .. } => dep,
            };
            if back.len() >= MAX_WALK {
                break;
            }
            k = dep;
        }
        back.reverse();
        // the refresh grid: every MAX_CHAIN-th node from the anchor is a forced intact I-frame
        let last = (back.len() - 1) / MAX_CHAIN * MAX_CHAIN;
        if last > 0 {
            back.drain(..last);
            let (k, _) = back[0];
            back[0] = (k, Node::Refresh { p: self.pkt_pos(k) });
        }
        back
    }
}

/// The content keys of one call (computed from addresses and rows, never from frames).
struct Keys<'s, 'a> {
    s: &'s Sched<'a>,
    enc: u64,
    brk: u64,
    clean: HashMap<(Side, i64), u64>,
}

impl<'s, 'a> Keys<'s, 'a> {
    fn new(s: &'s Sched<'a>) -> Keys<'s, 'a> {
        let enc = s.r.enc_hash(s.lay.size());
        Keys { s, enc, brk: s.r.break_hash(enc), clean: HashMap::new() }
    }

    fn gop_start(&self, side: Side, p: i64) -> Option<i64> {
        match side {
            Side::Main => Some(self.s.gop_start(p)),
            Side::Second => self.s.second_gop_start(p),
        }
    }

    /// The clean encoder's key at `p`: its configuration, and the addresses and quantisers from
    /// the GOP start.
    fn clean(&mut self, side: Side, p: i64) -> Option<u64> {
        if let Some(&k) = self.clean.get(&(side, p)) {
            return Some(k);
        }
        let gs = self.gop_start(side, p)?;
        let mut h = mix(mix(self.enc, side as u64 + 1), gs as u64);
        for j in gs..=p {
            h = mix(mix(h, self.s.lay.ident(side, j)), self.s.at(j).cfg.qp.to_bits());
            self.clean.insert((side, j), h);
        }
        Some(h)
    }

    fn intra(&self, p: i64) -> u64 {
        mix(mix(mix(self.enc, 0x1a), self.s.lay.ident(Side::Main, p)), self.s.at(p).cfg.qp.to_bits())
    }

    /// The decoded keys along a chain.
    fn decoded(&mut self, chain: &[(i64, Node)]) -> Vec<u64> {
        let mut out: Vec<u64> = Vec::with_capacity(chain.len());
        for (i, &(k, node)) in chain.iter().enumerate() {
            let prev = if i > 0 { out[i - 1] } else { 0 };
            let key = match node {
                Node::Base => mix(self.brk, 0xba5e),
                Node::Intra { p } => mix(self.brk ^ 1, self.clean(Side::Main, p).unwrap_or(0)),
                Node::Refresh { p } => mix(self.brk ^ 2, self.intra(p)),
                Node::Hold => mix(prev, 3),
                Node::Inter { p, .. } => {
                    let a = self.clean(Side::Main, p).unwrap_or(0);
                    let b = self.clean(Side::Second, p).unwrap_or(0);
                    let r = self.s.at(k);
                    let mut h = mix(mix(mix(prev, a), b), (k as u64) ^ 4);
                    for v in [r.scale, r.rotate, r.loss] {
                        h = mix(h, v.to_bits());
                    }
                    mix(h, r.gain_q8 as u64)
                }
            };
            out.push(key);
        }
        out
    }
}

fn clean_bytes(c: &Clean) -> usize {
    c.recon.bytes() * 3 + c.pkt.mv.len() * 4
}

/// The padded plane size of a layout.
fn padded(lay: &dyn StreamLayout) -> (usize, usize) {
    let (w, h) = lay.size();
    (codec::pad16(w.max(16)), codec::pad16(h.max(16)))
}

/// The positions whose frames are still needed to produce `n` given the memo (design D3, D5).
pub fn needs(r: Rows, lay: &dyn StreamLayout, n: i64, memo: &StreamMemo) -> Vec<(Side, i64)> {
    let s = Sched::new(r, lay);
    let chain = s.chain(n);
    let mut keys = Keys::new(&s);
    let dk = keys.decoded(&chain);
    let start = match (0..chain.len()).rev().find(|&i| memo.contains(dk[i])) {
        Some(i) if i + 1 == chain.len() && lay.region_ident(n) == 0 => return vec![],
        Some(i) => i + 1,
        None => 0,
    };
    let mut out = std::collections::BTreeSet::new();
    let need_clean = |keys: &mut Keys, side: Side, p: i64, out: &mut std::collections::BTreeSet<(Side, i64)>| {
        let Some(gs) = keys.gop_start(side, p) else { return };
        let mut j = p;
        while j >= gs {
            match keys.clean(side, j) {
                Some(k) if memo.contains(k) => break,
                _ => {}
            }
            out.insert((side, j));
            j -= 1;
        }
    };
    for &(_, node) in &chain[start..] {
        match node {
            Node::Base | Node::Hold => {}
            Node::Intra { p } => need_clean(&mut keys, Side::Main, p, &mut out),
            Node::Refresh { p } => {
                if !memo.contains(keys.intra(p)) {
                    out.insert((Side::Main, p));
                }
            }
            Node::Inter { p, .. } => {
                need_clean(&mut keys, Side::Main, p, &mut out);
                need_clean(&mut keys, Side::Second, p, &mut out);
            }
        }
    }
    if lay.region_ident(n) != 0 {
        need_clean(&mut keys, Side::Main, n, &mut out);
    }
    // the earliest position first, both sides interleaved: a host that warms the needs in bounded
    // batches (the export's pre-roll, D6) then always advances the replay
    let mut v: Vec<(Side, i64)> = out.into_iter().collect();
    v.sort_by_key(|&(s, k)| (k, s));
    v
}

/// The source frame at `k` as padded planes; `None` — the host does not have it.
fn source(input: &dyn StreamInput, side: Side, k: i64, w: usize, h: usize) -> Option<Planes> {
    let mut out = None;
    input.with_frame(side, k, &mut |f| out = Some(Planes::from_rgba(f.rgba, f.w, f.h, w, h)));
    out
}

/// The clean encoder's output at `p` (memo first, else encoded forward from the GOP start or the
/// last memoised position).
fn clean_at(keys: &mut Keys, input: &dyn StreamInput, memo: &StreamMemo, side: Side, p: i64) -> Option<Arc<Clean>> {
    let gs = keys.gop_start(side, p)?;
    let (w, h) = padded(keys.s.lay);
    // back to a held entry or the GOP start
    let mut j = p;
    let mut prev: Option<Arc<Clean>> = None;
    loop {
        let k = keys.clean(side, j)?;
        if let Some(c) = memo.get::<Clean>(k) {
            if j == p {
                return Some(c);
            }
            prev = Some(c);
            j += 1;
            break;
        }
        if j == gs {
            break;
        }
        j -= 1;
    }
    let mut last = None;
    for q in j..=p {
        let src = source(input, side, q, w, h)?;
        let cfg = keys.s.at(q).cfg;
        let c = Arc::new(codec::encode(&src, if q == gs { None } else { prev.as_ref().map(|c| &c.recon) }, &cfg)?);
        let k = keys.clean(side, q)?;
        memo.put(k, c.clone(), clean_bytes(&c));
        prev = Some(c.clone());
        last = Some(c);
    }
    last
}

/// Swap, scale, rotate — rounded to integer pels.
fn transform(r: &Rows, v: (i16, i16)) -> (i16, i16) {
    let (mut x, mut y) = (v.0 as f64, v.1 as f64);
    if r.swap {
        std::mem::swap(&mut x, &mut y);
    }
    if r.scale != 1.0 || r.rotate != 0.0 {
        let (s, c) = (r.rotate * std::f64::consts::PI / 180.0).fsin_cos();
        let (nx, ny) = (r.scale * (x * c - y * s), r.scale * (x * s + y * c));
        (x, y) = (nx, ny);
    }
    let rnd = |v: f64| (if v >= 0.0 { (v + 0.5).floor() } else { -((-v + 0.5).floor()) }).clamp(-127.0, 127.0) as i16;
    (rnd(x), rnd(y))
}

/// One P-frame of the broken decoder.
fn decode_inter(r: &Rows, k: i64, rf: &Planes, own: &Clean, second: Option<&Clean>) -> Option<Planes> {
    let bs = r.cfg.bs;
    // an intra packet of the second input carries no motion: its vectors are zero
    let zeros = vec![(0, 0); if second.is_some_and(|c| c.pkt.intra) { own.pkt.mv.len() } else { 0 }];
    let src_mv = match second {
        Some(c) if !c.pkt.intra => &c.pkt.mv,
        Some(_) => &zeros,
        None => &own.pkt.mv,
    };
    let mut mv: Vec<(i16, i16)> = src_mv.iter().map(|&v| transform(r, v)).collect();
    if r.loss > 0.0 {
        let mut res = own.pkt.res.clone();
        let rr = r.cfg.search;
        for (i, v) in mv.iter_mut().enumerate() {
            let h = lowbias32(r.seed ^ lowbias32((k as u32) ^ lowbias32(i as u32 ^ 0x6d6f_7368)));
            if (h as f64) < r.loss * 4_294_967_296.0 {
                let a = lowbias32(h ^ 0x5bd1_e995);
                let span = (2 * rr + 1) as u32;
                *v = (((a % span) as i32 - rr) as i16, (((a >> 16) % span) as i32 - rr) as i16);
                codec::drop_block_residual(&mut res, rf.w, bs, i);
            }
        }
        return codec::decode(rf, &mv, bs, &res, r.gain_q8);
    }
    codec::decode(rf, &mv, bs, &own.pkt.res, r.gain_q8)
}

/// Frame `n` of the broken stream as planes plus its key; `None` — a frame of the history is
/// missing (everything computed before the gap stays in the memo).
pub fn render(r: Rows, input: &dyn StreamInput, n: i64, memo: &StreamMemo) -> Option<(Arc<Planes>, u64)> {
    let lay: &dyn StreamLayout = input;
    let s = Sched::new(r, lay);
    let (pic, key) = broken(&s, input, n, memo)?;
    // the REGION OF INTEREST (the bridge for masks and detectors): the broken stream inside it, the
    // intact one outside — a per-pixel blend by its coverage; the broken loop itself is untouched
    let rid = lay.region_ident(n);
    if rid == 0 {
        return Some((pic, key));
    }
    let mut cov: Option<Vec<u8>> = None;
    if !input.with_region(n, &mut |c| cov = Some(c.to_vec())) {
        return Some((pic, key));
    }
    let cov = cov?;
    let mut keys = Keys::new(&s);
    let clean = clean_at(&mut keys, input, memo, Side::Main, n)?;
    Some((Arc::new(region_blend(&pic, &clean.recon, &cov, lay.size())), mix(key, rid)))
}

/// `clean + (broken − clean) × coverage`, per plane; the coverage is at the stream's size, read
/// with clamped coordinates over the padding, at half resolution for the chroma.
fn region_blend(broken: &Planes, clean: &Planes, cov: &[u8], size: (u32, u32)) -> Planes {
    let (sw, sh) = (size.0.max(1) as usize, size.1.max(1) as usize);
    let at = |x: usize, y: usize| -> i32 { cov.get(y.min(sh - 1) * sw + x.min(sw - 1)).copied().unwrap_or(0) as i32 };
    // floor division (`div_euclid`): a truncating `/` rounds a negative difference the other way,
    // and a fully covered pixel came out one level off the broken picture (t1244)
    let mix_px = |b: u8, c: u8, a: i32| -> u8 { (c as i32 + ((b as i32 - c as i32) * a + 127).div_euclid(255)).clamp(0, 255) as u8 };
    let mut out = clean.clone();
    let w = broken.w;
    for y in 0..broken.h {
        for x in 0..w {
            let i = y * w + x;
            out.y[i] = mix_px(broken.y[i], clean.y[i], at(x, y));
        }
    }
    let cw = w / 2;
    for y in 0..broken.h / 2 {
        for x in 0..cw {
            let i = y * cw + x;
            let a = at(2 * x, 2 * y);
            out.cb[i] = mix_px(broken.cb[i], clean.cb[i], a);
            out.cr[i] = mix_px(broken.cr[i], clean.cr[i], a);
        }
    }
    out
}

/// The broken decoder's picture at `n` and its key.
fn broken(s: &Sched, input: &dyn StreamInput, n: i64, memo: &StreamMemo) -> Option<(Arc<Planes>, u64)> {
    let lay: &dyn StreamLayout = input;
    let r = s.r;
    let chain = s.chain(n);
    let mut keys = Keys::new(s);
    let dk = keys.decoded(&chain);
    let (w, h) = padded(lay);
    let mut start = 0;
    let mut cur: Option<Arc<Planes>> = None;
    for i in (0..chain.len()).rev() {
        if let Some(p) = memo.get::<Planes>(dk[i]) {
            if i + 1 == chain.len() {
                return Some((p, dk[i]));
            }
            cur = Some(p);
            start = i + 1;
            break;
        }
    }
    for i in start..chain.len() {
        let (k, node) = chain[i];
        let pic: Arc<Planes> = match node {
            Node::Base => Arc::new(Planes::grey(w, h)),
            Node::Hold => cur.clone().unwrap_or_else(|| Arc::new(Planes::grey(w, h))),
            Node::Intra { p } => Arc::new(clean_at(&mut keys, input, memo, Side::Main, p)?.recon.clone()),
            Node::Refresh { p } => {
                let ik = keys.intra(p);
                match memo.get::<Planes>(ik) {
                    Some(x) => x,
                    None => {
                        let src = source(input, Side::Main, p, w, h)?;
                        let x = Arc::new(codec::encode(&src, None, &s.at(p).cfg)?.recon);
                        memo.put(ik, x.clone(), x.bytes());
                        x
                    }
                }
            }
            Node::Inter { p, .. } => {
                let own = clean_at(&mut keys, input, memo, Side::Main, p)?;
                let second = if r.second != SecondFrom::None && lay.second_start(p).is_some() {
                    Some(clean_at(&mut keys, input, memo, Side::Second, p)?)
                } else {
                    None
                };
                let rf = cur.clone().unwrap_or_else(|| Arc::new(Planes::grey(w, h)));
                Arc::new(decode_inter(&s.at(k), k, &rf, &own, second.as_deref())?)
            }
        };
        memo.put(dk[i], pic.clone(), pic.bytes());
        cur = Some(pic);
    }
    cur.map(|p| (p, dk[chain.len() - 1]))
}
