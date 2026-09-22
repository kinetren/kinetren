//! Тесты 181–183 (Р.29): конформанс GPU-исполнителя против CPU-эталона.
//!
//! - t181: каждый эффект и цепочки — допуски по видам арифметики
//!   (целочисленные ядра байт-в-байт, билинейные ±LSB, пороговые с
//!   законными выбросами у границы порога);
//! - t182: композит-сцены (over/scale/solo/dissolve/недостающие слои) —
//!   байт-в-байт (композит целочисленный);
//! - t183: детерминизм на одном устройстве + «грязный» пул текстур.
//!
//! Без wgpu-адаптера — skip с печатью; KT_REQUIRE_GPU=1 требует адаптер.

use std::sync::Arc;

use kt_cache::{BurstCache, CacheConfig, SourceId};
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec, compose_plan_cpu};
use kt_gpu::{FrameTol, assert_frames_close, test_engine};
use kt_media::VideoFrame;
use kt_model::{CamKeyframe, Dir, FxInstance, IrisShape, PanMode, TransitionKind};
use kt_time::Tick;

const W: u32 = 64;
const H: u32 = 36;

/// Детерминированный кадр: градиенты + зелёный угол (как в t26 kt-fx),
/// опционально с градиентной альфой.
fn frame(w: u32, h: u32, alpha: bool, tint: u8) -> Arc<VideoFrame> {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            if x > w / 2 && y > h / 2 {
                rgba[i] = 0;
                rgba[i + 1] = 255;
                rgba[i + 2] = 0;
            } else {
                rgba[i] = (x * 4) as u8 ^ tint;
                rgba[i + 1] = (y * 7) as u8;
                rgba[i + 2] = ((x + y) * 3) as u8;
            }
            rgba[i + 3] = if alpha { ((x * 5 + y * 3) % 256) as u8 } else { 255 };
        }
    }
    Arc::new(VideoFrame {
        pts: Tick(0),
        width: w,
        height: h,
        rgba: rgba.into_boxed_slice(),
        has_alpha: alpha,
    })
}

struct Src {
    source: SourceId,
    frame: Option<Arc<VideoFrame>>,
    fx: Vec<FxInstance>,
    fx_hash: u64,
    u: f64,
    cam3d: Option<kt_engine::Cam3d>,
    /// Ступень декода слоя (Р.46): кадр в src_scale раз крупнее Quality.
    src_scale: u32,
}

impl Src {
    fn new(id: u64, frame: Option<Arc<VideoFrame>>, fx: Vec<FxInstance>) -> Src {
        // fx_hash обязан быть уникален на набор параметров: CPU-путь
        // мемоизирует пост-FX по нему (как fx_chain_hash в kt-engine)
        let repr = format!("{fx:?}|{id}");
        Src {
            source: SourceId(id),
            frame,
            fx,
            fx_hash: xxhash_rust::xxh3::xxh3_64(repr.as_bytes()),
            u: 0.5,
            cam3d: None,
            src_scale: 1,
        }
    }

    fn layer(&self) -> LayerSrc<'_> {
        LayerSrc {
            source: self.source,
            cache_idx: 0,
            // серийный номер содержимого (Р.53, п.409): в продукте его
            // выдаёт BurstCache, здесь — хэш самих байт кадра, чтобы
            // разные кадры не делили текстуру в UploadCache
            content: self.frame.as_ref().map(|f| xxhash_rust::xxh3::xxh3_64(&f.rgba)).unwrap_or(0),
            src_scale: self.src_scale,
            input: kt_engine::video_plan::LayerInput::Frame(self.frame.clone()),
            fx: std::borrow::Cow::Borrowed(&self.fx[..]),
            fx_hash: self.fx_hash,
            u: self.u,
            cam3d: self.cam3d,
            blend: None,
        }
    }
}

fn one(src: &Src, alpha_k: u32, scale_alpha: bool) -> PlanLayer<'_> {
    PlanLayer::One { src: src.layer(), alpha_k, scale_alpha, fade: None }
}

fn plan<'a>(t: i64, layers: Vec<PlanLayer<'a>>) -> FramePlan<'a> {
    FramePlan { t: Tick(t), out_w: W, out_h: H, layers, memoize_postfx: true }
}

/// CPU против GPU на одном плане; возвращает (cpu, gpu, complete_cpu,
/// complete_gpu).
fn run_both(engine: &dyn VideoGpuExec, p: &FramePlan<'_>) -> (Vec<u8>, Vec<u8>, bool, bool) {
    let cache = BurstCache::new(CacheConfig::default());
    let mut cpu = Vec::new();
    let c_cpu = compose_plan_cpu(p, &cache, &mut cpu);
    let mut gpu = Vec::new();
    let c_gpu = engine.execute_into(p, &mut gpu).expect("GPU execute");
    (cpu, gpu, c_cpu, c_gpu)
}

