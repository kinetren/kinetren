//! compiled-curves (JIT tier T1): curves, speed profiles, links and formulas are compiled ONCE per
//! revision of what determines them, and every per-sample / per-frame question asks the compiled
//! form. Each test here holds one compiled form to the former interpreted path BIT FOR BIT
//! (invariant 11), or counts that a sweep stays linear.

use kt_model::*;
use kt_time::Tick;
use std::cell::Cell;
use std::collections::BTreeMap;

// ---- a counting allocator: bytes allocated on THIS thread --------------------------------------

struct Counting;

thread_local! {
    static BYTES: Cell<u64> = const { Cell::new(0) };
}

unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        let _ = BYTES.try_with(|b| b.set(b.get() + l.size() as u64));
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        let _ = BYTES.try_with(|b| b.set(b.get() + n as u64));
        unsafe { std::alloc::System.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn bytes_of<R>(f: impl FnOnce() -> R) -> (R, u64) {
    let b0 = BYTES.with(|b| b.get());
    let r = f();
    (r, BYTES.with(|b| b.get()) - b0)
}

// ---- the former interpreted paths, frozen ------------------------------------------------------

fn vars(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
}

/// The former segment formula: the interpreter over a map of four names, a straight line on error.
fn frozen_segment(text: &str, s: f64, a: f64, b: f64, secs: f64) -> f64 {
    let s = s.clamp(0.0, 1.0);
    match Formula::parse(text).ok().and_then(|f| f.eval(&vars(&[("s", s), ("a", a), ("b", b), ("t", secs)])).ok()) {
        Some(v) => v,
        None => a + (b - a) * s,
    }
}

/// The former `rate_profile_keys`, verbatim but for its names: the interpreter per grid point and
/// a halving `curve_value` per point.
fn frozen_rate_profile_keys(c: &ParamCurve, src_in_frames: i64, clip_len_frames: i64) -> Vec<AutoKey> {
    let straight = |k: &AutoKey| AutoKey::shaped(k.t, k.v, SegShape::Linear);
    let grid_up = |t: i64| t.div_euclid(RATE_GRID_TICKS) * RATE_GRID_TICKS + RATE_GRID_TICKS;
    if let Some(expr) = c.source.expr()
        && c.source.link_path().is_none()
        && let Ok(f) = Formula::parse(expr)
    {
        let from = src_in_frames * 15 / 8;
        let to = from + clip_len_frames.max(1) * 15 / 8;
        let lo = c.keys.first().map_or(from, |k| k.t.0.min(from));
        let hi = c.keys.last().map_or(to, |k| k.t.0.max(to));
        let mut out = Vec::new();
        let mut t = lo;
        loop {
            let k = if c.keys.is_empty() { 0.0 } else { curve_value(&c.keys, Tick(t)) };
            match f.eval(&vars(&[("src", 0.0), ("k", k), ("t", (t - from) as f64 / 90_000.0)])) {
                Ok(v) if v.is_finite() => out.push(AutoKey::shaped(Tick(t), v, SegShape::Linear)),
                _ => return c.keys.iter().map(straight).collect(),
            }
            if t >= hi {
                break;
            }
            t = grid_up(t).min(hi);
        }
        return out;
    }
    let keys = &c.keys;
    if keys.len() < 2 || keys[..keys.len() - 1].iter().all(|k| k.shape == SegShape::Linear) {
        return keys.iter().map(straight).collect();
    }
    let mut out = Vec::with_capacity(keys.len());
    for (i, k) in keys.iter().enumerate() {
        out.push(straight(k));
        let Some(next) = keys.get(i + 1) else { break };
        if k.shape == SegShape::Linear || next.t.0 - k.t.0 < 2 {
            continue;
        }
        let mut t = grid_up(k.t.0);
        while t < next.t.0 - 1 {
            out.push(AutoKey::shaped(Tick(t), curve_value(keys, Tick(t)), SegShape::Linear));
            t += RATE_GRID_TICKS;
        }
        let last = next.t.0 - 1;
        out.push(AutoKey::shaped(Tick(last), curve_value(keys, Tick(last)), SegShape::Linear));
    }
    out
}

fn bits(keys: &[AutoKey]) -> Vec<(i64, u64, SegShape)> {
    keys.iter().map(|k| (k.t.0, k.v.to_bits(), k.shape)).collect()
}

/// A deterministic generator (no randomness from the environment, t1671).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.next() as usize % xs.len()]
    }
    fn f(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f64 / 1_000_000.0
    }
}

