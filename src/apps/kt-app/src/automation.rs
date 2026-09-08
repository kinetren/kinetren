//! Единый виджет автоматизации (Р.33 С3).
//!
//! Требование пользователя дословно: «Хотелось бы унифицированный виджет
//! управления автоматизацией, и чтобы можно было переключаться
//! график<->таблица-список… График хотелось бы удобный как таймлайн
//! проекта сделать (переиспользовать код виджета). А в текущем
//! таблице-списке нужно несколько параметров на строку иметь а не по
//! одной, и перемещать их мышкой (причем сейчас есть дебильность —
//! программа вроде сбрасывает/сортирует рядом стоящие значения если я
//! ввиду после t=1.0 следующей строчкой t=0.5)… Ну и превью не хватает
//! там, гадаешь не пойми как по нормализованному времени, а можно было бы
//! сделать так, что курсор внутри мини-таймлайна двигает курсор в
//! основном таймлайне… Естественно надо и реверсирование учитывать и
//! скорость».
//!
//! Поэтому здесь: ОДИН виджет на все кривые слота, линейка и снап из
//! таймлайна проекта, ключи в секундах материала (доля клипа — второй
//! строкой, не нагромождая), таблица со всеми параметрами в строке и
//! сортировкой ТОЛЬКО на отпускании, мини-таймлайн, двигающий главный
//! курсор.

use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::{ClipEvent, FxInstance, ParamCurve, ParamSpec};
use kt_time::Tick;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutoMode {
    #[default]
    Graph,
    Table,
}

/// Состояние виджета (живёт в окне, не в модели).
#[derive(Debug, Clone)]
pub struct AutoView {
    pub mode: AutoMode,
    /// Выбранный ключ: (кривая, индекс).
    pub sel: Option<(usize, usize)>,
    /// Ключ под мышью во время драга.
    pub drag: Option<(usize, usize)>,
    /// Кривая, в которую кладёт ключ клик по пустому месту графика.
    pub active: usize,
    /// Ось времени графика СВЯЗАНА с окном главного таймлайна (Р.38 п.14,
    /// дословно: «в таймлайне автоматизации я не понимаю как зазумится»).
    /// По умолчанию включено — решение пользователя: зум один на оба, и
    /// специально настраивать программу, чтобы она вела себя нормально, не
    /// нужно (правило 4 чек-листа). Колесо над графиком замок снимает.
    pub linked: bool,
    /// Собственное окно оси (домен кривой), когда замок снят.
    pub win: Option<(i64, i64)>,
    /// Незакоммиченный ввод времени в таблице: (кривая, ключ, секунды).
    /// Пока строка редактируется, ключи НЕ пересортировываются — иначе
    /// строка уезжает из-под рук на каждой цифре.
    pub t_edit: Option<(usize, usize, f64)>,
    /// Замороженное окно значений тянущейся кривой: (кривая, lo, hi).
    /// Без заморозки шкала пересчитывалась бы по самим же ключам и
    /// «дышала» под курсором.
    pub drag_win: Option<(usize, f64, f64)>,
}

impl Default for AutoView {
    fn default() -> Self {
        AutoView {
            mode: AutoMode::default(),
            sel: None,
            drag: None,
            active: 0,
            // замок ВКЛЮЧЁН по умолчанию (решение пользователя, п.14)
            linked: true,
            win: None,
            t_edit: None,
            drag_win: None,
        }
    }
}

impl AutoView {
    /// Видимое окно оси времени в домене кривой. Связанный график
    /// показывает то же, что главный таймлайн; отвязанный — своё окно.
    /// И то и другое клампится диапазоном носителя: за его пределами
    /// кривой нет, и пустое поле только путает.
    pub fn axis(&self, ctx: &AutoCtx<'_>) -> (i64, i64) {
        let (r0, r1) = (ctx.range.0.0, ctx.range.1.0.max(ctx.range.0.0 + 1));
        let (a, b) = match (self.linked, self.win) {
            (true, _) => (ctx.view.0.0, ctx.view.1.0),
            (false, Some(w)) => w,
            (false, None) => (r0, r1),
        };
        let (a, b) = (a.clamp(r0, r1), b.clamp(r0, r1));
        // окно уже кадра бессмысленно и делает деление на ноль
        if b - a < ctx.frame_step.max(1) { (r0, r1) } else { (a, b) }
    }
}

