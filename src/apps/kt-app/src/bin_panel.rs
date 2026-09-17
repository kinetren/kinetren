//! Медиа-бин (порт SourceAreaWidget + п.11 р.6): карточки с превью,
//! отсмотр «возюканьем» мыши по превью (hover x → момент файла → тумба из
//! общего кэша/sidecar-БД), имя + длительность; клик — поставить на курсор.
//!
//! Уроки CLAUDE.md: repaint при pending-тумбе — request_repaint_after,
//! не безусловный; текстуры — общие thumb_textures/LRU приложения;
//! «last good» на медиа — превью не мигает при недогруженных бакетах.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::thumbs::{THUMB_STEP_LADDER, ThumbKey, ThumbState, thumb_key_t, thumb_source_step};
use eframe::egui;
use kt_model::{Command, MediaId};

const PREVIEW_W: f32 = 128.0;
const PREVIEW_H: f32 = 72.0;
/// Логических позиций скраба на файл (п.14 р.7: было 8 — «слишком мало
/// превьюшек»); тумбы филмстрипа подхватываются near-поиском в БД.
const BIN_SCRUB_SLOTS: f32 = 48.0;

/// Ширина полосы автоскролла у верхней/нижней кромки видимой части
/// колонки (п.1 р.16). Внутри неё драг не «выносит» карточку, а мотает
/// список — как у таймлайна.
const BIN_EDGE_MARGIN: f32 = 28.0;

/// Насколько курсор должен УЙТИ за видимую полосу, чтобы это считалось
/// выносом на таймлайн (п.1 р.17). Тычок в кромку — это «мотай список»,
/// а не «выноси»: пользователь жаловался, что при быстром тычке карточка
/// улетает обратно, а вид остаётся отскролленным. По вертикали запас
/// больше ширины полосы автоскролла, по горизонтали — минимальный
/// (уход вбок однозначно означает вынос).
const BIN_ESCAPE_SLACK: egui::Vec2 = egui::vec2(6.0, 36.0);

/// Скорость автоскролла бина по вертикали: та же кривая, что у зон
/// «прыжка» таймлайна (heat^4.15) — у самой кромки быстро, чуть выше
/// почти незаметно. Возвращает СМЕЩЕНИЕ СОДЕРЖИМОГО в точках за кадр:
/// вниз (курсор у нижней кромки) — отрицательное, вверх — положительное.
pub fn bin_edge_scroll(view: egui::Rect, y: f32) -> f32 {
    let margin = BIN_EDGE_MARGIN.min(view.height() * 0.25).max(1.0);
    let heat = if y < view.top() + margin {
        -((view.top() + margin - y) / margin).clamp(0.0, 1.0)
    } else if y > view.bottom() - margin {
        ((y - (view.bottom() - margin)) / margin).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if heat == 0.0 {
        return 0.0;
    }
    // знак: heat>0 (низ) → содержимое едет ВВЕРХ, то есть дельта скролла
    // отрицательная (см. Ui::scroll_with_delta)
    -heat.signum() * (heat.abs().powf(4.15) * 0.18 * 240.0).clamp(1.0, 42.0)
}

/// Сырое вертикальное колесо за кадр в точках (п.1 р.17). Берём
/// `Event::MouseWheel` ДО обработки egui: `smooth_scroll_delta` при
/// активном драге уже обнулён гейтом ScrollArea.
fn raw_wheel_y(ui: &egui::Ui) -> f32 {
    ui.input(|i| {
        i.raw
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::MouseWheel { unit, delta, .. } => Some(match unit {
                    egui::MouseWheelUnit::Point => delta.y,
                    egui::MouseWheelUnit::Line => delta.y * 20.0,
                    egui::MouseWheelUnit::Page => delta.y * 200.0,
                }),
                _ => None,
            })
            .sum()
    })
}

/// Полезная нагрузка перетаскивания карточки из бина (п.5 р.8).
#[derive(Clone, Copy, Debug)]
pub struct BinDrag(pub MediaId);

/// Состояние перетаскивания карточки внутри бина (п.8 р.9).
#[derive(Clone, Debug)]
pub struct BinDragState {
    pub id: MediaId,
    /// Смещение курсора внутри карточки.
    pub grab: egui::Vec2,
    /// Исходная позиция — на неё откатываемся при выносе на таймлайн.
    pub start: egui::Pos2,
    /// Курсор уже покидал бин: дальше это вынос, раскладка не меняется.
    pub escaped: bool,
    /// Кто едет вместе (группа или выделение) и с каких позиций: дельта
    /// применяется ко всем сразу (п.13 р.11).
    pub mates: Vec<(MediaId, egui::Pos2)>,
}

/// Глаголы ленты в бине (Р.55, С6). Отдельный тип, а не строка: меню
/// строится в замыкании, а действовать надо СНАРУЖИ него — иначе `self`
/// занят заимствованием.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ReelVerb {
    Expand,
    Open,
    Rename,
    Remove,
}

impl KtApp {
    /// Индекс группы, в которой состоит карточка.
    pub fn bin_group_of(&self, id: MediaId) -> Option<usize> {
        self.bin_groups.iter().position(|g| g.members.contains(&id))
    }