/// A random expression over `t`, `src`, `k` and, rarely, an unknown name.
fn random_expr(g: &mut Lcg, depth: u32) -> String {
    if depth == 0 || g.next().is_multiple_of(4) {
        return match g.next() % 6 {
            0 => format!("{}", g.f(-5.0, 5.0)),
            1 => "0".into(),
            5 if g.next().is_multiple_of(8) => "nope".into(),
            _ => g.pick(&["t", "src", "k"]).into(),
        };
    }
    let a = random_expr(g, depth - 1);
    let b = random_expr(g, depth - 1);
    let c = random_expr(g, depth - 1);
    match g.next() % 9 {
        0..=3 => format!("({a} {} {b})", g.pick(&["+", "-", "*", "/", "%", "<", ">", "<=", ">=", "==", "!="])),
        4 => format!("-{a}"),
        5 => format!("if({a}, {b}, {c})"),
        6 => format!("{}({a})", g.pick(&["abs", "floor", "ceil", "round", "sqrt", "sin", "cos", "noise"])),
        7 => format!("{}({a}, {b})", g.pick(&["min", "max"])),
        _ => format!("{}({a}, {b}, {c})", g.pick(&["clamp", "lerp"])),
    }
}

fn same(a: &Result<f64, FormulaError>, b: &Result<f64, FormulaError>) -> bool {
    match (a, b) {
        (Ok(x), Ok(y)) => x.to_bits() == y.to_bits(),
        (Err(x), Err(y)) => x.kind == y.kind,
        _ => false,
    }
}

// ---- t2100: compiled formulas are the interpreter's bits ---------------------------------------

#[test]
fn t2100_compiled_formula_is_the_interpreter_bit_for_bit() {
    kt_plugins_builtin::install();
    let fixed = [
        "1 + 2 * 3",
        "t / 0",
        "t % 0",
        "if(1, 5, 1/0)",
        "if(0, 1/0, 7)",
        "min(1/0, 2)",
        "lerp(1/0, 2, 3)",
        "nope + 1",
        "sqrt(0 - 1)",
        "src * src * src",
        "noise(t * 7.5) + sin(src) * cos(k)",
        "clamp(k, 3, -3)",
        "round(-2.5) + floor(-0.5) + ceil(0.5)",
        "t < src == k",
    ];
    let mut g = Lcg(2100);
    let mut texts: Vec<String> = fixed.iter().map(|s| s.to_string()).collect();
    while texts.len() < 600 {
        let e = random_expr(&mut g, 4);
        if Formula::parse(&e).is_ok() {
            texts.push(e);
        }
    }
    let inputs: Vec<[f64; 3]> = (0..40)
        .map(|i| match i {
            0 => [0.0, 0.0, 0.0],
            1 => [-0.0, 1.0, -1.0],
            2 => [1e-300, -1e300, 0.5],
            _ => [g.f(-20.0, 20.0), g.f(-3.0, 3.0), g.f(-100.0, 100.0)],
        })
        .collect();
    let mut errors = 0;
    for text in &texts {
        let f = Formula::parse(text).expect("parsed");
        let c = f.compile(LAYER_VARS);
        for x in &inputs {
            let interp = f.eval(&vars(&[("t", x[0]), ("src", x[1]), ("k", x[2])]));
            let comp = c.eval(x);
            assert!(same(&interp, &comp), "{text} at {x:?}: interpreted {interp:?}, compiled {comp:?}");
            errors += usize::from(interp.is_err());
        }
    }
    assert!(errors > 100, "the errors must be exercised too, got {errors}");
}

// ---- t2101: a segment formula reads compiled, a cursor reads as the halving --------------------

