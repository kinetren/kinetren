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
    /// Выбранное в ОДНОЙ полосе цепочки (Р.35 п.3): голова —
    /// подмена источника, дальше эффекты обоих видов подряд.
    pub sel: crate::widgets::ChainSel,
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
            sel: Default::default(),
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

    /// Поле источника морф-слота: пикер медиа проекта с поиском, дроп
    /// файла из системы, карточка из бина и сдвиг точки входа. Показано,
    /// когда в полосе выбрана ГОЛОВА (Р.35 п.3).
    fn morph_source_section(
        &self,
        ui: &mut egui::Ui,
        ed: &mut MorphEditor,
        ev: &mut MorpherEvent,
        pick_file: &mut bool,
    ) -> bool {
        let mut changed = false;
        let media_list: Vec<(MediaId, String)> =
            self.model.project().media.iter().map(|m| (m.id, m.path.clone())).collect();
        let cur_name = ev
            .patch
            .media
            .and_then(|id| media_list.iter().find(|(i, _)| *i == id).map(|(_, p)| p.clone()))
            .unwrap_or_else(|| tr(k::mrph::KEEP_SOURCE).to_string());
        // Поле источника (Р.33 п.2): принимает дроп файла из системы и
        // карточки из медиа-бина, открывает список медиа проекта с
        // поиском и системный диалог файла.
        let frame = egui::Frame::group(ui.style())
            .fill(ui.visuals().extreme_bg_color)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(k::mrph::NEW_SOURCE));
                    ui.strong(short_name(&cur_name));
                    if let Some(id) = crate::widgets::media_picker(
                        ui,
                        ("morph_src", ed.id.0),
                        &media_list,
                        &mut ed.src_query,
                    ) {
                        ev.patch.media = id;
                        changed = true;
                    }
                    if ui.button(tr(k::mrph::BROWSE)).clicked() {
                        *pick_file = true;
                    }
                });
                ui.weak(tr(k::mrph::SOURCE_DROP));
            });
        let rect = frame.response.rect;
        ed.src_rect = Some(rect);
        // карточка из бина: тот же payload, что несёт перетаскивание на таймлайн
        if ui.rect_contains_pointer(rect)
            && let Some(id) = egui::DragAndDrop::payload::<crate::bin_panel::BinDrag>(ui.ctx())
        {
            ui.painter().rect_stroke(
                rect,
                3.0,
                egui::Stroke::new(2.0, crate::theme::ACCENT),
                egui::StrokeKind::Inside,
            );
            if ui.input(|i| i.pointer.any_released()) {
                ev.patch.media = Some(id.0);
                changed = true;
                let _ = egui::DragAndDrop::take_payload::<crate::bin_panel::BinDrag>(ui.ctx());
            }
        }
        ui.separator();
        // --- сдвиг source-in (delta / absolute, D4) ---------
        let mut absolute = ev.patch.absolute_source_in.is_some();
        if ui.checkbox(&mut absolute, tr(k::mrph::ABSOLUTE_IN)).changed() {
            ev.patch.absolute_source_in = absolute.then_some(Tick(0));
            changed = true;
        }
        if let Some(abs) = &mut ev.patch.absolute_source_in {
            let mut ms = abs.0 / MS;
            ui.horizontal(|ui| {
                ui.label(tr(k::mrph::SOURCE_IN_MS));
                if ui.add(egui::DragValue::new(&mut ms).speed(10).range(0..=i64::MAX)).changed() {
                    *abs = Tick(ms * MS);
                    changed = true;
                }
            });
        } else {
            let mut ms = ev.patch.source_in_delta.0 / MS;
            ui.horizontal(|ui| {
                ui.label(tr(k::mrph::SOURCE_IN_DELTA_MS));
                if ui.add(egui::DragValue::new(&mut ms).speed(10)).changed() {
                    ev.patch.source_in_delta = Tick(ms * MS);
                    changed = true;
                }
            });
        }
        changed
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
        // имена медиа для подписи морф-слота
        let media_names: Vec<(MediaId, String)> = self
            .model
            .project()
            .media
            .iter()
            .map(|m| (m.id, m.path.clone()))
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
        // чем заполняется замороженный список: регион приоритетнее
        // выделения, оба расширяются link-группами (правило v1 —
        // единственная точка сбора целей на всю программу)
        let selection: Vec<ClipId> = self.morph_target_ids();
        // имена ОДНОГО пространства: области и метки-диапазоны. `live` —
        // те, что реально существуют: мёртвая ссылка остаётся в файле и
        // рисуется ошибочным стилем (правило 16), а не выкидывается
        let areas_mode = matches!(orig.targets, MorphTargets::Areas { .. });
        let (all_names, live_names): (Vec<String>, Vec<(String, Tick)>) = {
            let p = self.model.project();
            if areas_mode {
                (
                    p.areas.iter().map(|a| a.name.clone()).collect(),
                    p.areas.iter().map(|a| (a.name.clone(), a.start)).collect(),
                )
            } else {
                let ranges: Vec<(String, Tick)> = p
                    .markers
                    .iter()
                    .filter(|m| !m.name.is_empty() && m.duration.0 > 0)
                    .map(|m| (m.name.clone(), m.t))
                    .collect();
                (ranges.iter().map(|(n, _)| n.clone()).collect(), ranges)
            }
        };
        let mut seek_to: Option<Tick> = None;
        let mut need_selection = false;
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
                // --- на кого и когда действует (Р.35) ------------------------
                let frozen = ev.targets.clip_ids().to_vec();
                ui.horizontal(|ui| {
                    ui.label(tr(k::mrph::ON));
                    let cur = match &ev.targets {
                        MorphTargets::Clips { ids } => crate::tf!(k::mrph::ON_CLIPS, ids.len()),
                        MorphTargets::Areas { names } => {
                            crate::tf!(k::mrph::ON_AREAS_N, names.len())
                        }
                        MorphTargets::Marks { names } => {
                            crate::tf!(k::mrph::ON_MARKS_N, names.len())
                        }
                        MorphTargets::Below => tr(k::mrph::ON_BELOW).to_string(),
                    };
                    egui::ComboBox::from_id_salt(("morph_on", ed.id.0))
                        .selected_text(cur)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(false, tr(k::mrph::ON_BELOW)).clicked()
                                && !matches!(ev.targets, MorphTargets::Below)
                            {
                                ev.targets = MorphTargets::Below;
                                changed = true;
                            }
                            // именованные ссылки (Р.35 п.2): область и
                            // метка-диапазон живут в ОДНОМ пространстве
                            // имён, поэтому и режимов ровно два, а не
                            // «выберите сначала вид ссылки»
                            if ui.selectable_label(false, tr(k::mrph::ON_AREAS)).clicked()
                                && !matches!(ev.targets, MorphTargets::Areas { .. })
                            {
                                ev.targets = MorphTargets::Areas { names: Vec::new() };
                                changed = true;
                            }
                            if ui.selectable_label(false, tr(k::mrph::ON_MARKS)).clicked()
                                && !matches!(ev.targets, MorphTargets::Marks { .. })
                            {
                                ev.targets = MorphTargets::Marks { names: Vec::new() };
                                changed = true;
                            }
                            // замораживаем ТЕКУЩЕЕ выделение; пустой
                            // список у Clips запрещён моделью, поэтому
                            // отказ показывается ошибкой, а не молчанием
                            let sel: Vec<ClipId> = if frozen.is_empty() {
                                selection.clone()
                            } else {
                                frozen.clone()
                            };
                            let label = crate::tf!(k::mrph::ON_CLIPS, sel.len());
                            if ui.selectable_label(false, label).clicked() {
                                if sel.is_empty() {
                                    need_selection = true;
                                } else if ev.targets.clip_ids() != sel.as_slice() {
                                    ev.targets = MorphTargets::Clips { ids: sel };
                                    changed = true;
                                }
                            }
                        });
                    let mut timed = ev.timed;
                    // в ссылочных режимах время задаёт САМА ссылка —
                    // галка тут ничего не решает и честно гаснет
                    let by_ref =
                        matches!(ev.targets, MorphTargets::Areas { .. } | MorphTargets::Marks { .. });
                    if crate::app::hint_into(
                        &mut hover_out,
                        ui.add_enabled(!by_ref, egui::Checkbox::new(&mut timed, tr(k::mrph::TIMED))),
                        if by_ref { tr(k::mrph::TIMED_BY_REF) } else { tr(k::mrph::TIMED_HINT) },
                    )
                    .changed()
                    {
                        ev.timed = timed;
                        changed = true;
                    }
                });
                if let MorphTargets::Areas { names } | MorphTargets::Marks { names } =
                    &mut ev.targets
                {
                    let areas = matches!(orig.targets, MorphTargets::Areas { .. });
                    changed |= refs_row(
                        ui,
                        ed.id.0,
                        names,
                        &all_names,
                        &live_names,
                        areas,
                        &mut ed.src_query,
                        &mut seek_to,
                        &mut hover_out,
                    );
                } else if matches!(ev.targets, MorphTargets::Below) {
                    ui.label(egui::RichText::new(tr(k::mrph::BELOW_NOTE)).weak().small());
                } else {
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
                }
                ui.separator();

                // Р.35 п.3: ОДНА полоса цепочки, в которой первым
                // стоит фиолетовый морф-слот (подмена источника). Виды
                // эффектов в патче лежат раздельно (video/audio), но
                // пользователю это не показываем: полоса склеивает их и
                // раскладывает обратно по `FxInstance::class()`.
                let (mut fx, mut bypass, split) = merge_chains(&ev.patch);
                let mut src_on = !ev.patch.src_off;
                let ch = crate::widgets::chain_strip(
                    ui,
                    ("morphchain", ed.id.0),
                    crate::widgets::ChainEdit {
                        fx: &mut fx,
                        bypass: &mut bypass,
                        auto: None,
                        selected: &mut ed.sel,
                        master_on: &mut enabled,
                        head: Some(crate::widgets::HeadSlot {
                            label: head_label(&ev.patch, &media_names),
                            on: &mut src_on,
                            hint: tr(k::mrph::SRC_SLOT_HINT),
                        }),
                    },
                    false,
                    &mut ed.picker_query,
                    &mut fx_clip,
                    kt_time::Tick(0),
                    &mut hover_out,
                );
                if ch.any() {
                    ev.patch.src_off = !src_on;
                    split_chains(&mut ev.patch, &fx, &bypass, split);
                    changed = true;
                }
                ui.label(egui::RichText::new(tr(k::mrph::LIVE_NOTE)).weak().small());
                ui.separator();

                match ed.sel {
                    crate::widgets::ChainSel::Head => {
                        changed |= self.morph_source_section(ui, ed, &mut ev, &mut pick_file);
                    }
                    crate::widgets::ChainSel::Fx(i) => {
                        // параметры — ТЕМ ЖЕ кодом, что в окне события
                        if let Some(f) = fx.get_mut(i)
                            && crate::fx_editor::draw_video_fx_params(ui, f, &cameras, false)
                        {
                            split_chains(&mut ev.patch, &fx, &bypass, split);
                            changed = true;
                        }
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
        if need_selection {
            // невозможность действия — ошибкой, а не блёклой строкой (п.16 р.28)
            self.note(tr(k::mrph::NEED_SELECTION));
        }
        if let Some(t) = seek_to {
            // клик по чипу подматывает таймлайн к этому месту
            self.player.seek(t);
        }
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

/// Подпись фиолетового морф-слота: что подмена ДЕЛАЕТ. «Источник не
/// менять» — честное состояние, а не пустое место: слот приколот всегда.
fn head_label(patch: &MorphPatch, media: &[(MediaId, String)]) -> String {
    match patch.media {
        Some(id) => {
            let name = media
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, p)| short_name(p))
                .unwrap_or_else(|| format!("#{}", id.0));
            crate::tf!(k::mrph::SRC_SLOT, name)
        }
        None if patch.absolute_source_in.is_some() || patch.source_in_delta.0 != 0 => {
            tr(k::mrph::SRC_SLOT_SHIFT).to_string()
        }
        None => tr(k::mrph::SRC_SLOT_NONE).to_string(),
    }
}

