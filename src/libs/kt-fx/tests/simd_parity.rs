//! Test 198 (R.32): the SIMD kernels of the video FX are BYTE-FOR-BYTE equal to the scalar ones.
//!
//! The CPU path is the determinism reference (invariant 11): the goldens t26/t11/t67
//! were not regenerated, so any divergence of SIMD from scalar is
//! a render regression. The test calls BOTH paths explicitly (env is not touched: KT_SIMD
//! is read once per process) and compares the buffers in full.
//!
//! Without AVX2 it skips with a printout; `KT_REQUIRE_SIMD=1` turns the skip into
//! a failure (lesson of t194: vacuous skips hide broken code).

#![cfg(target_arch = "x86_64")]

use kt_fx::simd_testing as st;
use kt_fx::video::{Frame, noise_hash};

/// AVX2 available? Otherwise skip (or fail with KT_REQUIRE_SIMD=1).
fn avx2() -> bool {
    if std::is_x86_feature_detected!("avx2") {
        return true;
    }
    if std::env::var("KT_REQUIRE_SIMD").as_deref() == Ok("1") {
        panic!("KT_REQUIRE_SIMD=1, but AVX2 is not on this CPU");
    }
    eprintln!("skipped: the CPU has no AVX2 — the SIMD path is unreachable");
    false
}

/// Deterministic “evil” frame: counter-hash noise, gradients, extreme
/// 0/255 values and a checkerboard, one pattern per `kind`.
fn frame(w: u32, h: u32, kind: u32) -> Frame {
    let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let px: [u8; 4] = match kind {
                0 => [(x * 4) as u8, (y * 7) as u8, ((x + y) * 3) as u8, 255],
                1 => {
                    let hsh = noise_hash(x ^ noise_hash(y ^ 0x9e37_79b9));
                    hsh.to_le_bytes()
                }
                2 => [0, 0, 0, 0],
                3 => [255, 255, 255, 255],
                _ => {
                    let v = if (x + y) % 2 == 0 { 0 } else { 255 };
                    [v, v, v, if (x + y) % 3 == 0 { 0 } else { 255 }]
                }
            };
            rgba[i..i + 4].copy_from_slice(&px);
        }
    }
    Frame { w, h, rgba, has_alpha: true }
}

/// Micro-proof: the rounding emulation matches `f64::round`
/// over the whole “dangerous” part of the domain (exact halves, neighbours of halves,
/// the classic 0.49999999999999994 and hash-random values).
#[test]
fn t198_round_half_away_matches_scalar() {
    if !avx2() {
        return;
    }
    let mut cases: Vec<f64> = Vec::new();
    // all k*0.5 and their nearest neighbours by ulp, where half-even goes wrong
    for k in 0..=511u32 {
        let base = k as f64 * 0.5;
        cases.push(base);
        for n in 1..=4 {
            let mut up = base;
            let mut dn = base;
            for _ in 0..n {
                up = f64::from_bits(up.to_bits() + 1);
                dn = if dn > 0.0 { f64::from_bits(dn.to_bits() - 1) } else { dn };
            }
            cases.push(up);
            cases.push(dn);
        }
    }
    cases.push(0.499_999_999_999_999_94); // fl(x+0.5) == 1.0, while round(x) == 0
    cases.push(0.0);
    cases.push(255.0);
    // hash-random in [0, 256)
    for i in 0..1_000_000u32 {
        let h = noise_hash(i);
        cases.push((h as f64 / u32::MAX as f64) * 256.0);
    }
    for chunk in cases.chunks(4) {
        let mut v = [0f64; 4];
        v[..chunk.len()].copy_from_slice(chunk);
        let got = st::round_half_away_avx2(v);
        for (i, x) in v.iter().enumerate() {
            assert_eq!(
                got[i],
                x.round(),
                "round({x:?}) = {} for scalar, {} for AVX2 (bit {:#x})",
                x.round(),
                got[i],
                x.to_bits()
            );
        }
    }
}

