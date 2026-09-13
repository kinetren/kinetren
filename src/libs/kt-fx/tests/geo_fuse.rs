//! Тест 297 (п.307, Р.44): слияние геометрических серий — правило
//! маршрутизации и байт-гарантии. Серия длины 1 из Pan/CamMotion/Mirror
//! обязана идти СТАРЫМИ ядрами (байты до Р.44 сторожат голдены t26);
//! слитые серии обязаны совпадать с точной перестановкой там, где она
//! математически точна, и не зависеть от прозрачных аудио-FX в цепочке.

use kt_fx::geom::{GeomExec, build_geom_exec, is_geo};
use kt_fx::video::{Frame, VideoFxCtx, apply_video_chain};
use kt_model::{FxInstance, PanMode};

const W: u32 = 64;
const H: u32 = 36;

fn ctx() -> VideoFxCtx {
    VideoFxCtx { out_w: W, out_h: H, u: 0.5, burst_index: 90_000, cam3d: None }
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
    assert!(matches!(build_geom_exec(&[&pan], W, H, W, H, 0.5, None, 90_000), GeomExec::Legacy));
    assert!(matches!(build_geom_exec(&[&mir], W, H, W, H, 0.5, None, 90_000), GeomExec::Legacy));
    assert!(matches!(build_geom_exec(&[&grid], W, H, W, H, 0.5, None, 90_000), GeomExec::Fused(_)));
    assert!(matches!(build_geom_exec(&[&grid, &pan], W, H, W, H, 0.5, None, 90_000), GeomExec::Fused(_)));
    assert!(matches!(build_geom_exec(&[&mir, &grid], W, H, W, H, 0.5, None, 90_000), GeomExec::Fused(_)));
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

/// Серия без ресемплящих шагов (только зеркала) НЕ меняет размер кадра:
/// [Mirror, Mirror] на кадре чужого размера обязан вернуть кадр ТОГО ЖЕ
/// размера (старый путь работал in-place; слой чужого размера дальше
/// молча пропускается композитом — семантика сохранена).
#[test]
fn t297f_mirror_only_run_keeps_dims() {
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let src = Frame { w: 96, h: 54, rgba: vec![7u8; 96 * 54 * 4], has_alpha: false };
    let out = apply_video_chain(&[mir.clone(), mir], src, &ctx());
    assert_eq!((out.w, out.h), (96, 54), "зеркала не ресемплят");
    assert!(out.rgba.iter().all(|&b| b == 7), "и не трогают байты");
}

/// Тест 298 (п.308, Р.44): Pan3d на CPU — правила краёв обратной
/// гомографии. Тождество на дефолтах байт-в-байт; scale<1 — прозрачные
/// поля и непрозрачный центр; поворот к ребру (точки за камерой, w≤0) —
/// прозрачность без паники и мусора; без камеры — passthrough.
#[test]
fn t298_pan3d_cpu_edges() {
    use kt_fx::geom::Cam3d;
    let p3 = |rot_x: f64, scale: f64| FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x,
        rot_y: 0.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale,
        camera: None,
    };
    let cam = Some(Cam3d { cam: kt_model::CameraParams::default(), world_scale: 1.0 });
    let ctx3d = VideoFxCtx { cam3d: cam, ..ctx() };

    // тождество: дефолты + камера «50 мм» на авто-дистанции = 1:1
    let ident = apply_video_chain(&[p3(0.0, 1.0)], test_frame(), &ctx3d);
    assert_eq!(ident.rgba, test_frame().rgba, "pan3d-тождество на CPU");

    // scale 0.5: углы прозрачны, центр непрозрачен, альфа объявлена
    let half = apply_video_chain(&[p3(0.0, 0.5)], test_frame(), &ctx3d);
    assert!(half.has_alpha);
    let a = |f: &Frame, x: u32, y: u32| f.rgba[((y * W + x) * 4 + 3) as usize];
    assert_eq!(a(&half, 1, 1), 0, "угол прозрачен");
    assert_eq!(a(&half, W / 2, H / 2), 255, "центр непрозрачен");

    // поворот к ребру: часть плоскости за камерой (w ≤ 0) — прозрачно,
    // без NaN/паники и детерминировано. Кромка силуэта с п.309
    // анти-алиасится (сабтапы мешают прозрачное с контентом), поэтому
    // альфа на контуре дробная — но есть и чистая прозрачность, и
    // чистый контент.
    let edge = apply_video_chain(&[p3(110.0, 1.0)], test_frame(), &ctx3d);
    assert!(edge.rgba.chunks_exact(4).any(|p| p[3] == 0), "за камерой — прозрачность");
    assert!(edge.rgba.chunks_exact(4).any(|p| p[3] == 255), "контент остался");
    let again = apply_video_chain(&[p3(110.0, 1.0)], test_frame(), &ctx3d);
    assert_eq!(edge.rgba, again.rgba, "детерминизм");

    // без камеры Pan3d пропускается: passthrough байт-в-байт
    let noop = apply_video_chain(&[p3(30.0, 0.5)], test_frame(), &ctx());
    assert_eq!(noop.rgba, test_frame().rgba, "без камеры — как раньше");
}