#[test]
fn t181_effect_parity_gpu_vs_cpu() {
    let Some(engine) = test_engine() else { return };
    let cases: Vec<(&str, Vec<FxInstance>, FrameTol)> = vec![
        (
            "pan",
            vec![FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.3, pan_y: -0.2, zoom: 1.5 }],
            FrameTol::BILINEAR,
        ),
        (
            "cammotion",
            vec![FxInstance::CamMotion {
                mode: PanMode::Fit,
            ease: true,
                time_scale: 1.0,
                keyframes: vec![
                    CamKeyframe { t: 0.0, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 },
                    CamKeyframe { t: 1.0, pan_x: 0.5, pan_y: 0.0, zoom: 2.0 },
                ],
            }],
            FrameTol::BILINEAR,
        ),
        ("mirror", vec![FxInstance::Mirror { horizontal: true, vertical: false }], FrameTol::EXACT),
        ("blur", vec![FxInstance::blur(8)], FrameTol::BILINEAR),
        ("greenscreen", vec![FxInstance::greenscreen([0, 255, 0], 70, 25)], FrameTol::THRESHOLD),
        ("edges", vec![FxInstance::edges(48)], FrameTol::THRESHOLD),
        ("noise", vec![FxInstance::noise(0.3)], FrameTol::EXACT),
        // новые параметры Р.33 С5 — зеркала обязаны сойтись и на них
        ("blur-xy", vec![FxInstance::Blur { radius: 24, radius_y: 4 }], FrameTol::BILINEAR),
        (
            "edges-thick-color",
            vec![FxInstance::Edges {
                threshold: 48,
                thickness: 3,
                color_r: 255,
                color_g: 90,
                color_b: 0,
            }],
            FrameTol::THRESHOLD,
        ),
        (
            "noise-coarse",
            vec![FxInstance::Noise { amount: 0.3, size: 8, color: false }],
            FrameTol::EXACT,
        ),
        (
            "noise-color",
            vec![FxInstance::Noise { amount: 0.3, size: 3, color: true }],
            FrameTol::EXACT,
        ),
        (
            "vhs-scan-jitter",
            vec![FxInstance::Vhs { rgb_shift: 4, bleed: 0.4, scan_step: 7, jitter: 0.5 }],
            FrameTol::BILINEAR,
        ),
        (
            "vhs-no-scan",
            vec![FxInstance::Vhs { rgb_shift: 2, bleed: 0.2, scan_step: 0, jitter: 0.0 }],
            FrameTol::BILINEAR,
        ),
        (
            "glow-radius-tint",
            vec![FxInstance::Glow {
                strength: 0.9,
                radius: 8,
                threshold: 0,
                tint_r: 255,
                tint_g: 120,
                tint_b: 40,
            }],
            FrameTol::BILINEAR,
        ),
        (
            "glow-threshold",
            vec![FxInstance::Glow {
                strength: 1.2,
                radius: 4,
                threshold: 90,
                tint_r: 255,
                tint_g: 255,
                tint_b: 255,
            }],
            FrameTol::BILINEAR,
        ),
        (
            "greenscreen-spill",
            vec![FxInstance::Greenscreen {
                key_r: 0,
                key_g: 255,
                key_b: 0,
                tolerance: 70,
                softness: 25,
                spill: 0.8,
            }],
            FrameTol::THRESHOLD,
        ),
        ("vhs", vec![FxInstance::vhs(4, 0.4)], FrameTol::BILINEAR),
        ("glow", vec![FxInstance::glow(0.65)], FrameTol::BILINEAR),
        (
            "eventgrid",
            vec![FxInstance::EventGrid { cols: 2, rows: 2, center: true }],
            FrameTol::BILINEAR,
        ),
        // Слитые гео-серии (Р.44): один проход обратных отображений на
        // обоих бэкендах (программа общая, kt-fx::geom); допуск билинейный —
        // f32 против f64 в композиции карт и тапах.
        (
            "geo-mirror-grid",
            vec![
                FxInstance::Mirror { horizontal: true, vertical: false },
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
            ],
            FrameTol::BILINEAR,
        ),
        (
            "geo-grid-mirror",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: true },
                FxInstance::Mirror { horizontal: true, vertical: true },
            ],
            FrameTol::BILINEAR,
        ),
        (
            "geo-pan-pan",
            vec![
                FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.3, pan_y: -0.2, zoom: 1.5 },
                FxInstance::Pan { mode: PanMode::Stretch, pan_x: -0.1, pan_y: 0.1, zoom: 1.2 },
            ],
            FrameTol::BILINEAR,
        ),
        // ядро сценария Р.44: сетка уменьшила, зум вернул — слитый проход
        // сэмплирует ИСХОДНИК, а не растр сетки
        (
            "geo-grid-pan-zoomback",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: true },
                FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 2.0 },
            ],
            FrameTol::BILINEAR,
        ),
        // zoom<1: рамка шире входа — «плато» (Clamp-prim) на обоих путях
        (
            "geo-pan-shrink-clamp",
            vec![
                FxInstance::Mirror { horizontal: false, vertical: true },
                FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 0.6 },
            ],
            FrameTol::BILINEAR,
        ),
        // Поглощённые поточечные (Р.45, п.312): Greenscreen/Noise на
        // сабтапе внутри серии. Пороговые (мягкий ключ) и ячеистые (сетка
        // зерна) решения законно флипают на f32/f64-границах — THRESHOLD
        // (прецедент edges/greenscreen).
        (
            "tap-gs-in-run",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
                FxInstance::greenscreen([0, 255, 0], 70, 25),
                FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 2.0 },
            ],
            FrameTol::THRESHOLD,
        ),
        (
            "tap-noise-in-run",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: true },
                FxInstance::Noise { amount: 0.6, size: 2, color: true },
                FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.1, pan_y: 0.0, zoom: 1.8 },
            ],
            FrameTol::THRESHOLD,
        ),
        // K×-барьеры (Р.45, п.314): сетка стиснула → барьер в K=2 →
        // зум вернул. CPU-ядро и GPU-пасс работают на K×-буферах.
        (
            "k2-vhs",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
                FxInstance::Vhs { rgb_shift: 2, bleed: 0.3, scan_step: 5, jitter: 0.4 },
                FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 2.0 },
            ],
            FrameTol::BILINEAR,
        ),
        (
            "k2-blur",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: true },
                FxInstance::blur(8),
                FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 2.0 },
            ],
            FrameTol::BILINEAR,
        ),
        (
            "k2-glow",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
                FxInstance::glow(0.9),
                FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.1, pan_y: 0.0, zoom: 1.8 },
            ],
            FrameTol::BILINEAR,
        ),
        (
            "k2-edges",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
                FxInstance::edges(48),
                FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 2.0 },
            ],
            FrameTol::THRESHOLD,
        ),
        // Noise-БАРЬЕР в K× (не поглощён: стоит сразу за жёстким барьером,
        // ведущий для следующего рана)
        (
            "k2-noise-barrier",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
                FxInstance::blur(4),
                FxInstance::Noise { amount: 0.5, size: 2, color: true },
                FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 2.0 },
            ],
            FrameTol::THRESHOLD,
        ),
        (
            "chain-user",
            vec![
                FxInstance::Mirror { horizontal: true, vertical: false },
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
                FxInstance::glow(1.0),
                FxInstance::vhs(2, 0.5),
            ],
            FrameTol::BILINEAR,
        ),
        (
            "chain-key",
            vec![FxInstance::greenscreen([0, 255, 0], 70, 25), FxInstance::noise(0.5)],
            FrameTol::THRESHOLD,
        ),
        // СИЛА СЛОЯ (Р.38 п.12): смешение сухой и мокрой сторон. Ядро
        // целочисленное (зеркало dissolve_into), поэтому байт-в-байт —
        // но вход мокрой стороны билинейный, отсюда общий допуск.
        (
            "layer-mix-half",
            vec![
                FxInstance::Mirror { horizontal: true, vertical: false },
                FxInstance::blur(8),
                FxInstance::LayerMix { amount: 0.5, from: 1 },
            ],
            FrameTol::BILINEAR,
        ),
        (
            "layer-mix-zero",
            vec![FxInstance::edges(48), FxInstance::LayerMix { amount: 0.0, from: 1 }],
            FrameTol::THRESHOLD,
        ),
        // сила ПОСЛЕ ресемплящего эффекта: сухая сторона обязана быть
        // вписана той же билинейкой, что и на CPU
        (
            "layer-mix-after-resize",
            vec![
                FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.2, pan_y: 0.0, zoom: 1.4 },
                FxInstance::blur(6),
                FxInstance::LayerMix { amount: 0.35, from: 1 },
            ],
            FrameTol::BILINEAR,
        ),
        // сила НУЛЬ поверх всего: обязана вернуть состояние до слоя
        (
            "layer-mix-whole-chain",
            vec![FxInstance::vhs(4, 0.4), FxInstance::LayerMix { amount: 0.25, from: 0 }],
            FrameTol::BILINEAR,
        ),
    ];
    for (i, (name, fx, tol)) in cases.into_iter().enumerate() {
        let src = Src::new(1000 + i as u64, Some(frame(W, H, false, 0)), fx);
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{name}: complete разошёлся");
        assert_frames_close(&cpu, &gpu, W, H, tol, name);
    }
    // ПРАВИЛО АЛЬФЫ (Р.53, п.418): вход С АЛЬФОЙ. Тут бэкенды расходятся
    // легче всего — премультипликация перед мипом и поднятие альфы у
    // edges живут ДВАЖДЫ (kt-fx и WGSL), и разойдись они, титр с
    // размытием выглядел бы на GPU иначе, чем в экспорте на CPU.
    let alpha_cases: Vec<(&str, Vec<FxInstance>, FrameTol)> = vec![
        ("alpha-blur", vec![FxInstance::blur(8)], FrameTol::BILINEAR),
        ("alpha-blur-xy", vec![FxInstance::Blur { radius: 24, radius_y: 4 }], FrameTol::BILINEAR),
        ("alpha-glow", vec![FxInstance::glow(1.2)], FrameTol::BILINEAR),
        (
            "alpha-glow-tinted",
            vec![FxInstance::Glow {
                strength: 0.8,
                radius: 9,
                threshold: 40,
                tint_r: 200,
                tint_g: 120,
                tint_b: 255,
            }],
            FrameTol::BILINEAR,
        ),
        ("alpha-edges", vec![FxInstance::edges(48)], FrameTol::THRESHOLD),
        (
            "alpha-gs-blur",
            vec![FxInstance::greenscreen([0, 255, 0], 70, 25), FxInstance::blur(8)],
            FrameTol::THRESHOLD,
        ),
        (
            "alpha-grid-blur",
            vec![FxInstance::EventGrid { cols: 2, rows: 2, center: true }, FxInstance::blur(12)],
            FrameTol::BILINEAR,
        ),
    ];
    for (i, (name, fx, tol)) in alpha_cases.into_iter().enumerate() {
        let src = Src::new(3000 + i as u64, Some(frame(W, H, true, 0)), fx);
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{name}: complete разошёлся");
        assert_frames_close(&cpu, &gpu, W, H, tol, name);
    }

    // Pan с кадром ЧУЖОГО размера (ресайз в Quality обязан совпасть)
    let src = Src::new(
        2000,
        Some(frame(96, 54, false, 0)),
        vec![FxInstance::Pan { mode: PanMode::Fill, pan_x: 0.1, pan_y: 0.0, zoom: 1.2 }],
    );
    let p = plan(90_000, vec![one(&src, 4096, false)]);
    let (cpu, gpu, _, _) = run_both(engine.as_ref(), &p);
    assert_frames_close(&cpu, &gpu, W, H, FrameTol::BILINEAR, "pan-resize");

    // Pan3d-серии (Р.44): CPU-эталон появился (обратная гомография);
    // допуск GEOM — силуэт квада законно флипает единичные пиксели
    // (f32-предикаты против f64), внутри — билинейная точность.
    let p3 = |pos_x: f64, rot_y: f64, scale: f64| FxInstance::Pan3d {
        pos_x,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale,
        camera: None,
    };
    let grid2 = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    let geo3d: Vec<(&str, Vec<FxInstance>)> = vec![
        ("pan3d-alone", vec![p3(4.0, 20.0, 0.9)]),
        ("grid-pan3d", vec![grid2.clone(), p3(0.0, 35.0, 1.0)]),
        ("pan3d-grid", vec![p3(0.0, 0.0, 0.7), grid2.clone()]),
        // ЦЕПОЧКА ПОЛЬЗОВАТЕЛЯ (Р.44): grid → pan3d → grid → pan3d-зум,
        // нетто-масштаб ≈ 1 — слитый проход сэмплирует исходник
        ("user-grid-pan3d-x2", vec![grid2.clone(), p3(2.0, 15.0, 1.0), grid2, p3(0.0, 0.0, 4.0)]),
    ];
    for (i, (name, fx)) in geo3d.into_iter().enumerate() {
        let mut src = Src::new(3000 + i as u64, Some(frame(W, H, false, 0)), fx);
        src.cam3d =
            Some(kt_engine::Cam3d { cam: kt_model::CameraParams::default(), world_scale: 1.0 });
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{name}: complete разошёлся");
        assert_frames_close(&cpu, &gpu, W, H, FrameTol::GEOM, name);
    }
}

