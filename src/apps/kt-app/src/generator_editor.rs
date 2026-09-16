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
use kt_model::{Command, GeneratorKind, MediaId};

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
        GeneratorKind::Transparent => tr(k::genkind::TRANSPARENT),
        GeneratorKind::Rect { .. } => tr(k::genkind::RECT),
        GeneratorKind::Ellipse { .. } => tr(k::genkind::ELLIPSE),
        GeneratorKind::Line { .. } => tr(k::genkind::LINE),
        GeneratorKind::Leader { .. } => tr(k::genkind::LEADER),
        // разновидность таблицы — часть ИМЕНИ: в меню их пять, и «просто
        // таблица» не сказала бы, какая именно (правило UI 18)
        GeneratorKind::TestPattern { table } => match table {
            kt_model::TestTable::Ebu75 => tr(k::genkind::EBU75),
            kt_model::TestTable::Ebu100 => tr(k::genkind::EBU100),
            kt_model::TestTable::Grayscale => tr(k::genkind::GRAYSCALE),
            kt_model::TestTable::Steps10 => tr(k::genkind::STEPS10),
            kt_model::TestTable::RampYcbcr => tr(k::genkind::YCBCR),
        },
        GeneratorKind::WindowPattern { .. } => tr(k::genkind::WINDOW),
    }
}

/// Группа меню создания (Р.53, п.422): видов стало девятнадцать, и
/// плоский список из них — свалка. Группы — функция ВИДА, второго
/// списка не заводим.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenGroup {
    Background,
    Shapes,
    Titles,
    Check,
}

impl GenGroup {
    pub(crate) const ALL: [GenGroup; 4] =
        [GenGroup::Background, GenGroup::Shapes, GenGroup::Titles, GenGroup::Check];

    pub(crate) fn name(self) -> &'static str {
        match self {
            GenGroup::Background => tr(k::genkind::GROUP_BG),
            GenGroup::Shapes => tr(k::genkind::GROUP_SHAPES),
            GenGroup::Titles => tr(k::genkind::GROUP_TITLES),
            GenGroup::Check => tr(k::genkind::GROUP_CHECK),
        }
    }

    pub(crate) fn of(kind: &GeneratorKind) -> GenGroup {
        match kind {
            GeneratorKind::Solid { .. }
            | GeneratorKind::Gradient { .. }
            | GeneratorKind::Corners { .. }
            | GeneratorKind::Transparent
            | GeneratorKind::NoiseField { .. } => GenGroup::Background,
            GeneratorKind::Rect { .. }
            | GeneratorKind::Ellipse { .. }
            | GeneratorKind::Line { .. } => GenGroup::Shapes,
            GeneratorKind::Text { .. }
            | GeneratorKind::Timecode { .. }
            | GeneratorKind::Leader { .. } => GenGroup::Titles,
            GeneratorKind::Checker { .. }
            | GeneratorKind::SmpteBars { .. }
            | GeneratorKind::TestPattern { .. }
            | GeneratorKind::WindowPattern { .. } => GenGroup::Check,
        }
    }
}

/// Ширина самого длинного имени вида: по ней фиксируется ширина окна и
/// комбо (правило 3 «ничего не скачет» — мерить галлеей, а не «примерным»
/// числом). Считается раз на кадр, список видов короткий.
fn widest_kind_width(ctx: &egui::Context) -> f32 {
    let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("gen-measure"));
    let painter = ctx.layer_painter(layer);
    GeneratorKind::all()
        .iter()
        .map(|k| {
            painter
                .layout_no_wrap(
                    kind_name(k).to_string(),
                    egui::FontId::proportional(14.0),
                    egui::Color32::WHITE,
                )
                .size()
                .x
        })
        .fold(0.0f32, f32::max)
}

/// Цветовые группы вида для пипеток окна (подпись, [имена R,G,B]).
/// Публична для t508: параметр либо подписан, либо закрыт пипеткой —
/// третьего не бывает (Р.53, п.414).
pub(crate) fn color_groups(
    kind: &GeneratorKind,
) -> &'static [(crate::i18n::Key, [&'static str; 3])] {
    use crate::i18n::k;
    match kind {
        GeneratorKind::Solid { .. } => &[(k::trn::COLOR, ["color_r", "color_g", "color_b"])],
        GeneratorKind::Gradient { .. } => {
            &[(k::genw::COLOR_A, ["a_r", "a_g", "a_b"]), (k::genw::COLOR_B, ["b_r", "b_g", "b_b"])]
        }
        GeneratorKind::Corners { .. } => &[
            (k::genw::CORNER_TL, ["tl_r", "tl_g", "tl_b"]),
            (k::genw::CORNER_TR, ["tr_r", "tr_g", "tr_b"]),
            (k::genw::CORNER_BL, ["bl_r", "bl_g", "bl_b"]),
            (k::genw::CORNER_BR, ["br_r", "br_g", "br_b"]),
        ],
        GeneratorKind::Checker { .. } => {
            &[(k::genw::COLOR_A, ["a_r", "a_g", "a_b"]), (k::genw::COLOR_B, ["b_r", "b_g", "b_b"])]
        }
        // у фигур цвет с АЛЬФОЙ (Р.53, п.419)
        GeneratorKind::Rect { .. } | GeneratorKind::Ellipse { .. } | GeneratorKind::Line { .. } => {
            &[(k::trn::COLOR, ["color_r", "color_g", "color_b"])]
        }
        GeneratorKind::Text { .. }
        | GeneratorKind::NoiseField { .. }
        | GeneratorKind::SmpteBars { .. }
        | GeneratorKind::Timecode { .. }
        | GeneratorKind::Transparent
        | GeneratorKind::Leader { .. }
        | GeneratorKind::TestPattern { .. }
        | GeneratorKind::WindowPattern { .. } => &[],
    }
}