/// Сегментация: жёсткий барьер (blur) рвёт серию, аудио — нет; стоп-индекс
/// силы слоя рвёт (с Р.45 сегментация живёт в plan_chain — п.311/т306).
#[test]
fn t297e_run_end_stops_at_barriers_only() {
    use kt_fx::plan::{PlanOp, plan_chain};
    let fx = vec![
        FxInstance::EventGrid { cols: 2, rows: 2, center: false },
        FxInstance::Gain { db: 0.0, pan: 0.0 },
        FxInstance::Mirror { horizontal: true, vertical: false },
        FxInstance::blur(8),
        FxInstance::EventGrid { cols: 3, rows: 3, center: false },
    ];
    assert!(is_geo(&fx[0]));
    // ран накрывает grid + (прозрачный gain) + mirror, но не blur
    let p = plan_chain(&fx, W, H, W, H, 0.5, None, 90_000, &[]);
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
    assert!(matches!(p.ops[1], PlanOp::Barrier { idx: 3, .. }));
    assert!(matches!(p.ops[2], PlanOp::Run { i: 4, j: 5, .. }));
    // стоп-индекс (снапшот силы слоя) рвёт ран
    let ps = plan_chain(&fx, W, H, W, H, 0.5, None, 90_000, &[2]);
    assert!(matches!(ps.ops[0], PlanOp::Run { i: 0, j: 1, .. }));
    assert!(matches!(ps.ops[1], PlanOp::Snapshot));
    assert!(matches!(ps.ops[2], PlanOp::Run { i: 2, j: 3, .. }));
}

/// Шахматка 1×1 px — самый хрупкий к ресемплингу узор.
fn checkerboard() -> Frame {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            let v = if (x + y) % 2 == 0 { 0 } else { 255 };
            rgba[i] = v;
            rgba[i + 1] = v;
            rgba[i + 2] = v;
            rgba[i + 3] = 255;
        }
    }
    Frame { w: W, h: H, rgba, has_alpha: false }
}

/// Средний горизонтальный контраст красного канала (0..255).
fn contrast(f: &Frame) -> f64 {
    let mut sum = 0u64;
    let mut n = 0u64;
    for y in 0..f.h {
        for x in 1..f.w {
            let i = ((y * f.w + x) * 4) as usize;
            let j = ((y * f.w + x - 1) * 4) as usize;
            sum += (f.rgba[i] as i64 - f.rgba[j] as i64).unsigned_abs();
            n += 1;
        }
    }
    sum as f64 / n as f64
}

