//! Окно FX-цепочки клипа (порт ClipFxChainWindow + FxChainEditorWidget):
//! полоса карточек «Media → fx → fx», форма параметров, полоса автоматизации
//! (ромбы: клик = выбор, драг = перенос t, double-click = удалить),
//! live-применение через apply_coalesced (вся сессия окна = 1 undo),
//! Отмена = откат к состоянию на момент открытия.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::*;

/// Снимок одной цепочки на момент открытия окна: сама цепочка, кривые,
/// выключатели слотов и мастер-выключатель.
pub type ChainSnapshot = (ClipId, Vec<FxInstance>, Vec<Vec<kt_model::ParamCurve>>, Vec<bool>, bool);

pub struct FxEditor {
    /// Якорь окна: событие, по которому его открыли (его id — id окна).
    pub clip: ClipId,
    /// Все цепочки, которые показывает окно: якорь и его link-партнёры
    /// (Р.33 п.2 — «для одного и того же события с примагниченными треками
    /// можно и сразу все chain в одном окне давать»).
    pub members: Vec<ClipId>,
    pub picker_query: String,
    /// Состояние КАЖДОЙ цепочки на момент открытия — «Отмена» откатывает
    /// всё окно, а не только якорь.
    pub backup: Vec<ChainSnapshot>,
    pub gesture: u64,
    /// Выбранный слот в каждой цепочке.
    pub selected: std::collections::HashMap<ClipId, usize>,
    /// Состояние виджета автоматизации на каждую (цепочку, слот):
    /// вид, выбранный ключ, незакоммиченный ввод времени.
    pub auto_views: std::collections::HashMap<(ClipId, usize), crate::automation::AutoView>,
    /// Момент, на кадре которого считаются миниатюры пикера (Р.54).
    /// Пока НЕ играем — следует за курсором (он неподвижен, картинка
    /// стабильна); при игре остаётся на последнем значении: пересчитывать
    /// десятки ячеек на каждый квант часов незачем, а мерцающая полоска
    /// хуже слегка устаревшей.
    pub preview_t: Option<kt_time::Tick>,
    /// Когда окно открыли — мисклик-гард ЗАКАЗОВ декода (п.430).
    pub opened_at: std::time::Instant,
    pub open: bool,
}

