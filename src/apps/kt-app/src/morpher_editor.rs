//! Окно редактора morpher'а (порт MorpherEditWindow v1, sourceeditwidgets):
//! показывает замороженные цели; для Replace — выбор нового источника из
//! медиа проекта + сдвиг source-in (delta или absolute — фикс D4 v1); для
//! FX-батча — редактирование двух цепочек (видео/аудио), действующих LIVE
//! (фикс D1 v1). Все правки — apply_coalesced(SetMorpher): вся сессия
//! окна = один undo-шаг. Автоматизация в батч-цепочках не редактируется —
//! после bake доступен полный FX-редактор клипа.

use crate::i18n::{k, tr};
use crate::app::KtApp;
use eframe::egui;
use kt_model::*;
use kt_time::{TICK_HZ, Tick};

pub struct MorphEditor {
    pub id: MorphId,
    pub picker_query: String,
    /// Поиск в пикере источника (Replace-морфер).
    pub src_query: String,
    pub gesture: u64,
    pub open: bool,
    /// Выбранные слоты цепочек батча (видео, аудио).
    pub sel: (usize, usize),
    /// Прямоугольник поля источника на экране: по нему `handle_drops`
    /// узнаёт, что файл бросили ИМЕННО СЮДА (Р.33 п.2 — «хотелось бы
    /// чтобы можно было и dnd туда делать файла»).
    pub src_rect: Option<egui::Rect>,
    /// Снимок состояния на момент открытия — для «Отмена» (в окне FX
    /// откат был, у морфера его не было вовсе).
    pub backup: Option<kt_model::MorpherEvent>,
}

const MS: i64 = TICK_HZ / 1000; // тиков в мс

impl KtApp {
    pub fn open_morph_editor(&mut self, id: MorphId) {
        if self.morph_editors.iter().any(|e| e.id == id) {
            return;
        }
        self.gesture_counter += 1;
        let backup = self.model.project().morphers.iter().find(|m| m.id == id).cloned();
        self.morph_editors.push(MorphEditor {
            id,
            picker_query: String::new(),
            src_query: String::new(),
            gesture: self.gesture_counter,
            open: true,
            sel: (0, 0),
            src_rect: None,
            backup,
        });
    }

    /// Новый источник Replace-морфера из ФАЙЛА: медиа добавляется в
    /// проект (если его там не было) и подставляется морферу — одним
    /// undo-шагом. Общая точка для системного диалога и дропа файла в
    /// поле источника (Р.33 п.2).
    pub(crate) fn set_morph_source_file(&mut self, id: MorphId, path: &str) {
        let Some(mut ev) = self.model.project().morphers.iter().find(|m| m.id == id).cloned()
        else {
            return;
        };
        let existing = self.model.project().media.iter().find(|m| m.path == path).map(|m| m.id);
        let mut cmds = Vec::new();
        let mid = match existing {
            Some(mid) => mid,
            None => {
                let mid = MediaId(self.next_media_id());
                cmds.push(Command::AddMedia { id: mid, path: path.to_string() });
                mid
            }
        };
        ev.patch.media = Some(mid);
        cmds.push(Command::SetMorpher(ev));
        self.apply_batch(cmds);
    }