/// Тест 296 (п.307/п.309, Р.44): нетто-единичная гео-цепочка ВОЗВРАЩАЕТ
/// детализацию. Сетка 2×2 уменьшила вдвое, зум ×2 вернул — слитый проход
/// сэмплирует ИСХОДНИК (позиции выходят точно на текселя), и шахматка
/// приходит целой. Пошаговый путь (до Р.44) растеризовал сетку в
/// полутекстельной фазе: шахматка усреднялась в ровный серый, и зум
/// увеличивал кашу — контраст падал до нуля. Тест ОБЯЗАН падать на
/// старом коде (проверено на коммите К2 в worktree — контраст 0.0).
#[test]
fn t296_net_identity_chain_preserves_detail() {
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let zoom2 = FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 2.0 };
    let out = apply_video_chain(&[grid, zoom2], checkerboard(), &ctx());
    let c = contrast(&out);
    assert!(c >= 250.0, "шахматка обязана вернуться целой, контраст {c:.1}");

    // тот же принцип с перспективой (PerPixel-суперсемплинг): сетка →
    // pan3d-тождество → сетка → pan3d-зум ×4; нетто ≈ 1, центр кадра
    // обязан сохранить высокий контраст (кайма сеток размывает края)
    use kt_fx::geom::Cam3d;
    let ctx3d = VideoFxCtx {
        cam3d: Some(Cam3d { cam: kt_model::CameraParams::default(), world_scale: 1.0 }),
        ..ctx()
    };
    let p3 = |scale: f64| FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 0.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale,
        camera: None,
    };
    let grid_c = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    let out3d = apply_video_chain(
        &[grid_c.clone(), p3(1.0), grid_c, p3(4.0)],
        checkerboard(),
        &ctx3d,
    );
    // контраст по центральной трети кадра
    let mut sum = 0u64;
    let mut n = 0u64;
    for y in H / 3..2 * H / 3 {
        for x in W / 3 + 1..2 * W / 3 {
            let i = ((y * W + x) * 4) as usize;
            let j = ((y * W + x - 1) * 4) as usize;
            sum += (out3d.rgba[i] as i64 - out3d.rgba[j] as i64).unsigned_abs();
            n += 1;
        }
    }
    let c3 = sum as f64 / n as f64;
    assert!(c3 >= 150.0, "нетто-единица с pan3d сохраняет детализацию, контраст {c3:.1}");
}

/// Суперсемплинг реально включён: сетка 4×4 (минификация ×4) на шумовом
/// кадре обязана дать НИЗКУЮ дисперсию (16-таповое box-среднее), а не
/// дисперсию точечной билинейки (2 тапа из 4 столбцов).
#[test]
fn t296b_supersampling_tames_minification_noise() {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for (p, px) in rgba.chunks_exact_mut(4).enumerate() {
        // детерминированный «шум» — мультипликативный хэш позиции
        let v = ((p as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 56) as u8;
        px[0] = v;
        px[1] = v;
        px[2] = v;
        px[3] = 255;
    }
    let src = Frame { w: W, h: H, rgba, has_alpha: false };
    let var = |f: &Frame, x0: u32, x1: u32, y0: u32, y1: u32| {
        let mut vals = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                vals.push(f.rgba[((y * f.w + x) * 4) as usize] as f64);
            }
        }
        let m = vals.iter().sum::<f64>() / vals.len() as f64;
        vals.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / vals.len() as f64
    };
    let src_var = var(&src, 0, W, 0, H);
    let out = apply_video_chain(
        &[FxInstance::EventGrid { cols: 4, rows: 4, center: false }],
        src,
        &ctx(),
    );
    // первая плитка (16×9) — уменьшенный вчетверо кадр
    let out_var = var(&out, 0, 16, 0, 9);
    assert!(
        out_var < src_var / 6.0,
        "box-суперсемплинг обязан гасить шум: {out_var:.0} против {src_var:.0} у источника"
    );
}

