//! Окно камеры (Р.34 К3).
//!
//! До этого раунда у камер не было редактора ВООБЩЕ: событие создавалось
//! из меню камерной дорожки, рисовалось бирюзовым блоком — и всё; ни
//! выделить, ни перенести, ни удалить, ни поправить параметры. При этом
//! комментарий у `add_camera_at_playhead` обещал «параметры правятся в
//! окне камеры (двойной клик)» — то есть код врал ровно так же, как
//! врала подсказка про FX до п.204.
//!
//! Устроено как окно перехода: та же `widgets::params_grid` из тех же
//! `ParamSpec` и ТОТ ЖЕ виджет автоматизации, что у эффектов — своего
//! редактора кейфреймов у камеры больше нет.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::widgets;
use eframe::egui;
use kt_model::{CameraEvent, CameraId, Command, HasParams, ParamCurve};
use kt_time::Tick;

pub struct CameraEditor {
    pub id: CameraId,
    /// Состояние на момент открытия — «Отмена» возвращает его.
    pub backup: CameraEvent,
    pub gesture: u64,
    pub view: crate::automation::AutoView,
    pub open: bool,
}

impl KtApp {
    pub fn open_camera_editor(&mut self, id: CameraId) {
        if let Some(ed) = self.camera_editors.iter_mut().find(|e| e.id == id) {
            ed.open = true;
            return;
        }
        let Some(c) = self.model.project().cameras.iter().find(|c| c.id == id).cloned() else {
            return;
        };
        self.gesture_counter += 1;
        self.camera_editors.push(CameraEditor {
            id,
            backup: c,
            gesture: self.gesture_counter,
            view: Default::default(),
            open: true,
        });
    }