#[test]
fn t2101_segment_formula_and_cursor_are_the_former_bits() {
    kt_plugins_builtin::install();
    let texts = ["a + (b - a) * s * s", "a + sin(t * 9) * (b - a)", "b / (s - 0.5)", "s * zzz", "lerp(a, b, s) + noise(t * 30) * 0.1"];
    let mut g = Lcg(2101);
    for text in texts {
        let shape = SegShape::Formula { expr: ExprId::intern(text) };
        for _ in 0..500 {
            let (s, a, b, secs) = (g.f(-0.2, 1.2), g.f(-5.0, 5.0), g.f(-5.0, 5.0), g.f(0.0, 30.0));
            let got = shape.at_t(s, a, b, secs);
            let want = frozen_segment(text, s, a, b, secs);
            assert_eq!(got.to_bits(), want.to_bits(), "{text} at s={s} a={a} b={b} t={secs}");
        }
    }

    // a reading allocates nothing and parses nothing once the text is compiled on this thread
    let keys = vec![
        AutoKey::shaped(Tick(0), 1.0, SegShape::Formula { expr: ExprId::intern(texts[1]) }),
        AutoKey::at(Tick(90_000 * 10), 2.0),
    ];
    let _ = curve_value(&keys, Tick(1));
    let ((_, costs), bytes) = bytes_of(|| {
        kt_time::cost::meter(|| {
            let mut acc = 0.0;
            for i in 0..2000 {
                acc += curve_value(&keys, Tick(i * 450));
            }
            acc
        })
    });
    assert_eq!(costs[kt_time::cost::Cost::FormulaParse as usize], 0, "a reading never parses");
    assert_eq!(bytes, 0, "a reading of a segment formula allocates nothing (was a text clone and a map of four names)");

    // the cursor: any order of positions, the halving's bits
    let mut keys = Vec::new();
    let shapes = [
        SegShape::Linear,
        SegShape::Hold,
        SegShape::DEFAULT,
        SegShape::EaseIn { tension: 2.0 },
        SegShape::Zigzag { periods: 3.0, amplitude: 0.4, profile: ZigProfile::Sine },
        SegShape::Formula { expr: ExprId::intern(texts[0]) },
    ];
    let mut t = -9000;
    for i in 0usize..300 {
        // duplicate ticks are legal (a step)
        if !i.is_multiple_of(17) {
            t += (g.next() % 9000) as i64;
        }
        keys.push(AutoKey::shaped(Tick(t), g.f(-3.0, 3.0), shapes[i % shapes.len()]));
    }
    let mut cur = CurveCursor::new(&keys);
    let mut at = -20_000i64;
    for i in 0..20_000 {
        at += if i % 50 == 49 { -((g.next() % 400_000) as i64) } else { (g.next() % 3000) as i64 };
        assert_eq!(cur.value(Tick(at)).to_bits(), curve_value(&keys, Tick(at)).to_bits(), "at {at}");
    }
    assert_eq!(CurveCursor::new(&[]).value(Tick(5)), 0.0);
}

// ---- t2102 / t2103: a speed profile sampled by the compiled forms ------------------------------

fn rate(keys: Vec<AutoKey>, source: CurveSource) -> ParamCurve {
    ParamCurve { param: "rate".into(), keys, source }
}

fn profile_cases() -> Vec<(ParamCurve, i64, i64)> {
    let f = |s: &str| SegShape::Formula { expr: ExprId::intern(s) };
    let sec = 90_000;
    let layer = |e: &str| CurveSource::Formula { expr: e.into() };
    vec![
        (rate(AutoKey::linear(vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(sec), 2.0)]), CurveSource::Keys), 0, 48_000),
        (
            rate(
                vec![
                    AutoKey::shaped(Tick(0), 1.0, SegShape::DEFAULT),
                    AutoKey::shaped(Tick(3 * sec), 0.5, SegShape::Hold),
                    AutoKey::shaped(Tick(5 * sec), 2.0, SegShape::EaseOut { tension: 1.5 }),
                    AutoKey::shaped(Tick(9 * sec), -1.0, SegShape::Zigzag { periods: 7.0, amplitude: 0.8, profile: ZigProfile::Triangle }),
                    AutoKey::shaped(Tick(14 * sec), 1.0, f("a + (b - a) * s + 0.3 * sin(t * 6)")),
                    AutoKey::at(Tick(40 * sec), 1.5),
                ],
                CurveSource::Keys,
            ),
            -96_000,
            40 * 48_000,
        ),
        (rate(Vec::new(), layer("1 + 0.5 * sin(6 * t)")), 4_800, 20 * 48_000),
        (
            rate(AutoKey::linear(vec![AutoKey::at(Tick(sec), 0.5), AutoKey::at(Tick(8 * sec), 3.0)]), layer("k * (1 + 0.2 * cos(t * 3))")),
            48_000,
            10 * 48_000,
        ),
        // division by zero at one grid point: the keys stay (validation names it)
        (rate(AutoKey::linear(vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(sec), 2.0)]), layer("1 / (t - 0.5)")), 0, 48_000),
        // a link with a formula is resolved elsewhere: the keys as they are
        (
            rate(
                vec![AutoKey::shaped(Tick(0), 1.0, SegShape::DEFAULT), AutoKey::at(Tick(sec), 2.0)],
                CurveSource::Link { path: "1/fx0/mix".into(), expr: Some("src * 2".into()), stops: Vec::new() },
            ),
            0,
            48_000,
        ),
    ]
}