/// Тест 300 (п.309): формула тапов и граница включения суперсемплинга.
#[test]
fn t300_ss_taps_and_routing_boundary() {
    use kt_fx::geom::ss_taps;
    // квант 1/32: включение строго при len > 1 + 1/64
    assert_eq!(ss_taps(1.0), 1);
    assert_eq!(ss_taps(1.01), 1);
    assert_eq!(ss_taps(1.015624), 1);
    assert_eq!(ss_taps(1.015625), 2); // ровно 1 + 1/64
    assert_eq!(ss_taps(2.0), 2);
    assert_eq!(ss_taps(2.04), 3);
    assert_eq!(ss_taps(3.97), 4);
    assert_eq!(ss_taps(100.0), 4); // потолок

    // Pan без минификации — старое ядро (байты до Р.44); с минификацией —
    // слитый проход ради суперсемплинга
    let pan1 = FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 };
    let pan_min = FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 0.9 };
    assert!(matches!(build_geom_exec(&[&pan1], W, H, W, H, 0.5, None, 90_000), GeomExec::Legacy));
    assert!(matches!(build_geom_exec(&[&pan_min], W, H, W, H, 0.5, None, 90_000), GeomExec::Fused(_)));

    // детерминизм слитого суперсемплинга
    let a = run(std::slice::from_ref(&pan_min));
    let b = run(std::slice::from_ref(&pan_min));
    assert_eq!(a.rgba, b.rgba);
}

/// Замер (не тест): нс/px слитого прохода на 1080p. Запуск:
/// `cargo test -p kt-fx --release --test geo_fuse -- --ignored --nocapture`
/// О скорости судить ТОЛЬКО по релизу (п.301).
#[test]
#[ignore]
fn bench_geom_1080p() {
    use kt_fx::geom::Cam3d;
    use std::time::Instant;
    let (bw, bh) = (1920u32, 1080u32);
    let mut rgba = vec![0u8; (bw * bh * 4) as usize];
    for (p, px) in rgba.chunks_exact_mut(4).enumerate() {
        px[0] = (p % 251) as u8;
        px[1] = (p % 241) as u8;
        px[2] = (p % 239) as u8;
        px[3] = 255;
    }
    let mk = || Frame { w: bw, h: bh, rgba: rgba.clone(), has_alpha: false };
    let bctx = VideoFxCtx { out_w: bw, out_h: bh, u: 0.5, burst_index: 90_000, cam3d: None };
    let ctx3d = VideoFxCtx {
        cam3d: Some(Cam3d { cam: kt_model::CameraParams::default(), world_scale: 1.0 }),
        ..bctx
    };
    let p3 = |scale: f64| FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 10.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale,
        camera: None,
    };
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    let npx = (bw * bh) as f64;
    let bench = |name: &str, fx: &[FxInstance], c: &VideoFxCtx| {
        let t0 = Instant::now();
        let out = apply_video_chain(fx, mk(), c);
        let dt = t0.elapsed();
        println!(
            "{name}: {:.1} мс/кадр 1080p, {:.1} нс/px (выход {}x{})",
            dt.as_secs_f64() * 1e3,
            dt.as_secs_f64() * 1e9 / npx,
            out.w,
            out.h
        );
    };
    bench("pan legacy (zoom 1.5)", &[FxInstance::Pan {
        mode: PanMode::Stretch,
        pan_x: 0.0,
        pan_y: 0.0,
        zoom: 1.5,
    }], &bctx);
    bench("grid 2x2 слитый + SS 2x2", std::slice::from_ref(&grid), &bctx);
    bench("цепочка пользователя grid→pan3d→grid→pan3d(зум)", &[
        grid.clone(),
        p3(1.0),
        grid.clone(),
        p3(4.0),
    ], &ctx3d);
    bench("pan3d одиночный (perpixel-пробы)", &[p3(0.9)], &ctx3d);
}