/// Окно значений кривой: замороженное на время драга или посчитанное по
/// её ключам.
fn win_of(c: &ParamCurve, spec: &ParamSpec, view: &AutoView, ci: usize) -> (f64, f64) {
    match view.drag_win {
        Some((i, lo, hi)) if i == ci => (lo, hi),
        _ => value_window(c, spec),
    }
}

/// Что виджету нужно знать о событии.
/// Владелец кривых глазами виджета (Р.34 К1). Виджет больше НЕ знает про
/// клип: ему нужны только спеки параметров, диапазон оси времени и
/// перевод «позиция кривой ↔ тик таймлайна». Благодаря этому один и тот
/// же виджет обслуживает слот эффекта, камеру и переход — иначе у каждого
/// нового носителя параметров заводился бы свой редактор кейфреймов, а
/// именно так и разъехались slowed с pan3d (п.221).
pub struct AutoCtx<'a> {
    /// Спеки параметров носителя (min/max/шаг/единица).
    pub specs: &'a [ParamSpec],
    /// ПОЛНЫЙ диапазон оси времени в домене кривой (границы носителя).
    pub range: (Tick, Tick),
    /// Окно ГЛАВНОГО таймлайна в домене кривой — то, что график
    /// показывает при включённом замке (Р.38 п.14).
    pub view: (Tick, Tick),
    /// Домен кривой → тик таймлайна (мини-таймлайн двигает главный курсор).
    pub to_timeline: &'a dyn Fn(Tick) -> Tick,
    /// Тик таймлайна → домен кривой (позиция курсора на графике).
    pub to_curve: &'a dyn Fn(Tick) -> Tick,
    /// Шаг кадра проекта — им выравнивается позиция ключа.
    pub frame_step: i64,
}

impl AutoCtx<'_> {
    /// Доля диапазона (вторая подпись времени: «секунды и проценты»).
    fn frac(&self, t: Tick) -> f64 {
        let span = (self.range.1.0 - self.range.0.0).max(1);
        ((t.0 - self.range.0.0) as f64 / span as f64).clamp(0.0, 1.0)
    }

    fn spec(&self, param: &str) -> Option<&ParamSpec> {
        self.specs.iter().find(|s| s.name == param)
    }
}

/// Окно главного таймлайна в домене кривой. Переводится ТЕМ ЖЕ
/// отображением, что и всё остальное; у реверснутого носителя концы
/// меняются местами — поэтому min/max, а не «первый-второй».
pub fn view_in_curve(
    to_curve: &dyn Fn(Tick) -> Tick,
    tl_view: (Tick, Tick),
) -> (Tick, Tick) {
    let (a, b) = (to_curve(tl_view.0).0, to_curve(tl_view.1).0);
    (Tick(a.min(b)), Tick(a.max(b)))
}

/// Контекст для слота эффекта на событии — прежнее поведение виджета
/// одной строкой (домен = материал клипа).
pub fn clip_ctx<'a>(
    clip: &'a ClipEvent,
    fx: &'a FxInstance,
    frame_step: i64,
    to_tl: &'a dyn Fn(Tick) -> Tick,
    to_mat: &'a dyn Fn(Tick) -> Tick,
    tl_view: (Tick, Tick),
) -> AutoCtx<'a> {
    AutoCtx {
        specs: fx.param_specs(),
        range: (clip.source_in, Tick(clip.source_in.0 + clip.duration.0.max(1))),
        view: view_in_curve(to_mat, tl_view),
        to_timeline: to_tl,
        to_curve: to_mat,
        frame_step,
    }
}

#[derive(Default)]
pub struct AutoResult {
    pub changed: bool,
    /// Куда двинуть ГЛАВНЫЙ курсор (жест по линейке мини-таймлайна).
    pub seek: Option<Tick>,
}

fn secs(t: Tick) -> f64 {
    t.0 as f64 / 90_000.0
}

pub fn curve_color(i: usize) -> egui::Color32 {
    const C: [egui::Color32; 6] = [
        egui::Color32::from_rgb(120, 180, 255),
        egui::Color32::from_rgb(255, 190, 120),
        egui::Color32::from_rgb(140, 230, 150),
        egui::Color32::from_rgb(230, 140, 210),
        egui::Color32::from_rgb(230, 220, 130),
        egui::Color32::from_rgb(160, 200, 220),
    ];
    C[i % C.len()]
}

