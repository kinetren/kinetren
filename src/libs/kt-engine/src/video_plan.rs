//! План кадра (Р.29): СТРУКТУРА композиции момента t, построенная до
//! выбора бэкенда. Порядок Vegas, пары dissolve, соло-фейды, уровни —
//! решаются один раз в `VideoRenderer` (video.rs, build_plan); исполнителей
//! два: CPU (`compose_from_cache*` — байт-эталон и фолбэк, инвариант №11)
//! и GPU (крейт kt-gpu, wgpu f32 — инжектится сверху, kt-engine от него
//! не зависит). «Путь рендеринга один» (инвариант №7) = один построитель
//! плана; бэкенд — параметр исполнения, не второй путь.

use std::borrow::Cow;
use std::sync::Arc;

use kt_cache::SourceId;
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

/// Разрешённая 3D-камера слоя (Р.29): активная/прибитая камера с уже
/// интерполированными кейфреймами. Считается при построении плана —
/// исполнитель получает готовые параметры.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cam3d {
    pub cam: kt_model::CameraParams,
    /// Мировой масштаб: quality px / project px (превью меньше проекта,
    /// а координаты Pan3d/камер — в px ПРОЕКТА).
    pub world_scale: f64,
}

/// Источник одного слоя: пре-FX кадр (уже прочитан из кэша при построении
/// плана — recency бампнут ВСЕГДА, полоски прогретости не «остывают») +
/// цепочка FX клипа.
pub struct LayerSrc<'a> {
    pub source: SourceId,
    /// Индекс пре-FX кадра в кэше (pts источника в тиках).
    pub cache_idx: i64,
    /// Пре-FX кадр; None — голод. Исполнители решают сами: CPU может
    /// вернуть мемоизированный пост-FX кадр и без него, GPU — нет.
    pub frame: Option<Arc<VideoFrame>>,
    /// Цепочка С УЖЕ применённой автоматизацией (Р.30): кривые разрешаются
    /// один раз при построении плана — оба исполнителя видят готовые
    /// параметры. Без кривых — заём из VideoClip (ноль клонов).
    pub fx: Cow<'a, [FxInstance]>,
    /// Ключ пост-FX мемоизации CPU-пути.
    pub fx_hash: u64,
    /// Прогресс клипа 0..1 (CamMotion).
    pub u: f64,
    /// Some — в цепочке есть 3D-эффект (камера разрешена по t). CPU-путь
    /// 3D не считает (клип рисуется без эффекта, notice — решение Р.29).
    pub cam3d: Option<Cam3d>,
}

pub enum PlanLayer<'a> {
    /// Обычный слой или соло-фейд. `alpha_k` (0..4096) — уровень × вес
    /// фейда; `scale_alpha` — прогонять через масштабирование альфы даже
    /// при 4096 (v1-семантика соло-фейда: кадр без альфы получает A=255,
    /// а не копию сырых байт альфы).
    One { src: LayerSrc<'a>, alpha_k: u32, scale_alpha: bool },
    /// Пара перехода: dst = a·(1−k) + b·k (k из 4096 долей), затем альфа
    /// × alpha_k (уровни соседей уже смешаны тем же весом при построении).
    Dissolve { a: LayerSrc<'a>, b: LayerSrc<'a>, k: u32, alpha_k: u32 },
}

/// План композиции кадра момента t: слои back-to-front (порядок Vegas
/// уже применён), размер — Quality вызывающего.
pub struct FramePlan<'a> {
    pub t: Tick,
    pub out_w: u32,
    pub out_h: u32,
    pub layers: Vec<PlanLayer<'a>>,
    /// В кадре есть 3D-эффекты (CPU-фолбэк их не отобразит — notice).
    pub has_3d: bool,
}

/// Ошибка GPU-исполнителя. `DeviceLost` — вызывающий «отравляет» GPU до
/// конца сессии (CPU-фолбэк + notice); `TooLarge` — фолбэк только для
/// этого плана (источник больше лимита текстур устройства).
#[derive(Debug)]
pub enum GpuExecError {
    DeviceLost,
    Validation(String),
    TooLarge { w: u32, h: u32 },
}

/// Паспорт бэкенда (notice и строка перф-панели).
#[derive(Debug, Clone)]
pub struct GpuBackendInfo {
    pub name: String,
    pub backend: String,
    /// Софт-растеризатор (llvmpipe/lavapipe) — корректен, считается «GPU
    /// есть» (Р.29); флаг только для диагностики.
    pub software: bool,
}

/// GPU-исполнитель плана (реализация — крейт kt-gpu; kt-engine знает
/// только трейт и получает исполнителя инжекцией сверху).
pub trait VideoGpuExec: Send + Sync {
    /// План → RGBA8 straight в `out` (readback). Ok(complete) — те же
    /// правила «playback не ждёт» (инвариант №5), что у CPU-исполнителя;
    /// отличие одно: мемоизированный пост-FX кадр CPU-пути GPU не видит,
    /// слой без пре-FX кадра для него всегда голод.
    fn execute_into(&self, plan: &FramePlan<'_>, out: &mut Vec<u8>)
    -> Result<bool, GpuExecError>;
    fn info(&self) -> &GpuBackendInfo;
}