#[test]
fn t2103_rate_profile_keys_is_the_former_sampling_bit_for_bit() {
    kt_plugins_builtin::install();
    for (i, (c, src_in, len)) in profile_cases().iter().enumerate() {
        let got = rate_profile_keys(c, *src_in, *len);
        let want = frozen_rate_profile_keys(c, *src_in, *len);
        assert_eq!(bits(&got), bits(&want), "case {i}");
    }
}

#[test]
fn t2102_a_sampled_profile_costs_its_points_not_points_times_names() {
    kt_plugins_builtin::install();
    let sec = 90_000;
    // 60 s of a segment formula and 60 s of a layer formula over dense keys (a resolved link)
    let seg = rate(
        vec![
            AutoKey::shaped(Tick(0), 1.0, SegShape::Formula { expr: ExprId::intern("a + (b - a) * s + 0.25 * sin(t * 5)") }),
            AutoKey::at(Tick(60 * sec), 2.0),
        ],
        CurveSource::Keys,
    );
    let dense: Vec<AutoKey> = (0..=1800).map(|i| AutoKey::shaped(Tick(i * 3000), 1.0 + (i % 7) as f64 * 0.1, SegShape::Linear)).collect();
    let layer = rate(dense.clone(), CurveSource::Formula { expr: "k * (1 + 0.1 * sin(t))".into() });
    for c in [&seg, &layer] {
        let _ = rate_profile_keys(c, 0, 60 * 48_000); // the segment text compiled on this thread
        let ((out, costs), bytes) = bytes_of(|| kt_time::cost::meter(|| rate_profile_keys(c, 0, 60 * 48_000)));
        let n = out.len() as u64;
        assert!(n > 10_000, "a 5 ms grid over 60 s");
        let walks = costs[kt_time::cost::Cost::CurveWalk as usize];
        assert!(walks <= 2 * (n + c.keys.len() as u64), "the curve under the sampling is read by a cursor: {walks} walks for {n} points");
        assert!(costs[kt_time::cost::Cost::FormulaParse as usize] <= 1, "one parse per profile");
        let per_point = bytes / n;
        assert!(
            per_point <= 3 * std::mem::size_of::<AutoKey>() as u64,
            "a point allocates only its place in the output: {per_point} bytes a point (was a map of names per point)"
        );
    }
}

// ---- t2104 / t2105: the profile memo is the function's own value -------------------------------

fn warped(keys: Vec<AutoKey>, source: CurveSource) -> ClipEvent {
    ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(90_000),
        duration: Tick(90_000 * 30),
        source_in: Tick(45_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::slowed_reverb(0.8, 0.0, 85.0)],
        fx_bypass: vec![false],
        fx_auto: vec![vec![rate(keys, source)]],
        opacity: 1.0,
        fx_off: false,
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
    }
}

fn warp_clips() -> Vec<ClipEvent> {
    let sec = 90_000;
    let shaped = vec![
        AutoKey::shaped(Tick(0), 0.7, SegShape::DEFAULT),
        AutoKey::shaped(Tick(10 * sec), -0.5, SegShape::Zigzag { periods: 4.0, amplitude: 0.3, profile: ZigProfile::Square }),
        AutoKey::shaped(Tick(20 * sec), 1.3, SegShape::Formula { expr: ExprId::intern("a + (b - a) * s * s") }),
        AutoKey::at(Tick(31 * sec), 0.9),
    ];
    let mut out = vec![
        warped(shaped.clone(), CurveSource::Keys),
        warped(Vec::new(), CurveSource::Formula { expr: "1 + 0.5 * sin(4 * t)".into() }),
        warped(AutoKey::linear(vec![AutoKey::at(Tick(0), 0.5), AutoKey::at(Tick(31 * sec), 2.0)]), CurveSource::Keys),
        warped(Vec::new(), CurveSource::Keys),
    ];
    let mut r = out[0].clone();
    r.reversed = true;
    out.push(r);
    let mut trimmed = out[0].clone();
    trimmed.source_in = Tick(5 * sec);
    trimmed.duration = Tick(7 * sec);
    out.push(trimmed);
    out
}

