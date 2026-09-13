//! Тест 297 (п.307, Р.44): слияние геометрических серий — правило
//! маршрутизации и байт-гарантии. Серия длины 1 из Pan/CamMotion/Mirror
//! обязана идти СТАРЫМИ ядрами (байты до Р.44 сторожат голдены t26);
//! слитые серии обязаны совпадать с точной перестановкой там, где она
//! математически точна, и не зависеть от прозрачных аудио-FX в цепочке.

use kt_fx::geom::{GeomExec, build_geom_exec, geo_run_end, is_geo};
use kt_fx::video::{Frame, VideoFxCtx, apply_video_chain};
use kt_model::{FxInstance, PanMode};

const W: u32 = 64;
const H: u32 = 36;

fn ctx() -> VideoFxCtx {
    VideoFxCtx { out_w: W, out_h: H, u: 0.5, burst_index: 90_000 }
}

/// Кадр как в t26: градиенты + зелёный угол.
fn test_frame() -> Frame {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            if x > W / 2 && y > H / 2 {
                rgba[i + 1] = 255;
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

fn run(fx: &[FxInstance]) -> Frame {
    apply_video_chain(fx, test_frame(), &ctx())
}

/// Точное зеркало кадра (перестановка пикселей) — эталон для t297b.
fn mirror_h(f: &Frame) -> Vec<u8> {
    let (w, h) = (f.w as usize, f.h as usize);
    let mut out = vec![0u8; f.rgba.len()];
    for y in 0..h {
        for x in 0..w {
            let s = (y * w + x) * 4;
            let d = (y * w + (w - 1 - x)) * 4;
            out[d..d + 4].copy_from_slice(&f.rgba[s..s + 4]);
        }
    }
    out
}

/// Правило маршрутизации: одиночные Pan/CamMotion/Mirror — старые ядра,
/// одиночный EventGrid и любая серия ≥ 2 — слитый проход. Решение живёт
/// РОВНО в одном месте (kt-fx), оба бэкенда его спрашивают.
#[test]
fn t297_routing_single_legacy_multi_fused() {
    let pan = FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.3, pan_y: -0.2, zoom: 1.5 };
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    assert!(matches!(build_geom_exec(&[&pan], W, H, W, H, 0.5), GeomExec::Legacy));
    assert!(matches!(build_geom_exec(&[&mir], W, H, W, H, 0.5), GeomExec::Legacy));
    assert!(matches!(build_geom_exec(&[&grid], W, H, W, H, 0.5), GeomExec::Fused(_)));
    assert!(matches!(build_geom_exec(&[&grid, &pan], W, H, W, H, 0.5), GeomExec::Fused(_)));
    assert!(matches!(build_geom_exec(&[&mir, &grid], W, H, W, H, 0.5), GeomExec::Fused(_)));
}

/// Зеркало ПОСЛЕ сетки — точная перестановка: слитая серия обязана дать
/// байт-в-байт mirror(grid) — доказательство, что слияние не растеризует
/// промежутков и совпадает с пошаговым путём там, где тот точен.
#[test]
fn t297b_grid_then_mirror_is_exact_permutation() {
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let fused = run(&[grid.clone(), mir]);
    let reference = run(&[grid]);
    assert_eq!(fused.rgba, mirror_h(&reference), "grid→mirror слитым проходом");
}

/// Аудио-FX внутри серии прозрачен: не рвёт слияние и не меняет байты
/// (GPU-зеркало его тоже пропускает — расхождение сегментации запрещено).
#[test]
fn t297c_audio_fx_inside_run_is_transparent() {
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let gain = FxInstance::Gain { db: -6.0, pan: 0.0 };
    let with_audio = run(&[grid.clone(), gain, mir.clone()]);
    let without = run(&[grid, mir]);
    assert_eq!(with_audio.rgba, without.rgba);
}

/// Два зеркала сливаются в тождество: композиция аффинных карт в f64 даёт
/// identity, тап в целых координатах точен — вход байт-в-байт.
#[test]
fn t297d_double_mirror_fuses_to_identity() {
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let out = run(&[mir.clone(), mir]);
    assert_eq!(out.rgba, test_frame().rgba, "mirror∘mirror ≠ identity");
}

/// Сегментация: барьер (blur) рвёт серию, аудио — нет.
#[test]
fn t297e_run_end_stops_at_barriers_only() {
    let fx = vec![
        FxInstance::EventGrid { cols: 2, rows: 2, center: false },
        FxInstance::Gain { db: 0.0, pan: 0.0 },
        FxInstance::Mirror { horizontal: true, vertical: false },
        FxInstance::blur(8),
        FxInstance::EventGrid { cols: 3, rows: 3, center: false },
    ];
    assert!(is_geo(&fx[0]));
    // ран накрывает grid + (прозрачный gain) + mirror, но не blur
    assert_eq!(geo_run_end(&fx, 0, &[]), 3);
    // стоп-индекс (снапшот силы слоя) рвёт ран
    assert_eq!(geo_run_end(&fx, 0, &[2]), 1);
}
