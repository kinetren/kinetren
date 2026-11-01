//! The broken stream (datamosh-codec-path, design D3): the GOP schedule, the breakages, the
//! decoder's dependency chain, the content keys and the replay through the host's memo.
//!
//! Frame N is the decoder's loop from its ANCHOR (the last self-contained node of its dependency
//! chain) to N — a pure function of the rows and of the source frames the chain reads. The memo
//! only skips work: every entry is exactly what the replay computes from the same inputs.

use std::collections::HashMap;
use std::sync::Arc;

use kt_plugin_api::generator::lowbias32;
use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::stream::{mix, SecondFrom, Side, StreamInput, StreamLayout, StreamMemo};

use crate::codec::{self, Cfg, Clean, Planes};

/// The decoder's intra refresh (design D3): a broken chain meets a forced intact I-frame every
/// `MAX_CHAIN` frames counted from its anchor — a fixed grid, so consecutive frames continue each
/// other and the memo is reused.
pub const MAX_CHAIN: usize = 300;

/// A walk longer than this is cut (only reachable by rows outside their passport).
const MAX_WALK: usize = 4096;

pub const LINES: &[&str] = &["144", "240", "360", "480", "720"];
pub const LINES_PX: [u32; 5] = [144, 240, 360, 480, 720];
pub const BLOCKS: &[&str] = &["16", "8"];
pub const FROM: &[&str] = &["own", "track_above", "track_below"];

/// The rows, read once per call.
#[derive(Debug, Clone, Copy)]
pub struct Rows {
    pub gop: i64,
    pub cfg: Cfg,
    pub lines: u32,
    pub before: u32,
    pub drop_at: i64,
    pub drop_count: i64,
    pub bloom_at: i64,
    pub bloom_len: i64,
    pub freeze_at: i64,
    pub freeze_len: i64,
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
        let n = |i, d: f64, lo: f64, hi: f64| row(p, i, d).round().clamp(lo, hi);
        let lines = LINES_PX[n(2, 2.0, 0.0, 4.0) as usize];
        Rows {
            gop: n(0, 24.0, 1.0, 60.0) as i64,
            cfg: Cfg {
                qp: n(1, 24.0, 0.0, 51.0),
                bs: if n(3, 0.0, 0.0, 1.0) == 0.0 { 16 } else { 8 },
                search: n(4, 16.0, 1.0, 32.0) as i32,
            },
            lines,
            before: n(5, 0.0, 0.0, 60.0) as u32,
            drop_at: n(6, 0.0, 0.0, 3600.0) as i64,
            drop_count: n(7, 0.0, 0.0, 4.0) as i64,
            bloom_at: n(8, 0.0, 0.0, 3600.0) as i64,
            bloom_len: n(9, 0.0, 0.0, 60.0) as i64,
            freeze_at: n(10, 0.0, 0.0, 3600.0) as i64,
            freeze_len: n(11, 0.0, 0.0, 300.0) as i64,
            scale: row(p, 12, 1.0).clamp(-4.0, 4.0),
            rotate: row(p, 13, 0.0).clamp(-180.0, 180.0),
            swap: row(p, 14, 0.0) >= 0.5,
            second: match n(15, 0.0, 0.0, 2.0) as u32 {
                1 => SecondFrom::TrackAbove,
                2 => SecondFrom::TrackBelow,
                _ => SecondFrom::None,
            },
            gain_q8: (row(p, 16, 1.0).clamp(0.0, 1.0) * 256.0).round() as i32,
            loss: row(p, 17, 0.0).clamp(0.0, 1.0),
            seed: n(18, 0.0, 0.0, 9999.0) as u32,
        }
    }

    /// What the ENCODER's output depends on besides the source addresses.
    fn enc_hash(&self, size: (u32, u32)) -> u64 {
        let mut h = mix(0x00c0_dec0, self.gop as u64);
        h = mix(h, self.cfg.qp.to_bits());
        h = mix(h, self.cfg.bs as u64);
        h = mix(h, self.cfg.search as u64);
        h = mix(h, ((size.0 as u64) << 32) | size.1 as u64);
        mix(h, self.before as u64)
    }

    /// What the DECODER's output depends on besides the packets.
    fn break_hash(&self, enc: u64) -> u64 {
        let mut h = mix(enc, 0x0b0e_a4ed);
        for v in [self.drop_at, self.drop_count, self.bloom_at, self.bloom_len, self.freeze_at, self.freeze_len] {
            h = mix(h, v as u64);
        }
        for v in [self.scale, self.rotate, self.loss] {
            h = mix(h, v.to_bits());
        }
        h = mix(h, self.swap as u64);
        h = mix(h, self.second as u64);
        h = mix(h, self.gain_q8 as u64);
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

/// The schedule of one stream: rows + layout.
pub struct Sched<'a> {
    pub r: Rows,
    pub lay: &'a dyn StreamLayout,
    first: i64,
    cut: i64,
}

