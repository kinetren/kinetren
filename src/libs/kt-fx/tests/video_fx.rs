//! Тест 26 (docs/porting-notes.md, п.25): golden-тесты 10 видео-FX.
//! Синтетический кадр детерминирован; хэш фиксирует поведение, свойства
//! ловят смысловые поломки (агенты обязаны гонять при правках kt-fx).

use kt_fx::video::{Frame, VideoFxCtx, apply_video_chain};
use kt_model::{CamKeyframe, FxInstance, PanMode};
use xxhash_rust::xxh3::xxh3_64;

const W: u32 = 64;
const H: u32 = 36;

fn ctx() -> VideoFxCtx {
    VideoFxCtx { out_w: W, out_h: H, u: 0.5, burst_index: 90_000 }
}

/// Детерминированный кадр: градиенты + «зелёный экран» в правом нижнем углу.
fn test_frame() -> Frame {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            if x > W / 2 && y > H / 2 {
                rgba[i] = 0;
                rgba[i + 1] = 255;
                rgba[i + 2] = 0;
            } else {
                rgba[i] = (x * 4) as u8;
                rgba[i + 1] = (y * 7) as u8;
                rgba[i + 2] = ((x + y) * 3) as u8;
            }
            rgba[i + 3] = 255;
        }
    }
    Frame { w: W, h: H, rgba, has_alpha: false }
}

fn run(fx: FxInstance) -> Frame {
    apply_video_chain(&[fx], test_frame(), &ctx())
}

fn mean_rgb(f: &Frame) -> f64 {
    let s: u64 = f.rgba.chunks_exact(4).map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64).sum();
    s as f64 / (f.rgba.len() as f64 * 3.0 / 4.0)
}

/// Golden-хэши: фиксируют текущее поведение всех 10 эффектов. Менять
/// эталон можно ТОЛЬКО при осознанном изменении алгоритма.
#[test]
fn t26_golden_hashes_all_effects() {
    let cases: Vec<(&str, FxInstance)> = vec![
        ("pan", FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.3, pan_y: -0.2, zoom: 1.5 }),
        (
            "cammotion",
            FxInstance::CamMotion {
                mode: PanMode::Fit,
                ease: true,
                time_scale: 1.0,
                keyframes: vec![
                    CamKeyframe { t: 0.0, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 },
                    CamKeyframe { t: 1.0, pan_x: 0.5, pan_y: 0.0, zoom: 2.0 },
                ],
            },
        ),
        ("mirror", FxInstance::Mirror { horizontal: true, vertical: false }),
        ("blur", FxInstance::Blur { radius: 8 }),
        (
            "greenscreen",
            FxInstance::Greenscreen { key_r: 0, key_g: 255, key_b: 0, tolerance: 70, softness: 25 },
        ),
        ("edges", FxInstance::Edges { threshold: 48 }),
        ("noise", FxInstance::Noise { amount: 0.3 }),
        ("vhs", FxInstance::Vhs { rgb_shift: 4, bleed: 0.4 }),
        ("glow", FxInstance::Glow { strength: 0.65 }),
        ("eventgrid", FxInstance::EventGrid { cols: 2, rows: 2, center: true }),
    ];
    let mut lines = Vec::new();
    for (name, fx) in cases {
        let f = run(fx);
        lines.push(format!("{name}: 0x{:016x} ({}x{})", xxh3_64(&f.rgba), f.w, f.h));
    }
    let combined = lines.join("\n");
    let total = xxh3_64(combined.as_bytes());
    // при осознанной смене алгоритма: cargo test -- --nocapture, взять новую
    // сумму из вывода и обновить константу вместе с описанием изменения
    println!("{combined}\nTOTAL: 0x{total:016x}");
    const GOLDEN_TOTAL: u64 = 0xf069de998f74eed4;
    if GOLDEN_TOTAL != 0 {
        assert_eq!(total, GOLDEN_TOTAL, "видео-FX изменили поведение:\n{combined}");
    }
}

#[test]
fn t26_mirror_is_involution() {
    let m = FxInstance::Mirror { horizontal: true, vertical: true };
    let once = apply_video_chain(std::slice::from_ref(&m), test_frame(), &ctx());
    let twice = apply_video_chain(&[m], once, &ctx());
    assert_eq!(twice.rgba, test_frame().rgba, "двойное зеркало = исходник");
}

#[test]
fn t26_blur_preserves_mean_and_reduces_variance() {
    let src = test_frame();
    let out = run(FxInstance::Blur { radius: 12 });
    assert!((mean_rgb(&src) - mean_rgb(&out)).abs() < 8.0, "blur сохраняет среднюю яркость");
    let var = |f: &Frame| {
        let m = mean_rgb(f);
        f.rgba
            .chunks_exact(4)
            .map(|p| {
                let v = (p[0] as f64 + p[1] as f64 + p[2] as f64) / 3.0;
                (v - m) * (v - m)
            })
            .sum::<f64>()
    };
    assert!(var(&out) < var(&src), "blur снижает дисперсию");
}

