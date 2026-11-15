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
//!
//! The walk's own state never crosses the bus (stream-round-trips D1, found-defects 401, 621): the
//! decoder's picture and the clean encoder's state at a batch's end stay on the device as held
//! buffers the next batch reads, and at the walk's end they go into the memo's LIVE tier, where the
//! next walk continues from them. What comes back is only what the host shows and the checkpoints.

use std::collections::HashMap;
use std::sync::Arc;

use kt_plugin_api::compute::Held;
use kt_plugin_api::stream::{OnDevice, SecondFrom, Side, StreamInput, StreamLayout, StreamMemo};

use crate::codec::{self, Batch, Clean, Dev3, DevClean, Got, Kept, Loss, Planes};
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

/// A picture kept on the compute device (stream-round-trips D1): its three planes.
pub struct HeldPic(pub [Held; 3]);

impl HeldPic {
    pub fn bytes(&self) -> usize {
        self.0.iter().map(|h| h.len as usize).sum()
    }
}

/// A clean encoder's output kept on the device: its reconstruction and, when a later position
/// repeats it, its packet (vectors — `None` for intra — and residual).
pub struct HeldClean {
    pub recon: Arc<HeldPic>,
    pub pkt: Option<(Option<Held>, [Held; 3])>,
}

impl HeldClean {
    pub fn bytes(&self) -> usize {
        self.recon.bytes() + self.pkt.as_ref().map_or(0, |(mv, res)| mv.as_ref().map_or(0, |m| m.len as usize) + res.iter().map(|h| h.len as usize).sum::<usize>())
    }
}

/// A picture inside a walk.
#[derive(Clone)]
enum Val {
    Host(Arc<Planes>),
    /// The reconstruction of a clean output the memo holds.
    Recon(Arc<Clean>),
    Dev(Dev3),
    /// On the device from an earlier batch or walk.
    Held(Arc<HeldPic>),
}

/// A clean encoder's output inside a walk.
#[derive(Clone)]
enum CleanVal {
    Host(Arc<Clean>),
    Dev(DevClean),
    /// The reconstruction alone (a batch brought back only what the next encoder reads): a reference,
    /// never a packet.
    Recon(Arc<Planes>),
    /// On the device from an earlier batch or walk; whole when it keeps its packet.
    Held(Arc<HeldClean>),
}

impl CleanVal {
    fn recon(&self) -> Val {
        match self {
            CleanVal::Host(c) => Val::Recon(c.clone()),
            CleanVal::Dev(d) => Val::Dev(d.recon),
            CleanVal::Recon(p) => Val::Host(p.clone()),
            CleanVal::Held(c) => Val::Held(c.recon.clone()),
        }
    }

    fn intra(&self) -> bool {
        match self {
            CleanVal::Host(c) => c.pkt.intra,
            CleanVal::Dev(d) => d.mv.is_none(),
            CleanVal::Recon(_) => false,
            CleanVal::Held(c) => c.pkt.as_ref().is_some_and(|p| p.0.is_none()),
        }
    }