impl<'a> Sched<'a> {
    pub fn new(r: Rows, lay: &'a dyn StreamLayout) -> Sched<'a> {
        let cut = lay.cut();
        Sched { r, lay, first: lay.first().min(cut), cut }
    }

    /// An I-frame of the clean encoder: the stream start, the cut, every `gop` from the cut (and
    /// from the start inside the event before).
    pub fn sched_i(&self, k: i64) -> bool {
        let g = self.r.gop;
        k == self.first || k == self.cut || (k > self.cut && (k - self.cut) % g == 0) || (k > self.first && k < self.cut && (k - self.first) % g == 0)
    }

    /// The clean encoder's GOP start of `k` on the main side.
    pub fn gop_start(&self, k: i64) -> i64 {
        let g = self.r.gop;
        if k >= self.cut { self.cut + (k - self.cut) / g * g } else { self.first + (k - self.first).max(0) / g * g }
    }

    fn second_gop_start(&self, k: i64) -> Option<i64> {
        if self.r.second == SecondFrom::None {
            return None;
        }
        let s = self.lay.second_start(k)?;
        Some(s + (k - s).max(0) / self.r.gop * self.r.gop)
    }

    /// The packet the decoder receives at `k`: a bloom repeats the packet at `B` over (B, B+len].
    pub fn pkt_pos(&self, k: i64) -> i64 {
        let b = self.cut + self.r.bloom_at;
        if self.r.bloom_len > 0 && k > b && k <= b + self.r.bloom_len { b } else { k }
    }

    /// The first `drop_count` I-frames at or after `cut + drop_at` are deleted from the stream.
    pub fn dropped(&self, k: i64) -> bool {
        if self.r.drop_count == 0 || !self.sched_i(k) {
            return false;
        }
        let d = self.cut + self.r.drop_at;
        if k < d {
            return false;
        }
        let g = self.r.gop;
        // the scheduled I-frames from the cut on lie on the cut's grid; the cut itself is one
        let first = self.cut + (d - self.cut + g - 1) / g * g;
        k >= first && (k - first) / g < self.r.drop_count
    }

    fn self_contained(&self, k: i64) -> bool {
        let p = self.pkt_pos(k);
        self.sched_i(p) && !self.dropped(p)
    }

    /// The frozen reference of `k`: inside [F, F+len) and before any intact I-frame there, every
    /// P-frame predicts from the picture at F−1.
    fn frozen_ref(&self, k: i64) -> Option<i64> {
        let f = self.cut + self.r.freeze_at;
        if self.r.freeze_len == 0 || k < f || k >= f + self.r.freeze_len {
            return None;
        }
        if (f..=k).any(|j| self.self_contained(j)) {
            return None;
        }
        Some(f - 1)
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

/// The content keys of one call (computed from addresses, never from frames).
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

    /// The clean encoder's key at `p`: its configuration and the addresses from the GOP start.
    fn clean(&mut self, side: Side, p: i64) -> Option<u64> {
        if let Some(&k) = self.clean.get(&(side, p)) {
            return Some(k);
        }
        let gs = self.gop_start(side, p)?;
        let mut h = mix(self.enc, side as u64 + 1);
        for j in gs..=p {
            h = mix(h, self.s.lay.ident(side, j));
            self.clean.insert((side, j), h);
        }
        Some(h)
    }

    fn intra(&self, p: i64) -> u64 {
        mix(mix(self.enc, 0x1a), self.s.lay.ident(Side::Main, p))
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
                    mix(mix(mix(prev, a), b), (k as u64) ^ 4)
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
        Some(i) if i + 1 == chain.len() => return vec![],
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
    let cfg = keys.s.r.cfg;
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
        let c = Arc::new(codec::encode(&src, if q == gs { None } else { prev.as_ref().map(|c| &c.recon) }, &cfg));
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
fn decode_inter(r: &Rows, k: i64, rf: &Planes, own: &Clean, second: Option<&Clean>) -> Planes {
    let bs = r.cfg.bs;
    // an intra packet of the second input carries no motion: its vectors are zero
    let zeros = vec![(0, 0); if second.is_some_and(|c| c.pkt.intra) { own.pkt.mv.len() } else { 0 }];
    let src_mv = match second {
        Some(c) if !c.pkt.intra => &c.pkt.mv,
        Some(_) => &zeros,
        None => &own.pkt.mv,
    };
    let mut mv: Vec<(i16, i16)> = src_mv.iter().map(|&v| transform(r, v)).collect();
    let pred;
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
        pred = codec::predict(rf, &mv, bs);
        return codec::reconstruct(&pred, &res, r.gain_q8);
    }
    pred = codec::predict(rf, &mv, bs);
    codec::reconstruct(&pred, &own.pkt.res, r.gain_q8)
}

/// Frame `n` of the broken stream as planes plus its key; `None` — a frame of the history is
/// missing (everything computed before the gap stays in the memo).
pub fn render(r: Rows, input: &dyn StreamInput, n: i64, memo: &StreamMemo) -> Option<(Arc<Planes>, u64)> {
    let lay: &dyn StreamLayout = input;
    let s = Sched::new(r, lay);
    let chain = s.chain(n);
    let mut keys = Keys::new(&s);
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
                        let x = Arc::new(codec::encode(&src, None, &r.cfg).recon);
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
                Arc::new(decode_inter(&r, k, &rf, &own, second.as_deref()))
            }
        };
        memo.put(dk[i], pic.clone(), pic.bytes());
        cur = Some(pic);
    }
    cur.map(|p| (p, dk[chain.len() - 1]))
}
