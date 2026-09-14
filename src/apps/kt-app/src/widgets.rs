//! Общие виджеты полей ввода (Р.33 С2).
//!
//! До этого раунда каждое окно рисовало параметры само: одни и те же
//! значения жили под слайдером в одном месте и под drag-полем в другом, а
//! их диапазоны расходились между окном FX, окном морфера и спекой модели
//! (у которой они и есть настоящие). Требование пользователя: «ui полей
//! хромает везде», «ревизия во всех полях ввода».
//!
//! Отсюда правило: числовой параметр эффекта рисуется ТОЛЬКО этими
//! функциями, а диапазон, шаг и единица берутся из `ParamSpec` — одного
//! источника правды в kt-model. Разойтись им теперь негде.

use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::{FxInstance, HasParams, ParamSpec, ParamUnit};

/// Суффикс единицы: значение с единицей само себя объясняет, поэтому
/// подпись остаётся короткой (правило «очевидных надписей не делать»).
fn unit_suffix(u: ParamUnit) -> &'static str {
    match u {
        ParamUnit::Db => " dB",
        ParamUnit::Ms => " ms",
        ParamUnit::Hz => " Hz",
        ParamUnit::Deg => "°",
        ParamUnit::Px => " px",
        ParamUnit::St => " st",
        ParamUnit::None | ParamUnit::Norm | ParamUnit::Ratio | ParamUnit::Rgb8 | ParamUnit::Count => {
            ""
        }
    }
}