    /// Кого тянуть вместе с карточкой: всю её группу, а если группы нет —
    /// всё выделение (когда карточка в него входит), иначе её одну.
    pub fn bin_drag_set(&self, id: MediaId) -> Vec<MediaId> {
        if let Some(gi) = self.bin_group_of(id) {
            return self.bin_groups[gi].members.clone();
        }
        if self.bin_sel.contains(&id) {
            return self.bin_sel.iter().copied().collect();
        }
        vec![id]
    }

    /// Заперта ли карточка (замок группы).
    pub fn bin_locked(&self, id: MediaId) -> bool {
        self.bin_group_of(id).is_some_and(|gi| self.bin_groups[gi].locked)
    }

    /// Собрать выделенные карточки в группу (одиночная — тоже группа:
    /// так «замок на карточке» не заводит второй механизм).
    pub fn bin_group_selection(&mut self, name: String) -> Option<usize> {
        let members: Vec<MediaId> = self.bin_sel.iter().copied().collect();
        if members.is_empty() {
            return None;
        }
        // карточка состоит максимум в одной группе
        for m in &members {
            if let Some(gi) = self.bin_group_of(*m) {
                self.bin_groups[gi].members.retain(|x| x != m);
            }
        }
        self.bin_groups.retain(|g| !g.members.is_empty());
        self.bin_groups.push(kt_model::BinGroup { name, members, locked: false });
        self.dirty_ui = true;
        Some(self.bin_groups.len() - 1)
    }

    pub fn bin_ungroup(&mut self, gi: usize) {
        if gi < self.bin_groups.len() {
            self.bin_groups.remove(gi);
            self.dirty_ui = true;
        }
    }

