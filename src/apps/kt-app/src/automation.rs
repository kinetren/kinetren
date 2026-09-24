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

/// Радиус хвата ключа. Шире рисунка (правило 1 чек-листа): в ромбик
/// радиусом 4.5 px целиться не должно быть нужно.
const KEY_GRAB_PX: f32 = 9.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutoMode {
    #[default]
    Graph,
    Table,
}

/// Состояние виджета (живёт в окне, не в модели).
///
/// Замка среди полей больше нет: он один на всю программу и живёт в
/// `UiState` (Р.40 п.15) — окно его только читает через `AutoCtx`.
/// Во что целится правый клик по графику (Р.63 п.5).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MenuTarget {
    /// Ключ (кривая, индекс).
    Key(usize, usize),
    /// Участок между ключами `ki` и `ki+1` кривой `ci`: форма живёт на
    /// ЛЕВОМ ключе отрезка.
    Segment(usize, usize),
}

#[derive(Debug, Clone, Default)]
pub struct AutoView {
    pub mode: AutoMode,
    /// Выбранный ключ: (кривая, индекс).
    pub sel: Option<(usize, usize)>,
    /// Ключ под мышью во время драга.
    pub drag: Option<(usize, usize)>,
    /// Кривая, в которую кладёт ключ клик по пустому месту графика.
    pub active: usize,
    /// Собственное окно оси (домен кривой), когда замок снят.
    pub win: Option<(i64, i64)>,
    /// Незакоммиченный ввод времени в таблице: (кривая, ключ, секунды).
    /// Пока строка редактируется, ключи НЕ пересортировываются — иначе
    /// строка уезжает из-под рук на каждой цифре.
    pub t_edit: Option<(usize, usize, f64)>,
    /// Экранные позиции ключей ПОСЛЕДНЕЙ отрисовки: (кривая, ключ, точка).
    ///
    /// Рисунок и щуп — из одного места правды (Р.49, п.364). До этого
    /// раунда хит-тест жил внутри цикла отрисовки и срабатывал только на
    /// `interact_pointer_pos` — то есть при НАЖАТОЙ кнопке. Поэтому ключ
    /// молчал и на наведение, и на правую кнопку: подсказки не было, меню
    /// не было, а «ПКМ удаляет» не завели намеренно — удаление и так есть
    /// двумя путями, а destructive правая кнопка обесценила бы обещание
    /// «правой кнопкой можно пробовать безопасно» во всех остальных местах.
    ///
    /// Буфер ПЕРЕИСПОЛЬЗУЕТСЯ (`clear`, не `Vec::new`): инвариант 12 —
    /// в draw-путях на кадр не аллоцируем.
    pub key_marks: Vec<(usize, usize, egui::Pos2)>,
    /// Замороженное окно значений тянущейся кривой: (кривая, lo, hi).
    /// Без заморозки шкала пересчитывалась бы по самим же ключам и
    /// «дышала» под курсором.
    pub drag_win: Option<(usize, f64, f64)>,
    /// ЦЕЛЬ ПРАВОГО КЛИКА, пока меню открыто (Р.63 п.5): либо ключ, либо
    /// участок между двумя ключами.
    ///
    /// До этого и ключ, и участок считались от `hover_pos()` КАЖДЫЙ КАДР,
    /// а содержимое `context_menu` перерисовывается каждый кадр: указатель
    /// ушёл к пунктам — попадание пропало — меню закрылось само.
    /// «Контекстное меню просто пропадает, я не успеваю дотянуться до
    /// него» (владелец, Р.63 п.5).
    pub menu_target: Option<MenuTarget>,
    /// НАБИРАЕМЫЙ текст формулы: (кривая, текст). Р.63 п.3.
    ///
    /// Поле держит свой текст между кадрами и отдаёт его модели ТОЛЬКО по
    /// подтверждению (Enter или уход фокуса). До этого текст читался из
    /// модели каждый кадр и применялся по первой же законной приставке:
    /// набор `sin(t)` умирал на `sin(` («стирает формулу»), а первая буква
    /// `t` законна сама по себе — она немедленно применялась и обнуляла
    /// ключи кривой («либо нихера не происходит»). Владелец, Р.63 п.3.
    pub formula: Option<(usize, String)>,
}