/// Склеить обе цепочки патча в ОДНУ полосу. `split` запоминает, сколько
/// слотов было видео: обратная раскладка идёт по `FxInstance::class()`,
/// а число нужно только для проверки, что порядок не потерялся.
fn merge_chains(patch: &MorphPatch) -> (Vec<FxInstance>, Vec<bool>, usize) {
    let mut fx = patch.video_fx.clone();
    let mut bypass = patch.video_bypass.clone();
    bypass.resize(fx.len(), false);
    let n = fx.len();
    fx.extend(patch.audio_fx.iter().cloned());
    let mut ab = patch.audio_bypass.clone();
    ab.resize(patch.audio_fx.len(), false);
    bypass.extend(ab);
    (fx, bypass, n)
}

/// Разложить полосу обратно по видам. Вид слота — функция от самого
/// эффекта (`is_audio`), поэтому склейка обратима без побочных таблиц;
/// относительный порядок внутри каждого вида сохраняется.
fn split_chains(patch: &mut MorphPatch, fx: &[FxInstance], bypass: &[bool], _was_video: usize) {
    patch.video_fx.clear();
    patch.video_bypass.clear();
    patch.audio_fx.clear();
    patch.audio_bypass.clear();
    for (i, f) in fx.iter().enumerate() {
        let off = bypass.get(i).copied().unwrap_or(false);
        if f.is_audio() {
            patch.audio_fx.push(f.clone());
            patch.audio_bypass.push(off);
        } else {
            patch.video_fx.push(f.clone());
            patch.video_bypass.push(off);
        }
    }
}