impl KtApp {
    /// Открыть окно FX. Окно, которое уже показывает эту цепочку (своим
    /// якорем или как партнёра), второй раз не открывается: у A/V-пары
    /// одно окно на обе дорожки.
    pub fn open_fx_editor(&mut self, clip: ClipId) {
        if self.fx_editors.iter().any(|e| e.members.contains(&clip)) {
            return;
        }
        let Some(c) = self.model.clip(clip).cloned() else { return };
        let members: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|m| m.id == clip || (c.link_group.is_some() && m.link_group == c.link_group))
            // якорь первым, дальше по виду и id — порядок стабилен
            .map(|m| (m.id != clip, m.kind == ClipKind::Audio, m.id.0, m.id))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|(_, _, _, id)| id)
            .collect();
        self.gesture_counter += 1;
        let backup = members
            .iter()
            .filter_map(|id| self.model.clip(*id))
            .map(|m| (m.id, m.fx.clone(), m.fx_auto.clone(), m.fx_bypass.clone(), m.fx_off))
            .collect();
        let selected = members
            .iter()
            .filter_map(|id| self.model.clip(*id).map(|m| (m.id, m.fx.len().saturating_sub(1))))
            .collect();
        self.fx_editors.push(FxEditor {
            clip,
            members,
            picker_query: String::new(),
            backup,
            gesture: self.gesture_counter,
            selected,
            auto_views: std::collections::HashMap::new(),
            preview_t: None,
            opened_at: std::time::Instant::now(),
            open: true,
        });
    }

    pub(crate) fn set_fx_coalesced(&mut self, clip: ClipId, fx: Vec<FxInstance>, gesture: u64) {
        self.apply_coalesced(Command::SetFx { id: clip, fx }, gesture);
    }

    /// Кривые автоматизации: тот же gesture, что у SetFx — один undo-шаг
    /// на сессию окна.
    pub(crate) fn set_fx_auto_coalesced(
        &mut self,
        clip: ClipId,
        mut auto: Vec<Vec<kt_model::ParamCurve>>,
        gesture: u64,
    ) {
        while auto.last().is_some_and(|v| v.is_empty()) {
            auto.pop(); // хвост без кривых не сериализуется зря
        }
        self.apply_coalesced(Command::SetFxAuto { id: clip, auto }, gesture);
    }

    pub fn draw_fx_editors(&mut self, ctx: &egui::Context) {
        let mut editors = std::mem::take(&mut self.fx_editors);
        for ed in &mut editors {
            self.draw_one_fx_editor(ctx, ed);
        }
        // закрыли окно — считать его ячейки больше незачем (Р.54): всё,
        // что осталось в очереди миниатюр, выбрасывается сменой поколения
        if editors.iter().any(|e| !e.open) {
            self.fx_thumbs.bump_generation();
        }
        editors.retain(|e| e.open);
        self.fx_editors = editors;
    }

    #[allow(clippy::too_many_lines)]
    fn draw_one_fx_editor(&mut self, ctx: &egui::Context, ed: &mut FxEditor) {
        // подсказки из ui-замыканий (там `self` занят) — в накопитель
        // кадра, потом в hover_hint: попапов нет (п.14 р.8)
        let mut hover_out: Option<String> = None;
        // цель правого клика в полосе цепочки (п.363): полоса — виджет,
        // про KtApp она не знает, и цель отдаёт наружу
        let mut panel_pick: Option<crate::ctx_menu::PanelTarget> = None;
        // окно живёт, пока жива хоть одна его цепочка
        ed.members.retain(|id| self.model.clip(*id).is_some());
        let Some(anchor) = self.model.clip(ed.clip).cloned() else {
            ed.open = false;
            return;
        };
        // пресеты: показываем только те, что законны для ЭТОГО вида
        // события (совместимость считается по реестру эффектов)
        let is_audio_anchor = anchor.kind == ClipKind::Audio;
        let preset_rows: Vec<crate::user_store::PresetRow> = self
            .presets
            .rows()
            .iter()
            .filter(|r| if is_audio_anchor { r.audio_ok } else { r.video_ok })
            .cloned()
            .collect();
        // состояние библиотеки сообщается ОДИН раз за сессию окна: файл
        // битый, файл новее нас, часть записей не разобралась
        if !self.presets_reported {
            self.presets_reported = true;
            if self.presets.is_broken() {
                self.note(tr(k::msg::PRESETS_BROKEN));
            }
            if self.presets.is_readonly() {
                self.note(tr(k::msg::PRESETS_READONLY));
            }
            let skipped = self.presets.skipped();
            if skipped > 0 {
                self.note(format!("{} {skipped}", tr(k::msg::PRESETS_SKIPPED)));
            }
        }
        let mut preset_save: Option<kt_model::ops::FxChainBlock> = None;
        let mut preset_action: Option<crate::widgets::PresetAction> = None;
        // камеры для комбо Pan3d (Р.29): id + подпись
        let cameras: Vec<(u64, String)> = self
            .model
            .project()
            .cameras
            .iter()
            .map(|c| {
                (c.id.0, if c.name.is_empty() { format!("C{}", c.id.0) } else { c.name.clone() })
            })
            .collect();
        let title = if ed.members.len() > 1 {
            crate::tf!(k::fx::TITLE_GROUP, ed.clip.0)
        } else {
            crate::tf!(
                k::fx::TITLE,
                if anchor.kind == ClipKind::Audio {
                    tr(k::fx::KIND_AUDIO)
                } else {
                    tr(k::fx::KIND_VIDEO)
                },
                ed.clip.0
            )
        };
        let playhead = self.player.transport.playhead();
        // буферы живут в приложении; в окно отдаём их на время кадра,
        // чтобы не держать &mut self внутри ui-замыкания
        let mut env_clip = self.env_clipboard.clone();
        let mut fx_clip = self.fx_clipboard.clone();
        // что окно наредактировало за кадр: применяем ПОСЛЕ отрисовки
        struct Edited {
            clip: ClipId,
            fx: Option<Vec<FxInstance>>,
            auto: Option<Vec<Vec<kt_model::ParamCurve>>>,
            bypass: Option<Vec<bool>>,
            off: Option<bool>,
        }
        let mut edits: Vec<Edited> = Vec::new();
        let mut revert = false;
        // мини-таймлайн виджета автоматизации двигает ГЛАВНЫЙ курсор
        let mut seek: Option<kt_time::Tick> = None;
        let frame_step = 90_000 / self.model.project().settings.fps.max(1) as i64;
        // окно главного таймлайна: график автоматизации показывает то же
        // самое, пока не сняли замок (Р.38 п.14)
        let tl_view = self.tl_view();
        // замок оси один на программу (Р.40 п.15)
        let axis_linked = self.auto_axis_linked;
        let mut axis_req = crate::automation::AxisReq::default();
        let mut open = ed.open;
        let members = ed.members.clone();
        // ---- контекст миниатюр пикера (Р.54, п.432) -------------------
        // WYSIWYG: ячейка показывает ЦЕПОЧКУ СОБЫТИЯ плюс кандидата в
        // хвосте, то есть ровно то, что получится после клика. Кадр —
        // ближайшая готовая тумба к позиции курсора: формула перевода
        // «тик события → кадр источника» ОДНА с филмстрипом
        // (`thumbs::material_key_t`), поэтому ключи общие и нового декода
        // полоска не порождает (инвариант №8).
        let pv_t = {
            let want = anchor
                .start
                .0
                .max(playhead.0 - playhead.0.rem_euclid(frame_step))
                .min((anchor.start.0 + anchor.duration.0 - 1).max(anchor.start.0));
            if self.player.transport.is_playing() {
                *ed.preview_t.get_or_insert(kt_time::Tick(want))
            } else {
                let t = kt_time::Tick(want);
                ed.preview_t = Some(t);
                t
            }
        };
        let pv_span = anchor.duration.0.max(1);
        let pv_u = (pv_t.0 - anchor.start.0) as f64 / pv_span as f64;
        // камера разрешается ПО ЯКОРЮ, но как если бы Pan3d в цепочке был:
        // вариант «Pan 3D» в полоске обязан показать перспективу и на
        // клипе, где 3D ещё не стоит. Правило разрешения — одно с движком
        // (kt_engine::resolve_cam3d, п.429).
        let pv_cam = kt_engine::resolve_cam3d(
            &[kt_model::FxInstance::Pan3d {
                pos_x: 0.0,
                pos_y: 0.0,
                pos_z: 0.0,
                rot_x: 0.0,
                rot_y: 0.0,
                rot_z: 0.0,
                anchor_x: 0.5,
                anchor_y: 0.5,
                scale: 1.0,
                camera: None,
            }],
            &self.model.project().cameras,
            pv_t,
            crate::fx_thumbs::REF_W,
            self.model.project().settings.width,
        );
        // цепочка ЯКОРЯ с разрешёнными кривыми: значения на момент
        // курсора, домен — материал события (Р.37: скорость ось не жмёт)
        let pv_chain: Vec<FxInstance> = {
            let mat = kt_model::clip_material_at(
                anchor.source_in,
                anchor.start,
                anchor.duration,
                anchor.reversed,
                frame_step,
                pv_t,
            );
            anchor
                .fx
                .iter()
                .enumerate()
                .filter(|(i, f)| {
                    !anchor.fx_bypass.get(*i).copied().unwrap_or(false) && !f.is_audio()
                })
                .map(|(i, f)| {
                    let mut fx = f.clone();
                    if let Some(curves) = anchor.fx_auto.get(i) {
                        kt_model::apply_fx_auto(&mut fx, curves, mat);
                    }
                    fx
                })
                .collect()
        };
        // кадр-основа: точная тумба → постер медиа → встроенный образец.
        // Пустых полосок не бывает, а ЗАКАЗЫ декода ставятся только после
        // мисклик-гарда (п.430).
        let decode_ok = crate::fx_thumbs::decode_orders_allowed(ed.opened_at.elapsed());
        let is_nest = self.model.project().sequence(anchor.media).is_some();
        let pv_step = crate::thumbs::THUMB_STEP_LADDER[1];
        let (pv_base, pv_img) = if is_nest {
            // у ленты своего файла нет, и тумб у неё не бывает (п.395) —
            // показываем образец, а не пустоту
            (crate::fx_thumbs::FxThumbBase::Pattern, None)
        } else {
            let warps = self.warp_maps();
            let key = crate::thumbs::ThumbKey {
                media: anchor.media,
                t: crate::thumbs::material_key_t(
                    anchor.source_in.0,
                    warps.get(&anchor.id),
                    anchor.duration,
                    anchor.reversed,
                    pv_t.0 - anchor.start.0,
                    pv_step,
                ),
            };
            match self.thumbs.get(&key) {
                Some(img) => (crate::fx_thumbs::FxThumbBase::Thumb(key), Some(img)),
                None => {
                    if decode_ok {
                        // ближайшая готовая тумба из sidecar-БД вместо
                        // декода (механизм скраба бина, п.14 р.7)
                        self.thumbs.request_near(key, pv_step * 2);
                    }
                    let poster = if decode_ok {
                        self.thumbs.poster_t_or_request(anchor.media)
                    } else {
                        None
                    };
                    let pk = poster
                        .map(|t| crate::thumbs::ThumbKey { media: anchor.media, t })
                        .and_then(|pk| self.thumbs.get(&pk).map(|img| (pk, img)));
                    match pk {
                        Some((pk, img)) => (crate::fx_thumbs::FxThumbBase::Thumb(pk), Some(img)),
                        None => (crate::fx_thumbs::FxThumbBase::Pattern, None),
                    }
                }
            }
        };
        let mut pv = crate::fx_thumbs::PickerCtx::new(
            &self.fx_thumbs,
            pv_base,
            pv_img,
            pv_chain,
            pv_u,
            pv_t.0,
            pv_cam,
        );
        egui::Window::new(title)
            .id(egui::Id::new(("fx", ed.clip)))
            .open(&mut open)
            .default_size([580.0, 520.0])
            .min_size([460.0, 320.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                for (n, id) in members.iter().enumerate() {
                    let Some(clip) = self.model.clip(*id).cloned() else { continue };
                    let mut fx = clip.fx.clone();
                    let mut bypass = clip.fx_bypass.clone();
                    bypass.resize(fx.len(), false);
                    let mut auto = clip.fx_auto.clone();
                    auto.resize(fx.len(), Vec::new());
                    let mut chain_on = !clip.fx_off;
                    let mut ch = crate::widgets::ChainChanged::default();
                    let mut params_changed = crate::widgets::ParamsChanged::default();
                    let head = crate::tf!(
                        k::fx::CHAIN_OF,
                        if clip.kind == ClipKind::Audio {
                            tr(k::fx::KIND_AUDIO)
                        } else {
                            tr(k::fx::KIND_VIDEO)
                        },
                        id.0
                    );
                    // свёртка на цепочку: у пары видно обе, лишнюю можно
                    // свернуть («можно дать возможность скрытия-переключения»)
                    egui::CollapsingHeader::new(head)
                        .id_salt(("fxgrp", id.0))
                        .default_open(n == 0 || members.len() <= 2)
                        .show(ui, |ui| {
                            // выбранный слот и строку поиска берём в
                            // локальные переменные: иначе два поля одного
                            // и того же `ed` одолжены одновременно
                            // у цепочки события головы нет: выбор всегда
                            // слот эффекта, поэтому индекс хранится как был
                            let mut sel_slot =
                                crate::widgets::ChainSel::Fx(*ed.selected.get(id).unwrap_or(&0));
                            let mut query = std::mem::take(&mut ed.picker_query);
                            ch = crate::widgets::chain_strip(
                                ui,
                                ("fxchain", id.0),
                                crate::widgets::ChainEdit {
                                    fx: &mut fx,
                                    bypass: &mut bypass,
                                    auto: &mut auto,
                                    selected: &mut sel_slot,
                                    master_on: &mut chain_on,
                                    head: None,
                                    presets: crate::widgets::ChainPresets {
                                        rows: &preset_rows,
                                        save: &mut preset_save,
                                        action: &mut preset_action,
                                    },
                                },
                                clip.kind == ClipKind::Audio,
                                &mut query,
                                &mut fx_clip,
                                clip.source_in,
                                &mut hover_out,
                                &mut crate::widgets::ChainCtx {
                                    chain: *id,
                                    picked: &mut panel_pick,
                                },
                                // у аудио-цепочки картинок нет: миниатюру
                                // звука не нарисовать (решение владельца)
                                if clip.kind == ClipKind::Audio { None } else { Some(&mut pv) },
                            );
                            ed.picker_query = query;
                            let sel = sel_slot.fx().unwrap_or(0);
                            ed.selected.insert(*id, sel);
                            if let Some(f) = fx.get_mut(sel) {
                                // параметры — ОДНИМ кодом по спекам модели
                                // позиция курсора в домене кривой слота —
                                // материал события (Р.37: скорость ось не
                                // сжимает)
                                let at = kt_model::clip_material_at(
                                    clip.source_in,
                                    clip.start,
                                    clip.duration,
                                    clip.reversed,
                                    frame_step,
                                    playhead,
                                );
                                let view_sel = ed.auto_views.get(&(*id, sel)).and_then(|v| v.sel);
                                let mut bind = auto.get_mut(sel).map(|curves| {
                                    crate::widgets::CurveBind { curves, sel: view_sel, at }
                                });
                                // Р.43 п.291: правка ключа ЧИСЛОМ уходит в
                                // `auto`, а не в `fx` — и коммититься обязана
                                // командой SetFxAuto. Пока флаг был один,
                                // локальный клон `auto` умирал вместе с
                                // кадром, и поле «скидывалось на предыдущее
                                // значение».
                                params_changed |= draw_video_fx_params(
                                    ui,
                                    f,
                                    bind.as_mut(),
                                    &cameras,
                                    clip.kind == ClipKind::Video,
                                    &mut hover_out,
                                );
                                let mut auto_ch = false;
                                if let Some(curves) = auto.get_mut(sel) {
                                    let view = ed.auto_views.entry((*id, sel)).or_default();
                                    // домен слота события — материал;
                                    // перевод учитывает РЕВЕРС и больше
                                    // ничего (Р.37: скорость ось не
                                    // сжимает)
                                    let to_tl = |t: kt_time::Tick| {
                                        kt_model::clip_timeline_at(
                                            clip.source_in,
                                            clip.start,
                                            clip.duration,
                                            clip.reversed,
                                            frame_step,
                                            t,
                                        )
                                    };
                                    let to_mat = |t: kt_time::Tick| {
                                        kt_model::clip_material_at(
                                            clip.source_in,
                                            clip.start,
                                            clip.duration,
                                            clip.reversed,
                                            frame_step,
                                            t,
                                        )
                                    };
                                    let actx = crate::automation::clip_ctx(
                                        &clip,
                                        f,
                                        frame_step,
                                        &to_tl,
                                        &to_mat,
                                        tl_view,
                                        axis_linked,
                                    );
                                    let r = draw_auto_for(
                                        ui,
                                        f,
                                        curves,
                                        view,
                                        actx,
                                        playhead,
                                        &mut env_clip,
                                        (id.0, sel),
                                        &mut hover_out,
                                    );
                                    auto_ch = r.changed;
                                    // правый клик по ключу — тоже цель (п.364)
                                    if let Some((ci, ki)) = r.key_menu {
                                        panel_pick = Some(crate::ctx_menu::PanelTarget::CurveKey {
                                            chain: *id,
                                            slot: sel,
                                            curve: ci,
                                            key: ki,
                                        });
                                    }
                                    // Поле параметра читает выбранный ключ
                                    // ДО отрисовки графика, поэтому смена
                                    // выбора видна ему только следующим
                                    // кадром. В простое следующего кадра
                                    // может не быть (repaint = движение,
                                    // п.38), и поле застревало на значении
                                    // прошлого ключа — просим кадр явно
                                    // (Р.43 п.291).
                                    if view.sel != view_sel {
                                        ui.ctx().request_repaint();
                                    }
                                    if let Some(t) = r.seek {
                                        seek = Some(t);
                                    }
                                    axis_req = axis_req.merge(&r);
                                }
                                ch.auto |= auto_ch;
                            }
                            // про пустую цепочку говорит сама полоса —
                            // второй раз повторять не надо
                        });
                    if ch.any() || params_changed.any() {
                        edits.push(Edited {
                            clip: *id,
                            fx: (ch.fx || params_changed.base).then_some(fx),
                            auto: (ch.auto || params_changed.curve).then_some(auto),
                            bypass: ch.bypass.then_some(bypass),
                            off: ch.master.then_some(!chain_on),
                        });
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr(k::fx::DONE)).clicked() {
                        ed.open = false;
                    }
                    if ui.button(tr(k::fx::CANCEL)).clicked() {
                        revert = true;
                        ed.open = false;
                    }
                });
            });

        // запросы полосы по пресетам (Р.48): полоса их только кладёт —
        // файл пишет окно, потому что диалог имени живёт у приложения
        if let Some(block) = preset_save.take() {
            self.preset_save = Some(crate::app::PresetSaveDialog {
                block,
                name: String::new(),
                rename_of: None,
                focus: true,
            });
        }
        match preset_action.take() {
            Some(crate::widgets::PresetAction::Rename(old)) => {
                self.preset_save = Some(crate::app::PresetSaveDialog {
                    // переименование блок не использует
                    block: kt_model::ops::FxChainBlock {
                        slots: vec![],
                        src_in: kt_time::Tick(0),
                        fx_off: false,
                    },
                    name: old.clone(),
                    rename_of: Some(old),
                    focus: true,
                });
            }
            Some(crate::widgets::PresetAction::Delete(name)) => match self.presets.remove(&name) {
                Ok(()) => self.note(format!("{} {name}", tr(k::msg::PRESET_DELETED))),
                Err(e) => self.note(format!("{}: {e}", tr(k::msg::PRESET_SAVE_FAILED))),
            },
            None => {}
        }
        if let Some(t) = panel_pick {
            self.set_panel_target(t); // цели гасят друг друга (п.363)
        }
        if let Some(h) = hover_out {
            self.hover_hint = Some(h); // подсказка едет в строку состояния
        }
        // просьбы графика про ОСЬ применяет одно место (Р.40 п.15)
        self.apply_axis_req(axis_req);
        if let Some(t) = seek {
            // «курсор внутри мини-таймлайна двигает курсор в основном
            // таймлайне, чтобы сразу видеть к чему относится» (Р.33 п.5)
            self.player.seek(kt_time::Tick(t.0.max(0)));
        }
        if !open {
            ed.open = false;
        }
        self.env_clipboard = env_clip;
        self.fx_clipboard = fx_clip;
        if revert {
            // «Отмена» откатывает ВСЁ окно: у пары обе цепочки
            for (id, fx, auto, bypass, off) in ed.backup.clone() {
                self.set_fx_coalesced(id, fx, ed.gesture);
                self.set_fx_auto_coalesced(id, auto, ed.gesture);
                self.apply_coalesced(Command::SetFxBypass { id, bypass }, ed.gesture);
                self.apply_coalesced(Command::SetFxOff { id, off }, ed.gesture);
            }
            return;
        }
        for e in edits {
            if let Some(fx) = e.fx {
                self.set_fx_coalesced(e.clip, fx, ed.gesture);
            }
            if let Some(mut auto) = e.auto {
                // ключи держим отсортированными (модель это валидирует)
                for curves in &mut auto {
                    for c in curves.iter_mut() {
                        c.keys.sort_by_key(|k| k.t.0);
                    }
                }
                self.set_fx_auto_coalesced(e.clip, auto, ed.gesture);
            }
            if let Some(mut b) = e.bypass {
                while b.last() == Some(&false) {
                    b.pop(); // хвостовые false не сериализуются зря
                }
                self.apply_coalesced(Command::SetFxBypass { id: e.clip, bypass: b }, ed.gesture);
            }
            if let Some(off) = e.off {
                self.apply_coalesced(Command::SetFxOff { id: e.clip, off }, ed.gesture);
            }
        }
    }
}