#[test]
fn t26_greenscreen_keys_out_green_corner() {
    let out = run(FxInstance::Greenscreen {
        key_r: 0,
        key_g: 255,
        key_b: 0,
        tolerance: 70,
        softness: 25,
    });
    assert!(out.has_alpha);
    // центр зелёного угла прозрачен
    let (x, y) = (W * 3 / 4, H * 3 / 4);
    assert_eq!(out.rgba[((y * W + x) * 4 + 3) as usize], 0, "зелёный угол выключен");
    // не-зелёный угол непрозрачен
    assert_eq!(out.rgba[3], 255, "обычные пиксели не тронуты");
}

#[test]
fn t26_noise_is_deterministic_per_position() {
    let a = run(FxInstance::Noise { amount: 0.3 });
    let b = run(FxInstance::Noise { amount: 0.3 });
    assert_eq!(a.rgba, b.rgba, "одна позиция таймлайна = одно зерно");
    let other = apply_video_chain(
        &[FxInstance::Noise { amount: 0.3 }],
        test_frame(),
        &VideoFxCtx { burst_index: 180_000, ..ctx() },
    );
    assert_ne!(a.rgba, other.rgba, "другая позиция = другое зерно");
}

#[test]
fn t26_eventgrid_tiles_are_identical() {
    let out = run(FxInstance::EventGrid { cols: 2, rows: 2, center: true });
    assert_eq!((out.w, out.h), (W, H));
    // левая верхняя и правая верхняя ячейки идентичны
    let cell_w = (W / 2) as usize;
    let cell_h = (H / 2) as usize;
    for y in 0..cell_h {
        let a0 = (y * W as usize) * 4;
        let b0 = (y * W as usize + cell_w) * 4;
        assert_eq!(
            &out.rgba[a0..a0 + cell_w * 4],
            &out.rgba[b0..b0 + cell_w * 4],
            "строка {y}: плитки мозаики должны совпадать"
        );
    }
}

#[test]
fn t26_pan_stretch_identity_when_no_zoom() {
    // stretch + zoom 1 + pan 0 на кадре размеров out = почти тождество
    let out = run(FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 });
    let src = test_frame();
    let diff: u64 = out
        .rgba
        .iter()
        .zip(src.rgba.iter())
        .map(|(a, b)| (*a as i64 - *b as i64).unsigned_abs())
        .sum();
    let per_px = diff as f64 / (W * H * 4) as f64;
    assert!(per_px < 1.5, "stretch 1:1 почти без искажений, avg diff {per_px:.3}");
}

/// Тест 68 (п.2 р.7): pooled-цепочка с ГРЯЗНЫМ переиспользованным пулом
/// байт-идентична свежему прогону — для каждого эффекта и для цепочки
/// Mirror+EventGrid+Glow+VHS (пул не протекает между кадрами).
#[test]
fn t68_pooled_chain_identical_with_dirty_scratch() {
    use kt_fx::video::{FxScratch, apply_video_chain_pooled};
    let all: Vec<Vec<FxInstance>> = vec![
        vec![FxInstance::Pan { mode: PanMode::Fill, pan_x: 0.3, pan_y: -0.2, zoom: 1.4 }],
        vec![FxInstance::CamMotion {
            mode: PanMode::Fill,
            ease: true,
            time_scale: 1.0,
            keyframes: vec![],
        }],
        vec![FxInstance::Mirror { horizontal: true, vertical: false }],
        vec![FxInstance::Mirror { horizontal: true, vertical: true }],
        vec![FxInstance::Blur { radius: 9 }],
        vec![FxInstance::Greenscreen {
            key_r: 0,
            key_g: 255,
            key_b: 0,
            tolerance: 90,
            softness: 30,
        }],
        vec![FxInstance::Edges { threshold: 40 }],
        vec![FxInstance::Noise { amount: 0.5 }],
        vec![FxInstance::Vhs { rgb_shift: 3, bleed: 0.4 }],
        vec![FxInstance::Glow { strength: 1.2 }],
        vec![FxInstance::EventGrid { cols: 3, rows: 2, center: true }],
        // цепочка из жалобы пользователя (п.2 р.7)
        vec![
            FxInstance::Mirror { horizontal: true, vertical: false },
            FxInstance::EventGrid { cols: 2, rows: 2, center: false },
            FxInstance::Glow { strength: 1.0 },
            FxInstance::Vhs { rgb_shift: 2, bleed: 0.5 },
        ],
    ];
    let mut dirty = FxScratch::default();
    // испачкать пул чужими данными разных размеров
    let junk = apply_video_chain_pooled(
        &[FxInstance::Blur { radius: 5 }],
        test_frame(),
        &ctx(),
        &mut dirty,
    );
    drop(junk);
    for fx in &all {
        let fresh = apply_video_chain(fx, test_frame(), &ctx());
        let pooled = apply_video_chain_pooled(fx, test_frame(), &ctx(), &mut dirty);
        assert_eq!(fresh.rgba, pooled.rgba, "грязный пул изменил результат: {fx:?}");
        assert_eq!((fresh.w, fresh.h, fresh.has_alpha), (pooled.w, pooled.h, pooled.has_alpha));
        // второй прогон тем же пулом — тоже идентичен
        let again = apply_video_chain_pooled(fx, test_frame(), &ctx(), &mut dirty);
        assert_eq!(fresh.rgba, again.rgba);
    }
}