/// Окно значений кривой по ВЕРТИКАЛИ. Берётся по её собственным ключам с
/// запасом, а не по всему диапазону спеки: у скорости spec — 0.25..32, и
/// на полном диапазоне обычная кривая 0.7..1.0 вырождается в прямую у
/// нижнего края (ровно та жалоба, что была про старый график slowed).
fn value_window(c: &ParamCurve, spec: &ParamSpec) -> (f64, f64) {
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for k in &c.keys {
        lo = lo.min(k.v);
        hi = hi.max(k.v);
    }
    if !lo.is_finite() || !hi.is_finite() {
        return (spec.min, spec.max);
    }
    let span = (hi - lo).max((spec.max - spec.min) * 0.02);
    let pad = span * 0.15;
    ((lo - pad).max(spec.min), (hi + pad).min(spec.max))
}

/// Виджет целиком: переключатель вида + график или таблица.
pub fn draw(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    ctx: &AutoCtx<'_>,
    curves: &mut [ParamCurve],
    view: &mut AutoView,
    playhead: Tick,
    hover: &mut Option<String>,
) -> AutoResult {
    let mut out = AutoResult::default();
    ui.horizontal(|ui| {
        ui.label(tr(k::fx::AUTO_CLIP));
        // переключение вида — требование «чтобы можно было переключаться
        // график<->таблица-список»
        for (m, label) in
            [(AutoMode::Graph, tr(k::fx::VIEW_GRAPH)), (AutoMode::Table, tr(k::fx::VIEW_TABLE))]
        {
            if ui.selectable_label(view.mode == m, label).clicked() {
                view.mode = m;
            }
        }
        if let Some((c, k)) = view.sel
            && let Some(key) = curves.get(c).and_then(|c| c.keys.get(k))
        {
            // ОБА времени сразу, но одной строкой: секунды материала и
            // доля события (пользователь: «писать и одно и другое, но не
            // нагромождая»)
            ui.weak(crate::tf!(
                k::fx::KEY_AT,
                format!("{:.3}", secs(key.t)),
                format!("{:.0}", ctx.frac(key.t) * 100.0)
            ));
        }
    });
    if curves.is_empty() {
        return out;
    }
    // активная кривая могла исчезнуть (удалили параметр) — иначе клик по
    // пустому месту молча ушёл бы в никуда
    view.active = view.active.min(curves.len() - 1);
    toolbar(ui, ctx, curves, view, playhead, &mut out, hover);
    match view.mode {
        AutoMode::Graph => graph(ui, id, ctx, curves, view, playhead, &mut out, hover),
        AutoMode::Table => table(ui, id, ctx, curves, view, &mut out),
    }
    out
}