    pub fn draw_morph_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.morph_editors);
        for ed in &mut editors {
            self.draw_one_morph_editor(ctx, ed);
        }
        editors.retain(|e| e.open);
        self.morph_editors = editors;
    }

    #[allow(clippy::too_many_lines)]
    fn draw_one_morph_editor(&mut self, ctx: &egui::Context, ed: &mut MorphEditor) {
        // событие исчезло (undo/удаление) — окно закрывается само (v1)
        let Some(orig) = self.model.project().morphers.iter().find(|m| m.id == ed.id).cloned()
        else {
            ed.open = false;
            return;
        };
        // с Р.35 морфер несёт и подмену, и цепочки — окно показывает всё
        // сразу; подпись выбирается по СОДЕРЖИМОМУ, а не по «виду»
        let is_batch = !orig.patch.video_fx.is_empty() || !orig.patch.audio_fx.is_empty();
        // камеры для комбо Pan3d в FX-батче (Р.29)
        let cameras: Vec<(u64, String)> = self
            .model
            .project()
            .cameras
            .iter()
            .map(|c| {
                (c.id.0, if c.name.is_empty() { format!("C{}", c.id.0) } else { c.name.clone() })
            })
            .collect();
        let title = if is_batch {
            crate::tf!(k::mrph::TITLE_BATCH, ed.id.0)
        } else {
            crate::tf!(k::mrph::TITLE, ed.id.0)
        };
        let mut ev = orig.clone();
        let mut changed = false;
        let mut bake = false;
        let mut delete = false;
        // «Отмена» откатывает сессию окна (в окне FX так было всегда, у
        // морфера отката не было вовсе — Р.33)
        let mut revert = false;
        let mut pick_file = false;
        let mut open = ed.open;
        // выключатель события (Р.33) — та же мастер-галка, что у цепочки
        // события; буфер FX общий с окном событий
        let mut enabled = ev.enabled;
        let mut fx_clip = self.fx_clipboard.clone();
        // подсказки — в статусбар, как везде (попапов в программе нет)
        let mut hover_out: Option<String> = None;
        egui::Window::new(title)
            .id(egui::Id::new(("morph", ed.id.0)))
            .open(&mut open)
            .default_size(if is_batch { [560.0, 480.0] } else { [420.0, 260.0] })
            .min_size([380.0, 200.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                // --- замороженные цели (v1: morpherTargetsDescription) -------
                let frozen = ev.targets.clip_ids().to_vec();
                let mut ids: Vec<String> =
                    frozen.iter().take(4).map(|t| format!("#{}", t.0)).collect();
                if frozen.len() > 4 {
                    ids.push("…".into());
                }
                ui.label(
                    egui::RichText::new(crate::tf!(
                        k::mrph::FROZEN,
                        frozen.len(),
                        ids.join(", ")
                    ))
                    .weak(),
                );
                ui.separator();

                // Р.35: подмена источника и цепочки живут ВМЕСТЕ, поэтому
                // окно показывает обе секции подряд, а не одну из двух
                {
                    {
                        let media = &mut ev.patch.media;
                        let source_in_delta = &mut ev.patch.source_in_delta;
                        let absolute_source_in = &mut ev.patch.absolute_source_in;
                        // --- подмена источника ------------------------------
                        let media_list: Vec<(MediaId, String)> = self
                            .model
                            .project()
                            .media
                            .iter()
                            .map(|m| (m.id, m.path.clone()))
                            .collect();
                        let cur_name = media
                            .and_then(|id| {
                                media_list.iter().find(|(i, _)| *i == id).map(|(_, p)| p.clone())
                            })
                            .unwrap_or_else(|| tr(k::mrph::KEEP_SOURCE).to_string());
                        // Поле источника (Р.33 п.2): принимает дроп файла
                        // из системы и карточки из медиа-бина, открывает
                        // список медиа проекта с поиском и системный диалог
                        // файла. Раньше это был ComboBox, а на «файла нет в
                        // проекте» отвечала блёклая строчка-инструкция.
                        let frame = egui::Frame::group(ui.style())
                            .fill(ui.visuals().extreme_bg_color)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(tr(k::mrph::NEW_SOURCE));
                                    ui.strong(short_name(&cur_name));
                                    // выбор из медиа проекта — тем же
                                    // пикером с поиском, что у эффектов
                                    if let Some(id) = crate::widgets::media_picker(
                                        ui,
                                        ("morph_src", ed.id.0),
                                        &media_list,
                                        &mut ed.src_query,
                                    ) {
                                        *media = id;
                                        changed = true;
                                    }
                                    if ui.button(tr(k::mrph::BROWSE)).clicked() {
                                        pick_file = true;
                                    }
                                });
                                ui.weak(tr(k::mrph::SOURCE_DROP));
                            });
                        let rect = frame.response.rect;
                        ed.src_rect = Some(rect);
                        // карточка из бина: тот же payload, что несёт
                        // перетаскивание на таймлайн
                        if ui.rect_contains_pointer(rect)
                            && let Some(id) =
                                egui::DragAndDrop::payload::<crate::bin_panel::BinDrag>(ui.ctx())
                        {
                            {
                                ui.painter().rect_stroke(
                                    rect,
                                    3.0,
                                    egui::Stroke::new(2.0, crate::theme::ACCENT),
                                    egui::StrokeKind::Inside,
                                );
                                if ui.input(|i| i.pointer.any_released()) {
                                    *media = Some(id.0);
                                    changed = true;
                                    let _ = egui::DragAndDrop::take_payload::<
                                        crate::bin_panel::BinDrag,
                                    >(ui.ctx());
                                }
                            }
                        }
                        ui.separator();
                        // --- сдвиг source-in (delta / absolute, D4) ---------
                        let mut absolute = absolute_source_in.is_some();
                        if ui
                            .checkbox(&mut absolute, tr(k::mrph::ABSOLUTE_IN))
                            .changed()
                        {
                            *absolute_source_in =
                                if absolute { Some(Tick(0)) } else { None };
                            changed = true;
                        }
                        if let Some(abs) = absolute_source_in {
                            let mut ms = abs.0 / MS;
                            ui.horizontal(|ui| {
                                ui.label(tr(k::mrph::SOURCE_IN_MS));
                                if ui
                                    .add(egui::DragValue::new(&mut ms).speed(10).range(0..=i64::MAX))
                                    .changed()
                                {
                                    *abs = Tick(ms * MS);
                                    changed = true;
                                }
                            });
                        } else {
                            let mut ms = source_in_delta.0 / MS;
                            ui.horizontal(|ui| {
                                ui.label(tr(k::mrph::SOURCE_IN_DELTA_MS));
                                if ui.add(egui::DragValue::new(&mut ms).speed(10)).changed() {
                                    *source_in_delta = Tick(ms * MS);
                                    changed = true;
                                }
                            });
                        }
                    }
                    ui.separator();
                    {
                        let patch = &mut ev.patch;
                        let (video_fx, audio_fx) = (&mut patch.video_fx, &mut patch.audio_fx);
                        let (video_bypass, audio_bypass) =
                            (&mut patch.video_bypass, &mut patch.audio_bypass);
                        ui.label(
                            egui::RichText::new(
                                tr(k::mrph::LIVE_NOTE),
                            )
                            .weak()
                            .small(),
                        );
                        // ОДИН виджет цепочки на события и морферы (Р.33):
                        // галки, крестики, перестановка мышью, мастер-галка
                        // и копипаст — одинаково в обоих местах
                        changed |= chain_section(
                            ui,
                            (ed.id.0, true),
                            tr(k::mrph::VIDEO_FX),
                            video_fx,
                            video_bypass,
                            &mut ed.sel.0,
                            true,
                            &mut ed.picker_query,
                            &cameras,
                            &mut enabled,
                            &mut fx_clip,
                            &mut hover_out,
                        );
                        ui.separator();
                        changed |= chain_section(
                            ui,
                            (ed.id.0, false),
                            tr(k::mrph::AUDIO_FX),
                            audio_fx,
                            audio_bypass,
                            &mut ed.sel.1,
                            false,
                            &mut ed.picker_query,
                            &cameras,
                            &mut enabled,
                            &mut fx_clip,
                            &mut hover_out,
                        );
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .button(if is_batch {
                            tr(k::mrph::BAKE_BATCH)
                        } else {
                            tr(k::mrph::BAKE_MORPH)
                        })
                        .clicked()
                    {
                        bake = true;
                    }
                    if ui.button(tr(k::mrph::REMOVE)).clicked() {
                        delete = true;
                    }
                    if crate::app::hint_into(
                        &mut hover_out,
                        ui.button(tr(k::fx::CANCEL)),
                        tr(k::mrph::REVERT_HINT),
                    )
                    .clicked()
                    {
                        revert = true;
                    }
                });
            });
        ed.open = open;

        // системный диалог файла: новый источник может не быть в проекте
        if pick_file
            && let Some(path) = rfd::FileDialog::new().pick_file()
        {
            // окно остаётся открытым: пользователь видит новый источник
            self.set_morph_source_file(ed.id, &path.to_string_lossy());
            return;
        }
        if revert {
            if let Some(b) = ed.backup.clone() {
                self.apply_coalesced(Command::SetMorpher(b), ed.gesture);
            }
            ed.open = false;
            return;
        }
        if bake {
            if self.apply(Command::BakeMorpher { id: ed.id }) {
                self.selected_morph = None;
            }
            ed.open = false;
            return;
        }
        if delete {
            if self.apply(Command::RemoveMorpher { id: ed.id }) {
                self.selected_morph = None;
            }
            ed.open = false;
            return;
        }
        self.fx_clipboard = fx_clip;
        if let Some(h) = hover_out {
            self.hover_hint = Some(h);
        }
        ev.enabled = enabled;
        changed |= ev.enabled != orig.enabled;
        if changed && ev != orig {
            self.apply_coalesced(Command::SetMorpher(ev), ed.gesture);
        }
    }
}