/// Локализованная подпись параметра. Имя поля — ключ, перевод живёт в
/// каталогах: kt-model про язык интерфейса ничего не знает.
pub fn param_label(name: &str) -> String {
    let s: &str = match name {
        "db" => tr(k::fxp::DB),
        "rate" => tr(k::fxp::RATE),
        "mix" => tr(k::fxp::MIX),
        "reverb_delay_ms" => tr(k::fxp::DELAY),
        "damp" => tr(k::fxp::DAMP),
        "decay" => tr(k::fxp::DECAY),
        "wet_gain" => tr(k::fxp::WET_GAIN),
        // аудио-эффекты Р.47 (Ка2)
        "cutoff_hz" => tr(k::fxp::CUTOFF_HZ),
        "resonance" => tr(k::fxp::RESONANCE),
        "low_gain_db" => tr(k::fxp::LOW_GAIN_DB),
        "low_freq_hz" => tr(k::fxp::LOW_FREQ_HZ),
        "mid_gain_db" => tr(k::fxp::MID_GAIN_DB),
        "mid_freq_hz" => tr(k::fxp::MID_FREQ_HZ),
        "mid_q" => tr(k::fxp::MID_Q),
        "high_gain_db" => tr(k::fxp::HIGH_GAIN_DB),
        "high_freq_hz" => tr(k::fxp::HIGH_FREQ_HZ),
        "drive_db" => tr(k::fxp::DRIVE_DB),
        "tone" => tr(k::fxp::TONE),
        "bits" => tr(k::fxp::BITS),
        "rate_hz" => tr(k::fxp::RATE_HZ),
        "depth" => tr(k::fxp::DEPTH),
        "depth_ms" => tr(k::fxp::DEPTH_MS),
        "feedback" => tr(k::fxp::FEEDBACK),
        "voices" => tr(k::fxp::VOICES),
        "stages" => tr(k::fxp::STAGES),
        "time_ms" => tr(k::fxp::TIME_MS),
        "threshold_db" => tr(k::fxp::THRESHOLD_DB),
        "ratio" => tr(k::fxp::RATIO),
        "attack_ms" => tr(k::fxp::ATTACK_MS),
        "release_ms" => tr(k::fxp::RELEASE_MS),
        "hold_ms" => tr(k::fxp::HOLD_MS),
        "makeup_db" => tr(k::fxp::MAKEUP_DB),
        // лимитер Р.48
        "ceiling_db" => tr(k::fxp::CEILING_DB),
        "lookahead_ms" => tr(k::fxp::LOOKAHEAD_MS),
        // питч Р.48
        "semitones" => tr(k::fxp::SEMITONES),
        "cents" => tr(k::fxp::CENTS),
        "pan_x" => tr(k::fxp::PAN_X),
        "pan_y" => tr(k::fxp::PAN_Y),
        "zoom" => tr(k::fxp::ZOOM),
        "time_scale" => tr(k::fxp::TIME_SCALE),
        "radius" => tr(k::fx::RADIUS),
        // новые параметры Р.33 С5
        "radius_y" => tr(k::fxp::RADIUS_Y),
        "thickness" => tr(k::fxp::THICKNESS),
        "size" => tr(k::fxp::SIZE),
        "scan_step" => tr(k::fxp::SCAN_STEP),
        "jitter" => tr(k::fxp::JITTER),
        "spill" => tr(k::fxp::SPILL),
        "pan" => tr(k::fxp::PAN),
        "predelay_ms" => tr(k::fxp::PREDELAY_MS),
        "width" => tr(k::fxp::WIDTH),
        "key_r" => tr(k::fxp::KEY_R),
        "key_g" => tr(k::fxp::KEY_G),
        "key_b" => tr(k::fxp::KEY_B),
        "tolerance" => tr(k::fx::TOLERANCE),
        "softness" => tr(k::fx::SOFTNESS),
        "threshold" => tr(k::fx::THRESHOLD),
        "amount" => tr(k::fxp::AMOUNT),
        // параметры видов переходов (Р.33 С4)
        "angle" => tr(k::fxp::ANGLE),
        "turns" => tr(k::fxp::TURNS),
        "blur" => tr(k::fxp::BLUR),
        "rgb_shift" => tr(k::fx::RGB_SHIFT),
        "bleed" => tr(k::fxp::BLEED),
        "strength" => tr(k::fx::STRENGTH),
        "cols" => tr(k::fx::COLUMNS),
        "rows" => tr(k::fx::ROWS),
        "pos_x" => tr(k::fxp::POS_X),
        "pos_y" => tr(k::fxp::POS_Y),
        "pos_z" => tr(k::fxp::POS_Z),
        "rot_x" => tr(k::fxp::ROT_X),
        "rot_y" => tr(k::fxp::ROT_Y),
        "rot_z" => tr(k::fxp::ROT_Z),
        "anchor_x" => tr(k::fxp::ANCHOR_X),
        "anchor_y" => tr(k::fxp::ANCHOR_Y),
        "scale" => tr(k::fxp::SCALE),
        "fov_deg" => tr(k::fxp::FOV_DEG),
        // видео-эффекты Р.47 (Кв2)
        "brightness" => tr(k::fxp::BRIGHTNESS),
        "contrast" => tr(k::fxp::CONTRAST),
        "saturation" => tr(k::fxp::SATURATION),
        "hue" => tr(k::fxp::HUE),
        "levels" => tr(k::fxp::LEVELS),
        "x0" => tr(k::fxp::X0),
        "y0" => tr(k::fxp::Y0),
        "x1" => tr(k::fxp::X1),
        "y1" => tr(k::fxp::Y1),
        "x2" => tr(k::fxp::X2),
        "y2" => tr(k::fxp::Y2),
        "x3" => tr(k::fxp::X3),
        "y3" => tr(k::fxp::Y3),
        // барьеры Р.47 (Кв3)
        "offset_x" => tr(k::fxp::OFFSET_X),
        "offset_y" => tr(k::fxp::OFFSET_Y),
        "opacity" => tr(k::fxp::OPACITY),
        "center_x" => tr(k::fxp::CENTER_X),
        "center_y" => tr(k::fxp::CENTER_Y),
        // гео Р.47 (Кв4)
        "k1" => tr(k::fxp::K1),
        "k2" => tr(k::fxp::K2),
        "freq" => tr(k::fxp::FREQ),
        "phase" => tr(k::fxp::PHASE),
        "segments" => tr(k::fxp::SEGMENTS),
        "curvature" => tr(k::fxp::CURVATURE),
        // параметр без подписи в каталоге — видно сразу, а не молча
        other => return other.to_string(),
    };
    s.to_string()
}

