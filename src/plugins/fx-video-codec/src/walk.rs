//! The history walk on the device (stream-history-on-device D1–D5, found-defects 391, 394).
//!
//! `mosh::broken` walked a frame's dependency chain position by position, and each position was two
//! compute jobs — a submit, a blocking wait and a readback each: 46 ms of round trip for 0.2 ms of
//! device work in the owner's trace. This walk follows the SAME algorithm — the chain from the
//! latest picture the memo holds, the clean encoder forward from its latest state or its GOP start, the
//! decoder's node at each position — but records the codec's operations into a [`Batch`] whose values
//! stay on the device between positions, and submits once per batch. What comes back: the frame asked,
//! the positions at or after the host's view, every `CHECKPOINT`-th position (pinned), and the state a
//! later walk continues from. The bytes are the per-position walk's (t2350): every step is the same
//! shader on the same inputs, and the vector edit is the CPU's, tabled (D3).

use std::collections::HashMap;
use std::sync::Arc;

use kt_plugin_api::stream::{SecondFrom, Side, StreamInput, StreamLayout, StreamMemo};

use crate::codec::{self, Batch, Clean, Dev3, DevClean, Loss, Planes};
use crate::mosh::{self, Node, Plan, Rows};

/// A walk keeps the decoder's reference and the clean encoder's state at every position that is a
/// multiple of this (D5): a re-walk after an eviction or a jump computes at most this many positions.
pub const CHECKPOINT: i64 = 16;

/// A batch closes at this many positions…
pub const MAX_BATCH: usize = 32;
/// …or when its uploads and buffers reach this many bytes (the source frames dominate: 3.7 MB a
/// position at 720 lines).
pub const BATCH_BYTES: usize = 192 << 20;

/// The checkpoints a walk from `anchor` to `n` keeps: the multiples of `CHECKPOINT` in `[anchor, n)`.
pub fn checkpoints(anchor: i64, n: i64) -> usize {
    if n <= anchor {
        return 0;
    }
    (n - 1).div_euclid(CHECKPOINT).saturating_sub((anchor - 1).div_euclid(CHECKPOINT)).max(0) as usize
}

/// The checkpoint's rank in the pinned tier: how many times its index divides by two, so the tier
/// thins to every 32nd, 64th… position instead of opening a hole.
pub fn rank(k: i64) -> u32 {
    let i = k.div_euclid(CHECKPOINT);
    if i == 0 { 31 } else { (i.unsigned_abs()).trailing_zeros().min(30) }
}

/// A picture inside a walk.
#[derive(Clone)]
enum Val {
    Host(Arc<Planes>),
    /// The reconstruction of a clean output the memo holds.
    Recon(Arc<Clean>),
    Dev(Dev3),
}

/// A clean encoder's output inside a walk.
#[derive(Clone)]
enum CleanVal {
    Host(Arc<Clean>),
    Dev(DevClean),
    /// The reconstruction alone (a batch brought back only what the next encoder reads): a reference,
    /// never a packet.
    Recon(Arc<Planes>),
}

impl CleanVal {
    fn recon(&self) -> Val {
        match self {
            CleanVal::Host(c) => Val::Recon(c.clone()),
            CleanVal::Dev(d) => Val::Dev(d.recon),
            CleanVal::Recon(p) => Val::Host(p.clone()),
        }
    }

    fn intra(&self) -> bool {
        match self {
            CleanVal::Host(c) => c.pkt.intra,
            CleanVal::Dev(d) => d.mv.is_none(),
            CleanVal::Recon(_) => false,
        }
    }

    /// It carries its packet.
    fn whole(&self) -> bool {
        !matches!(self, CleanVal::Recon(_))
    }
}

/// How a picture that comes back goes into the memo.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Put {
    Lru,
    /// The resume point of a later walk: evicted before the pictures.
    Transient,
    Pinned(u32),
}

/// What a batch brings back.
enum Back {
    Pic { key: u64, d: Dev3, put: Put },
    Clean { key: u64, c: DevClean, bs: usize },
}

