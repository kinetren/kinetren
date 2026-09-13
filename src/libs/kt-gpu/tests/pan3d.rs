//! Тест 187 (Р.29, ред. Р.44): Pan 3D. Свойства:
//! - тождество: дефолтные параметры + дефолтная камера «50 мм» на
//!   авто-дистанции дают кадр 1:1 (в допуске f32-матрицы);
//! - геометрия: сдвиг pos_x перемещает маркер ровно на столько px;
//! - масштаб < 1 открывает прозрачные поля — снизу просвечивает фон;
//! - детерминизм на устройстве (два движка байт-в-байт);
//! - с Р.44 CPU — снова ЭТАЛОН и для Pan3d (обратная гомография в слитом
//!   гео-проходе): каждый GPU-кадр сверяется с compose_plan_cpu в GEOM.

use std::sync::Arc;

use kt_cache::{BurstCache, CacheConfig, SourceId};
use kt_engine::{Cam3d, FramePlan, LayerSrc, PlanLayer, VideoGpuExec, compose_plan_cpu};
use kt_gpu::{FrameTol, assert_frames_close, test_engine};
use kt_media::VideoFrame;
use kt_model::{CameraParams, FxInstance};
use kt_time::Tick;

const W: u32 = 64;
const H: u32 = 36;

fn frame_with_marker(mx: u32, my: u32) -> Arc<VideoFrame> {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            let v = if (x, y) == (mx, my) { 255 } else { 24 + ((x + y) % 8) as u8 };
            rgba[i] = v;
            rgba[i + 1] = v;
            rgba[i + 2] = v;
            rgba[i + 3] = 255;
        }
    }
    Arc::new(VideoFrame {
        pts: Tick(0),
        width: W,
        height: H,
        rgba: rgba.into_boxed_slice(),
        has_alpha: false,
    })
}

fn pan3d(pos_x: f64, scale: f64) -> FxInstance {
    FxInstance::Pan3d {
        pos_x,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 0.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale,
        camera: None,
    }
}

fn layer<'a>(
    fx: &'a [FxInstance],
    frame: &Arc<VideoFrame>,
    id: u64,
) -> PlanLayer<'a> {
    PlanLayer::One {
        src: LayerSrc {
            source: SourceId(id),
            cache_idx: 0,
            frame: Some(frame.clone()),
            fx: std::borrow::Cow::Borrowed(fx),
            fx_hash: id,
            u: 0.5,
            cam3d: Some(Cam3d { cam: CameraParams::default(), world_scale: 1.0 }),
            blend: None,
        },
        alpha_k: 4096,
        scale_alpha: false,
        fade: None,
    }
}

fn run(engine: &dyn VideoGpuExec, layers: Vec<PlanLayer<'_>>) -> Vec<u8> {
    let plan = FramePlan { t: Tick(90_000), out_w: W, out_h: H, layers, memoize_postfx: true };
    let mut out = Vec::new();
    let complete = engine.execute_into(&plan, &mut out).expect("GPU execute");
    assert!(complete);
    // Р.44: CPU-эталон считает ТОТ ЖЕ план (Pan3d — обратная гомография);
    // сверка на каждом прогоне — GPU-кадры pan3d больше не бесхозные
    let cache = BurstCache::new(CacheConfig::default());
    let mut cpu = Vec::new();
    let complete_cpu = compose_plan_cpu(&plan, &cache, &mut cpu);
    assert_eq!(complete_cpu, complete, "complete CPU vs GPU");
    assert_frames_close(&cpu, &out, W, H, FrameTol::GEOM, "pan3d-cpu-vs-gpu");
    out
}

fn brightest(rgba: &[u8]) -> (u32, u32) {
    let mut best = (0u32, 0u32);
    let mut bv = 0u32;
    for (p, px) in rgba.chunks_exact(4).enumerate() {
        let v = px[0] as u32 + px[1] as u32 + px[2] as u32;
        if v > bv {
            bv = v;
            best = (p as u32 % W, p as u32 / W);
        }
    }
    best
}

#[test]
fn t187_pan3d_identity_shift_scale() {
    let Some(e) = test_engine() else { return };
    let f = frame_with_marker(20, 12);

    // тождество: дефолтный Pan3d == кадр без эффекта
    let fx_id = [pan3d(0.0, 1.0)];
    let ident = run(e.as_ref(), vec![layer(&fx_id, &f, 1)]);
    let none: [FxInstance; 0] = [];
    let plain = run(e.as_ref(), vec![layer(&none, &f, 2)]);
    assert_frames_close(&plain, &ident, W, H, FrameTol::BILINEAR, "pan3d-identity");

    // сдвиг: маркер уехал ровно на pos_x (±1 px растеризации)
    let fx_shift = [pan3d(8.0, 1.0)];
    let shifted = run(e.as_ref(), vec![layer(&fx_shift, &f, 3)]);
    let (mx, my) = brightest(&shifted);
    assert!(
        (mx as i64 - 28).abs() <= 1 && (my as i64 - 12).abs() <= 1,
        "маркер (20,12) при pos_x=8 обязан оказаться у (28,12), а он у ({mx},{my})"
    );

    // масштаб 0.5: углы кадра прозрачны (виден фон), центр — контент
    let fx_half = [pan3d(0.0, 0.5)];
    let bg = frame_with_marker(999, 999); // без маркера, серый
    let out = run(e.as_ref(), vec![layer(&none, &bg, 4), layer(&fx_half, &f, 5)]);
    let px = |x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        (out[i], out[i + 3])
    };
    // центр: слой поверх (маркер тут не в центре — просто контент слоя)
    let (_, ca) = px(W / 2, H / 2);
    assert_eq!(ca, 255, "центр непрозрачен");
    // угол: квад ужат вдвое — угол прозрачен, виден нижний слой (~24+)
    let (cr, _) = px(1, 1);
    assert!(cr >= 24, "в углу просвечивает фон, а не чёрная пустота: {cr}");
    // и маркер слоя стоит в ужатой позиции: (20-32)*0.5+32 = 26, (12-18)*0.5+18 = 15
    let (mx2, my2) = brightest(&out);
    assert!(
        (mx2 as i64 - 26).abs() <= 1 && (my2 as i64 - 15).abs() <= 1,
        "маркер при scale 0.5 у (26,15), а он у ({mx2},{my2})"
    );
}

#[test]
fn t187b_pan3d_deterministic_on_device() {
    let Some(e1) = test_engine() else { return };
    let Some(e2) = test_engine() else { return };
    let f = frame_with_marker(20, 12);
    let fx = [pan3d(3.0, 0.8)];
    let a = run(e1.as_ref(), vec![layer(&fx, &f, 1)]);
    let b = run(e2.as_ref(), vec![layer(&fx, &f, 1)]);
    assert_eq!(a, b, "Pan3d недетерминирован на одном устройстве");
}