/// Секция автоматизации слота: выбор кривых, копипаст огибающих, ключ по
/// курсору — и сам виджет (`automation::draw`: график ↔ таблица).
/// У слота без автоматизируемых параметров (Mirror) не рисуется вовсе.
///
/// Возвращает `AutoResult` целиком (Р.40 п.15): кроме «изменилось» и «куда
/// двинуть курсор» в нём едут просьбы про ось — переключить замок и
/// подвинуть окно ГЛАВНОГО таймлайна. Пара `(bool, Option<Tick>)` их
/// молча теряла бы, а замок теперь один на всю программу.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_auto_for(
    ui: &mut egui::Ui,
    f: &FxInstance,
    curves: &mut Vec<kt_model::ParamCurve>,
    view: &mut crate::automation::AutoView,
    ctx: crate::automation::AutoCtx<'_>,
    playhead: kt_time::Tick,
    env_clip: &mut Option<kt_model::ops::EnvelopeBlock>,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    hover_out: &mut Option<String>,
) -> crate::automation::AutoResult {
    let (src_in, mat_end) = (ctx.range.0, ctx.range.1.0);
    // только Smooth-параметры: структурные (длина линий реверба) кривой
    // не подлежат — их смена пересобирает DSP (Р.33)
    let specs: Vec<&kt_model::ParamSpec> = f.auto_params().collect();
    if specs.is_empty() {
        return crate::automation::AutoResult::default();
    }
    let mut changed = false;
    ui.separator();
    ui.horizontal(|ui| {
        // параметры без кривой — кандидаты на добавление
        let free: Vec<&kt_model::ParamSpec> =
            specs.iter().copied().filter(|s| !curves.iter().any(|c| c.param == s.name)).collect();
        if !free.is_empty() {
            // МЕНЮ, а не ComboBox: выбранный текст у него никогда не
            // менялся (это список действий, а не селектор), и заодно у
            // кнопки появляется имя для доступности — ComboBox отдавал
            // пустое, и до пункта нельзя было добраться иначе как мышью
            ui.menu_button(tr(k::fx::AUTO_ADD), |ui| {
                {
                    for spec in &free {
                        let n: &str = &spec.name;
                        if ui.button(crate::widgets::param_label(n)).clicked() {
                            // стартовая кривая — константа текущего значения
                            let v0 = f.get_param(n).unwrap_or(spec.default);
                            curves.push(kt_model::ParamCurve {
                                param: n.to_string(),
                                ease: true,
                                keys: vec![
                                    kt_model::AutoKey { t: src_in, v: v0 },
                                    kt_model::AutoKey { t: kt_time::Tick(mat_end), v: v0 },
                                ],
                                source: Default::default(),
                            });
                            // новый параметр становится АКТИВНЫМ (Р.38
                            // п.17: «добавил второй параметр — но
                            // редактируется только первый»): раньше клик
                            // по графику продолжал класть ключи в первую
                            // кривую, и вторая выглядела мёртвой
                            view.active = curves.len() - 1;
                            view.sel = None;
                            changed = true;
                        }
                    }
                }
            });
        }
        // выбор активной кривой и кнопки ключей — В САМОМ ВИДЖЕТЕ
        // (automation::toolbar): их получают все носители кривых разом.
        // Здесь остаётся то, что про КРИВУЮ как параметр слота.
        if let Some(c) = curves.get_mut(view.active) {
            changed |= ui.checkbox(&mut c.ease, tr(k::fx::EASE)).changed();
            if ui.button(tr(k::fx::AUTO_REMOVE)).clicked() {
                let i = view.active;
                curves.remove(i);
                view.active = 0;
                view.sel = None;
                changed = true;
            }
        }
    });
    ui.horizontal(|ui| {
        // копипаст огибающих (модель — kt_model::ops, Р.33)
        if !curves.is_empty() && ui.button(tr(k::fx::COPY_ENV)).clicked() {
            let block = kt_model::ops::EnvelopeBlock {
                fx_tag: kt_model::ops::fx_tag(f),
                curves: curves.clone(),
                src_in,
            };
            if let Ok(json) = serde_json::to_string_pretty(&block) {
                ui.ctx().copy_text(json);
            }
            *env_clip = Some(block);
        }
        let same_kind = env_clip.as_ref().is_some_and(|b| b.fx_tag == kt_model::ops::fx_tag(f));
        if crate::app::hint_into(
            hover_out,
            ui.add_enabled(same_kind, egui::Button::new(tr(k::fx::PASTE_ENV))),
            tr(k::fx::PASTE_ENV_HINT),
        )
        .clicked()
            && let Some(b) = env_clip.clone()
        {
            // ключи живут в тиках МАТЕРИАЛА: переносим их на точку входа
            // этого события, иначе кривая уехала бы на чужой кусок
            let d = src_in.0 - b.src_in.0;
            for mut c in b.curves {
                for kf in &mut c.keys {
                    kf.t = kt_time::Tick((kf.t.0 + d).max(0));
                }
                curves.retain(|old| old.param != c.param);
                curves.push(c);
            }
            changed = true;
        }
        // Кнопка «+ keyframe at playhead» отсюда УБРАНА (Р.40 п.18,
        // дословно: «а еще получается "+keyframe at playhead" кнопка
        // лишняя т.к. есть в таймлайне автоматизации "+"»). Её
        // единственное отличие от «+» — она писала во ВСЕ кривые слота, о
        // чём подпись не говорила ни слова. Это поведение переехало на
        // «+» модификатором Ctrl, и подсказка о нём говорит (правило 18).
    });
    // единый виджет: график ↔ таблица (Р.33 С3)
    let mut res =
        crate::automation::draw(ui, ("auto", id), &ctx, curves, view, playhead, hover_out);
    res.changed |= changed;
    res
}
pub(crate) fn fx_display_name(f: &FxInstance) -> &'static str {
    match f {
        FxInstance::Gain { .. } => "Gain",
        // сила слоя не пользовательский эффект: в пикере её нет, а в
        // полосе морфера она подписана как «Сила» (Р.38 п.12)
        FxInstance::LayerMix { .. } => "Layer mix",
        FxInstance::SlowedReverb { .. } => "Slowed+Reverb",
        FxInstance::Pan { .. } => "Pan",
        FxInstance::CamMotion { .. } => "CamMotion",
        FxInstance::Mirror { .. } => "Mirror",
        FxInstance::Blur { .. } => "Blur",
        FxInstance::Greenscreen { .. } => "Greenscreen",
        FxInstance::Edges { .. } => "Edges",
        FxInstance::Noise { .. } => "Noise",
        FxInstance::Vhs { .. } => "VHS",
        FxInstance::Glow { .. } => "Glow",
        FxInstance::EventGrid { .. } => "EventGrid",
        FxInstance::Pan3d { .. } => "Pan 3D",
        FxInstance::Filter { .. } => "Filter",
        FxInstance::Eq { .. } => "EQ",
        FxInstance::Distortion { .. } => "Distortion",
        FxInstance::Bitcrusher { .. } => "Bitcrusher",
        FxInstance::StereoWidth { .. } => "Stereo Width",
        FxInstance::Tremolo { .. } => "Tremolo",
        FxInstance::AutoPan { .. } => "AutoPan",
        FxInstance::Vibrato { .. } => "Vibrato",
        FxInstance::Chorus { .. } => "Chorus",
        FxInstance::Flanger { .. } => "Flanger",
        FxInstance::Phaser { .. } => "Phaser",
        FxInstance::Delay { .. } => "Delay",
        FxInstance::Compressor { .. } => "Compressor",
        FxInstance::Gate { .. } => "Gate",
        FxInstance::Limiter { .. } => "Limiter",
        FxInstance::PitchShift { .. } => "Pitch Shift",
        FxInstance::ColorGrade { .. } => "Color Grade",
        FxInstance::Duotone { .. } => "Duotone",
        FxInstance::Posterize { .. } => "Posterize",
        FxInstance::Vignette { .. } => "Vignette",
        FxInstance::CornerPin { .. } => "Corner Pin",
        FxInstance::Sharpen { .. } => "Sharpen",
        FxInstance::Pixelate { .. } => "Pixelate",
        FxInstance::ChromaticAberration { .. } => "Chromatic Aberration",
        FxInstance::RadialBlur { .. } => "Radial Blur",
        FxInstance::DropShadow { .. } => "Drop Shadow",
        FxInstance::Halftone { .. } => "Halftone",
        FxInstance::LensDistort { .. } => "Lens Distort",
        FxInstance::Twirl { .. } => "Twirl",
        FxInstance::Ripple { .. } => "Ripple",
        FxInstance::Kaleidoscope { .. } => "Kaleidoscope",
        FxInstance::Cylinder3d { .. } => "Cylinder 3D",
    }
}