struct Walk<'a> {
    s: &'a Plan,
    input: &'a dyn StreamInput,
    memo: &'a StreamMemo,
    n: i64,
    view: Option<i64>,
    w: usize,
    h: usize,
    b: Batch,
    /// Positions recorded into the current batch, and its first.
    positions: usize,
    first: i64,
    /// The decoder's picture at the last position.
    cur: Option<Val>,
    /// The clean encoder's latest output per side, and its position.
    last: [Option<(i64, CleanVal)>; 2],
    /// A packet later positions repeat (`bloom`), per side.
    bloom: [Option<(i64, CleanVal)>; 2],
    back: Vec<Back>,
    /// Host pictures uploaded into the current batch, by their address.
    uploaded: HashMap<usize, Dev3>,
    tables: HashMap<(u64, u64, bool), usize>,
    grey: Arc<Planes>,
    /// The picture at `n`, once it is on the host.
    out: Option<Arc<Planes>>,
    out_dev: Option<Dev3>,
    region: bool,
    checkpoints: u32,
}

/// A source frame's RGBA and size, copied out of the host's cache.
type Rgba = (Vec<u8>, u32, u32);

/// A walk ended short: a source frame is missing, or the host stopped it. What was computed comes
/// back into the memo; a device failure ends it at the flush.
enum Short {
    Missing,
}

/// The broken decoder's picture at `n` and its key, the walk in batches on the device.
pub fn broken(s: &Plan, input: &dyn StreamInput, n: i64, memo: &StreamMemo) -> Option<(Arc<Planes>, u64)> {
    let lay: &dyn StreamLayout = input;
    let chain = s.chain(n);
    let (w, h) = mosh::padded(lay);
    let mut start = 0;
    let mut cur: Option<Val> = None;
    for i in (0..chain.len()).rev() {
        if let Some(p) = memo.get::<Planes>(chain[i].2) {
            if i + 1 == chain.len() {
                return Some((p, chain[i].2));
            }
            cur = Some(Val::Host(p));
            start = i + 1;
            break;
        }
    }
    let mut walk = Walk {
        s,
        input,
        memo,
        n,
        view: lay.shown_from(),
        w,
        h,
        b: Batch::new(w, h),
        positions: 0,
        first: chain[start].0,
        cur,
        last: [None, None],
        bloom: [None, None],
        back: Vec::new(),
        uploaded: HashMap::new(),
        tables: HashMap::new(),
        grey: Arc::new(Planes::grey(w, h)),
        out: None,
        out_dev: None,
        region: lay.region_ident(n) != 0,
        checkpoints: 0,
    };
    for &(k, node, dk) in &chain[start..] {
        // before each position: the host may have left the frame (gpu-arbiter, row 147); every
        // position computed so far comes back into the memo
        if input.stop() {
            walk.flush();
            return None;
        }
        if walk.positions == 0 {
            walk.first = k;
        }
        match walk.position(k, node, dk) {
            Ok(()) => {}
            Err(Short::Missing) => {
                walk.flush();
                return None;
            }
        }
        walk.positions += 1;
        if walk.positions >= MAX_BATCH || walk.b.bytes >= BATCH_BYTES {
            walk.flush()?;
        }
    }
    walk.flush()?;
    let key = chain[chain.len() - 1].2;
    walk.out.map(|p| (p, key))
}

