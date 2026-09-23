//! Тест 297 (п.307, Р.44): слияние геометрических серий — правило
//! маршрутизации и байт-гарантии. Серия длины 1 из Pan/CamMotion/Mirror
//! обязана идти СТАРЫМИ ядрами (байты до Р.44 сторожат голдены t26);
//! слитые серии обязаны совпадать с точной перестановкой там, где она
//! математически точна, и не зависеть от прозрачных аудио-FX в цепочке.

use kt_fx::geom::{GeomExec, build_geom_exec, is_geo};
use kt_fx::video::{Frame, VideoFxCtx, apply_video_chain};
use kt_model::FxInstance;

const W: u32 = 64;
const H: u32 = 36;

fn ctx() -> VideoFxCtx {
    VideoFxCtx {
        out_w: W,
        out_h: H,
        u: 0.5,
        burst_index: 90_000,
        cam3d: None,
        in_scale: 1,
        lead: None,
    }
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
    // Р.62: Pan упразднён; старое ядро осталось у одиночного CamMotion, а
    // узел «Геометрия» ВСЕГДА слитый (у него прозрачные поля)
    let cam = FxInstance::cam_motion();
    let pan = FxInstance::geometry_at(-0.05, 0.03333, 1.5);
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    assert!(matches!(
        build_geom_exec(&[&cam], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Legacy
    ));
    assert!(matches!(
        build_geom_exec(&[&pan], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Fused(_)
    ));
    assert!(matches!(
        build_geom_exec(&[&mir], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Legacy
    ));
    assert!(matches!(
        build_geom_exec(&[&grid], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Fused(_)
    ));
    assert!(matches!(
        build_geom_exec(&[&grid, &pan], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Fused(_)
    ));
    assert!(matches!(
        build_geom_exec(&[&mir, &grid], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Fused(_)
    ));
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
    let p = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
    assert!(matches!(p.ops[1], PlanOp::Barrier { idx: 3, .. }));
    assert!(matches!(p.ops[2], PlanOp::Run { i: 4, j: 5, .. }));
    // стоп-индекс (снапшот силы слоя) рвёт ран
    let ps = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[2]);
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
    let zoom2 = FxInstance::geometry_zoom(2.0);
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
    let out3d =
        apply_video_chain(&[grid_c.clone(), p3(1.0), grid_c, p3(4.0)], checkerboard(), &ctx3d);
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

    // CamMotion без минификации — старое ядро (байты до Р.44); узел
    // «Геометрия» с минификацией — слитый проход ради суперсемплинга
    let pan1 = FxInstance::cam_motion();
    let pan_min = FxInstance::geometry_zoom(0.9);
    assert!(matches!(
        build_geom_exec(&[&pan1], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Legacy
    ));
    assert!(matches!(
        build_geom_exec(&[&pan_min], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Fused(_)
    ));

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
    let bctx = VideoFxCtx {
        out_w: bw,
        out_h: bh,
        u: 0.5,
        burst_index: 90_000,
        cam3d: None,
        in_scale: 1,
        lead: None,
    };
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
    bench("pan legacy (zoom 1.5)", &[FxInstance::geometry_zoom(1.5)], &bctx);
    bench("grid 2x2 слитый + SS 2x2", std::slice::from_ref(&grid), &bctx);
    bench(
        "цепочка пользователя grid→pan3d→grid→pan3d(зум)",
        &[grid.clone(), p3(1.0), grid.clone(), p3(4.0)],
        &ctx3d,
    );
    bench("pan3d одиночный (perpixel-пробы)", &[p3(0.9)], &ctx3d);
    // K×-барьеры (п.314): на 1080p k_cap = 4096/1920 = 2 — барьер-сегмент
    // работает в 3840×2160
    let zoom2 = FxInstance::geometry_zoom(2.0);
    bench(
        "grid→vhs(K=2)→zoom, барьер в 3840×2160",
        &[
            grid.clone(),
            FxInstance::Vhs { rgb_shift: 2, bleed: 0.3, scan_step: 5, jitter: 0.4 },
            zoom2.clone(),
        ],
        &bctx,
    );
    bench("grid→blur(K=2)→zoom", &[grid.clone(), FxInstance::blur(8), zoom2.clone()], &bctx);
    bench(
        "grid→greenscreen(tap)→zoom",
        &[grid, FxInstance::greenscreen([0, 255, 0], 70, 25), zoom2],
        &bctx,
    );
}

/// Перф-печать Р.47 (п.336): цена новых эффектов на 1080p, нс/px.
/// `cargo test -p kt-fx --test geo_fuse --release -- --ignored bench_fx`
#[test]
#[ignore]
fn bench_fx_1080p() {
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
    let bctx = VideoFxCtx {
        out_w: bw,
        out_h: bh,
        u: 0.5,
        burst_index: 90_000,
        cam3d: None,
        in_scale: 1,
        lead: None,
    };
    let npx = (bw * bh) as f64;
    let bench = |name: &str, fx: &[FxInstance]| {
        let t0 = Instant::now();
        let out = apply_video_chain(fx, mk(), &bctx);
        let dt = t0.elapsed();
        println!(
            "{name}: {:.1} мс/кадр 1080p, {:.1} нс/px (выход {}x{})",
            dt.as_secs_f64() * 1e3,
            dt.as_secs_f64() * 1e9 / npx,
            out.w,
            out.h
        );
    };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    bench(
        "colorgrade+vignette (2 барьера)",
        &[
            FxInstance::ColorGrade { brightness: 0.05, contrast: 0.3, saturation: 1.4, hue: 20.0 },
            FxInstance::vignette(),
        ],
    );
    bench("sharpen r=3", &[FxInstance::Sharpen { amount: 1.5, radius: 3 }]);
    bench("pixelate 12", &[FxInstance::Pixelate { size: 12 }]);
    bench("chroma_ab 0.02", &[FxInstance::ChromaticAberration { amount: 0.02 }]);
    bench(
        "radial_blur zoom (16 тапов)",
        &[FxInstance::RadialBlur { amount: 0.5, center_x: 0.0, center_y: 0.0, spin: false }],
    );
    bench(
        "radial_blur spin",
        &[FxInstance::RadialBlur { amount: 0.5, center_x: 0.0, center_y: 0.0, spin: true }],
    );
    bench("halftone 8/45", &[FxInstance::Halftone { size: 8, angle: 45.0 }]);
    bench(
        "greenscreen→drop_shadow (тень с альфой)",
        &[FxInstance::greenscreen([0, 255, 0], 70, 25), FxInstance::drop_shadow()],
    );
    bench(
        "grid→lens→twirl→zoom (слитый ран, perpixel-SS)",
        &[grid, FxInstance::lens_distort(), FxInstance::twirl(), zoom2],
    );
    bench("kaleidoscope соло", &[FxInstance::kaleidoscope()]);
    bench("cylinder 0.8", &[FxInstance::Cylinder3d { curvature: 0.8 }]);
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
    let p = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert_eq!(p.ops.len(), 3);
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
    assert!(matches!(p.ops[1], PlanOp::Barrier { idx: 3, res_scale: 1 }));
    assert!(matches!(p.ops[2], PlanOp::Run { i: 4, j: 5, .. }));

    // сила слоя: снапшот-точка рвёт ран, слот — маркер Mix
    let fxm = vec![grid.clone(), FxInstance::LayerMix { amount: 0.5, from: 0 }, mir.clone()];
    let pm = plan_chain(&fxm, W, H, 1, W, H, 0.5, None, 90_000, &[0]);
    assert!(matches!(pm.ops[0], PlanOp::Snapshot));
    assert!(matches!(pm.ops[1], PlanOp::Run { i: 0, j: 1, .. }));
    assert!(matches!(pm.ops[2], PlanOp::Mix { idx: 1 }));
    assert!(matches!(pm.ops[3], PlanOp::Run { i: 2, j: 3, .. }));
    assert_eq!(pm.ops.len(), 4);

    // «план слайса ≡ кусок плана целого»: не-маркерные опы совпадают
    let s1 = plan_chain(&fxm[0..1], W, H, 1, W, H, 0.5, None, 90_000, &[]);
    let s2 = plan_chain(&fxm[2..3], W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert!(matches!(s1.ops[..], [PlanOp::Run { i: 0, j: 1, .. }]));
    assert!(matches!(s2.ops[..], [PlanOp::Run { i: 0, j: 1, .. }]));

    // бюджет гео-шагов: 7 подряд режутся на 6 + 1 (в точке разреза
    // качество = сегодняшнее)
    let seven: Vec<FxInstance> = (0..7).map(|_| mir.clone()).collect();
    let p7 = plan_chain(&seven, W, H, 1, W, H, 0.5, None, 90_000, &[]);
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
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let gs = FxInstance::greenscreen([0, 255, 0], 70, 25);

    // (0) сегментация: внутренний поточечный поглощён — ран ОДИН
    let fx = vec![grid.clone(), gs.clone(), zoom2.clone()];
    let p = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert_eq!(p.ops.len(), 1, "внутренний greenscreen поглощён в ран");
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
    // ведущий поточечный НЕ поглощается (байты K=1-цепочек)
    let lead = vec![gs.clone(), zoom2.clone()];
    let pl = plan_chain(&lead, W, H, 1, W, H, 0.5, None, 90_000, &[]);
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
        // Р.47 Кв3 (гладкие): пороги ×2 от замера (см. printout)
        for (name, fx) in [
            ("sharpen", FxInstance::Sharpen { amount: 2.0, radius: 4 }),
            ("chroma", FxInstance::ChromaticAberration { amount: 0.03 }),
            (
                "radial-zoom",
                FxInstance::RadialBlur { amount: 0.5, center_x: 0.0, center_y: 0.0, spin: false },
            ),
            (
                "radial-spin",
                FxInstance::RadialBlur { amount: 0.5, center_x: 0.2, center_y: 0.1, spin: true },
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
        // тень требует настоящей альфы: жёсткий круг в центре
        {
            let alpha_frame = || {
                let mut f = test_frame();
                f.has_alpha = true;
                for y in 0..H {
                    for x in 0..W {
                        let (dx, dy) = (x as f64 - W as f64 / 2.0, y as f64 - H as f64 / 2.0);
                        let i = ((y * W + x) * 4 + 3) as usize;
                        f.rgba[i] = if dx * dx + dy * dy < 144.0 { 255 } else { 0 };
                    }
                }
                f
            };
            let fx = FxInstance::drop_shadow();
            let base = apply_barrier(&fx, alpha_frame(), &ctx(), &mut scratch, 1);
            let (kw, kh) = (W * k, H * k);
            let mut up = vec![0u8; (kw * kh * 4) as usize];
            fit_into(&alpha_frame(), kw, kh, &mut up);
            let upf = Frame { w: kw, h: kh, rgba: up, has_alpha: true };
            let kf = apply_barrier(&fx, upf, &ctx(), &mut scratch, k);
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
            println!("t303 drop-shadow K={k}: mean {mean:.2}, p99 {p99}");
            // кайма тени на ЖЁСТКОЙ альфе (двоичный круг) сдвигается на
            // фазу мипа — пороги свои, ×2 от замера (K=2: 2.14/56)
            assert!(
                mean <= 4.5 && p99 <= 112.0,
                "drop-shadow K={k}: вид уехал (mean {mean:.2}, p99 {p99})"
            );
        }
        // halftone: K×-исполнение антиалиасит кромки точек (это его
        // выигрыш, а не дефект) — попиксельная метрика не годится.
        // Сверяется ПОКРЫТИЕ: средняя люма блоков 8×8 (позиция и размер
        // точек), пороги ×2 от замера (см. printout).
        {
            let fx = FxInstance::Halftone { size: 8, angle: 45.0 };
            let base = apply_barrier(&fx, test_frame(), &ctx(), &mut scratch, 1);
            let (kw, kh) = (W * k, H * k);
            let mut up = vec![0u8; (kw * kh * 4) as usize];
            fit_into(&test_frame(), kw, kh, &mut up);
            let upf = Frame { w: kw, h: kh, rgba: up, has_alpha: false };
            let kf = apply_barrier(&fx, upf, &ctx(), &mut scratch, k);
            let mut down = vec![0u8; (W * H * 4) as usize];
            fit_into(&kf, W, H, &mut down);
            let block = 8u32;
            let mut worst = 0.0f64;
            for by in 0..H / block {
                for bx in 0..W / block {
                    let (mut sa, mut sb) = (0u64, 0u64);
                    for y in by * block..(by + 1) * block {
                        for x in bx * block..(bx + 1) * block {
                            let i = ((y * W + x) * 4) as usize;
                            sa += base.rgba[i] as u64;
                            sb += down[i] as u64;
                        }
                    }
                    let n = (block * block) as f64;
                    worst = worst.max((sa as f64 / n - sb as f64 / n).abs());
                }
            }
            println!("t303 halftone K={k}: worst block delta {worst:.1}");
            // замер: 14.0 (K=2), 6.0 (K=4); порог ×2
            assert!(worst <= 28.0, "halftone K={k}: узор уехал (block {worst:.1})");
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
            // пороговый Кв3: фаза ячейки на границе законно флипает
            ("pixelate", FxInstance::Pixelate { size: 8 }),
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

/// Тест 302 (п.314, Р.45): БАРЬЕР между сеткой и зумом больше не убивает
/// детализацию — vhs с нулевыми параметрами (identity-ядро) исполняется
/// в K×, и шахматка проходит цепочку [grid 2×2 → vhs → zoom ×2] целой.
/// До включения K (п.311-п.313) тот же тест давал контраст ~0 — сетка
/// растеризовалась в 1× перед барьером (red-proof прогнан на этом же
/// дереве до коммита п.314).
#[test]
fn t302_barrier_no_longer_kills_detail() {
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let vhs0 = FxInstance::Vhs { rgb_shift: 0, bleed: 0.0, scan_step: 0, jitter: 0.0 };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let out = apply_video_chain(&[grid, vhs0, zoom2], checkerboard(), &ctx());
    let c = contrast(&out);
    assert!(c >= 200.0, "шахматка сквозь K×-барьер, контраст {c:.1}");
}

/// Тест 304 (п.314): правило K. Без нетто-зума хвоста ИЛИ без сжатия выше
/// по цепочке — K=1 (байты пп.311-313 включение K не трогает); сжатие +
/// зум — K>1; соседние барьеры делят один K; слот силы рвёт.
#[test]
fn t304_k_only_with_squeeze_and_zoom() {
    use kt_fx::plan::{PlanOp, plan_chain};
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let vhs = FxInstance::vhs(4, 0.4);
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let zoom05 = FxInstance::geometry_zoom(0.5);
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let k_of = |fx: &[FxInstance]| -> Vec<u32> {
        plan_chain(fx, W, H, 1, W, H, 0.5, None, 90_000, &[])
            .ops
            .iter()
            .filter_map(|op| match op {
                PlanOp::Barrier { res_scale, .. } => Some(*res_scale),
                _ => None,
            })
            .collect()
    };
    assert_eq!(k_of(&[grid.clone(), vhs.clone()]), vec![1], "хвост пуст");
    assert_eq!(k_of(&[grid.clone(), vhs.clone(), mir.clone()]), vec![1], "хвост без зума");
    assert_eq!(k_of(&[grid.clone(), vhs.clone(), zoom05.clone()]), vec![1], "хвост сжимает");
    // squeeze-cap: при входе 1× деталей выше взять неоткуда — K=1, лишний
    // буфер не покупается (источник деталей — ступень декода, t308)
    assert_eq!(k_of(&[vhs.clone(), zoom2.clone()]), vec![1], "сжатия выше нет");
    assert_eq!(k_of(&[grid.clone(), vhs.clone(), zoom2.clone()]), vec![2], "сжатие+зум → K=2");
    // соседние барьеры — один K (ресемплов между барьерами не бывает)
    assert_eq!(k_of(&[grid.clone(), vhs.clone(), FxInstance::blur(8), zoom2.clone()]), vec![2, 2]);
    // t68-цепочка пользователя: хвост после барьеров не зумит — K=1
    assert_eq!(
        k_of(&[mir.clone(), grid.clone(), FxInstance::glow(1.0), FxInstance::vhs(2, 0.5)]),
        vec![1, 1]
    );
    // слот силы рвёт оба аккумулятора: барьер до Mix остаётся 1×
    assert_eq!(
        k_of(&[grid.clone(), vhs.clone(), FxInstance::LayerMix { amount: 0.5, from: 0 }, zoom2]),
        vec![1]
    );
    // байтовый якорь: K=1-цепочка с барьером байт-в-байт с пп.311-313
    // (включение K её не тронуло); при осознанной смене — новый хэш из
    // вывода, как у t26
    let f = run(&[grid, vhs]);
    let hash = xxhash_rust::xxh3::xxh3_64(&f.rgba);
    println!("t304 anchor: 0x{hash:016x}");
    assert_eq!(hash, 0x1a3aad166d0a0c18, "K=1-байты уехали");
}

/// Тест 308 (п.317, Р.46): ступень декода кормит бюджет сжатия. Кадр,
/// пришедший в S× логического размера, — это РЕАЛЬНО стиснутая деталь:
/// squeeze-cap перестаёт резать K там, где сетки выше по цепочке нет, а
/// ведущий барьер (ресемпла до него не было) считается прямо в S×.
#[test]
fn t308_in_scale_feeds_the_squeeze_budget() {
    use kt_fx::plan::{PlanOp, plan_chain};
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let vhs = FxInstance::vhs(4, 0.4);
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let zoom12 = FxInstance::geometry_zoom(1.2);
    let k_of = |fx: &[FxInstance], s: u32| -> Vec<u32> {
        plan_chain(fx, W * s, H * s, s, W, H, 0.5, None, 90_000, &[])
            .ops
            .iter()
            .filter_map(|op| match op {
                PlanOp::Barrier { res_scale, .. } => Some(*res_scale),
                _ => None,
            })
            .collect()
    };
    // ровно тот случай, ради которого делался раунд: зум без сетки выше
    assert_eq!(k_of(&[vhs.clone(), zoom2.clone()], 2), vec![2], "ступень = живая деталь");
    assert_eq!(k_of(&[vhs.clone(), zoom2.clone()], 1), vec![1], "без ступени — прежний K");
    // ведущий барьер исполняется в разрешении кадра, а не по benefit:
    // зум 1.2 сам по себе K не дал бы (квант), но кадр физически 2×
    assert_eq!(k_of(&[vhs.clone(), zoom12], 2), vec![2], "ведущий барьер по физике кадра");
    // сетка выше по цепочке ступень не отменяет и не удваивает сверх потолка
    assert_eq!(k_of(&[grid.clone(), vhs.clone(), zoom2.clone()], 1), vec![2], "прежний путь цел");
    // зеркало на равных краях остаётся старым ядром и на ступени
    assert_eq!(k_of(&[mir, vhs, zoom2], 2), vec![2], "Mirror-ран не ломает сегмент");
}

/// Тест 308b (п.317): гео-ран МЕЖДУ двумя K×-барьерами. Одиночное зеркало
/// маршрутизировалось в Legacy безусловно, а сборка плана требует 1×-краёв
/// — цепочка «сетка → vhs → зеркало → blur → зум» валила debug_assert ещё
/// до Р.46 (латентный дефект Р.45). Правило: Legacy только при k_in==k_out.
#[test]
fn t308b_mirror_run_between_k_segments() {
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let vhs = FxInstance::vhs(4, 0.4);
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let out = run(&[grid, vhs, mir, FxInstance::blur(8), zoom2]);
    assert_eq!((out.w, out.h), (W, H), "chain must land in Quality");
}

/// Люма пикселя (та же формула 77/150/29, что в tap-ядрах).
fn luma(f: &Frame, x: u32, y: u32) -> f64 {
    let i = ((y * f.w + x) * 4) as usize;
    (77.0 * f.rgba[i] as f64 + 150.0 * f.rgba[i + 1] as f64 + 29.0 * f.rgba[i + 2] as f64) / 256.0
}

/// Тест 330 (Р.47 Кв2): новые поточечные — ColorGrade/Duotone/Posterize/
/// Vignette. (а) внутренний поточечный поглощается в гео-ран (план длиной
/// 1); (б) смысл каждого на барьерной позиции; (в) Vignette считает
/// координату СВОЕЙ СТАДИИ, а не выхода: зум после неё срезает углы.
#[test]
fn t330_new_pointwise_absorb_and_meaning() {
    use kt_fx::plan::{PlanOp, plan_chain};
    let grid2 = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let zoom2 = FxInstance::geometry_zoom(2.0);

    // (а) поглощение: каждый новый поточечный между гео-шагами — ран ОДИН
    for f in [
        FxInstance::color_grade(),
        FxInstance::duotone(),
        FxInstance::posterize(),
        FxInstance::vignette(),
    ] {
        let fx = vec![grid2.clone(), f.clone(), zoom2.clone()];
        let p = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[]);
        assert_eq!(p.ops.len(), 1, "{}: must absorb into the run", f.tag());
        assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }), "{}", f.tag());
    }

    // (б-1) ColorGrade: насыщенность 0 — кадр обесцвечен (r==g==b)
    let gray = run(&[FxInstance::ColorGrade {
        brightness: 0.0,
        contrast: 0.0,
        saturation: 0.0,
        hue: 0.0,
    }]);
    let src = test_frame();
    assert_ne!(gray.rgba, src.rgba, "saturation 0 must change a colored frame");
    for px in gray.rgba.chunks_exact(4) {
        assert!(px[0] == px[1] && px[1] == px[2], "saturation 0 must produce gray: {px:?}");
    }

    // (б-2) Duotone: две краски — на выходе только их отрезок
    let duo = run(&[FxInstance::Duotone {
        shadow_r: 0,
        shadow_g: 0,
        shadow_b: 255,
        high_r: 255,
        high_g: 0,
        high_b: 0,
        amount: 1.0,
    }]);
    for px in duo.rgba.chunks_exact(4) {
        assert_eq!(px[1], 0, "duotone (blue..red) leaves no green");
        let sum = px[0] as i32 + px[2] as i32;
        assert!((254..=256).contains(&sum), "off the shadow..high segment: {px:?}");
    }

    // (б-3) Posterize: не больше levels уникальных значений на канал
    let post = run(&[FxInstance::Posterize { levels: 4 }]);
    for c in 0..3 {
        let uniq: std::collections::BTreeSet<u8> =
            post.rgba.chunks_exact(4).map(|px| px[c]).collect();
        assert!(uniq.len() <= 4, "channel {c}: {} levels out of 4", uniq.len());
        let before: std::collections::BTreeSet<u8> =
            src.rgba.chunks_exact(4).map(|px| px[c]).collect();
        assert!(before.len() > 4, "source must have had detail to lose");
    }

    // (б-4) Vignette: углы темнее, центр нетронут (radius > r в центре).
    // Кадр — ровный серый: у test_frame угол (0,0) сам чёрный, и темнить
    // там нечего.
    // radius 0.5: угол кадра (r=1) гаснет в ноль, а точка стадии под
    // углом ЗУМА (r≈0.48) остаётся внутри радиуса — контраст (в) полный
    let vig = FxInstance::Vignette { strength: 1.0, radius: 0.5, softness: 0.3 };
    let flat = || {
        let mut rgba = vec![200u8; (W * H * 4) as usize];
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
        Frame { w: W, h: H, rgba, has_alpha: false }
    };
    let v = apply_video_chain(std::slice::from_ref(&vig), flat(), &ctx());
    let (cx, cy) = (W / 2, H / 2);
    assert!(luma(&v, 0, 0) < 100.0, "corner must darken: {}", luma(&v, 0, 0));
    assert_eq!(
        v.rgba[((cy * W + cx) * 4) as usize..((cy * W + cx) * 4 + 4) as usize],
        [200, 200, 200, 255],
        "inside radius the pixel is untouched"
    );

    // (в) координата стадии: [grid, vig, zoom2] виньетирует ДО зума —
    // зум срезает тёмные углы, и угол выхода светлее прямого виньетажа.
    let direct = apply_video_chain(std::slice::from_ref(&vig), flat(), &ctx());
    let fused = apply_video_chain(&[grid2.clone(), vig, zoom2.clone()], flat(), &ctx());
    assert!(
        luma(&fused, 0, 0) > luma(&direct, 0, 0) + 20.0,
        "vignette must use STAGE coords (zoom crops the dark corners): fused {} direct {}",
        luma(&fused, 0, 0),
        luma(&direct, 0, 0)
    );
}

/// Тест 331 (Р.47 Кв2): CornerPin. Дефолт — тождество байт-в-байт (шаг
/// пропущен); сжатие в правую половину кладёт маркер по формуле и
/// оставляет левую прозрачной; перспективный квад двигает угловой маркер
/// к назначенному углу; вырожденный квад — прозрачность, детерминированно;
/// в серии с гео CornerPin остаётся ОДНИМ раном.
#[test]
fn t331_corner_pin() {
    use kt_fx::plan::{PlanOp, plan_chain};
    // (1) дефолт: identity байт-в-байт, альфа не объявляется
    let out = run(&[FxInstance::corner_pin()]);
    let src = test_frame();
    assert_eq!(out.rgba, src.rgba, "default corners are a byte-exact identity");
    assert!(!out.has_alpha, "skipped step must not declare alpha");

    // маркерный кадр: белое поле, чёрный блок 4×4 в центре
    let marker = || {
        let mut m = Frame { w: W, h: H, rgba: vec![255u8; (W * H * 4) as usize], has_alpha: false };
        for y in H / 2 - 2..H / 2 + 2 {
            for x in W / 2 - 2..W / 2 + 2 {
                let i = ((y * W + x) * 4) as usize;
                m.rgba[i] = 0;
                m.rgba[i + 1] = 0;
                m.rgba[i + 2] = 0;
            }
        }
        m
    };
    let centroid = |f: &Frame| {
        let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0.0f64);
        for y in 0..f.h {
            for x in 0..f.w {
                let i = ((y * f.w + x) * 4) as usize;
                if f.rgba[i + 3] == 255 && f.rgba[i] < 128 {
                    sx += x as f64;
                    sy += y as f64;
                    n += 1.0;
                }
            }
        }
        assert!(n > 0.0, "marker must survive");
        (sx / n, sy / n)
    };

    // (2) сжатие в правую половину: uv-центр → x = 0.75·W
    let right = FxInstance::CornerPin {
        x0: 0.5,
        y0: 0.0,
        x1: 1.0,
        y1: 0.0,
        x2: 0.5,
        y2: 1.0,
        x3: 1.0,
        y3: 1.0,
    };
    let o = apply_video_chain(std::slice::from_ref(&right), marker(), &ctx());
    assert!(o.has_alpha, "corner pin declares alpha");
    for y in 0..H {
        for x in 0..W / 2 - 1 {
            assert_eq!(o.rgba[((y * W + x) * 4 + 3) as usize], 0, "left half is off-quad");
        }
    }
    let (mx, my) = centroid(&o);
    assert!(
        (mx - 0.75 * W as f64).abs() <= 2.0 && (my - 0.5 * H as f64).abs() <= 2.0,
        "marker must land at (0.75W, 0.5H): got ({mx:.1}, {my:.1})"
    );

    // (3) перспектива: TL-угол утянут внутрь — угловой маркер едет за ним
    let mut tlm = Frame { w: W, h: H, rgba: vec![255u8; (W * H * 4) as usize], has_alpha: false };
    for y in 0..4u32 {
        for x in 0..4u32 {
            let i = ((y * W + x) * 4) as usize;
            tlm.rgba[i] = 0;
            tlm.rgba[i + 1] = 0;
            tlm.rgba[i + 2] = 0;
        }
    }
    let persp = FxInstance::CornerPin {
        x0: 0.25,
        y0: 0.25,
        x1: 1.0,
        y1: 0.0,
        x2: 0.0,
        y2: 1.0,
        x3: 1.0,
        y3: 1.0,
    };
    let o = apply_video_chain(std::slice::from_ref(&persp), tlm, &ctx());
    let (mx, my) = centroid(&o);
    assert!(
        (mx - 0.25 * W as f64).abs() <= 4.0 && (my - 0.25 * H as f64).abs() <= 4.0,
        "TL marker must follow the pinned corner: got ({mx:.1}, {my:.1})"
    );

    // (4) вырожденный квад: прозрачно и детерминированно
    let degen = FxInstance::CornerPin {
        x0: 0.3,
        y0: 0.3,
        x1: 0.3,
        y1: 0.3,
        x2: 0.3,
        y2: 0.3,
        x3: 0.3,
        y3: 0.3,
    };
    let a = apply_video_chain(std::slice::from_ref(&degen), marker(), &ctx());
    let b = apply_video_chain(std::slice::from_ref(&degen), marker(), &ctx());
    assert!(a.rgba.chunks_exact(4).all(|px| px[3] == 0), "degenerate quad is transparent");
    assert_eq!(a.rgba, b.rgba, "and deterministic");

    // (5) серия с гео — один ран
    let grid2 = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let fx = vec![grid2, right, zoom2];
    let p = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert_eq!(p.ops.len(), 1, "corner pin is geo: the series stays one run");
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
}

/// Тест 332 (Р.47 Кв3): смысл шести барьеров + альфа-контракт тени.
#[test]
fn t332_barrier_semantics() {
    use kt_fx::video::{FxScratch, apply_barrier};
    let mut scratch = FxScratch::default();

    // Sharpen: локальный контраст растёт
    let sharp = run(&[FxInstance::Sharpen { amount: 2.0, radius: 4 }]);
    assert!(
        contrast(&sharp) > contrast(&test_frame()) * 1.2,
        "sharpen must raise local contrast: {} vs {}",
        contrast(&sharp),
        contrast(&test_frame())
    );

    // Pixelate: каждая ячейка 12×12 константна
    let pix = run(&[FxInstance::Pixelate { size: 12 }]);
    for cy in 0..H / 12 {
        for cx in 0..W / 12 {
            let base = ((cy * 12 * W + cx * 12) * 4) as usize;
            for y in cy * 12..(cy + 1) * 12 {
                for x in cx * 12..(cx + 1) * 12 {
                    let i = ((y * W + x) * 4) as usize;
                    assert_eq!(
                        pix.rgba[i..i + 3],
                        pix.rgba[base..base + 3],
                        "pixelate cell must be flat at ({x},{y})"
                    );
                }
            }
        }
    }

    // ChromaticAberration: центр нетронут, у края каналы разъехались
    let ca = run(&[FxInstance::ChromaticAberration { amount: 0.04 }]);
    let src = test_frame();
    let c = (((H / 2) * W + W / 2) * 4) as usize;
    assert_eq!(ca.rgba[c..c + 4], src.rgba[c..c + 4], "chroma: center pixel is a fixed point");
    let edge = ((H / 2 * W + 2) * 4) as usize;
    assert_ne!(ca.rgba[edge], src.rgba[edge], "chroma: red must shift near the edge");

    // RadialBlur: центр — неподвижная точка луча, край смазан
    let rb =
        run(&[FxInstance::RadialBlur { amount: 0.8, center_x: 0.0, center_y: 0.0, spin: false }]);
    let mut edge_moved = 0;
    for x in 0..W {
        let i = ((H / 4 * W + x) * 4) as usize;
        if rb.rgba[i] != src.rgba[i] {
            edge_moved += 1;
        }
    }
    assert!(edge_moved > (W / 4) as usize, "radial blur must smear off-center rows");

    // DropShadow на непрозрачном входе — passthrough байт-в-байт
    let ds = run(&[FxInstance::drop_shadow()]);
    assert_eq!(ds.rgba, src.rgba, "opaque input: drop shadow is a byte passthrough");

    // [greenscreen, drop_shadow]: в вырезанной зоне на +offset от
    // непрозрачной кромки появляется ТЕНЬ (альфа > 0, цвет — tint)
    let keyed = apply_barrier(
        &FxInstance::greenscreen([0, 255, 0], 70, 25),
        test_frame(),
        &ctx(),
        &mut scratch,
        1,
    );
    assert!(keyed.has_alpha);
    let shadowed = apply_barrier(
        &FxInstance::DropShadow {
            offset_x: 6,
            offset_y: 6,
            radius: 2,
            opacity: 1.0,
            tint_r: 0,
            tint_g: 0,
            tint_b: 0,
        },
        keyed,
        &ctx(),
        &mut scratch,
        1,
    );
    // зелёный угол выключен ключом; кромка непрозрачной зоны — на
    // x == W/2 (и y == H/2); точка чуть правее+ниже кромки в вырезанной
    // зоне обязана получить тень
    let px = (W / 2 + 4, H / 2 + 4);
    let i = ((px.1 * W + px.0) * 4) as usize;
    assert!(
        shadowed.rgba[i + 3] > 32,
        "shadow must appear in the keyed-out zone: alpha {}",
        shadowed.rgba[i + 3]
    );
    assert!(
        shadowed.rgba[i] < 32 && shadowed.rgba[i + 1] < 32 && shadowed.rgba[i + 2] < 32,
        "shadow colour is the tint: {:?}",
        &shadowed.rgba[i..i + 3]
    );

    // Halftone: выход двухуровневый; в тёмной зоне точек больше
    let ht = run(&[FxInstance::Halftone { size: 8, angle: 45.0 }]);
    for pxl in ht.rgba.chunks_exact(4) {
        assert!(
            (pxl[0] == 0 || pxl[0] == 255) && pxl[0] == pxl[1] && pxl[1] == pxl[2],
            "halftone output is binary mono: {pxl:?}"
        );
    }
    let dark_dots =
        ht.rgba.chunks_exact(4).take((W * H / 4) as usize).filter(|p| p[0] == 0).count();
    let light_dots =
        ht.rgba.chunks_exact(4).skip((W * H * 3 / 4) as usize).filter(|p| p[0] == 0).count();
    assert!(
        dark_dots > light_dots,
        "darker area must grow bigger dots: {dark_dots} vs {light_dots}"
    );
}

/// Тест 333 (Р.47 Кв4): Warp-прим — LensDistort/Twirl/Ripple/
/// Kaleidoscope/Cylinder3d. Нейтральные параметры — пропуск шага
/// (identity байт); смысл каждого режима; PerPixel-суперсемплинг;
/// детерминизм; серия варпов — один ран.
#[test]
fn t333_warp_prims() {
    use kt_fx::geom::SsSpec;
    use kt_fx::plan::{PlanOp, plan_chain};
    let src = test_frame();

    // (1) нейтральные параметры каждого варпа — тождество байт-в-байт
    for f in [
        FxInstance::LensDistort { k1: 0.0, k2: 0.0 },
        FxInstance::Twirl { angle: 0.0, radius: 0.8 },
        FxInstance::Ripple { amount: 0.0, freq: 10.0, phase: 0.0 },
        FxInstance::Cylinder3d { curvature: 0.0 },
    ] {
        let out = run(std::slice::from_ref(&f));
        assert_eq!(out.rgba, src.rgba, "{}: neutral params are a byte identity", f.tag());
    }

    // (2) PerPixel-суперсемплинг включён у программы с варпом
    let twirl = FxInstance::twirl();
    match build_geom_exec(&[&twirl], W, H, W, H, 0.5, None, 90_000, 1, 1) {
        GeomExec::Fused(prog) => {
            assert_eq!(prog.ss, SsSpec::PerPixel, "warp jacobian varies per pixel");
        }
        GeomExec::Legacy => panic!("warp must go fused"),
    }

    // (3) детерминизм: два прогона байт-в-байт
    let hard = [
        FxInstance::Twirl { angle: 540.0, radius: 1.2 },
        FxInstance::Ripple { amount: 0.3, freq: 30.0, phase: 137.0 },
    ];
    assert_eq!(run(&hard).rgba, run(&hard).rgba, "warp chain must be deterministic");

    // (4) twirl: центр — неподвижная точка; точка на радиусе повёрнута.
    // Маркерный кадр: белое поле, чёрный блок справа от центра.
    let marker_at = |mx: u32, my: u32| {
        let mut f = Frame { w: W, h: H, rgba: vec![255u8; (W * H * 4) as usize], has_alpha: false };
        for y in my.saturating_sub(1)..(my + 2).min(H) {
            for x in mx.saturating_sub(1)..(mx + 2).min(W) {
                let i = ((y * W + x) * 4) as usize;
                f.rgba[i] = 0;
                f.rgba[i + 1] = 0;
                f.rgba[i + 2] = 0;
            }
        }
        f
    };
    let centroid = |f: &Frame| {
        let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0.0f64);
        for y in 0..f.h {
            for x in 0..f.w {
                let i = ((y * f.w + x) * 4) as usize;
                if f.rgba[i + 3] == 255 && f.rgba[i] < 128 {
                    sx += x as f64;
                    sy += y as f64;
                    n += 1.0;
                }
            }
        }
        assert!(n > 0.0, "marker must survive");
        (sx / n, sy / n)
    };
    // центр: маркер в центре кадра не двигается
    let tw = FxInstance::Twirl { angle: 180.0, radius: 1.0 };
    let out = apply_video_chain(std::slice::from_ref(&tw), marker_at(W / 2, H / 2), &ctx());
    let (cx, cy) = centroid(&out);
    assert!(
        (cx - (W / 2) as f64).abs() <= 1.5 && (cy - (H / 2) as f64).abs() <= 1.5,
        "twirl center is a fixed point: ({cx:.1}, {cy:.1})"
    );
    // точка на полурадиусе: обратная карта поворачивает НА θ — маркер
    // видимо уезжает с исходного места
    let out = apply_video_chain(std::slice::from_ref(&tw), marker_at(W / 2 + 9, H / 2), &ctx());
    let (mx, my) = centroid(&out);
    let moved = ((mx - (W / 2 + 9) as f64).powi(2) + (my - (H / 2) as f64).powi(2)).sqrt();
    assert!(moved > 4.0, "twirl must move an off-center marker: ({mx:.1}, {my:.1})");

    // (5) lens: сдвиг маркера по формуле. Обратная карта: выходной r
    // сэмплит источник в r·(1+k1·r²) — маркер источника на радиусе rs
    // виден на выходе там, где rs = r·(1+k1·r²).
    let k1 = 0.5f64;
    let lens = FxInstance::LensDistort { k1: k1 as f32, k2: 0.0 };
    let rr = (H / 2 - 4) as f64; // радиус маркера в px (по Y, короткая ось)
    let rn = rr / (H as f64 / 2.0); // нормализованный
    let out =
        apply_video_chain(std::slice::from_ref(&lens), marker_at(W / 2, H / 2 - rr as u32), &ctx());
    let (_, my) = centroid(&out);
    // решить rs = r·(1+k1·r²) относительно r (выходной радиус маркера)
    let mut r_out = rn;
    for _ in 0..40 {
        r_out = rn / (1.0 + k1 * r_out * r_out);
    }
    let expect_y = H as f64 / 2.0 - r_out * H as f64 / 2.0;
    assert!(
        (my - expect_y).abs() <= 1.5,
        "lens marker at formula position: got y {my:.1}, expect {expect_y:.1}"
    );

    // (6) kaleidoscope: точки первого клина остаются собой (≤1 LSB)
    let kal = FxInstance::Kaleidoscope { segments: 6, angle: 0.0 };
    let out = apply_video_chain(std::slice::from_ref(&kal), test_frame(), &ctx());
    let tau = std::f64::consts::TAU / 6.0;
    let mut checked = 0;
    for y in 0..H {
        for x in 0..W {
            let (dx, dy) = (x as f64 + 0.5 - W as f64 / 2.0, y as f64 + 0.5 - H as f64 / 2.0);
            let phi = dy.atan2(dx);
            let r = (dx * dx + dy * dy).sqrt() / (H as f64 / 2.0);
            // глубоко внутри первой ПОЛОВИНЫ клина (там m == q — тождество)
            if r < 0.9 && phi > 0.1 && phi < tau / 2.0 - 0.1 {
                let i = ((y * W + x) * 4) as usize;
                for c in 0..4 {
                    let d = (out.rgba[i + c] as i32 - src.rgba[i + c] as i32).abs();
                    assert!(d <= 1, "first wedge must be itself at ({x},{y}): d {d}");
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 20, "wedge probe must cover pixels: {checked}");

    // (7) cylinder: края и центральный столбец на месте, сжатие к краям
    let cyl = FxInstance::Cylinder3d { curvature: 0.9 };
    let out = apply_video_chain(std::slice::from_ref(&cyl), marker_at(W / 2, H / 4), &ctx());
    let (mx, _) = centroid(&out);
    assert!((mx - (W / 2) as f64).abs() <= 1.5, "cylinder center column fixed: {mx:.1}");
    // маркер на 3/4 ширины уезжает НАРУЖУ (сжатие у краёв: источник
    // там семплится ближе к центру, картинка распухает к краю)
    let out = apply_video_chain(std::slice::from_ref(&cyl), marker_at(3 * W / 4, H / 2), &ctx());
    let (mx, _) = centroid(&out);
    assert!(mx > (3 * W / 4) as f64 + 2.0, "cylinder must push mid-side content outward: {mx:.1}");

    // (8) серия варпов + зум — ОДИН ран
    let fx = vec![FxInstance::lens_distort(), FxInstance::twirl(), FxInstance::geometry_zoom(2.0)];
    let p = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert_eq!(p.ops.len(), 1, "warp series stays one fused run");
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
}

/// Тест 519 (Р.53, п.417): правило альфы — свёртка, а не «последний
/// объявивший выигрывает» среди ВСЕХ шагов.
///
/// `EventGrid` объявлял себя убийцей альфы, хотя тайл — это
/// пересемплированный вход и альфа входа живёт в нём по-прежнему. Из-за
/// этого титр или выход гринскрина под сеткой ложились в композит
/// НЕПРОЗРАЧНЫМ прямоугольником.
#[test]
fn t519_alpha_rule_folds_produces_and_destroys() {
    use kt_fx::geom::build_geom_exec;
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let gs = FxInstance::Greenscreen {
        key_r: 0,
        key_g: 255,
        key_b: 0,
        tolerance: 80,
        softness: 20,
        spill: 0.0,
    };
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };

    let prog = |fx: &[FxInstance]| {
        let refs: Vec<&FxInstance> = fx.iter().collect();
        match build_geom_exec(&refs, W, H, W, H, 0.5, None, 90_000, 1, 1) {
            GeomExec::Fused(p) => p,
            _ => panic!("серия обязана слиться"),
        }
    };

    // сетка одна: про альфу ей сказать нечего — берётся альфа ИСТОЧНИКА
    let p = prog(&[grid.clone(), mir.clone()]);
    assert!(!p.has_alpha_known, "EventGrid снова объявляет альфу за источник");

    // гринскрин СОЗДАЁТ альфу, и сетка за ним её не стирает
    let p = prog(&[gs.clone(), grid.clone()]);
    assert!(p.has_alpha_known && p.has_alpha, "сетка стёрла альфу гринскрина");

    // порядок наоборот — тот же ответ
    let p = prog(&[grid.clone(), gs.clone(), mir.clone()]);
    assert!(p.has_alpha_known && p.has_alpha);
}

/// Тест 520 (Р.53, п.417): следствие В ПИКСЕЛЯХ. Кадр с прозрачной
/// половиной проходит сетку и остаётся прозрачным там, где был.
#[test]
fn t520_grid_keeps_transparent_pixels_transparent() {
    let mut f = test_frame();
    f.has_alpha = true;
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            if x < W / 2 {
                f.rgba[i + 3] = 0;
            }
        }
    }
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let out = apply_video_chain(&[grid], f, &ctx());
    assert!(out.has_alpha, "кадр объявлен непрозрачным — композит зальёт его плитой");
    // левая четверть каждого тайла приходит из прозрачной половины входа
    let a = |x: u32, y: u32| out.rgba[(((y * W + x) * 4) + 3) as usize];
    assert_eq!(a(2, 4), 0, "прозрачный пиксель стал непрозрачным");
    assert_eq!(a(W / 2 + 2, 4), 0, "второй тайл потерял прозрачность входа");
    assert_eq!(a(W / 4 + 2, 4), 255, "непрозрачная половина входа испортилась");
}

/// Тест 522 (Р.53, п.418): размытие кадра С АЛЬФОЙ не тянет цвет
/// прозрачных пикселей в видимые.
///
/// Без премультипликации мип смешивает RGB прозрачных соседей (у титра
/// это чёрный фон вокруг глифа) с видимыми, и по краю появляется тёмный
/// ореол. Проверяется СЛЕДСТВИЕ: яркий непрозрачный пиксель рядом с
/// прозрачной чёрной областью после размытия не темнеет.
#[test]
fn t522_blur_of_an_alpha_frame_has_no_dark_halo() {
    // белая непрозрачная полоса в чёрном ПРОЗРАЧНОМ поле
    let mut f = Frame { w: W, h: H, rgba: vec![0u8; (W * H * 4) as usize], has_alpha: true };
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            let inside = (W / 4..W * 3 / 4).contains(&x);
            if inside {
                f.rgba[i] = 255;
                f.rgba[i + 1] = 255;
                f.rgba[i + 2] = 255;
                f.rgba[i + 3] = 255;
            }
        }
    }
    let out = apply_video_chain(&[FxInstance::blur(8)], f, &ctx());
    // там, где альфа ещё заметна, цвет обязан остаться белым: чёрный из
    // прозрачного поля в него не затёк
    let mut worst = 255u8;
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            if out.rgba[i + 3] >= 200 {
                worst = worst.min(out.rgba[i]);
            }
        }
    }
    assert!(worst >= 250, "цвет прозрачных соседей затёк в видимые: минимум {worst} вместо ~255");
}

/// Тест 523 (Р.53, п.418): гейт по `has_alpha` — НЕПРОЗРАЧНЫЙ кадр
/// проходит те же ядра БАЙТ В БАЙТ.
///
/// Премультипликация при `a == 255` тождественна, и именно на этом стоит
/// обещание «ни один голден не двинулся»: правило включается только там,
/// где альфа есть.
#[test]
fn t523_opaque_frames_are_byte_identical() {
    for fx in [
        FxInstance::blur(8),
        FxInstance::blur(24),
        FxInstance::Glow {
            strength: 1.0,
            radius: 4,
            threshold: 0,
            tint_r: 255,
            tint_g: 255,
            tint_b: 255,
        },
        FxInstance::Glow {
            strength: 0.7,
            radius: 9,
            threshold: 40,
            tint_r: 200,
            tint_g: 120,
            tint_b: 255,
        },
    ] {
        // тот же кадр, но объявленный с альфой: все пиксели a == 255
        let mut with = test_frame();
        with.has_alpha = true;
        let a = apply_video_chain(std::slice::from_ref(&fx), test_frame(), &ctx());
        let b = apply_video_chain(std::slice::from_ref(&fx), with, &ctx());
        assert_eq!(a.rgba, b.rgba, "{fx:?}: гейт альфы сдвинул байты на непрозрачном кадре");
    }
}