/// Тест 306 (п.311, Р.45): план цепочки — единственное место сегментации.
/// Раны/барьеры/маркеры силы, бюджет шагов, и «план слайса ≡ кусок плана
/// целого» (CPU режет по стопам уровнем выше, GPU планирует целиком со
/// stops — картинки обязаны совпасть).
#[test]
fn t306_chain_plan_segmentation() {
    use kt_fx::plan::{PlanOp, plan_chain};
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let gain = FxInstance::Gain { db: 0.0, pan: 0.0 };
    let blur = FxInstance::blur(8);

    // ран (grid + прозрачный аудио + mirror), барьер, ран
    let fx = vec![grid.clone(), gain.clone(), mir.clone(), blur.clone(), grid.clone()];
    let p = plan_chain(&fx, W, H, W, H, 0.5, None, 90_000, &[]);
    assert_eq!(p.ops.len(), 3);
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
    assert!(matches!(p.ops[1], PlanOp::Barrier { idx: 3, res_scale: 1 }));
    assert!(matches!(p.ops[2], PlanOp::Run { i: 4, j: 5, .. }));

    // сила слоя: снапшот-точка рвёт ран, слот — маркер Mix
    let fxm = vec![grid.clone(), FxInstance::LayerMix { amount: 0.5, from: 0 }, mir.clone()];
    let pm = plan_chain(&fxm, W, H, W, H, 0.5, None, 90_000, &[0]);
    assert!(matches!(pm.ops[0], PlanOp::Snapshot));
    assert!(matches!(pm.ops[1], PlanOp::Run { i: 0, j: 1, .. }));
    assert!(matches!(pm.ops[2], PlanOp::Mix { idx: 1 }));
    assert!(matches!(pm.ops[3], PlanOp::Run { i: 2, j: 3, .. }));
    assert_eq!(pm.ops.len(), 4);

    // «план слайса ≡ кусок плана целого»: не-маркерные опы совпадают
    let s1 = plan_chain(&fxm[0..1], W, H, W, H, 0.5, None, 90_000, &[]);
    let s2 = plan_chain(&fxm[2..3], W, H, W, H, 0.5, None, 90_000, &[]);
    assert!(matches!(s1.ops[..], [PlanOp::Run { i: 0, j: 1, .. }]));
    assert!(matches!(s2.ops[..], [PlanOp::Run { i: 0, j: 1, .. }]));

    // бюджет гео-шагов: 7 подряд режутся на 6 + 1 (в точке разреза
    // качество = сегодняшнее)
    let seven: Vec<FxInstance> = (0..7).map(|_| mir.clone()).collect();
    let p7 = plan_chain(&seven, W, H, W, H, 0.5, None, 90_000, &[]);
    assert_eq!(p7.ops.len(), 2);
    assert!(matches!(p7.ops[0], PlanOp::Run { i: 0, j: 6, .. }));
    assert!(matches!(p7.ops[1], PlanOp::Run { i: 6, j: 7, .. }));
}

