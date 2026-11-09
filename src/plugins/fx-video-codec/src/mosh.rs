//! The broken stream (datamosh-codec-path, design D3): the GOP schedule, the breakages, the
//! decoder's dependency chain, the content keys and the replay through the host's memo.
//!
//! Frame N is the decoder's loop from its ANCHOR (the last self-contained node of its dependency
//! chain) to N — a pure function of the rows (their curves included, read per position through
//! `StreamLayout::rows_at`) and of the source frames the chain reads. The memo only skips work:
//! every entry is exactly what the replay computes from the same inputs. The schedule itself — the
//! rows per position, the chain, the keys — is compiled once per layout (`Plan`,
//! stream-layout-per-build).
//!
//! The breakages are GATES drawn by curves (the owner, 2026-11-01: "reset/inactivity marks by hand
//! on the beat"): `mosh` drops the I-frames while on, a `reset` key rising to on forces an intact
//! I-frame there, `bloom` repeats the motion of its run's first P-frame, `freeze` stops the
//! reference from updating.

use std::sync::{Arc, Mutex};

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
#[derive(Debug, Clone, Copy, PartialEq)]
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

/// One position of the compiled schedule: the rows read there and everything the decoder's
/// schedule and the content keys derive from them. Each field is a forward recurrence over
/// positions (what a position depends on lies before it), so the schedule is filled once, in order.
#[derive(Debug, Clone, Copy)]
struct Pos {
    r: Rows,
    reset_edge: bool,
    sched_i: bool,
    dropped: bool,
    /// The packet the decoder receives here (a bloom repeats its run's first P-frame).
    pkt: i64,
    /// The decoder's node, the intra refresh applied.
    node: Node,
    /// The position before this one in its dependency chain (`Hold`, `Inter`); itself for an anchor.
    dep: i64,
    /// The index in the RAW chain, counted from its self-contained anchor (the refresh grid).
    idx: u64,
    /// The broken decoder's content key here.
    dk: u64,
    /// The clean encoder's key per side and its GOP start.
    clean: [Option<u64>; 2],
    gs: [Option<i64>; 2],
    /// The starts of the `bloom` and `freeze` runs reaching here (uncapped), the latest reset
    /// edge and the latest self-contained position at or before here, the first P-frame of the
    /// bloom run before here.
    s_bloom: i64,
    s_freeze: i64,
    lre: i64,
    lsc: i64,
    fni: Option<i64>,
}

/// The schedule of one stream COMPILED (stream-layout-per-build): the static rows and, per position
/// from the stream's first, the rows with their curves, the decoder's node, its dependency and the
/// content keys. Filled lazily up to the furthest position asked, under a lock, by the forward
/// recurrences of `fill`; a question afterwards reads stored values. Before it, every call (`needs`
/// per frame of the prefetch window, `held`, `key`, `render`) built a schedule and two maps, walked the
/// chain back to its anchor reading the rows at each node, and hashed the GOP again (found-defects 144).
/// The values are those of that walk (t2270); a host keeps one per stage, build and size
/// (`StreamLayout::plan_cell`), a host without a cell gets one per call.
pub struct Plan {
    pub r: Rows,
    first: i64,
    cut: i64,
    g: i64,
    enc: u64,
    brk: u64,
    pos: Mutex<Vec<Pos>>,
}

impl Plan {
    pub fn new(r: Rows, lay: &dyn StreamLayout) -> Plan {
        let cut = lay.cut();
        let enc = r.enc_hash(lay.size());
        Plan {
            r,
            first: lay.first().min(cut),
            cut,
            g: if r.gop == 0 { KEYINT_MAX } else { r.gop },
            enc,
            brk: r.break_hash(enc),
            pos: Mutex::new(Vec::new()),
        }
    }