impl AutoView {
    /// Ключ под точкой. Один щуп на все каналы: нажатие, наведение и
    /// правая кнопка спрашивают ОДНО И ТО ЖЕ.
    pub fn key_at(&self, pos: egui::Pos2) -> Option<(usize, usize)> {
        self.key_marks
            .iter()
            .find(|(_, _, p)| (*p - pos).length() < KEY_GRAB_PX)
            .map(|(ci, ki, _)| (*ci, *ki))
    }

    /// Видимое окно оси времени в домене кривой.
    ///
    /// **Связанный график показывает окно таймлайна КАК ЕСТЬ** (Р.40).
    /// Раньше оно клампилось диапазоном носителя — и в этом был дефект,
    /// на который пожаловался пользователь: «когда я оттягиваю левый край
    /// события — у меня ключи автоматизации тоже сдвигаются к началу».
    /// Ключ НЕ двигался (`clip_slice` его не трогает, он остаётся на том
    /// же кадре материала и в том же моменте таймлайна) — врала ОСЬ: она
    /// нормировалась на событие, событие укорачивалось, и ключ переезжал,
    /// скажем, с половины графика на треть. Чинится осью, а не данными.
    ///
    /// Отвязанный график по-прежнему живёт своим окном, а без окна
    /// показывает носитель целиком.
    pub fn axis(&self, ctx: &AutoCtx<'_>) -> (i64, i64) {
        let (r0, r1) = (ctx.range.0.0, ctx.range.1.0.max(ctx.range.0.0 + 1));
        let (a, b) = match (ctx.linked, self.win) {
            (true, _) => (ctx.view.0.0, ctx.view.1.0),
            (false, Some(w)) => (w.0.clamp(r0, r1), w.1.clamp(r0, r1)),
            (false, None) => (r0, r1),
        };
        // окно уже кадра бессмысленно и делает деление на ноль
        if b - a < ctx.frame_step.max(1) { (r0, r1) } else { (a, b) }
    }
}

/// Вертикальная ось графика: как значение кладётся на высоту (Р.57, п.456).
///
/// Правило КЛАССА, а не имени параметра: знаковое ОТНОШЕНИЕ — это
/// скорость, и её −32…32 на линейной оси делают обычную кривую 0.66…1.0
/// прямой у самого края (та самая жалоба Р.34 про старый график slowed).
/// Логарифм по модулю растягивает окрестность единицы и оставляет ноль
/// нулём; внутри полосы ±knee ось линейна, иначе логарифм ушёл бы в −∞.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Axis {
    Linear,
    SymLog { knee: f64 },
}

/// Ось параметра по его СПЕКЕ.
pub fn axis_of(spec: &ParamSpec) -> Axis {
    if spec.unit == kt_model::ParamUnit::Ratio && spec.min < 0.0 {
        Axis::SymLog { knee: 0.25 }
    } else {
        Axis::Linear
    }
}

impl Axis {
    /// Значение → координата оси (нечётная, монотонная, непрерывная).
    pub fn map(self, v: f64) -> f64 {
        match self {
            Axis::Linear => v,
            Axis::SymLog { knee } => {
                let a = v.abs();
                let g = if a <= knee { a / knee } else { 1.0 + (a / knee).log2() };
                g.copysign(v)
            }
        }
    }