/// Тест 305 (п.312, Р.45): поточечные Greenscreen/Noise ВНУТРИ гео-серии
/// перестают рвать слияние. (а) grid → greenscreen → zoom: ключ работает
/// (зелёный вырезан), а резкость шахматки не потеряна — до Р.45 барьер
/// растеризовал сетку в 1× и зум мылил. (б) grid → noise → zoom: зерно
/// детерминировано и прибито к сетке СВОЕГО шага (зумится с картинкой).
#[test]
fn t305_pointwise_inside_run() {
    use kt_fx::plan::{PlanOp, plan_chain};
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let zoom2 = FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 2.0 };
    let gs = FxInstance::greenscreen([0, 255, 0], 70, 25);

    // (0) сегментация: внутренний поточечный поглощён — ран ОДИН
    let fx = vec![grid.clone(), gs.clone(), zoom2.clone()];
    let p = plan_chain(&fx, W, H, W, H, 0.5, None, 90_000, &[]);
    assert_eq!(p.ops.len(), 1, "внутренний greenscreen поглощён в ран");
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
    // ведущий поточечный НЕ поглощается (байты K=1-цепочек)
    let lead = vec![gs.clone(), zoom2.clone()];
    let pl = plan_chain(&lead, W, H, W, H, 0.5, None, 90_000, &[]);
    assert!(matches!(pl.ops[0], PlanOp::Barrier { idx: 0, .. }));

    // (а) шахматка с зелёным: ключ вырезает зелёные клетки, резкость
    // остальных сохранена (нетто-единица grid ×0.5 → zoom ×2)
    let mut cb = checkerboard();
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            if cb.rgba[i] == 255 {
                cb.rgba[i] = 0;
                cb.rgba[i + 1] = 255; // белые клетки становятся зелёным ключом
                cb.rgba[i + 2] = 0;
            }
        }
    }
    let out = apply_video_chain(&[grid.clone(), gs, zoom2.clone()], cb, &ctx());
    assert!(out.has_alpha, "greenscreen объявил альфу и в слитом проходе");
    let mut keyed = 0usize;
    let mut kept = 0usize;
    for px in out.rgba.chunks_exact(4) {
        if px[3] == 0 {
            keyed += 1;
        } else if px[3] == 255 {
            kept += 1;
        }
    }
    let total = (W * H) as usize;
    assert!(keyed > total / 3, "зелёные клетки вырезаны ({keyed}/{total})");
    assert!(kept > total / 3, "тёмные клетки целы ({kept}/{total})");

    // (б) зерно: детерминизм и привязка к шагу — зерно size=1 на стадии
    // ПЕРЕД зумом растягивается им в блоки 2×2 выходных пикселя
    let noise = FxInstance::Noise { amount: 0.9, size: 1, color: false };
    let grey = Frame { w: W, h: H, rgba: vec![128u8; (W * H * 4) as usize], has_alpha: false };
    let mk = || Frame { w: W, h: H, rgba: grey.rgba.clone(), has_alpha: false };
    let a = apply_video_chain(&[grid.clone(), noise.clone(), zoom2.clone()], mk(), &ctx());
    let b = apply_video_chain(&[grid.clone(), noise.clone(), zoom2.clone()], mk(), &ctx());
    assert_eq!(a.rgba, b.rgba, "зерно детерминировано");
    // блочность ~2×2 (фаза блоков смещена кропом зума — сравниваем
    // СОСЕДЕЙ): зерно стадии до зума растянуто ×2, поэтому около половины
    // горизонтальных и вертикальных пар пикселей совпадают; у экранного
    // зерна size=1 совпадений почти нет
    let px = |f: &Frame, x: u32, y: u32| f.rgba[((y * W + x) * 4) as usize];
    let (mut eq_h, mut n_h) = (0usize, 0usize);
    let (mut eq_v, mut n_v) = (0usize, 0usize);
    for y in 0..H {
        for x in 0..W - 1 {
            n_h += 1;
            if px(&a, x, y) == px(&a, x + 1, y) {
                eq_h += 1;
            }
        }
    }
    for y in 0..H - 1 {
        for x in 0..W {
            n_v += 1;
            if px(&a, x, y) == px(&a, x, y + 1) {
                eq_v += 1;
            }
        }
    }
    assert!(
        eq_h * 100 >= n_h * 35 && eq_v * 100 >= n_v * 35,
        "зерно прибито к шагу (растянуто зумом): равных соседей {eq_h}/{n_h} по X, {eq_v}/{n_v} по Y"
    );
    // и зерно РЕАЛЬНО есть (не съедено усреднением)
    assert!(a.rgba.chunks_exact(4).any(|p| p[0] != 128), "зерно видно");
}