    /// The layout's compiled schedule: the host's cell when it gives one (compiled once), else a
    /// schedule for this call.
    pub fn of(r: Rows, lay: &dyn StreamLayout) -> Arc<Plan> {
        match lay.plan_cell().and_then(|c| c.get_or_init(|| Plan::new(r, lay))) {
            Some(p) if p.r == r => p,
            _ => Arc::new(Plan::new(r, lay)),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Pos>> {
        // a position is complete once pushed; a panic elsewhere cannot leave one half-written
        self.pos.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Positions compiled so far.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn first(&self) -> i64 {
        self.first
    }

    /// Compile the schedule up to position `n` (inclusive).
    pub fn ensure(&self, lay: &dyn StreamLayout, n: i64) {
        let mut v = self.lock();
        let have = self.first + v.len() as i64;
        if n < have {
            return;
        }
        kt_plugin_api::stream::compiling(have, n + 1, || {
            v.reserve((n + 1 - have) as usize);
            for k in have..=n {
                let p = self.fill(lay, &v, k);
                v.push(p);
            }
            v.len()
        });
    }

    /// The GOP start of the second input at `k` (its event's start, the GOP grid from there).
    fn second_gs(&self, lay: &dyn StreamLayout, k: i64) -> Option<i64> {
        if self.r.second == SecondFrom::None {
            return None;
        }
        let s = lay.second_start(k)?;
        Some(s + (k - s).max(0) / self.g * self.g)
    }

    fn h0(&self, side: Side, gs: i64) -> u64 {
        mix(mix(self.enc, side as u64 + 1), gs as u64)
    }

    /// Position `k` from the ones before it (`v` holds `first .. k`).
    fn fill(&self, lay: &dyn StreamLayout, v: &[Pos], k: i64) -> Pos {
        let (first, cut, g) = (self.first, self.cut, self.g);
        let at = |j: i64| &v[(j - first) as usize];
        let prev = (k > first).then(|| at(k - 1));
        let r = lay.rows_at(k).map_or(self.r, |x| Rows::from(&x));
        let reset_edge = prev.is_some_and(|p| r.reset && !p.r.reset);
        let on_grid = (k > cut && (k - cut) % g == 0) || (k > first && k < cut && (k - first) % g == 0);
        let sched_i = k == first || k == cut || on_grid || reset_edge;
        let dropped = sched_i && k != first && !reset_edge && r.mosh;
        // the clean encoder's GOP start: the grid, or a later reset (a reset adds a keyframe, the
        // grid does not move)
        let grid = if k >= cut { cut + (k - cut) / g * g } else { first + (k - first).max(0) / g * g };
        let lre = if reset_edge { k } else { prev.map_or(i64::MIN, |p| p.lre) };
        let gs = if lre > grid { lre } else { grid };
        // the runs of the gates reaching here
        let s_bloom = match prev {
            Some(p) if p.r.bloom => p.s_bloom,
            _ => k,
        };
        let s_freeze = match prev {
            Some(p) if p.r.freeze => p.s_freeze,
            _ => k,
        };
        let fni = match prev {
            Some(p) if s_bloom < k => p.fni.or((!p.sched_i).then_some(k - 1)),
            _ => None,
        };
        // the packet received: while `bloom` is on, the run's first P-frame (within MAX_WALK)
        let pkt = if !r.bloom {
            k
        } else {
            let r0 = s_bloom.max(k - MAX_WALK as i64);
            match fni {
                Some(j) if j >= r0 => j,
                Some(_) => (r0..k).find(|&j| !at(j).sched_i).unwrap_or(k),
                None => k,
            }
        };
        let (pi, pd) = if pkt == k { (sched_i, dropped) } else { (at(pkt).sched_i, at(pkt).dropped) };
        let sc = pi && !pd;
        let lsc = if sc { k } else { prev.map_or(i64::MIN, |p| p.lsc) };
        // the frozen reference: the run's first picture, or the last self-contained one inside it
        let frozen = if !r.freeze {
            None
        } else {
            let r0 = s_freeze.max(k - MAX_WALK as i64);
            (r0 != k).then_some(match prev {
                Some(p) if p.lsc > r0 => p.lsc,
                _ => r0,
            })
        };
        let raw = if pi {
            if pd { Node::Hold } else { Node::Intra { p: pkt } }
        } else {
            Node::Inter { p: pkt, dep: frozen.unwrap_or(k - 1) }
        };
        let dep = match raw {
            Node::Hold => k - 1,
            Node::Inter { dep, .. } => dep,
            _ => k,
        };
        let idx = if dep == k { 0 } else { at(dep).idx + 1 };
        // the intra refresh: every MAX_CHAIN-th node of a chain from its anchor is a forced intact
        // I-frame — a grid fixed by the chain, so consecutive frames continue each other
        let node = if idx > 0 && idx % MAX_CHAIN as u64 == 0 { Node::Refresh { p: pkt } } else { raw };
        // the clean encoder's keys: its configuration, the addresses and quantisers from the GOP start
        let rows_of = |j: i64| if j >= k { r } else { at(j.max(first)).r };
        let main = {
            let base = match prev {
                Some(p) if k > gs => p.clean[0].unwrap_or(0),
                _ => self.h0(Side::Main, gs),
            };
            mix(mix(base, lay.ident(Side::Main, k)), r.cfg.qp.to_bits())
        };
        let gs2 = self.second_gs(lay, k);
        let second = gs2.map(|g2| {
            let base = match prev {
                Some(p) if k > g2 && p.gs[1] == Some(g2) => p.clean[1].unwrap_or(0),
                _ => {
                    let mut h = self.h0(Side::Second, g2);
                    for j in g2..k {
                        h = mix(mix(h, lay.ident(Side::Second, j)), rows_of(j).cfg.qp.to_bits());
                    }
                    h
                }
            };
            mix(mix(base, lay.ident(Side::Second, k)), r.cfg.qp.to_bits())
        });
        let clean_of = |side: usize, p: i64| -> Option<u64> {
            if p == k { [Some(main), second][side] } else { at(p).clean[side] }
        };
        let dk = match node {
            Node::Base => mix(self.brk, 0xba5e),
            Node::Intra { p } => mix(self.brk ^ 1, clean_of(0, p).unwrap_or(0)),
            Node::Refresh { p } => mix(self.brk ^ 2, self.intra_key(lay, p, rows_of(p))),
            Node::Hold => mix(at(dep).dk, 3),
            Node::Inter { p, .. } => {
                let mut h = mix(mix(mix(at(dep).dk, clean_of(0, p).unwrap_or(0)), clean_of(1, p).unwrap_or(0)), (k as u64) ^ 4);
                for x in [r.scale, r.rotate, r.loss] {
                    h = mix(h, x.to_bits());
                }
                mix(h, r.gain_q8 as u64)
            }
        };
        Pos {
            r,
            reset_edge,
            sched_i,
            dropped,
            pkt,
            node,
            dep,
            idx,
            dk,
            clean: [Some(main), second],
            gs: [Some(gs), gs2],
            s_bloom,
            s_freeze,
            lre,
            lsc,
            fni,
        }
    }

    fn intra_key(&self, lay: &dyn StreamLayout, p: i64, r: Rows) -> u64 {
        mix(mix(mix(self.enc, 0x1a), lay.ident(Side::Main, p)), r.cfg.qp.to_bits())
    }

    /// The stored position `k` (compiled: `ensure` first); `None` before the stream.
    fn pos(&self, k: i64) -> Option<Pos> {
        if k < self.first {
            return None;
        }
        let p = self.lock().get((k - self.first) as usize).copied();
        debug_assert!(p.is_some(), "position {k} read before it was compiled");
        p
    }

    /// The rows at position `k` (its curves applied; before the stream, the stream's first).
    pub fn at(&self, k: i64) -> Rows {
        self.pos(k.max(self.first)).map_or(self.r, |p| p.r)
    }

    /// An I-frame of the clean encoder: the stream start, the cut, the grid, a reset.
    pub fn sched_i(&self, k: i64) -> bool {
        self.pos(k).is_some_and(|p| p.sched_i)
    }

    /// An I-frame `mosh` deletes from the stream.
    pub fn dropped(&self, k: i64) -> bool {
        self.pos(k).is_some_and(|p| p.dropped)
    }

    /// A `reset` key rising to on at `k`.
    pub fn reset_edge(&self, k: i64) -> bool {
        self.pos(k).is_some_and(|p| p.reset_edge)
    }

    /// The packet the decoder receives at `k`.
    pub fn pkt_pos(&self, k: i64) -> i64 {
        self.pos(k).map_or(k, |p| p.pkt)
    }

    /// The decoder's node at `k`, the intra refresh applied.
    pub fn node(&self, k: i64) -> Node {
        self.pos(k).map_or(Node::Base, |p| p.node)
    }

    /// The broken decoder's key at `k`.
    pub fn dk(&self, k: i64) -> u64 {
        self.pos(k).map_or(mix(self.brk, 0xba5e), |p| p.dk)
    }

    /// The dependency chain of `n`, anchor first: positions and nodes, the intra refresh applied.
    pub fn chain(&self, n: i64) -> Vec<(i64, Node, u64)> {
        let v = self.lock();
        let Some(mut i) = (n >= self.first).then(|| (n - self.first) as usize) else {
            return vec![(n, Node::Base, mix(self.brk, 0xba5e))];
        };
        let mut back = vec![];
        loop {
            let p = &v[i];
            let k = self.first + i as i64;
            back.push((k, p.node, p.dk));
            match p.node {
                Node::Hold | Node::Inter { .. } => i = (p.dep - self.first) as usize,
                _ => break,
            }
        }
        back.reverse();
        back
    }

    /// The clean encoder's GOP start of `p` on a side.
    fn gop_start(&self, lay: &dyn StreamLayout, side: Side, p: i64) -> Option<i64> {
        match (side, self.pos(p)) {
            (Side::Main, Some(x)) => x.gs[0],
            (Side::Second, Some(x)) => x.gs[1],
            (Side::Main, None) => Some(self.first),
            (Side::Second, None) => self.second_gs(lay, p),
        }
    }

    /// The clean encoder's key at `p` on a side (positions before the stream: hashed from its GOP
    /// start, the rows at the stream's first, as before).
    fn clean(&self, lay: &dyn StreamLayout, side: Side, p: i64) -> Option<u64> {
        if let Some(x) = self.pos(p) {
            return x.clean[side as usize];
        }
        let gs = self.gop_start(lay, side, p)?;
        let mut h = self.h0(side, gs);
        for j in gs..=p {
            h = mix(mix(h, lay.ident(side, j)), self.at(j).cfg.qp.to_bits());
        }
        Some(h)
    }

    fn intra(&self, lay: &dyn StreamLayout, p: i64) -> u64 {
        self.intra_key(lay, p, self.at(p))
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

/// The layout's schedule compiled up to `n`.
fn plan(r: Rows, lay: &dyn StreamLayout, n: i64) -> Arc<Plan> {
    let p = Plan::of(r, lay);
    p.ensure(lay, n);
    p
}

/// The positions whose frames are still needed to produce `n` given the memo (design D3, D5).
pub fn needs(r: Rows, lay: &dyn StreamLayout, n: i64, memo: &StreamMemo) -> Vec<(Side, i64)> {
    let s = plan(r, lay, n);
    let chain = s.chain(n);
    let start = match (0..chain.len()).rev().find(|&i| memo.contains(chain[i].2)) {
        Some(i) if i + 1 == chain.len() && lay.region_ident(n) == 0 => return vec![],
        Some(i) => i + 1,
        None => 0,
    };
    let mut out = std::collections::BTreeSet::new();
    let need_clean = |side: Side, p: i64, out: &mut std::collections::BTreeSet<(Side, i64)>| {
        let Some(gs) = s.gop_start(lay, side, p) else { return };
        let mut j = p;
        while j >= gs {
            match s.clean(lay, side, j) {
                Some(k) if memo.contains(k) => break,
                _ => {}
            }
            out.insert((side, j));
            j -= 1;
        }
    };
    for &(_, node, _) in &chain[start..] {
        match node {
            Node::Base | Node::Hold => {}
            Node::Intra { p } => need_clean(Side::Main, p, &mut out),
            Node::Refresh { p } => {
                if !memo.contains(s.intra(lay, p)) {
                    out.insert((Side::Main, p));
                }
            }
            Node::Inter { p, .. } => {
                need_clean(Side::Main, p, &mut out);
                need_clean(Side::Second, p, &mut out);
            }
        }
    }
    if lay.region_ident(n) != 0 {
        need_clean(Side::Main, n, &mut out);
    }
    // the earliest position first, both sides interleaved: a host that warms the needs in bounded
    // batches (the export's pre-roll, D6) then always advances the replay
    let mut v: Vec<(Side, i64)> = out.into_iter().collect();
    v.sort_by_key(|&(s, k)| (k, s));
    v
}

/// The key `render` returns for `n`, from the rows and addresses alone (structured-telemetry).
pub fn key(r: Rows, lay: &dyn StreamLayout, n: i64) -> u64 {
    let key = plan(r, lay, n).dk(n);
    match lay.region_ident(n) {
        0 => key,
        rid => mix(key, rid),
    }
}

/// The latest position of `n`'s chain whose broken picture the memo holds (decode-request-storm).
pub fn held(r: Rows, lay: &dyn StreamLayout, n: i64, memo: &StreamMemo) -> Option<i64> {
    plan(r, lay, n).chain(n).into_iter().rev().find(|c| memo.contains(c.2)).map(|c| c.0)
}

/// The clean encoder's output for the source frame at `k` against `prev` (`None` — intra): the job
/// converts the frame's RGBA to the padded planes itself. `None` — the host does not have the frame,
/// or the device failed.
fn encode_at(input: &dyn StreamInput, side: Side, k: i64, prev: Option<&Planes>, cfg: &Cfg) -> Option<Clean> {
    let (w, h) = padded(input);
    let mut out = None;
    input.with_frame(side, k, &mut |f| out = codec::encode_rgba(f.rgba, f.w, f.h, w, h, prev, cfg));
    out
}

/// The clean encoder's output at `p` (memo first, else encoded forward from the GOP start or the
/// last memoised position).
fn clean_at(s: &Plan, input: &dyn StreamInput, memo: &StreamMemo, side: Side, p: i64) -> Option<Arc<Clean>> {
    let lay: &dyn StreamLayout = input;
    let gs = s.gop_start(lay, side, p)?;
    // back to a held entry or the GOP start
    let mut j = p;
    let mut prev: Option<Arc<Clean>> = None;
    loop {
        let k = s.clean(lay, side, j)?;
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
        let cfg = s.at(q).cfg;
        let c = Arc::new(encode_at(input, side, q, if q == gs { None } else { prev.as_ref().map(|c| &c.recon) }, &cfg)?);
        let k = s.clean(lay, side, q)?;
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


/// Frame `n` of the broken stream as planes plus its key; `None` — a frame of the history is
/// missing (everything computed before the gap stays in the memo).
pub fn render(r: Rows, input: &dyn StreamInput, n: i64, memo: &StreamMemo) -> Option<(Arc<Planes>, u64)> {
    let lay: &dyn StreamLayout = input;
    let s = plan(r, lay, n);
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
    let clean = clean_at(&s, input, memo, Side::Main, n)?;
    Some((Arc::new(region_blend(&pic, &clean.recon, &cov, lay.size())), mix(key, rid)))
}

/// The broken decoder's picture at `n` and its key.
fn broken(s: &Plan, input: &dyn StreamInput, n: i64, memo: &StreamMemo) -> Option<(Arc<Planes>, u64)> {
    let lay: &dyn StreamLayout = input;
    let r = s.r;
    let chain = s.chain(n);
    let (w, h) = padded(lay);
    let mut start = 0;
    let mut cur: Option<Arc<Planes>> = None;
    for i in (0..chain.len()).rev() {
        if let Some(p) = memo.get::<Planes>(chain[i].2) {
            if i + 1 == chain.len() {
                return Some((p, chain[i].2));
            }
            cur = Some(p);
            start = i + 1;
            break;
        }
    }
    for &(k, node, dk) in &chain[start..] {
        let pic: Arc<Planes> = match node {
            Node::Base => Arc::new(Planes::grey(w, h)),
            Node::Hold => cur.clone().unwrap_or_else(|| Arc::new(Planes::grey(w, h))),
            Node::Intra { p } => Arc::new(clean_at(s, input, memo, Side::Main, p)?.recon.clone()),
            Node::Refresh { p } => {
                let ik = s.intra(lay, p);
                match memo.get::<Planes>(ik) {
                    Some(x) => x,
                    None => {
                        let x = Arc::new(encode_at(input, Side::Main, p, None, &s.at(p).cfg)?.recon);
                        memo.put(ik, x.clone(), x.bytes());
                        x
                    }
                }
            }
            Node::Inter { p, .. } => {
                let own = clean_at(s, input, memo, Side::Main, p)?;
                let second = if r.second != SecondFrom::None && lay.second_start(p).is_some() {
                    Some(clean_at(s, input, memo, Side::Second, p)?)
                } else {
                    None
                };
                let rf = cur.clone().unwrap_or_else(|| Arc::new(Planes::grey(w, h)));
                Arc::new(decode_inter(&s.at(k), k, &rf, &own, second.as_deref())?)
            }
        };
        memo.put(dk, pic.clone(), pic.bytes());
        cur = Some(pic);
    }
    cur.map(|p| (p, chain[chain.len() - 1].2))
}
