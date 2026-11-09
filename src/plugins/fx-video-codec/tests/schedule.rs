//! The datamosh's schedule compiled once (stream-layout-per-build): t2270 (the keys, the needs and
//! the held positions are the values of the per-call walk, pinned from it), t2271 (a layout with the
//! host's cell compiles each position once and answers as one without), t2272 (the audio half's bytes
//! under every gate, pinned from the per-frame schedule). No compute device is needed: nothing here
//! encodes a frame.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Arc;

use fx_video_codec::mosh::{self, Rows};
use fx_video_codec::rows;
use kt_plugin_api::stream::{mix, PlanCell, Side, StreamLayout, StreamMemo};

/// A stream described by functions: its rows per position (the curves) and a second input made of
/// events `[s, e)`. Addresses are a fixed hash of the side and the position.
struct Lay {
    first: i64,
    cut: i64,
    end: i64,
    base: Vec<f64>,
    curve: Option<fn(i64, &mut Vec<f64>)>,
    second: Vec<(i64, i64)>,
    cell: Option<PlanCell>,
    rows_read: AtomicU64,
}

impl Lay {
    fn new(first: i64, cut: i64, end: i64, base: Vec<f64>) -> Lay {
        Lay { first, cut, end, base, curve: None, second: vec![], cell: None, rows_read: AtomicU64::new(0) }
    }
}

