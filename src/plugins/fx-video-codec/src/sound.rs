//! The sound of the broken stream (sim-audio-pairs; note F-FX-013): the audio half of `datamosh`.
//! The event's sound is coded by the codec of `kt_plugin_api::audio_codec` (an AAC-Main-like MDCT
//! codec with per-line prediction and a bit reservoir) into the same muxed stream as the picture,
//! and the stream is broken by the SAME rows on the SAME schedule — the picture's own `mosh::Sched`
//! over a layout of the audio's event:
//!
//! - every I-frame of the picture's GOP is a random-access point of the stream: the audio encoder
//!   resets its predictors in the audio frame that carries it (a segmented stream aligns the two);
//! - an I-frame `mosh` deletes is deleted with its audio packet: the decoder conceals that frame,
//!   misses the reset, and the residual of the frames after it lands on the old prediction — the
//!   sound before the cut smears into the sound after it until the next intact I-frame; a frame that
//!   borrowed bits from the deleted packet (the reservoir) breaks too;
//! - `bloom` repeats the packet of the run's first P-frame, so the decoder adds the same residual
//!   again and again to its moving prediction;
//! - `freeze` stops the decoder's reference (the predictor's history) from updating;
//! - `residual` scales the residual the decoder applies, as it scales the picture's;
//! - `loss` hits a packet slot with bit errors: the Rice-coded lines after the first flipped bit
//!   decode as garbage, or the decoder's checks catch it and it conceals;
//! - `qp` is the finest quantiser step the encoder may use (6 steps double it, as the picture's).
//!
//! Motion vectors, the block size, the search range and the second input have no audio meaning.
//!
//! Frame grids: the audio frames sit on the absolute timeline (frame `n` = samples `[nN, (n+1)N)`),
//! the picture's positions on the project grid (`k·90000/fps` ticks); the audio frame of position
//! `k` is the one its time falls in.

use std::collections::VecDeque;

use kt_plugin_api::audio_codec::{self as codec, Damage, Decoder, Encoder, Packet, N};
use kt_plugin_api::generator::lowbias32;
use kt_plugin_api::stream::{Side, StreamLayout};
use kt_plugin_api::{AudioState, CurveRead, ParamKind, Placement, SlotCurves};

use crate::mosh::{Rows, Sched};

/// The audio look-ahead: a frame is decoded once its whole window (2N) has arrived.
pub const LA: i64 = 2 * N as i64;
/// Packets kept for a bloom's repeat (≈ 5.5 s).
const KEEP: usize = 256;
/// The finest gain index at `qp` 24; 6 steps of `qp` add 4 (double the step).
const G24: f64 = 49.0;
/// Bits flipped in a slot `loss` hits.
const FLIPS: u32 = 3;

/// The audio bitrates (kbps, the whole stereo stream) and their coded bandwidth (Hz).
pub const KBPS: &[&str] = &["32", "64", "96", "128", "192"];
const KBPS_V: [f64; 5] = [32.0, 64.0, 96.0, 128.0, 192.0];
const BANDWIDTH: [f64; 5] = [7_000.0, 11_000.0, 15_000.0, 16_000.0, 19_000.0];

/// The layout of the audio's event on the picture's grid: the cut, the stream's first position,
/// the rows per position from the slot's curves.
struct Layout<'a> {
    first: i64,
    cut: i64,
    end: i64,
    step: f64,
    start_tick: f64,
    len_tick: f64,
    base: &'a [f64],
    curves: &'a [(usize, Box<dyn CurveRead>)],
}

impl StreamLayout for Layout<'_> {
    fn size(&self) -> (u32, u32) {
        (16, 16)
    }
    fn first(&self) -> i64 {
        self.first
    }
    fn cut(&self) -> i64 {
        self.cut
    }
    fn end(&self) -> i64 {
        self.end
    }
    fn second_start(&self, _k: i64) -> Option<i64> {
        None
    }
    fn ident(&self, _side: Side, _k: i64) -> u64 {
        0
    }
    fn rows_at(&self, k: i64) -> Option<Vec<f64>> {
        if self.curves.is_empty() {
            return None;
        }
        let t = (k as f64 * self.step).max(self.start_tick);
        let s = ((t - self.start_tick) / self.len_tick.max(1.0)).clamp(0.0, 1.0);
        let mut v = self.base.to_vec();
        for (i, c) in self.curves {
            v[*i] = c.value(v[*i], s);
        }
        Some(v)
    }
}