/// A map as bits: base, points, used, span, blend, the material at 201 moments.
type WarpBits = (u64, Vec<(u64, u64)>, i64, (i64, i64), bool, Vec<i64>);

fn warp_bits(w: &ClipWarp) -> WarpBits {
    (
        w.base().to_bits(),
        w.prof().iter().map(|p| (p.0.to_bits(), p.1.to_bits())).collect(),
        w.used(),
        w.span(),
        w.blend(),
        (0..=200).map(|i| w.material_off(Tick(90_000 * 30), false, i * 13_500)).collect(),
    )
}

fn profile_bits(c: &ClipEvent) -> Vec<(u64, u64)> {
    let len = c.duration.to_audio_frames().max(1);
    AutoProfiles::from_curves_rev(&c.active_fx_auto(), c.source_in.to_audio_frames(), len, c.reversed)
        .rate_points(&c.active_fx())
        .iter()
        .map(|p| (p.0.to_bits(), p.1.to_bits()))
        .collect()
}

#[test]
fn t2104_memoized_profiles_equal_fresh_ones() {
    kt_plugins_builtin::install();
    for (i, c) in warp_clips().iter().enumerate() {
        let fresh = without_profile_memo(|| clip_warp(c)).map(|w| warp_bits(&w));
        let first = clip_warp(c).map(|w| warp_bits(&w));
        let hit = clip_warp(c).map(|w| warp_bits(&w));
        assert_eq!(first, fresh, "clip {i}: memo miss");
        assert_eq!(hit, fresh, "clip {i}: memo hit");
        let fresh_p = without_profile_memo(|| profile_bits(c));
        assert_eq!(profile_bits(c), fresh_p, "clip {i}: audio profile, miss");
        assert_eq!(profile_bits(c), fresh_p, "clip {i}: audio profile, hit");
        // a hit samples nothing
        let (_, costs) = kt_time::cost::meter(|| (clip_warp(c), profile_bits(c)));
        assert_eq!(costs[kt_time::cost::Cost::ProfileSample as usize], 0, "clip {i}: a hit samples no profile");
        assert_eq!(costs[kt_time::cost::Cost::WarpSegment as usize], 0, "clip {i}: a hit lays out no map");
    }
}

#[test]
fn t2105_the_memo_key_is_everything_that_determines_the_profile() {
    kt_plugins_builtin::install();
    let base = warp_clips().remove(0);
    let _ = clip_warp(&base);
    let _ = profile_bits(&base);
    let mut variants: Vec<(&str, ClipEvent)> = Vec::new();
    let mut v = base.clone();
    v.fx_auto[0][0].keys[1].v = f64::from_bits(v.fx_auto[0][0].keys[1].v.to_bits() + 1);
    variants.push(("a value by one ulp", v));
    let mut v = base.clone();
    v.fx_auto[0][0].keys[0].shape = SegShape::Smooth { tension: 1.5 };
    variants.push(("a shape's tension", v));
    let mut v = base.clone();
    v.fx_auto[0][0].keys[2].shape = SegShape::Formula { expr: ExprId::intern("a + (b - a) * s * s * s") };
    variants.push(("a segment formula's text", v));
    let mut v = base.clone();
    v.fx_auto[0][0].keys[1].t = Tick(v.fx_auto[0][0].keys[1].t.0 + 1);
    variants.push(("a key's tick", v));
    let mut v = base.clone();
    v.fx_auto[0][0].source = CurveSource::Formula { expr: "k + 0.1".into() };
    variants.push(("a layer formula", v));
    let mut v = base.clone();
    v.source_in = Tick(v.source_in.0 + 16);
    variants.push(("the material entry", v));
    let mut v = base.clone();
    v.duration = Tick(v.duration.0 + 16);
    variants.push(("the length", v));
    let mut v = base.clone();
    v.reversed = true;
    variants.push(("the reverse", v));
    let mut v = base.clone();
    v.fx[0] = FxInstance::slowed_reverb(0.9, 0.0, 85.0);
    variants.push(("the base speed", v));
    let base_bits = clip_warp(&base).map(|w| warp_bits(&w));
    for (what, v) in variants {
        let fresh = without_profile_memo(|| clip_warp(&v)).map(|w| warp_bits(&w));
        assert_ne!(fresh, base_bits, "{what}: the scene must move the map at all");
        assert_eq!(clip_warp(&v).map(|w| warp_bits(&w)), fresh, "{what}: the memo must not return the base's map");
        assert_eq!(profile_bits(&v), without_profile_memo(|| profile_bits(&v)), "{what}: audio profile");
    }
}

