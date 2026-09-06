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
        }
    }

    fn layer(&self) -> LayerSrc<'_> {
        LayerSrc {
            source: self.source,
            cache_idx: 0,
            frame: self.frame.clone(),
            fx: std::borrow::Cow::Borrowed(&self.fx[..]),
            fx_hash: self.fx_hash,
            u: self.u,
            cam3d: None,
        }
    }
}

fn one(src: &Src, alpha_k: u32, scale_alpha: bool) -> PlanLayer<'_> {
    PlanLayer::One { src: src.layer(), alpha_k, scale_alpha, fade: None }
}

fn plan<'a>(t: i64, layers: Vec<PlanLayer<'a>>) -> FramePlan<'a> {
    FramePlan { t: Tick(t), out_w: W, out_h: H, layers, has_3d: false, memoize_postfx: true }
}

/// CPU против GPU на одном плане; возвращает (cpu, gpu, complete_cpu,
/// complete_gpu).
fn run_both(
    engine: &dyn VideoGpuExec,
    p: &FramePlan<'_>,
) -> (Vec<u8>, Vec<u8>, bool, bool) {
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
        (
            "mirror",
            vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            FrameTol::EXACT,
        ),
        ("blur", vec![FxInstance::blur(8)], FrameTol::BILINEAR),
        (
            "greenscreen",
            vec![FxInstance::greenscreen([0, 255, 0], 70, 25)],
            FrameTol::THRESHOLD,
        ),
        ("edges", vec![FxInstance::edges(48)], FrameTol::THRESHOLD),
        ("noise", vec![FxInstance::noise(0.3)], FrameTol::EXACT),
        // новые параметры Р.33 С5 — зеркала обязаны сойтись и на них
        (
            "blur-xy",
            vec![FxInstance::Blur { radius: 24, radius_y: 4 }],
            FrameTol::BILINEAR,
        ),
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
            vec![
                FxInstance::greenscreen([0, 255, 0], 70, 25),
                FxInstance::noise(0.5),
            ],
            FrameTol::THRESHOLD,
        ),
    ];
    for (i, (name, fx, tol)) in cases.into_iter().enumerate() {
        let src = Src::new(1000 + i as u64, Some(frame(W, H, false, 0)), fx);
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{name}: complete разошёлся");
        assert_frames_close(&cpu, &gpu, W, H, tol, name);
    }
    // Pan с кадром ЧУЖОГО размера (ресайз в Quality обязан совпасть)
    let src = Src::new(2000, Some(frame(96, 54, false, 0)), vec![FxInstance::Pan {
        mode: PanMode::Fill,
        pan_x: 0.1,
        pan_y: 0.0,
        zoom: 1.2,
    }]);
    let p = plan(90_000, vec![one(&src, 4096, false)]);
    let (cpu, gpu, _, _) = run_both(engine.as_ref(), &p);
    assert_frames_close(&cpu, &gpu, W, H, FrameTol::BILINEAR, "pan-resize");
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
        ("misfit-skipped", vec![one(&bg, 4096, false), one(&misfit, 4096, false)], true),
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
    let src = Src::new(10, Some(frame(96, 54, false, 3)), vec![
        FxInstance::Pan { mode: PanMode::Fill, pan_x: 0.2, pan_y: 0.1, zoom: 1.3 },
        FxInstance::glow(1.2),
    ]);
    let bg = Src::new(11, Some(frame(W, H, false, 0)), vec![]);
    let mk = |t: i64| plan(t, vec![one(&bg, 4096, false), one(&src, 3000, true)]);

    let mut a1 = Vec::new();
    e1.execute_into(&mk(90_000), &mut a1).unwrap();
    // два независимых движка на одном адаптере — байт-в-байт
    let mut b1 = Vec::new();
    e2.execute_into(&mk(90_000), &mut b1).unwrap();
    assert_eq!(a1, b1, "два движка на одном устройстве разошлись");

    // испачкать пулы e1 другими планами
    let junk = Src::new(12, Some(frame(W, H, false, 9)), vec![
        FxInstance::blur(12),
        FxInstance::EventGrid { cols: 3, rows: 2, center: true },
    ]);
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
        TransitionKind::Iris { shape: IrisShape::Circle, softness: 0.15 },
        TransitionKind::Iris { shape: IrisShape::Diamond, softness: 0.0 },
        TransitionKind::Iris { shape: IrisShape::Box, softness: 0.4 },
        TransitionKind::Clock { reverse: false, softness: 0.1 },
        TransitionKind::Clock { reverse: true, softness: 0.0 },
        TransitionKind::Barn { vertical: false, softness: 0.2 },
        TransitionKind::Barn { vertical: true, softness: 0.0 },
        TransitionKind::Luma { invert: false, softness: 0.2 },
        TransitionKind::Luma { invert: true, softness: 0.05 },
        TransitionKind::Push { dir: Dir::Left },
        TransitionKind::Push { dir: Dir::Down },
        TransitionKind::Slide { dir: Dir::Right },
        TransitionKind::Slide { dir: Dir::Up },
        TransitionKind::Zoom { amount: 0.4 },
        TransitionKind::Spin { turns: 1.0, amount: 0.4 },
        TransitionKind::Spin { turns: -0.5, amount: 0.0 },
        TransitionKind::Whip { dir: Dir::Left, blur: 0.6 },
        TransitionKind::Whip { dir: Dir::Down, blur: 1.0 },
        TransitionKind::BlurDissolve { amount: 24.0 },
        TransitionKind::Pixelate { amount: 48.0 },
        TransitionKind::Glitch { amount: 0.5 },
        TransitionKind::Flash { color_r: 255, color_g: 200, color_b: 40, strength: 1.0 },
        TransitionKind::Dip { color_r: 0, color_g: 0, color_b: 0 },
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
                vec![PlanLayer::Dissolve {
                    a: a.layer(),
                    b: b.layer(),
                    k,
                    alpha_k: 4096,
                    mix,
                }],
            );
            let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
            assert!(cc && cg, "{}: план обязан быть полным", kind.tag());
            assert_frames_close(&cpu, &gpu, W, H, soft, &format!("{}@{k}", kind.tag()));
        }
    }
    assert_eq!(seen.len(), 15, "все 15 не-кроссфейдных режимов проверены: {seen:?}");
    assert_eq!(
        seen.len() + 1,
        TransitionKind::ALL.len(),
        "новый вид перехода обязан появиться и в этом списке"
    );

    // сольный фейд С ВИДОМ: вторая сторона — пустота, тот же код
    for (incoming, kind) in [
        (true, TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.2 }),
        (false, TransitionKind::Iris { shape: IrisShape::Circle, softness: 0.1 }),
        (true, TransitionKind::Push { dir: Dir::Up }),
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
