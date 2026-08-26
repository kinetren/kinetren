//! Медиа-бин (порт SourceAreaWidget + п.11 р.6): карточки с превью,
//! отсмотр «возюканьем» мыши по превью (hover x → момент файла → тумба из
//! общего кэша/sidecar-БД), имя + длительность; клик — поставить на курсор.
//!
//! Уроки CLAUDE.md: repaint при pending-тумбе — request_repaint_after,
//! не безусловный; текстуры — общие thumb_textures/LRU приложения;
//! «last good» на медиа — превью не мигает при недогруженных бакетах.

use crate::app::KtApp;
use crate::thumbs::{THUMB_STEP_LADDER, ThumbKey, ThumbState, thumb_key_t, thumb_source_step};
use eframe::egui;
use kt_model::{Command, MediaId};

const PREVIEW_W: f32 = 128.0;
const PREVIEW_H: f32 = 72.0;
/// Логических позиций скраба на файл (п.14 р.7: было 8 — «слишком мало
/// превьюшек»); тумбы филмстрипа подхватываются near-поиском в БД.
const BIN_SCRUB_SLOTS: f32 = 48.0;

/// Полезная нагрузка перетаскивания карточки из бина (п.5 р.8).
#[derive(Clone, Copy, Debug)]
pub struct BinDrag(pub MediaId);

/// Состояние перетаскивания карточки внутри бина (п.8 р.9).
#[derive(Clone, Copy, Debug)]
pub struct BinDragState {
    pub id: MediaId,
    /// Смещение курсора внутри карточки.
    pub grab: egui::Vec2,
    /// Исходная позиция — на неё откатываемся при выносе на таймлайн.
    pub start: egui::Pos2,
    /// Курсор уже покидал бин: дальше это вынос, раскладка не меняется.
    pub escaped: bool,
}