fn short_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Секция одной цепочки батча: полоса слотов + добавление + параметры
/// выбранного слота. Возвращает true при изменении.
#[allow(clippy::too_many_arguments)]
fn chain_section(
    ui: &mut egui::Ui,
    win_id: (u64, bool),
    title: &str,
    fx: &mut Vec<FxInstance>,
    bypass: &mut Vec<bool>,
    sel: &mut usize,
    video: bool,
    query: &mut String,
    cameras: &[(u64, String)],
    enabled: &mut bool,
    clipboard: &mut Option<kt_model::ops::FxChainBlock>,
    hover: &mut Option<String>,
) -> bool {
    let mut changed = false;
    ui.label(egui::RichText::new(title).strong());
    // морфер не хранит кривых: у батча автоматизации нет (она живёт на
    // событии), поэтому auto = None
    let ch = crate::widgets::chain_strip(
        ui,
        win_id,
        crate::widgets::ChainEdit {
            fx,
            bypass,
            auto: None,
            selected: sel,
            master_on: enabled,
        },
        !video,
        query,
        clipboard,
        kt_time::Tick(0),
        hover,
    );
    changed |= ch.any();
    if let Some(f) = fx.get_mut(*sel) {
        // те же параметры тем же кодом, что в окне событий (Р.33):
        // раньше здесь была вторая форма со СВОИМИ диапазонами
        changed |= crate::fx_editor::draw_video_fx_params(ui, f, cameras);
    }
    changed
}