/// Единый числовой контрол параметра. Диапазон, шаг и единица — из спеки.
pub fn param_value(ui: &mut egui::Ui, spec: &ParamSpec, v: &mut f64) -> egui::Response {
    let mut dv = egui::DragValue::new(v).range(spec.min..=spec.max).speed(spec.step);
    let suffix = unit_suffix(spec.unit);
    if !suffix.is_empty() {
        dv = dv.suffix(suffix);
    }
    // целочисленные параметры не показывают дробную часть
    if spec.step >= 1.0 {
        dv = dv.max_decimals(0);
    }
    ui.add(dv)
}

/// Связь полей параметров с кривыми (Р.38 пп.15-16).
///
/// Претензии дословно: «при нажатии на ключ — чтобы значения которые
/// прописаны в полях — имели связь с ключем» и «ощущение что у эффектов
/// есть какое-то отдельное "дефолтное" состояние значений».
///
/// Второе — следствие первого: при живой кривой поле правило БАЗУ, а
/// движок брал значение кривой. База оставалась невидимой, всплывала при
/// снятии кривой и выглядела как «отдельное состояние».
pub struct CurveBind<'a> {
    pub curves: &'a mut Vec<kt_model::ParamCurve>,
    /// Выбранный ключ виджета автоматизации: (кривая, индекс).
    pub sel: Option<(usize, usize)>,
    /// Позиция курсора В ДОМЕНЕ КРИВОЙ — сюда садится ключ, когда ни один
    /// не выбран.
    pub at: kt_time::Tick,
}

impl CurveBind<'_> {
    fn index_of(&self, param: &str) -> Option<usize> {
        self.curves.iter().position(|c| c.param == param)
    }
}

/// Что именно тронуло поле параметра (Р.43, п.291).
///
/// Прежде обе правки возвращались ОДНИМ `bool`, и вызывающий не мог знать,
/// куда писать результат: в `SetFx` или в `SetFxAuto`. `fx_editor` писал
/// только в `SetFx`, поэтому правка ключа ЧИСЛОМ умирала вместе с кадром и
/// поле «скидывалось на предыдущее значение» — претензия пользователя
/// Р.43 п.1. У морфера флаг был один на оба источника правки, и потому там
/// это работало.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ParamsChanged {
    /// Тронута БАЗА эффекта (параметр без живой кривой) → `SetFx`.
    pub base: bool,
    /// Тронута КРИВАЯ (ключ параметра с живой кривой) → `SetFxAuto`.
    pub curve: bool,
}

impl ParamsChanged {
    pub fn any(self) -> bool {
        self.base || self.curve
    }
    fn base() -> ParamsChanged {
        ParamsChanged { base: true, curve: false }
    }
    fn curve() -> ParamsChanged {
        ParamsChanged { base: false, curve: true }
    }
}

impl std::ops::BitOrAssign for ParamsChanged {
    fn bitor_assign(&mut self, o: ParamsChanged) {
        self.base |= o.base;
        self.curve |= o.curve;
    }
}

/// Прямоугольники полей параметров последнего кадра — ТОЛЬКО для живых
/// сценариев (`KT_GEOM_DUMP`, правило Р.27 «координаты берутся из САМОЙ
/// программы»). У `egui::DragValue` нет узла в дереве доступности, и по
/// подписи его не найти ни харнессу, ни скрипту: без этого списка сценарий
/// на поле параметра пришлось бы писать угаданными смещениями внутри окна.
/// В обычном запуске ветка стоит один `OnceLock`-чек.
pub(crate) mod field_probe {
    use eframe::egui;
    use std::cell::RefCell;
    use std::sync::OnceLock;

    thread_local! {
        static RECTS: RefCell<Vec<(String, egui::Rect)>> = const { RefCell::new(Vec::new()) };
    }

    pub(crate) fn on() -> bool {
        static ON: OnceLock<bool> = OnceLock::new();
        *ON.get_or_init(|| std::env::var("KT_GEOM_DUMP").is_ok())
    }

    pub(crate) fn note(name: &str, r: egui::Rect) {
        if !on() {
            return;
        }
        RECTS.with(|v| {
            let mut v = v.borrow_mut();
            v.retain(|(n, _)| n != name);
            v.push((name.to_string(), r));
        });
    }

    /// Забрать накопленное (дамп геометрии пишется раз в 100 мс).
    pub(crate) fn take() -> Vec<(String, egui::Rect)> {
        RECTS.with(|v| std::mem::take(&mut *v.borrow_mut()))
    }
}

