//! shape-masks: the rasterizer — shapes, antialiasing, feather, falloff, expansion, modes,
//! resolution invariance, determinism.

use kt_fx::mask::{MaskSpace, apply_alpha, apply_mix, rasterize, rasterize_cached};
use kt_model::{Falloff, Mask, MaskMode, MaskShape, Vertex};

fn sp(n: u32) -> MaskSpace {
    MaskSpace::plain(n as f64, n as f64, n, n, n as f64)
}

fn ellipse(cx: f64, cy: f64, w: f64, h: f64) -> Mask {
    Mask::new(1, "e", MaskShape::Ellipse { cx, cy, w, h, rot: 0.0 })
}

fn rect(cx: f64, cy: f64, w: f64, h: f64, rot: f64, round: f64) -> Mask {
    Mask::new(1, "r", MaskShape::Rect { cx, cy, w, h, rot, round })
}

fn area(c: &kt_fx::mask::Coverage) -> f64 {
    c.cov.iter().map(|&v| v as f64 / 255.0).sum()
}

fn at(c: &kt_fx::mask::Coverage, x: u32, y: u32) -> u8 {
    c.cov[(y * c.w + x) as usize]
}

/// Test 1266: each shape covers its analytic area; the edge is antialiased and a pixel clear of
/// the contour is fully in or out; a Bézier circle is the ellipse.
#[test]
fn t1266_shapes_cover_their_area_with_antialiased_edges() {
    let n = 200;
    let rel = |got: f64, want: f64| (got - want).abs() / want;
    let r = rasterize(&[rect(0.5, 0.5, 0.4, 0.3, 0.0, 0.0)], &sp(n), n, n);
    assert!(rel(area(&r), 4800.0) < 0.005, "rect {}", area(&r));
    let r = rasterize(&[rect(0.5, 0.5, 0.4, 0.3, 30.0, 0.0)], &sp(n), n, n);
    assert!(rel(area(&r), 4800.0) < 0.005, "rotated rect {}", area(&r));
    let r = rasterize(&[rect(0.5, 0.5, 0.4, 0.3, 0.0, 1.0)], &sp(n), n, n);
    let stadium = 4800.0 - (4.0 - std::f64::consts::PI) * 900.0;
    assert!(rel(area(&r), stadium) < 0.005, "rounded rect {} vs {stadium}", area(&r));
    let c = rasterize(&[ellipse(0.5, 0.5, 0.5, 0.5)], &sp(n), n, n);
    let disc = std::f64::consts::PI * 2500.0;
    assert!(rel(area(&c), disc) < 0.005, "disc {}", area(&c));
    // the edge: intermediate values exist, and 1.5 px off the circle everything is 0 or 255
    let mut soft = 0;
    for y in 0..n {
        for x in 0..n {
            let v = at(&c, x, y);
            let (dx, dy) = (x as f64 + 0.5 - 100.0, y as f64 + 0.5 - 100.0);
            let d = ((dx * dx + dy * dy).sqrt() - 50.0).abs();
            if v > 0 && v < 255 {
                soft += 1;
            }
            if d > 1.5 {
                assert!(v == 0 || v == 255, "a pixel {d} px off the contour is {v}");
            }
        }
    }
    assert!(soft > 200, "no antialiasing: {soft} soft pixels");
    // the same circle as a Bézier path
    let k = 0.552_284_749_830_793_4 * 0.25;
    let v = |id, x: f64, y: f64, ix: f64, iy: f64| Vertex { id, x, y, ix, iy, ox: -ix, oy: -iy };
    let path = Mask::new(
        1,
        "p",
        MaskShape::Path {
            verts: vec![
                v(0, 0.75, 0.5, 0.0, -k),
                v(1, 0.5, 0.75, k, 0.0),
                v(2, 0.25, 0.5, 0.0, k),
                v(3, 0.5, 0.25, -k, 0.0),
            ],
        },
    );
    let p = rasterize(&[path], &sp(n), n, n);
    let close = p.cov.iter().zip(&c.cov).filter(|(a, b)| (**a as i32 - **b as i32).abs() <= 1).count();
    assert!(close as f64 / p.cov.len() as f64 > 0.99, "the Bézier circle differs from the ellipse");
}