/// One frame's verdict from the schedule.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSched {
    /// The encoder resets its predictors here (an I-frame of the picture falls in the frame).
    pub reset: bool,
    /// An I-frame here is deleted: the audio packet goes with it.
    pub deleted: bool,
    /// The decoder receives the packet of this earlier audio frame instead (a bloom's repeat).
    pub repeat: Option<i64>,
    pub freeze: bool,
    pub residual: f64,
    pub qp: f64,
    pub loss: f64,
}

/// The audio state of `datamosh_sound`.
pub struct MoshSound {
    v: Vec<f64>,
    at: Placement,
    clip_len: i64,
    curves: Vec<(usize, Box<dyn CurveRead>)>,
    enc: [Encoder; 2],
    dec: [Decoder; 2],
    /// The input of the timeline frames not yet coded, from `buf_start`.
    input: [Vec<f64>; 2],
    buf_start: i64,
    /// The decoded output from `out_start`.
    out: [VecDeque<f32>; 2],
    out_start: i64,
    /// The next audio frame to code.
    next_frame: Option<i64>,
    /// Recent packets per channel with their frame index.
    packets: VecDeque<(i64, [Packet; 2])>,
    /// Slots deleted (by frame index) — recent.
    deleted: VecDeque<i64>,
}

impl Clone for MoshSound {
    fn clone(&self) -> MoshSound {
        MoshSound {
            v: self.v.clone(),
            at: self.at,
            clip_len: self.clip_len,
            curves: self.curves.iter().map(|(i, c)| (*i, c.clone_box())).collect(),
            enc: self.enc.clone(),
            dec: self.dec.clone(),
            input: self.input.clone(),
            buf_start: self.buf_start,
            out: self.out.clone(),
            out_start: self.out_start,
            next_frame: self.next_frame,
            packets: self.packets.clone(),
            deleted: self.deleted.clone(),
        }
    }
}

impl MoshSound {
    pub fn new() -> MoshSound {
        let v = crate::defaults();
        let kbps = kbps_of(&v);
        MoshSound {
            v,
            at: Placement::default(),
            clip_len: 1,
            curves: Vec::new(),
            enc: [Encoder::new(kbps / 2.0), Encoder::new(kbps / 2.0)],
            dec: [Decoder::new(0x0A5D), Decoder::new(0x0A5E)],
            input: [Vec::new(), Vec::new()],
            buf_start: 0,
            out: [VecDeque::new(), VecDeque::new()],
            out_start: 0,
            next_frame: None,
            packets: VecDeque::new(),
            deleted: VecDeque::new(),
        }
    }

    /// The frames the decoders concealed so far (both channels' count of the first).
    pub fn concealed(&self) -> u64 {
        self.dec[0].concealed
    }

    fn kbps_idx(&self) -> usize {
        (self.v[crate::rows::AUDIO_KBPS].round().max(0.0) as usize).min(4)
    }

    fn predict(&self) -> bool {
        self.v[crate::rows::AUDIO_PREDICT] >= 0.5
    }