/// Bilinear: evil frames × evil rects × odd sizes.
#[test]
fn t198_bilinear_scalar_eq_avx2() {
    if !avx2() {
        return;
    }
    // (sw, sh, ow, oh): tails, upscale, downscale, degenerate cases
    let sizes = [
        (1u32, 1u32, 1u32, 1u32),
        (3, 3, 7, 5),
        (7, 5, 3, 3),
        (64, 36, 64, 36),
        (64, 36, 128, 72), // upscale ×2: fx/fy == 0.5 EXACTLY (catches half-even)
        (128, 72, 64, 36),
        (641, 359, 640, 360),
        (37, 23, 641, 359),
    ];
    for kind in 0..5u32 {
        for &(sw, sh, ow, oh) in &sizes {
            let src = frame(sw, sh, kind);
            // rects: identity, crop by zoom, going beyond the bounds (clamps),
            // fractional sizes
            let frames = [
                (0.0, 0.0, sw as f64, sh as f64),
                (0.0, 0.0, sw as f64 / 32.0, sh as f64 / 32.0), // zoom 32
                (sw as f64 * 0.25, sh as f64 * 0.25, sw as f64 * 0.5, sh as f64 * 0.5),
                (-3.5, -2.25, sw as f64 + 7.0, sh as f64 + 5.0), // edge clamps
                (0.3, 0.7, sw as f64 * 0.999, sh as f64 * 1.001),
            ];
            for (fi, &(x0, y0, cw, ch)) in frames.iter().enumerate() {
                let n = (ow as usize) * (oh as usize) * 4;
                let mut a = vec![0u8; n];
                let mut b = vec![0u8; n];
                st::bilinear_scalar(&src, x0, y0, cw, ch, ow, oh, &mut a);
                st::bilinear_avx2(&src, x0, y0, cw, ch, ow, oh, &mut b);
                if a != b {
                    let i = a.iter().zip(&b).position(|(x, y)| x != y).unwrap();
                    panic!(
                        "kind={kind} {sw}x{sh}->{ow}x{oh} frame#{fi}: byte {i} \
                         (pixel {}, channel {}) scalar {} != avx2 {}",
                        i / 4,
                        i % 4,
                        a[i],
                        b[i]
                    );
                }
            }
        }
    }
}