/// Имя альфа-компоненты цветовой группы вида, если она есть (Р.53,
/// п.419). У фигур цвет четырёхкомпонентный, у остальных видов — нет.
pub(crate) fn color_alpha(kind: &GeneratorKind) -> Option<&'static str> {
    matches!(
        kind,
        GeneratorKind::Rect { .. } | GeneratorKind::Ellipse { .. } | GeneratorKind::Line { .. }
    )
    .then_some("color_a")
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
        // не генератор (или вид новее сборки — заглушка): окна нет, но
        // МОЛЧАТЬ нельзя (правило 16 — невозможность действия видна)
        if kt_model::parse_gen_path(&path).is_none() {
            self.note(crate::tf!(k::genw::UNKNOWN_KIND, path));
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
        for ed in editors.iter().filter(|e| !e.open) {
            // закрылось — фокус ОТДАЁТСЯ (правило 14): иначе горячие
            // клавиши мертвы, пока не кликнешь мимо (дефект п.8 р.12)
            ctx.memory_mut(|m| {
                if m.focused().is_some() {
                    m.stop_text_input();
                }
            });
            let _ = ed;
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
        let mut switched = false;
        let mut revert = false;
        let mut open = ed.open;
        // Ширина ФИКСИРОВАНА (правило 3 «ничего не скачет»): у видов
        // разный состав (у сплошного цвета одна пипетка, у углов —
        // четыре), а egui `Resize` окно только растит и назад не ужимает.
        // Меряем по самому длинному имени вида, а не «примерным» числом.
        let kind_w = widest_kind_width(ctx);
        let win_w = (kind_w + 210.0).clamp(300.0, 460.0);
        egui::Window::new(tr(k::genw::TITLE))
            .id(egui::Id::new(("generator", ed.media)))
            .open(&mut open)
            .default_width(win_w)
            .min_width(win_w)
            .max_width(win_w)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(win_w - 16.0);
                let mut g = cur;
                let mut changed = false;
                // смена ВИДА — отдельный шаг истории (правило 20): один
                // Ctrl+Z обязан отменять «случайно ткнул другой вид», а
                // не всю работу в окне
                let mut kind_switched = false;
                // ---- вид: пикер из реестра -----------------------------
                ui.horizontal(|ui| {
                    widgets::hint(&ui.label(tr(k::trn::KIND)), tr(k::genw::HINT_KIND));
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
                                    kind_switched = true;
                                }
                            }
                        });
                });
                // ---- цвета: Rgb8-тройки — пипетками (правило 13, идиома
                // Dip/Flash), по группе на цвет вида; в числовой сетке
                // компоненты скрыты. У титра цвета свои — их рисует его
                // собственное окно (С3)
                let groups = color_groups(&g);
                let alpha = color_alpha(&g);
                let mut skip: Vec<&str> = Vec::new();
                for (_, names) in groups {
                    skip.extend_from_slice(names);
                }
                for (i, (label, names)) in groups.iter().enumerate() {
                    // пипетка + HEX-поле: канон пути — именно hex (п.415)
                    changed |= widgets::color_row(
                        ui,
                        tr(*label),
                        &mut g,
                        *names,
                        alpha,
                        (i == 0).then_some("genw.color"),
                    );
                }
                // тайм-код показывает время МАТЕРИАЛА — сказать прямо
                if matches!(g, GeneratorKind::Timecode { .. }) {
                    ui.weak(tr(k::genw::TC_NOTE));
                }
                // ФЛАГИ вида — один виджет на все окна (Р.53, п.414): до
                // него тон 1 кГц и цветной шум существовали в формате, а
                // включить их было нечем
                changed |= widgets::flags_row(ui, ("genf", ed.media), &mut g);
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
                    switched = kind_switched;
                }
            });
        ed.open = open;
        if let Some(g) = edited {
            let new_path = kt_model::format_gen_path(&g);
            // команда ТОЛЬКО при реально изменившемся значении (п.200)
            if new_path != path {
                if switched {
                    // смена вида — СВОЙ шаг истории (правило 20)
                    self.gesture_counter += 1;
                    ed.gesture = self.gesture_counter;
                }
                self.apply_coalesced(
                    Command::SetMediaPath { id: ed.media, path: new_path.clone() },
                    ed.gesture,
                );
                // титру своё окно: вид сменили — уходим туда (правило 17,
                // пикер один на оба окна)
                if matches!(kt_model::parse_gen_path(&new_path), Some(GeneratorKind::Text { .. })) {
                    ed.open = false;
                    self.open_title_editor(ed.media);
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
