//! График АЧХ эквалайзера (Р.48, п.347).
//!
//! Семь чисел в сетке — честно, но чтобы понять, что они делают со
//! звуком, их надо крутить и слушать. График показывает результат сразу
//! и правит его мышью: точка полосы тянется по частоте и усилению,
//! колесо над средней правит добротность.
//!
//! Два правила, из которых всё следует:
//! - **кривая считается тем же кодом, что звучит** (`kt_fx::eq_mag_db` по
//!   коэффициентам настоящего `Biquad`) — картинка не может разойтись с
//!   DSP;
//! - **драг пишет ТУДА ЖЕ, куда писало бы поле** (через `CurveBind`:
//!   база или выбранный ключ кривой) — парные способы одного действия
//!   ведут себя одинаково (правило 17 чек-листа).

use eframe::egui;
use kt_model::FxInstance;

use crate::i18n::{k, tr};

/// Высота графика. Больше не нужно: кривая читается, а место в окне FX
/// дорого (правило 13 — не тратить место впустую).
const H: f32 = 140.0;
/// Границы оси частот — слышимый диапазон, а не диапазоны спек (у полос
/// они разные, а ось одна).
const HZ_MIN: f64 = 20.0;
const HZ_MAX: f64 = 20_000.0;

/// Цвета точек полос: низ тёплый, середина зелёная, верх голубой.
const BAND_COLORS: [egui::Color32; 3] = [
    egui::Color32::from_rgb(232, 160, 76),
    egui::Color32::from_rgb(123, 201, 111),
    egui::Color32::from_rgb(90, 167, 232),
];

/// Имена параметров полосы: (частота, усиление).
const BANDS: [(&str, &str); 3] = [
    ("low_freq_hz", "low_gain_db"),
    ("mid_freq_hz", "mid_gain_db"),
    ("high_freq_hz", "high_gain_db"),
];

/// Частота → доля ширины (лог-шкала: декады равны по ширине).
pub(crate) fn hz_to_u(hz: f64) -> f32 {
    ((hz.max(1.0) / HZ_MIN).ln() / (HZ_MAX / HZ_MIN).ln()) as f32
}

/// Обратно: доля ширины → частота.
pub(crate) fn u_to_hz(u: f32) -> f64 {
    HZ_MIN * (HZ_MAX / HZ_MIN).powf(u.clamp(0.0, 1.0) as f64)
}

/// Захваченная полоса живёт в памяти egui, а не в полях окна: виджет
/// зовут ДВА окна (события и морфера), и своё состояние заводить каждому
/// было бы дублированием.
fn grabbed(ctx: &egui::Context, id: egui::Id) -> Option<usize> {
    ctx.memory(|m| m.data.get_temp::<usize>(id))
}