    /// Обратно: координата оси → значение.
    pub fn unmap(self, g: f64) -> f64 {
        match self {
            Axis::Linear => g,
            Axis::SymLog { knee } => {
                let a = g.abs();
                let v = if a <= 1.0 { a * knee } else { knee * (a - 1.0).exp2() };
                v.copysign(g)
            }
        }
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
    /// Замок оси. ОДИН НА ВСЮ ПРОГРАММУ и живёт в `UiState` (Р.40 п.15):
    /// раньше он был полем `AutoView` и умирал вместе с окном. Виджет его
    /// только ЧИТАЕТ, а просьбу переключить возвращает в `AutoResult` —
    /// иначе одно окно молча правило бы состояние всех остальных.
    pub linked: bool,
    /// Форма отрезка у НОВЫХ кривых (поле ПРОЕКТА, Р.61). Новый КЛЮЧ
    /// форму наследует у отрезка, который делит, — умолчание нужно лишь
    /// когда делить нечего.
    pub default_shape: kt_model::SegShape,
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

    /// Подпись кривой. У встроенных рядов имя snake_case переводится
    /// каталогом; у ДИНАМИЧЕСКОГО владельца (публичный параметр
    /// композиции, завтра плагин) имя придумал АВТОР, в каталог оно
    /// попасть не может — и лежит готовой подписью в `ParamSpec::label`
    /// (Р.59). Виджет обязан рисовать её дословно, мимо `tr()` (t684).
    pub(crate) fn label(&self, param: &str) -> String {
        match self.spec(param) {
            Some(s) if !s.label.is_empty() => s.label.to_string(),
            _ => crate::widgets::param_label(param),
        }
    }
}

/// Окно главного таймлайна в домене кривой. Переводится ТЕМ ЖЕ
/// отображением, что и всё остальное; у реверснутого носителя концы
/// меняются местами — поэтому min/max, а не «первый-второй».
pub fn view_in_curve(to_curve: &dyn Fn(Tick) -> Tick, tl_view: (Tick, Tick)) -> (Tick, Tick) {
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
    linked: bool,
    default_shape: kt_model::SegShape,
) -> AutoCtx<'a> {
    AutoCtx {
        default_shape,
        specs: fx.param_specs(),
        range: (clip.source_in, Tick(clip.source_in.0 + clip.duration.0.max(1))),
        view: view_in_curve(to_mat, tl_view),
        to_timeline: to_tl,
        to_curve: to_mat,
        frame_step,
        linked,
    }
}

#[derive(Default)]
pub struct AutoResult {
    pub changed: bool,
    /// Куда двинуть ГЛАВНЫЙ курсор (жест по линейке мини-таймлайна).
    pub seek: Option<Tick>,
    /// Просьба переключить замок оси (Р.40 п.15). Пишет её ТОЛЬКО кнопка
    /// замка: «надо сделать так чтобы его надо было СНЯТЬ чтобы
    /// отключилась синхронизация таймлайнов» — дословно.
    pub set_linked: Option<bool>,
    /// Просьба изменить окно ГЛАВНОГО таймлайна: при ВКЛЮЧЁННОМ замке
    /// колесо над графиком зумит и панорамирует именно его — оси остаются
    /// синхронными, а жест не пропадает.
    pub set_view: Option<(Tick, Tick)>,
    /// По какому ключу открыто контекстное меню (Р.49, п.364). Нужен
    /// владельцу окна, чтобы записать панельную цель: сам виджет про
    /// `KtApp` не знает.
    pub key_menu: Option<(usize, usize)>,
}

/// Просьбы виджета про ОСЬ, собранные с нескольких графиков одного окна
/// (Р.40 п.15). Замок один на программу, поэтому окно с несколькими
/// слотами обязано слить их и применить один раз.
#[derive(Default, Clone, Copy)]
pub struct AxisReq {
    pub set_linked: Option<bool>,
    pub set_view: Option<(Tick, Tick)>,
}