/// Тест 309 (п.319, Р.46): СТУПЕНЬ ДЕКОДА на GPU. Пре-FX кадр приходит
/// вдвое крупнее Quality (`src_scale = 2`), план строится в логике, а
/// исполняется на крупном буфере: конъюгация, ведущие барьеры в 2×,
/// поглощённые поточечные и Legacy-зеркало обязаны совпасть с CPU в тех же
/// допусках, что и K×-барьеры Р.45.
#[test]
fn t309_decode_stage_parity_gpu_vs_cpu() {
    let Some(engine) = test_engine() else { return };
    let zoom2 = FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 2.0 };
    let grid2 = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let cases: Vec<(&str, Vec<FxInstance>, FrameTol)> = vec![
        // чистая конъюгация: 2×-вход сводится зумом ровно в Quality
        ("s2-zoom", vec![zoom2.clone()], FrameTol::BILINEAR),
        // ведущий барьер исполняется в 2× (ресемпла до него не было)
        ("s2-vhs-zoom", vec![FxInstance::vhs(4, 0.4), zoom2.clone()], FrameTol::BILINEAR),
        // ступень + сетка выше барьера: K берётся по обоим источникам
        (
            "s2-grid-vhs-zoom",
            vec![grid2.clone(), FxInstance::vhs(2, 0.3), zoom2.clone()],
            FrameTol::BILINEAR,
        ),
        // зеркало на равных краях — старое ядро прямо на 2×-буфере
        (
            "s2-mirror-zoom",
            vec![
                FxInstance::Mirror { horizontal: true, vertical: false },
                FxInstance::vhs(4, 0.4),
                zoom2.clone(),
            ],
            FrameTol::BILINEAR,
        ),
        // поглощённые поточечные на сабтапе крупного входа
        (
            "s2-tap-gs",
            vec![grid2, FxInstance::greenscreen([0, 255, 0], 70, 25), zoom2.clone()],
            FrameTol::THRESHOLD,
        ),
        // ядро сценария фичи: зум ×2 берёт детали, которых в 1× не было
        ("s2-zoom-edges", vec![zoom2, FxInstance::edges(48)], FrameTol::THRESHOLD),
    ];
    for (i, (name, fx, tol)) in cases.into_iter().enumerate() {
        let mut src = Src::new(4000 + i as u64, Some(frame(W * 2, H * 2, false, 0)), fx);
        src.src_scale = 2;
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{name}: complete разошёлся");
        assert_frames_close(&cpu, &gpu, W, H, tol, name);
    }
    // 3D на ступени: обратная гомография считается в логике, тапы — на
    // крупном буфере (допуск GEOM — силуэт квада, как у t181)
    let mut src = Src::new(
        4100,
        Some(frame(W * 2, H * 2, false, 0)),
        vec![FxInstance::Pan3d {
            pos_x: 2.0,
            pos_y: 0.0,
            pos_z: 0.0,
            rot_x: 0.0,
            rot_y: 15.0,
            rot_z: 0.0,
            anchor_x: 0.5,
            anchor_y: 0.5,
            scale: 2.0,
            camera: None,
        }],
    );
    src.src_scale = 2;
    src.cam3d = Some(kt_engine::Cam3d { cam: kt_model::CameraParams::default(), world_scale: 1.0 });
    let p = plan(90_000, vec![one(&src, 4096, false)]);
    let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
    assert_eq!(cc, cg, "s2-pan3d: complete разошёлся");
    assert_frames_close(&cpu, &gpu, W, H, FrameTol::GEOM, "s2-pan3d");
}

