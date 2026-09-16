//! Окно генератора (Р.52, F-TXT-003): вид и параметры МЕДИА.
//!
//! Параметры генератора — свойство медиа, а не клипа (кэш движка
//! ключуется MediaId): окно правит `MediaRef.path` командой
//! `SetMediaPath`, и все клипы этого генератора меняются вместе
//! (семантика Vegas). Каркас — как у окна перехода: пикер вида из
//! реестра `GeneratorKind::ALL`, числа — той же `widgets::params_grid`,
//! цвет-тройка — одной пипеткой (правило UI 13), «Отмена» откатывает
//! сессию тем же gesture = один undo-шаг.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::widgets;
use eframe::egui;
use kt_model::{Command, GeneratorKind, HasParams, MediaId};

pub struct GeneratorEditor {
    pub media: MediaId,
    /// Путь на момент открытия — «Отмена» возвращает его тем же жестом.
    pub backup_path: String,
    pub gesture: u64,
    pub open: bool,
    /// (ревизия модели, число мест) — Р.53, п.413: счётчик линейно
    /// обходит клипы корня и всех лент, на кадр его считать незачем.
    uses_cache: Option<(u64, usize)>,
}

/// Локализованное имя вида. Полные пути — тест 125 ищет ключ по ним.
pub fn kind_name(kind: &GeneratorKind) -> &'static str {
    match kind {
        GeneratorKind::Solid { .. } => tr(k::genkind::SOLID),
        GeneratorKind::Text { .. } => tr(k::genkind::TEXT),
        GeneratorKind::Gradient { .. } => tr(k::genkind::GRADIENT),
        GeneratorKind::Corners { .. } => tr(k::genkind::CORNERS),
        GeneratorKind::Checker { .. } => tr(k::genkind::CHECKER),
        GeneratorKind::NoiseField { .. } => tr(k::genkind::NOISE),
        GeneratorKind::SmpteBars { .. } => tr(k::genkind::SMPTE),
        GeneratorKind::Timecode { .. } => tr(k::genkind::TIMECODE),
    }
}

/// Цветовые группы вида для пипеток окна (подпись, [имена R,G,B]).
fn color_groups(kind: &GeneratorKind) -> &'static [(crate::i18n::Key, [&'static str; 3])] {
    use crate::i18n::k;
    match kind {
        GeneratorKind::Solid { .. } => &[(k::trn::COLOR, ["color_r", "color_g", "color_b"])],
        GeneratorKind::Gradient { .. } => &[
            (k::genw::COLOR_A, ["a_r", "a_g", "a_b"]),
            (k::genw::COLOR_B, ["b_r", "b_g", "b_b"]),
        ],
        GeneratorKind::Corners { .. } => &[
            (k::genw::CORNER_TL, ["tl_r", "tl_g", "tl_b"]),
            (k::genw::CORNER_TR, ["tr_r", "tr_g", "tr_b"]),
            (k::genw::CORNER_BL, ["bl_r", "bl_g", "bl_b"]),
            (k::genw::CORNER_BR, ["br_r", "br_g", "br_b"]),
        ],
        GeneratorKind::Checker { .. } => &[
            (k::genw::COLOR_A, ["a_r", "a_g", "a_b"]),
            (k::genw::COLOR_B, ["b_r", "b_g", "b_b"]),
        ],
        GeneratorKind::Text { .. }
        | GeneratorKind::NoiseField { .. }
        | GeneratorKind::SmpteBars { .. }
        | GeneratorKind::Timecode { .. } => &[],
    }
}

impl KtApp {
    pub fn open_generator_editor(&mut self, media: MediaId) {
        // у титра СВОЁ окно (строка и цвета спекой не выражаются) — все
        // входы (карточка, меню события, создание) маршрутизируются здесь
        if let Some(kt_model::Source::File(m)) = self.model.project().source(media)
            && matches!(kt_model::parse_gen_path(&m.path), Some(GeneratorKind::Text { .. }))
        {
            self.open_title_editor(media);
            return;
        }
        if let Some(ed) = self.generator_editors.iter_mut().find(|e| e.media == media) {
            ed.open = true;
            return;
        }
        // от адреса к медиа — только через `Project::source()` (t426b)
        let Some(kt_model::Source::File(m)) = self.model.project().source(media) else {
            return;
        };
        let path = m.path.clone();
        // не генератор (или вид новее сборки — заглушка): окна нет
        if kt_model::parse_gen_path(&path).is_none() {
            return;
        }
        self.gesture_counter += 1;
        self.generator_editors.push(GeneratorEditor {
            media,
            backup_path: path,
            gesture: self.gesture_counter,
            open: true,
            uses_cache: None,
        });
    }