    pub fn draw_camera_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.camera_editors);
        for ed in &mut editors {
            self.draw_one_camera_editor(ctx, ed);
        }
        editors.retain(|e| e.open);
        self.camera_editors = editors;
    }

    fn draw_one_camera_editor(&mut self, ctx: &egui::Context, ed: &mut CameraEditor) {
        // документ для публикации — один клон на отрисовку окна (Р.59)
        let pub_doc = self.model.document();
        let Some(cur) = self.model.project().cameras.iter().find(|c| c.id == ed.id).cloned() else {
            ed.open = false; // камеру удалили — окно уходит
            return;
        };
        let mut hover_out: Option<String> = None;
        let mut edited: Option<CameraEvent> = None;
        let mut revert = false;
        let mut seek: Option<Tick> = None;
        let mut axis_req = crate::automation::AxisReq::default();
        let mut open = ed.open;
        let playhead = self.player.transport.playhead();
        let frame_step = 90_000 / self.model.project().settings.fps.max(1) as i64;
        let title = if cur.name.is_empty() {
            crate::tf!(k::cam::TITLE, ed.id.0)
        } else {
            crate::tf!(k::cam::TITLE_NAMED, cur.name.clone())
        };
        egui::Window::new(title)
            .id(egui::Id::new(("camera", ed.id)))
            .open(&mut open)
            .default_size([560.0, 480.0])
            .min_size([420.0, 320.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                let mut c = cur.clone();
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label(tr(k::cam::NAME));
                    changed |= ui.text_edit_singleline(&mut c.name).changed();
                });
                // числовые параметры — та же сетка, что у эффектов
                // подписи камеры — та же дверь публикации (Р.59): камера
                // это структура ленты, и «наезд выпуска» объявляется
                // наружу ровно так же, как заголовок
                let pubc = self.publish_ctx(
                    &pub_doc,
                    kt_model::ClipId(0),
                    kt_model::ParamOwner::Camera { id: ed.id },
                );
                changed |= widgets::params_grid(ui, ("camp", ed.id), &mut c.cam, &[], Some(&pubc));
                ui.horizontal(|ui| {
                    changed |= ui.checkbox(&mut c.cam.ortho, tr(k::cam::ORTHO)).changed();
                    // точка интереса: two-node камера, как в AE
                    let mut look = c.cam.target.is_some();
                    if crate::app::hint_into(
                        &mut hover_out,
                        ui.checkbox(&mut look, tr(k::cam::LOOK_AT)),
                        tr(k::cam::LOOK_AT_HINT),
                    )
                    .changed()
                    {
                        c.cam.target = look.then_some((0.0, 0.0, 0.0));
                        changed = true;
                    }
                });
                if let Some(t) = &mut c.cam.target {
                    ui.horizontal(|ui| {
                        for (label, v) in
                            [(tr(k::fxp::POS_X), &mut t.0), (tr(k::fxp::POS_Y), &mut t.1)]
                        {
                            ui.label(label);
                            changed |= ui.add(egui::DragValue::new(v).speed(1.0)).changed();
                        }
                        ui.label(tr(k::fxp::POS_Z));
                        changed |= ui.add(egui::DragValue::new(&mut t.2).speed(1.0)).changed();
                    });
                }
                ui.separator();

                // ---- автоматизация: ТОТ ЖЕ виджет, что у эффектов -------
                ui.horizontal(|ui| {
                    let specs = c.cam.param_specs();
                    let mut add: Option<String> = None;
                    egui::ComboBox::from_id_salt(("camauto", ed.id))
                        .selected_text(tr(k::fx::AUTO_ADD))
                        .show_ui(ui, |ui| {
                            for s in specs {
                                if c.auto.iter().any(|cu| cu.param == s.name) {
                                    continue;
                                }
                                if ui.button(widgets::param_label(&s.name)).clicked() {
                                    add = Some(s.name.to_string());
                                }
                            }
                        });
                    if let Some(name) = add {
                        let v = c.cam.get_param(&name).unwrap_or(0.0);
                        c.auto.push(ParamCurve {
                            param: name,
                            keys: vec![kt_model::AutoKey::at(Tick(0), v)],
                            source: Default::default(),
                        });
                        changed = true;
                    }
                });
                let tl_view = self.tl_view();
                // домен кривых камеры — тики ОТ НАЧАЛА события
                let start = c.start;
                let to_tl = move |t: Tick| Tick(start.0 + t.0);
                let to_cur = move |t: Tick| Tick(t.0 - start.0);
                let actx = crate::automation::AutoCtx {
                    specs: c.cam.param_specs(),
                    range: (Tick(0), Tick(c.duration.0.max(1))),
                    view: crate::automation::view_in_curve(&to_cur, tl_view),
                    to_timeline: &to_tl,
                    to_curve: &to_cur,
                    frame_step,
                    linked: self.auto_axis_linked,
                    default_shape: self.model.document().settings.default_shape,
                };
                let res = crate::automation::draw(
                    ui,
                    ("camauto", ed.id.0),
                    &actx,
                    &mut c.auto,
                    &mut ed.view,
                    playhead,
                    &mut hover_out,
                );
                changed |= res.changed;
                seek = res.seek;
                // замок оси один на программу (Р.40 п.15)
                axis_req = axis_req.merge(&res);

                ui.separator();
                ui.horizontal(|ui| {
                    // ДЛИНА ПРАВИТСЯ (Р.38 п.5): была read-only подписью,
                    // а кромок у блока не было — изменить её было нечем
                    ui.label(crate::tf!(k::cam::LENGTH, ""));
                    let mut secs = cur.duration.0 as f64 / 90_000.0;
                    if ui
                        .add(
                            egui::DragValue::new(&mut secs)
                                .range(0.04..=3600.0)
                                .speed(0.05)
                                .suffix(" s"),
                        )
                        .changed()
                    {
                        c.duration = kt_time::Tick(
                            ((secs * 90_000.0) as i64).max(kt_model::MIN_CLIP_DURATION.0),
                        );
                        changed = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(tr(k::fx::CANCEL)).clicked() {
                            revert = true;
                        }
                    });
                });
                if changed {
                    edited = Some(c);
                }
            });
        ed.open = open;
        if let Some(c) = edited {
            self.apply_coalesced(Command::SetCamera(c), ed.gesture);
        }
        if revert {
            self.apply_coalesced(Command::SetCamera(ed.backup.clone()), ed.gesture);
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