#[test]
fn t182_composite_parity_byte_exact() {
    let Some(engine) = test_engine() else { return };
    let bg = Src::new(1, Some(frame(W, H, false, 0)), vec![]);
    let top_alpha = Src::new(2, Some(frame(W, H, true, 0x55)), vec![]);
    let top_opaque = Src::new(3, Some(frame(W, H, false, 0xaa)), vec![]);
    let missing = Src::new(4, None, vec![]);
    let misfit = Src::new(5, Some(frame(32, 18, false, 0)), vec![]);

    // (имя, слои, ожидаемый complete)
    let cases: Vec<(&str, Vec<PlanLayer<'_>>, bool)> = vec![
        ("two-opaque", vec![one(&bg, 4096, false), one(&top_opaque, 4096, false)], true),
        ("alpha-over", vec![one(&bg, 4096, false), one(&top_alpha, 4096, false)], true),
        ("opacity", vec![one(&bg, 4096, false), one(&top_alpha, 2048, true)], true),
        ("solo-fade-full", vec![one(&bg, 4096, false), one(&top_opaque, 4096, true)], true),
        ("solo-fade-mid", vec![one(&top_opaque, 1365, true)], true),
        (
            "dissolve",
            vec![
                one(&bg, 4096, false),
                PlanLayer::Dissolve {
                    a: top_opaque.layer(),
                    b: top_alpha.layer(),
                    k: 1000,
                    alpha_k: 4096,
                    mix: Default::default(),
                },
            ],
            true,
        ),
        (
            "dissolve-level",
            vec![
                one(&bg, 4096, false),
                PlanLayer::Dissolve {
                    a: top_opaque.layer(),
                    b: top_alpha.layer(),
                    k: 3000,
                    alpha_k: 2000,
                    mix: Default::default(),
                },
            ],
            true,
        ),
        (
            "dissolve-missing-half",
            vec![
                one(&bg, 4096, false),
                PlanLayer::Dissolve {
                    a: top_opaque.layer(),
                    b: missing.layer(),
                    k: 2048,
                    alpha_k: 3000,
                    mix: Default::default(),
                },
            ],
            false,
        ),
        ("missing-layer", vec![one(&bg, 4096, false), one(&missing, 4096, false)], false),
        // Р.58, п.459: слой ЧУЖОГО РАЗМЕРА больше не проходит молча. До
        // этого раунда кадр объявлялся ПОЛНЫМ без него — так вложение
        // исчезало из готового кадра, а экспорт писал его как годный.
        // Байты по-прежнему сходятся у обоих исполнителей: оба слой не
        // рисуют, — а вот «полон» стало общим и честным `false`.
        ("misfit-not-silent", vec![one(&bg, 4096, false), one(&misfit, 4096, false)], false),
        ("empty", vec![one(&missing, 4096, false)], false),
    ];
    for (name, layers, want_complete) in cases {
        let p = plan(90_000, layers);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, want_complete, "{name}: complete CPU");
        assert_eq!(cg, want_complete, "{name}: complete GPU");
        assert_frames_close(&cpu, &gpu, W, H, FrameTol::EXACT, name);
    }
}