/// Тест 303 (п.313, Р.45): ядёрная инвариантность вида барьеров к
/// разрешению БЕЗ цепочек: `down_K(kernel_K(up_K(F))) ≈ kernel_1(F)`
/// (up/down — та же билинейка fit_into, что и все ресемплы). Пороги ×2 от
/// замера (правило kt-gpu/CLAUDE.md); edges — пороговый класс (бинарные
/// решения на интерполированной люме законно флипают).
#[test]
fn t303_barrier_kernels_res_invariant() {
    use kt_fx::video::{FxScratch, apply_barrier, fit_into};
    // Замер (K=2 и K=4, кадр t26): worst mean 1.04 (vhs), worst p99 24
    // (vhs K=2); пороги ×2.
    let cases: Vec<(&str, FxInstance)> = vec![
        ("blur", FxInstance::blur(8)),
        ("glow", FxInstance::glow(0.9)),
        ("vhs", FxInstance::Vhs { rgb_shift: 4, bleed: 0.4, scan_step: 5, jitter: 0.5 }),
        ("noise", FxInstance::Noise { amount: 0.5, size: 2, color: true }),
        ("noise-special", FxInstance::noise(0.5)),
    ];
    let mut scratch = FxScratch::default();
    for k in [2u32, 4] {
        for (name, fx) in &cases {
            let base = apply_barrier(fx, test_frame(), &ctx(), &mut scratch, 1);
            let (kw, kh) = (W * k, H * k);
            let mut up = vec![0u8; (kw * kh * 4) as usize];
            fit_into(&test_frame(), kw, kh, &mut up);
            let upf = Frame { w: kw, h: kh, rgba: up, has_alpha: false };
            let kf = apply_barrier(fx, upf, &ctx(), &mut scratch, k);
            assert_eq!((kf.w, kf.h), (kw, kh), "{name}: ядро не меняет размер");
            let mut down = vec![0u8; (W * H * 4) as usize];
            fit_into(&kf, W, H, &mut down);
            let mut diffs: Vec<u32> = base
                .rgba
                .iter()
                .zip(&down)
                .map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs())
                .collect();
            diffs.sort_unstable();
            let mean = diffs.iter().map(|&d| d as u64).sum::<u64>() as f64 / diffs.len() as f64;
            let p99 = diffs[diffs.len() * 99 / 100] as f64;
            println!("t303 {name} K={k}: mean {mean:.2}, p99 {p99}");
            assert!(
                mean <= 2.5 && p99 <= 48.0,
                "{name} K={k}: вид уехал (mean {mean:.2}, p99 {p99})"
            );
        }
        // edges — пороговый класс: сравниваем ДОЛЮ разъехавшихся пикселей
        for (name, fx) in [
            ("edges", FxInstance::edges(48)),
            (
                "edges-thick",
                FxInstance::Edges {
                    threshold: 48,
                    thickness: 2,
                    color_r: 255,
                    color_g: 255,
                    color_b: 255,
                },
            ),
        ] {
            let base = apply_barrier(&fx, test_frame(), &ctx(), &mut scratch, 1);
            let (kw, kh) = (W * k, H * k);
            let mut up = vec![0u8; (kw * kh * 4) as usize];
            fit_into(&test_frame(), kw, kh, &mut up);
            let upf = Frame { w: kw, h: kh, rgba: up, has_alpha: false };
            let kf = apply_barrier(&fx, upf, &ctx(), &mut scratch, k);
            let mut down = vec![0u8; (W * H * 4) as usize];
            fit_into(&kf, W, H, &mut down);
            let over = base
                .rgba
                .iter()
                .zip(&down)
                .filter(|(a, b)| (**a as i32 - **b as i32).unsigned_abs() > 16)
                .count();
            let frac = over as f64 / base.rgba.len() as f64;
            println!("t303 {name} K={k}: |Δ|>16 у {:.2}%", frac * 100.0);
            // замер: 0.88 % (K=2); порог ×2
            assert!(frac <= 0.02, "{name} K={k}: контуры разъехались ({:.2}%)", frac * 100.0);
        }
    }
}