    pub fn draw_bin(&mut self, ui: &mut egui::Ui) {
        // прямоугольники карточек живут ОДИН кадр: карточка могла уехать
        // скроллом, схлопнуться в группу или исчезнуть с медиа. В
        // свободном режиме их считает сам режим (рамки групп рисуются
        // РАНЬШЕ карточек), в сетке — заполняет `draw_bin_card`.
        if !self.bin_free {
            self.bin_card_rects.clear();
        }
        // фон — ПЕРВЫМ: в egui позже зарегистрированная интеракция лежит
        // выше, значит иконки и карточки, добавленные ниже, свои клики и
        // наведение забирают себе, а фону достаётся только мимо них
        self.bin_background_menu(ui);
        // Переключатель режимов — ИКОНКА, не галочка (п.8 р.8: галочек
        // «не к месту» больше не делаем)
        ui.horizontal(|ui| {
            let free = self.bin_free;
            let r = ui
                .add_sized([22.0, 20.0], egui::Button::selectable(free, crate::icons::ARROWS_OUT));
            if self.hint(r, tr(k::bin::HINT_FREE)).clicked() {
                self.bin_free = true;
            }
            let r = ui.add_sized(
                [22.0, 20.0],
                egui::Button::selectable(!free, crate::icons::SQUARES_FOUR),
            );
            if self.hint(r, tr(k::bin::HINT_GRID)).clicked() {
                self.bin_free = false;
            }
        });
        let offline = self.offline.lock().unwrap().clone();
        // offline→online: негативные кэши тумб устарели (п.1 р.7)
        for id in self.bin_last_offline.difference(&offline) {
            self.thumbs.invalidate_media(*id);
        }
        self.bin_last_offline = offline.clone();
        // Карточки бина — файлы, генераторы И ЛЕНТЫ (Р.55, С6).
        //
        // До этого раунда лент в бине не было вовсе: поставить композицию
        // во второе место можно было только копипастом уже стоящего
        // экземпляра, завести пустую — ничем, удалить конкретную — тоже
        // («убрать неиспользованные» убирает все сразу). Претензия
        // владельца дословно: «наверное еще куча вещей недоступна из ui».
        //
        // Различает их НАБОР АДРЕСОВ, а не префикс строки: псевдо-пути
        // разбирает только `kt_media::factory` (сноска п.211), а у ленты
        // `MediaRef`-а нет НАМЕРЕННО (п.378) — заведи мы ей путь, watchdog
        // покрасил бы её офлайном, а служба индекса полезла бы строить
        // пики ffmpeg'ом.
        let reels: std::collections::HashSet<MediaId> =
            self.model.document().sequences.iter().map(|s| s.id).collect();
        let media: Vec<(MediaId, String)> = self
            .model
            .project()
            .media
            .iter()
            .map(|m| (m.id, m.path.clone()))
            .chain(self.model.document().sequences.iter().map(|s| (s.id, s.name.clone())))
            .collect();
        // ширина карточки считается ОДИН раз от ширины панели: внутри
        // wrapped-строки available_width убывает и карточки выходили
        // рваными (п.5 р.8)
        // размер карточки НЕ раздуваем (жалоба р.13): широкая колонка даёт
        // больше карточек в ряд, а не гигантские превью
        let card_w = ui.available_width().clamp(56.0, PREVIEW_W);
        let card_h = card_w * PREVIEW_H / PREVIEW_W;
        let row_h = card_h + 18.0;
        let mut to_place: Option<MediaId> = None;
        let mut to_measure: Option<MediaId> = None;
        let mut to_gen: Option<MediaId> = None;
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
            // фон бина ловит рамку выделения; карточки регистрируются
            // ПОЗЖЕ и потому выигрывают хит-тест (п.13 р.11)
            let (area, bg) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), need_h),
                egui::Sense::click_and_drag(),
            );
            let add_mode =
                ui.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
            if bg.drag_started()
                && let Some(pos) = bg.interact_pointer_pos()
            {
                if !add_mode {
                    self.bin_sel.clear();
                }
                self.bin_rubber = Some(pos);
            }
            if bg.clicked() && !add_mode {
                self.bin_sel.clear();
            }
            let mut card_rects = std::mem::take(&mut self.bin_card_rects);
            card_rects.clear();
            // прямоугольники считаем ЗАРАНЕЕ: рамки групп рисуются ДО
            // карточек и потому лежат под ними (жалоба р.13)
            for (id, _) in &media {
                let p = self.bin_layout.get(id).copied().unwrap_or_default();
                let max_x = (area.width() - card_w).max(0.0);
                card_rects.push((
                    *id,
                    egui::Rect::from_min_size(
                        area.min + egui::vec2(p.x.min(max_x), p.y),
                        egui::vec2(card_w, row_h),
                    ),
                ));
            }
            self.draw_bin_groups(ui, &card_rects);
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
                    &reels,
                    &mut pending,
                    &mut to_place,
                    &mut to_measure,
                    &mut to_gen,
                    &mut to_remove,
                );
                // перетаскивание внутри бина двигает карточку (и всю её
                // группу/выделение); уход мышью за пределы панели = вынос
                if resp.drag_started()
                    && let Some(pos) = resp.interact_pointer_pos()
                    && !self.bin_locked(*id)
                {
                    let mates = self
                        .bin_drag_set(*id)
                        .into_iter()
                        .map(|m| (m, self.bin_layout.get(&m).copied().unwrap_or_default()))
                        .collect();
                    self.bin_drag = Some(BinDragState {
                        id: *id,
                        grab: pos - rect.min,
                        start: self.bin_layout.get(id).copied().unwrap_or(p),
                        escaped: false,
                        mates,
                    });
                }
                if resp.dragged()
                    && let Some(st) = self.bin_drag.clone()
                    && st.id == *id
                    && let Some(pos) = resp.interact_pointer_pos()
                {
                    // Липкий режим «вынос»: как только курсор ушёл за
                    // границу бина, раскладку НЕ трогаем и откатываем
                    // позиции — иначе карточки уезжали даже при удачном
                    // дропе на таймлайн (п.8 р.9).
                    //
                    // Граница берётся по clip_rect — ВИДИМОЙ полосе
                    // колонки. `ui.max_rect()` внутри ScrollArea равен
                    // вьюпорту в координатах СОДЕРЖИМОГО и при скролле
                    // уезжает, поэтому по нему выходило, что курсор
                    // «покинул бин» уже у нижней кромки экрана: карточку
                    // нельзя было положить ниже одного экрана, список
                    // никогда не перерастал вьюпорт — и скроллить было
                    // нечего ни колесом, ни как-либо ещё (п.1 р.16).
                    let view = ui.clip_rect();
                    // Колесо ВО ВРЕМЯ драга: egui глушит скролл у
                    // ScrollArea, пока что-нибудь тащат (гейт
                    // `dragged_id().is_none()`), а человек крутит колесо
                    // именно тогда — чтобы дотянуться до нужного места
                    // (жалоба «scroll мышки по прежнему не работает»,
                    // п.1 р.17). Читаем сырые события и мотаем сами.
                    let wheel = raw_wheel_y(ui);
                    if wheel != 0.0 {
                        ui.scroll_with_delta_animation(
                            egui::vec2(0.0, wheel),
                            egui::style::ScrollAnimation::none(),
                        );
                    }
                    if !st.escaped {
                        // у кромок — мотаем список, а не выносим карточку;
                        // scroll_with_delta идёт мимо гейта колеса egui
                        // (тот глушит колесо на время любого драга)
                        let dy = bin_edge_scroll(view, pos.y);
                        if dy != 0.0 {
                            ui.scroll_with_delta_animation(
                                egui::vec2(0.0, dy),
                                egui::style::ScrollAnimation::none(),
                            );
                            // курсор может стоять на месте — кадры всё
                            // равно должны идти, иначе скролл замирает
                            ui.ctx().request_repaint();
                        }
                    }
                    // «Вынос» засчитываем, только если курсор УШЁЛ за
                    // полосу с запасом (п.1 р.17): тычок в кромку — это
                    // «мотай список», и карточка обязана остаться там,
                    // куда её ведут, а не улететь на старое место.
                    if st.escaped {
                        drag_out = Some(*id);
                    } else if view.expand2(BIN_ESCAPE_SLACK).contains(pos) {
                        let np = pos - st.grab - area.min.to_vec2();
                        let delta = egui::pos2(np.x.max(0.0), np.y.max(0.0)) - st.start;
                        for (m, from) in &st.mates {
                            let to = *from + delta;
                            self.bin_layout.insert(*m, egui::pos2(to.x.max(0.0), to.y.max(0.0)));
                        }
                    } else {
                        if let Some(st) = self.bin_drag.as_mut() {
                            st.escaped = true;
                        }
                        for (m, from) in &st.mates {
                            self.bin_layout.insert(*m, *from);
                        }
                        drag_out = Some(*id);
                    }
                }
                if resp.drag_stopped()
                    && let Some(st) = self.bin_drag.take()
                    && !st.escaped
                    && self.bin_layout.get(id).copied() != Some(st.start)
                {
                    self.dirty_ui = true; // раскладка — удобство, не содержимое
                }
            }

            // рамка выделения: рисуем и по отпусканию забираем карточки
            if let Some(anchor) = self.bin_rubber {
                let cur = ui.ctx().pointer_latest_pos().unwrap_or(anchor);
                let band = egui::Rect::from_two_pos(anchor, cur);
                ui.painter().rect_filled(
                    band,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(90, 140, 220, 40),
                );
                ui.painter().rect_stroke(
                    band,
                    0.0,
                    egui::Stroke::new(1.0, crate::theme::ACCENT),
                    egui::StrokeKind::Middle,
                );
                if !ui.ctx().input(|i| i.pointer.primary_down()) {
                    for (id, r) in &card_rects {
                        if band.intersects(*r) {
                            self.bin_sel.insert(*id);
                        }
                    }
                    self.bin_rubber = None;
                }
            }

            self.draw_bin_selection(ui, &card_rects);
            self.draw_bin_group_name_edit(ui);
            self.bin_card_rects = card_rects;
        } else {
            // --- сетка ---------------------------------------------------
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for (id, path) in &media {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(card_w, row_h), egui::Sense::hover());
                    let resp = self.draw_bin_card(
                        ui,
                        *id,
                        path,
                        rect,
                        card_w,
                        card_h,
                        &offline,
                        &reels,
                        &mut pending,
                        &mut to_place,
                        &mut to_measure,
                        &mut to_gen,
                        &mut to_remove,
                    );
                    if resp.dragged() {
                        drag_out = Some(*id);
                    }
                }
            });
        }
        if media.is_empty() {
            ui.weak(tr(k::bin::EMPTY));
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
        if let Some(id) = to_measure {
            self.measure_loudness(id);
        }
        if let Some(id) = to_gen {
            self.open_generator_editor(id);
        }
        if let Some(id) = to_place {
            self.place_media_at_cursor(id);
        }
        // Свободная полоса внизу: место, куда всегда можно бросить файл и
        // куда всегда можно нажать правой кнопкой. Без неё «фон бина» на
        // плотной сетке физически отсутствует — карточки заняли всё.
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::hover());
        self.bin_rect = ui.min_rect();
        if let Some(id) = to_remove
            && self.apply(Command::RemoveMedia { id })
        {
            // раскладка и группы жили отдельно от медиа и текли: чистим
            // их вместе с удалением (п.13 р.11)
            self.bin_layout.remove(&id);
            self.bin_sel.remove(&id);
            let alive: Vec<MediaId> = self.model.project().media.iter().map(|m| m.id).collect();
            kt_model::prune_bin_groups(&mut self.bin_groups, &alive);
            self.dirty_ui = true;
        }
    }

    /// Правый клик по СВОБОДНОМУ ФОНУ бина (Р.49, п.365).
    ///
    /// Раскладка переключалась только двумя иконками наверху панели — и
    /// человек, привыкший к правой кнопке на таймлайне, на фоне бина
    /// получал молчание. Пункты — те же два действия, что у иконок:
    /// новых ключей ноль.
    ///
    /// Интеракция регистрируется ПЕРВОЙ, до иконок и карточек: в egui
    /// позже зарегистрированное лежит ВЫШЕ, и всё, во что человек целится,
    /// забирает клик себе, а фону достаётся только мимо. Граница берётся по
    /// `clip_rect`, а не `max_rect`: внутри `ScrollArea` второй равен
    /// вьюпорту в координатах СОДЕРЖИМОГО и при скролле уезжает (п.118).
    fn bin_background_menu(&mut self, ui: &mut egui::Ui) {
        // Прямоугольник ПРОШЛОГО кадра: свою высоту бин узнаёт, только
        // дорисовав карточки, а интеракция обязана регистрироваться ДО них.
        // На первом кадре его ещё нет — меню появляется со второго, и это
        // незаметно (первый кадр окна человек не успевает щёлкнуть).
        // `ui.clip_rect()` тут не годится: внутри `ScrollArea` он равен
        // вьюпорту КОЛОНКИ, а ниже бина живёт перф-панель — её правый клик
        // уходил бы в меню бина.
        let view = self.bin_rect.intersect(ui.clip_rect());
        if !view.is_positive() {
            return;
        }
        let resp = ui.interact(view, ui.id().with("bin-bg"), egui::Sense::click());
        let mut free: Option<bool> = None;
        let mut picked = false;
        let mut new_gen: Option<kt_model::GeneratorKind> = None;
        let mut new_reel = false;
        resp.context_menu(|ui| {
            picked = true;
            if ui.button(tr(k::bin::HINT_FREE)).clicked() {
                free = Some(true);
                ui.close();
            }
            if ui.button(tr(k::bin::HINT_GRID)).clicked() {
                free = Some(false);
                ui.close();
            }
            ui.separator();
            // генераторы (Р.52) — ТО ЖЕ ТЕЛО, что в главном меню
            // (правило 17): второго списка видов не бывает
            if let Some(kd) = crate::generator_editor::generator_menu(ui) {
                new_gen = Some(kd);
            }
            // ПУСТАЯ ЛЕНТА (Р.55, С6). До этого раунда `AddSequence`
            // конструировалась только внутри сворачивания и отвязывания:
            // завести композицию с нуля было нечем.
            let r = ui.button(tr(k::bin::menu::NEW_REEL));
            crate::widgets::field_probe::note("bin.new_reel", r.rect);
            if r.clicked() {
                new_reel = true;
                ui.close();
            }
        });
        if picked {
            self.set_panel_target(crate::ctx_menu::PanelTarget::BinEmpty);
        }
        if let Some(kd) = new_gen {
            self.create_generator(kd);
        }
        if new_reel {
            self.create_empty_reel();
        }
        if let Some(v) = free {
            self.bin_free = v;
            self.dirty_ui = true;
        }
    }

    /// Рамки групп: объединение прямоугольников участников, имя — В
    /// РАЗРЫВЕ верхней линии (лишней высоты не занимает), замок — глифом.
    /// Плюс подсветка выделенных карточек.
    /// Подсветка выделенных карточек — рисуется ПОСЛЕ них (тонкая рамка
    /// вокруг, её видно), в отличие от рамок групп (те уходят ПОД карточки).
    fn draw_bin_selection(&mut self, ui: &mut egui::Ui, cards: &[(MediaId, egui::Rect)]) {
        let painter = ui.painter().clone();
        for (id, r) in cards {
            if self.bin_sel.contains(id) {
                painter.rect_stroke(
                    r.expand(1.0),
                    2.0,
                    egui::Stroke::new(1.0, crate::theme::ACCENT),
                    egui::StrokeKind::Middle,
                );
            }
        }
    }

    /// Рамки групп: рисуются ДО карточек, поэтому лежат ПОД ними — линия
    /// поверх превью выглядела нелепо (жалоба р.13).
    fn draw_bin_groups(&mut self, ui: &mut egui::Ui, cards: &[(MediaId, egui::Rect)]) {
        let painter = ui.painter().clone();
        let at = |id: MediaId| cards.iter().find(|(m, _)| *m == id).map(|(_, r)| *r);
        let mut rename: Option<usize> = None;
        for gi in 0..self.bin_groups.len() {
            let g = self.bin_groups[gi].clone();
            let mut frame: Option<egui::Rect> = None;
            for m in &g.members {
                if let Some(r) = at(*m) {
                    frame = Some(frame.map_or(r, |f: egui::Rect| f.union(r)));
                }
            }
            let Some(frame) = frame.map(|f| f.expand(4.0)) else { continue };
            let col = if g.locked {
                egui::Color32::from_rgb(150, 130, 90)
            } else {
                egui::Color32::from_rgb(90, 120, 165)
            };
            let stroke = egui::Stroke::new(1.0, col);
            // рамка без верхней линии — её нарисуем с разрывом под имя
            for seg in [
                [frame.left_bottom(), frame.right_bottom()],
                [frame.left_top(), frame.left_bottom()],
                [frame.right_top(), frame.right_bottom()],
            ] {
                painter.line_segment(seg, stroke);
            }
            let title = if g.locked {
                format!("{} {}", crate::icons::LOCK, g.name)
            } else {
                g.name.clone()
            };
            let title = title.trim().to_string();
            if title.is_empty() {
                painter.line_segment([frame.left_top(), frame.right_top()], stroke);
            } else {
                let galley = painter.layout_no_wrap(title, egui::FontId::proportional(10.5), col);
                let w = galley.size().x.min(frame.width() - 12.0).max(0.0);
                let x0 = frame.left() + 6.0;
                painter.line_segment([frame.left_top(), egui::pos2(x0 - 3.0, frame.top())], stroke);
                painter.line_segment(
                    [egui::pos2(x0 + w + 3.0, frame.top()), frame.right_top()],
                    stroke,
                );
                painter.galley(egui::pos2(x0, frame.top() - galley.size().y * 0.5), galley, col);
            }
            // клик по РАМКЕ (не по карточке): выделить группу, ПКМ — меню
            let bar = egui::Rect::from_min_max(
                frame.left_top() - egui::vec2(0.0, 5.0),
                egui::pos2(frame.right(), frame.top() + 2.0),
            );
            let r = ui.interact(bar, ui.id().with(("bin-group", gi)), egui::Sense::click());
            let r = self.hint(r, tr(k::bin::HINT_GROUP));
            if r.clicked() {
                self.bin_sel = g.members.iter().copied().collect();
            }
            r.context_menu(|ui| {
                if ui.button(tr(k::bin::menu::RENAME)).clicked() {
                    rename = Some(gi);
                    ui.close();
                }
                let lock_label =
                    if g.locked { tr(k::bin::menu::UNLOCK) } else { tr(k::bin::menu::LOCK) };
                if ui.button(lock_label).clicked() {
                    self.bin_groups[gi].locked = !g.locked;
                    self.dirty_ui = true;
                    ui.close();
                }
                if ui.button(tr(k::bin::menu::UNGROUP)).clicked() {
                    self.bin_ungroup(gi);
                    ui.close();
                }
            });
        }
        if let Some(gi) = rename {
            let name = self.bin_groups[gi].name.clone();
            self.bin_group_edit = Some((gi, name));
            self.bin_group_focus = true;
        }
    }

    /// Инлайн-ввод имени группы (поверх всего, как прочие инлайн-поля).
    fn draw_bin_group_name_edit(&mut self, ui: &mut egui::Ui) {
        if let Some((gi, buf)) = self.bin_group_edit.clone() {
            let mut buf = buf;
            let mut done: Option<bool> = None;
            egui::Area::new(egui::Id::new("bin-group-name"))
                .order(egui::Order::Foreground)
                .fixed_pos(ui.max_rect().left_top() + egui::vec2(8.0, 8.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut buf)
                                .hint_text(tr(k::bin::GROUP_NAME))
                                .desired_width(150.0),
                        );
                        // фокус запрашиваем РОВНО один раз: иначе поле его
                        // никогда не «теряет», lost_focus не наступает и
                        // Enter не сохраняет имя (жалоба р.13)
                        if !r.has_focus() && self.bin_group_focus {
                            r.request_focus();
                            self.bin_group_focus = false;
                        }
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            done = Some(true);
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            done = Some(false);
                        }
                    });
                });
            match done {
                Some(true) => {
                    if gi < self.bin_groups.len() {
                        self.bin_groups[gi].name = buf.trim().to_string();
                        self.dirty_ui = true;
                    }
                    self.bin_group_edit = None;
                }
                Some(false) => self.bin_group_edit = None,
                None => self.bin_group_edit = Some((gi, buf)),
            }
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
        // Адреса ЛЕНТ: у них нет ни файла, ни тумб, ни громкости, и
        // глаголы у них свои (Р.55, С6).
        reels: &std::collections::HashSet<MediaId>,
        pending: &mut bool,
        to_place: &mut Option<MediaId>,
        to_measure: &mut Option<MediaId>,
        to_gen: &mut Option<MediaId>,
        to_remove: &mut Option<MediaId>,
    ) -> egui::Response {
        let is_reel = reels.contains(&id);
        let gen_kind = if is_reel { None } else { kt_model::parse_gen_path(path) };
        // подпись генератора — имя вида, не сырой gen:-путь (правило 7);
        // у ленты подпись — её собственное имя, и путь ей не положен
        let name = if is_reel {
            path.to_string()
        } else {
            match &gen_kind {
                Some(kd) => crate::generator_editor::kind_name(kd).to_string(),
                None => std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string()),
            }
        };
        let is_synth = path.starts_with("synth:");
        let is_off = offline.contains(&id);
        let thumb_rect = egui::Rect::from_min_size(rect.min, egui::vec2(card_w, card_h));
        if !self.bin_free {
            // в сетке прямоугольники не считает никто другой, а щупать
            // карточку мышью надо и тесту, и живому сценарию (Р.53, п.416)
            self.bin_card_rects.push((id, thumb_rect));
        }
        if gen_kind.is_some() {
            // карточка генератора — наружу для живых сценариев (Р.52)
            crate::widgets::field_probe::note("bin.gen", thumb_rect);
        }
        let resp = ui.interact(
            thumb_rect,
            ui.id().with(("bin-card", id.0)),
            egui::Sense::click_and_drag(),
        );
        let p = ui.painter_at(rect);
        p.rect_filled(thumb_rect, 2.0, egui::Color32::from_rgb(10, 11, 13));
        // аудио-медиа: вместо чёрного квадрата — мини-волноформа из уже
        // построенной peak-пирамиды (п.5 р.9)
        if is_reel {
            // МИНИ-КАРТА вместо тумбы: филмстрипа у ленты нет и быть не
            // может — своего файла у неё нет, а тумбы содержимого были бы
            // декодом на скролле (инвариант 8, п.395). Тот же приём, что
            // на самом событии-вложении.
            crate::widgets::field_probe::note("bin.reel", thumb_rect);
            if let Some(seq) = self.model.document().sequence(id) {
                let end = seq.end().0.max(1);
                let lanes = seq.video_lanes.max(1) + seq.audio_lanes.max(1);
                let bar_h = (thumb_rect.height() - 4.0) / lanes as f32;
                let mut mesh = egui::Mesh::default();
                for ch in &seq.clips {
                    let row = match ch.kind {
                        kt_model::ClipKind::Video => ch.lane,
                        kt_model::ClipKind::Audio => seq.video_lanes.max(1) + ch.lane,
                    };
                    let y = thumb_rect.top() + 2.0 + row as f32 * bar_h;
                    let x0 =
                        thumb_rect.left() + thumb_rect.width() * ch.start.0 as f32 / end as f32;
                    let x1 = thumb_rect.left()
                        + thumb_rect.width() * (ch.start + ch.duration).0 as f32 / end as f32;
                    mesh.add_colored_rect(
                        egui::Rect::from_min_max(
                            egui::pos2(x0, y),
                            egui::pos2(x1.max(x0 + 1.0), y + (bar_h - 1.0).max(1.0)),
                        ),
                        crate::theme::BAND_EDGE.gamma_multiply(0.7),
                    );
                }
                p.add(egui::Shape::mesh(mesh));
                // БЕЙДЖ ×N — только когда мест НЕСКОЛЬКО (то же правило,
                // что на событии, t458): «×1» было бы шумом
                let uses = self.model.document().sequence_uses(id);
                if uses > 1 {
                    p.text(
                        thumb_rect.right_top() + egui::vec2(-4.0, 2.0),
                        egui::Align2::RIGHT_TOP,
                        format!("×{uses}"),
                        egui::FontId::proportional(11.0),
                        crate::theme::BAND_EDGE,
                    );
                }
            }
        }
        let kinds = if is_reel { None } else { self.thumbs.kinds(id) };
        let audio_only = kinds == Some((false, true));
        if is_reel {
            // всё уже нарисовано выше
        } else if audio_only {
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
                _ => (self.thumbs.poster_t_or_request(id).unwrap_or(0), THUMB_STEP_LADDER[0] / 2),
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
        // Ctrl/Shift+клик — набор выделения, обычный клик — как раньше
        // (поставить на курсор), заодно делая карточку единственной
        // выделенной: иначе «выделил рамкой, кликнул мимо — а выделение
        // осталось» (п.13 р.11)
        // Второй клик двойного НЕ ставит второй клип (Р.53, п.416): у
        // egui `clicked()` истинно на обоих, и двойной клик по карточке
        // давал ДВА события внахлёст. Теперь он даёт один клип (от
        // первого клика) и открывает окно параметров — тот же глагол
        // «открыть», что у пункта меню и у двойного клика по событию
        // (правило 17).
        if resp.clicked() && !resp.double_clicked() {
            let add = ui.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
            if add {
                if !self.bin_sel.remove(&id) {
                    self.bin_sel.insert(id);
                }
            } else {
                self.bin_sel.clear();
                self.bin_sel.insert(id);
                *to_place = Some(id);
            }
        }
        if resp.double_clicked() && gen_kind.is_some() {
            *to_gen = Some(id);
        }
        let sel_n = self.bin_sel.len();
        let group_of = self.bin_group_of(id);
        let mut group_now = false;
        let mut ungroup: Option<usize> = None;
        let mut lock_toggle: Option<usize> = None;
        let mut reel_verb: Option<ReelVerb> = None;
        resp.clone().context_menu(|ui| {
            let r = ui.button(tr(k::bin::menu::PLACE));
            crate::widgets::field_probe::note("bin.place", r.rect);
            if r.clicked() {
                *to_place = Some(id);
                ui.close();
            }
            // ГЛАГОЛЫ ЛЕНТЫ (Р.55, С6). Раньше их не было ни одного: лента
            // в бине не показывалась вовсе, и «создать можно, управлять
            // нельзя» относилось к ней целиком.
            if is_reel {
                for (key, verb) in [
                    (k::tl::menu::NEST_EXPAND_HERE, ReelVerb::Expand),
                    (k::tl::menu::NEST_ENTER, ReelVerb::Open),
                    (k::tl::menu::NEST_RENAME, ReelVerb::Rename),
                ] {
                    let r = ui.button(tr(key));
                    crate::widgets::field_probe::note(
                        match verb {
                            ReelVerb::Expand => "bin.reel.expand",
                            ReelVerb::Open => "bin.reel.open",
                            ReelVerb::Rename => "bin.reel.rename",
                            ReelVerb::Remove => "bin.reel.remove",
                        },
                        r.rect,
                    );
                    if r.clicked() {
                        reel_verb = Some(verb);
                        ui.close();
                    }
                }
                ui.separator();
                let r = ui.button(tr(k::bin::menu::REMOVE));
                crate::widgets::field_probe::note("bin.reel.remove", r.rect);
                if r.clicked() {
                    reel_verb = Some(ReelVerb::Remove);
                    ui.close();
                }
                return;
            }
            if gen_kind.is_some() {
                // у генератора вместо замера громкости (мерить нечего:
                // аудио нет) — его окно параметров; тот же пункт, что в
                // меню события на таймлайне (правило 17)
                if ui.button(tr(k::tl::menu::GEN_PARAMS)).clicked() {
                    *to_gen = Some(id);
                    ui.close();
                }
            } else if ui.button(tr(k::bin::menu::MEASURE_LOUDNESS)).clicked() {
                *to_measure = Some(id);
                ui.close();
            }
            ui.separator();
            match group_of {
                Some(gi) => {
                    if ui.button(tr(k::bin::menu::UNGROUP)).clicked() {
                        ungroup = Some(gi);
                        ui.close();
                    }
                    if ui.button(tr(k::bin::menu::LOCK_TOGGLE)).clicked() {
                        lock_toggle = Some(gi);
                        ui.close();
                    }
                }
                None => {
                    let label = if sel_n > 1 {
                        crate::tf!(k::bin::menu::GROUP_SELECTED, sel_n)
                    } else {
                        tr(k::bin::menu::GROUP_ONE).to_string()
                    };
                    if ui.button(label).clicked() {
                        group_now = true;
                        ui.close();
                    }
                }
            }
            ui.separator();
            if ui.button(tr(k::bin::menu::REMOVE)).clicked() {
                *to_remove = Some(id);
                ui.close();
            }
        });
        if let Some(v) = reel_verb {
            self.reel_verb_from_bin(id, v);
        }
        if group_now {
            if self.bin_sel.is_empty() {
                self.bin_sel.insert(id);
            }
            if let Some(gi) = self.bin_group_selection(String::new()) {
                self.bin_group_edit = Some((gi, String::new()));
                self.bin_group_focus = true;
            }
        }
        if let Some(gi) = ungroup {
            self.bin_ungroup(gi);
        }
        if let Some(gi) = lock_toggle
            && gi < self.bin_groups.len()
        {
            self.bin_groups[gi].locked = !self.bin_groups[gi].locked;
            self.dirty_ui = true;
        }
        // подпись: длительность (моно) + имя, под превью
        let label_y = thumb_rect.bottom() + 2.0;
        // у генератора длительности НЕТ (duration == 0) — не писать ни
        // «0:00:00», ни «1000:00:00» (минус F-TXT-003: бин обязан уметь
        // случай «длины нет»)
        let dur_txt = self.thumbs.duration(id).filter(|d| d.0 > 0).map(|d| {
            let s = d.0 / 90_000;
            format!("{}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
        });
        // «×N» — сколько РАЗ медиа стоит на монтаже (Р.53, п.416). У
        // генератора параметры общие на все места, и правка цвета меняет
        // картинку сразу везде: число обязано быть видно ДО правки, а не
        // только в окне. Место не новое — это пустующий слот длительности
        // (правило 13), поэтому и показываем только там, где он пуст.
        let uses_txt = (dur_txt.is_none())
            .then(|| self.media_uses().get(&id).copied().unwrap_or(0))
            .filter(|n| *n > 0)
            .map(|n| format!("×{n}"));
        let mut x = rect.left();
        if let Some(d) = dur_txt.or(uses_txt) {
            let g =
                p.layout_no_wrap(d, egui::FontId::monospace(10.0), egui::Color32::from_gray(120));
            p.galley(egui::pos2(x, label_y), g.clone(), egui::Color32::from_gray(120));
            x += g.size().x + 4.0;
        }
        let label = if is_off { format!("{} {name}", crate::icons::WARN) } else { name };
        let avail = (rect.right() - x).max(10.0);
        let g =
            p.layout(label, egui::FontId::proportional(11.0), egui::Color32::from_gray(190), avail);
        p.galley(egui::pos2(x, label_y), g, egui::Color32::from_gray(190));
        // Громкость материала — В ПОДСКАЗКЕ, без новой полосы в карточке
        // (Р.49, п.373). Показ НИЧЕГО НЕ ЗАПУСКАЕТ: провести мышью по бину
        // значило бы начать декод десятка файлов — прямая родня инварианта 8
        // («скролл/зум не порождает декод-запросов»). Пока не мерили, так и
        // сказано: «ещё не измерена», а не выдуманный ноль.
        let loud = match self.loudness.lock().unwrap().get(&id) {
            Some((lufs, dbtp, _)) => {
                crate::tf!(k::bin::LOUDNESS, format!("{lufs:.1}"), format!("{dbtp:.1}"))
            }
            None => tr(k::bin::LOUDNESS_PENDING).to_string(),
        };
        self.hint(resp.clone(), &format!("{path}\n{loud}"));
        resp
    }

    /// Мини-волноформа аудио-медиа в карточке (п.5 р.9): один меш из
    /// уже готовой peak-пирамиды — обзор всего файла. Если весь файл ниже
    /// порога тишины, рисуем плоскую линию и значок.
    /// Исполнить глагол ленты, выбранный в бине (Р.55, С6).
    ///
    /// Каждый глагол — ТО ЖЕ ТЕЛО, что у пункта меню события (правило 17):
    /// второго способа раскрыть, войти, переименовать или убрать ленту
    /// заводить нельзя, иначе они разойдутся.
    fn reel_verb_from_bin(&mut self, seq: MediaId, verb: ReelVerb) {
        // «раскрыть» и «войти» действуют НАД ЭКЗЕМПЛЯРОМ: раскрытие — это
        // вид вокруг конкретного места, а не свойство ленты. Нет ни одного
        // места — сказать об этом, а не молчать.
        let inst = self.model.project().clips.iter().find(|c| c.media == seq).map(|c| c.id);
        match verb {
            ReelVerb::Expand | ReelVerb::Open => {
                let Some(inst) = inst else {
                    self.warn(tr(k::msg::NEST_NOT_PLACED));
                    return;
                };
                if verb == ReelVerb::Expand {
                    self.expand_in_place(inst);
                } else {
                    self.enter_nest(inst);
                }
            }
            ReelVerb::Rename => {
                let name =
                    self.model.document().sequence(seq).map(|s| s.name.clone()).unwrap_or_default();
                self.seq_name = Some(crate::app::SeqNameDialog { seq, name, focus: true });
            }
            ReelVerb::Remove => {
                // АДРЕСНОЕ удаление: раньше была только кнопка «убрать
                // неиспользованные», то есть все сразу. Занятая лента
                // отказывается уходить сама (t429) — отказ с причиной, а
                // не молчание.
                if self.apply(Command::RemoveSequence { id: seq }) {
                    self.note(tr(k::msg::NEST_REMOVED));
                }
            }
        }
    }

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
    fn thumb_texture(&mut self, ui: &egui::Ui, key: ThumbKey) -> Option<egui::TextureHandle> {
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
