//! Окно ТИТРА (Р.52 С3, F-TXT-001): текст, начертание, выравнивание,
//! якорь и слои оформления (F-TXT-005). Правит МЕДИА (`SetMediaPath`),
//! как окно генератора, — титр «gen:text» и есть генератор, просто с
//! собственным окном: строку и цвета `ParamSpec` не выражает («первое
//! законное исключение» из правила param_row), числа — та же
//! `params_grid`. «Отмена» = backup тем же gesture = один undo-шаг.
//! Содержимое `TextEdit` НЕ переупорядочивается bidi (правило
//! «Локализация»), при закрытии окна фокус ОТДАЁТСЯ (правило 14).

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::widgets;
use eframe::egui;
use kt_model::{Command, GeneratorKind, MediaId, TextAlign, TextAnchor};

pub struct TitleEditor {
    pub media: MediaId,
    pub backup_path: String,
    pub gesture: u64,
    pub open: bool,
    /// Кэш предупреждений на кадр (Р.53, п.413): «глифа нет» перебирает
    /// символы через резолв начертания, «шрифта нет» — карту семей, и
    /// оба считались НА КАЖДЫЙ КАДР окна. Ключ — путь: другого входа у
    /// них нет.
    warn_cache: Option<(String, Option<String>, Option<String>)>,
    /// (ревизия модели, число мест) — счётчик «мест на монтаже» линейно
    /// обходит все клипы корня и всех лент.
    uses_cache: Option<(u64, usize)>,
}

/// Предупреждение о ПОДМЕНЕ шрифта (С4): выбранной семьи на этой машине
/// нет — кадр рисуется вендоренной (решение владельца: «промах = подмена
/// вендоренным + видимое уведомление»).
pub fn font_warning(kind: &GeneratorKind) -> Option<String> {
    let GeneratorKind::Text { family, bold, .. } = kind else { return None };
    match kt_text::resolve_family(family, *bold) {
        kt_text::FontResolve::MissingFallback => {
            Some(crate::tf!(k::titlew::FONT_MISSING, family.clone()))
        }
        _ => None,
    }
}

/// Предупреждение «глифа нет» для текущего текста (правило UI 11: t100
/// проверяет каталоги, а текст титра приходит от пользователя — ответ
/// обязан быть видимым, тофу в кадре — допустимым, молчание — нет).
pub fn title_warning(kind: &GeneratorKind) -> Option<String> {
    let GeneratorKind::Text { text, family, bold, .. } = kind else { return None };
    let style = if *bold { kt_text::FaceStyle::Bold } else { kt_text::FaceStyle::Regular };
    let missing = kt_text::uncovered(text, family, style);
    let first = *missing.first()?;
    Some(crate::tf!(
        k::titlew::MISSING,
        format!("«{first}» U+{:04X}", first as u32),
        missing.len().saturating_sub(1)
    ))
}

impl KtApp {
    pub fn open_title_editor(&mut self, media: MediaId) {
        if let Some(ed) = self.title_editors.iter_mut().find(|e| e.media == media) {
            ed.open = true;
            return;
        }
        let Some(kt_model::Source::File(m)) = self.model.project().source(media) else {
            return;
        };
        let path = m.path.clone();
        if !matches!(kt_model::parse_gen_path(&path), Some(GeneratorKind::Text { .. })) {
            return;
        }
        self.gesture_counter += 1;
        self.title_editors.push(TitleEditor {
            media,
            backup_path: path,
            gesture: self.gesture_counter,
            open: true,
            warn_cache: None,
            uses_cache: None,
        });
    }