/// Нарисовать график и обработать жесты. Возвращает, что изменилось —
/// база или кривая (как и числовые поля).
pub fn draw(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    bind: Option<&mut crate::widgets::CurveBind<'_>>,
    hover: &mut Option<String>,
) -> crate::widgets::ParamsChanged {
    let mut ch = crate::widgets::ParamsChanged::default();
    let id = ui.id().with("eq_graph");
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), H), egui::Sense::click_and_drag());
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, ui.is_enabled(), "eq_graph")
    });
    crate::app::hint_into(hover, resp.clone(), tr(k::fx::EQ_GRAPH_HINT));
    crate::widgets::field_probe::note("eq_graph", rect);

    // Диапазон оси значений — из спеки усиления, а не из литерала: у
    // параметров один источник правды (kt-model).
    let specs = kt_model::HasParams::param_specs(f);
    let gspec = specs.iter().find(|s| s.name == "low_gain_db").expect("eq has gain specs");
    let (lo, hi) = (gspec.min, gspec.max);
    let plot = rect.shrink2(egui::vec2(2.0, 6.0));
    let x_of = |hz: f64| plot.left() + hz_to_u(hz) * plot.width();
    let y_of = |db: f64| plot.top() + ((hi - db) / (hi - lo)) as f32 * plot.height();
    let db_of = |y: f32| hi - ((y - plot.top()) / plot.height()).clamp(0.0, 1.0) as f64 * (hi - lo);

    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, egui::Color32::from_gray(18));
    let grid = egui::Stroke::new(1.0, egui::Color32::from_gray(38));
    let axis = egui::Stroke::new(1.0, egui::Color32::from_gray(70));
    // сетка частот: декады ярче, между ними — 2 и 5
    for hz in [20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0, 20_000.0] {
        let x = x_of(hz);
        let decade = matches!(hz as i64, 100 | 1000 | 10_000);
        p.line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            if decade { axis } else { grid },
        );
        if decade {
            let label =
                if hz >= 1000.0 { format!("{:.0}k", hz / 1000.0) } else { format!("{hz:.0}") };
            p.text(
                egui::pos2(x + 3.0, plot.bottom() - 2.0),
                egui::Align2::LEFT_BOTTOM,
                label,
                egui::FontId::proportional(9.0),
                egui::Color32::from_gray(90),
            );
        }
    }
    // сетка усиления каждые 6 дБ, ноль ярче
    let mut db = lo;
    while db <= hi + 0.01 {
        let y = y_of(db);
        p.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            if db == 0.0 { axis } else { grid },
        );
        if db == 0.0 || db == hi || db == lo {
            p.text(
                egui::pos2(plot.left() + 2.0, y),
                egui::Align2::LEFT_CENTER,
                format!("{db:+.0}"),
                egui::FontId::proportional(9.0),
                egui::Color32::from_gray(90),
            );
        }
        db += 6.0;
    }

    // Значения — ровно те, что показывают поля: база или выбранный ключ
    // кривой. Снимаются ОДИН раз за кадр: дальше и рисование, и жесты
    // работают с этим снимком (заодно не спорят с borrow checker'ом).
    let mut bind = bind;
    let names = [
        "low_gain_db",
        "low_freq_hz",
        "mid_gain_db",
        "mid_freq_hz",
        "mid_q",
        "high_gain_db",
        "high_freq_hz",
    ];
    let mut vals = [0f32; 7];
    for (i, name) in names.iter().enumerate() {
        vals[i] = shown(f, &bind, name) as f32;
    }

    // кривая отклика: точек по ширине, но не больше разумного
    let n = (plot.width() / 3.0).clamp(64.0, 200.0) as usize;
    let pts: Vec<egui::Pos2> = (0..=n)
        .map(|i| {
            let hz = u_to_hz(i as f32 / n as f32);
            let db = kt_fx::eq_mag_db(&vals, hz as f32).clamp(lo - 2.0, hi + 2.0);
            egui::pos2(x_of(hz), y_of(db))
        })
        .collect();
    p.add(egui::Shape::line(pts, egui::Stroke::new(1.5, egui::Color32::from_gray(215))));

    // точки полос: (индекс частоты, индекс усиления) в снимке значений
    const IDX: [(usize, usize); 3] = [(1, 0), (3, 2), (6, 5)];
    let centers: [egui::Pos2; 3] = std::array::from_fn(|i| {
        let (fi, gi) = IDX[i];
        egui::pos2(x_of(vals[fi] as f64), y_of((vals[gi] as f64).clamp(lo, hi)))
    });
    let hovered = resp.hover_pos();
    let grab = grabbed(ui.ctx(), id);
    for (i, &c) in centers.iter().enumerate() {
        let near =
            grab == Some(i) || hovered.is_some_and(|h| h.distance(c) <= 9.0 && grab.is_none());
        // «усы» середины показывают ширину полосы — иначе добротность,
        // которую правит колесо, никак не видна
        if i == 1 {
            let q = (vals[4] as f64).max(0.1);
            let f0 = vals[3] as f64;
            let (a, b) = (f0 * (1.0 - 0.5 / q).max(0.05), f0 * (1.0 + 0.5 / q));
            p.line_segment(
                [egui::pos2(x_of(a), c.y), egui::pos2(x_of(b), c.y)],
                egui::Stroke::new(1.0, BAND_COLORS[i].gamma_multiply(0.5)),
            );
        }
        p.circle(
            c,
            if near { 6.0 } else { 4.5 },
            if near { egui::Color32::WHITE } else { BAND_COLORS[i] },
            egui::Stroke::new(1.5, BAND_COLORS[i]),
        );
    }

    // --- жесты ---------------------------------------------------------
    if resp.drag_started()
        && let Some(pos) = resp.interact_pointer_pos()
        && let Some(i) = (0..3).find(|i| pos.distance(centers[*i]) <= 9.0)
    {
        ui.ctx().memory_mut(|m| m.data.insert_temp(id, i));
    }
    if resp.dragged()
        && let Some(i) = grabbed(ui.ctx(), id)
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let (fname, gname) = BANDS[i];
        let fspec = specs.iter().find(|s| s.name == fname).expect("freq spec");
        let hz = u_to_hz((pos.x - plot.left()) / plot.width()).clamp(fspec.min, fspec.max);
        let db = db_of(pos.y).clamp(gspec.min, gspec.max);
        // писать ТОЛЬКО при реальном изменении: иначе ревизия модели
        // растёт каждый кадр жеста и зря сбрасывает кэши кадра
        let (fi, gi) = IDX[i];
        if (hz - vals[fi] as f64).abs() > 1e-6 {
            ch |= write(f, &mut bind, fname, hz);
        }
        if (db - vals[gi] as f64).abs() > 1e-6 {
            ch |= write(f, &mut bind, gname, db);
        }
        // подпись значений у руки: пользователь видит, что он делает
        let c = centers[i];
        let label = if i == 1 {
            format!("{} {:+.1} dB  Q {:.2}", fmt_hz(hz), db, vals[4])
        } else {
            format!("{} {:+.1} dB", fmt_hz(hz), db)
        };
        p.text(
            egui::pos2(c.x + 10.0, c.y - 10.0),
            egui::Align2::LEFT_BOTTOM,
            label,
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
    }
    if resp.drag_stopped() {
        ui.ctx().memory_mut(|m| m.data.remove::<usize>(id));
    }

    // Колесо правит добротность СЕРЕДИНЫ и только у её точки: иначе
    // прокрутка окна FX (`.vscroll(true)`) уехала бы вместе с жестом.
    if let Some(h) = hovered
        && h.distance(centers[1]) <= 12.0
    {
        let mut delta = 0.0f32;
        ui.input_mut(|inp| {
            inp.events.retain(|e| match e {
                egui::Event::MouseWheel { delta: d, .. } => {
                    delta += d.y;
                    false
                }
                _ => true,
            });
            if delta != 0.0 {
                // жест поглощён: иначе тем же колесом прокрутится окно FX
                inp.smooth_scroll_delta = egui::Vec2::ZERO;
            }
        });
        if delta != 0.0 {
            let qspec = specs.iter().find(|s| s.name == "mid_q").expect("q spec");
            let q = (vals[4] as f64 + delta as f64 * 0.1).clamp(qspec.min, qspec.max);
            if (q - vals[4] as f64).abs() > 1e-9 {
                ch |= write(f, &mut bind, "mid_q", q);
            }
        }
    }
    ch
}

/// Что показывает поле этого параметра — база или значение кривой.
fn shown(f: &FxInstance, bind: &Option<&mut crate::widgets::CurveBind<'_>>, name: &str) -> f64 {
    match bind {
        Some(b) => b.shown(f, name),
        None => f.get_param(name).unwrap_or(0.0),
    }
}

fn fmt_hz(hz: f64) -> String {
    if hz >= 1000.0 { format!("{:.2}k Hz", hz / 1000.0) } else { format!("{hz:.0} Hz") }
}

/// Записать значение туда же, куда его записало бы числовое поле.
fn write(
    f: &mut FxInstance,
    bind: &mut Option<&mut crate::widgets::CurveBind<'_>>,
    name: &str,
    v: f64,
) -> crate::widgets::ParamsChanged {
    match bind {
        Some(b) if b.has_curve(f, name) => {
            b.apply(f, name, v);
            crate::widgets::ParamsChanged { base: false, curve: true }
        }
        _ => {
            kt_model::HasParams::set_param(f, name, v);
            crate::widgets::ParamsChanged { base: true, curve: false }
        }
    }
}