/// Та же строка, но знающая про кривые.
///
/// Правило одно: **поле правит ровно то, что показывает.**
/// - кривой нет → база эффекта, как раньше;
/// - кривая есть и выбран ЕЁ ключ → этот ключ;
/// - кривая есть, ключ не выбран → значение кривой под курсором, а правка
///   ставит там ключ (то же делает «+» в панели автоматизации).
pub fn param_row_bound<P: HasParams + ?Sized>(
    ui: &mut egui::Ui,
    f: &mut P,
    spec: &ParamSpec,
    bind: Option<&mut CurveBind<'_>>,
) -> ParamsChanged {
    let none = ParamsChanged::default();
    let Some(base) = f.get_param(spec.name) else { return none };
    let Some(bind) = bind else {
        ui.label(param_label(spec.name));
        let mut v = base;
        let r = param_value(ui, spec, &mut v);
        field_probe::note(spec.name, r.rect);
        if r.changed() {
            f.set_param(spec.name, v);
            return ParamsChanged::base();
        }
        return none;
    };
    let Some(ci) = bind.index_of(spec.name) else {
        ui.label(param_label(spec.name));
        let mut v = base;
        let r = param_value(ui, spec, &mut v);
        field_probe::note(spec.name, r.rect);
        if r.changed() {
            f.set_param(spec.name, v);
            return ParamsChanged::base();
        }
        return none;
    };
    // подпись ЦВЕТОМ кривой — видно, что поле правит именно её (и какую
    // линию на графике искать глазами)
    let key = match bind.sel {
        Some((c, k)) if c == ci && k < bind.curves[ci].keys.len() => Some(k),
        _ => None,
    };
    ui.label(
        egui::RichText::new(param_label(spec.name)).color(crate::automation::curve_color(ci)),
    );
    let c = &mut bind.curves[ci];
    let mut v = bound_value(c, key, bind.at);
    let r = param_value(ui, spec, &mut v);
    field_probe::note(spec.name, r.rect);
    if r.changed() {
        bound_apply(c, key, bind.at, v);
        // правка ушла в КРИВУЮ — значит и команда обязана быть SetFxAuto
        return ParamsChanged::curve();
    }
    none
}

/// Что ПОКАЗЫВАЕТ поле параметра с живой кривой: значение выбранного
/// ключа, а нет выбранного — значение кривой под курсором. Базы эффекта
/// тут нет вовсе: именно её невидимость и была «отдельным дефолтным
/// состоянием» (Р.38 п.16).
pub fn bound_value(c: &kt_model::ParamCurve, key: Option<usize>, at: kt_time::Tick) -> f64 {
    match key.filter(|k| *k < c.keys.len()) {
        Some(k) => c.keys[k].v,
        None => kt_model::curve_value(&c.keys, c.ease, at),
    }
}

/// Что поле ПРАВИТ. Ровно то же, что показывает (Р.38 п.15): выбранный
/// ключ либо новый ключ под курсором.
pub fn bound_apply(
    c: &mut kt_model::ParamCurve,
    key: Option<usize>,
    at: kt_time::Tick,
    v: f64,
) {
    match key.filter(|k| *k < c.keys.len()) {
        Some(k) => c.keys[k].v = v,
        None => {
            c.keys.retain(|k| k.t != at);
            c.keys.push(kt_model::AutoKey { t: at, v });
            c.keys.sort_by_key(|k| k.t.0);
        }
    }
}

/// Сетка ВСЕХ числовых параметров эффекта в две пары «подпись/значение».
/// `skip` — параметры, у которых есть свой контрол (цвет ключа рисуется
/// пипеткой, а не тремя числами).
pub fn params_grid<P: HasParams + ?Sized>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    f: &mut P,
    skip: &[&str],
) -> bool {
    params_grid_bound(ui, id, f, skip, None).base
}