    pub fn draw_title_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.title_editors);
        for ed in &mut editors {
            self.draw_one_title_editor(ctx, ed);
        }
        for ed in editors.iter().filter(|e| !e.open) {
            // окно закрылось — фокус ОТДАЁТСЯ, иначе горячие клавиши
            // мертвы до клика мимо (правило 14, дефект п.8 р.12)
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new(("title-text", ed.media))));
        }
        editors.retain(|e| e.open);
        self.title_editors = editors;
    }

    fn draw_one_title_editor(&mut self, ctx: &egui::Context, ed: &mut TitleEditor) {
        let Some(kt_model::Source::File(m)) = self.model.project().source(ed.media) else {
            ed.open = false;
            return;
        };
        let path = m.path.clone();
        let Some(cur @ GeneratorKind::Text { .. }) = kt_model::parse_gen_path(&path) else {
            ed.open = false;
            return;
        };
        let mut edited: Option<GeneratorKind> = None;
        let mut revert = false;
        let mut open = ed.open;
        egui::Window::new(tr(k::titlew::TITLE))
            .id(egui::Id::new(("title", ed.media)))
            .open(&mut open)
            .default_size([360.0, 420.0])
            .min_size([300.0, 260.0])
            .resizable(true)
            .show(ctx, |ui| {
                let mut g = cur.clone();
                let mut changed = false;
                // ---- текст: многострочное поле (исключение из param_row) --
                {
                    let GeneratorKind::Text { text, .. } = &mut g else {
                        unreachable!("title editor only opens for gen:text");
                    };
                    let te = egui::TextEdit::multiline(text)
                        .id(egui::Id::new(("title-text", ed.media)))
                        .desired_rows(3)
                        .desired_width(f32::INFINITY);
                    let r = ui.add(te);
                    crate::widgets::field_probe::note("titlew.text", r.rect);
                    changed |= r.changed();
                }

                // ---- «глифа нет» и подмена шрифта — ошибочным стилем -----
                // считаются при СМЕНЕ ПУТИ, не на каждый кадр (п.413)
                if ed.warn_cache.as_ref().map(|(p, _, _)| p.as_str()) != Some(path.as_str()) {
                    ed.warn_cache = Some((path.clone(), title_warning(&g), font_warning(&g)));
                }
                if let Some((_, miss, font)) = &ed.warn_cache {
                    for w in [miss, font].into_iter().flatten() {
                        ui.colored_label(egui::Color32::from_rgb(255, 105, 97), w);
                    }
                }

                // ---- семья шрифта: вендоренная + системные (С4) ----------
                {
                    let GeneratorKind::Text { family, .. } = &mut g else { unreachable!() };
                    ui.horizontal(|ui| {
                        ui.label(tr(k::titlew::FAMILY));
                        egui::ComboBox::from_id_salt(("title-family", ed.media))
                            .selected_text(family.clone())
                            .show_ui(ui, |ui| {
                                let vend = kt_text::VENDORED_FAMILY;
                                if ui.selectable_label(family == vend, vend).clicked()
                                    && family != vend
                                {
                                    *family = vend.to_string();
                                    changed = true;
                                }
                                ui.separator();
                                for f in kt_text::system_families() {
                                    if ui.selectable_label(family == f, f).clicked() && family != f
                                    {
                                        family.clone_from(f);
                                        changed = true;
                                    }
                                }
                            });
                    });
                }

                let GeneratorKind::Text {
                    bold,
                    align,
                    anchor,
                    fill_r,
                    fill_g,
                    fill_b,
                    fill_a,
                    stroke_r,
                    stroke_g,
                    stroke_b,
                    stroke_a,
                    shadow_r,
                    shadow_g,
                    shadow_b,
                    shadow_a,
                    plate_r,
                    plate_g,
                    plate_b,
                    plate_a,
                    ..
                } = &mut g
                else {
                    unreachable!();
                };

                // ---- начертание и выравнивание ---------------------------
                ui.horizontal(|ui| {
                    changed |= ui.checkbox(bold, tr(k::titlew::BOLD)).changed();
                    ui.separator();
                    for (a, label) in [
                        (TextAlign::Left, tr(k::titlew::ALIGN_L)),
                        (TextAlign::Center, tr(k::titlew::ALIGN_C)),
                        (TextAlign::Right, tr(k::titlew::ALIGN_R)),
                    ] {
                        if ui.selectable_label(*align == a, label).clicked() && *align != a {
                            *align = a;
                            changed = true;
                        }
                    }
                });

                // ---- якорь 3×3 -------------------------------------------
                ui.horizontal(|ui| {
                    ui.label(tr(k::titlew::ANCHOR));
                    egui::Grid::new(("title-anchor", ed.media)).spacing([2.0, 2.0]).show(
                        ui,
                        |ui| {
                            use TextAnchor as A;
                            for (row, anchors) in [
                                [A::TopLeft, A::TopCenter, A::TopRight],
                                [A::CenterLeft, A::Center, A::CenterRight],
                                [A::BottomLeft, A::BottomCenter, A::BottomRight],
                            ]
                            .iter()
                            .enumerate()
                            {
                                for a in anchors {
                                    let on = *anchor == *a;
                                    if ui.add_sized([18.0, 14.0], egui::Button::selectable(on, "•"))
                                        .clicked()
                                        && !on
                                    {
                                        *anchor = *a;
                                        changed = true;
                                    }
                                }
                                let _ = row;
                                ui.end_row();
                            }
                        },
                    );
                });

                // ---- цвета слоёв: RGBA-пипетки (правило 13) --------------
                let color_row =
                    |ui: &mut egui::Ui, label: &str, r: &mut u8, g_: &mut u8, b: &mut u8, a: &mut u8| {
                        let mut c = egui::Color32::from_rgba_unmultiplied(*r, *g_, *b, *a);
                        ui.horizontal(|ui| {
                            ui.label(label);
                            let resp = ui.color_edit_button_srgba(&mut c);
                            if resp.changed() {
                                (*r, *g_, *b, *a) = (c.r(), c.g(), c.b(), c.a());
                                return true;
                            }
                            false
                        })
                        .inner
                    };
                changed |= color_row(ui, tr(k::titlew::FILL), fill_r, fill_g, fill_b, fill_a);
                changed |=
                    color_row(ui, tr(k::titlew::STROKE), stroke_r, stroke_g, stroke_b, stroke_a);
                changed |=
                    color_row(ui, tr(k::titlew::SHADOW), shadow_r, shadow_g, shadow_b, shadow_a);
                changed |= color_row(ui, tr(k::titlew::PLATE), plate_r, plate_g, plate_b, plate_a);

                // ---- числа: та же сетка, что у эффектов ------------------
                changed |= widgets::params_grid(ui, ("titlep", ed.media), &mut g, &[]);

                ui.separator();
                ui.horizontal(|ui| {
                    // счётчик мест — линейный обход дерева: раз на ревизию
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
