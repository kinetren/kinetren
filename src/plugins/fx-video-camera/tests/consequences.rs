//! camera_isp's consequences (camera-isp-simulation; capability `video-fx`): the CPU kernel, the byte
//! reference, checked against what the physics and the ISP predict — not against its own output.

use fx_video_camera::cpu::{plane_after, srgb_lin};
use fx_video_camera::{CameraIsp, Plan, cfa, ix, pass};
use kt_plugin_api::barrier::{BarrierCtx, BarrierKernel, Frame, RasterHost, Scratch};
use kt_plugin_api::{ParamValues, Plugin};

struct NoRaster;
impl RasterHost for NoRaster {
    fn resample(&self, _: &Frame, _: u32, _: u32, _: &mut dyn Scratch) -> Frame {
        unreachable!("camera_isp uses no host raster")
    }
    fn sample(&self, _: &Frame, _: f64, _: f64) -> [f64; 4] {
        unreachable!()
    }
    fn premultiply(&self, _: &mut [u8]) {}
    fn unpremultiply(&self, _: &mut [u8]) {}
}

#[derive(Default)]
struct Pool;
impl Scratch for Pool {
    fn take(&mut self, len: usize) -> Vec<u8> {
        vec![0; len]
    }
    fn put(&mut self, _: Vec<u8>) {}
}

fn values(sets: &[(&str, f64)]) -> Vec<f64> {
    let cam = CameraIsp::new();
    let pp = &cam.manifest().passport;
    let mut v = ParamValues::defaults(pp).0;
    for (n, x) in sets {
        let i = pp.iter().position(|s| s.name == *n).unwrap_or_else(|| panic!("no row {n}"));
        v[i] = *x;
    }
    v
}

fn run_with(vals: &[f64], f: &Frame, k: u32, burst: i64, simd: bool) -> Vec<u8> {
    let cam = CameraIsp::new();
    let cx = BarrierCtx { res_scale: k, burst_index: burst, simd, raster: &NoRaster };
    let f = Frame { w: f.w, h: f.h, rgba: f.rgba.clone(), has_alpha: f.has_alpha };
    cam.frame_values(vals, f, &cx, &mut Pool).rgba
}

fn run(vals: &[f64], f: &Frame) -> Vec<u8> {
    run_with(vals, f, 1, 90_000, false)
}

fn frame_of(w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 3]) -> Frame {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let [r, g, b] = px(x, y);
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Frame { w, h, rgba, has_alpha: false }
}

/// The chain with nothing that adds error but its own sampling: no temporal noise, no fixed pattern,
/// no defects, flat optics, calibrated WB and CCM, sRGB, no NR or sharpening, 4:4:4; exposure kept
/// under the RGBW white pixel's clip, a 14-bit ADC.
fn clean() -> Vec<(&'static str, f64)> {
    vec![
        ("optics", 0.0),
        ("vignetting", 0.0),
        ("noise", 0.0),
        ("dark_current", 0.0),
        ("dsnu", 0.0),
        ("prnu", 0.0),
        ("hot_pixels", 0.0),
        ("dead_pixels", 0.0),
        ("exposure_ev", -1.5),
        ("digital_gain_ev", 1.5),
        ("adc_bits", 14.0),
        ("ir_sub", 0.0),
        ("denoise", 0.0),
        ("sharpen", 0.0),
        ("chroma", 0.0),
        ("row_noise", 0.0),
        ("column_fpn", 0.0),
        ("raw_nr", 0.0),
        ("fcs", 0.0),
        ("purple", 0.0),
    ]
}