/// Шаблоны видео-FX с дефолтами v1 (кнопка «+ видео-FX…»).
pub(crate) fn video_fx_templates() -> Vec<(&'static str, FxInstance)> {
    use kt_model::PanMode;
    vec![
        // CamMotion из списка убран (Р.34 К2): его кейфреймы — это Pan
        // с кривыми, и старые проекты конвертируются при чтении.
        ("Pan", FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }),
        ("Mirror", FxInstance::Mirror { horizontal: true, vertical: false }),
        ("Blur", FxInstance::blur(8)),
        ("Greenscreen", FxInstance::greenscreen([0, 255, 0], 70, 25)),
        ("Edges", FxInstance::edges(48)),
        ("Noise", FxInstance::noise(0.08)),
        ("VHS", FxInstance::vhs(4, 0.4)),
        ("Glow", FxInstance::glow(0.65)),
        (
            // Р.29: первый 3D-эффект; дефолты — тождество (слой 1:1)
            "Pan 3D",
            FxInstance::Pan3d {
                pos_x: 0.0,
                pos_y: 0.0,
                pos_z: 0.0,
                rot_x: 0.0,
                rot_y: 0.0,
                rot_z: 0.0,
                anchor_x: 0.5,
                anchor_y: 0.5,
                scale: 1.0,
                camera: None,
            },
        ),
        ("EventGrid", FxInstance::EventGrid { cols: 2, rows: 2, center: true }),
        // Р.47: поточечные цвет/стиль + первая честная перспектива
        ("Color Grade", FxInstance::color_grade()),
        ("Duotone", FxInstance::duotone()),
        ("Posterize", FxInstance::posterize()),
        ("Vignette", FxInstance::vignette()),
        ("Corner Pin", FxInstance::corner_pin()),
        // Р.47 Кв3: барьеры
        ("Sharpen", FxInstance::sharpen()),
        ("Pixelate", FxInstance::pixelate()),
        ("Chromatic Aberration", FxInstance::chromatic_aberration()),
        ("Radial Blur", FxInstance::radial_blur()),
        ("Drop Shadow", FxInstance::drop_shadow()),
        ("Halftone", FxInstance::halftone()),
        // Р.47 Кв4: нелинейные гео (Warp-прим) + цилиндр
        ("Lens Distort", FxInstance::lens_distort()),
        ("Twirl", FxInstance::twirl()),
        ("Ripple", FxInstance::ripple()),
        ("Kaleidoscope", FxInstance::kaleidoscope()),
        ("Cylinder 3D", FxInstance::cylinder3d()),
    ]
}

