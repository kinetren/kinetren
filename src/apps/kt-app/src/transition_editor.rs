//! Окно перехода (Р.33 С4): вид, его параметры, кривая аудио.
//!
//! До этого раунда у перехода не было НИ ОДНОГО окна: вид существовал в
//! одном варианте, а офсеты тянулись мышью. Претензия пользователя —
//! «на любых событиях склейки переход всегда fade… управление ими надо
//! унифицировать как fx-chain», поэтому окно устроено как FX-редактор:
//! пикер вида по категориям (реестр `TransitionKind::ALL`), параметры —
//! той же `widgets::params_grid` из тех же `ParamSpec`, что у эффектов,
//! отмена откатывает сессию целиком одним undo-шагом.

use crate::app::KtApp;
use crate::i18n::{Key, k, tr};
use crate::widgets;
use eframe::egui;
use kt_model::{AudioCurve, Command, TransitionEvent, TransitionId, TransitionKind};

pub struct TransitionEditor {
    pub id: TransitionId,
    /// Состояние виджета кривых (Р.34 К4).
    pub view: crate::automation::AutoView,
    /// Состояние на момент открытия — «Отмена» возвращает его.
    pub backup: TransitionEvent,
    pub gesture: u64,
    pub open: bool,
}

/// Категория вида для группировки в пикере. Семейства — те самые три, о
/// которых договорились с пользователем (вайпы, сдвиги, стилизованные).
pub fn kind_category(kind: &TransitionKind) -> Key {
    match kind {
        TransitionKind::Crossfade => k::trn::cat::BASIC,
        TransitionKind::Wipe { .. }
        | TransitionKind::Iris { .. }
        | TransitionKind::Clock { .. }
        | TransitionKind::Barn { .. }
        | TransitionKind::Luma { .. } => k::trn::cat::WIPE,
        TransitionKind::Push { .. }
        | TransitionKind::Slide { .. }
        | TransitionKind::Zoom { .. }
        | TransitionKind::Spin { .. }
        | TransitionKind::Whip { .. } => k::trn::cat::MOTION,
        _ => k::trn::cat::STYLE,
    }
}

/// Локализованное имя вида. Полные пути — тест 125 ищет ключ по ним.
pub fn kind_name(kind: &TransitionKind) -> &'static str {
    match kind {
        TransitionKind::Crossfade => tr(k::trn::kind::CROSSFADE),
        TransitionKind::Wipe { .. } => tr(k::trn::kind::WIPE),
        TransitionKind::Iris { .. } => tr(k::trn::kind::IRIS),
        TransitionKind::Clock { .. } => tr(k::trn::kind::CLOCK),
        TransitionKind::Barn { .. } => tr(k::trn::kind::BARN),
        TransitionKind::Luma { .. } => tr(k::trn::kind::LUMA),
        TransitionKind::Push { .. } => tr(k::trn::kind::PUSH),
        TransitionKind::Slide { .. } => tr(k::trn::kind::SLIDE),
        TransitionKind::Zoom { .. } => tr(k::trn::kind::ZOOM),
        TransitionKind::Spin { .. } => tr(k::trn::kind::SPIN),
        TransitionKind::Whip { .. } => tr(k::trn::kind::WHIP),
        TransitionKind::BlurDissolve { .. } => tr(k::trn::kind::BLUR),
        TransitionKind::Pixelate { .. } => tr(k::trn::kind::PIXELATE),
        TransitionKind::Glitch { .. } => tr(k::trn::kind::GLITCH),
        TransitionKind::Flash { .. } => tr(k::trn::kind::FLASH),
        TransitionKind::Dip { .. } => tr(k::trn::kind::DIP),
    }
}

fn curve_name(c: AudioCurve) -> &'static str {
    match c {
        AudioCurve::EqualPower => tr(k::trn::curve::EQUAL_POWER),
        AudioCurve::Linear => tr(k::trn::curve::LINEAR),
        AudioCurve::SCurve => tr(k::trn::curve::S_CURVE),
        AudioCurve::Exponential => tr(k::trn::curve::EXPONENTIAL),
    }
}