fn with(
    mut base: Vec<(&'static str, f64)>,
    more: &[(&'static str, f64)],
) -> Vec<(&'static str, f64)> {
    base.extend_from_slice(more);
    base
}

/// t1211: a clean chain reproduces the input — every layout, every demosaic.
#[test]
fn t1211_a_clean_chain_reproduces_the_input() {
    let (w, h) = (64u32, 48u32);
    let smooth =
        frame_of(w, h, |x, y| [(40 + x * 3) as u8, (30 + y * 4) as u8, (150 + x - y) as u8]);
    let flat = frame_of(w, h, |x, y| match (x < w / 2, y < h / 2) {
        (true, true) => [200, 60, 40],
        (false, true) => [50, 180, 90],
        (true, false) => [70, 80, 210],
        (false, false) => [128, 128, 128],
    });
    for layout in 0..cfa::LAYOUTS.len() {
        for method in 0..3 {
            let vals =
                values(&with(clean(), &[("cfa", layout as f64), ("demosaic", method as f64)]));
            let what = format!("{} / {}", cfa::LAYOUTS[layout], fx_video_camera::DEMOSAICS[method]);
            let out = run(&vals, &smooth);
            let (mut sum, mut n, mut worst) = (0u64, 0u64, 0i32);
            for y in 3..h - 3 {
                for x in 3..w - 3 {
                    for c in 0..3 {
                        let i = ((y * w + x) * 4 + c) as usize;
                        let d = (out[i] as i32 - smooth.rgba[i] as i32).abs();
                        sum += d as u64;
                        n += 1;
                        worst = worst.max(d);
                    }
                }
            }
            let mean = sum as f64 / n as f64;
            eprintln!("{what}: mean {mean:.2} worst {worst}");
            // a 2×2 Bayer has the exact kernels; the other mosaics a generic interpolation
            let (mean_max, worst_max) = if layout < 4 { (1.0, 3) } else { (2.0, 12) };
            assert!(mean <= mean_max, "{what}: mean |Δ| {mean:.2} on a smooth frame");
            assert!(worst <= worst_max, "{what}: worst |Δ| {worst} on a smooth frame");
            let out = run(&vals, &flat);
            for y in (4..h / 2 - 4).chain(h / 2 + 4..h - 4) {
                for x in (4..w / 2 - 4).chain(w / 2 + 4..w - 4) {
                    for c in 0..3 {
                        let i = ((y * w + x) * 4 + c) as usize;
                        let d = (out[i] as i32 - flat.rgba[i] as i32).abs();
                        assert!(d <= 1, "{what}: a flat patch moved by {d} at ({x},{y}) ch {c}");
                    }
                }
            }
        }
    }
}

/// t1212: shot-noise variance ∝ signal: Var(DN)/(mean − black) is the conversion gain at every level;
/// with read noise σ the variance adds σ²·g².
#[test]
fn t1212_shot_noise_variance_grows_with_the_signal() {
    let (w, h) = (128u32, 128u32);
    let base = with(
        clean(),
        &[
            ("noise", 1.0),
            ("read_noise", 0.0),
            ("adc_bits", 14.0),
            ("exposure_ev", 0.0),
            ("digital_gain_ev", 0.0),
        ],
    );
    let measure = |level: u8, extra: &[(&'static str, f64)]| -> (f64, f64, f64) {
        let f = frame_of(w, h, |_, _| [level, level, level]);
        let vals = values(&with(base.clone(), extra));
        let plan = Plan::new(&vals, 1, 90_000);
        let raw = plane_after(&plan, &f, pass::READOUT);
        let mut xs = Vec::new();
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                if cfa::code(&plan.sensor_tab, plan.period, x, y) == cfa::G {
                    let t = raw[(y as u32 * w + x as u32) as usize];
                    xs.push(((t & 0xFF) << 8 | (t >> 8) & 0xFF) as f64);
                }
            }
        }
        let m = xs.iter().sum::<f64>() / xs.len() as f64;
        let var = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (xs.len() - 1) as f64;
        (m - plan.black as f64, var, plan.cg as f64)
    };
    for level in [30u8, 80, 150, 230] {
        let (signal, var, g) = measure(level, &[]);
        // Var(DN) = g²·λ + 1/12 (the ADC's rounding); signal = g·λ
        let ratio = (var - 1.0 / 12.0) / signal;
        assert!(
            (ratio / g - 1.0).abs() < 0.10,
            "level {level}: Var/signal {ratio:.4} against the gain {g:.4}"
        );
    }
    let sigma = 20.0;
    let (signal, var, g) = measure(30, &[("read_noise", sigma)]);
    let want = g * signal + sigma * sigma * g * g + 1.0 / 12.0;
    assert!((var / want - 1.0).abs() < 0.10, "read noise: Var {var:.2} against {want:.2}");
}

fn patches() -> Frame {
    frame_of(48, 16, |x, _| match x / 16 {
        0 => [230, 20, 20],
        1 => [20, 230, 20],
        _ => [20, 20, 230],
    })
}

fn patch_mean(out: &[u8], i: u32) -> [f64; 3] {
    let mut s = [0.0; 3];
    let mut n = 0.0;
    for y in 4..12u32 {
        for x in i * 16 + 4..i * 16 + 12 {
            for c in 0..3 {
                s[c] += out[((y * 48 + x) * 4 + c as u32) as usize] as f64;
            }
            n += 1.0;
        }
    }
    s.map(|v| v / n)
}

/// t1213: the ISP that assumes the wrong CFA phase swaps colours exactly as the mosaic predicts.
#[test]
fn t1213_a_wrong_cfa_phase_swaps_colours_as_predicted() {
    let f = patches();
    let base = with(clean(), &[("cfa", 0.0), ("demosaic", 0.0), ("crosstalk", 0.0), ("ccm", 0.0)]);
    let right = run(&values(&base), &f);
    let [r, g, b] = patch_mean(&right, 0);
    assert!(
        r > 180.0 && g < 60.0 && b < 60.0,
        "the exact phase: red is red ({r:.0},{g:.0},{b:.0})"
    );
    // BGGR assumed: R and B sites trade places, G stays
    let xy = run(&values(&with(base.clone(), &[("isp_phase", 3.0)])), &f);
    let [r, g, b] = patch_mean(&xy, 0);
    assert!(
        b > 180.0 && r < 60.0 && g < 60.0,
        "shift x,y: red should come out blue ({r:.0},{g:.0},{b:.0})"
    );
    let [r, g, b] = patch_mean(&xy, 2);
    assert!(
        r > 180.0 && b < 60.0 && g < 60.0,
        "shift x,y: blue should come out red ({r:.0},{g:.0},{b:.0})"
    );
    let [r, g, b] = patch_mean(&xy, 1);
    assert!(
        g > 180.0 && r < 60.0 && b < 60.0,
        "shift x,y: green stays green ({r:.0},{g:.0},{b:.0})"
    );
    // GRBG assumed: the ISP's R and B sites are real G, its G sites real R and B
    let x = run(&values(&with(base, &[("isp_phase", 1.0)])), &f);
    let [r, g, b] = patch_mean(&x, 1);
    assert!(
        r > 180.0 && b > 180.0 && g < 60.0,
        "shift x: green should come out magenta ({r:.0},{g:.0},{b:.0})"
    );
    for i in [0, 2] {
        let [r, g, b] = patch_mean(&x, i);
        assert!(
            g > r + 40.0 && g > b + 40.0,
            "shift x: patch {i} should come out green ({r:.0},{g:.0},{b:.0})"
        );
    }
}

/// t1214: without the IR-cut filter the dyes' NIR leak, amplified by the visible WB, tints grey
/// magenta by exactly `qe_ir·T_c·IR/qe_c`; RGB-IR subtraction brings it back to neutral.
#[test]
fn t1214_an_infrared_leak_tints_as_predicted() {
    let f = frame_of(32, 32, |_, _| [128, 128, 128]);
    let s = 2.0;
    let base = with(
        clean(),
        &[
            ("tone", 0.0),
            ("ccm", 0.0),
            ("crosstalk", 0.0),
            ("scene_ir", s),
            ("exposure_ev", -1.5),
            ("digital_gain_ev", 1.5),
            ("yuv", 0.0),
        ],
    );
    let g0 = srgb_lin(128) as f64;
    let ir = s * g0; // the NIR estimate's weights sum to 1
    let (qe, t, qe_ir) = ([0.42, 0.55, 0.38], [1.0, 0.9, 0.95], 0.3);
    let mid = |out: &[u8]| {
        let i = ((16 * 32 + 16) * 4) as usize;
        [out[i] as f64, out[i + 1] as f64, out[i + 2] as f64]
    };
    let leak = mid(&run(&values(&with(base.clone(), &[("ir_cut", 0.0)])), &f));
    for c in 0..3 {
        let want = (g0 + qe_ir * t[c] * ir / qe[c]) * 255.0;
        assert!(
            (leak[c] / want - 1.0).abs() < 0.03,
            "channel {c}: {:.1} against the predicted {want:.1}",
            leak[c]
        );
    }
    assert!(leak[0] > leak[1] + 10.0 && leak[2] > leak[1] + 10.0, "no magenta cast: {leak:?}");
    let cut = mid(&run(&values(&with(base.clone(), &[("ir_cut", 1.0)])), &f));
    for c in 0..3 {
        assert!(
            (cut[c] - g0 * 255.0).abs() <= 1.5,
            "the IR-cut filter in: channel {c} {:.1} is not grey",
            cut[c]
        );
    }
    let rgbir =
        mid(&run(&values(&with(base, &[("ir_cut", 0.0), ("cfa", 7.0), ("ir_sub", 1.0)])), &f));
    let spread = rgbir.iter().cloned().fold(f64::MIN, f64::max)
        - rgbir.iter().cloned().fold(f64::MAX, f64::min);
    assert!(spread <= 3.0, "RGB-IR with subtraction is not neutral: {rgbir:?}");
}

/// t1215: the bilinear demosaic zips a vertical edge (a two-row colour alternation); the
/// edge-directed one interpolates the colour difference and halves it at least.
#[test]
fn t1215_the_demosaic_zipper_on_a_known_edge() {
    let (w, h) = (64u32, 32u32);
    let f = frame_of(w, h, |x, _| if x < 31 { [10, 10, 10] } else { [240, 240, 240] });
    let zip = |method: f64| -> f64 {
        let out = run(
            &values(&with(clean(), &[("cfa", 0.0), ("crosstalk", 0.0), ("demosaic", method)])),
            &f,
        );
        let chroma = |x: u32, y: u32| {
            let i = ((y * w + x) * 4) as usize;
            (out[i] as f64 - out[i + 1] as f64) + (out[i + 2] as f64 - out[i + 1] as f64)
        };
        let mut worst = 0.0f64;
        for x in 28..34 {
            let mut s = 0.0;
            for y in 4..h - 5 {
                s += (chroma(x, y) - chroma(x, y + 1)).abs();
            }
            worst = worst.max(s / (h - 9) as f64);
        }
        worst
    };
    let (bil, mhc, edge) = (zip(0.0), zip(1.0), zip(2.0));
    assert!(bil > 20.0, "bilinear shows no zipper: {bil:.1}");
    assert!(edge <= bil * 0.5, "edge-directed {edge:.1} against bilinear {bil:.1}");
    eprintln!("zipper: bilinear {bil:.1}, MHC {mhc:.1}, edge-directed {edge:.1}");
}

/// t1216: the noise is deterministic; the fixed pattern does not move with time; SIMD = scalar; K=2
/// is the K=1 sensor magnified.
#[test]
fn t1216_the_noise_is_deterministic_and_the_pattern_is_fixed() {
    let night = CameraIsp::new().manifest().ui.variants[0];
    let mut sets: Vec<(&str, f64)> = night.sets.to_vec();
    sets.push(("lateral_ca", 0.0));
    let vals = values(&sets);
    let (w, h) = (61u32, 37u32);
    let f = frame_of(w, h, |x, y| [(x * 4) as u8, (y * 6) as u8, ((x + y) * 2) as u8]);
    let a = run_with(&vals, &f, 1, 90_000, false);
    assert_eq!(a, run_with(&vals, &f, 1, 90_000, false), "the same moment gave other bytes");
    assert_ne!(
        a,
        run_with(&vals, &f, 1, 93_000, false),
        "another moment gave the same temporal noise"
    );
    if std::is_x86_feature_detected!("avx2") {
        assert_eq!(a, run_with(&vals, &f, 1, 90_000, true), "the AVX2 twin moved a byte");
    }
    // the fixed pattern: with temporal noise off, two moments give the same raw
    let mut still = sets.clone();
    still.push(("noise", 0.0));
    let sv = values(&still);
    let r1 = plane_after(&Plan::new(&sv, 1, 90_000), &f, pass::READOUT);
    let r2 = plane_after(&Plan::new(&sv, 1, 93_000), &f, pass::READOUT);
    assert_eq!(r1, r2, "the fixed pattern moved with time");
    let big = frame_of(200, 150, |_, _| [20, 20, 20]);
    let hot = plane_after(&Plan::new(&sv, 1, 90_000), &big, pass::READOUT)
        .iter()
        .filter(|t| ((*t & 0xFF) << 8 | (*t >> 8) & 0xFF) > 2000)
        .count();
    // 300 per million of 30 000 pixels: about nine
    assert!((2..=30).contains(&hot), "the warm sensor has {hot} hot pixels on 200×150");
    // K=2: the 2× frame of the same picture gives the K=1 result magnified (the OLPF and the
    // distortion off: they sample between logical pixels, which a finer frame legitimately resolves)
    let mut k_sets = sets.clone();
    k_sets.push(("olpf", 0.0));
    k_sets.push(("distortion", 0.0));
    let kv = values(&k_sets);
    let one = run_with(&kv, &f, 1, 90_000, false);
    let f2 = frame_of(w * 2, h * 2, |x, y| {
        let i = (((y / 2) * w + x / 2) * 4) as usize;
        [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2]]
    });
    let two = run_with(&kv, &f2, 2, 90_000, false);
    for y in 0..h * 2 {
        for x in 0..w * 2 {
            let i = ((y * w * 2 + x) * 4) as usize;
            let j = (((y / 2) * w + x / 2) * 4) as usize;
            assert_eq!(
                two[i..i + 4],
                one[j..j + 4],
                "K=2 at ({x},{y}) is not the K=1 sensor pixel"
            );
        }
    }
}

/// t1218: each layout is its camera's mosaic, the row constants are the passport, the presets set
/// only rows within their ranges.
#[test]
fn t1218_each_layout_is_the_cameras_mosaic() {
    let frac = |i: usize, code: u32| {
        let l = cfa::layout(i);
        l.cells.iter().filter(|c| **c == code).count() as f64 / l.cells.len() as f64
    };
    for i in 0..4 {
        assert_eq!(cfa::layout(i).period, 2);
        assert_eq!(
            (frac(i, cfa::R), frac(i, cfa::G), frac(i, cfa::B)),
            (0.25, 0.5, 0.25),
            "{}",
            cfa::LAYOUTS[i]
        );
    }
    let quad = cfa::layout(4);
    assert_eq!((quad.period, frac(4, cfa::G)), (4, 0.5));
    for y in 0..4 {
        for x in 0..4 {
            let c = quad.cells[y * 4 + x];
            assert_eq!(c, quad.cells[(y & !1) * 4 + (x & !1)], "Quad Bayer is 2×2 blocks");
        }
    }
    assert_eq!(cfa::layout(5).period, 6);
    assert_eq!(
        (frac(5, cfa::G), frac(5, cfa::R), frac(5, cfa::B)),
        (20.0 / 36.0, 8.0 / 36.0, 8.0 / 36.0)
    );
    assert_eq!((frac(6, cfa::W), cfa::layout(6).period), (0.5, 4));
    assert_eq!((frac(7, cfa::IR), frac(7, cfa::G), cfa::layout(7).period), (0.25, 0.5, 4));
    // a phase shift of the packed table is the table read one pixel off
    let l = cfa::layout(0);
    let shifted = cfa::pack(&l, 1, 1);
    assert_eq!(cfa::code(&shifted, 2, 0, 0), cfa::B, "RGGB shifted by (1,1) reads as BGGR");

    let cam = CameraIsp::new();
    let m = cam.manifest();
    // `ix` finds every row by its name, in passport order
    for (i, s) in m.passport.iter().enumerate() {
        assert_eq!(ix(&s.name), i, "row {}", s.name);
    }
    for v in m.ui.variants {
        for (n, x) in v.sets {
            let s = m
                .passport
                .iter()
                .find(|s| s.name == *n)
                .unwrap_or_else(|| panic!("{}: no row {n}", v.name_key));
            assert!(
                *x >= s.min && *x <= s.max,
                "{}: {n} = {x} outside {}…{}",
                v.name_key,
                s.min,
                s.max
            );
        }
    }
    assert_eq!(m.ui.variants.len(), 6);
}

/// The mean |Δ| over the RGB of two frames, away from a border of `m` pixels.
fn mean_diff(a: &[u8], b: &[u8], w: u32, h: u32, m: u32) -> f64 {
    let (mut s, mut n) = (0u64, 0u64);
    for y in m..h - m {
        for x in m..w - m {
            for c in 0..3 {
                let i = ((y * w + x) * 4 + c) as usize;
                s += (a[i] as i32 - b[i] as i32).unsigned_abs() as u64;
                n += 1;
            }
        }
    }
    s as f64 / n as f64
}

/// A test card: a colour checker of 12-px squares with 3-px light bars (a geometric defect moves
/// them; two bilinear resamplings soften but do not move them).
fn card(w: u32, h: u32) -> Frame {
    frame_of(w, h, |x, y| {
        if x % 24 < 3 || y % 24 < 3 {
            return [230, 230, 230];
        }
        match (x / 12 + y / 12 * 3) % 6 {
            0 => [200, 60, 50],
            1 => [60, 170, 70],
            2 => [50, 70, 190],
            3 => [210, 200, 60],
            4 => [120, 120, 120],
            _ => [40, 40, 40],
        }
    })
}

/// t1220: every defect of the lens and the sensor has the ISP block that removes it. With the block
/// calibrated the picture returns to the camera without the defect; with the block off the defect
/// stays.
#[test]
fn t1220_every_defect_has_its_correcting_block() {
    let (w, h) = (96u32, 64u32);
    // a smooth pattern (period 24 px): a geometric defect moves it, a bilinear resampling barely
    // softens it — so the comparison measures the defect, not the interpolation
    let f = frame_of(w, h, |x, y| {
        use kt_plugin_api::math::FrozenMath;
        let a = (x as f64 * std::f64::consts::TAU / 24.0).fsin();
        let b = (y as f64 * std::f64::consts::TAU / 24.0).fcos();
        let q = |c: f64| (128.0 + 90.0 * c).round() as u8;
        [q(a * b), q(0.8 * a), q(-0.8 * b)]
    });
    // the reference camera: the lens on but ideal (no OLPF, no shading), so a pair differs from it
    // only by its defect
    let base = with(
        clean(),
        &[("cfa", 0.0), ("demosaic", 2.0), ("vignetting", 0.0), ("optics", 1.0), ("olpf", 0.0)],
    );
    let reference = run(&values(&base), &f);
    // (what, the defect, the block switched off)
    let pairs: &[(&str, &[(&str, f64)], &str)] = &[
        ("distortion ↔ dewarp", &[("optics", 1.0), ("distortion", 0.2)], "dewarp"),
        ("lateral CA ↔ CA correction", &[("optics", 1.0), ("lateral_ca", 3.0)], "ca_corr"),
        ("vignetting ↔ lens shading", &[("optics", 1.0), ("vignetting", 1.5)], "lsc"),
        (
            "colour shading ↔ colour lens shading",
            &[("optics", 1.0), ("color_shading", 0.4)],
            "color_lsc",
        ),
        ("Gr/Gb imbalance ↔ green equalisation", &[("green_imbalance", 0.06)], "geq"),
        ("column fixed pattern ↔ optical-black correction", &[("column_fpn", 20.0)], "rowcol"),
    ];
    for (what, defect, block) in pairs {
        let mut on = base.clone();
        on.extend_from_slice(defect);
        let corrected = mean_diff(&run(&values(&on), &f), &reference, w, h, 6);
        on.push((block, 0.0));
        let raw = mean_diff(&run(&values(&on), &f), &reference, w, h, 6);
        eprintln!("{what}: with the block {corrected:.2}, without {raw:.2}");
        assert!(raw > 0.8, "{what}: the defect is invisible without its block ({raw:.2})");
        assert!(
            corrected < raw * 0.5,
            "{what}: the block removes too little ({corrected:.2} against {raw:.2})"
        );
    }
    // hot pixels are points, not a mean: count the pixels a hot site lights up
    let lit = |vals: &[(&'static str, f64)]| {
        let out = run(&values(vals), &f);
        (0..(w * h) as usize)
            .filter(|i| {
                (0..3).any(|c| (out[i * 4 + c] as i32 - reference[i * 4 + c] as i32).abs() > 40)
            })
            .count()
    };
    let hot = with(base.clone(), &[("hot_pixels", 2000.0)]);
    let (with_dpc, without) = (lit(&hot), lit(&with(hot.clone(), &[("dpc", 0.0)])));
    eprintln!(
        "hot pixels ↔ defect-pixel correction: lit with the block {with_dpc}, without {without}"
    );
    assert!(
        without >= 5 && with_dpc * 4 <= without,
        "hot pixels: {with_dpc} lit with DPC, {without} without"
    );
}

/// t1221: the view shows the picture after any stage: the scene of a clean source is the input, the
/// raw is a mosaic in its filter colours, every other stage is a picture of its own.
#[test]
fn t1221_the_view_shows_any_stage() {
    let (w, h) = (48u32, 32u32);
    let f = frame_of(w, h, |_, _| [128, 128, 128]);
    let vals = |view: usize| values(&with(clean(), &[("view", view as f64)]));
    let scene = run(&vals(1), &f);
    assert!(mean_diff(&scene, &f.rgba, w, h, 0) <= 1.0, "the scene of an sRGB source is the input");
    let raw = run(&vals(4), &f);
    // RGGB: the top-left pixel is red only, its right neighbour green only, the diagonal blue only
    let px = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        [raw[i], raw[i + 1], raw[i + 2]]
    };
    let (r, g, b) = (px(10, 10), px(11, 10), px(11, 11));
    assert!(r[0] > 60 && r[1] == 0 && r[2] == 0, "a red site shows red: {r:?}");
    assert!(g[1] > 60 && g[0] == 0 && g[2] == 0, "a green site shows green: {g:?}");
    assert!(b[2] > 60 && b[0] == 0 && b[1] == 0, "a blue site shows blue: {b:?}");
    let out = run(&vals(0), &f);
    for view in 1..fx_video_camera::VIEWS.len() {
        let v = run(&vals(view), &f);
        assert_eq!(v.len(), out.len());
        if (2..=7).contains(&view) {
            assert!(
                mean_diff(&v, &out, w, h, 0) > 5.0,
                "view {} looks like the output",
                fx_video_camera::VIEWS[view]
            );
        }
    }
}

/// t1222: the frame is unprocessed first — a frame developed with more saturation and a warmer white
/// balance, unprocessed with exactly those settings, films as the plain frame does.
#[test]
fn t1222_the_frame_is_unprocessed_into_the_scene() {
    let (w, h) = (48u32, 32u32);
    // muted colours: a developed picture that clipped has lost what no unprocessing can return
    let plain = frame_of(w, h, |x, y| {
        [(100 + x % 40) as u8, (110 + y % 30) as u8, (120 - (x + y) % 35) as u8]
    });
    let (s, wr, wb) = (1.4f32, 1.2f32, 0.85f32);
    let wl = [0.2126f32, 0.7152, 0.0722];
    let enc =
        |v: f32| (fx_video_camera::cpu::oetf(v, 1) * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8;
    // develop: linear → white balance → saturation s → sRGB
    let developed = frame_of(w, h, |x, y| {
        let i = ((y * w + x) * 4) as usize;
        let l = [
            srgb_lin(plain.rgba[i] as u32) * wr,
            srgb_lin(plain.rgba[i + 1] as u32),
            srgb_lin(plain.rgba[i + 2] as u32) * wb,
        ];
        let yy = wl[0] * l[0] + wl[1] * l[1] + wl[2] * l[2];
        let sat = |c: f32| s * c + (1.0 - s) * yy;
        [enc(sat(l[0])), enc(sat(l[1])), enc(sat(l[2]))]
    });
    let a = run(&values(&with(clean(), &[("view", 1.0)])), &plain);
    let undone = values(&with(
        clean(),
        &[
            ("view", 1.0),
            ("src_saturation", s as f64),
            ("src_wb_r", wr as f64),
            ("src_wb_b", wb as f64),
        ],
    ));
    let b = run(&undone, &developed);
    let kept = run(&values(&with(clean(), &[("view", 1.0)])), &developed);
    let (d_undone, d_kept) = (mean_diff(&a, &b, w, h, 0), mean_diff(&a, &kept, w, h, 0));
    assert!(d_undone < 2.5, "the unprocessed scene differs from the plain one by {d_undone:.2}");
    assert!(
        d_kept > 3.0 * d_undone,
        "without unprocessing the scene keeps the source's look ({d_kept:.2})"
    );
}

/// t1223: raw NR follows the sensor's noise model — on a flat field it removes most of the noise, on
/// an edge it keeps the edge; blooming spills the charge above the well into the neighbours.
#[test]
fn t1223_raw_nr_follows_the_noise_model_and_blooming_spills() {
    let (w, h) = (64u32, 64u32);
    let f = frame_of(w, h, |x, _| if x < 32 { [70, 70, 70] } else { [170, 170, 170] });
    let noisy = with(
        clean(),
        &[("noise", 1.0), ("read_noise", 6.0), ("full_well", 3000.0), ("raw_nr", 0.0)],
    );
    let var_of = |out: &[u8], x0: u32| {
        let xs: Vec<f64> = (8..56)
            .flat_map(|y| (x0..x0 + 16).map(move |x| (y, x)))
            .map(|(y, x)| out[((y * w + x) * 4 + 1) as usize] as f64)
            .collect();
        let m = xs.iter().sum::<f64>() / xs.len() as f64;
        xs.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / xs.len() as f64
    };
    let off = run(&values(&noisy), &f);
    let mut nr = noisy.clone();
    nr.push(("raw_nr", 1.0));
    nr.push(("raw_nr_strength", 2.0));
    let on = run(&values(&nr), &f);
    assert!(
        var_of(&on, 8) < 0.5 * var_of(&off, 8),
        "raw NR left the flat noise: {} → {}",
        var_of(&off, 8),
        var_of(&on, 8)
    );
    let step = |out: &[u8]| {
        out[((32 * w + 40) * 4 + 1) as usize] as f64 - out[((32 * w + 24) * 4 + 1) as usize] as f64
    };
    assert!(
        (step(&on) - step(&off)).abs() < 12.0,
        "raw NR blurred the edge: {} → {}",
        step(&off),
        step(&on)
    );

    // blooming: one overexposed spot; the raw of its neighbours rises with the spill
    let spot =
        frame_of(32, 32, |x, y| if x == 16 && y == 16 { [255, 255, 255] } else { [20, 20, 20] });
    let raw_at = |bloom: f64| {
        let vals =
            values(&with(clean(), &[("exposure_ev", 4.0), ("blooming", bloom), ("olpf", 0.0)]));
        let plan = Plan::new(&vals, 1, 90_000);
        let raw = plane_after(&plan, &spot, pass::READOUT);
        let t = raw[(16 * 32 + 17) as usize];
        ((t & 0xFF) << 8 | (t >> 8) & 0xFF) as i64
    };
    assert!(
        raw_at(1.0) > raw_at(0.0) + 200,
        "blooming did not spill: {} → {}",
        raw_at(0.0),
        raw_at(1.0)
    );
}