#[test]
fn t183_gpu_deterministic_on_device_and_dirty_pool() {
    let Some(e1) = test_engine() else { return };
    let Some(e2) = test_engine() else { return };
    // цепочка БЕЗ t-зависимых эффектов: смена t меняет ключ кэша
    // движка, но не байты → форсит пересчёт «грязным» пулом
    let src = Src::new(
        10,
        Some(frame(96, 54, false, 3)),
        vec![
            FxInstance::Pan { mode: PanMode::Fill, pan_x: 0.2, pan_y: 0.1, zoom: 1.3 },
            FxInstance::glow(1.2),
        ],
    );
    let bg = Src::new(11, Some(frame(W, H, false, 0)), vec![]);
    let mk = |t: i64| plan(t, vec![one(&bg, 4096, false), one(&src, 3000, true)]);

    let mut a1 = Vec::new();
    e1.execute_into(&mk(90_000), &mut a1).unwrap();
    // два независимых движка на одном адаптере — байт-в-байт
    let mut b1 = Vec::new();
    e2.execute_into(&mk(90_000), &mut b1).unwrap();
    assert_eq!(a1, b1, "два движка на одном устройстве разошлись");

    // испачкать пулы e1 другими планами (вкл. слитую гео-серию Р.44 и
    // K×-барьер Р.45: сетка стиснула → blur в 2× → зум)
    let junk = Src::new(
        12,
        Some(frame(W, H, false, 9)),
        vec![
            FxInstance::EventGrid { cols: 3, rows: 2, center: true },
            FxInstance::blur(12),
            FxInstance::Mirror { horizontal: true, vertical: false },
            FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 2.0 },
        ],
    );
    let mut _j = Vec::new();
    e1.execute_into(&plan(1920, vec![one(&junk, 4096, false)]), &mut _j).unwrap();
    e1.execute_into(&plan(3840, vec![one(&junk, 2000, true)]), &mut _j).unwrap();

    // тот же план с другим t (обход ReadbackCache) → пересчёт грязным
    // пулом обязан дать те же байты
    let mut a2 = Vec::new();
    e1.execute_into(&mk(180_000), &mut a2).unwrap();
    assert_eq!(a1, a2, "грязный пул текстур изменил результат");

    // а повтор с исходным t — хит ReadbackCache с теми же байтами
    let mut a3 = Vec::new();
    e1.execute_into(&mk(90_000), &mut a3).unwrap();
    assert_eq!(a1, a3, "ReadbackCache вернул не те байты");
}