/// Та же сетка, но связанная с кривыми (Р.38 пп.15-16).
pub fn params_grid_bound<P: HasParams + ?Sized>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    f: &mut P,
    skip: &[&str],
    mut bind: Option<&mut CurveBind<'_>>,
) -> ParamsChanged {
    let specs: Vec<&ParamSpec> =
        f.param_specs().iter().filter(|s| !skip.contains(&s.name)).collect();
    if specs.is_empty() {
        return ParamsChanged::default();
    }
    let mut changed = ParamsChanged::default();
    egui::Grid::new(id).num_columns(4).spacing([8.0, 4.0]).show(ui, |ui| {
        for pair in specs.chunks(2) {
            for spec in pair {
                changed |= param_row_bound(ui, f, spec, bind.as_deref_mut());
            }
            ui.end_row();
        }
    });
    changed
}

// ---- единая полоса FX-цепочки (Р.33 С2) ---------------------------------
// «в двух местах один и тот же смысл, а сделано у обоих не до конца»:
// у событий были галки и стрелки-перестановка, у морферов — крестики, и
// ни там, ни там не было ни мыши, ни мастер-галки, ни копипаста. Теперь
// полоса одна и умеет всё; вызывают её окно событий и обе секции морфера.

/// Что цепочка отдаёт на редактирование. `bypass` и `auto` параллельны
/// `fx`.
///
/// Кривые есть У ОБОИХ носителей (Р.38 п.13 дал их морферу), и `auto`
/// здесь НЕ `Option` намеренно: пока он был опциональным, вставка
/// цепочки при `None` молча роняла кривые на пол — дефект жил
/// незамеченным, потому что «тихая ветка» выглядела как забота о
/// носителе без автоматизации. Домены разные, и это нормально: у
/// события ключ живёт в тиках МАТЕРИАЛА (`src_in` = `source_in`), у
/// морфера — во времени БЛОКА (`src_in` = 0). Вставка переводит ключи
/// «вход к входу» в обе стороны (`ops::rebase_curves`).
pub struct ChainEdit<'a> {
    pub fx: &'a mut Vec<FxInstance>,
    pub bypass: &'a mut Vec<bool>,
    pub auto: &'a mut Vec<Vec<kt_model::ParamCurve>>,
    pub selected: &'a mut ChainSel,
    /// Мастер-выключатель цепочки (у морфера — выключатель события).
    pub master_on: &'a mut bool,
    /// Приколотый ПЕРВЫЙ слот, эффектом не являющийся (Р.35): у морфера
    /// это подмена источника. Своя галка, фиолетовая подпись, мышью не
    /// переставляется и крестика не имеет.
    pub head: Option<HeadSlot<'a>>,
}

/// Псевдо-слот в начале полосы.
pub struct HeadSlot<'a> {
    pub label: String,
    /// Включён ли (у морфера — `!patch.src_off`).
    pub on: &'a mut bool,
    pub hint: &'a str,
}

/// Что выбрано в полосе. Отдельный тип, а не «голова = 0» — иначе
/// смещение индексов эффектов на единицу гарантированно даёт off-by-one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChainSel {
    #[default]
    Head,
    Fx(usize),
}