/// Строка ссылок морфера: чипы с крестиками и `[+ ссылка]` тем же
/// popup'ом с поиском, что медиа-пикер. Мёртвая ссылка (имя, которого в
/// проекте нет) рисуется ошибочным стилем — это не «пусто», а «сломано»
/// (п.16 р.28). Клик по живому чипу подматывает таймлайн.
#[allow(clippy::too_many_arguments)]
fn refs_row(
    ui: &mut egui::Ui,
    id: u64,
    names: &mut Vec<String>,
    all: &[String],
    live: &[(String, Tick)],
    areas: bool,
    query: &mut String,
    seek: &mut Option<Tick>,
    hover: &mut Option<String>,
) -> bool {
    let mut changed = false;
    let mut remove: Option<usize> = None;
    ui.horizontal_wrapped(|ui| {
        for (i, n) in names.iter().enumerate() {
            let at = live.iter().find(|(ln, _)| ln == n).map(|(_, t)| *t);
            let text = egui::RichText::new(n.clone()).color(match at {
                Some(_) => crate::theme::AREA,
                None => egui::Color32::from_rgb(255, 105, 97),
            });
            let resp = crate::app::hint_into(
                hover,
                ui.selectable_label(false, text),
                match at {
                    Some(_) => tr(k::mrph::REF_GOTO),
                    None => tr(k::mrph::REF_DEAD),
                },
            );
            if resp.clicked()
                && let Some(t) = at
            {
                *seek = Some(t);
            }
            if ui.small_button(crate::icons::CLEAR).clicked() {
                remove = Some(i);
            }
        }
        // добавление — тот же popup с поиском, что у медиа-пикера
        let free: Vec<(u64, String)> = all
            .iter()
            .enumerate()
            .filter(|(_, n)| !names.contains(n))
            .map(|(i, n)| (i as u64, n.clone()))
            .collect();
        if let Some(i) = crate::widgets::media_picker_raw(
            ui,
            ("morph_refs", id),
            &free,
            query,
            tr(if areas { k::mrph::ADD_AREA_REF } else { k::mrph::ADD_MARK_REF }),
        ) && let Some((_, n)) = free.iter().find(|(j, _)| *j == i)
        {
            names.push(n.clone());
            changed = true;
        }
        if all.is_empty() {
            ui.weak(tr(if areas { k::mrph::NO_AREAS } else { k::mrph::NO_MARKS }));
        }
    });
    if let Some(i) = remove {
        names.remove(i);
        changed = true;
    }
    changed
}