    /// The picture's schedule read for audio frame `n`.
    pub fn sched(&self, n: i64) -> FrameSched {
        let fps = self.at.fps();
        let step = 90_000.0 / fps;
        let start_tick = self.at.start as f64 * 15.0 / 8.0;
        let len_tick = self.clip_len as f64 * 15.0 / 8.0;
        let r = Rows::from(&self.v);
        let cut = (start_tick / step).ceil() as i64;
        let lay = Layout {
            first: cut - r.before as i64,
            cut,
            end: ((start_tick + len_tick) / step).ceil() as i64,
            step,
            start_tick,
            len_tick,
            base: &self.v,
            curves: &self.curves,
        };
        let s = Sched::new(r, &lay);
        let t0 = (n * N as i64) as f64 * 15.0 / 8.0;
        let t1 = ((n + 1) * N as i64) as f64 * 15.0 / 8.0;
        let (k0, k1) = ((t0 / step).ceil() as i64, (t1 / step).ceil() as i64);
        let mut f = FrameSched { residual: 1.0, ..Default::default() };
        let first_frame = (self.at.start).div_euclid(N as i64);
        f.reset = n == first_frame;
        let rk = s.at(k0.max(lay.first));
        f.freeze = rk.freeze;
        f.residual = rk.gain_q8 as f64 / 256.0;
        f.qp = rk.cfg.qp;
        f.loss = rk.loss;
        for k in k0..k1 {
            if k < lay.first {
                continue;
            }
            if s.sched_i(k) {
                // the clean encoder resets at every I-frame; only the decoder can miss one
                f.reset = true;
                f.deleted |= s.dropped(k);
            }
            let p = s.pkt_pos(k);
            if p != k {
                let tp = p as f64 * step;
                f.repeat = Some((tp * 8.0 / 15.0).div_euclid(N as f64) as i64);
            }
        }
        f
    }

    /// Code and decode audio frame `n` from the input buffer (which holds `[(n−1)N, (n+1)N)`).
    fn frame(&mut self, n: i64) {
        let fs = self.sched(n);
        let bw = (BANDWIDTH[self.kbps_idx()] / (24_000.0 / N as f64)) as usize;
        let floor = codec::step_of((G24 + (fs.qp - 24.0) * 4.0 / 6.0).round() as i32);
        let predict = self.predict();
        let off = ((n - 1) * N as i64 - self.buf_start) as usize;
        let pk: [Packet; 2] = std::array::from_fn(|c| {
            let x = &self.input[c][off..off + 2 * N];
            self.enc[c].encode(x, fs.reset, floor, bw, predict)
        });
        self.packets.push_back((n, pk));
        while self.packets.len() > KEEP {
            self.packets.pop_front();
        }
        if fs.deleted {
            self.deleted.push_back(n);
            while self.deleted.len() > 64 {
                self.deleted.pop_front();
            }
        }
        let seed = self.v[crate::rows::SEED] as u32;
        let src = fs.repeat.unwrap_or(n);
        let mut out = [vec![0f64; N], vec![0f64; N]];
        for (c, o) in out.iter_mut().enumerate() {
            let found = self.packets.iter().rev().find(|(m, _)| *m == src).map(|(_, p)| p[c].clone());
            let mut dmg = Damage { residual: fs.residual, freeze: fs.freeze, ..Default::default() };
            let pkt = match found {
                Some(p) => p,
                None => {
                    dmg.lost = true;
                    self.packets.back().map(|(_, p)| p[c].clone()).expect("this frame's packet")
                }
            };
            // the packet slots the frame's main data lives in: deleted → lost; hit by `loss` → bits flipped
            let slot = self.enc[c].slot.max(1);
            let (b0, b1) = (pkt.begin, pkt.begin + pkt.payload.v.len());
            let mut bits = pkt.payload.v.clone();
            for j in (b0 / slot)..=(b1.saturating_sub(1) / slot) {
                let j = j as i64;
                // the stream's packet j is audio frame j of this channel's stream counted from its
                // first: map back to the timeline frame of the same packet
                let frame_j = n - (self.enc[c].frame as i64 - 1 - j);
                if self.deleted.contains(&frame_j) {
                    dmg.lost = true;
                }
                let h = lowbias32(seed ^ (frame_j as u32).wrapping_mul(0x85EB_CA6B) ^ (c as u32 * 0x27D4_EB2F));
                if ((h >> 8) as f64 / 16_777_216.0) < fs.loss {
                    for f in 0..FLIPS {
                        let hp = lowbias32(h ^ (f + 1));
                        let at = j as usize * slot + hp as usize % slot;
                        if at >= b0 && at < b1 {
                            bits[at - b0] = !bits[at - b0];
                        }
                    }
                }
            }
            if fs.deleted && src == n {
                dmg.lost = true;
                dmg.no_reset = true;
            }
            self.dec[c].decode(&pkt, &bits, dmg, bw, predict, n as u64, o);
        }
        // the decoded frame n completes the samples [(n−1)N, nN)
        for (c, o) in out.iter().enumerate() {
            // the decoder's PCM saturates at full scale
            self.out[c].extend(o.iter().map(|x| x.clamp(-1.0, 1.0) as f32));
        }
    }