/// Тест 224 (Р.33 С4): КАЖДЫЙ вид перехода считается на GPU так же, как на
/// CPU. Ядра переходов — единственное место, где одну формулу пришлось
/// написать дважды (Rust и WGSL), поэтому расхождение здесь означает, что
/// у пользователя превью и экспорт покажут РАЗНОЕ — ровно то, что
/// запрещает инвариант 7.
#[test]
fn t224_transition_parity_gpu_vs_cpu() {
    let Some(engine) = test_engine() else { return };
    let a = Src::new(1, Some(frame(W, H, false, 0)), vec![]);
    let b = Src::new(2, Some(frame(W, H, false, 0x5a)), vec![]);
    // допуск: ядра переходов считаются в f32 на обоих путях, но выборка
    // и тригонометрия у драйвера своя — берём билинейный допуск, а
    // маскам с резкой кромкой разрешаем полосу пикселей на границе
    let soft = FrameTol { max_lsb: 2, outlier_frac: 0.01, outlier_max: 4, mean_lsb: 0.05 };
    // ВАРИАЦИИ, а не только умолчания: у вайпа/сдвига дефолт — Dir::Left,
    // то есть вектор (−1, 0). На нём ошибка в Y-члене формулы не видна
    // вовсе — именно так первая версия этого теста прозевала намеренно
    // сломанное зеркало. Каждая ось и каждый флаг обязаны быть в списке.
    let cases: Vec<TransitionKind> = vec![
        TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.15 },
        TransitionKind::Wipe { dir: Dir::Up, angle: 0.0, softness: 0.0 },
        TransitionKind::Wipe { dir: Dir::Right, angle: 37.0, softness: 0.3 },
        TransitionKind::Iris { shape: IrisShape::Circle, cx: 0.5, cy: 0.5, softness: 0.15 },
        TransitionKind::Iris { shape: IrisShape::Diamond, cx: 0.5, cy: 0.5, softness: 0.0 },
        TransitionKind::Iris { shape: IrisShape::Box, cx: 0.5, cy: 0.5, softness: 0.4 },
        // смещённый центр (Р.51): без него ошибка в члене центра не видна
        TransitionKind::Iris { shape: IrisShape::Circle, cx: 0.2, cy: 0.7, softness: 0.1 },
        TransitionKind::Clock { reverse: false, cx: 0.5, cy: 0.5, softness: 0.1 },
        TransitionKind::Clock { reverse: true, cx: 0.5, cy: 0.5, softness: 0.0 },
        TransitionKind::Clock { reverse: false, cx: 0.7, cy: 0.3, softness: 0.1 },
        TransitionKind::Barn { vertical: false, softness: 0.2 },
        TransitionKind::Barn { vertical: true, softness: 0.0 },
        TransitionKind::Luma { invert: false, softness: 0.2 },
        TransitionKind::Luma { invert: true, softness: 0.05 },
        TransitionKind::Push { dir: Dir::Left },
        TransitionKind::Push { dir: Dir::Down },
        TransitionKind::Slide { dir: Dir::Right },
        TransitionKind::Slide { dir: Dir::Up },
        TransitionKind::Zoom { amount: 0.4, cx: 0.5, cy: 0.5 },
        TransitionKind::Zoom { amount: 1.5, cx: 0.25, cy: 0.6 },
        TransitionKind::Spin { turns: 1.0, amount: 0.4, cx: 0.5, cy: 0.5 },
        TransitionKind::Spin { turns: -0.5, amount: 0.0, cx: 0.8, cy: 0.2 },
        TransitionKind::Whip { dir: Dir::Left, blur: 0.6 },
        TransitionKind::Whip { dir: Dir::Down, blur: 1.0 },
        TransitionKind::BlurDissolve { amount: 24.0 },
        TransitionKind::Pixelate { amount: 48.0 },
        TransitionKind::Glitch { amount: 0.5 },
        TransitionKind::Flash { color_r: 255, color_g: 200, color_b: 40, strength: 1.0 },
        TransitionKind::Dip { color_r: 0, color_g: 0, color_b: 0 },
        // Р.51 (п.400): узоры, сдвиги-симбиозы, стилизованные — обе оси
        // и оба флага у каждого направленного, как требует шапка списка
        TransitionKind::Blinds { count: 8, angle: 0.0, softness: 0.1 },
        TransitionKind::Blinds { count: 5, angle: 30.0, softness: 0.0 },
        TransitionKind::Checker { cols: 8, rows: 6, softness: 0.1 },
        TransitionKind::Checker { cols: 5, rows: 7, softness: 0.0 },
        TransitionKind::Blocks { cols: 16, rows: 9, softness: 0.25 },
        TransitionKind::Blocks { cols: 7, rows: 5, softness: 0.05 },
        TransitionKind::Spiral { turns: 2.0, cx: 0.5, cy: 0.5, softness: 0.1 },
        TransitionKind::Spiral { turns: 4.0, cx: 0.3, cy: 0.6, softness: 0.05 },
        TransitionKind::Split { vertical: false },
        TransitionKind::Split { vertical: true },
        TransitionKind::Squeeze { dir: Dir::Left },
        TransitionKind::Squeeze { dir: Dir::Right },
        TransitionKind::Squeeze { dir: Dir::Up },
        TransitionKind::Squeeze { dir: Dir::Down },
        TransitionKind::Stretch { vertical: false },
        TransitionKind::Stretch { vertical: true },
        TransitionKind::Swap { vertical: false, depth: 0.3 },
        TransitionKind::Swap { vertical: true, depth: 0.7 },
        TransitionKind::Shake { amount: 0.5, steps: 12 },
        TransitionKind::Shake { amount: 1.0, steps: 3 },
        TransitionKind::FilmBurn { scale: 6.0, softness: 0.2, tint_r: 255, tint_g: 120, tint_b: 0 },
        TransitionKind::FilmBurn {
            scale: 14.0,
            softness: 0.05,
            tint_r: 200,
            tint_g: 40,
            tint_b: 20,
        },
        TransitionKind::LightLeaks { strength: 0.6, tint_r: 255, tint_g: 90, tint_b: 40 },
        TransitionKind::LightLeaks { strength: 1.0, tint_r: 80, tint_g: 160, tint_b: 255 },
        TransitionKind::ZoomBlur { amount: 0.6, cx: 0.5, cy: 0.5 },
        TransitionKind::ZoomBlur { amount: 1.0, cx: 0.2, cy: 0.8 },
    ];
    let mut seen = std::collections::BTreeSet::new();
    for kind in &cases {
        if matches!(kind, TransitionKind::Crossfade) {
            continue; // целочисленный путь, его держит t182
        }
        let mix = kt_engine::TransMix::of(kind);
        seen.insert(mix.mode);
        // несколько фаз: у переходов формула зависит от k нелинейно.
        // 1800 = 0.44 — фаза МЕЖДУ четвертью и серединой: без неё
        // мутация «порог фазы dip сдвинут с 0.5 на 0.4» проходила мимо.
        for k in [200u32, 1024, 1800, 2048, 3600] {
            let p = plan(
                k as i64,
                vec![PlanLayer::Dissolve { a: a.layer(), b: b.layer(), k, alpha_k: 4096, mix }],
            );
            let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
            assert!(cc && cg, "{}: план обязан быть полным", kind.tag());
            assert_frames_close(&cpu, &gpu, W, H, soft, &format!("{}@{k}", kind.tag()));
        }
    }
    // объёмные (Р.51, п.401): жёсткие кромки панелей и перспективное
    // деление — допуск GEOM, как у pan3d (пиксель на кромке может
    // перещёлкнуться целиком)
    let hard: Vec<TransitionKind> = vec![
        TransitionKind::Flip { vertical: false, persp: 0.5, shade: 0.5 },
        TransitionKind::Flip { vertical: true, persp: 1.0, shade: 0.0 },
        TransitionKind::Cube { dir: Dir::Left, persp: 0.5, shade: 0.5 },
        TransitionKind::Cube { dir: Dir::Right, persp: 0.2, shade: 0.0 },
        TransitionKind::Cube { dir: Dir::Up, persp: 0.8, shade: 0.3 },
        TransitionKind::Door { vertical: false, persp: 0.5, shade: 0.5 },
        TransitionKind::Door { vertical: true, persp: 0.9, shade: 0.2 },
        TransitionKind::PagePeel { dir: Dir::Left, curl: 0.18, shadow: 0.5 },
        TransitionKind::PagePeel { dir: Dir::Down, curl: 0.35, shadow: 1.0 },
        TransitionKind::PageRoll { dir: Dir::Right, curl: 0.18, shadow: 0.5 },
        TransitionKind::PageRoll { dir: Dir::Up, curl: 0.3, shadow: 0.0 },
    ];
    for kind in &hard {
        let mix = kt_engine::TransMix::of(kind);
        seen.insert(mix.mode);
        for k in [200u32, 1024, 1800, 2048, 3600] {
            let p = plan(
                k as i64,
                vec![PlanLayer::Dissolve { a: a.layer(), b: b.layer(), k, alpha_k: 4096, mix }],
            );
            let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
            assert!(cc && cg, "{}: план обязан быть полным", kind.tag());
            assert_frames_close(&cpu, &gpu, W, H, FrameTol::GEOM, &format!("{}@{k}", kind.tag()));
        }
    }
    assert_eq!(seen.len(), 32, "все 32 не-кроссфейдных режима проверены: {seen:?}");
    assert_eq!(
        seen.len() + 1,
        TransitionKind::ALL.len(),
        "новый вид перехода обязан появиться и в этом списке"
    );

    // сольный фейд С ВИДОМ: вторая сторона — пустота, тот же код
    for (incoming, kind) in [
        (true, TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.2 }),
        (false, TransitionKind::Iris { shape: IrisShape::Circle, cx: 0.5, cy: 0.5, softness: 0.1 }),
        (true, TransitionKind::Push { dir: Dir::Up }),
        // представители Р.51: узор и сдвиг-симбиоз фейдом «из пустоты»
        (true, TransitionKind::Blinds { count: 8, angle: 0.0, softness: 0.1 }),
        (false, TransitionKind::Squeeze { dir: Dir::Right }),
    ] {
        let mix = kt_engine::TransMix::of(&kind);
        let p = plan(
            7,
            vec![PlanLayer::One {
                src: a.layer(),
                alpha_k: 4096,
                scale_alpha: true,
                fade: Some(kt_engine::video_plan::SoloFade { k: 1500, mix, incoming }),
            }],
        );
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert!(cc && cg, "{}: сольный фейд обязан быть полным", kind.tag());
        assert_frames_close(&cpu, &gpu, W, H, soft, &format!("solo-{}", kind.tag()));
        // фейд обязан ЧТО-ТО скрывать: иначе тест зелен на «вида нет»
        let opaque = cpu.chunks_exact(4).filter(|p| p[3] > 250).count();
        let clear = cpu.chunks_exact(4).filter(|p| p[3] < 5).count();
        assert!(
            opaque > 100 && clear > 100,
            "{}: у сольного фейда обязаны быть и открытая, и пустая части ({opaque}/{clear})",
            kind.tag()
        );
    }
}