/// Кнопки управления элементами графика (Р.38 п.18, дословно: «еще не
/// хватает кнопок управления элементами таймлайна автоматизации»).
///
/// Живут В САМОМ ВИДЖЕТЕ, а не в окне FX: их получают все носители кривых
/// разом — слот эффекта, камера, переход, морфер. Именно так разъехались
/// slowed с pan3d, когда у каждого был свой редактор ключей (п.221).
#[allow(clippy::too_many_arguments)]
fn toolbar(
    ui: &mut egui::Ui,
    ctx: &AutoCtx<'_>,
    curves: &mut [ParamCurve],
    view: &mut AutoView,
    playhead: Tick,
    out: &mut AutoResult,
    hover: &mut Option<String>,
) {
    ui.horizontal(|ui| {
        // выбор активной кривой — ЦВЕТОМ её линии на графике: иначе
        // «добавил второй параметр, а редактируется первый» (п.17). До
        // Р.38 новый параметр не становился активным вовсе.
        for (ci, c) in curves.iter().enumerate() {
            let name = crate::widgets::param_label(&c.param);
            let text = egui::RichText::new(name).color(curve_color(ci));
            if crate::app::hint_into(
                hover,
                ui.selectable_label(view.active == ci, text),
                tr(k::fx::ACTIVE_CURVE_HINT),
            )
            .clicked()
            {
                view.active = ci;
                view.sel = None;
            }
        }
        ui.separator();
        // ---- ключи ----
        let ph = (ctx.to_curve)(playhead);
        if crate::app::hint_into(
            hover,
            ui.small_button(crate::icons::PLUS),
            tr(k::fx::KEY_ADD_HINT),
        )
        .clicked()
        {
            let c = &mut curves[view.active];
            let v = kt_model::curve_value(&c.keys, c.ease, ph);
            c.keys.retain(|k| k.t != ph);
            c.keys.push(kt_model::AutoKey { t: ph, v });
            c.keys.sort_by_key(|k| k.t.0);
            view.sel = c.keys.iter().position(|k| k.t == ph).map(|ki| (view.active, ki));
            out.changed = true;
        }
        // прыжок по ключам активной кривой двигает ГЛАВНЫЙ курсор: «сразу
        // видно, к чему относится»
        for (icon, fwd, hint) in [
            (crate::icons::SKIP_BACK, false, tr(k::fx::KEY_PREV_HINT)),
            (crate::icons::SKIP_FWD, true, tr(k::fx::KEY_NEXT_HINT)),
        ] {
            if crate::app::hint_into(hover, ui.small_button(icon), hint).clicked() {
                let c = &curves[view.active];
                let next = if fwd {
                    c.keys.iter().position(|k| k.t.0 > ph.0)
                } else {
                    c.keys.iter().rposition(|k| k.t.0 < ph.0)
                };
                if let Some(ki) = next {
                    view.sel = Some((view.active, ki));
                    out.seek = Some((ctx.to_timeline)(c.keys[ki].t));
                }
            }
        }
        let has_sel = view
            .sel
            .is_some_and(|(c, k)| curves.get(c).is_some_and(|c| k < c.keys.len()));
        if crate::app::hint_into(
            hover,
            ui.add_enabled(has_sel, egui::Button::new(crate::icons::TRASH).small()),
            tr(k::fx::KEY_DEL_HINT),
        )
        .clicked()
            && let Some((ci, ki)) = view.sel
        {
            curves[ci].keys.remove(ki);
            view.sel = None;
            out.changed = true;
        }
        ui.separator();
        // ---- ось времени (п.14) ----
        if crate::app::hint_into(
            hover,
            ui.selectable_label(view.linked, crate::icons::LOCK),
            tr(k::fx::AXIS_LINK_HINT),
        )
        .clicked()
        {
            view.linked = !view.linked;
            if !view.linked {
                // снимая замок, оставляем ТО ЖЕ окно, что было видно:
                // «ничего не скачет» (правило 3 чек-листа)
                view.win = Some(view.axis(ctx));
            }
        }
        if crate::app::hint_into(
            hover,
            ui.small_button(crate::icons::ARROWS_OUT),
            tr(k::fx::AXIS_FIT_HINT),
        )
        .clicked()
        {
            view.linked = false;
            view.win = None;
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn graph(
    ui: &mut egui::Ui,
    _id: impl std::hash::Hash + std::fmt::Debug + Copy,
    ctx: &AutoCtx<'_>,
    curves: &mut [ParamCurve],
    view: &mut AutoView,
    playhead: Tick,
    out: &mut AutoResult,
    hover: &mut Option<String>,
) {
    const RULER_H: f32 = 16.0;
    let h = 150.0;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, egui::Color32::from_gray(18));

    // ось X — ВИДИМОЕ окно (замок к таймлайну или своё); шкала и снап
    // взяты у таймлайна проекта
    let (m0, m1) = view.axis(ctx);
    let tpp = (m1 - m0) as f64 / rect.width().max(1.0) as f64;

    // колесо над графиком: скролл — пан, Ctrl — зум с якорем под
    // указателем. Те же жесты, что на главном таймлайне (правило 17:
    // парные способы ведут себя одинаково), и любой из них снимает замок.
    if resp.hovered() {
        let (scroll, zoom) = ui.input(|i| {
            let mut dx = 0.0f32;
            let mut z = 1.0f32;
            for e in &i.events {
                if let egui::Event::MouseWheel { delta, modifiers, .. } = e {
                    if modifiers.ctrl || modifiers.command {
                        z *= (1.0 + delta.y * 0.15).clamp(0.2, 5.0);
                    } else {
                        dx += delta.x + delta.y;
                    }
                }
            }
            (dx, z)
        });
        if scroll != 0.0 || zoom != 1.0 {
            let anchor = ui
                .input(|i| i.pointer.hover_pos())
                .map(|p| ((p.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0) as f64)
                .unwrap_or(0.5);
            let span = (m1 - m0) as f64;
            let at = m0 as f64 + span * anchor;
            let ns = (span / zoom as f64).max(ctx.frame_step.max(1) as f64);
            let shift = -scroll as f64 * tpp * 8.0;
            let a = at - ns * anchor + shift;
            view.linked = false;
            view.win = Some((a.round() as i64, (a + ns).round() as i64));
        }
    }
    let x_of = |t: i64| rect.left() + ((t - m0) as f64 / tpp) as f32;
    let t_of = |x: f32| m0 + ((x - rect.left()) as f64 * tpp) as i64;
    let plot = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top() + RULER_H),
        egui::pos2(rect.right(), rect.bottom()),
    );

    // линейка: те же лестница шагов и подписи, что на таймлайне проекта
    let (minor, label) = crate::timeline::ruler_steps(tpp);
    let mut t = m0 - m0.rem_euclid(minor);
    while t <= m1 {
        let x = x_of(t);
        if x >= rect.left() && x <= rect.right() {
            let major = t % label == 0;
            p.line_segment(
                [
                    egui::pos2(x, rect.top()),
                    egui::pos2(x, rect.top() + if major { RULER_H } else { 5.0 }),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_gray(if major { 110 } else { 60 })),
            );
            if major {
                p.text(
                    egui::pos2(x + 3.0, rect.top() + 1.0),
                    egui::Align2::LEFT_TOP,
                    crate::timeline::format_ruler_time_step(t, label),
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_gray(150),
                );
            }
        }
        t += minor;
    }
    // подпись ушедшей влево риски — прижата к краю (Р.40 п.14): ось графика
    // это МАТЕРИАЛ события, при зуме её начало уезжает, и без этого на
    // линейке не оставалось ни одного времени
    {
        let pinned = crate::timeline::ruler_pinned_label(m0, label);
        if x_of(pinned) < rect.left() {
            let next = x_of(pinned + label);
            let g = p.layout_no_wrap(
                crate::timeline::format_ruler_time_step(pinned, label),
                egui::FontId::proportional(10.0),
                egui::Color32::from_gray(120),
            );
            if g.size().x + 6.0 <= next - rect.left() {
                p.galley(
                    egui::pos2(rect.left() + 2.0, rect.top() + 1.0),
                    g,
                    egui::Color32::from_gray(120),
                );
            }
        }
    }

    // курсор главного таймлайна в координатах материала
    let ph_mat = (ctx.to_curve)(playhead);
    if ph_mat.0 >= m0 && ph_mat.0 <= m1 {
        let x = x_of(ph_mat.0);
        p.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 90, 90)),
        );
    }

    // кривые и ключи
    let mut hit: Option<(usize, usize)> = None;
    for (ci, c) in curves.iter().enumerate() {
        let Some(spec) = ctx.spec(&c.param) else { continue };
        let (lo, hi) = win_of(c, spec, view, ci);
        let y_of = |v: f64| -> f32 {
            let u = ((v - lo) / (hi - lo).max(1e-9)).clamp(0.0, 1.0) as f32;
            plot.bottom() - 6.0 - u * (plot.height() - 12.0)
        };
        let col = curve_color(ci);
        if c.keys.len() > 1 {
            let pts: Vec<egui::Pos2> =
                c.keys.iter().map(|k| egui::pos2(x_of(k.t.0), y_of(k.v))).collect();
            p.add(egui::Shape::line(pts, egui::Stroke::new(1.0, col)));
        }
        for (ki, k) in c.keys.iter().enumerate() {
            let (x, y) = (x_of(k.t.0), y_of(k.v));
            let selected = view.sel == Some((ci, ki));
            let r = if selected { 6.0 } else { 4.5 };
            p.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(x, y - r),
                    egui::pos2(x + r, y),
                    egui::pos2(x, y + r),
                    egui::pos2(x - r, y),
                ],
                if selected { egui::Color32::WHITE } else { col },
                egui::Stroke::new(1.0, col),
            ));
            if let Some(pos) = resp.interact_pointer_pos()
                && (pos - egui::pos2(x, y)).length() < 9.0
            {
                hit = Some((ci, ki));
            }
        }
    }
    // подпись активной кривой (в неё кладёт ключ клик по пустому месту)
    if let Some(c) = curves.get(view.active)
        && let Some(spec) = ctx.spec(&c.param)
    {
        let (lo, hi) = win_of(c, spec, view, view.active);
        let col = curve_color(view.active);
        p.text(
            egui::pos2(rect.right() - 6.0, plot.top() + 2.0),
            egui::Align2::RIGHT_TOP,
            crate::widgets::param_label(&c.param),
            egui::FontId::proportional(11.0),
            col,
        );
        // границы шкалы значений: без них непонятно, «насколько» кривая
        // поднимается — а шкала у каждой кривой своя
        for (v, y) in [(hi, plot.top() + 2.0), (lo, plot.bottom() - 12.0)] {
            p.text(
                egui::pos2(rect.left() + 4.0, y),
                egui::Align2::LEFT_TOP,
                format!("{v:.2}"),
                egui::FontId::proportional(10.0),
                egui::Color32::from_gray(120),
            );
        }
    }

    let Some(pos) = resp.interact_pointer_pos() else { return };
    let on_ruler = pos.y < rect.top() + RULER_H;
    // мини-таймлайн: жест по линейке двигает ГЛАВНЫЙ курсор — «сразу
    // видно, к чему относится» (реверс и скорость учитывает обратный
    // перевод материал → таймлайн)
    if on_ruler && (resp.clicked() || resp.dragged()) {
        out.seek = Some((ctx.to_timeline)(Tick(t_of(pos.x))));
        crate::app::hint_into(hover, resp.clone(), tr(k::fx::GRAPH_RULER_HINT));
        return;
    }
    if resp.double_clicked() {
        if let Some((ci, ki)) = hit {
            curves[ci].keys.remove(ki);
            view.sel = None;
            out.changed = true;
        }
        return;
    }
    if resp.drag_started() || resp.clicked() {
        if let Some((ci, ki)) = hit {
            view.sel = Some((ci, ki));
            view.active = ci;
            view.drag = Some((ci, ki));
            if let Some(spec) = ctx.spec(&curves[ci].param) {
                let (lo, hi) = value_window(&curves[ci], spec);
                view.drag_win = Some((ci, lo, hi));
            }
        } else if resp.clicked() {
            // клик по пустому месту — новый ключ активной кривой
            if let Some(spec) = curves
                .get(view.active)
                .and_then(|c| ctx.spec(&c.param))
                .copied()
            {
                let (lo, hi) = win_of(&curves[view.active], &spec, view, view.active);
                let c = &mut curves[view.active];
                let t = Tick(t_of(pos.x).clamp(m0, m1));
                let u = ((plot.bottom() - 6.0 - pos.y) / (plot.height() - 12.0)).clamp(0.0, 1.0);
                let v = (lo + (hi - lo) * u as f64).clamp(spec.min, spec.max);
                c.keys.retain(|k| k.t != t);
                c.keys.push(kt_model::AutoKey { t, v });
                c.keys.sort_by_key(|k| k.t.0);
                view.sel = c.keys.iter().position(|k| k.t == t).map(|ki| (view.active, ki));
                out.changed = true;
            }
        }
    }
    if resp.dragged()
        && let Some((ci, ki)) = view.drag
        && ci < curves.len()
        && ki < curves[ci].keys.len()
    {
        let Some(spec) = ctx.spec(&curves[ci].param).copied() else { return };
        // снап по сетке кадров: ключ садится на кадр, а не «около»
        let raw = t_of(pos.x).clamp(m0, m1);
        let step = ctx.frame_step.max(1);
        let t = Tick(raw - raw.rem_euclid(step));
        let (lo, hi) = win_of(&curves[ci], &spec, view, ci);
        let u = ((plot.bottom() - 6.0 - pos.y) / (plot.height() - 12.0)).clamp(0.0, 1.0);
        let v = (lo + (hi - lo) * u as f64).clamp(spec.min, spec.max);
        curves[ci].keys[ki] = kt_model::AutoKey { t, v };
        // порядок восстанавливаем сразу, но выбор следует за СВОИМ ключом
        curves[ci].keys.sort_by_key(|k| k.t.0);
        view.sel = curves[ci].keys.iter().position(|k| k.t == t).map(|ki| (ci, ki));
        view.drag = view.sel;
        out.changed = true;
    }
    if resp.drag_stopped() {
        view.drag = None;
        view.drag_win = None;
    }
}