    /// It carries its packet.
    fn whole(&self) -> bool {
        match self {
            CleanVal::Recon(_) => false,
            CleanVal::Held(c) => c.pkt.is_some(),
            _ => true,
        }
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

/// A picture of the batch, after it ran: on the device or on the host.
fn back_pic(b: &Batch, got: &Got, d: Dev3) -> Val {
    let ks = d.map(|id| b.kept(got, id));
    if ks.iter().all(|k| matches!(k, Kept::Held(_))) {
        let [Kept::Held(y), Kept::Held(cb), Kept::Held(cr)] = ks else { unreachable!() };
        return Val::Held(Arc::new(HeldPic([y, cb, cr])));
    }
    let bytes = |k: Kept| match k {
        Kept::Bytes(v) => v,
        Kept::Held(_) => Vec::new(),
    };
    let [y, cb, cr] = ks;
    let (w, h) = (b.width(), b.height());
    let take = |mut v: Vec<u8>, n: usize| {
        v.truncate(n);
        v
    };
    Val::Host(Arc::new(Planes { w, h, y: take(bytes(y), w * h), cb: take(bytes(cb), w * h / 4), cr: take(bytes(cr), w * h / 4) }))
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
    /// Pictures the walk computed that go into the live tier at the flush (a refresh's I-frame, D3).
    live: Vec<(u64, Dev3)>,
    /// The last position computed.
    at: Option<i64>,
    /// Host and held pictures in the current batch, by their address.
    uploaded: HashMap<usize, Dev3>,
    /// Held packets in the current batch, by their address.
    packets: HashMap<usize, (Option<usize>, Dev3)>,
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
        let last = i + 1 == chain.len();
        // the state a walk left on the device (D1): a picture the walk goes on from is read where it
        // lies, never uploaded from the memo's copy
        let held = |memo: &StreamMemo| memo.get_live::<HeldPic>(chain[i].2);
        if !last && let Some(h) = held(memo) {
            cur = Some(Val::Held(h));
            start = i + 1;
            break;
        }
        if let Some(p) = memo.get::<Planes>(chain[i].2) {
            if last {
                return Some((p, chain[i].2));
            }
            cur = Some(Val::Host(p));
            start = i + 1;
            break;
        }
        if last && let Some(h) = held(memo) {
            cur = Some(Val::Held(h));
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
        first: chain[start.min(chain.len() - 1)].0,
        cur,
        last: [None, None],
        bloom: [None, None],
        back: Vec::new(),
        live: Vec::new(),
        at: None,
        uploaded: HashMap::new(),
        packets: HashMap::new(),
        tables: HashMap::new(),
        grey: Arc::new(Planes::grey(w, h)),
        out: None,
        out_dev: None,
        region: lay.region_ident(n) != 0,
        checkpoints: 0,
    };
    let key = chain[chain.len() - 1].2;
    if start == chain.len() {
        // the frame asked is only on the device: it comes back alone, and to the memo as shown
        let pic = walk.cur.clone().expect("found above");
        walk.keep(key, &pic, Put::Lru);
        let d = walk.dev(&pic);
        walk.out_dev = Some(d);
        walk.flush(false)?;
        return walk.out.map(|p| (p, key));
    }
    for &(k, node, dk) in &chain[start..] {
        // before each position: the host may have left the frame (gpu-arbiter, row 147); every
        // position computed so far comes back into the memo
        if input.stop() {
            walk.flush(true);
            return None;
        }
        if walk.positions == 0 {
            walk.first = k;
        }
        match walk.position(k, node, dk) {
            Ok(()) => {}
            Err(Short::Missing) => {
                walk.flush(true);
                return None;
            }
        }
        walk.positions += 1;
        walk.at = Some(k);
        // the frame on screen comes back with its own history, not after the frames ahead of it
        if walk.positions >= MAX_BATCH || walk.b.bytes >= BATCH_BYTES || (walk.view == Some(k) && k != n) {
            walk.flush(false)?;
        }
    }
    // a region of interest blends with the clean encoder's picture at `n` (D4, found-defects 405): at a
    // held position the walk did not encode it, so it is encoded in this batch, not by a job of its own
    if walk.region && !walk.last[Side::Main as usize].as_ref().is_some_and(|(q, _)| *q == n) && s.clean(lay, Side::Main, n).is_some_and(|k| memo.get::<Clean>(k).is_none()) {
        match walk.clean(Side::Main, n) {
            Ok(_) => {}
            Err(Short::Missing) => {
                walk.flush(true);
                return None;
            }
        }
    }
    walk.flush(true)?;
    walk.out.map(|p| (p, key))
}

impl Walk<'_> {
    fn lay(&self) -> &dyn StreamLayout {
        self.input
    }

    /// A picture's planes in the current batch (a host one uploaded once a batch, a held one copied on
    /// the device once a batch).
    fn dev(&mut self, v: &Val) -> Dev3 {
        match v {
            Val::Dev(d) => *d,
            Val::Held(h) => {
                let at = Arc::as_ptr(h) as usize;
                if let Some(d) = self.uploaded.get(&at) {
                    return *d;
                }
                let d = h.0.clone().map(|p| self.b.held(p));
                self.uploaded.insert(at, d);
                d
            }
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
            CleanVal::Held(h) => {
                let at = Arc::as_ptr(h) as usize;
                if let Some(p) = self.packets.get(&at) {
                    return *p;
                }
                let (mv, res) = h.pkt.clone().expect("a reconstruction alone is never asked for its packet");
                let p = (mv.map(|m| self.b.held(m)), res.map(|r| self.b.held(r)));
                self.packets.insert(at, p);
                p
            }
            CleanVal::Recon(_) => unreachable!("a reconstruction alone is never asked for its packet"),
        }
    }

    /// The source frame at `q` on a side, converted on the device.
    fn source(&mut self, side: Side, q: i64) -> Result<Dev3, Short> {
        let (w, h) = (self.w, self.h);
        // a frame the slots in front made on this device is read where it lies
        // (stream-front-on-device): no readback, no upload
        match self.input.on_device(side, q) {
            OnDevice::Held { w: sw, h: sh, held } if sw > 0 && sh > 0 && held.len >= sw as u64 * sh as u64 * 4 => {
                return Ok(self.b.convert_held(held, sw, sh));
            }
            OnDevice::Missing => return Err(Short::Missing),
            _ => {}
        }
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
        if let Some(c) = self.memo.get_live::<HeldClean>(k) {
            return Some(Val::Held(c.recon.clone()));
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
        if let Some(c) = self.memo.get_live::<HeldClean>(key)
            && c.pkt.is_some()
        {
            return Ok(CleanVal::Held(c));
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
                match (self.memo.get::<Planes>(ik), self.memo.get_live::<HeldPic>(ik)) {
                    (Some(x), _) => Val::Host(x),
                    (None, Some(h)) => Val::Held(h),
                    (None, None) => {
                        let cur = self.source(Side::Main, p)?;
                        let d = self.b.encode(cur, None, &s.at(p).cfg).recon;
                        // kept beside the walk (found-defects 402): a re-walk across the refresh reads it
                        self.live.push((ik, d));
                        Val::Dev(d)
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
                Val::Held(_) => self.out_dev = Some(self.dev(&pic)),
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
            Val::Dev(_) | Val::Held(_) => {
                let d = self.dev(v);
                self.b.read3(d);
                self.back.push(Back::Pic { key, d, put });
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
        for (_, d) in &self.live {
            live.extend_from_slice(d);
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

    /// Submit the batch, wait once, and put what came back into the memo. The walk's state goes on
    /// from the device (D1): the decoder's picture and the clean encoder's state stay there as held
    /// buffers, and at the walk's `end` they go into the memo's live tier. A host that keeps nothing
    /// reads them back and the memo keeps them as transient entries, as before. `None` — the device
    /// failed.
    fn flush(&mut self, end: bool) -> Option<()> {
        let s = self.s;
        let on_device = self.b.keeps_on_device();
        if let Some(Val::Dev(d)) = self.cur {
            self.b.keep3(d);
        }
        for side in [Side::Main, Side::Second] {
            let i = side as usize;
            for (slot, bloom) in [(self.last[i].clone(), false), (self.bloom[i].clone(), true)] {
                let Some((q, CleanVal::Dev(c))) = slot else { continue };
                if !bloom && self.region && q == self.n && side == Side::Main {
                    // the region's blend reads the clean picture on the host (`mosh::render`)
                    let Some(key) = s.clean(self.lay(), side, q) else { continue };
                    self.b.read3(c.recon);
                    self.b.read3(c.res);
                    if let Some(m) = c.mv {
                        self.b.read(m);
                    }
                    self.back.push(Back::Clean { key, c, bs: s.at(q).cfg.bs });
                    continue;
                }
                self.b.keep3(c.recon);
                // the packet only where a later position repeats it (a `bloom` run)
                if bloom || self.repeated(q) {
                    self.b.keep3(c.res);
                    if let Some(m) = c.mv {
                        self.b.keep(m);
                    }
                }
            }
        }
        for (_, d) in &self.live {
            self.b.keep3(*d);
        }
        if let Some(d) = self.out_dev {
            self.b.read3(d);
        }
        if !self.b.is_empty() || !self.back.is_empty() {
            self.run_batch()?;
        }
        if end && on_device {
            // what the next walk continues from: the decoder's picture at the last position and the
            // clean encoder's state per side
            if let (Some(Val::Held(h)), Some(k)) = (&self.cur, self.at) {
                self.memo.put_live(s.dk(k), h.clone(), h.bytes());
            }
            for side in [Side::Main, Side::Second] {
                let i = side as usize;
                for slot in [&self.last[i], &self.bloom[i]] {
                    if let Some((q, CleanVal::Held(c))) = slot
                        && let Some(key) = s.clean(self.lay(), side, *q)
                    {
                        self.memo.put_live(key, c.clone(), c.bytes());
                    }
                }
            }
        }
        Some(())
    }

    /// Run the batch and turn its values into what the walk goes on from.
    fn run_batch(&mut self) -> Option<()> {
        let s = self.s;
        let got = self.b.run()?;
        let b = std::mem::replace(&mut self.b, Batch::new(self.w, self.h));
        let mut pics: HashMap<usize, Arc<Planes>> = HashMap::new();
        let mut pic = |d: Dev3| -> Arc<Planes> { pics.entry(d[0]).or_insert_with(|| Arc::new(b.planes_of(&got.reads, b.read_index3(d)))).clone() };
        let mut reads = 0;
        let mut cleans: HashMap<usize, Arc<Clean>> = HashMap::new();
        for back in std::mem::take(&mut self.back) {
            match back {
                Back::Pic { key, d, put } => {
                    reads += 1;
                    self.put(key, pic(d), put);
                }
                Back::Clean { key, c, bs } => {
                    let clean = Arc::new(b.clean_of(&got.reads, c.mv.map(|m| b.read_index(m)), b.read_index3(c.res), b.read_index3(c.recon), bs));
                    self.memo.put_transient(key, clean.clone(), mosh::clean_bytes(&clean));
                    cleans.insert(c.recon[0], clean);
                }
            }
        }
        let mut kept: HashMap<usize, Val> = HashMap::new();
        let mut back = |d: Dev3| -> Val { kept.entry(d[0]).or_insert_with(|| back_pic(&b, &got, d)).clone() };
        if let Some(Val::Dev(d)) = self.cur {
            let v = back(d);
            // a host that keeps nothing: the resume point goes into the memo, as before D1
            if let (Val::Host(p), Some(k)) = (&v, self.at) {
                self.put(s.dk(k), p.clone(), Put::Transient);
            }
            self.cur = Some(v);
        }
        if let Some(d) = self.out_dev.take() {
            self.out = Some(pic(d));
        }
        for side in [Side::Main, Side::Second] {
            let i = side as usize;
            for slot in [&mut self.last[i], &mut self.bloom[i]] {
                let Some((q, CleanVal::Dev(c))) = slot.take() else { continue };
                let key = s.clean(self.input as &dyn StreamLayout, side, q);
                *slot = Some((q, match (cleans.get(&c.recon[0]), back(c.recon)) {
                    (Some(full), _) => CleanVal::Host(full.clone()),
                    (None, Val::Held(recon)) => {
                        let pkt = b.is_kept(c.res[0]).then(|| {
                            let held = |id: usize| match b.kept(&got, id) {
                                Kept::Held(h) => h,
                                Kept::Bytes(_) => unreachable!("a batch that keeps keeps every buffer"),
                            };
                            (c.mv.map(held), c.res.map(held))
                        });
                        CleanVal::Held(Arc::new(HeldClean { recon, pkt }))
                    }
                    (None, Val::Host(_)) if b.is_kept(c.res[0]) => {
                        let full = Arc::new(b.clean_of(&got.reads, c.mv.map(|m| b.read_index(m)), b.read_index3(c.res), b.read_index3(c.recon), s.at(q).cfg.bs));
                        if let Some(key) = key {
                            self.memo.put_transient(key, full.clone(), mosh::clean_bytes(&full));
                        }
                        CleanVal::Host(full)
                    }
                    (None, Val::Host(recon)) => {
                        if let Some(key) = key {
                            self.memo.put_transient(mosh::recon_key(key), recon.clone(), recon.bytes());
                        }
                        CleanVal::Recon(recon)
                    }
                    (None, _) => unreachable!("a kept picture is held or on the host"),
                }));
            }
        }
        // a refresh's I-frame beside the walk (found-defects 402)
        for (key, d) in std::mem::take(&mut self.live) {
            match back(d) {
                Val::Held(h) => self.memo.put_live(key, h.clone(), h.bytes()),
                Val::Host(p) => self.memo.put(key, p.clone(), p.bytes()),
                _ => {}
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
