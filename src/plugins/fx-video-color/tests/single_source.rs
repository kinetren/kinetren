//! t1500 (compute-backends, capability `plugins`): Color Grade written once — its CPU form is the
//! shipped kernel byte for byte at every SIMD level and thread count. The benchmark below
//! (`--ignored`) gives the numbers of F-EXT-004; judge them only on a release build.

use fx_video_color::single_source::{color_grade, shipped_frame, tap_args};
use kt_plugin_api::Plugin;
use kt_plugin_api::shade::{Isa, f32x, f64x, run_pointwise};

/// A frame that crosses every byte value on every channel, with an alpha ramp.
fn frame(w: u32, h: u32) -> Vec<u8> {
    let mut v = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let s = x.wrapping_mul(2654435761).wrapping_add(y.wrapping_mul(40503));
            v[i] = (x * 7 + y) as u8;
            v[i + 1] = (s >> 13) as u8;
            v[i + 2] = (x ^ (y * 3)) as u8;
            v[i + 3] = (x + y * 5) as u8;
        }
    }
    v
}

fn cooked(p: &[f64]) -> [f64; 8] {
    let cg = fx_video_color::ColorGrade::new();
    cg.video().unwrap().cook(p)
}

/// The shared text's CPU form as a pixel kernel.
fn kernel(k: &[f64; 8]) -> impl Fn(f64x::vec4f, f64, f64) -> f64x::vec4f + Sync {
    let (a, b) = tap_args(k);
    let a = f64x::vec4f(a[0], a[1], a[2], a[3]);
    let b = f64x::vec4f(b[0], b[1], b[2], b[3]);
    move |v, x, y| color_grade::cpu::tap_color_grade(v, a, b, f64x::vec2f(x, y))
}

const PARAMS: [[f64; 4]; 6] = [
    [0.0, 0.0, 1.0, 0.0],
    [0.12, 0.35, 1.6, 60.0],
    [-0.4, -0.8, 0.0, -180.0],
    [1.0, 1.0, 2.0, 180.0],
    [-1.0, 0.5, 0.3, 37.0],
    [0.05, -0.2, 1.2, 0.0],
];

#[test]
fn t1500_single_text_cpu_form_is_the_shipped_kernel() {
    let (w, h) = (257, 67);
    let src = frame(w, h);
    for p in PARAMS {
        let k = cooked(&p);
        let mut want = src.clone();
        shipped_frame(&k, &mut want);
        for isa in [Isa::Base, Isa::Sse41, Isa::Avx2, Isa::Avx512] {
            for threads in [1, 3, 8] {
                let mut got = src.clone();
                run_pointwise(&mut got, w, threads, isa, &kernel(&k));
                assert!(
                    got == want,
                    "params {p:?}, {isa:?} ×{threads}: the single text left the shipped bytes"
                );
            }
        }
    }
    // the WGSL form is the same tokens: both functions, no Rust-only syntax
    let wgsl = color_grade::WGSL;
    assert!(wgsl.contains("fn cg_hue") && wgsl.contains("fn tap_color_grade"), "{wgsl}");
    for rust_only in ["let mut", "::", "pub ", "as f", "#["] {
        assert!(!wgsl.contains(rust_only), "{rust_only:?} in the WGSL form: {wgsl}");
    }
}

/// The real-f32 form: how far an f32 lane is from the f64 reference (reported, not a contract).
fn f32_diff(k: &[f64; 8], src: &[u8]) -> (usize, u8) {
    let (a, b) = tap_args(k);
    let a = f32x::vec4f(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32);
    let b = f32x::vec4f(b[0] as f32, b[1] as f32, b[2] as f32, b[3] as f32);
    let mut want = src.to_vec();
    shipped_frame(k, &mut want);
    let (mut n, mut max) = (0, 0u8);
    for (px, wpx) in src.chunks_exact(4).zip(want.chunks_exact(4)) {
        let v = f32x::vec4f(px[0] as f32, px[1] as f32, px[2] as f32, px[3] as f32);
        let o = color_grade::cpu_f32::tap_color_grade(v, a, b, f32x::vec2f(0.0, 0.0));
        for (c, wc) in [o.x, o.y, o.z, o.w].into_iter().zip(wpx) {
            let g = c.round().clamp(0.0, 255.0) as u8;
            if g != *wc {
                n += 1;
                max = max.max(g.abs_diff(*wc));
            }
        }
    }
    (n, max)
}

fn best_of(reps: usize, mut f: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..reps {
        let t = std::time::Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64() * 1e3);
    }
    best
}

/// The F-EXT-004 table: ms per frame (best of N) for the shipped scalar kernel and the single
/// text at each SIMD level and thread count, 1080p and 4K. `cargo test --release -p fx-video-color
/// --test single_source -- --ignored --nocapture`.
#[test]
#[ignore]
fn bench_color_grade_cpu() {
    let k = cooked(&[0.12, 0.35, 1.6, 60.0]);
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    println!("best ISA {:?}, {cores} threads available", Isa::best());
    for (name, w, h) in [("1080p", 1920u32, 1080u32), ("4K", 3840, 2160)] {
        let src = frame(w, h);
        let mut buf = src.clone();
        let reps = if h > 1500 { 8 } else { 15 };
        let t = best_of(reps, || {
            buf.copy_from_slice(&src);
            shipped_frame(&k, &mut buf);
        });
        let copy = best_of(reps, || buf.copy_from_slice(&src));
        println!("{name}: copy {copy:.2} ms; shipped scalar {:.2} ms", t - copy);
        for isa in [Isa::Base, Isa::Sse41, Isa::Avx2, Isa::Avx512] {
            if !isa.available() {
                println!("{name}: {isa:?} not on this CPU");
                continue;
            }
            let mut row = format!("{name}: single text {isa:?}");
            for threads in [1usize, 2, 4, 8, 16, cores] {
                let t = best_of(reps, || {
                    buf.copy_from_slice(&src);
                    run_pointwise(&mut buf, w, threads, isa, &kernel(&k));
                });
                row += &format!(" | ×{threads} {:.2}", t - copy);
            }
            println!("{row}");
        }
        let (n, max) = f32_diff(&k, &src);
        println!(
            "{name}: real-f32 form vs the f64 reference: {n} channels differ of {}, max {max}",
            src.len()
        );
    }
}