/// Параметры эффекта: числовые — ЕДИНОЙ сеткой по спекам модели
/// (`widgets::params_grid`), а рядом — то, что числом не выражается:
/// режим кроп-рамки, флаги зеркала, пипетка ключевого цвета, выбор
/// камеры, кейфреймы CamMotion. `cameras` — (id, подпись) для комбо Pan3d.
///
/// До Р.33 у каждого варианта была своя ручная форма: одни и те же
/// величины рисовались слайдером в одном окне и drag-полем в другом, а
/// диапазоны расходились между окнами и спекой. Теперь диапазон, шаг и
/// единица приходят из одного места.
pub(crate) fn draw_video_fx_params(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    mut bind: Option<&mut crate::widgets::CurveBind<'_>>,
    cameras: &[(u64, String)],
    on_video: bool,
    // накопитель подсказок кадра (Р.48: у графика АЧХ своя подсказка)
    hover: &mut Option<String>,
) -> crate::widgets::ParamsChanged {
    use kt_model::PanMode;
    // Р.43 п.291: правка БАЗЫ и правка КРИВОЙ — разные команды. Всё
    // нечисловое (галки, режимы, пипетки, выбор камеры) — всегда база.
    let mut ch = crate::widgets::ParamsChanged::default();
    // ключевой цвет удобнее пипеткой, чем тремя числами
    let skip: &[&str] = match f {
        FxInstance::Greenscreen { .. } => &["key_r", "key_g", "key_b"],
        // цвет линий контура и оттенок свечения — тоже пипеткой (Р.33 С5)
        FxInstance::Edges { .. } => &["color_r", "color_g", "color_b"],
        FxInstance::Glow { .. } => &["tint_r", "tint_g", "tint_b"],
        FxInstance::DropShadow { .. } => &["tint_r", "tint_g", "tint_b"],
        // обе краски дуотона — пипетками (Р.47 Кв2)
        FxInstance::Duotone { .. } => {
            &["shadow_r", "shadow_g", "shadow_b", "high_r", "high_g", "high_b"]
        }
        // на ВИДЕО-экземпляре у варпа значит только скорость: реверб там
        // не звучит, и показывать его ручки — врать пользователю (Р.37)
        FxInstance::SlowedReverb { .. } if on_video => {
            &["mix", "reverb_delay_ms", "damp", "decay", "wet_gain", "predelay_ms", "width"]
        }
        _ => &[],
    };

    // --- нечисловые контролы варианта ------------------------------------
    let mode_row =
        |ui: &mut egui::Ui, mode: &mut PanMode, ch: &mut crate::widgets::ParamsChanged| {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::MODE));
                egui::ComboBox::from_id_salt("panmode").selected_text(format!("{mode:?}")).show_ui(
                    ui,
                    |ui| {
                        for m in [PanMode::Fit, PanMode::Fill, PanMode::Stretch] {
                            ch.base |= ui.selectable_value(mode, m, format!("{m:?}")).changed();
                        }
                    },
                );
            });
        };
    match f {
        FxInstance::Pan { mode, .. } => mode_row(ui, mode, &mut ch),
        FxInstance::Delay { pingpong, .. } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(pingpong, tr(k::fx::PINGPONG)).changed();
            });
        }
        // вид среза (Р.47): подписи из каталога, значения — serde-виды
        FxInstance::Filter { mode, .. } => {
            let label = |m: kt_model::FilterMode| match m {
                kt_model::FilterMode::Lowpass => tr(k::fx::FILTER_LP),
                kt_model::FilterMode::Highpass => tr(k::fx::FILTER_HP),
                kt_model::FilterMode::Bandpass => tr(k::fx::FILTER_BP),
            };
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::MODE));
                egui::ComboBox::from_id_salt("filtermode").selected_text(label(*mode)).show_ui(
                    ui,
                    |ui| {
                        for m in [
                            kt_model::FilterMode::Lowpass,
                            kt_model::FilterMode::Highpass,
                            kt_model::FilterMode::Bandpass,
                        ] {
                            ch.base |= ui.selectable_value(mode, m, label(m)).changed();
                        }
                    },
                );
            });
        }
        FxInstance::CamMotion { mode, ease, .. } => {
            let mut e = *ease;
            mode_row(ui, mode, &mut ch);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut e, tr(k::fx::EASE)).changed() {
                    ch.base = true;
                }
            });
            if let FxInstance::CamMotion { ease, .. } = f {
                *ease = e;
            }
        }
        FxInstance::SlowedReverb { frames, .. } if on_video => {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::FRAMES));
                egui::ComboBox::from_id_salt("slowedframes")
                    .selected_text(tr(match frames {
                        kt_model::FrameMode::Nearest => k::fx::FRAMES_NEAREST,
                        kt_model::FrameMode::Blend => k::fx::FRAMES_BLEND,
                    }))
                    .show_ui(ui, |ui| {
                        // Р.56 (п.448): «смешивание» законно и на ВЛОЖЕНИИ —
                        // режим наследуется содержимому, и пару смешивают
                        // его листья. Гасить тут больше нечего.
                        for (m, key) in [
                            (kt_model::FrameMode::Nearest, k::fx::FRAMES_NEAREST),
                            (kt_model::FrameMode::Blend, k::fx::FRAMES_BLEND),
                        ] {
                            ch.base |= ui.selectable_value(frames, m, tr(key)).changed();
                        }
                    });
            });
            ui.label(egui::RichText::new(tr(k::fx::RATE_LINKED)).weak().small());
        }
        FxInstance::Mirror { horizontal, vertical } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(horizontal, tr(k::fx::HORIZONTAL)).changed();
                ch.base |= ui.checkbox(vertical, tr(k::fx::VERTICAL)).changed();
            });
        }
        FxInstance::Greenscreen { key_r, key_g, key_b, .. } => {
            ui.horizontal(|ui| {
                let mut col = [*key_r as f32 / 255.0, *key_g as f32 / 255.0, *key_b as f32 / 255.0];
                ui.label(tr(k::fx::KEY_COLOR));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *key_r = (col[0] * 255.0) as u8;
                    *key_g = (col[1] * 255.0) as u8;
                    *key_b = (col[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::EventGrid { center, .. } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(center, tr(k::fx::CENTER)).changed();
            });
        }
        FxInstance::Edges { color_r, color_g, color_b, .. } => {
            ui.horizontal(|ui| {
                let mut col =
                    [*color_r as f32 / 255.0, *color_g as f32 / 255.0, *color_b as f32 / 255.0];
                ui.label(tr(k::fx::LINE_COLOR));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *color_r = (col[0] * 255.0) as u8;
                    *color_g = (col[1] * 255.0) as u8;
                    *color_b = (col[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::Glow { tint_r, tint_g, tint_b, .. } => {
            ui.horizontal(|ui| {
                let mut col =
                    [*tint_r as f32 / 255.0, *tint_g as f32 / 255.0, *tint_b as f32 / 255.0];
                ui.label(tr(k::fx::TINT));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *tint_r = (col[0] * 255.0) as u8;
                    *tint_g = (col[1] * 255.0) as u8;
                    *tint_b = (col[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::Duotone { shadow_r, shadow_g, shadow_b, high_r, high_g, high_b, .. } => {
            ui.horizontal(|ui| {
                let mut sh =
                    [*shadow_r as f32 / 255.0, *shadow_g as f32 / 255.0, *shadow_b as f32 / 255.0];
                ui.label(tr(k::fx::SHADOW_COLOR));
                if ui.color_edit_button_rgb(&mut sh).changed() {
                    *shadow_r = (sh[0] * 255.0) as u8;
                    *shadow_g = (sh[1] * 255.0) as u8;
                    *shadow_b = (sh[2] * 255.0) as u8;
                    ch.base = true;
                }
                let mut hi =
                    [*high_r as f32 / 255.0, *high_g as f32 / 255.0, *high_b as f32 / 255.0];
                ui.label(tr(k::fx::HIGH_COLOR));
                if ui.color_edit_button_rgb(&mut hi).changed() {
                    *high_r = (hi[0] * 255.0) as u8;
                    *high_g = (hi[1] * 255.0) as u8;
                    *high_b = (hi[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::DropShadow { tint_r, tint_g, tint_b, .. } => {
            ui.horizontal(|ui| {
                let mut col =
                    [*tint_r as f32 / 255.0, *tint_g as f32 / 255.0, *tint_b as f32 / 255.0];
                ui.label(tr(k::fx::TINT));
                if ui.color_edit_button_rgb(&mut col).changed() {
                    *tint_r = (col[0] * 255.0) as u8;
                    *tint_g = (col[1] * 255.0) as u8;
                    *tint_b = (col[2] * 255.0) as u8;
                    ch.base = true;
                }
            });
        }
        FxInstance::RadialBlur { spin, .. } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(spin, tr(k::fx::SPIN)).changed();
            });
        }
        FxInstance::PitchShift { .. } => {
            // правило 19: диалог обязан сказать, ГДЕ менять то, чего в нём
            // нет — движок питча живёт в настройках проекта
            ui.label(tr(k::fx::PITCH_ENGINE_NOTE));
        }
        FxInstance::Noise { color, .. } => {
            ui.horizontal(|ui| {
                ch.base |= ui.checkbox(color, tr(k::fx::COLOR_NOISE)).changed();
            });
        }
        _ => {}
    }

    // --- график АЧХ эквалайзера (Р.48, п.347) ------------------------------
    // Рисуется ПЕРЕД сеткой: драг правит значения до того, как поля их
    // покажут, — и в одном кадре они уже синхронны. Поля при этом
    // остаются: график их дополняет, а не заменяет.
    if matches!(f, FxInstance::Eq { .. }) {
        ch |= crate::eq_graph::draw(ui, f, bind.as_deref_mut(), hover);
    }

    // --- числовые параметры: одна сетка на все варианты -------------------
    // связанная с кривыми (Р.38 пп.15-16): у параметра с живой кривой поле
    // правит ВЫБРАННЫЙ КЛЮЧ, а не невидимую базу
    ch |= crate::widgets::params_grid_bound(ui, ("fxp", fx_display_name(f)), f, skip, bind);

    // --- хвост: камера Pan3d и кейфреймы CamMotion ------------------------
    match f {
        FxInstance::Pan3d { camera, .. } => {
            ui.horizontal(|ui| {
                ui.label(tr(k::fx::CAMERA));
                let sel = match camera {
                    None => tr(k::fx::CAMERA_ACTIVE).to_string(),
                    Some(id) => cameras
                        .iter()
                        .find(|(cid, _)| *cid == id.0)
                        .map(|(_, n)| n.clone())
                        .unwrap_or_else(|| format!("C{}", id.0)),
                };
                egui::ComboBox::from_id_salt("pan3dcam").selected_text(sel).show_ui(ui, |ui| {
                    ch.base |=
                        ui.selectable_value(camera, None, tr(k::fx::CAMERA_ACTIVE)).changed();
                    for (cid, name) in cameras {
                        ch.base |= ui
                            .selectable_value(camera, Some(kt_model::CameraId(*cid)), name.clone())
                            .changed();
                    }
                });
            });
        }
        FxInstance::CamMotion { keyframes, .. } => {
            ui.weak(crate::tf!(k::fx::KEYFRAMES_N, keyframes.len()));
            ui.horizontal(|ui| {
                if ui.button(tr(k::fx::ADD_KF)).clicked() {
                    let t = keyframes.last().map(|k| (k.t + 0.25).min(1.0)).unwrap_or(0.0);
                    keyframes.push(kt_model::CamKeyframe { t, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 });
                    ch.base = true;
                }
                if !keyframes.is_empty() && ui.button(tr(k::fx::DROP_LAST)).clicked() {
                    keyframes.pop();
                    ch.base = true;
                }
            });
            for (i, kf) in keyframes.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("#{i}"));
                    ch.base |= ui
                        .add(egui::DragValue::new(&mut kf.t).range(0.0..=1.0).speed(0.005))
                        .changed();
                    ui.label(tr(k::fxp::PAN_X));
                    ch.base |= ui
                        .add(egui::DragValue::new(&mut kf.pan_x).range(-4.0..=4.0).speed(0.01))
                        .changed();
                    ui.label(tr(k::fxp::PAN_Y));
                    ch.base |= ui
                        .add(egui::DragValue::new(&mut kf.pan_y).range(-4.0..=4.0).speed(0.01))
                        .changed();
                    ui.label(tr(k::fxp::ZOOM));
                    ch.base |= ui
                        .add(egui::DragValue::new(&mut kf.zoom).range(0.05..=32.0).speed(0.01))
                        .changed();
                });
            }
        }
        _ => {}
    }
    ch
}
