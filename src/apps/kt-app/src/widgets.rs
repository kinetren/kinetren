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
        ParamUnit::Deg => "°",
        ParamUnit::Px => " px",
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

/// Строка «подпись + значение» прямо по эффекту: чтение и запись идут
/// через `get_param`/`set_param`, поэтому кламп и округление те же, что у
/// автоматизации. true — значение изменилось.
pub fn param_row<P: HasParams + ?Sized>(
    ui: &mut egui::Ui,
    f: &mut P,
    spec: &ParamSpec,
) -> bool {
    let Some(mut v) = f.get_param(spec.name) else { return false };
    ui.label(param_label(spec.name));
    if param_value(ui, spec, &mut v).changed() {
        f.set_param(spec.name, v);
        return true;
    }
    false
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
    let specs: Vec<&ParamSpec> =
        f.param_specs().iter().filter(|s| !skip.contains(&s.name)).collect();
    if specs.is_empty() {
        return false;
    }
    let mut changed = false;
    egui::Grid::new(id).num_columns(4).spacing([8.0, 4.0]).show(ui, |ui| {
        for pair in specs.chunks(2) {
            for spec in pair {
                changed |= param_row(ui, f, spec);
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

/// Что цепочка отдаёт на редактирование. `bypass` параллелен `fx`;
/// `auto` есть только у событий (у морфера кривых нет).
pub struct ChainEdit<'a> {
    pub fx: &'a mut Vec<FxInstance>,
    pub bypass: &'a mut Vec<bool>,
    pub auto: Option<&'a mut Vec<Vec<kt_model::ParamCurve>>>,
    pub selected: &'a mut usize,
    /// Мастер-выключатель цепочки (у морфера — выключатель события).
    pub master_on: &'a mut bool,
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct ChainChanged {
    pub fx: bool,
    pub bypass: bool,
    pub auto: bool,
    pub master: bool,
}

impl ChainChanged {
    pub fn any(self) -> bool {
        self.fx || self.bypass || self.auto || self.master
    }
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
    if let Some(a) = edit.auto.as_deref_mut() {
        a.resize(edit.fx.len(), Vec::new());
    }
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
            // имя слота — источник перетаскивания: порядок меняется мышью
            let slot_id = egui::Id::new((&id, "slot", i));
            let name = crate::fx_editor::fx_display_name(&edit.fx[i]);
            let mut text = egui::RichText::new(name);
            if off {
                text = text.weak().strikethrough();
            }
            let resp = ui
                .dnd_drag_source(slot_id, i, |ui| {
                    if ui.selectable_label(*edit.selected == i, text).clicked() {
                        *edit.selected = i;
                    }
                })
                .response;
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
        if let Some(tmpl) = crate::fx_registry::fx_picker(ui, (&id, "add"), audio, query) {
            edit.fx.push(tmpl);
            edit.bypass.push(false);
            if let Some(a) = edit.auto.as_deref_mut() {
                a.push(Vec::new());
            }
            *edit.selected = edit.fx.len() - 1;
            ch.fx = true;
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
                        curves: edit
                            .auto
                            .as_deref()
                            .and_then(|a| a.get(i).cloned())
                            .unwrap_or_default(),
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
            let d = src_in.0 - b.src_in.0;
            for slot in &b.slots {
                edit.fx.push(slot.fx.clone());
                edit.bypass.push(slot.bypass);
                if let Some(a) = edit.auto.as_deref_mut() {
                    let mut curves = slot.curves.clone();
                    for c in &mut curves {
                        for kf in &mut c.keys {
                            kf.t = kt_time::Tick((kf.t.0 + d).max(0));
                        }
                    }
                    a.push(curves);
                }
            }
            *edit.selected = edit.fx.len().saturating_sub(1);
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
        if let Some(a) = edit.auto.as_deref_mut() {
            let c = a.remove(from);
            a.insert(to, c);
            ch.auto = true;
        }
        *edit.selected = to;
        ch.fx = true;
        ch.bypass = true;
    }
    if let Some(i) = remove {
        edit.fx.remove(i);
        edit.bypass.remove(i);
        if let Some(a) = edit.auto.as_deref_mut()
            && i < a.len()
        {
            a.remove(i);
            ch.auto = true;
        }
        *edit.selected = (*edit.selected).min(edit.fx.len().saturating_sub(1));
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