/// Bench (not in the regular run): `cargo test -p kt-fx --release --test
/// simd_parity -- --ignored --nocapture bench`.
#[test]
#[ignore = "bench: cargo test --release -- --ignored --nocapture bench"]
fn bench_simd_1080p() {
    if !avx2() {
        return;
    }
    let src = frame(1920, 1080, 1);
    let (ow, oh) = (1920u32, 1080u32);
    let n = (ow as usize) * (oh as usize) * 4;
    let mut out = vec![0u8; n];
    let iters = 20;
    let run = |f: &dyn Fn(&mut Vec<u8>), out: &mut Vec<u8>| {
        let t = std::time::Instant::now();
        for _ in 0..iters {
            f(out);
        }
        t.elapsed().as_secs_f64() * 1000.0 / iters as f64
    };
    // downscale 1080p→1080p with zoom (a typical Pan)
    let ms_s = run(
        &|o: &mut Vec<u8>| st::bilinear_scalar(&src, 10.0, 5.0, 1600.0, 900.0, ow, oh, o),
        &mut out,
    );
    let ms_a = run(
        &|o: &mut Vec<u8>| st::bilinear_avx2(&src, 10.0, 5.0, 1600.0, 900.0, ow, oh, o),
        &mut out,
    );
    println!("bilinear 1080p: scalar {ms_s:.2} ms, avx2 {ms_a:.2} ms, ×{:.2}", ms_s / ms_a);

    // noise (integer) and glow-additive (f64)
    let pixels = 1920 * 1080;
    let mut nb = src.rgba.clone();
    let t = std::time::Instant::now();
    for _ in 0..iters {
        st::noise_scalar(&mut nb, 1920, 0xdead, 10);
    }
    let ns = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    let mut nb2 = src.rgba.clone();
    let t = std::time::Instant::now();
    for _ in 0..iters {
        st::noise_avx2(&mut nb2, 1920, 1080, 0xdead, 10);
    }
    let na = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    println!("noise 1080p: scalar {ns:.2} ms, avx2 {na:.2} ms, ×{:.2}", ns / na);

    let up = src.rgba.clone();
    let mut gb = src.rgba.clone();
    let t = std::time::Instant::now();
    for _ in 0..iters {
        st::glow_add_scalar(&mut gb, &up, 0.35);
    }
    let gs = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    let mut gb2 = src.rgba.clone();
    let t = std::time::Instant::now();
    for _ in 0..iters {
        st::glow_add_avx2(&mut gb2, &up, 0.35, pixels);
    }
    let ga = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    println!("glow-add 1080p: scalar {gs:.2} ms, avx2 {ga:.2} ms, ×{:.2}", gs / ga);

    // What the kernels that are NOT yet vectorized actually cost (the decision
    // “keep scalar or do it” is made by these numbers, R.32 step 5).
    use kt_fx::video::{VideoFxCtx, apply_video_chain};
    use kt_model::FxInstance;
    let ctx = VideoFxCtx {
        out_w: 1920,
        out_h: 1080,
        u: 0.5,
        burst_index: 90_000,
        cam3d: None,
        in_scale: 1,
        lead: None,
    };
    for (name, fx) in [
        ("greenscreen", FxInstance::greenscreen([0, 255, 0], 70, 25)),
        ("edges", FxInstance::edges(48)),
        ("vhs", FxInstance::vhs(4, 0.4)),
        ("mirror", FxInstance::Mirror { horizontal: true, vertical: false }),
    ] {
        let t = std::time::Instant::now();
        for _ in 0..iters {
            let f = Frame { w: 1920, h: 1080, rgba: src.rgba.clone(), has_alpha: true };
            std::hint::black_box(apply_video_chain(std::slice::from_ref(&fx), f, &ctx));
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
        // subtract the cost of cloning the frame (it is inside the loop)
        let t = std::time::Instant::now();
        for _ in 0..iters {
            std::hint::black_box(src.rgba.clone());
        }
        let clone_ms = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
        println!("{name} 1080p (through the chain, detected path): {:.2} ms", ms - clone_ms);
    }
}

/// noise: an integer kernel, must match BIT FOR BIT (including the
/// division n*k/32767 with truncation toward zero for negative n).
#[test]
fn t198_noise_scalar_eq_avx2() {
    if !avx2() {
        return;
    }
    for kind in 0..5u32 {
        for (w, h) in [(8u32, 4u32), (9, 3), (64, 36), (641, 7), (1, 1)] {
            for k in [1i64, 7, 16, 32] {
                for seed in [0u32, 1, 0xdead_beef, 0x9e37_79b9] {
                    let base = frame(w, h, kind);
                    let mut a = base.rgba.clone();
                    let mut b = base.rgba.clone();
                    st::noise_scalar(&mut a, w, seed, k);
                    if w >= 8 {
                        st::noise_avx2(&mut b, w, h, seed, k);
                    }
                    // the SIMD column tail is not touched, we finish it with scalar
                    let vec_w = if w >= 8 { w & !7 } else { 0 };
                    for y in 0..h {
                        for x in vec_w..w {
                            let i = ((y * w + x) * 4) as usize;
                            let hh =
                                kt_fx::video::noise_hash(x ^ kt_fx::video::noise_hash(y ^ seed));
                            let n = ((hh >> 16) & 0x7fff) as i64 - 16384;
                            let off = (n * k / 32767) as i32;
                            for c in 0..3 {
                                b[i + c] = (b[i + c] as i32 + off).clamp(0, 255) as u8;
                            }
                        }
                    }
                    assert_eq!(a, b, "noise kind={kind} {w}x{h} k={k} seed={seed:#x}");
                }
            }
        }
    }
}

/// glow-additive: f64 mul+add with a cap and TRUNCATION (not round).
#[test]
fn t198_glow_add_scalar_eq_avx2() {
    if !avx2() {
        return;
    }
    for kind in 0..5u32 {
        for pixels in [1usize, 7, 64, 1000] {
            let up = frame(pixels as u32, 1, (kind + 1) % 5).rgba;
            for s in [0.001f64, 0.35, 0.7, 2.0 * 0.35] {
                let base = frame(pixels as u32, 1, kind).rgba;
                let mut a = base.clone();
                let mut b = base.clone();
                st::glow_add_scalar(&mut a, &up, s);
                st::glow_add_avx2(&mut b, &up, s, pixels);
                assert_eq!(a, b, "glow kind={kind} pixels={pixels} s={s}");
            }
        }
    }
}

/// edges: luma + Sobel + threshold. The PUBLIC chain is checked (it goes via the
/// SIMD path on detection) against the scalar reference.
#[test]
fn t198_edges_scalar_eq_simd() {
    if !avx2() {
        return;
    }
    use kt_fx::video::{VideoFxCtx, apply_video_chain};
    use kt_model::FxInstance;
    for kind in 0..5u32 {
        for (w, h) in [(3u32, 3u32), (9, 5), (11, 4), (64, 36), (65, 37), (129, 9)] {
            for th in [1u32, 48, 255] {
                // The AVX2 path of edges exists only on an OPAQUE frame
                // (R.53, item 418: on a frame with alpha the kernel raises it to
                // the line strength and computes with scalar). This test is about SIMD, so
                // the frame is declared opaque too. The RGB bytes do not
                // depend on this: luma reads only RGB.
                let mut src = frame(w, h, kind);
                src.has_alpha = false;
                let want = st::edges_scalar(&src.rgba, w as usize, h as usize, th);
                let ctx = VideoFxCtx {
                    out_w: w,
                    out_h: h,
                    u: 0.5,
                    burst_index: 90_000,
                    cam3d: None,
                    in_scale: 1,
                    lead: None,
                };
                let got = apply_video_chain(&[FxInstance::edges(th)], src, &ctx);
                if got.rgba != want {
                    let i = got.rgba.iter().zip(&want).position(|(a, b)| a != b).unwrap();
                    let p = i / 4;
                    panic!(
                        "edges kind={kind} {w}x{h} th={th}: pixel ({}, {}) channel {}: \
                         scalar {} != simd {}",
                        p % w as usize,
                        p / w as usize,
                        i % 4,
                        want[i],
                        got.rgba[i]
                    );
                }
            }
        }
    }
}

/// vhs: RGB shift with lerp and scan lines. The vector interior plus scalar
/// edges must together give exactly the scalar frame.
#[test]
fn t198_vhs_scalar_eq_simd() {
    if !avx2() {
        return;
    }
    use kt_fx::video::{VideoFxCtx, apply_video_chain};
    use kt_model::FxInstance;
    for kind in 0..5u32 {
        for (w, h) in [(4u32, 4u32), (13, 5), (33, 4), (64, 36), (65, 9)] {
            for shift in [0u32, 1, 4, 12] {
                for bleed in [0.0f64, 0.4, 1.0] {
                    for burst in [0i64, 90_000, 3000 * 5] {
                        let src = frame(w, h, kind);
                        let mut want = src.rgba.clone();
                        st::vhs_scalar(&mut want, w as usize, h as usize, shift, bleed, burst);
                        let ctx = VideoFxCtx {
                            out_w: w,
                            out_h: h,
                            u: 0.5,
                            burst_index: burst,
                            cam3d: None,
                            in_scale: 1,
                            lead: None,
                        };
                        let got = apply_video_chain(&[FxInstance::vhs(shift, bleed)], src, &ctx);
                        if got.rgba != want {
                            let i = got.rgba.iter().zip(&want).position(|(a, b)| a != b).unwrap();
                            let p = i / 4;
                            panic!(
                                "vhs kind={kind} {w}x{h} shift={shift} bleed={bleed} \
                                 burst={burst}: pixel ({}, {}) channel {}: \
                                 scalar {} != simd {}",
                                p % w as usize,
                                p / w as usize,
                                i % 4,
                                want[i],
                                got.rgba[i]
                            );
                        }
                    }
                }
            }
        }
    }
}