impl KtApp {
    pub fn draw_bin(&mut self, ui: &mut egui::Ui) {
        // Переключатель режимов — ИКОНКА, не галочка (п.8 р.8: галочек
        // «не к месту» больше не делаем)
        ui.horizontal(|ui| {
            let free = self.bin_free;
            let r = ui.add_sized(
                [22.0, 20.0],
                egui::Button::selectable(free, crate::icons::ARROWS_OUT),
            );
            if self.hint(r, "Свободная раскладка: карточки двигаются мышью").clicked() {
                self.bin_free = true;
            }
            let r = ui.add_sized(
                [22.0, 20.0],
                egui::Button::selectable(!free, crate::icons::SQUARES_FOUR),
            );
            if self.hint(r, "Сетка: карточки выстраиваются автоматически").clicked() {
                self.bin_free = false;
            }
        });
        let offline = self.offline.lock().unwrap().clone();
        // offline→online: негативные кэши тумб устарели (п.1 р.7)
        for id in self.bin_last_offline.difference(&offline) {
            self.thumbs.invalidate_media(*id);
        }
        self.bin_last_offline = offline.clone();
        let media: Vec<(MediaId, String)> =
            self.model.project().media.iter().map(|m| (m.id, m.path.clone())).collect();
        // ширина карточки считается ОДИН раз от ширины панели: внутри
        // wrapped-строки available_width убывает и карточки выходили
        // рваными (п.5 р.8)
        let card_w = ui.available_width().clamp(56.0, PREVIEW_W);
        let card_h = card_w * PREVIEW_H / PREVIEW_W;
        let row_h = card_h + 18.0;
        let mut to_place: Option<MediaId> = None;
        let mut to_remove: Option<MediaId> = None;
        let mut pending = false;
        let mut drag_out: Option<MediaId> = None;

        if self.bin_free {
            // --- свободная раскладка: позиции живут в проекте ------------
            let cols = ((ui.available_width() / (card_w + 6.0)).floor() as usize).max(1);
            for (i, (id, _)) in media.iter().enumerate() {
                self.bin_layout.entry(*id).or_insert_with(|| {
                    // новая карточка встаёт в первую свободную клетку
                    egui::pos2(
                        (i % cols) as f32 * (card_w + 6.0),
                        (i / cols) as f32 * (row_h + 6.0),
                    )
                });
            }
            let rows = (media.len().div_ceil(cols)).max(1);
            let need_h = self
                .bin_layout
                .values()
                .map(|p| p.y + row_h)
                .fold(rows as f32 * (row_h + 6.0), f32::max);
            let (area, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), need_h), egui::Sense::hover());
            for (id, path) in &media {
                let p = self.bin_layout.get(id).copied().unwrap_or_default();
                // позиция клампится по ФАКТИЧЕСКОЙ ширине панели: иначе
                // карточка вылезает вправо, панель растёт под контент и
                // её сепаратор уезжает от «своей» границы (ловил t76)
                let max_x = (area.width() - card_w).max(0.0);
                let rect = egui::Rect::from_min_size(
                    area.min + egui::vec2(p.x.min(max_x), p.y),
                    egui::vec2(card_w, row_h),
                );
                let resp = self.draw_bin_card(
                    ui,
                    *id,
                    path,
                    rect,
                    card_w,
                    card_h,
                    &offline,
                    &mut pending,
                    &mut to_place,
                    &mut to_remove,
                );
                // перетаскивание внутри бина двигает карточку; уход мышью
                // за пределы панели = вынос на таймлайн
                if resp.drag_started()
                    && let Some(pos) = resp.interact_pointer_pos()
                {
                    self.bin_drag = Some(BinDragState {
                        id: *id,
                        grab: pos - rect.min,
                        start: self.bin_layout.get(id).copied().unwrap_or(p),
                        escaped: false,
                    });
                }
                if resp.dragged()
                    && let Some(st) = self.bin_drag.as_mut()
                    && st.id == *id
                    && let Some(pos) = resp.interact_pointer_pos()
                {
                    // Липкий режим «вынос»: как только курсор ушёл за
                    // границу бина, раскладку НЕ трогаем и откатываем
                    // позицию — иначе карточка уезжала даже при удачном
                    // дропе на таймлайн (п.8 р.9). Гистерезис 4 px, чтобы
                    // дрожание у кромки не переключало режим.
                    if st.escaped {
                        drag_out = Some(*id);
                    } else if ui.max_rect().shrink(4.0).contains(pos) {
                        let np = pos - st.grab - area.min.to_vec2();
                        self.bin_layout.insert(*id, egui::pos2(np.x.max(0.0), np.y.max(0.0)));
                    } else {
                        st.escaped = true;
                        let start = st.start;
                        self.bin_layout.insert(*id, start);
                        drag_out = Some(*id);
                    }
                }
                if resp.drag_stopped()
                    && let Some(st) = self.bin_drag.take()
                    && !st.escaped
                    && self.bin_layout.get(id).copied() != Some(st.start)
                {
                    self.dirty = true; // раскладка реально изменилась
                }
            }
        } else {
            // --- сетка ---------------------------------------------------
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for (id, path) in &media {
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(card_w, row_h),
                        egui::Sense::hover(),
                    );
                    let resp = self.draw_bin_card(
                        ui,
                        *id,
                        path,
                        rect,
                        card_w,
                        card_h,
                        &offline,
                        &mut pending,
                        &mut to_place,
                        &mut to_remove,
                    );
                    if resp.dragged() {
                        drag_out = Some(*id);
                    }
                }
            });
        }
        if media.is_empty() {
            ui.weak("пусто — перетащи медиафайлы на окно");
        }
        // вынос карточки на таймлайн (п.5 р.8): дальше её ведёт обычный
        // силуэт-призрак таймлайна, как при дропе файла из системы
        if let Some(id) = drag_out {
            egui::DragAndDrop::set_payload(ui.ctx(), BinDrag(id));
        }
        if pending {
            // тумба грузится — мягкий ретрай, не 60 fps
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(120));
        }
        if let Some(id) = to_place {
            self.place_media_at_cursor(id);
        }
        if let Some(id) = to_remove
            && !self.apply(Command::RemoveMedia { id })
        {
            // ошибка уже в статусе (MediaInUse)
        }
    }

    /// Одна карточка бина: превью со скрабом + длительность и имя.
    #[allow(clippy::too_many_arguments)]
    fn draw_bin_card(
        &mut self,
        ui: &mut egui::Ui,
        id: MediaId,
        path: &str,
        rect: egui::Rect,
        card_w: f32,
        card_h: f32,
        offline: &std::collections::HashSet<MediaId>,
        pending: &mut bool,
        to_place: &mut Option<MediaId>,
        to_remove: &mut Option<MediaId>,
    ) -> egui::Response {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        let is_synth = path.starts_with("synth:");
        let is_off = offline.contains(&id);
        let thumb_rect = egui::Rect::from_min_size(rect.min, egui::vec2(card_w, card_h));
        let resp = ui.interact(
            thumb_rect,
            ui.id().with(("bin-card", id.0)),
            egui::Sense::click_and_drag(),
        );
        let p = ui.painter_at(rect);
        p.rect_filled(thumb_rect, 2.0, egui::Color32::from_rgb(10, 11, 13));
        // аудио-медиа: вместо чёрного квадрата — мини-волноформа из уже
        // построенной peak-пирамиды (п.5 р.9)
        let kinds = self.thumbs.kinds(id);
        let audio_only = kinds == Some((false, true));
        if audio_only {
            self.draw_bin_waveform(&p, thumb_rect, id);
        } else if !is_synth && !is_off {
            let dur = self.thumbs.duration_or_probe(id);
            // момент: hover-x → доля файла; без hover — постер
            let (t, tol) = match (resp.hover_pos(), dur) {
                (Some(pos), Some(d)) if d.0 > 0 => {
                    let frac = ((pos.x - thumb_rect.left()) / thumb_rect.width()).clamp(0.0, 0.999);
                    let step = thumb_source_step(d, BIN_SCRUB_SLOTS * card_w, card_w);
                    (thumb_key_t(0, (frac as f64 * d.0 as f64) as i64, step), step / 2)
                }
                // постер: самый информативный кадр, а не первый (п.4 р.9)
                _ => (
                    self.thumbs.poster_t_or_request(id).unwrap_or(0),
                    THUMB_STEP_LADDER[0] / 2,
                ),
            };
            let key = ThumbKey { media: id, t };
            let tex = self.thumb_texture(ui, key);
            let shown = match &tex {
                Some(t) => Some(t.clone()),
                None => {
                    // Failed НЕ перезаказываем и НЕ держим repaint-цикл
                    if self.thumbs.state(&key) != ThumbState::Failed {
                        self.thumbs.request_near(key, tol);
                        *pending = true;
                    }
                    let last = self.bin_last_good.get(&id).copied();
                    last.and_then(|k| self.thumb_texture(ui, k))
                }
            };
            if tex.is_some() {
                self.bin_last_good.insert(id, key);
            }
            if let Some(t) = shown {
                p.image(
                    t.id(),
                    thumb_rect.shrink(1.0),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            if let (Some(pos), Some(d)) = (resp.hover_pos(), dur)
                && d.0 > 0
            {
                p.line_segment(
                    [
                        egui::pos2(pos.x, thumb_rect.top() + 1.0),
                        egui::pos2(pos.x, thumb_rect.bottom() - 1.0),
                    ],
                    egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 170),
                    ),
                );
            }
        } else {
            p.text(
                thumb_rect.center(),
                egui::Align2::CENTER_CENTER,
                if is_off { crate::icons::WARN } else { "♪" },
                egui::FontId::proportional(18.0),
                egui::Color32::from_gray(120),
            );
        }
        if resp.clicked() {
            *to_place = Some(id);
        }
        resp.clone().context_menu(|ui| {
            if ui.button("Поставить на курсор").clicked() {
                *to_place = Some(id);
                ui.close();
            }
            if ui.button("Убрать из проекта").clicked() {
                *to_remove = Some(id);
                ui.close();
            }
        });
        // подпись: длительность (моно) + имя, под превью
        let label_y = thumb_rect.bottom() + 2.0;
        let dur_txt = self.thumbs.duration(id).map(|d| {
            let s = d.0 / 90_000;
            format!("{}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
        });
        let mut x = rect.left();
        if let Some(d) = dur_txt {
            let g = p.layout_no_wrap(d, egui::FontId::monospace(10.0), egui::Color32::from_gray(120));
            p.galley(egui::pos2(x, label_y), g.clone(), egui::Color32::from_gray(120));
            x += g.size().x + 4.0;
        }
        let label = if is_off { format!("{} {name}", crate::icons::WARN) } else { name };
        let avail = (rect.right() - x).max(10.0);
        let g = p.layout(
            label,
            egui::FontId::proportional(11.0),
            egui::Color32::from_gray(190),
            avail,
        );
        p.galley(egui::pos2(x, label_y), g, egui::Color32::from_gray(190));
        self.hint(resp.clone(), path);
        resp
    }

    /// Мини-волноформа аудио-медиа в карточке (п.5 р.9): один меш из
    /// уже готовой peak-пирамиды — обзор всего файла. Если весь файл ниже
    /// порога тишины, рисуем плоскую линию и значок.
    fn draw_bin_waveform(&self, p: &egui::Painter, rect: egui::Rect, id: MediaId) {
        let peaks = self.peaks.lock().unwrap();
        let Some(levels) = peaks.get(&id) else {
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "♪",
                egui::FontId::proportional(18.0),
                egui::Color32::from_gray(120),
            );
            return;
        };
        let cols = rect.width().max(1.0) as usize;
        // уровень пирамиды под ширину карточки: меньше блоков — грубее
        let li = levels
            .iter()
            .position(|(l, _)| l.len() <= cols.max(1))
            .unwrap_or(levels.len().saturating_sub(1));
        let Some((lch, rch)) = levels.get(li) else {
            return;
        };
        let n = lch.len().max(1);
        let mid = rect.center().y;
        let half = rect.height() * 0.45;
        let mut mesh = egui::Mesh::default();
        let mut loud = false;
        for c in 0..cols {
            let i = c * n / cols.max(1);
            let (lo, hi) = lch.get(i).copied().unwrap_or((0, 0));
            let (rlo, rhi) = rch.get(i).copied().unwrap_or((0, 0));
            let amp_l = lo.unsigned_abs().max(hi.unsigned_abs()) as f32 / 32767.0;
            let amp_r = rlo.unsigned_abs().max(rhi.unsigned_abs()) as f32 / 32767.0;
            if amp_l.max(amp_r) > 0.02 {
                loud = true;
            }
            let x = rect.left() + c as f32;
            for (amp, dir) in [(amp_l, -1.0f32), (amp_r, 1.0)] {
                let hgt = (amp * half).max(0.5);
                mesh.add_colored_rect(
                    egui::Rect::from_min_max(
                        egui::pos2(x, mid.min(mid + dir * hgt)),
                        egui::pos2(x + 1.0, mid.max(mid + dir * hgt)),
                    ),
                    egui::Color32::from_rgb(96, 168, 220),
                );
            }
        }
        p.add(egui::Shape::mesh(mesh));
        if !loud {
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "♪",
                egui::FontId::proportional(16.0),
                egui::Color32::from_gray(120),
            );
        }
    }

    /// Тумба как egui-текстура из ОБЩЕГО кэша приложения (тот же путь,
    /// что у филмстрипа таймлайна): get → load_texture → LRU.
    fn thumb_texture(
        &mut self,
        ui: &egui::Ui,
        key: ThumbKey,
    ) -> Option<egui::TextureHandle> {
        if let Some(t) = self.thumb_textures.get(&key) {
            return Some(t.clone());
        }
        let img = self.thumbs.get(&key)?;
        let tex = ui.ctx().load_texture(
            format!("thumb-{}-{}", key.media.0, key.t),
            egui::ImageData::from((*img).clone()),
            egui::TextureOptions::LINEAR,
        );
        self.thumb_textures.insert(key, tex.clone());
        self.thumb_lru.retain(|k| k != &key);
        self.thumb_lru.push_back(key);
        while self.thumb_lru.len() > crate::thumbs::MAX_READY {
            if let Some(old) = self.thumb_lru.pop_front() {
                self.thumb_textures.remove(&old);
            }
        }
        Some(tex)
    }
}