/// Направление вида, если оно у него есть: своим контролом (стрелки), а не
/// числом — направление не число, и спекой параметров не выражается.
fn dir_of(kind: &mut TransitionKind) -> Option<&mut kt_model::Dir> {
    match kind {
        TransitionKind::Wipe { dir, .. }
        | TransitionKind::Push { dir }
        | TransitionKind::Slide { dir }
        | TransitionKind::Whip { dir, .. } => Some(dir),
        _ => None,
    }
}

impl KtApp {
    pub fn open_transition_editor(&mut self, id: TransitionId) {
        if let Some(ed) = self.transition_editors.iter_mut().find(|e| e.id == id) {
            ed.open = true;
            return;
        }
        let Some(t) = self.model.project().transitions.iter().find(|t| t.id == id).cloned() else {
            return;
        };
        self.gesture_counter += 1;
        self.transition_editors.push(TransitionEditor {
            id,
            view: Default::default(),
            backup: t,
            gesture: self.gesture_counter,
            open: true,
        });
    }

    pub fn draw_transition_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.transition_editors);
        for ed in &mut editors {
            self.draw_one_transition_editor(ctx, ed);
        }
        editors.retain(|e| e.open);
        self.transition_editors = editors;
    }

    fn draw_one_transition_editor(&mut self, ctx: &egui::Context, ed: &mut TransitionEditor) {
        let Some(cur) = self.model.project().transitions.iter().find(|t| t.id == ed.id).cloned()
        else {
            ed.open = false; // переход умер (растащили клипы) — окно уходит
            return;
        };
        let mut hover_out: Option<String> = None;
        let mut edited: Option<TransitionEvent> = None;
        let mut revert = false;
        let mut seek: Option<kt_time::Tick> = None;
        let mut axis_req = crate::automation::AxisReq::default();
        let playhead = self.player.transport.playhead();
        let frame_step = 90_000 / self.model.project().settings.fps.max(1) as i64;
        // окно перехода на таймлайне: домен кривых — тик от его начала
        let w0 = cur.window(self.model.project()).map(|w| w.start).unwrap_or(kt_time::Tick::ZERO);
        let win_len = (cur.in_offset + cur.out_offset).0.max(1);
        let mut open = ed.open;
        let is_fade = cur.is_fade();
        // переход на АУДИО-дорожке: вид картинки ему не нужен вовсе —
        // звук всегда кроссфейдится, у него есть только кривая
        let audio_only = cur
            .a
            .or(cur.b)
            .and_then(|id| self.model.clip(id).map(|c| c.kind == kt_model::ClipKind::Audio))
            .unwrap_or(false);
        egui::Window::new(crate::tf!(k::trn::TITLE, ed.id.0))
            .id(egui::Id::new(("transition", ed.id)))
            .open(&mut open)
            .default_size([420.0, 260.0])
            .min_size([340.0, 200.0])
            .resizable(true)
            .show(ctx, |ui| {
                let mut t = cur.clone();
                let mut changed = false;
                // ---- вид: пикер по семействам --------------------------
                if !audio_only {
                    ui.horizontal(|ui| {
                        ui.label(tr(k::trn::KIND));
                        egui::ComboBox::from_id_salt(("trkind", ed.id))
                            .selected_text(kind_name(&t.kind))
                            .show_ui(ui, |ui| {
                                let mut cat: Option<Key> = None;
                                for k in TransitionKind::ALL {
                                    let c = kind_category(k);
                                    if cat != Some(c) {
                                        if cat.is_some() {
                                            ui.separator();
                                        }
                                        ui.label(tr(c));
                                        cat = Some(c);
                                    }
                                    let same = k.tag() == t.kind.tag();
                                    if ui.selectable_label(same, kind_name(k)).clicked() && !same {
                                        // смена вида = его УМОЛЧАНИЯ: параметры
                                        // у видов разные, переносить нечего
                                        t.kind = k.clone();
                                        changed = true;
                                    }
                                }
                            });
                    });
                    // ---- параметры вида: та же сетка, что у эффектов -------
                    let color =
                        matches!(t.kind, TransitionKind::Flash { .. } | TransitionKind::Dip { .. });
                    let skip: &[&str] =
                        if color { &["color_r", "color_g", "color_b"] } else { &[] };
                    if let Some(dir) = dir_of(&mut t.kind) {
                        ui.horizontal(|ui| {
                            ui.label(tr(k::trn::DIR));
                            for (d, label) in [
                                (kt_model::Dir::Left, tr(k::trn::dir::LEFT)),
                                (kt_model::Dir::Right, tr(k::trn::dir::RIGHT)),
                                (kt_model::Dir::Up, tr(k::trn::dir::UP)),
                                (kt_model::Dir::Down, tr(k::trn::dir::DOWN)),
                            ] {
                                if ui.selectable_label(*dir == d, label).clicked() && *dir != d {
                                    *dir = d;
                                    changed = true;
                                }
                            }
                        });
                    }
                    if let TransitionKind::Iris { shape, .. } = &mut t.kind {
                        ui.horizontal(|ui| {
                            ui.label(tr(k::trn::SHAPE));
                            for (s, label) in [
                                (kt_model::IrisShape::Circle, tr(k::trn::shape::CIRCLE)),
                                (kt_model::IrisShape::Diamond, tr(k::trn::shape::DIAMOND)),
                                (kt_model::IrisShape::Box, tr(k::trn::shape::BOX)),
                            ] {
                                if ui.selectable_label(*shape == s, label).clicked() && *shape != s
                                {
                                    *shape = s;
                                    changed = true;
                                }
                            }
                        });
                    }
                    if let TransitionKind::Clock { reverse, .. } = &mut t.kind
                        && ui.checkbox(reverse, tr(k::trn::REVERSE)).changed()
                    {
                        changed = true;
                    }
                    if let TransitionKind::Barn { vertical, .. } = &mut t.kind
                        && ui.checkbox(vertical, tr(k::trn::VERTICAL)).changed()
                    {
                        changed = true;
                    }
                    if let TransitionKind::Luma { invert, .. } = &mut t.kind
                        && ui.checkbox(invert, tr(k::trn::INVERT)).changed()
                    {
                        changed = true;
                    }
                    // инверсия перехода (Р.51, п.398) — свойство СОБЫТИЯ:
                    // жест играется задом наперёд. У симметричных видов
                    // это тождество — галка не показывается (правило 16)
                    if !t.kind.symmetric() && ui.checkbox(&mut t.invert, tr(k::trn::INVERSE)).changed()
                    {
                        changed = true;
                    }
                    if color {
                        ui.horizontal(|ui| {
                            ui.label(tr(k::trn::COLOR));
                            let get = |n: &str| t.kind.get_param(n).unwrap_or(0.0) as f32 / 255.0;
                            let mut c = [get("color_r"), get("color_g"), get("color_b")];
                            if ui.color_edit_button_rgb(&mut c).changed() {
                                for (n, v) in ["color_r", "color_g", "color_b"].iter().zip(c) {
                                    t.kind.set_param(n, (v * 255.0).round() as f64);
                                }
                                changed = true;
                            }
                        });
                    }
                    changed |= widgets::params_grid(ui, ("trp", ed.id), &mut t.kind, skip);

                    // ---- кривые параметров вида (Р.34 К4) --------------
                    if !t.kind.param_specs().is_empty() {
                        ui.horizontal(|ui| {
                            let specs = t.kind.param_specs();
                            let mut add: Option<&str> = None;
                            egui::ComboBox::from_id_salt(("trauto", ed.id))
                                .selected_text(tr(k::fx::AUTO_ADD))
                                .show_ui(ui, |ui| {
                                    for s in specs {
                                        if t.auto.iter().any(|c| c.param == s.name) {
                                            continue;
                                        }
                                        if ui.button(widgets::param_label(s.name)).clicked() {
                                            add = Some(s.name);
                                        }
                                    }
                                });
                            if let Some(name) = add {
                                let v = t.kind.get_param(name).unwrap_or(0.0);
                                t.auto.push(kt_model::ParamCurve {
                                    param: name.into(),
                                    ease: true,
                                    keys: vec![kt_model::AutoKey { t: kt_time::Tick::ZERO, v }],
                                });
                                changed = true;
                            }
                        });
                        let tl_view = self.tl_view();
                        let to_tl = move |x: kt_time::Tick| kt_time::Tick(w0.0 + x.0);
                        let to_cur = move |x: kt_time::Tick| kt_time::Tick(x.0 - w0.0);
                        let actx = crate::automation::AutoCtx {
                            specs: t.kind.param_specs(),
                            range: (kt_time::Tick::ZERO, kt_time::Tick(win_len)),
                            view: crate::automation::view_in_curve(&to_cur, tl_view),
                            to_timeline: &to_tl,
                            to_curve: &to_cur,
                            frame_step,
                            linked: self.auto_axis_linked,
                        };
                        let res = crate::automation::draw(
                            ui,
                            ("trauto", ed.id.0),
                            &actx,
                            &mut t.auto,
                            &mut ed.view,
                            playhead,
                            &mut hover_out,
                        );
                        changed |= res.changed;
                        seek = res.seek;
                        // замок оси один на программу (Р.40 п.15)
                        axis_req = axis_req.merge(&res);
                    }
                }

                // ---- звук ----------------------------------------------
                if !audio_only {
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    ui.label(tr(k::trn::AUDIO));
                    egui::ComboBox::from_id_salt(("trcurve", ed.id))
                        .selected_text(curve_name(t.audio))
                        .show_ui(ui, |ui| {
                            for c in [
                                AudioCurve::EqualPower,
                                AudioCurve::Linear,
                                AudioCurve::SCurve,
                                AudioCurve::Exponential,
                            ] {
                                if ui.selectable_label(t.audio == c, curve_name(c)).clicked()
                                    && t.audio != c
                                {
                                    t.audio = c;
                                    changed = true;
                                }
                            }
                        });
                });
                // звук у стилизованных видов всё равно кроссфейдится —
                // сказать это прямо дешевле, чем дать пользователю
                // гадать, почему «глитч» не звучит глитчем
                crate::app::hint_into(
                    &mut hover_out,
                    ui.label(tr(k::trn::AUDIO_NOTE)),
                    tr(k::trn::AUDIO_NOTE),
                );

                ui.separator();
                ui.horizontal(|ui| {
                    // длина окна — величина, которую тянут мышью; здесь
                    // она ЧИТАЕТСЯ, чтобы не гадать по пикселям
                    let len = (cur.in_offset.0 + cur.out_offset.0) as f64 / 90_000.0;
                    ui.label(crate::tf!(k::trn::LENGTH, format!("{len:.2}")));
                    if is_fade {
                        ui.label(tr(k::trn::IS_FADE));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(tr(k::fx::CANCEL)).clicked() {
                            revert = true;
                        }
                    });
                });
                if changed {
                    edited = Some(t);
                }
            });
        ed.open = open;
        if let Some(t) = edited {
            self.apply_coalesced(Command::SetTransition(t), ed.gesture);
        }
        if revert {
            self.apply_coalesced(Command::SetTransition(ed.backup.clone()), ed.gesture);
            ed.open = false;
        }
        // просьбы графика про ОСЬ применяет одно место (Р.40 п.15)
        self.apply_axis_req(axis_req);
        if let Some(t) = seek {
            self.player.seek(t);
        }
        if let Some(h) = hover_out {
            self.hover_hint = Some(h);
        }
    }
}
