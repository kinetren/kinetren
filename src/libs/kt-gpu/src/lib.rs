//! GPU-бэкенд видео-FX и композита (Р.29): wgpu f32/целочисленные
//! фрагментные пассы поверх устройства eframe (превью) или headless
//! (экспорт/kt-cli). Реализует kt_engine::VideoGpuExec; kt-engine от
//! этого крейта НЕ зависит — исполнитель инжектится сверху (kt-app,
//! kt-cli). CPU-путь kt-fx/kt-engine остаётся байт-эталоном; конформанс
//! GPU-vs-CPU — tests/parity.rs (допуски, t181/t182/t183).

mod context;
mod engine;
mod pool;

pub use context::{GpuContext, GpuInitError};
pub use engine::GpuFxEngine;

/// Единая методика сравнения кадров GPU-vs-CPU (Р.29): ad-hoc epsilon в
/// тестах запрещены — только этот хелпер.
#[derive(Clone, Copy)]
pub struct FrameTol {
    /// |Δ| ≤ max_lsb для «обычных» пикселей.
    pub max_lsb: u32,
    /// Доля пикселей, которым разрешено превысить max_lsb.
    pub outlier_frac: f64,
    /// Потолок |Δ| даже для выбросов (255 = без ограничения — пороговые
    /// эффекты флипают 0↔255 у границы порога).
    pub outlier_max: u32,
    /// Средний |Δ| по всем каналам.
    pub mean_lsb: f64,
}

impl FrameTol {
    pub const EXACT: FrameTol =
        FrameTol { max_lsb: 0, outlier_frac: 0.0, outlier_max: 0, mean_lsb: 0.0 };
    pub const BILINEAR: FrameTol =
        FrameTol { max_lsb: 2, outlier_frac: 0.001, outlier_max: 4, mean_lsb: 0.5 };
    pub const THRESHOLD: FrameTol =
        FrameTol { max_lsb: 2, outlier_frac: 0.005, outlier_max: 255, mean_lsb: 1.0 };
    /// Слитые гео-серии с Pan3d (Р.44): внутри квада — билинейная точность;
    /// СИЛУЭТ (границы квада, тест w≤0) решается f32-предикатами против f64.
    /// Замер (lavapipe, t181 geo3d + t187 + t301): worst |Δ| = 1–2 LSB,
    /// доля |Δ|=1 до 6.2 %, mean ≤ 0.07, выбросов НЕТ ВОВСЕ — предикаты
    /// сошлись на всех пикселях. Бюджет outlier'ов оставлен на драйверы с
    /// FMA-контракцией предикатов (кросс-бэкенд битовость недостижима,
    /// п.308): контурные пиксели там флипают на полную величину.
    pub const GEOM: FrameTol =
        FrameTol { max_lsb: 2, outlier_frac: 0.002, outlier_max: 255, mean_lsb: 0.5 };
}

/// Панихида по расхождению: где, сколько, гистограмма. Паникует при
/// нарушении допуска.
pub fn assert_frames_close(cpu: &[u8], gpu: &[u8], w: u32, h: u32, tol: FrameTol, label: &str) {
    assert_eq!(cpu.len(), gpu.len(), "{label}: buffer sizes differ");
    assert_eq!(cpu.len(), (w * h * 4) as usize, "{label}: size {w}x{h}");
    let mut worst = 0u32;
    let mut worst_at = (0u32, 0u32, 0usize);
    let mut over = 0usize;
    let mut sum = 0u64;
    let mut hist = [0usize; 9]; // 0,1,2,3,4,5-8,9-16,17-64,65+
    for (i, (c, g)) in cpu.iter().zip(gpu.iter()).enumerate() {
        let d = (*c as i32 - *g as i32).unsigned_abs();
        sum += d as u64;
        let bucket = match d {
            0..=4 => d as usize,
            5..=8 => 5,
            9..=16 => 6,
            17..=64 => 7,
            _ => 8,
        };
        hist[bucket] += 1;
        if d > tol.max_lsb {
            over += 1;
        }
        if d > worst {
            worst = d;
            let px = i / 4;
            worst_at = (px as u32 % w, px as u32 / w, i % 4);
        }
    }
    let total = cpu.len();
    let mean = sum as f64 / total as f64;
    let frac = over as f64 / total as f64;
    let ok = worst <= tol.outlier_max.max(tol.max_lsb)
        && frac <= tol.outlier_frac
        && mean <= tol.mean_lsb;
    assert!(
        ok,
        "{label}: GPU diverged from CPU: worst |d|={worst} @({},{}) ch{}, \
         >|{}| at {:.4}% (allowed {:.4}%), mean {:.4} (allowed {}), hist {:?}",
        worst_at.0,
        worst_at.1,
        worst_at.2,
        tol.max_lsb,
        frac * 100.0,
        tol.outlier_frac * 100.0,
        mean,
        tol.mean_lsb,
        hist
    );
}

/// Адаптер для тестов: skip с печатью, если wgpu-адаптера НЕТ;
/// `KT_REQUIRE_GPU=1` превращает и этот пропуск в провал (калька
/// KT_REQUIRE_CROSS_RENDER). lavapipe/llvmpipe — полноценный адаптер.
///
/// Адаптер ЕСТЬ, а движок не собрался (шейдер/пайплайн) — это ВСЕГДА
/// провал, не skip: урок Р.31 — сломанный layout Pan3d ронял создание
/// движка целиком, а тесты вакуумно зеленели через «skip без адаптера»
/// и дефект доехал до машины пользователя.
pub fn test_engine() -> Option<std::sync::Arc<GpuFxEngine>> {
    let ctx = match GpuContext::new_headless() {
        Ok(c) => c,
        Err(err) => {
            if std::env::var("KT_REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("KT_REQUIRE_GPU=1, but no wgpu adapter: {err}");
            }
            eprintln!("skipped: wgpu adapter unavailable ({err}); apt install mesa-vulkan-drivers");
            return None;
        }
    };
    let name = ctx.adapter_info.name.clone();
    match GpuFxEngine::new(ctx) {
        Ok(e) => Some(e),
        Err(err) => panic!("adapter '{name}' is present, but the GPU engine failed to build: {err}"),
    }
}