impl StreamLayout for Lay {
    fn size(&self) -> (u32, u32) {
        (64, 48)
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
    fn second_start(&self, k: i64) -> Option<i64> {
        self.second.iter().find(|(s, e)| *s <= k && k < *e).map(|(s, _)| *s)
    }
    fn ident(&self, side: Side, k: i64) -> u64 {
        match side {
            Side::Main if k >= self.first && k < self.end => mix(0x11, k as u64),
            Side::Second if self.second_start(k).is_some() => mix(0x22, k as u64),
            _ => 0,
        }
    }
    fn rows_at(&self, k: i64) -> Option<Vec<f64>> {
        self.rows_read.fetch_add(1, Relaxed);
        let c = self.curve?;
        let mut v = self.base.clone();
        c(k, &mut v);
        Some(v)
    }
    fn plan_cell(&self) -> Option<&PlanCell> {
        self.cell.as_ref()
    }
}

fn params(sets: &[(usize, f64)]) -> Vec<f64> {
    let mut v = fx_video_codec::defaults();
    for &(i, x) in sets {
        v[i] = x;
    }
    v
}

/// The streams of the test: every breakage the schedule knows, gates drawn per position.
fn streams() -> Vec<Lay> {
    let mut out = vec![];
    // a cut mosh from 12 frames before the cut
    out.push(Lay::new(-12, 0, 160, params(&[(rows::BEFORE, 12.0), (rows::MOSH, 1.0), (rows::GOP, 24.0)])));
    // beats: mosh on, a reset key at 40 and 90, bloom over [10, 30), freeze over [50, 66), per-frame
    // rotation and loss, vectors from a second input of two events
    let mut b = Lay::new(0, 0, 200, params(&[(rows::GOP, 30.0), (rows::VECTORS_FROM, 1.0)]));
    b.curve = Some(|k, v| {
        v[rows::MOSH] = if (5..120).contains(&k) { 1.0 } else { 0.0 };
        v[rows::RESET] = if (40..44).contains(&k) || (90..95).contains(&k) { 1.0 } else { 0.0 };
        v[rows::BLOOM] = if (10..30).contains(&k) || (130..150).contains(&k) { 1.0 } else { 0.0 };
        v[rows::FREEZE] = if (50..66).contains(&k) { 1.0 } else { 0.0 };
        v[rows::MV_ROTATE] = (k % 17) as f64 * 3.0;
        v[rows::LOSS] = if k % 5 == 0 { 0.2 } else { 0.0 };
        v[rows::QP] = 20.0 + (k % 4) as f64;
    });
    b.second = vec![(-30, 70), (70, 180)];
    out.push(b);
    // gop 0 with the mosh held: a chain past the intra refresh (600) twice
    out.push(Lay::new(0, 0, 1400, params(&[(rows::GOP, 0.0), (rows::MOSH, 1.0)])));
    out
}

/// Everything a host asks of the schedule, over every position (and two before the stream), folded
/// into one number: the key, the needs with an empty memo, the needs and the held position with a
/// memo holding the broken pictures of every 7th position.
fn fold(lay: &Lay, order: &[i64]) -> u64 {
    let r = Rows::from(&lay.base);
    let empty = StreamMemo::new(1 << 20);
    let some = StreamMemo::new(1 << 30);
    for k in (lay.first..lay.end).step_by(7) {
        some.put(mosh::key(r, lay, k), Arc::new(0u8), 1);
    }
    let mut h = 0u64;
    for &n in order {
        h = mix(h, mosh::key(r, lay, n));
        for (s, k) in mosh::needs(r, lay, n, &empty) {
            h = mix(mix(h, s as u64), k as u64);
        }
        h = mix(h, 0xfeed);
        for (s, k) in mosh::needs(r, lay, n, &some) {
            h = mix(mix(h, s as u64), k as u64);
        }
        h = mix(h, mosh::held(r, lay, n, &some).map_or(u64::MAX, |p| p as u64));
    }
    h
}

/// The values the per-call walk gave (stream-layout-per-build: pinned from it before the schedule
/// was compiled).
const PINNED: [u64; 3] = [0x9637_b77c_9a8b_a9bb, 0xe1b2_c8f1_41c3_9d49, 0xadd3_fce4_fe2f_75c9];

/// t2270: the compiled schedule answers exactly what the walk did, key by key, need by need.
#[test]
fn t2270_the_compiled_schedule_is_the_walk() {
    let got: Vec<u64> = streams().iter().map(|l| fold(l, &(l.first - 2..l.end).collect::<Vec<_>>())).collect();
    eprintln!("{got:#x?}");
    assert_eq!(got, PINNED);
}

/// t2271: with the host's cell each position's rows are read once, whatever the order of the
/// questions, and the answers are those of a layout without a cell.
#[test]
fn t2271_a_cell_compiles_once() {
    for mut l in streams() {
        let fwd: Vec<i64> = (l.first - 2..l.end).collect();
        // a seek far ahead, then back, then the rest
        let mut mixed: Vec<i64> = fwd.iter().rev().step_by(3).copied().collect();
        mixed.extend(fwd.iter().step_by(2));
        mixed.extend(fwd.iter().skip(1).step_by(2));
        let want = fold(&l, &mixed);
        let without = l.rows_read.swap(0, Relaxed);
        l.cell = Some(PlanCell::default());
        assert_eq!(fold(&l, &mixed), want, "the cell changed an answer");
        let with = l.rows_read.load(Relaxed);
        let positions = (l.end - l.first) as u64;
        assert!(with <= positions, "{with} rows read for {positions} positions");
        assert!(without > 20 * with, "the test does not exercise the walk ({without} vs {with})");
    }
}

/// Gates of the sound test, drawn over the event's progress `s` (0…1).
#[derive(Clone)]
struct Drawn(&'static str);

impl kt_plugin_api::CurveRead for Drawn {
    fn value(&self, base: f64, s: f64) -> f64 {
        let on = |a: f64, b: f64| if s >= a && s < b { 1.0 } else { 0.0 };
        match self.0 {
            "mosh" => on(0.1, 0.45),
            "reset" => on(0.45, 0.5),
            "bloom" => on(0.55, 0.65),
            "freeze" => on(0.7, 0.8),
            "loss" => 0.3 * on(0.82, 0.9),
            "residual" => 1.0 - 0.6 * on(0.9, 1.0),
            "qp" => 24.0 + (s * 40.0).floor() % 5.0,
            _ => base,
        }
    }
    fn clone_box(&self) -> Box<dyn kt_plugin_api::CurveRead> {
        Box::new(self.clone())
    }
}

struct Curves;

const DRAWN: [&str; 7] = ["mosh", "reset", "bloom", "freeze", "loss", "residual", "qp"];

impl kt_plugin_api::SlotCurves for Curves {
    fn value(&self, param: &str, base: f64, s: f64) -> f64 {
        self.owned(param).value(base, s)
    }
    fn points(&self, param: &str) -> &kt_plugin_api::curves::ParamPoints {
        if DRAWN.contains(&param) { &[(0.0, 0.0), (1.0, 1.0)] } else { &[] }
    }
    fn owned(&self, param: &str) -> Box<dyn kt_plugin_api::CurveRead> {
        Box::new(Drawn(DRAWN.iter().copied().find(|p| *p == param).unwrap_or("")))
    }
}

/// The datamosh's sound over 6 s of an event at 25 fps under every gate, played through with one
/// seek back into the middle; the output's bytes folded.
fn sound_bytes() -> u64 {
    use kt_plugin_api::{AudioState, Placement};
    let mut s = fx_video_codec::sound::MoshSound::new();
    let start = 48_000 * 2 + 777;
    s.place(Placement { start, fps_num: 25, fps_den: 1 });
    s.apply(&params(&[(rows::GOP, 12.0), (rows::SEED, 3.0)]), true);
    let n = 6 * 48_000;
    s.bind(&Curves, n);
    let mut h = 0u64;
    let mut pos = 0i64;
    let mut seeked = false;
    while pos < n {
        let mut buf = vec![0f32; 2048];
        for i in 0..1024 {
            let t = (start + pos + i as i64) as f64 / 48_000.0;
            let x = (0.4 * (t * 440.0 * std::f64::consts::TAU).sin() * (1.0 + 0.5 * (t * 3.0).sin())) as f32;
            buf[2 * i] = x;
            buf[2 * i + 1] = -x;
        }
        s.process(&mut buf, pos, None);
        for x in &buf {
            h = mix(h, x.to_bits() as u64);
        }
        pos += 1024;
        if !seeked && pos >= 4 * 48_000 {
            seeked = true;
            pos = 48_000 + 512;
        }
    }
    h
}

const SOUND_PINNED: u64 = 0xa7b1_057f_eb66_e315;

/// t2272: the audio half reads the picture's schedule compiled once and gives the bytes the per-frame
/// schedule gave, every gate drawn, across a seek.
#[test]
fn t2272_the_sound_keeps_its_bytes() {
    let a = sound_bytes();
    eprintln!("{a:#x}");
    assert_eq!(a, sound_bytes(), "two runs differ");
    assert_eq!(a, SOUND_PINNED);
}
