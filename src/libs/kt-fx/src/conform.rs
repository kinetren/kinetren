//! Приведение кадра источника к холсту (Р.62, F-MED-004): ОДНА функция
//! на размер декода, рамку размещения, тумбы, превью и экспорт.
//!
//! Правило («вписать / заполнить / растянуть / как есть») — свойство
//! ПРОЕКТА с исключением у события; здесь оно уже разрешено
//! (`ConformRule`). Вид источника — натив × пиксельное соотношение
//! (`MediaInfo::view_size`), холст — логический размер выхода.
//!
//! Контракт декодера «кадр ровно запрошенного качества» НЕ меняется —
//! меняется, кто считает запрошенное: `conform_geometry(...).decode` даёт
//! размер, в который декодер тянет кадр (при ступени 1), а ведущий узел
//! «Геометрия» в гео-программе (`geom.rs`) кладёт его в рамку `dest`
//! холста; вне рамки — прозрачно.

use crate::plan::MAX_INTER_DIM;
pub use kt_model::ConformRule;

/// Контекст ведущего узла: вид источника (квадратные пиксели) и
/// разрешённое правило. Едет из kt-engine в kt-fx через `VideoFxCtx`
/// и в kt-gpu через `chain_ops`; оба бэкенда получают одни числа.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeadCtx {
    /// Вид кадра источника (ширина × SAR, высота). `None` — вид
    /// неизвестен: правило вырождается в «растянуть» детерминированно.
    pub view: Option<(f64, f64)>,
    pub rule: ConformRule,
}

impl LeadCtx {
    pub fn new(view: Option<(f64, f64)>, rule: ConformRule) -> LeadCtx {
        LeadCtx { view, rule }
    }
}

/// Геометрия слоя: рамка размещения на холсте (логические пиксели, при
/// зуме 1 и положении 0) и размер декода полного кадра (без ступени).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerGeom {
    /// Размер рамки кропнутого источника на холсте.
    pub dest: (f64, f64),
    /// Размер, в который декодер тянет ПОЛНЫЙ кадр (чётный, ≥ 2, с
    /// потолком `MAX_INTER_DIM` пропорционально).
    pub decode: (u32, u32),
}

/// Размер рамки для вида `view` на холсте `canvas` по правилу (зум 1).
/// Нулевой/неизвестный вид — «растянуть».
pub fn conform_dest(view: Option<(f64, f64)>, canvas: (f64, f64), rule: ConformRule) -> (f64, f64) {
    let (cw, ch) = (canvas.0.max(2.0), canvas.1.max(2.0));
    let Some((vw, vh)) = view.filter(|(w, h)| *w > 0.0 && *h > 0.0) else {
        return (cw, ch);
    };
    match rule {
        ConformRule::Stretch => (cw, ch),
        ConformRule::AsIs => (vw, vh),
        ConformRule::Fit => {
            let k = (cw / vw).min(ch / vh);
            (vw * k, vh * k)
        }
        ConformRule::Fill => {
            let k = (cw / vw).max(ch / vh);
            (vw * k, vh * k)
        }
    }
}

/// Полная геометрия: вид источника, холст, правило, кроп (доли источника:
/// слева, сверху, справа, снизу). Кроп режет вид ДО правила: «заполнить»
/// заполняет кропнутым; размер декода — рамка, растянутая обратно на
/// весь кадр (декодер отдаёт кадр целиком, узел вырезает).
pub fn conform_geometry(
    view: Option<(f64, f64)>,
    canvas: (u32, u32),
    rule: ConformRule,
    crop: [f64; 4],
) -> LayerGeom {
    let kx = (1.0 - crop[0].clamp(0.0, 0.95) - crop[2].clamp(0.0, 0.95)).max(0.05);
    let ky = (1.0 - crop[1].clamp(0.0, 0.95) - crop[3].clamp(0.0, 0.95)).max(0.05);
    let cropped = view.map(|(w, h)| (w * kx, h * ky));
    let canvas_f = (canvas.0.max(2) as f64, canvas.1.max(2) as f64);
    let dest = conform_dest(cropped, canvas_f, rule);
    let (mut dw, mut dh) = (dest.0 / kx, dest.1 / ky);
    // потолок промежуточных буферов — проект-константа (п.314), пропорционально
    let cap = MAX_INTER_DIM as f64;
    if dw > cap || dh > cap {
        let k = (cap / dw).min(cap / dh);
        dw *= k;
        dh *= k;
    }
    let even = |v: f64| ((v.round() as u32).max(2)) & !1;
    LayerGeom { dest, decode: (even(dw), even(dh)) }
}