    pub fn draw_generator_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.generator_editors);
        for ed in &mut editors {
            self.draw_one_generator_editor(ctx, ed);
        }
        editors.retain(|e| e.open);
        self.generator_editors = editors;
    }

    fn draw_one_generator_editor(&mut self, ctx: &egui::Context, ed: &mut GeneratorEditor) {
        // медиа умерло (убрали из проекта) или путь перестал быть
        // генераторным (undo глубже открытия) — окно уходит само
        let Some(kt_model::Source::File(m)) = self.model.project().source(ed.media) else {
            ed.open = false;
            return;
        };
        let path = m.path.clone();
        let Some(cur) = kt_model::parse_gen_path(&path) else {
            ed.open = false;
            return;
        };
        let mut edited: Option<GeneratorKind> = None;
        let mut revert = false;
        let mut open = ed.open;
        egui::Window::new(tr(k::genw::TITLE))
            .id(egui::Id::new(("generator", ed.media)))
            .open(&mut open)
            .default_size([320.0, 160.0])
            .min_size([260.0, 120.0])
            .resizable(true)
            .show(ctx, |ui| {
                let mut g = cur;
                let mut changed = false;
                // ---- вид: пикер из реестра -----------------------------
                ui.horizontal(|ui| {
                    ui.label(tr(k::trn::KIND));
                    egui::ComboBox::from_id_salt(("genkind", ed.media))
                        .selected_text(kind_name(&g))
                        .show_ui(ui, |ui| {
                            for kd in GeneratorKind::all() {
                                let same = kd.tag() == g.tag();
                                if ui.selectable_label(same, kind_name(&kd)).clicked() && !same {
                                    // смена вида = его умолчания: параметры
                                    // у видов разные, переносить нечего
                                    g = kd;
                                    changed = true;
                                }
                            }
                        });
                });
                // ---- цвета: Rgb8-тройки — пипетками (правило 13, идиома
                // Dip/Flash), по группе на цвет вида; в числовой сетке
                // компоненты скрыты. У титра цвета свои — их рисует его
                // собственное окно (С3)
                let groups = color_groups(&g);
                let mut skip: Vec<&str> = Vec::new();
                for (_, names) in groups {
                    skip.extend_from_slice(names);
                }
                for (i, (label, names)) in groups.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(tr(*label));
                        let get = |n: &str| g.get_param(n).unwrap_or(0.0) as f32 / 255.0;
                        let mut c = [get(names[0]), get(names[1]), get(names[2])];
                        let r = ui.color_edit_button_rgb(&mut c);
                        if i == 0 {
                            crate::widgets::field_probe::note("genw.color", r.rect);
                        }
                        if r.changed() {
                            for (n, v) in names.iter().zip(c) {
                                g.set_param(n, (v * 255.0).round() as f64);
                            }
                            changed = true;
                        }
                    });
                }
                // тайм-код показывает время МАТЕРИАЛА — сказать прямо
                if matches!(g, GeneratorKind::Timecode { .. }) {
                    ui.weak(tr(k::genw::TC_NOTE));
                }
                changed |= widgets::params_grid(ui, ("genp", ed.media), &mut g, &skip);
                ui.separator();
                ui.horizontal(|ui| {
                    // правка заденет ВСЕ места этого генератора — сказать
                    // прямо (тот же счётчик, что у вложений: «×N»)
                    if ed.uses_cache.map(|(r, _)| r) != Some(self.model_rev) {
                        ed.uses_cache =
                            Some((self.model_rev, self.model.project().sequence_uses(ed.media)));
                    }
                    let uses = ed.uses_cache.map(|(_, n)| n).unwrap_or(0);
                    ui.label(crate::tf!(k::genw::USES, uses));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(tr(k::fx::CANCEL)).clicked() {
                            revert = true;
                        }
                    });
                });
                if changed {
                    edited = Some(g);
                }
            });
        ed.open = open;
        if let Some(g) = edited {
            let new_path = kt_model::format_gen_path(&g);
            // команда ТОЛЬКО при реально изменившемся значении (п.200)
            if new_path != path {
                self.apply_coalesced(
                    Command::SetMediaPath { id: ed.media, path: new_path },
                    ed.gesture,
                );
            }
        }
        if revert {
            if ed.backup_path != path {
                self.apply_coalesced(
                    Command::SetMediaPath { id: ed.media, path: ed.backup_path.clone() },
                    ed.gesture,
                );
            }
            ed.open = false;
        }
    }
}