/// Test 1267: the feather profile, the falloff, the expansion and the resolution.
#[test]
fn t1267_feather_falloff_expansion_and_resolution() {
    let n = 400;
    let ray = |m: &Mask| {
        let c = rasterize(std::slice::from_ref(m), &sp(n), n, n);
        (200..n).map(|x| at(&c, x, 200)).collect::<Vec<u8>>()
    };
    let mut prev_w = 0;
    for f in [0.0, 20.0, 80.0] {
        for fall in [Falloff::Smooth, Falloff::Linear] {
            let mut m = ellipse(0.5, 0.5, 0.25, 0.25); // radius 50
            m.feather = f;
            m.falloff = fall;
            let r = ray(&m);
            assert!(r.windows(2).all(|w| w[1] <= w[0]), "not monotonic: f={f} {fall:?}");
            // the 50 % level sits on the contour (the pixel centre 249.5 is r = 49.5: half a pixel in)
            let mid = r[49] as i32;
            assert!((mid - 128).abs() <= if f == 0.0 { 128 } else { 12 }, "f={f}: {mid}");
            let width = r.iter().filter(|&&v| v > 0 && v < 255).count();
            if fall == Falloff::Smooth {
                assert!(width >= prev_w, "a wider feather is not wider");
                prev_w = width;
            }
        }
    }
    // smooth is below linear a quarter into the feather, above it three quarters in
    let mut a = ellipse(0.5, 0.5, 0.25, 0.25);
    a.feather = 40.0;
    let mut b = a.clone();
    b.falloff = Falloff::Linear;
    let (sa, sb) = (ray(&a), ray(&b));
    assert!(sa[59] < sb[59] && sa[39] > sb[39], "smooth is not steeper in the middle");
    // expansion moves the 50 % level exactly
    for e in [-10.0, 10.0] {
        let mut m = ellipse(0.5, 0.5, 0.25, 0.25);
        m.feather = 20.0;
        m.expansion = e;
        let r = ray(&m);
        let edge = (49.5 + e) as usize;
        assert!((r[edge] as i32 - 128).abs() <= 12, "expansion {e}: {}", r[edge]);
    }
    // the same mask at 1/4 is the 1/1 one box-downsampled
    let mut m = ellipse(0.4, 0.55, 0.3, 0.2);
    m.feather = 20.0;
    let full = rasterize(std::slice::from_ref(&m), &sp(n), n, n);
    let q = MaskSpace::plain(n as f64, n as f64, n / 4, n / 4, n as f64);
    let quarter = rasterize(std::slice::from_ref(&m), &q, n / 4, n / 4);
    let mut worst = 0;
    for y in 0..n / 4 {
        for x in 0..n / 4 {
            let mut s = 0u32;
            for j in 0..4 {
                for i in 0..4 {
                    s += at(&full, x * 4 + i, y * 4 + j) as u32;
                }
            }
            let box_ = (s as f64 / 16.0).round() as i32;
            worst = worst.max((box_ - at(&quarter, x, y) as i32).abs());
        }
    }
    assert!(worst <= 3, "1/4 differs from 1/1 by {worst}");
}