    fn restart(&mut self) {
        let kbps = kbps_of(&self.v);
        self.enc = [Encoder::new(kbps / 2.0), Encoder::new(kbps / 2.0)];
        self.dec = [Decoder::new(0x0A5D), Decoder::new(0x0A5E)];
        self.input = [Vec::new(), Vec::new()];
        self.out = [VecDeque::new(), VecDeque::new()];
        self.next_frame = None;
        self.packets.clear();
        self.deleted.clear();
    }
}

fn kbps_of(v: &[f64]) -> f64 {
    KBPS_V[(v[crate::rows::AUDIO_KBPS].round().max(0.0) as usize).min(4)]
}

impl Default for MoshSound {
    fn default() -> MoshSound {
        MoshSound::new()
    }
}

impl AudioState for MoshSound {
    fn apply(&mut self, k: &[f64], structural: bool) {
        let mut v = crate::defaults();
        for (o, x) in v.iter_mut().zip(k) {
            *o = *x;
        }
        let rebuild = kbps_of(&v) != kbps_of(&self.v);
        self.v = v;
        if structural || rebuild {
            self.restart();
        }
    }

    fn bind(&mut self, c: &dyn SlotCurves, clip_len: i64) {
        self.clip_len = clip_len.max(1);
        let specs = crate::Datamosh::passport();
        self.curves = specs
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == ParamKind::Smooth && !c.points(&s.name).is_empty())
            .map(|(i, s)| (i, c.owned(&s.name)))
            .collect();
    }

    fn place(&mut self, at: Placement) {
        self.at = at;
    }

    fn lookahead(&self) -> i64 {
        LA
    }

    fn fork(&self) -> Option<Box<dyn AudioState>> {
        Some(Box::new(self.clone()))
    }

    fn reset(&mut self) {
        self.restart();
    }

    fn process(&mut self, buf: &mut [f32], pos: i64, _key: Option<&[f32]>) {
        let frames = buf.len() / 2;
        let n_in = self.at.start + pos;
        // a jump (not the continuation of what was fed): start the stream over at this frame grid
        let contiguous = self.next_frame.is_some() && self.buf_start + self.input[0].len() as i64 == n_in;
        if !contiguous {
            self.restart();
            let f = n_in.div_euclid(N as i64);
            // the first block reads one frame of silence before the grid point
            self.buf_start = (f - 1) * N as i64;
            let pad = (n_in - self.buf_start) as usize;
            self.input = [vec![0.0; pad], vec![0.0; pad]];
            self.next_frame = Some(f);
            self.out_start = (f - 1) * N as i64;
        }
        for i in 0..frames {
            self.input[0].push(buf[2 * i] as f64);
            self.input[1].push(buf[2 * i + 1] as f64);
        }
        // code every frame whose window [(n−1)N, (n+1)N) has arrived
        while let Some(n) = self.next_frame {
            if self.buf_start + (self.input[0].len() as i64) < (n + 1) * N as i64 {
                break;
            }
            self.frame(n);
            self.next_frame = Some(n + 1);
            // drop the input before the next window
            let keep_from = n * N as i64;
            let drop = (keep_from - self.buf_start) as usize;
            for c in 0..2 {
                self.input[c].drain(..drop);
            }
            self.buf_start = keep_from;
        }
        // the output: timeline frames from `n_in − LA`
        let o0 = n_in - LA;
        for i in 0..frames {
            let t = o0 + i as i64;
            let idx = t - self.out_start;
            let (l, r) = if idx >= 0 && (idx as usize) < self.out[0].len() {
                (self.out[0][idx as usize], self.out[1][idx as usize])
            } else {
                (0.0, 0.0)
            };
            buf[2 * i] = l;
            buf[2 * i + 1] = r;
        }
        // keep only what the next call can still read
        let next_o0 = o0 + frames as i64;
        let drop = (next_o0 - self.out_start).clamp(0, self.out[0].len() as i64) as usize;
        for c in 0..2 {
            self.out[c].drain(..drop);
        }
        self.out_start += drop as i64;
    }
}