impl AxisReq {
    pub fn merge(self, r: &AutoResult) -> Self {
        AxisReq {
            set_linked: r.set_linked.or(self.set_linked),
            set_view: r.set_view.or(self.set_view),
        }
    }
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
/// Окно считается В КООРДИНАТАХ ОСИ (Р.57): на знаковой скорости
/// линейное окно прижимало бы обычную кривую 0.66…1.0 к самому краю
/// −32…32 и рисовало прямую (жалоба Р.34).
fn value_window(c: &ParamCurve, spec: &ParamSpec) -> (f64, f64) {
    let ax = axis_of(spec);
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for k in &c.keys {
        let g = ax.map(k.v);
        lo = lo.min(g);
        hi = hi.max(g);
    }
    let (gmin, gmax) = (ax.map(spec.min), ax.map(spec.max));
    if !lo.is_finite() || !hi.is_finite() {
        return (gmin, gmax);
    }
    let span = (hi - lo).max((gmax - gmin) * 0.02);
    let pad = span * 0.15;
    ((lo - pad).max(gmin), (hi + pad).min(gmax))
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
            let name = ctx.label(&c.param);
            // производная кривая ОТЛИЧИМА: рядом с именем видно, чем она
            // ведома — адресом источника или формулой (Р.61)
            let name = match &c.source {
                kt_model::CurveSource::Keys => name,
                kt_model::CurveSource::Link { path, .. } => format!("{name} <- {path}"),
                kt_model::CurveSource::Formula { .. } => format!("{name} = ƒ"),
            };
            let text = egui::RichText::new(name).color(curve_color(ci));
            let text = if c.source.is_keys() { text } else { text.italics() };
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
        // ПОЛЕ ФОРМУЛЫ (Р.61): владелец просил «возможность задания кривой
        // автоматизации через ввод формулы (без прокликивания мышки)».
        // Проверка на лету: ошибка показывается С ПОЗИЦИЕЙ и К МОДЕЛИ НЕ
        // ПРИМЕНЯЕТСЯ — иначе полустёртый текст ронял бы кривую на каждом
        // нажатии клавиши.
        if let Some(c) = curves.get_mut(view.active) {
            // ТЕКСТ ЖИВЁТ В ВИДЖЕТЕ, а не перечитывается из модели каждый
            // кадр (Р.63 п.3): иначе незаконченный ввод стирается на
            // следующем же кадре.
            let from_model = || match &c.source {
                kt_model::CurveSource::Formula { expr } => expr.clone(),
                kt_model::CurveSource::Link { expr, .. } => expr.clone().unwrap_or_default(),
                kt_model::CurveSource::Keys => String::new(),
            };
            let mut text = match &view.formula {
                Some((ci, t)) if *ci == view.active => t.clone(),
                _ => from_model(),
            };
            ui.label(tr(k::fx::FORMULA));
            let r = ui
                .add(egui::TextEdit::singleline(&mut text).desired_width(170.0).hint_text("t * 2"));
            crate::widgets::field_probe::note("auto.formula", r.rect);
            crate::app::hint_into(hover, r.clone(), tr(k::fx::FORMULA_HINT));
            if r.changed() {
                view.formula = Some((view.active, text.clone()));
                // проверка НА ЛЕТУ — но только чтобы СКАЗАТЬ об ошибке
                if let Err(e) = kt_model::Formula::parse(&text)
                    && !text.trim().is_empty()
                {
                    *hover = Some(e.to_string());
                }
            }
            // ПОДТВЕРЖДЕНИЕ: Enter или уход фокуса. Только теперь текст
            // едет в модель — до этого ключи кривой целы.
            let entered = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if entered || (r.lost_focus() && !entered) {
                let text = view.formula.take().filter(|(ci, _)| *ci == view.active).map(|(_, t)| t);
                if let Some(text) = text {
                    if text.trim().is_empty() {
                        // пустая строка — «отвязать»: значения кривой уже
                        // разрешены, снимаем сам источник
                        if !c.source.is_keys() {
                            c.source = kt_model::CurveSource::Keys;
                            out.changed = true;
                        }
                    } else {
                        match kt_model::Formula::parse(&text) {
                            Ok(_) => {
                                c.source = match &c.source {
                                    kt_model::CurveSource::Link { path, stops, .. } => {
                                        kt_model::CurveSource::Link {
                                            path: path.clone(),
                                            expr: Some(text.clone()),
                                            stops: stops.clone(),
                                        }
                                    }
                                    _ => {
                                        c.keys.clear();
                                        kt_model::CurveSource::Formula { expr: text.clone() }
                                    }
                                };
                                out.changed = true;
                            }
                            // незаконченный текст к модели не применяется, а
                            // молчать о нём нельзя (правило 16)
                            Err(e) => *hover = Some(e.to_string()),
                        }
                    }
                }
            }
        }
        // «Отвязать» — производная кривая становится обычными ключами по
        // ТЕКУЩИМ значениям. Без этого действия контрол оставался бы
        // навсегда погашенным, а единственным выходом было бы снять
        // кривую целиком (правило 9).
        if let Some(c) = curves.get_mut(view.active)
            && !c.source.is_keys()
        {
            let b = ui.button(tr(k::fx::UNLINK));
            crate::app::hint_into(hover, b.clone(), tr(k::fx::UNLINK_HINT));
            crate::widgets::field_probe::note("auto.unlink", b.rect);
            if b.clicked() {
                // значения уже разрешены моделью при сборке — здесь
                // остаётся снять сам источник
                c.source = kt_model::CurveSource::Keys;
                out.changed = true;
            }
        }
        ui.separator();
        // ---- ключи ----
        //
        // ТРИ кнопки (Р.40 п.18, дословно: «не хватает по мимо кнопки
        // установки ключа - двух кнопок ключ ДО и ключ ПОСЛЕ текущей
        // позиции (чтобы зафиксировать предыдущие и следующие
        // параметры)»). «До» и «после» ставят ключ на СОСЕДНИЙ КАДР со
        // значением кривой в этой точке: форма кривой не меняется, но
        // участок слева (или справа) от курсора становится закреплён — и
        // дальнейшая правка ключа под курсором его уже не тянет.
        //
        // Ctrl — во ВСЕ кривые слота сразу. Это бывшая кнопка
        // «+ keyframe at playhead» из окна FX: она отличалась от «+»
        // ровно этим и ни словом об этом не говорила (правило 18:
        // название пункта говорит, ЧТО произойдёт).
        let ph = (ctx.to_curve)(playhead);
        let step = ctx.frame_step.max(1);
        let all = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        for (icon, at, hint) in [
            (crate::icons::KEY_BEFORE, Tick(ph.0 - step), tr(k::fx::KEY_BEFORE_HINT)),
            (crate::icons::PLUS, ph, tr(k::fx::KEY_ADD_HINT)),
            (crate::icons::KEY_AFTER, Tick(ph.0 + step), tr(k::fx::KEY_AFTER_HINT)),
        ] {
            if !crate::app::hint_into(hover, ui.small_button(icon), hint).clicked() {
                continue;
            }
            let active = view.active;
            let targets: Vec<usize> = if all { (0..curves.len()).collect() } else { vec![active] };
            for ci in targets {
                let Some(c) = curves.get_mut(ci) else { continue };
                let v = kt_model::curve_value(&c.keys, at);
                kt_model::insert_key(&mut c.keys, at, v, ctx.default_shape);
            }
            if let Some(c) = curves.get(active) {
                view.sel = c.keys.iter().position(|k| k.t == at).map(|ki| (active, ki));
            }
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
        let has_sel =
            view.sel.is_some_and(|(c, k)| curves.get(c).is_some_and(|c| k < c.keys.len()));
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
        // ЕДИНСТВЕННОЕ место, снимающее замок (Р.40 п.15, дословно: «надо
        // сделать так чтобы его надо было СНЯТЬ чтобы отключилась
        // синхронизация таймлайнов»). Прежде его снимало ЛЮБОЕ колесо над
        // графиком — включая прокрутку самого окна FX мимо графика.
        if crate::app::hint_into(
            hover,
            ui.selectable_label(ctx.linked, crate::icons::LOCK),
            tr(k::fx::AXIS_LINK_HINT),
        )
        .clicked()
        {
            out.set_linked = Some(!ctx.linked);
            if ctx.linked {
                // снимая замок, оставляем ТО ЖЕ окно, что было видно:
                // «ничего не скачет» (правило 3 чек-листа)
                let (a, b) = view.axis(ctx);
                view.win = Some((a, b));
            }
        }
        // «вписать» показывает носитель целиком — это ЯВНАЯ просьба про
        // ось графика, поэтому она замок снимает, и снимает ВИДИМО
        // (кнопка замка тут же гаснет), а не молча по дороге
        if crate::app::hint_into(
            hover,
            ui.small_button(crate::icons::ARROWS_OUT),
            tr(k::fx::AXIS_FIT_HINT),
        )
        .clicked()
        {
            out.set_linked = Some(false);
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
            let (na, nb) = (a.round() as i64, (a + ns).round() as i64);
            if ctx.linked {
                // ЗАМОК НАДЕТ — жест едет в ГЛАВНЫЙ таймлайн, оси остаются
                // синхронными (Р.40 п.15). Раньше он молча снимал замок, и
                // снимала его даже прокрутка самого окна FX колесом над
                // графиком: «замочек не до конца работает».
                out.set_view = Some(((ctx.to_timeline)(Tick(na)), (ctx.to_timeline)(Tick(nb))));
            } else {
                view.win = Some((na, nb));
            }
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
    view.key_marks.clear();
    for (ci, c) in curves.iter().enumerate() {
        let Some(spec) = ctx.spec(&c.param) else { continue };
        let (lo, hi) = win_of(c, spec, view, ci);
        // окно уже В КООРДИНАТАХ ОСИ (Р.57): значение переводится в них
        let ax = axis_of(spec);
        let y_of = |v: f64| -> f32 {
            let u = ((ax.map(v) - lo) / (hi - lo).max(1e-9)).clamp(0.0, 1.0) as f32;
            plot.bottom() - 6.0 - u * (plot.height() - 12.0)
        };
        // НУЛЕВАЯ ЛИНИЯ (Р.57, п.456): окно пересекает ноль — значит
        // «вперёд» и «назад» обязаны различаться не только подписью. Это
        // класс, а не случай скорости: панорама и dB тоже знаковые.
        if lo < 0.0 && hi > 0.0 {
            let y0 = y_of(0.0);
            p.add(egui::Shape::line_segment(
                [egui::pos2(plot.left(), y0), egui::pos2(plot.right(), y0)],
                egui::Stroke::new(1.0, crate::theme::AXIS_ZERO),
            ));
            p.text(
                egui::pos2(plot.left() + 2.0, y0),
                egui::Align2::LEFT_CENTER,
                "0",
                egui::FontId::proportional(9.0),
                crate::theme::AXIS_ZERO,
            );
        }
        // УЧАСТОК ПОД МЕНЮ подсвечен (Р.63 п.5): иначе непонятно, к чему
        // относится форма, которую сейчас выбирают
        if let Some(MenuTarget::Segment(mc, mk)) = view.menu_target
            && mc == ci
            && let (Some(a), Some(b)) = (c.keys.get(mk), c.keys.get(mk + 1))
        {
            p.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x_of(a.t.0), plot.top()),
                    egui::pos2(x_of(b.t.0), plot.bottom()),
                ),
                0.0,
                egui::Color32::from_rgba_unmultiplied(120, 160, 255, 28),
            );
        }
        let col = curve_color(ci);
        // ФОРМУЛЬНАЯ кривая ключей не хранит (контракт модели), поэтому
        // рисуется РАЗРЕШЁННЫМИ значениями — тем же вычислителем, что у
        // рендера (Р.63 п.3). Без этого верная формула давала пустой
        // график: «прописываю, а нихера не происходит».
        let shown: std::borrow::Cow<'_, [kt_model::AutoKey]> = if c.keys.is_empty()
            && !c.source.is_keys()
        {
            std::borrow::Cow::Owned(kt_model::formula_preview(
                c,
                Tick(m0),
                Tick(m1),
                (m1 - m0).max(1) / 120,
            ))
        } else {
            std::borrow::Cow::Borrowed(&c.keys[..])
        };
        if shown.len() > 1 {
            let pts: Vec<egui::Pos2> =
                shown.iter().map(|k| egui::pos2(x_of(k.t.0), y_of(k.v))).collect();
            p.add(egui::Shape::line(pts, egui::Stroke::new(1.0, col)));
        }
        // ПРОИЗВОДНАЯ кривая (связь или формула) мышкой не правится: ключей
        // у неё нет, и перетаскивание точки молча создало бы третью правду о
        // значении (Р.61). Её линия рисуется штрихом, а маркеров ключей нет
        // вовсе — значит и щуп по ним не сработает.
        if !c.source.is_keys() {
            continue;
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
            view.key_marks.push((ci, ki, egui::pos2(x, y)));
        }
        // УЧАСТКИ активной кривой — цель живого сценария (Р.63 п.5):
        // правый клик обязан попасть МЕЖДУ ключами, а где они, знает
        // только отрисовка. `format!` под флагом: в draw-путях на кадр не
        // аллоцируем (инвариант 12).
        if ci == view.active && crate::widgets::field_probe::on() {
            for ki in 0..c.keys.len().saturating_sub(1) {
                crate::widgets::field_probe::note(
                    &format!("auto.seg.{ki}"),
                    egui::Rect::from_min_max(
                        egui::pos2(x_of(c.keys[ki].t.0), plot.top()),
                        egui::pos2(x_of(c.keys[ki + 1].t.0), plot.bottom()),
                    ),
                );
            }
        }
    }
    // щуп — по НАРИСОВАННОМУ, и один на все каналы
    let hit = resp.interact_pointer_pos().and_then(|pos| view.key_at(pos));
    // Подсказка у ключа (Р.49, п.364): раньше он молчал в ОБОИХ каналах —
    // ни наведение, ни правая кнопка ничего не сообщали, и что это за
    // точка, узнать было негде, кроме как выбрать её и прочесть строку
    // сверху. Формат тот же, что у выбранного ключа: секунды материала и
    // доля события одной строкой.
    if let Some(hp) = resp.hover_pos()
        && let Some((ci, ki)) = view.key_at(hp)
        && let Some(key) = curves.get(ci).and_then(|c| c.keys.get(ki))
    {
        crate::app::hint_into(
            hover,
            resp.clone(),
            &crate::tf!(
                k::fx::KEY_AT,
                format!("{:.3}", secs(key.t)),
                format!("{:.0}", ctx.frac(key.t) * 100.0)
            ),
        );
    }
    // Меню ключа — ДО ранних `return` ниже (линейка, двойной клик): после
    // них правый клик по ключу не доходил бы до привязки вовсе.
    {
        // ЦЕЛЬ ФИКСИРУЕТСЯ НА НАЖАТИИ (Р.63 п.5), а не пересчитывается от
        // положения указателя каждый кадр: иначе движение к пунктам меню
        // сбрасывало попадание, и меню закрывалось само.
        if resp.secondary_clicked()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            view.menu_target = view
                .key_at(pos)
                .map(|(ci, ki)| MenuTarget::Key(ci, ki))
                .or_else(|| {
                    let ci = view.active;
                    let t = t_of(pos.x);
                    let keys = &curves.get(ci)?.keys;
                    keys.windows(2)
                        .position(|w| t >= w[0].t.0 && t < w[1].t.0)
                        .map(|ki| MenuTarget::Segment(ci, ki))
                });
        }
        let menu_hit = match view.menu_target {
            Some(MenuTarget::Key(ci, ki)) => Some((ci, ki)),
            _ => None,
        };
        let mut del: Option<(usize, usize)> = None;
        let mut picked: Option<(usize, usize)> = None;
        // Правый клик по УЧАСТКУ (не по ключу) — форма этого отрезка
        // (Р.61). Соседние участки не трогаются: форма принадлежит
        // ЛЕВОМУ ключу отрезка, и меняется ровно он.
        let seg_hit: Option<(usize, usize)> = match view.menu_target {
            Some(MenuTarget::Segment(ci, ki)) => Some((ci, ki)),
            _ => None,
        };
        let mut set_shape: Option<(usize, usize, kt_model::SegShape)> = None;
        let mut shape_all: Option<(usize, kt_model::SegShape)> = None;
        let menu = resp.clone().context_menu(|ui| {
            let Some((ci, ki)) = menu_hit else {
                if let Some((ci, ki)) = seg_hit {
                    let rate = curves.get(ci).is_some_and(|c| c.param == "rate");
                    let cur = curves[ci].keys[ki].shape;
                    // ЧЕЛОВЕК ВИДИТ, МЕЖДУ ЧЕМ ставит форму (Р.63 п.5):
                    // заголовок называет оба конца участка, а сам участок
                    // подсвечен на графике
                    let (a, b) = (curves[ci].keys[ki].t, curves[ci].keys[ki + 1].t);
                    ui.label(
                        egui::RichText::new(crate::tf!(
                            k::fx::SHAPE_SEGMENT_AT,
                            format!("{:.3}", secs(a)),
                            format!("{:.3}", secs(b)),
                            format!("{:.0}", ctx.frac(a) * 100.0),
                            format!("{:.0}", ctx.frac(b) * 100.0)
                        ))
                        .strong(),
                    );
                    if rate {
                        // невозможность — ОШИБКА с причиной, а не тишина
                        ui.label(egui::RichText::new(tr(k::fx::SHAPE_RATE_LINEAR)).weak().small());
                    } else {
                        for (i, sh) in crate::widgets::all_shapes().into_iter().enumerate() {
                            let on = std::mem::discriminant(&sh) == std::mem::discriminant(&cur);
                            let r = ui.selectable_label(on, crate::widgets::shape_label(sh));
                            // пункты меню — цель живого сценария: он обязан
                            // ДОТЯНУТЬСЯ до них указателем (Р.63 п.5)
                            if crate::widgets::field_probe::on() {
                                crate::widgets::field_probe::note(
                                    &format!("auto.shape.{i}"),
                                    r.rect,
                                );
                            }
                            if r.clicked() {
                                set_shape = Some((ci, ki, sh));
                                ui.close();
                            }
                        }
                        ui.separator();
                        ui.menu_button(tr(k::fx::SHAPE_ALL), |ui| {
                            for sh in crate::widgets::all_shapes() {
                                if ui.button(crate::widgets::shape_label(sh)).clicked() {
                                    shape_all = Some((ci, sh));
                                    ui.close();
                                }
                            }
                        });
                    }
                    return;
                }
                ui.close();
                return;
            };
            picked = Some((ci, ki));
            if let Some(key) = curves.get(ci).and_then(|c| c.keys.get(ki)) {
                ui.label(
                    egui::RichText::new(crate::tf!(
                        k::fx::KEY_AT,
                        format!("{:.3}", secs(key.t)),
                        format!("{:.0}", ctx.frac(key.t) * 100.0)
                    ))
                    .strong(),
                );
            }
            // «Удалить» БЕЗ аккорда: Del удаляет выделенные события
            // таймлайна, а не ключ (п.362)
            if crate::widgets::menu_item(ui, tr(k::tl::menu::DELETE), None) {
                del = Some((ci, ki));
                ui.close();
            }
        });
        // меню закрылось — цель забыта (иначе следующий правый клик по
        // пустому месту показал бы форму прошлого участка)
        if menu.is_none() {
            view.menu_target = None;
        }
        out.key_menu = picked;
        if let Some((ci, ki, sh)) = set_shape
            && curves.get(ci).is_some_and(|c| ki < c.keys.len())
        {
            curves[ci].keys[ki].shape = sh;
            out.changed = true;
        }
        if let Some((ci, sh)) = shape_all
            && let Some(c) = curves.get_mut(ci)
        {
            for k in &mut c.keys {
                k.shape = sh;
            }
            out.changed = true;
        }
        if let Some((ci, ki)) = del
            && curves.get(ci).is_some_and(|c| ki < c.keys.len())
        {
            curves[ci].keys.remove(ki);
            view.sel = None;
            out.changed = true;
        }
    }
    // подпись активной кривой (в неё кладёт ключ клик по пустому месту)
    if let Some(c) = curves.get(view.active)
        && let Some(spec) = ctx.spec(&c.param)
    {
        let (lo, hi) = win_of(c, spec, view, view.active);
        let ax = axis_of(spec);
        let col = curve_color(view.active);
        p.text(
            egui::pos2(rect.right() - 6.0, plot.top() + 2.0),
            egui::Align2::RIGHT_TOP,
            ctx.label(&c.param),
            egui::FontId::proportional(11.0),
            col,
        );
        // границы шкалы значений: без них непонятно, «насколько» кривая
        // поднимается — а шкала у каждой кривой своя
        for (g, y) in [(hi, plot.top() + 2.0), (lo, plot.bottom() - 12.0)] {
            let v = ax.unmap(g);
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
            if let Some(spec) = curves.get(view.active).and_then(|c| ctx.spec(&c.param)).cloned() {
                let (lo, hi) = win_of(&curves[view.active], &spec, view, view.active);
                let c = &mut curves[view.active];
                let t = Tick(t_of(pos.x).clamp(m0, m1));
                let u = ((plot.bottom() - 6.0 - pos.y) / (plot.height() - 12.0)).clamp(0.0, 1.0);
                let v = axis_of(&spec).unmap(lo + (hi - lo) * u as f64).clamp(spec.min, spec.max);
                kt_model::insert_key(&mut c.keys, t, v, ctx.default_shape);
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
        let Some(spec) = ctx.spec(&curves[ci].param).cloned() else { return };
        // снап по сетке кадров: ключ садится на кадр, а не «около»
        let raw = t_of(pos.x).clamp(m0, m1);
        let step = ctx.frame_step.max(1);
        let t = Tick(raw - raw.rem_euclid(step));
        let (lo, hi) = win_of(&curves[ci], &spec, view, ci);
        let u = ((plot.bottom() - 6.0 - pos.y) / (plot.height() - 12.0)).clamp(0.0, 1.0);
        let v = axis_of(&spec).unmap(lo + (hi - lo) * u as f64).clamp(spec.min, spec.max);
        // форма отрезка принадлежит ключу и при перетаскивании обязана
        // остаться: иначе драг молча превращал бы зигзаг в сглаживание
        let shape = curves[ci].keys[ki].shape;
        curves[ci].keys[ki] = kt_model::AutoKey::shaped(t, v, shape);
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
                ui.strong(ctx.label(&c.param));
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
                    let Some(spec) = ctx.spec(&c.param).cloned() else {
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
                                let v = kt_model::curve_value(&c.keys, tick);
                                kt_model::insert_key(
                                    &mut c.keys,
                                    tick,
                                    v,
                                    kt_model::SegShape::Linear,
                                );
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