impl ChainSel {
    /// Индекс слота эффекта, если выбран именно он.
    pub fn fx(self) -> Option<usize> {
        match self {
            ChainSel::Fx(i) => Some(i),
            ChainSel::Head => None,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct ChainChanged {
    pub fx: bool,
    pub bypass: bool,
    pub auto: bool,
    pub master: bool,
    /// Тронут приколотый слот (галка морф-слота).
    pub head: bool,
}

impl ChainChanged {
    pub fn any(self) -> bool {
        self.fx || self.bypass || self.auto || self.master || self.head
    }
}

/// Потолок подписи слота в символах (Р.38 пп.7-8, дословно: «если в кроше
/// Source указать сурс с длинным именем — то весь чейн разлетается по
/// длинне» и «если накидать много крошек — то последняя надпись упирается
/// в край»).
///
/// Причина была не в длине как таковой: полоса живёт в
/// `horizontal_wrapped`, а он ставит `TextWrapMode::Wrap`, и подпись
/// переносится ВНУТРИ СЕБЯ — в имени файла пробелов нет, поэтому по
/// буквам. На новую строку крошка при этом целиком не уходила: она
/// заранее ужималась под остаток ширины.
pub const SLOT_NAME_CHARS: usize = 22;
/// Хват подписи слота выше и ниже рисунка (правило 1 чек-листа).
pub const SLOT_GRAB_PAD: f32 = 2.0;

/// Имя, усечённое одной строкой с многоточием, а не перенесённое.
///
/// ОДНА функция на все места, где показывается имя пользователя: подпись
/// слота, голова полосы, поле источника морфера. Разойдись они — половина
/// мест снова начала бы разъезжаться, а это и была претензия пп.7-8.
pub fn elided_name(
    ui: &egui::Ui,
    name: &str,
    color: egui::Color32,
    off: bool,
) -> egui::WidgetText {
    let mut job = egui::text::LayoutJob::single_section(
        name.to_string(),
        egui::TextFormat {
            font_id: egui::TextStyle::Button.resolve(ui.style()),
            color,
            strikethrough: if off {
                egui::Stroke::new(1.0, color)
            } else {
                egui::Stroke::NONE
            },
            ..Default::default()
        },
    );
    // предел в ШИРИНЕ, а не «сколько влезет в остаток строки»: иначе
    // последняя крошка снова ужимается под край вместо переноса целиком
    job.wrap = egui::text::TextWrapping {
        max_width: ui.fonts_mut(|f| {
            let fid = egui::TextStyle::Button.resolve(ui.style());
            f.glyph_width(&fid, 'M') * SLOT_NAME_CHARS as f32
        }),
        max_rows: 1,
        overflow_character: Some('…'),
        ..Default::default()
    };
    // ГОТОВАЯ галлея, а не job: виджет переложил бы job под доступную
    // ширину строки и вернул бы перенос — то самое «разлетается»
    egui::WidgetText::Galley(ui.fonts_mut(|f| f.layout_job(job)))
}

fn slot_text(ui: &egui::Ui, name: &str, color: egui::Color32, off: bool) -> egui::WidgetText {
    elided_name(ui, name, color, off)
}

/// Полоса цепочки: мастер-галка, слоты (галка вкл/выкл, имя, крестик),
/// перестановка мышью, пикер и копипаст. `audio` — фильтр пикера.
#[allow(clippy::too_many_arguments)]
pub fn chain_strip(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    mut edit: ChainEdit<'_>,
    audio: bool,
    query: &mut String,
    clipboard: &mut Option<kt_model::ops::FxChainBlock>,
    src_in: kt_time::Tick,
    hover: &mut Option<String>,
) -> ChainChanged {
    let mut ch = ChainChanged::default();
    edit.bypass.resize(edit.fx.len(), false);
    edit.auto.resize(edit.fx.len(), Vec::new());
    let mut remove: Option<usize> = None;
    let mut swap: Option<(usize, usize)> = None;

    ui.horizontal_wrapped(|ui| {
        // мастер-галка: гасит ВСЮ цепочку, не трогая состояние слотов
        if crate::app::hint_into(
            hover,
            ui.checkbox(edit.master_on, tr(k::fx::CHAIN_ON)),
            tr(k::fx::CHAIN_ON_HINT),
        )
        .changed()
        {
            ch.master = true;
        }
        ui.separator();
        // приколотая голова: своя галка, фиолетовая подпись, ни драга,
        // ни крестика — она не эффект
        if let Some(h) = edit.head.as_mut() {
            if crate::app::hint_into(hover, ui.checkbox(h.on, ""), h.hint).changed() {
                ch.head = true;
            }
            let text = slot_text(ui, &h.label, crate::theme::MORPH_SLOT, !*h.on);
            let resp = crate::app::hint_into(
                hover,
                ui.selectable_label(*edit.selected == ChainSel::Head, text),
                h.hint,
            );
            if resp.clicked() {
                *edit.selected = ChainSel::Head;
            }
            // полное имя — в подсказке: усечённое видно, а какой файл
            // подставлен, узнать иначе негде
            if h.label.chars().count() > SLOT_NAME_CHARS {
                resp.on_hover_text(h.label.clone());
            }
            if !edit.fx.is_empty() {
                ui.label(crate::icons::CARET_RIGHT);
            }
        }
        for i in 0..edit.fx.len() {
            let off = edit.bypass[i];
            // галка включения слота (у морферов её не было вовсе)
            let mut on = !off;
            if crate::app::hint_into(
                hover,
                ui.checkbox(&mut on, ""),
                if off { tr(k::fx::ENABLE) } else { tr(k::fx::DISABLE) },
            )
            .changed()
            {
                edit.bypass[i] = !on;
                ch.bypass = true;
            }
            // Имя слота: и выбор кликом, и источник перетаскивания.
            //
            // ОДНА интеракция `click_and_drag`, а не `dnd_drag_source`
            // вокруг метки (Р.38 п.6, дословно: «в fx-chain виджетах
            // проблематично активировать ввод параметров»). В egui 0.36
            // `dnd_drag_source` регистрирует ПОВЕРХ содержимого
            // прямоугольник с `Sense::drag()` и делает это ПОСЛЕ него —
            // `hit_test` отдаёт `click: None`, и `.clicked()` внутри не
            // наступает НИКОГДА. Идиома репозитория (bin_panel.rs,
            // automation.rs, meters.rs, timeline.rs) — разбирать
            // `drag_started()`/`clicked()` у одной интеракции.
            let slot_id = egui::Id::new((&id, "slot", i));
            let name = crate::fx_editor::fx_display_name(&edit.fx[i]);
            let text = slot_text(ui, name, ui.visuals().text_color(), off);
            let drawn = ui.selectable_label(*edit.selected == ChainSel::Fx(i), text);
            // хват шире рисунка (правило 1 чек-листа): в подпись высотой
            // в строку целиться не должно быть нужно
            let resp = crate::app::hint_into(
                hover,
                ui.interact(
                    drawn.rect.expand2(egui::vec2(0.0, SLOT_GRAB_PAD)),
                    slot_id,
                    egui::Sense::click_and_drag(),
                ),
                tr(k::fx::SLOT_HINT),
            );
            if resp.clicked() {
                *edit.selected = ChainSel::Fx(i);
            }
            if resp.drag_started() {
                egui::DragAndDrop::set_payload(ui.ctx(), i);
            }
            if name.chars().count() > SLOT_NAME_CHARS {
                resp.clone().on_hover_text(name.to_string());
            }
            // отпустили чужой слот над этим — переставляем
            if let Some(from) = resp.dnd_release_payload::<usize>()
                && *from != i
            {
                swap = Some((*from, i));
            }
            if crate::app::hint_into(hover, ui.small_button(crate::icons::CLEAR), tr(k::fx::REMOVE_SLOT))
                .clicked()
            {
                remove = Some(i);
            }
            if i + 1 < edit.fx.len() {
                ui.label(crate::icons::CARET_RIGHT);
            }
        }
    });

    ui.horizontal_wrapped(|ui| {
        let head_name = edit.head.as_ref().map(|h| h.label.clone());
        match crate::fx_registry::fx_picker_with_head(
            ui,
            (&id, "add"),
            audio,
            query,
            head_name.as_deref(),
        ) {
            Some(crate::fx_registry::Picked::Fx(tmpl)) => {
                edit.fx.push(tmpl);
                edit.bypass.push(false);
                edit.auto.push(Vec::new());
                *edit.selected = ChainSel::Fx(edit.fx.len() - 1);
                ch.fx = true;
            }
            // голову не «добавляют» — она приколота; пикер лишь помогает
            // её НАЙТИ поиском наравне с эффектами
            Some(crate::fx_registry::Picked::Head) => *edit.selected = ChainSel::Head,
            None => {}
        }
        // копипаст цепочки (модель — kt_model::ops, Р.33)
        if !edit.fx.is_empty()
            && crate::app::hint_into(hover, ui.button(tr(k::fx::COPY_CHAIN)), tr(k::fx::COPY_CHAIN_HINT))
                .clicked()
        {
            let block = kt_model::ops::FxChainBlock {
                slots: edit
                    .fx
                    .iter()
                    .enumerate()
                    .map(|(i, f)| kt_model::ops::FxSlotBlock {
                        fx: f.clone(),
                        bypass: edit.bypass[i],
                        curves: edit.auto.get(i).cloned().unwrap_or_default(),
                    })
                    .collect(),
                src_in,
                fx_off: !*edit.master_on,
            };
            if let Ok(json) = serde_json::to_string_pretty(&block) {
                ui.ctx().copy_text(json);
            }
            *clipboard = Some(block);
        }
        let can_paste = clipboard.is_some();
        if crate::app::hint_into(
            hover,
            ui.add_enabled(can_paste, egui::Button::new(tr(k::fx::PASTE_CHAIN))),
            tr(k::fx::PASTE_CHAIN_HINT),
        )
        .clicked()
            && let Some(b) = clipboard.clone()
        {
            // ОДНА правда вставки: та же функция, что зовут пресеты
            // (Р.48). Ключи переезжают «вход к входу» — у события это
            // тики материала, у морфера время блока.
            kt_model::ops::append_chain_block(
                edit.fx,
                edit.bypass,
                edit.auto,
                &b,
                src_in,
            );
            *edit.selected = ChainSel::Fx(edit.fx.len().saturating_sub(1));
            ch.fx = true;
            ch.bypass = true;
            ch.auto = true;
        }
    });

    if let Some((from, to)) = swap {
        let slot = edit.fx.remove(from);
        edit.fx.insert(to, slot);
        let b = edit.bypass.remove(from);
        edit.bypass.insert(to, b);
        {
            let a = &mut *edit.auto;
            let c = a.remove(from);
            a.insert(to, c);
            ch.auto = true;
        }
        *edit.selected = ChainSel::Fx(to);
        ch.fx = true;
        ch.bypass = true;
    }
    if let Some(i) = remove {
        edit.fx.remove(i);
        edit.bypass.remove(i);
        if i < edit.auto.len() {
            edit.auto.remove(i);
            ch.auto = true;
        }
        // выбор съезжает на уцелевший слот, а голова остаётся головой
        if let Some(sel) = edit.selected.fx() {
            *edit.selected = if edit.fx.is_empty() {
                ChainSel::Head
            } else {
                ChainSel::Fx(sel.min(edit.fx.len() - 1))
            };
        }
        ch.fx = true;
        ch.bypass = true;
    }
    if edit.fx.is_empty() {
        ui.weak(tr(k::fx::CHAIN_EMPTY));
    }
    ch
}

/// Пикер медиа проекта с поиском — тот же жест, что у пикера эффектов.
/// `media` — (id, путь). Возвращает выбор: `Some(None)` — «оставить
/// источник», `Some(Some(id))` — конкретное медиа.
pub fn media_picker(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    media: &[(kt_model::MediaId, String)],
    query: &mut String,
) -> Option<Option<kt_model::MediaId>> {
    let mut picked = None;
    ui.menu_button(tr(k::mrph::CHOOSE), |ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(query)
                .hint_text(tr(k::mrph::SEARCH_MEDIA))
                .desired_width(220.0),
        );
        if ui.memory(|m| m.focused().is_none()) {
            resp.request_focus();
        }
        ui.separator();
        if ui.button(tr(k::mrph::KEEP_SOURCE)).clicked() {
            picked = Some(None);
            ui.close();
        }
        let q = query.to_lowercase();
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (mid, path) in media {
                if !q.is_empty() && !path.to_lowercase().contains(&q) {
                    continue;
                }
                let name = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                if ui.button(name).clicked() {
                    picked = Some(Some(*mid));
                    ui.close();
                }
            }
        });
    });
    let _ = id;
    picked
}

/// Тот же popup с поиском, но по произвольному списку «id → подпись»
/// (Р.35 п.2: ссылки морфера на области и метки-диапазоны). Подписи —
/// готовые имена, файловых путей тут нет.
pub fn media_picker_raw(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    items: &[(u64, String)],
    query: &mut String,
    label: &str,
) -> Option<u64> {
    let mut picked = None;
    ui.menu_button(label, |ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(query)
                .hint_text(tr(k::mrph::SEARCH_MEDIA))
                .desired_width(220.0),
        );
        if ui.memory(|m| m.focused().is_none()) {
            resp.request_focus();
        }
        ui.separator();
        let q = query.to_lowercase();
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (i, name) in items {
                if !q.is_empty() && !name.to_lowercase().contains(&q) {
                    continue;
                }
                if ui.button(name).clicked() {
                    picked = Some(*i);
                    query.clear();
                    ui.close();
                }
            }
        });
    });
    let _ = id;
    picked
}
