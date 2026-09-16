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
use kt_model::{Command, GeneratorKind, HasParams, MediaId, TextAlign, TextAnchor};

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

/// Порядок слоёв титра (Р.53, п.415). Поле `order` живёт в формате с
/// Р.52 — урок Kdenlive, где порядок был константой и перерисовал чужие
/// проекты, — но редактора у него не было: поменять местами заливку и
/// обводку было нечем. Четыре кнопки-слота, клик меняет местами с
/// соседом; подпись говорит, что верх списка рисуется ПОСЛЕДНИМ.
fn order_row(ui: &mut egui::Ui, g: &mut GeneratorKind) -> bool {
    let GeneratorKind::Text { order, .. } = g else { return false };
    let name = |l: kt_model::TextLayer| match l {
        kt_model::TextLayer::Plate => tr(k::titlew::PLATE),
        kt_model::TextLayer::Shadow => tr(k::titlew::SHADOW),
        kt_model::TextLayer::Stroke => tr(k::titlew::STROKE),
        kt_model::TextLayer::Fill => tr(k::titlew::FILL),
    };
    let mut changed = false;
    ui.horizontal(|ui| {
        crate::widgets::hint(&ui.label(tr(k::titlew::ORDER)), tr(k::titlew::HINT_ORDER));
        for i in 0..order.len() {
            let r = ui.small_button(name(order[i]));
            crate::widgets::field_probe::note(&format!("titlew.order.{i}"), r.rect);
            // клик — «поднять выше соседа»: перестановка без драга, как у
            // слотов цепочки (правило 1: попасть мышью с первого раза)
            if r.clicked() && i + 1 < order.len() {
                order.swap(i, i + 1);
                changed = true;
            }
        }
    });
    changed
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
            // мертвы до клика мимо (правило 14, дефект п.8 р.12).
            // Поле текста и ЛЮБОЕ числовое: DragValue тоже берёт фокус.
            ctx.memory_mut(|m| {
                m.surrender_focus(egui::Id::new(("title-text", ed.media)));
                if m.focused().is_some() {
                    m.stop_text_input();
                }
            });
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
            // путь перестал быть титром (undo глубже открытия) или вид
            // новее сборки — окно уходит, но не молча (правило 16)
            if kt_model::parse_gen_path(&path).is_none() {
                self.note(crate::tf!(k::genw::UNKNOWN_KIND, path));
            }
            ed.open = false;
            return;
        };
        let mut edited: Option<GeneratorKind> = None;
        let mut kind_switched = false;
        let mut revert = false;
        let mut open = ed.open;
        // Ширина фиксирована (правило 3), тело прокручивается: без
        // прокрутки подвал с «Отмена» уезжал за нижнюю кромку ноутбучного
        // экрана — окно титра самое высокое в программе (п.415).
        egui::Window::new(tr(k::titlew::TITLE))
            .id(egui::Id::new(("title", ed.media)))
            .open(&mut open)
            .default_size([380.0, 460.0])
            .min_width(380.0)
            .max_width(380.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.set_width(364.0);
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
                    widgets::hint(&r, tr(k::titlew::HINT_TEXT));
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

                // ---- вид генератора: ТОТ ЖЕ пикер, что в общем окне
                // (правило 17). Без него титр был тупиком: сменить вид
                // обратно на цвет или градиент было нечем (п.415)
                ui.horizontal(|ui| {
                    widgets::hint(&ui.label(tr(k::trn::KIND)), tr(k::genw::HINT_KIND));
                    egui::ComboBox::from_id_salt(("titlekind", ed.media))
                        .selected_text(crate::generator_editor::kind_name(&g))
                        .show_ui(ui, |ui| {
                            for kd in GeneratorKind::all() {
                                let same = kd.tag() == g.tag();
                                if ui
                                    .selectable_label(same, crate::generator_editor::kind_name(&kd))
                                    .clicked()
                                    && !same
                                {
                                    g = kd;
                                    changed = true;
                                    kind_switched = true;
                                }
                            }
                        });
                });
                if kind_switched {
                    // вид больше не титр — дальше рисовать нечего,
                    // остальное сделает роутинг ниже
                    edited = Some(g);
                    return;
                }

                // Тело прокручивается, подвал с «Отмена» остаётся на
                // месте: без этого подвал уезжал за нижнюю кромку экрана
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    // ---- семья шрифта: вендоренная + системные (С4) ----------
                    {
                        let GeneratorKind::Text { family, .. } = &mut g else { unreachable!() };
                        ui.horizontal(|ui| {
                            widgets::hint(
                                &ui.label(tr(k::titlew::FAMILY)),
                                tr(k::titlew::HINT_FAMILY),
                            );
                            egui::ComboBox::from_id_salt(("title-family", ed.media))
                                .width(190.0)
                                .selected_text(crate::widgets::elided_name(
                                    ui,
                                    family,
                                    ui.visuals().text_color(),
                                    false,
                                ))
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
                                        if ui.selectable_label(family == f, f).clicked()
                                            && family != f
                                        {
                                            family.clone_from(f);
                                            changed = true;
                                        }
                                    }
                                });
                        });
                    }

                    let mut center = false;
                    let GeneratorKind::Text { bold, align, anchor, .. } = &mut g else {
                        unreachable!();
                    };

                    // ---- начертание и выравнивание ---------------------------
                    ui.horizontal(|ui| {
                        // «жирный» теперь флаг паспорта (Р.53, п.414) — но
                        // рисуется тут же, рядом с выравниванием
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
                                        if ui
                                            .add_sized(
                                                [18.0, 14.0],
                                                egui::Button::selectable(on, "•"),
                                            )
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
                        // «по центру» — один жест вместо трёх полей: якорь в
                        // середину и позиция в середину кадра (п.415)
                        let r = ui.button(tr(k::titlew::CENTER));
                        widgets::hint(&r, tr(k::titlew::HINT_CENTER));
                        crate::widgets::field_probe::note("titlew.center", r.rect);
                        if r.clicked() {
                            *anchor = TextAnchor::Center;
                            center = true;
                            changed = true;
                        }
                    });
                    if center {
                        g.set_param("pos_x", 0.5);
                        g.set_param("pos_y", 0.5);
                    }

                    // ---- цвета слоёв: RGBA-пипетка + HEX (правила 13, п.415)
                    for (label, names, alpha) in [
                        (k::titlew::FILL, ["fill_r", "fill_g", "fill_b"], "fill_a"),
                        (k::titlew::STROKE, ["stroke_r", "stroke_g", "stroke_b"], "stroke_a"),
                        (k::titlew::SHADOW, ["shadow_r", "shadow_g", "shadow_b"], "shadow_a"),
                        (k::titlew::PLATE, ["plate_r", "plate_g", "plate_b"], "plate_a"),
                    ] {
                        changed |=
                            widgets::color_row(ui, tr(label), &mut g, names, Some(alpha), None);
                    }

                    // ---- порядок слоёв: поле формата с Р.52, редактора не
                    // было — поменять местами заливку и обводку было нечем
                    changed |= order_row(ui, &mut g);

                    // ---- числа: та же сетка, что у эффектов ------------------
                    changed |= widgets::params_grid(ui, ("titlep", ed.media), &mut g, &[]);
                });

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
                if kind_switched {
                    // смена вида — свой шаг истории (правило 20)
                    self.gesture_counter += 1;
                    ed.gesture = self.gesture_counter;
                }
                self.apply_coalesced(
                    Command::SetMediaPath { id: ed.media, path: new_path },
                    ed.gesture,
                );
                if kind_switched {
                    ed.open = false;
                    self.open_generator_editor(ed.media);
                }
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