/// Тест 329 (Р.47, п.331): конформанс-цикл CPU↔GPU по ВИДЕО-подмножеству
/// реестра FxInstance::all(): на каждый эффект — одиночный кейс и сэндвич
/// `[сетка 2×2, эффект, зум ×2]` (у барьеров сэндвич даёт K=2). Классы
/// допуска — из таблицы; замок полноты как у t224: вид без строки валит
/// тест. Slowed+Reverb на видео — варп времени, не пиксельный фильтр —
/// исключён явно.
#[test]
fn t329_parity_cycle_over_the_registry() {
    let Some(engine) = test_engine() else { return };
    // (тег, допуск одиночного, допуск сэндвича)
    let classes: &[(&str, FrameTol, FrameTol)] = &[
        ("pan", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("cam_motion", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("mirror", FrameTol::EXACT, FrameTol::BILINEAR),
        ("blur", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("greenscreen", FrameTol::THRESHOLD, FrameTol::THRESHOLD),
        ("edges", FrameTol::THRESHOLD, FrameTol::THRESHOLD),
        ("noise", FrameTol::EXACT, FrameTol::THRESHOLD),
        ("vhs", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("glow", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("event_grid", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("pan3d", FrameTol::GEOM, FrameTol::GEOM),
        // Р.47 Кв2: поточечные — f32-континуум против f64 (в сэндвиче они
        // едут тапами слитого гео-прохода); Posterize — пороговый квант;
        // CornerPin — проективный, как pan3d
        ("color_grade", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("duotone", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("posterize", FrameTol::THRESHOLD, FrameTol::THRESHOLD),
        ("vignette", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("corner_pin", FrameTol::GEOM, FrameTol::GEOM),
        // Р.47 Кв3: барьеры. Pixelate — целочисленная копия (EXACT
        // одиночный); drop_shadow на непрозрачном входе — тождество по
        // формуле; halftone — жёсткий край точки (пороговый класс)
        ("sharpen", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("pixelate", FrameTol::EXACT, FrameTol::BILINEAR),
        ("chromatic_aberration", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("radial_blur", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("drop_shadow", FrameTol::EXACT, FrameTol::BILINEAR),
        ("halftone", FrameTol::THRESHOLD, FrameTol::THRESHOLD),
        // Р.47 Кв4: гео Warp-прима
        ("lens_distort", FrameTol::GEOM, FrameTol::GEOM),
        ("twirl", FrameTol::GEOM, FrameTol::GEOM),
        ("ripple", FrameTol::GEOM, FrameTol::GEOM),
        ("kaleidoscope", FrameTol::GEOM, FrameTol::GEOM),
        ("cylinder3d", FrameTol::GEOM, FrameTol::GEOM),
    ];
    let table: std::collections::BTreeMap<&str, (FrameTol, FrameTol)> =
        classes.iter().map(|(t, a, b)| (*t, (*a, *b))).collect();
    let video: Vec<&FxInstance> = FxInstance::all()
        .iter()
        .filter(|f| {
            !f.is_audio() && !f.is_layer_mix() && !matches!(f, FxInstance::SlowedReverb { .. })
        })
        .collect();
    let tags: std::collections::BTreeSet<&str> = video.iter().map(|f| f.tag()).collect();
    assert_eq!(
        table.keys().copied().collect::<std::collections::BTreeSet<_>>(),
        tags,
        "t329 class table and the video registry must cover each other"
    );
    let grid2 = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let zoom2 = FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 2.0 };
    for (i, f) in video.iter().enumerate() {
        let (tol_single, tol_sand) = table[f.tag()];
        let cam = matches!(f, FxInstance::Pan3d { .. })
            .then(|| kt_engine::Cam3d { cam: kt_model::CameraParams::default(), world_scale: 1.0 });
        // одиночный
        let mut src =
            Src::new(9000 + i as u64 * 2, Some(frame(W, H, false, 0)), vec![(*f).clone()]);
        src.cam3d = cam;
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{}: complete diverged (single)", f.tag());
        assert_frames_close(&cpu, &gpu, W, H, tol_single, f.tag());
        // сэндвич
        let mut src = Src::new(
            9001 + i as u64 * 2,
            Some(frame(W, H, false, 0)),
            vec![grid2.clone(), (*f).clone(), zoom2.clone()],
        );
        src.cam3d = cam;
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{}: complete diverged (sandwich)", f.tag());
        assert_frames_close(&cpu, &gpu, W, H, tol_sand, &format!("sandwich-{}", f.tag()));
    }
    // Р.47 Кв2: у color_grade и corner_pin дефолт — тождество, и цикл выше
    // их формул не касается. Настроенные экземпляры сверяют математику:
    // одиночный — барьерный пасс (или Proj-ран), сэндвич — тап слитого
    // гео-прохода (kind 2..5 в geom_fused.wgsl).
    let tuned: &[(FxInstance, FrameTol, FrameTol)] = &[
        (
            FxInstance::ColorGrade { brightness: 0.12, contrast: 0.35, saturation: 1.6, hue: 60.0 },
            FrameTol::BILINEAR,
            FrameTol::BILINEAR,
        ),
        (
            FxInstance::Duotone {
                shadow_r: 40,
                shadow_g: 0,
                shadow_b: 80,
                high_r: 255,
                high_g: 220,
                high_b: 160,
                amount: 0.7,
            },
            FrameTol::BILINEAR,
            FrameTol::BILINEAR,
        ),
        (FxInstance::Posterize { levels: 4 }, FrameTol::THRESHOLD, FrameTol::THRESHOLD),
        (
            FxInstance::Vignette { strength: 0.9, radius: 0.3, softness: 0.2 },
            FrameTol::BILINEAR,
            FrameTol::BILINEAR,
        ),
        (
            FxInstance::CornerPin {
                x0: 0.08,
                y0: 0.06,
                x1: 0.92,
                y1: -0.04,
                x2: -0.05,
                y2: 0.95,
                x3: 1.05,
                y3: 1.1,
            },
            FrameTol::GEOM,
            FrameTol::GEOM,
        ),
    ];
    for (i, (f, tol_single, tol_sand)) in tuned.iter().enumerate() {
        let src = Src::new(9600 + i as u64 * 2, Some(frame(W, H, false, 0)), vec![f.clone()]);
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "tuned {}: complete diverged (single)", f.tag());
        assert_frames_close(&cpu, &gpu, W, H, *tol_single, &format!("tuned-{}", f.tag()));
        let src = Src::new(
            9601 + i as u64 * 2,
            Some(frame(W, H, false, 0)),
            vec![grid2.clone(), f.clone(), zoom2.clone()],
        );
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "tuned {}: complete diverged (sandwich)", f.tag());
        assert_frames_close(&cpu, &gpu, W, H, *tol_sand, &format!("tuned-sandwich-{}", f.tag()));
    }
    // Р.47 Кв4: жёсткие варпы — f32-тригонометрия под максимальной
    // нагрузкой (замер: оба бэкенда сессии держат GEOM; провал здесь —
    // расследование и, при подтверждении, ввод класса WARP ×2 от замера,
    // НЕ подкрутка GEOM)
    for (i, f) in [
        FxInstance::LensDistort { k1: -0.8, k2: 0.5 },
        FxInstance::Twirl { angle: 540.0, radius: 1.2 },
        FxInstance::Ripple { amount: 0.3, freq: 30.0, phase: 137.0 },
        FxInstance::Kaleidoscope { segments: 12, angle: 30.0 },
        FxInstance::Cylinder3d { curvature: 0.95 },
    ]
    .into_iter()
    .enumerate()
    {
        let src = Src::new(9800 + i as u64 * 2, Some(frame(W, H, false, 0)), vec![f.clone()]);
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, ..) = run_both(engine.as_ref(), &p);
        assert_frames_close(&cpu, &gpu, W, H, FrameTol::GEOM, &format!("warp-hard-{}", f.tag()));
        let src = Src::new(
            9801 + i as u64 * 2,
            Some(frame(W, H, false, 0)),
            vec![grid2.clone(), f.clone(), zoom2.clone()],
        );
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, ..) = run_both(engine.as_ref(), &p);
        assert_frames_close(
            &cpu,
            &gpu,
            W,
            H,
            FrameTol::GEOM,
            &format!("warp-hard-sandwich-{}", f.tag()),
        );
    }
    // Р.47 Кв3: spin-ветка радиального размытия (дуга — тригонометрия
    // f32 против f64) и тень на НАСТОЯЩЕЙ альфе (после greenscreen —
    // THRESHOLD, порог ключа рвёт билинейную гладкость)
    let spin = FxInstance::RadialBlur { amount: 0.6, center_x: 0.2, center_y: -0.1, spin: true };
    let src = Src::new(9700, Some(frame(W, H, false, 0)), vec![spin.clone()]);
    let p = plan(90_000, vec![one(&src, 4096, false)]);
    let (cpu, gpu, ..) = run_both(engine.as_ref(), &p);
    assert_frames_close(&cpu, &gpu, W, H, FrameTol::THRESHOLD, "tuned-radial-spin");
    let shadow_chain =
        vec![FxInstance::greenscreen([0, 255, 0], 70, 25), FxInstance::drop_shadow()];
    let src = Src::new(9702, Some(frame(W, H, false, 0)), shadow_chain.clone());
    let p = plan(90_000, vec![one(&src, 4096, false)]);
    let (cpu, gpu, ..) = run_both(engine.as_ref(), &p);
    assert_frames_close(&cpu, &gpu, W, H, FrameTol::THRESHOLD, "tuned-shadow-alpha");
    let mut with_zoom = shadow_chain;
    with_zoom.push(zoom2.clone());
    let src = Src::new(9704, Some(frame(W, H, false, 0)), with_zoom);
    let p = plan(90_000, vec![one(&src, 4096, false)]);
    let (cpu, gpu, ..) = run_both(engine.as_ref(), &p);
    assert_frames_close(&cpu, &gpu, W, H, FrameTol::THRESHOLD, "tuned-shadow-alpha-zoom");
}