fn table(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    ctx: &AutoCtx<'_>,
    curves: &mut [ParamCurve],
    view: &mut AutoView,
    out: &mut AutoResult,
) {
    // строки — МОМЕНТЫ, колонки — параметры: «нужно несколько параметров
    // на строку иметь а не по одной»
    let mut times: Vec<i64> = curves.iter().flat_map(|c| c.keys.iter().map(|k| k.t.0)).collect();
    times.sort_unstable();
    times.dedup();
    let mut drop_key: Option<(usize, usize)> = None;
    let mut commit = false;
    egui::Grid::new(id).num_columns(curves.len() + 2).spacing([10.0, 4.0]).striped(true).show(
        ui,
        |ui| {
            ui.strong(tr(k::fx::COL_TIME));
            ui.strong(tr(k::fx::COL_FRAC));
            for c in curves.iter() {
                ui.strong(crate::widgets::param_label(&c.param));
            }
            ui.end_row();
            for t in &times {
                let tick = Tick(*t);
                // время строки: правится ОДНИМ полем; сортировка — только
                // на отпускании, иначе строка уезжает из-под рук
                let edit = view
                    .t_edit
                    .filter(|(ci, ki, _)| {
                        curves.get(*ci).and_then(|c| c.keys.get(*ki)).is_some_and(|k| k.t == tick)
                    })
                    .map(|(_, _, s)| s);
                let mut s = edit.unwrap_or_else(|| secs(tick));
                let resp = ui.add(
                    egui::DragValue::new(&mut s).speed(0.01).suffix(" s").range(0.0..=f64::MAX),
                );
                if resp.changed()
                    && let Some((ci, ki)) = curves.iter().enumerate().find_map(|(ci, c)| {
                        c.keys.iter().position(|k| k.t == tick).map(|ki| (ci, ki))
                    })
                {
                    view.t_edit = Some((ci, ki, s));
                }
                if resp.drag_stopped() || resp.lost_focus() {
                    commit = true;
                }
                ui.weak(format!("{:.0}%", ctx.frac(tick) * 100.0));
                for (ci, c) in curves.iter_mut().enumerate() {
                    let Some(spec) = ctx.spec(&c.param).copied() else {
                        ui.label("");
                        continue;
                    };
                    match c.keys.iter().position(|k| k.t == tick) {
                        Some(ki) => {
                            ui.horizontal(|ui| {
                                let mut v = c.keys[ki].v;
                                if crate::widgets::param_value(ui, &spec, &mut v).changed() {
                                    c.keys[ki].v = v;
                                    out.changed = true;
                                }
                                if ui.small_button(crate::icons::CLEAR).clicked() {
                                    drop_key = Some((ci, ki));
                                }
                            });
                        }
                        None => {
                            // пустая клетка — кнопка «поставить сюда ключ»
                            if ui.small_button(tr(k::fx::CELL_ADD)).clicked() {
                                let v = kt_model::curve_value(&c.keys, c.ease, tick);
                                c.keys.push(kt_model::AutoKey { t: tick, v });
                                c.keys.sort_by_key(|k| k.t.0);
                                out.changed = true;
                            }
                        }
                    }
                }
                ui.end_row();
            }
        },
    );
    if let Some((ci, ki)) = drop_key {
        curves[ci].keys.remove(ki);
        view.sel = None;
        out.changed = true;
    }
    if commit && let Some((ci, ki, s)) = view.t_edit.take() {
        if let Some(k) = curves.get_mut(ci).and_then(|c| c.keys.get_mut(ki)) {
            k.t = Tick((s * 90_000.0).round() as i64);
        }
        if let Some(c) = curves.get_mut(ci) {
            let moved = c.keys[ki.min(c.keys.len().saturating_sub(1))].t;
            c.keys.sort_by_key(|k| k.t.0);
            view.sel = c.keys.iter().position(|k| k.t == moved).map(|ki| (ci, ki));
        }
        out.changed = true;
    }
}