// ---- t2109: links and formula layers resolve through the compiled forms -------------------------

#[test]
fn t2109_derived_curves_resolve_to_the_interpreted_values() {
    kt_plugins_builtin::install();
    let step = 3000; // 30 fps
    let mut c = warped(Vec::new(), CurveSource::Keys);
    c.fx = vec![FxInstance::plugin_with("tremolo", &[5.0, 1.0]).unwrap()];
    c.fx_bypass = vec![false];
    let under = vec![AutoKey::shaped(Tick(45_000), 0.5, SegShape::DEFAULT), AutoKey::at(Tick(90_000 * 20), 0.9)];
    let text = "k * (1 + 0.3 * sin(t * 2)) + noise(t * 10) * 0.01";
    c.fx_auto = vec![vec![ParamCurve { param: "depth".into(), keys: under.clone(), source: CurveSource::Formula { expr: text.into() } }]];
    let resolved = resolve_derived(std::slice::from_ref(&c), step);
    let keys = &resolved[0].fx_auto[0][0].keys;
    assert!(resolved[0].fx_auto[0][0].source.is_keys());
    assert!(keys.len() > 800);
    let f = Formula::parse(text).unwrap();
    for k in keys {
        let want = f
            .eval(&vars(&[("src", 0.0), ("k", curve_value(&under, k.t)), ("t", (k.t.0 - c.source_in.0) as f64 / 90_000.0)]))
            .unwrap();
        assert_eq!(k.v.to_bits(), want.to_bits(), "at {:?}", k.t);
    }
    // the preview of the same layer reads the same numbers
    let prev = formula_preview(&c.fx_auto[0][0], (c.source_in, c.source_in + c.duration), c.source_in, c.source_in + Tick(90_000), step);
    for p in &prev {
        let at = keys.iter().find(|k| k.t == p.t).expect("on the grid");
        assert_eq!(p.v.to_bits(), at.v.to_bits());
    }
    // a reel whose only derived curve is a speed formula layer resolves nothing: borrowed
    let speed = warped(Vec::new(), CurveSource::Formula { expr: "1 + 0.5 * sin(t)".into() });
    assert!(matches!(resolve_derived(std::slice::from_ref(&speed), step), std::borrow::Cow::Borrowed(_)));
}

// ---- t2110: a stream stage's slot equals the whole plan's slot ----------------------------------

#[test]
fn t2110_one_slot_of_the_plan_equals_the_whole_plan() {
    kt_plugins_builtin::install();
    let fx = vec![
        FxInstance::plugin_with("tremolo", &[5.0, 1.0]).unwrap(),
        FxInstance::plugin_with("tremolo", &[2.0, 0.5]).unwrap(),
    ];
    let curve = |p: &str, a: f64, b: f64| ParamCurve {
        param: p.into(),
        keys: vec![AutoKey::shaped(Tick(0), a, SegShape::DEFAULT), AutoKey::at(Tick(90_000 * 4), b)],
        source: CurveSource::Keys,
    };
    let auto = vec![vec![curve("depth", 0.1, 0.9)], vec![curve("rate_hz", 1.0, 9.0), curve("depth", 0.7, 0.2)]];
    let plan = AutoPlan::compile(&fx, &auto).expect("live curves");
    for at in (0..40).map(|i| Tick(i * 10_000 - 5_000)) {
        let mut whole = fx.clone();
        plan.apply(&mut whole, at);
        for (slot, f) in fx.iter().enumerate() {
            let mut one = f.clone();
            plan.apply_slot(slot, &mut one, at);
            assert_eq!(one, whole[slot], "slot {slot} at {at:?}");
        }
    }
}