impl Walk<'_> {
    fn lay(&self) -> &dyn StreamLayout {
        self.input
    }

    /// A picture's planes in the current batch (a host one uploaded once a batch).
    fn dev(&mut self, v: &Val) -> Dev3 {
        match v {
            Val::Dev(d) => *d,
            Val::Host(p) => {
                let at = Arc::as_ptr(p) as usize;
                if let Some(d) = self.uploaded.get(&at) {
                    return *d;
                }
                let d = self.b.upload_planes(p);
                self.uploaded.insert(at, d);
                d
            }
            Val::Recon(c) => {
                let at = Arc::as_ptr(c) as usize;
                if let Some(d) = self.uploaded.get(&at) {
                    return *d;
                }
                let d = self.b.upload_planes(&c.recon);
                self.uploaded.insert(at, d);
                d
            }
        }
    }

    /// A clean output's packet in the current batch: its vectors (`None` — intra) and residual.
    fn packet(&mut self, c: &CleanVal) -> (Option<usize>, Dev3) {
        match c {
            CleanVal::Dev(d) => (d.mv, d.res),
            CleanVal::Host(h) => self.b.upload_packet(&h.pkt),
            CleanVal::Recon(_) => unreachable!("a reconstruction alone is never asked for its packet"),
        }
    }

    /// The source frame at `q` on a side, converted on the device.
    fn source(&mut self, side: Side, q: i64) -> Result<Dev3, Short> {
        let (w, h) = (self.w, self.h);
        let mut got: Option<Result<Rgba, Planes>> = None;
        self.input.with_frame(side, q, &mut |f| {
            let n = f.w as usize * f.h as usize * 4;
            got = Some(if f.w == 0 || f.h == 0 || f.rgba.len() < n {
                Err(Planes::from_rgba(f.rgba, f.w, f.h, w, h))
            } else {
                Ok((f.rgba[..n].to_vec(), f.w, f.h))
            });
        });
        match got.ok_or(Short::Missing)? {
            Ok((rgba, sw, sh)) => Ok(self.b.convert(rgba, sw, sh)),
            Err(p) => Ok(self.b.upload_planes(&p)),
        }
    }

    /// The clean encoder's reconstruction at `j` on a side, where the walk or the memo holds it.
    fn recon_at(&self, side: Side, j: i64) -> Option<Val> {
        if let Some((q, c)) = &self.last[side as usize]
            && *q == j
        {
            return Some(c.recon());
        }
        let k = self.s.clean(self.lay(), side, j)?;
        if let Some(c) = self.memo.get::<Clean>(k) {
            return Some(Val::Recon(c));
        }
        self.memo.get::<Planes>(mosh::recon_key(k)).map(Val::Host)
    }

    /// The clean encoder's output at `p` on a side: the walk's, the memo's, or encoded forward from
    /// its latest state or its GOP start (`mosh::clean_at`, on the device).
    fn clean(&mut self, side: Side, p: i64) -> Result<CleanVal, Short> {
        let i = side as usize;
        for held in [&self.last[i], &self.bloom[i]] {
            if let Some((q, c)) = held
                && *q == p
                && c.whole()
            {
                return Ok(c.clone());
            }
        }
        let s = self.s;
        let lay = self.input as &dyn StreamLayout;
        let Some(key) = s.clean(lay, side, p) else { return Err(Short::Missing) };
        if let Some(c) = self.memo.get::<Clean>(key) {
            return Ok(CleanVal::Host(c));
        }
        let Some(gs) = s.gop_start(lay, side, p) else { return Err(Short::Missing) };
        let mut start = p;
        let mut prev: Option<Val> = None;
        while start > gs {
            if let Some(v) = self.recon_at(side, start - 1) {
                prev = Some(v);
                break;
            }
            start -= 1;
        }
        let mut out = None;
        for q in start..=p {
            if self.input.stop() {
                return Err(Short::Missing);
            }
            let cfg = s.at(q).cfg;
            let cur = self.source(side, q)?;
            let rf = match (q == gs, &prev) {
                (false, Some(v)) => Some(self.dev(&v.clone())),
                _ => None,
            };
            let c = CleanVal::Dev(self.b.encode(cur, rf, &cfg));
            self.set_last(side, q, c.clone());
            prev = Some(c.recon());
            out = Some(c);
        }
        out.ok_or(Short::Missing)
    }

    /// The clean encoder moved on to `q`; the output it leaves stays only when a later position's
    /// packet repeats it (a `bloom` run's first P-frame).
    fn set_last(&mut self, side: Side, q: i64, c: CleanVal) {
        let i = side as usize;
        if let Some((p, old)) = self.last[i].take() {
            // compiled one past: the schedule says whether the next position repeats this packet
            self.s.ensure(self.lay(), p + 1);
            if self.s.pkt_pos(p + 1) == p {
                self.bloom[i] = Some((p, old));
            }
        }
        self.last[i] = Some((q, c));
    }

    /// The vector table of a position's rows, uploaded once a batch; `None` — the identity.
    fn table(&mut self, r: &Rows) -> Option<usize> {
        if !r.swap && r.scale == 1.0 && r.rotate == 0.0 {
            return None;
        }
        let key = (r.scale.to_bits(), r.rotate.to_bits(), r.swap);
        if let Some(&t) = self.tables.get(&key) {
            return Some(t);
        }
        let rr = *r;
        let t = self.b.upload(codec::vector_table(|v| mosh::transform(&rr, v)));
        self.tables.insert(key, t);
        Some(t)
    }

    fn position(&mut self, k: i64, node: Node, dk: u64) -> Result<(), Short> {
        let s = self.s;
        let pic: Val = match node {
            Node::Base => Val::Host(self.grey.clone()),
            Node::Hold => self.cur.clone().unwrap_or_else(|| Val::Host(self.grey.clone())),
            Node::Intra { p } => self.clean(Side::Main, p)?.recon(),
            Node::Refresh { p } => {
                let ik = s.intra(self.lay(), p);
                match self.memo.get::<Planes>(ik) {
                    Some(x) => Val::Host(x),
                    None => {
                        let cur = self.source(Side::Main, p)?;
                        Val::Dev(self.b.encode(cur, None, &s.at(p).cfg).recon)
                    }
                }
            }
            Node::Inter { p, .. } => {
                let own = self.clean(Side::Main, p)?;
                let second = if s.r.second != SecondFrom::None && self.lay().second_start(p).is_some() {
                    Some(self.clean(Side::Second, p)?)
                } else {
                    None
                };
                let r = s.at(k);
                let bs = r.cfg.bs;
                let rf_val = self.cur.clone().unwrap_or_else(|| Val::Host(self.grey.clone()));
                let rf = self.dev(&rf_val);
                let (own_mv, own_res) = self.packet(&own);
                let src = match &second {
                    Some(c) if !c.intra() => {
                        let c = c.clone();
                        self.packet(&c).0.unwrap_or_else(|| self.b.zero_mv(bs))
                    }
                    Some(_) => self.b.zero_mv(bs),
                    None => own_mv.unwrap_or_else(|| self.b.zero_mv(bs)),
                };
                let table = self.table(&r);
                let loss = Loss::of(r.loss, r.seed, k, r.cfg.search);
                let (mv, res) = if table.is_none() && loss.is_none() {
                    (src, own_res)
                } else {
                    let (mv, lost) = self.b.edit(src, bs, table, loss);
                    (mv, lost.map_or(own_res, |l| self.b.drop_lost(own_res, l, bs)))
                };
                Val::Dev(self.b.decode(rf, mv, res, bs, r.gain_q8))
            }
        };
        // what comes back: the frame asked, what the host will show, the checkpoints (D4, D5)
        let shown = k == self.n || self.view.is_none_or(|v| k >= v);
        let checkpoint = k.rem_euclid(CHECKPOINT) == 0 && k != self.n;
        // a checkpoint is pinned even where it is shown: the LRU may drop what is shown
        let put = if checkpoint {
            Some(Put::Pinned(rank(k)))
        } else if shown {
            Some(Put::Lru)
        } else {
            None
        };
        if let Some(put) = put {
            self.keep(dk, &pic, put);
        }
        if checkpoint {
            // the clean encoder's state at the checkpoint: a re-walk from here encodes nothing before it
            for side in [Side::Main, Side::Second] {
                if let Some((q, c)) = self.last[side as usize].clone()
                    && q == k
                    && let Some(key) = s.clean(self.lay(), side, q)
                {
                    self.keep(mosh::recon_key(key), &c.recon(), Put::Pinned(rank(k)));
                }
            }
        }
        if k == self.n {
            match &pic {
                Val::Dev(d) => self.out_dev = Some(*d),
                Val::Host(p) => self.out = Some(p.clone()),
                Val::Recon(c) => self.out = Some(Arc::new(c.recon.clone())),
            }
        }
        self.cur = Some(pic);
        self.sweep();
        Ok(())
    }

    /// Bring a picture back into the memo under `key` (a host one goes in at once).
    fn keep(&mut self, key: u64, v: &Val, put: Put) {
        if let Put::Pinned(_) = put {
            self.checkpoints += 1;
        }
        match v {
            Val::Dev(d) => {
                self.b.read3(*d);
                self.back.push(Back::Pic { key, d: *d, put });
            }
            Val::Host(p) => self.put(key, p.clone(), put),
            Val::Recon(c) => self.put(key, Arc::new(c.recon.clone()), put),
        }
    }

    fn put(&self, key: u64, p: Arc<Planes>, put: Put) {
        let bytes = p.bytes();
        match put {
            Put::Lru => self.memo.put(key, p, bytes),
            Put::Transient => self.memo.put_transient(key, p, bytes),
            Put::Pinned(r) => self.memo.put_pinned(key, p, bytes, r),
        }
    }

    /// Hand out again every buffer the walk no longer reads (D1).
    fn sweep(&mut self) {
        let mut live: Vec<usize> = Vec::new();
        let mut add = |v: &Val| {
            if let Val::Dev(d) = v {
                live.extend_from_slice(d);
            }
        };
        if let Some(c) = &self.cur {
            add(c);
        }
        for (_, c) in self.last.iter().chain(&self.bloom).flatten() {
            if let CleanVal::Dev(d) = c {
                live.extend_from_slice(&d.recon);
                live.extend_from_slice(&d.res);
                live.extend(d.mv);
            }
        }
        self.b.sweep(&|id| live.contains(&id));
    }

    /// Whether a later position's packet repeats the clean output at `q` (a `bloom` run's first
    /// P-frame): it then comes back whole.
    fn repeated(&self, q: i64) -> bool {
        self.s.ensure(self.lay(), q + 1);
        self.s.pkt_pos(q + 1) == q
    }

    /// Submit the batch, wait once, and put what came back into the memo; the walk's state goes on
    /// from the host. `None` — the device failed.
    fn flush(&mut self) -> Option<()> {
        if self.b.is_empty() && self.back.is_empty() {
            return Some(());
        }
        let s = self.s;
        // the resume point: the decoder's picture and the clean encoder's state at the batch's end
        if let Some(Val::Dev(d)) = self.cur.clone()
            && !self.b.is_read(d[0])
        {
            let k = self.first + self.positions as i64 - 1;
            let key = s.dk(k);
            self.keep(key, &Val::Dev(d), Put::Transient);
        }
        for side in [Side::Main, Side::Second] {
            let i = side as usize;
            let whole_last = self.last[i].as_ref().is_some_and(|(q, _)| (self.region && *q == self.n && side == Side::Main) || self.repeated(*q));
            for (slot, whole) in [(self.last[i].clone(), whole_last), (self.bloom[i].clone(), true)] {
                let Some((q, CleanVal::Dev(c))) = slot else { continue };
                let Some(key) = s.clean(self.lay(), side, q) else { continue };
                if whole {
                    self.b.read3(c.recon);
                    self.b.read3(c.res);
                    if let Some(m) = c.mv {
                        self.b.read(m);
                    }
                    self.back.push(Back::Clean { key, c, bs: s.at(q).cfg.bs });
                } else if !self.b.is_read(c.recon[0]) {
                    self.keep(mosh::recon_key(key), &Val::Dev(c.recon), Put::Transient);
                }
            }
        }
        if let Some(d) = self.out_dev {
            self.b.read3(d);
        }
        let got = self.b.run()?;
        let b = std::mem::replace(&mut self.b, Batch::new(self.w, self.h));
        let mut pics: HashMap<usize, Arc<Planes>> = HashMap::new();
        let mut pic = |d: Dev3| -> Arc<Planes> { pics.entry(d[0]).or_insert_with(|| Arc::new(b.planes_of(&got, b.read_index3(d)))).clone() };
        let mut reads = 0;
        let mut cleans: HashMap<usize, Arc<Clean>> = HashMap::new();
        for back in std::mem::take(&mut self.back) {
            match back {
                Back::Pic { key, d, put } => {
                    reads += 1;
                    self.put(key, pic(d), put);
                }
                Back::Clean { key, c, bs } => {
                    let clean = Arc::new(b.clean_of(&got, c.mv.map(|m| b.read_index(m)), b.read_index3(c.res), b.read_index3(c.recon), bs));
                    self.memo.put_transient(key, clean.clone(), mosh::clean_bytes(&clean));
                    cleans.insert(c.recon[0], clean);
                }
            }
        }
        // the walk goes on from the host: every live value came back
        if let Some(Val::Dev(d)) = self.cur {
            self.cur = Some(Val::Host(pic(d)));
        }
        if let Some(d) = self.out_dev.take() {
            self.out = Some(pic(d));
        }
        for i in 0..2 {
            for slot in [&mut self.last[i], &mut self.bloom[i]] {
                if let Some((q, CleanVal::Dev(c))) = slot.take() {
                    *slot = match cleans.get(&c.recon[0]) {
                        Some(full) => Some((q, CleanVal::Host(full.clone()))),
                        None if b.is_read(c.recon[0]) => Some((q, CleanVal::Recon(pic(c.recon)))),
                        None => None,
                    };
                }
            }
        }
        kt_plugin_api::stream::walked(self.first, self.positions, reads, self.checkpoints);
        self.positions = 0;
        self.checkpoints = 0;
        self.uploaded.clear();
        self.tables.clear();
        Some(())
    }
}