/// Test 1268: the four modes are the soft Boolean operations; invert and opacity.
#[test]
fn t1268_modes_invert_and_opacity() {
    let n = 100;
    // A covers x 10..60, B covers x 40..90, both y 10..90
    let a = rect(0.35, 0.5, 0.5, 0.8, 0.0, 0.0);
    let b = Mask { id: 2, name: "b".into(), ..rect(0.65, 0.5, 0.5, 0.8, 0.0, 0.0) };
    let probe = |c: &kt_fx::mask::Coverage| [at(c, 25, 50), at(c, 75, 50), at(c, 50, 50), at(c, 5, 5)];
    for (mode, want) in [
        (MaskMode::Add, [255, 255, 255, 0]),
        (MaskMode::Subtract, [255, 0, 0, 0]),
        (MaskMode::Intersect, [0, 0, 255, 0]),
        (MaskMode::Difference, [255, 255, 0, 0]),
    ] {
        let c = rasterize(&[a.clone(), Mask { mode, ..b.clone() }], &sp(n), n, n);
        assert_eq!(probe(&c), want, "{mode:?}: only A, only B, both, none");
    }
    // a first subtract / intersect starts from the full frame
    let c = rasterize(&[Mask { mode: MaskMode::Subtract, ..b.clone() }], &sp(n), n, n);
    assert_eq!(probe(&c), [255, 0, 0, 255]);
    let c = rasterize(&[Mask { mode: MaskMode::Intersect, ..b.clone() }], &sp(n), n, n);
    assert_eq!(probe(&c), [0, 255, 255, 0]);
    // invert and opacity
    let c = rasterize(&[Mask { invert: true, ..a.clone() }], &sp(n), n, n);
    assert_eq!(probe(&c), [0, 255, 0, 255]);
    let c = rasterize(&[Mask { opacity: 0.5, ..a.clone() }], &sp(n), n, n);
    assert_eq!(probe(&c), [128, 0, 128, 0]);
    let c = rasterize(&[a.clone(), Mask { opacity: 0.5, ..b.clone() }], &sp(n), n, n);
    assert_eq!(probe(&c), [255, 128, 255, 0], "opacity scales the second mask's own share");

    // the apply kernels
    let mut px = vec![10, 20, 30, 200, 10, 20, 30, 255];
    apply_alpha(&mut px, &[255, 128], true);
    assert_eq!(px, [10, 20, 30, 200, 10, 20, 30, 128]);
    let dry = [0, 0, 0, 0, 0, 0, 0, 0];
    let mut wet = vec![255, 255, 255, 255, 255, 255, 255, 255];
    apply_mix(&dry, &mut wet, &[0, 128], true, true);
    assert_eq!(wet, [0, 0, 0, 0, 128, 128, 128, 128]);
}

/// Test 1269: the coverage of a fixed scene is frozen; the memo returns the same bytes.
#[test]
fn t1269_the_coverage_is_deterministic_and_the_memo_is_invisible() {
    let mut a = rect(0.45, 0.5, 0.5, 0.4, 17.0, 0.3);
    a.feather = 6.0;
    let mut b = Mask { id: 2, name: "b".into(), ..ellipse(0.6, 0.45, 0.3, 0.5) };
    b.mode = MaskMode::Difference;
    b.feather = 30.0;
    b.expansion = -4.0;
    b.falloff = Falloff::Linear;
    let c = Mask {
        id: 3,
        name: "c".into(),
        mode: MaskMode::Subtract,
        opacity: 0.7,
        ..Mask::new(
            3,
            "c",
            MaskShape::Path {
                verts: vec![
                    Vertex::corner(0, 0.1, 0.1),
                    Vertex { id: 1, x: 0.4, y: 0.2, ix: -0.05, iy: 0.0, ox: 0.05, oy: 0.1 },
                    Vertex::corner(2, 0.2, 0.4),
                ],
            },
        )
    };
    let masks = vec![a, b, c];
    let space = MaskSpace { ref_w: 320.0, ref_h: 180.0, xf: [0.9, 0.1, 12.0, -0.05, 1.1, -3.0], px: 1.2 };
    let one = rasterize(&masks, &space, 320, 180);
    let two = rasterize(&masks, &space, 320, 180);
    assert_eq!(one, two);
    let memo1 = rasterize_cached(&masks, &space, 320, 180);
    let memo2 = rasterize_cached(&masks, &space, 320, 180);
    assert_eq!(*memo1, one, "the memo changed the bytes");
    assert!(std::sync::Arc::ptr_eq(&memo1, &memo2), "the memo did not hit");
    // FNV-1a of the bytes, frozen
    let h = one.cov.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    let frozen = 0x435b_82b7_d7c8_b693u64;
    assert_eq!(h, frozen, "the coverage bytes moved");
}
