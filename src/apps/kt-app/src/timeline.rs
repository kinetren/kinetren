//! Таймлайн: отрисовка и интеракции. Паритет со старым Qt-таймлайном
//! (см. docs/porting-notes.md и план UI-паритета): маркеры, регион,
//! volume rail, стерео-волноформы, цвет клипа из хэша пути, офлайн-тинт,
//! edge-autoscroll, контекст-меню, снап к маркерам/региону/клипам.

use crate::i18n::{k, tr};
use crate::app::KtApp;
use eframe::egui;
use kt_cache::BurstKind;
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, Tick};

// Линейка = полоски кэша + ОДНА рабочая строка (п.6 р.11 + п.3 р.12).
// Сначала под метки была заведена отдельная полоса 13 px — и она пустовала
// у любого проекта без меток. Теперь имя метки пишется на той же строке,
// что и подписи времени (янтарным, поверх цифры): места не тратим, а на
// полоски кэша ничего не налезает.
pub const STRIPES_H: f32 = 11.0; // 4+1+4+2: глобальные полоски кэша
pub const TIME_H: f32 = 22.0; // риски, цифры времени, флажки и имена меток
pub const TIME_TOP: f32 = STRIPES_H;
pub const RULER_H: f32 = STRIPES_H + TIME_H;
pub const GUTTER_W: f32 = 34.0; // п.20 р.6: поуже
/// Предел ярлыка длинного имени дорожки (п.10 р.11): дальше — многоточие,
/// полное имя показывает подсказка.
pub const LANE_LABEL_MAX_W: f32 = 160.0;
/// Высота строки, которую занимает имя КЛИПА на его плашке. Ярлык дорожки
/// пишется строкой НИЖЕ (п.2 р.13): заезжать на дорожку можно, но буквы
/// поверх букв — каша, которую пользователь и не принял.
pub const CLIP_NAME_ROW_H: f32 = 19.0;

/// Y ярлыка дорожки: строкой ниже имени клипа, но не ниже дна дорожки.
/// Чистая функция ради теста 102 — «под именем клипа» проверяется числом,
/// а не на глаз.
pub fn lane_label_y(row_top: f32, lane_h: f32, galley_h: f32) -> f32 {
    let row_bottom = row_top + lane_h - 2.0;
    (row_top + CLIP_NAME_ROW_H).min(row_bottom - galley_h).max(row_top)
}
/// Единые пределы зума (п.17 р.6): 2e6 тиков/px = 22 с/px — 30-минутный
/// проект целиком в окне ~80 px... на всю ширину ≈ 1.5 px/с и мельче.
pub const TPP_MIN: f64 = 30.0;
pub const TPP_MAX: f64 = 2_000_000.0;
/// Лестница шагов руллера, тики: 1с/10с/1м/10м/1ч.
const RULER_LADDER: [i64; 5] = [90_000, 900_000, 5_400_000, 54_000_000, 324_000_000];
pub const EDGE_GRAB_PX: f32 = 7.0;
pub const SNAP_PX: f64 = 10.0;
pub const REGION_EDGE_PX: f32 = 8.0;
pub const VOLUME_RAIL_W: f32 = 9.0;
/// ±6.02 dB на весь ход рейла: gain = 2^((0.5−t)·2)  (порт старого).
pub const RAIL_DB_SPAN: f32 = 12.0412;

#[derive(Clone, Copy, PartialEq)]
pub enum DragMode {
    Move,
    TrimStart,
    TrimEnd,
}

/// Итог Move-драга (Р.24, п.139). Чистая функция — тест 139.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum MovePlan {
    /// Коллизий нет — обычный MoveClip.
    Free,
    /// Мульти-наезд/поглощение/наезд меньше минимума окна: упор в стык
    /// (решение пользователя п.10 Р.24 — чужой монтаж не разрушаем молча).
    Clamped { start: i64 },
    /// Частичный наезд на ОДНОГО соседа: жест кроссфейда — клипы встык до
    /// `joint` (середина наезда с клампами тел), окно перехода = наезд.
    Crossfade { outgoing: ClipId, incoming: ClipId, joint: i64, in_off: i64, out_off: i64 },
}

pub(crate) fn plan_move(p: &Project, id: ClipId, want: i64, lane: u32) -> MovePlan {
    let Some(me) = p.clips.iter().find(|c| c.id == id) else { return MovePlan::Free };
    let dur = me.duration.0;
    let (w0, w1) = (want, want + dur);
    let group = me.link_group;
    let hits: Vec<&ClipEvent> = p
        .clips
        .iter()
        .filter(|c| {
            c.id != id
                && c.kind == me.kind
                && c.lane == lane
                && !(group.is_some() && c.link_group == group)
                && c.start.0 < w1
                && c.start.0 + c.duration.0 > w0
        })
        .collect();
    let foreign: Vec<(i64, i64)> = p
        .foreign
        .iter()
        .filter(|f| f.kind == me.kind && f.lane == lane)
        .map(|f| (f.start.0, (f.start + f.duration).0))
        .filter(|(s, e)| *s < w1 && *e > w0)
        .collect();
    if hits.is_empty() && foreign.is_empty() {
        return MovePlan::Free;
    }
    // Переслаивание (Р.28): клипы можно класть друг на друга — жест НИЧЕГО
    // не режет и ничего не «доправляет». Требование пользователя: «сколько
    // было столько и должно остаться… НИЧЕГО НИГДЕ НЕ ДОЛЖНО ОТРАСТАТЬ/
    // ОТРУБАТЬСЯ». Кроссфейд слышен из самого перекрытия (веса v1), поэтому
    // ни тримов, ни TransitionEvent тут не нужно. Чужие плейсхолдеры
    // накладывать по-прежнему нельзя — их сырьё пишется обратно в OTIO, где
    // дорожка строго последовательна.
    if p.overlap_layers && foreign.is_empty() {
        return MovePlan::Free;
    }
    // кроссфейд — только одиночный частичный наезд на СВОЕЙ дорожке
    // (вертикальный перенос с коллизией не режет чужие клипы тримами)
    if hits.len() == 1 && foreign.is_empty() && lane == me.lane {
        let n = hits[0];
        let (n0, n1) = (n.start.0, n.start.0 + n.duration.0);
        let mid = |ov0: i64, ov1: i64, lo: i64, hi: i64| -> Option<i64> {
            (lo <= hi).then(|| (ov0 + (ov1 - ov0) / 2).clamp(lo, hi))
        };
        if w0 > n0 && w1 > n1 && n1 - w0 >= MIN_TRANSITION_DURATION.0 {
            // мы incoming: наше начало в его теле, наезд [w0, n1)
            if let Some(j) = mid(w0, n1, n0 + MIN_CLIP_DURATION.0, w1 - MIN_CLIP_DURATION.0) {
                return MovePlan::Crossfade {
                    outgoing: n.id,
                    incoming: id,
                    joint: j,
                    in_off: j - w0,
                    out_off: n1 - j,
                };
            }
        } else if w0 < n0 && w1 < n1 && w1 - n0 >= MIN_TRANSITION_DURATION.0 {
            // мы outgoing: наш конец в его теле, наезд [n0, w1)
            if let Some(j) = mid(n0, w1, w0 + MIN_CLIP_DURATION.0, n1 - MIN_CLIP_DURATION.0) {
                return MovePlan::Crossfade {
                    outgoing: id,
                    incoming: n.id,
                    joint: j,
                    in_off: j - n0,
                    out_off: w1 - j,
                };
            }
        }
    }
    // упор: откатываемся к ближайшему стыку по направлению движения
    let orig = me.start.0;
    let spans =
        hits.iter().map(|c| (c.start.0, c.start.0 + c.duration.0)).chain(foreign.iter().copied());
    let start = if want >= orig {
        spans.map(|(s, _)| s - dur).min().unwrap_or(want).clamp(orig.min(want), want)
    } else {
        spans.map(|(_, e)| e).max().unwrap_or(want).clamp(want, orig)
    };
    MovePlan::Clamped { start }
}

/// Прямоугольник окна перехода в строке дорожки. Чистая функция — тест 140.
pub(crate) fn transition_rect(x0: f32, x1: f32, lane_top: f32, lane_h: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(x0, lane_top + 1.0),
        egui::pos2(x1, lane_top + lane_h - 3.0),
    )
}

/// «Ручки» в окне перехода (Р.26): слева от стыка играет продолжение
/// ВХОДЯЩЕГО клипа (его материал до собственного начала), справа — хвост
/// уходящего. Ровно это и возвращается при растаскивании. Чистая функция —
/// тест 160.
pub(crate) fn transition_handle_rects(
    x0: f32,
    xj: f32,
    x1: f32,
    lane_top: f32,
    lane_h: f32,
) -> (egui::Rect, egui::Rect) {
    let (top, bot) = (lane_top + 1.0, lane_top + lane_h - 3.0);
    (
        egui::Rect::from_min_max(egui::pos2(x0, top), egui::pos2(xj, bot)),
        egui::Rect::from_min_max(egui::pos2(xj, top), egui::pos2(x1, bot)),
    )
}

/// Лента диапазонной метки на строке подписей времени (Р.24, п.142).
/// Чистая функция — тест 142.
pub(crate) fn marker_band_rect(x0: f32, x1: f32, time_top: f32) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(x0, time_top + 9.0), egui::pos2(x1, time_top + 13.0))
}

pub struct DragState {
    pub id: ClipId,
    pub mode: DragMode,
    /// Тики от начала клипа до точки захвата (для Move).
    pub grab_dt: i64,
    pub orig: ClipEvent,
    /// Текущее (превью) положение.
    pub cur_start: i64,
    pub cur_end: i64,
    pub cur_lane: u32,
    pub moved: bool,
}

/// Все виды перетаскиваний на таймлайне.
pub enum Drag {
    Clip(DragState),
    /// Перетаскивание заголовка СТРОКИ дорожек (реордер; п.6 v1.4 —
    /// строки в display-порядке, перемешивание V/A/M разрешено).
    LaneRow {
        from: usize,
        cur: usize,
    },
    /// Скраб курсора (захват треугольника playhead в руллере).
    Scrub,
    /// Выделение региона по пустому месту.
    Region {
        anchor: i64,
    },
    /// Перетаскивание границы региона (true = start).
    RegionEdge {
        start_edge: bool,
    },
    /// Перетаскивание маркера: (исходный тик, текущий).
    Marker {
        orig: i64,
        cur: i64,
    },
    /// Край диапазонной метки (Р.24, п.142): правый (left=false) меняет
    /// длительность, левый двигает начало при стоящем конце (батч).
    MarkerEdge {
        start: i64,
        left: bool,
        cur: i64,
    },
    /// Край окна перехода (true = левый/in_offset): live через
    /// apply_coalesced(SetTransition) — один undo на жест (Р.24, п.140).
    TransitionEdge {
        id: TransitionId,
        left: bool,
        gesture: u64,
    },
    /// Roll стыка перехода: превью-позиция, батч тримов на release.
    TransitionBody {
        id: TransitionId,
        orig_at: i64,
        cur_at: i64,
    },
    /// Volume rail аудио-клипа: live-громкость, SetGain на отпускании.
    Volume {
        id: ClipId,
        moved: bool,
    },
    /// Перенос morph-блока по времени (H1; в v1 блок был недвижим).
    Morph {
        id: MorphId,
        grab_dt: i64,
        cur_start: i64,
        moved: bool,
    },
}

/// Цель контекст-меню, захваченная на right-click.
#[derive(Clone, Copy, PartialEq)]
pub enum CtxTarget {
    Clip(ClipId),
    Marker(i64),
    /// Правый клик у границы региона (тик границы).
    RegionEdge(i64),
    /// Правый клик по заголовку дорожки (п.8).
    Lane {
        kind: ClipKind,
        lane: u32,
    },
    /// Правый клик по morph-блоку (H1).
    Morpher(MorphId),
    /// Правый клик по заголовку morph-дорожки (H1).
    MorphLane(u32),
    Empty,
}

/// Цвет клипа из хэша пути (порт colorFromPath: r=80+h%120 ...).
pub fn clip_color(path: &str, kind: ClipKind) -> egui::Color32 {
    let mut h: u64 = 1469598103934665603;
    let suffix = match kind {
        ClipKind::Video => ":v",
        ClipKind::Audio => ":a",
    };
    for b in path.bytes().chain(suffix.bytes()) {
        h = h.wrapping_mul(131).wrapping_add(b as u64);
    }
    egui::Color32::from_rgb(
        (80 + h % 120) as u8,
        (80 + (h / 7) % 120) as u8,
        (80 + (h / 17) % 120) as u8,
    )
}

/// Вертикальная раскладка дорожек (п.12 р.6, тест 61): сжатие ПАНЕЛИ
/// поджимает высоту строк как раньше (до 30), но рост ЧИСЛА дорожек
/// строки не плющит — появляется вертикальный скролл. Явный v_zoom
/// (Shift+Ctrl+колесо) задаёт высоту строки напрямую.
pub struct VLayout {
    pub lane_h: f32,
    pub content_h: f32,
    pub scroll_max: f32,
}

pub fn vertical_layout(avail_h: f32, lanes: usize, v_zoom: Option<f32>) -> VLayout {
    let lanes = lanes.max(1) as f32;
    let usable = (avail_h - RULER_H - 6.0).max(0.0);
    let lane_h = match v_zoom {
        Some(z) => z.clamp(16.0, 120.0),
        None => (usable / lanes).clamp(30.0, 84.0),
    };
    let content_h = lanes * lane_h;
    VLayout { lane_h, content_h, scroll_max: (content_h - usable).max(0.0) }
}

/// Шаги руллера для зума (п.17 р.6, тест 60): минорные риски — минимальная
/// ступень лестницы с >= 7 px, подписи — с >= 70 px. Раньше цикл шёл ПО
/// СЕКУНДАМ и на глубоком зум-ауте давал тысячи шейпов за кадр.
pub fn ruler_steps(tpp: f64) -> (i64, i64) {
    let minor = RULER_LADDER
        .iter()
        .copied()
        .find(|&st| st as f64 / tpp >= 7.0)
        .unwrap_or(RULER_LADDER[RULER_LADDER.len() - 1]);
    let label = RULER_LADDER
        .iter()
        .copied()
        .find(|&st| st >= minor && st as f64 / tpp >= 70.0)
        .unwrap_or(RULER_LADDER[RULER_LADDER.len() - 1]);
    (minor, label)
}

/// Подпись времени руллера: мм:сс до часа, ч:мм:сс дальше (тест 60).
pub fn format_ruler_time(ticks: i64) -> String {
    let s = ticks / 90_000;
    if s < 3600 {
        format!("{}:{:02}", s / 60, s % 60)
    } else {
        format!("{}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
    }
}

/// Порог снапа, тики (п.17 р.6, тест 60): пропорционален зуму БЕЗ верхнего
/// клампа — прежний потолок 9000 тиков (0.1 c) на глубоком зум-ауте делал
/// снап мёртвым (0.005 px при tpp=2e6).
pub fn snap_threshold(tpp: f64) -> i64 {
    ((SNAP_PX * tpp) as i64).max(1_080)
}

/// Ближайшая магнитная цель, если она в пределах порога.
pub fn snap_point(tick: i64, targets: &[i64], threshold: i64) -> Option<i64> {
    targets
        .iter()
        .copied()
        .map(|t| (t, (t - tick).abs()))
        .filter(|(_, d)| *d <= threshold)
        .min_by_key(|(t, d)| (*d, *t))
        .map(|(t, _)| t)
}

/// Перенос клипа: магнитим НАЧАЛО либо КОНЕЦ, побеждает меньшее смещение.
/// Возвращает новое начало и цель для жёлтой линии.
///
/// Дефект п.2 р.11: раньше кандидаты сравнивались уже ПОСЛЕ подстановки
/// «не нашлось = оставить как есть», и такой кандидат имел смещение 0 —
/// то есть всегда побеждал, а линия при этом рисовалась. Снап срабатывал
/// только при одновременном попадании обоих краёв, и пользователь видел
/// «линия есть, магнита нет».
pub fn snap_move(raw: i64, dur: i64, targets: &[i64], threshold: i64) -> (i64, Option<i64>) {
    let by_start = snap_point(raw, targets, threshold);
    let by_end = snap_point(raw + dur, targets, threshold);
    match (by_start, by_end) {
        (Some(s), Some(e)) => {
            if (s - raw).abs() <= (e - dur - raw).abs() {
                (s, Some(s))
            } else {
                (e - dur, Some(e))
            }
        }
        (Some(s), None) => (s, Some(s)),
        (None, Some(e)) => (e - dur, Some(e)),
        (None, None) => (raw, None),
    }
}

/// Зоны клипа по ширине (п.17 р.6, тест 60): у узких клипов (< 3 захватов)
/// трим-зоны сжимаются до w/4 (мин 2 px) — раньше клип уже 14 px целиком
/// состоял из трим-зон и его нельзя было перенести.
pub fn edge_grab_for_width(w: f32) -> f32 {
    if w >= 3.0 * EDGE_GRAB_PX { EDGE_GRAB_PX } else { (w / 4.0).max(2.0) }
}

/// Возврат окна к playhead при игре (п.14 р.6, тест 59): только в
/// follow-режиме (Play включает, ручной пан/зум выключает), не во время
/// драга; окно возвращается, когда курсор вышел из зоны [2%, 85%].
pub fn follow_view(
    follow: bool,
    playing: bool,
    dragging: bool,
    ph: f64,
    view_start: f64,
    vis: f64,
) -> Option<f64> {
    if !follow || !playing || dragging {
        return None;
    }
    if ph < view_start + vis * 0.02 || ph > view_start + vis * 0.85 {
        Some(ph - vis * 0.15)
    } else {
        None
    }
}

/// Авто-восстановление follow-режима (п.30 р.7): ручной скролл гасит
/// follow, но если играет loop/регион, регион ЦЕЛИКОМ видим в окне, курсор
/// внутри него и юзер перестал крутить (grace-пауза) — следование
/// включается обратно само.
pub fn follow_rearm(
    playing: bool,
    since_scroll_s: f32,
    grace_s: f32,
    ph: f64,
    region: Option<(f64, f64)>,
    view_start: f64,
    vis: f64,
) -> bool {
    let Some((rs, re)) = region.filter(|(s, e)| e > s) else {
        return false;
    };
    playing
        && since_scroll_s >= grace_s
        && rs >= view_start
        && re <= view_start + vis
        && ph >= rs
        && ph < re
}

impl KtApp {
    /// Дамп состояния для живых сценариев (Р.27, `KT_GEOM_DUMP=<файл>`):
    /// окно, зона дорожек, зум и вертикальные позиции строк — чтобы скрипт
    /// целился мышью точно, а не по эвристике «примерно треть снизу», плюс
    /// позиция курсора и признак игры — чтобы сценарии про воспроизведение
    /// (loop, заворот) читали правду из программы, а не гадали по картинке. Пишется не чаще 10 Гц и только при
    /// заданном env: в обычном запуске ветка стоит один `Option`-чек.
    fn dump_geom(&mut self, geom: &TlGeom, screen: egui::Rect) {
        let Some(path) = self.geom_dump_path.clone() else { return };
        let now = std::time::Instant::now();
        if self.geom_dumped_at.is_some_and(|t| now.duration_since(t).as_millis() < 100) {
            return;
        }
        self.geom_dumped_at = Some(now);
        let rows: Vec<String> = geom
            .rows
            .iter()
            .enumerate()
            .map(|(i, (kind, lane))| {
                format!(
                    "{{\"kind\":\"{}\",\"lane\":{lane},\"top\":{:.1},\"center\":{:.1}}}",
                    match kind {
                        kt_model::LaneKind::Video => "video",
                        kt_model::LaneKind::Audio => "audio",
                        kt_model::LaneKind::Morph => "morph",
                    },
                    geom.row_top(i),
                    geom.row_top(i) + geom.lane_h * 0.5,
                )
            })
            .collect();
        let json = format!(
            "{{\"playhead\":{},\"playing\":{},\
             \"window\":{{\"w\":{:.1},\"h\":{:.1}}},\
             \"timeline\":{{\"left\":{:.1},\"right\":{:.1},\"top\":{:.1},\"bottom\":{:.1}}},\
             \"lane_h\":{:.1},\"ticks_per_px\":{:.3},\"view_start\":{:.1},\"rows\":[{}]}}\n",
            self.player.video_playhead().0,
            self.player.transport.is_playing(),
            screen.width(),
            screen.height(),
            geom.rect.left(),
            geom.rect.right(),
            geom.rect.top(),
            geom.rect.bottom(),
            geom.lane_h,
            geom.tpp,
            geom.view_start,
            rows.join(",")
        );
        let tmp = format!("{path}.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// Геометрия таймлайна последнего кадра — маппинг позиций ВНЕ
/// draw_timeline (дроп файлов п.2, произвольный порядок дорожек п.6 v1.4).
/// Строки — display-порядок (LaneKind, lane); чистые функции — тесты 47/50.
#[derive(Clone)]
pub struct TlGeom {
    /// Зона отрисовки дорожек (правее gutter, включая руллер сверху).
    pub rect: egui::Rect,
    pub lane_h: f32,
    /// Display-порядок строк сверху вниз (нормализован).
    pub rows: std::sync::Arc<Vec<(LaneKind, u32)>>,
    /// Вертикальный скролл-офсет зоны дорожек (п.12 р.6).
    pub view_top: f32,
    pub view_start: f64,
    pub tpp: f64,
}

impl TlGeom {
    pub fn x_to_tick(&self, x: f32) -> i64 {
        (self.view_start + (x - self.rect.left()) as f64 * self.tpp) as i64
    }

    pub fn tick_to_x(&self, t: i64) -> f32 {
        self.rect.left() + ((t as f64 - self.view_start) / self.tpp) as f32
    }

    fn lanes_top(&self) -> f32 {
        self.rect.top() + RULER_H + 3.0 - self.view_top
    }

    /// Индекс строки под y (в пределах rows).
    pub fn row_at_y(&self, y: f32) -> Option<usize> {
        let rel = y - self.lanes_top();
        if rel < 0.0 {
            return None;
        }
        let idx = (rel / self.lane_h).floor() as usize;
        (idx < self.rows.len()).then_some(idx)
    }

    pub fn key_at_y(&self, y: f32) -> Option<(LaneKind, u32)> {
        self.rows.get(self.row_at_y(y)?).copied()
    }

    /// Дорожка КЛИПОВ под y (morph-строка/руллер → None).
    pub fn lane_at_y(&self, y: f32) -> Option<(ClipKind, u32)> {
        match self.key_at_y(y)? {
            (LaneKind::Video, l) => Some((ClipKind::Video, l)),
            (LaneKind::Audio, l) => Some((ClipKind::Audio, l)),
            (LaneKind::Morph, _) => None,
        }
    }

    pub fn morph_lane_at_y(&self, y: f32) -> Option<u32> {
        match self.key_at_y(y)? {
            (LaneKind::Morph, l) => Some(l),
            _ => None,
        }
    }

    pub fn row_of(&self, key: (LaneKind, u32)) -> Option<usize> {
        self.rows.iter().position(|&k| k == key)
    }

    pub fn row_top(&self, row: usize) -> f32 {
        self.lanes_top() + row as f32 * self.lane_h
    }

    pub fn lane_top(&self, kind: ClipKind, lane: u32) -> f32 {
        let key = match kind {
            ClipKind::Video => (LaneKind::Video, lane),
            ClipKind::Audio => (LaneKind::Audio, lane),
        };
        self.row_top(self.row_of(key).unwrap_or(0))
    }

    pub fn morph_lane_top(&self, lane: u32) -> f32 {
        self.row_top(self.row_of((LaneKind::Morph, lane)).unwrap_or(0))
    }

    /// Ближайшая к строке `row` дорожка вида `kind` (drag клипа через
    /// чужие строки «прилипает» к своему виду, п.6 v1.4, тест 50).
    pub fn nearest_lane(&self, kind: ClipKind, row: i64) -> u32 {
        let want = match kind {
            ClipKind::Video => LaneKind::Video,
            ClipKind::Audio => LaneKind::Audio,
        };
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, e)| e.0 == want)
            .min_by_key(|(i, _)| (*i as i64 - row).abs())
            .map(|(_, e)| e.1)
            .unwrap_or(0)
    }

    /// Точка внутри полосы дорожек (не gutter/не руллер)?
    pub fn in_lanes(&self, pos: egui::Pos2) -> bool {
        self.rect.contains(pos) && pos.y > self.rect.top() + RULER_H
    }
}

impl KtApp {
    /// Магнитные цели: границы чужих клипов, маркеры, границы региона,
    /// playhead, ноль (порт snapTimeMs).
    ///
    /// Исключается не только сам клип, но и вся его link-группа: A/V-партнёр
    /// стоит на тех же границах и притягивал клип обратно в точку старта
    /// (п.2 р.11).
    pub fn snap_targets(&self, exclude: Option<ClipId>) -> Vec<i64> {
        let p = self.model.project();
        let group = exclude
            .and_then(|id| p.clips.iter().find(|c| c.id == id))
            .and_then(|c| c.link_group);
        let mut t: Vec<i64> = vec![0, self.player.transport.playhead().0];
        for c in &p.clips {
            let same_group = group.is_some() && c.link_group == group;
            if Some(c.id) != exclude && !same_group {
                t.push(c.start.0);
                t.push(c.start.0 + c.duration.0);
            }
        }
        for m in &p.markers {
            t.push(m.t.0);
            if m.is_range() {
                t.push(m.end().0);
            }
        }
        if let Some(r) = p.region {
            t.push(r.start.0);
            t.push(r.end.0);
        }
        t
    }

    pub fn snap(&mut self, tick: i64, exclude: Option<ClipId>) -> i64 {
        if self.snap_off {
            return tick;
        }
        // v1 snapTimeMs: порог 10 px, но не меньше 12 мс и не больше 100 мс
        let threshold = snap_threshold(self.ticks_per_px);
        let targets = self.snap_targets(exclude);
        match snap_point(tick, &targets, threshold) {
            Some(t) => {
                self.snap_line = Some(t);
                t
            }
            None => tick,
        }
    }

    pub fn lane_counts(&self) -> (u32, u32) {
        let p = self.model.project();
        (p.video_lanes.max(1), p.audio_lanes.max(1))
    }
    pub fn draw_timeline(&mut self, ui: &mut egui::Ui) {
        // один проход coverage-опросов на кадр (кэш по epoch, п.1);
        // Arc-снапшот — клон указателя, не карты (п.1 v1.4)
        let coverage_snapshot = self.coverage_runs();
        let full = ui.available_rect_before_wrap();
        let gutter =
            egui::Rect::from_min_max(full.min, egui::pos2(full.left() + GUTTER_W, full.bottom()));
        let rect = egui::Rect::from_min_max(egui::pos2(gutter.right(), full.top()), full.max);
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let painter = ui.painter_at(full);
        painter.rect_filled(full, 0.0, crate::theme::TL_BG);
        // заголовки дорожек интерактивны: драг = реордер (п.7), ПКМ = меню
        // удаления/вставки (п.8)
        let gutter_resp =
            ui.interact(gutter, ui.id().with("lane-gutter"), egui::Sense::click_and_drag());

        // число строк заранее — вертикальному зуму колесом нужно до layout
        let lanes_hint = {
            let (v, a) = self.lane_counts();
            (v + a).max(1) as usize + self.model.project().morph_lanes as usize
        };
        // ---- зум/пан + edge-autoscroll при драге --------------------------
        let mut view_start = self.view_start;
        let mut tpp = self.ticks_per_px;
        // contains_pointer, НЕ hover_pos: во время чужого drag egui глушит
        // hovered у всех остальных виджетов — колесо переставало работать
        // (п.28 р.7); contains_pointer активным драгом не подавляется
        if response.contains_pointer()
            && let Some(pos) = ui.input(|i| i.pointer.hover_pos())
        {
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            // ВАЖНО: egui перекладывает Shift+колесо в scroll.x — для
            // вертикальных жестов (п.12 р.6) читаем СЫРОЕ колесо+модификаторы
            // сырое колесо: MouseWheel-события ДО egui-обработки (Shift
            // перекладывает smooth_scroll в X — а нам нужен вертикальный жест)
            let raw_y: f32 = ui.input(|i| {
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
            });
            let zoom = ui.input(|i| i.zoom_delta());
            let (ctrl, shift) = ui
                .input(|i| (i.modifiers.command || i.modifiers.ctrl, i.modifiers.shift));
            // Shift-вертикаль окно НЕ двигает — follow не гасим (п.30 р.7)
            let user_scrolled = (!shift && scroll != egui::Vec2::ZERO) || zoom != 1.0;
            if user_scrolled {
                // ручное движение окна отключает следование (п.14 р.6)
                self.follow_playhead = false;
                self.last_user_scroll = Some(std::time::Instant::now());
            }
            if shift && ctrl && raw_y != 0.0 {
                // Shift+Ctrl+колесо = вертикальный зум, якорь — строка
                // под курсором (п.12 р.6)
                let cur_h = self
                    .tl_v_zoom
                    .unwrap_or_else(|| {
                        vertical_layout(rect.height(), lanes_hint.max(1), None).lane_h
                    });
                let factor = if raw_y > 0.0 { 1.15 } else { 1.0 / 1.15 };
                let new_h = (cur_h * factor).clamp(16.0, 120.0);
                let rel = pos.y - (rect.top() + RULER_H + 3.0) + self.tl_scroll;
                let row = (rel / cur_h).max(0.0);
                self.tl_v_zoom = Some(new_h);
                self.tl_scroll =
                    (row * new_h - (pos.y - (rect.top() + RULER_H + 3.0))).max(0.0);
            } else if shift && raw_y != 0.0 {
                // Shift+колесо = вертикальный скролл (кламп ниже, по layout)
                self.tl_scroll = (self.tl_scroll - raw_y).max(0.0);
            } else if ctrl && scroll.y != 0.0 || zoom != 1.0 {
                // зум: Ctrl+колесо или пинч, якорь под курсором (порт старого)
                let factor = if zoom != 1.0 {
                    1.0 / zoom as f64
                } else if scroll.y > 0.0 {
                    0.8
                } else {
                    1.25
                };
                let anchor = view_start + (pos.x - rect.left()) as f64 * tpp;
                tpp = (tpp * factor).clamp(TPP_MIN, TPP_MAX);
                view_start = anchor - (pos.x - rect.left()) as f64 * tpp;
            } else if !shift && (scroll.x != 0.0 || scroll.y != 0.0) {
                // голое колесо (любая ось) = скролл по таймлайну (порт старого)
                let delta = if scroll.x.abs() > scroll.y.abs() { scroll.x } else { scroll.y };
                view_start -= delta as f64 * tpp * 2.5;
            }
        }
        // Зоны «прыжка» у краёв (порт): heat = близость к краю [-1..1].
        let edge_margin = (rect.width() / 8.0).max(40.0);
        let edge_heat = |x: f32| -> f32 {
            if x < rect.left() + edge_margin {
                -((rect.left() + edge_margin - x) / edge_margin).clamp(0.0, 1.0)
            } else if x > rect.right() - edge_margin {
                ((x - (rect.right() - edge_margin)) / edge_margin).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        // при драге — плавный autoscroll (heat^4.15, порт)
        if self.drag.is_some()
            && let Some(pos) = response.hover_pos()
        {
            let heat = edge_heat(pos.x);
            if heat != 0.0 {
                let ms = (heat.abs().powf(4.15) * 0.18 * 240.0).clamp(1.0, 42.0) as f64;
                view_start += heat.signum() as f64 * ms * 90.0;
            }
        }
        // ensureCursorVisible при воспроизведении — ТОЛЬКО в follow-режиме
        // (п.14 р.6): ручной пан/зум отключает следование до нового Play.
        // Авто-восстановление (п.30 р.7): регион видим + курсор в нём +
        // юзер перестал крутить ≥1.5 c → follow сам включается обратно.
        if !self.follow_playhead
            && follow_rearm(
                self.player.transport.is_playing(),
                self.last_user_scroll.map(|t| t.elapsed().as_secs_f32()).unwrap_or(f32::MAX),
                1.5,
                self.player.transport.playhead().0 as f64,
                self.model.project().region.map(|r| (r.start.0 as f64, r.end.0 as f64)),
                view_start,
                rect.width() as f64 * tpp,
            )
        {
            self.follow_playhead = true;
        }
        // подмотка после прыжка по точкам остановки (п.11 р.9): те же
        // зоны, что у follow_view, но без требования «играет» — иначе на
        // паузе Home/End оставляли окно на месте
        if let Some(at) = self.pending_reveal.take()
            && let Some(vs) = follow_view(
                true,
                true,
                false,
                at as f64,
                view_start,
                rect.width() as f64 * tpp,
            )
        {
            view_start = vs;
        }
        if let Some(vs) = follow_view(
            self.follow_playhead,
            self.player.transport.is_playing(),
            self.drag.is_some(),
            self.player.transport.playhead().0 as f64,
            view_start,
            rect.width() as f64 * tpp,
        ) {
            view_start = vs;
        }
        view_start = view_start.max(-(tpp * 40.0));
        self.view_start = view_start;
        self.ticks_per_px = tpp;
        let x_to_tick =
            move |x: f32| -> i64 { (view_start + (x - rect.left()) as f64 * tpp) as i64 };
        let tick_to_x =
            move |t: i64| -> f32 { rect.left() + ((t as f64 - view_start) / tpp) as f32 };

        // ---- глобальные полоски кэша (порт: cyan=аудио, green=видео) --------
        {
            let y0 = rect.top() + 1.0;
            let cyan = egui::Color32::from_rgb(114, 214, 255);
            let green = egui::Color32::from_rgb(72, 168, 96);
            // фоновые подложки
            for (row, _) in [(0.0f32, cyan), (5.0, green)] {
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(full.left(), y0 + row),
                        egui::pos2(rect.right(), y0 + row + 4.0),
                    ),
                    0.0,
                    egui::Color32::from_gray(30),
                );
            }
            let q = self.player.preview_quality();
            for c in &self.model.project().clips {
                let (kind, row, col) = match c.kind {
                    ClipKind::Audio => (BurstKind::Audio, 0.0f32, cyan),
                    ClipKind::Video => (BurstKind::video(Some(q)), 5.0, green),
                };
                let (b0, b1) = match c.kind {
                    ClipKind::Audio => {
                        let f0 = c.source_in.to_audio_frames();
                        let f1 = (c.source_in + c.duration).to_audio_frames();
                        (
                            f0.div_euclid(AUDIO_BURST_FRAMES as i64),
                            (f1 - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1,
                        )
                    }
                    ClipKind::Video => (c.source_in.0, c.source_in.0 + c.duration.0),
                };
                let _ = (kind, b0, b1);
                for run in coverage_snapshot.get(&c.id).map(|v| v.as_slice()).unwrap_or(&[]) {
                    let (t0, t1) = match c.kind {
                        ClipKind::Audio => (
                            c.start.0
                                + Tick::from_audio_frames(
                                    run.start * AUDIO_BURST_FRAMES as i64
                                        - c.source_in.to_audio_frames(),
                                )
                                .0,
                            c.start.0
                                + Tick::from_audio_frames(
                                    run.end * AUDIO_BURST_FRAMES as i64
                                        - c.source_in.to_audio_frames(),
                                )
                                .0,
                        ),
                        ClipKind::Video => (
                            c.start.0 + run.start - c.source_in.0,
                            c.start.0 + run.end - c.source_in.0,
                        ),
                    };
                    // полоска уходит влево под полосу имён, как дорожки
                    // (п.3 р.8) — градиент поверх неё просвечивает
                    let x0 = tick_to_x(t0).max(full.left());
                    let x1 = tick_to_x(t1).min(rect.right());
                    if x1 > x0 {
                        painter.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(x0, y0 + row),
                                egui::pos2(x1, y0 + row + 4.0),
                            ),
                            0.0,
                            col,
                        );
                    }
                }
            }
            // янтарная «голова» loop-региона поверх видео-полоски (порт head)
            if let Some(r) = self.model.project().region
                && self.model.project().loop_enabled
            {
                let head_end = (r.start.0 + 90_000 / 2).min(r.end.0);
                let x0 = tick_to_x(r.start.0).max(full.left());
                let x1 = tick_to_x(head_end).min(rect.right());
                if x1 > x0 {
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(x0, y0 + 5.0),
                            egui::pos2(x1, y0 + 9.0),
                        ),
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(255, 210, 72, 150),
                    );
                }
            }
        }

        // ---- линейка: лестница шагов (п.17 р.6), риски одним мешем ---------
        let (minor_step, label_step) = ruler_steps(tpp);
        let mut ruler_mesh = egui::Mesh::default();
        let tick_col = egui::Color32::from_gray(70);
        let t0 = (x_to_tick(rect.left()).max(0) / minor_step) * minor_step;
        let t1 = x_to_tick(rect.right()) + minor_step;
        let mut t = t0;
        while t <= t1 {
            let x = tick_to_x(t);
            let major = t % label_step == 0;
            ruler_mesh.add_colored_rect(
                egui::Rect::from_min_max(
                    egui::pos2(x, rect.top() + TIME_TOP),
                    egui::pos2(
                        x + 1.0,
                        rect.top() + if major { RULER_H } else { TIME_TOP + 6.0 },
                    ),
                ),
                tick_col,
            );
            if major {
                painter.text(
                    egui::pos2(x + 4.0, rect.top() + TIME_TOP + 1.0),
                    egui::Align2::LEFT_TOP,
                    format_ruler_time(t),
                    egui::FontId::proportional(10.5),
                    egui::Color32::from_gray(150),
                );
            }
            t += minor_step;
        }
        if !ruler_mesh.is_empty() {
            painter.add(egui::Shape::mesh(ruler_mesh));
        }
        painter.line_segment(
            [
                egui::pos2(full.left(), rect.top() + RULER_H),
                egui::pos2(full.right(), rect.top() + RULER_H),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_gray(45)),
        );

        // зона дорожек рисуется с клипом НИЖЕ руллера: при вертикальном
        // скролле (п.12 р.6) контент уезжает под линейку, не поверх неё
        let full_painter = painter.clone();
        let painter = ui.painter_at(egui::Rect::from_min_max(
            egui::pos2(full.left(), rect.top() + RULER_H + 1.0),
            full.max,
        ));

        // ---- дорожки ---------------------------------------------------------
        let (video_lanes, audio_lanes) = self.lane_counts();
        let morph_lanes = self.model.project().morph_lanes;
        let lanes_total = (video_lanes + audio_lanes).max(1) + morph_lanes;
        let vl = vertical_layout(rect.height(), lanes_total as usize, self.tl_v_zoom);
        let lane_h = vl.lane_h;
        // кламп скролла каждый кадр (удаление дорожек/ресайз панели)
        self.tl_scroll = self.tl_scroll.clamp(0.0, vl.scroll_max);
        // display-порядок строк (п.6 v1.4): произвольное перемешивание
        // V/A/M; нормализация каждый кадр — вставки/удаления дорожек
        // подтягиваются автоматически
        self.lane_order = kt_model::normalize_lane_order(
            &self.lane_order,
            video_lanes,
            audio_lanes,
            morph_lanes,
        );
        // кэш геометрии для маппинга вне draw_timeline (дроп п.2, строки п.6)
        let geom = TlGeom {
            rect,
            lane_h,
            rows: std::sync::Arc::new(self.lane_order.clone()),
            view_top: self.tl_scroll,
            view_start: self.view_start,
            tpp: self.ticks_per_px,
        };
        self.tl_geom = Some(geom.clone());
        // KT_GEOM_DUMP=<файл> — геометрия таймлайна наружу для живых
        // сценариев (Р.27): без неё скрипт вынужден УГАДЫВАТЬ, где дорожка и
        // сколько пикселей в секунде, и ломается от любой правки вёрстки.
        // Только по env, не чаще 10 Гц, атомарной заменой файла.
        self.dump_geom(&geom, ui.ctx().content_rect());
        let lane_top = |kind: ClipKind, lane: u32| -> f32 { geom.lane_top(kind, lane) };
        let morph_lane_top = |lane: u32| -> f32 { geom.morph_lane_top(lane) };
        let morph_lane_at_y = |y: f32| -> Option<u32> { geom.morph_lane_at_y(y) };
        // (подпись, центр строки, верх строки) — верх нужен, чтобы длинный
        // ярлык встал ПОД именем клипа, а не поверх него
        let mut gutter_labels: Vec<(String, f32, f32)> = Vec::with_capacity(geom.rows.len());
        for (row, &(kind, lane)) in geom.rows.iter().enumerate() {
            let y = geom.row_top(row);
            // фон дорожки тянется ПОД gutter — градиент ниже «растворяет»
            // номерную полосу в дорожках (п.20 р.6)
            let lane_rect = egui::Rect::from_min_max(
                egui::pos2(full.left(), y),
                egui::pos2(rect.right(), y + lane_h - 2.0),
            );
            // фоны дорожек в тонах старого: видео синеватый, аудио тёплый,
            // morph фиолетовый (темнее — тема р.6)
            let fill = match kind {
                LaneKind::Video => crate::theme::LANE_VIDEO,
                LaneKind::Audio => crate::theme::LANE_AUDIO,
                LaneKind::Morph => crate::theme::LANE_MORPH,
            };
            // подпись: пользовательское имя либо формула V1/A1/M1 (п.2 р.10)
            let label = kt_model::lane_label(&self.lane_names, kind, lane);
            painter.rect_filled(lane_rect, 0.0, fill);
            gutter_labels.push((label, y + lane_h * 0.5, y));
        }
        // хит-тест morph-блока (нужен и press-обработке, и ПКМ ниже)
        let morpher_at = |slf: &KtApp, pos: egui::Pos2| -> Option<MorphId> {
            let lane = geom.morph_lane_at_y(pos.y)?;
            slf.model
                .project()
                .morphers
                .iter()
                .rev()
                .filter(|m| m.lane == lane)
                .find(|m| {
                    let x0 = tick_to_x(m.start.0);
                    let x1 = tick_to_x((m.start + m.duration).0).max(x0 + 8.0);
                    pos.x >= x0 && pos.x <= x1
                })
                .map(|m| m.id)
        };

        // ---- дорожки: реордер драгом и меню (пп. 7-8; п.6 v1.4: строки) ----
        {
            // двойной клик по заголовку дорожки — переименование (п.2 р.10)
            if gutter_resp.double_clicked()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
                && let Some((kind, lane)) = geom.key_at_y(pos.y)
            {
                let cur = kt_model::lane_label(&self.lane_names, kind, lane);
                self.lane_edit = Some(crate::app::LaneEdit { kind, lane, buf: cur });
                self.lane_edit_focus = true;
                self.drag = None;
            }
            // пока идёт ввод имени — драг-реордер выключен, иначе «кликнул
            // в поле — уехала дорожка»
            if self.lane_edit.is_none()
                && gutter_resp.drag_started()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
                && let Some(row) = geom.row_at_y(pos.y)
            {
                self.drag = Some(Drag::LaneRow { from: row, cur: row });
            }
            if gutter_resp.dragged()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
                && let Some(Drag::LaneRow { from, .. }) = self.drag
                && let Some(cur) = geom.row_at_y(pos.y)
            {
                self.drag = Some(Drag::LaneRow { from, cur });
            }
            if gutter_resp.drag_stopped()
                && let Some(Drag::LaneRow { from, cur }) = self.drag.take()
                && from != cur
            {
                self.reorder_rows(from, cur);
            }
            // индикатор перетаскиваемой строки
            if let Some(Drag::LaneRow { cur, .. }) = self.drag {
                let y = geom.row_top(cur);
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(gutter.left(), y),
                        egui::pos2(rect.right(), y + lane_h - 2.0),
                    ),
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(120, 160, 255, 26),
                );
            }
            // ПКМ по заголовку — меню дорожки
            if gutter_resp.secondary_clicked()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
            {
                match geom.key_at_y(pos.y) {
                    Some((LaneKind::Video, lane)) => {
                        self.ctx_target = Some(CtxTarget::Lane { kind: ClipKind::Video, lane });
                    }
                    Some((LaneKind::Audio, lane)) => {
                        self.ctx_target = Some(CtxTarget::Lane { kind: ClipKind::Audio, lane });
                    }
                    Some((LaneKind::Morph, lane)) => {
                        self.ctx_target = Some(CtxTarget::MorphLane(lane));
                    }
                    None => {}
                }
            }
            gutter_resp.context_menu(|ui| {
                if let Some(CtxTarget::Lane { kind, lane }) = self.ctx_target {
                    let n = self
                        .model
                        .project()
                        .clips
                        .iter()
                        .filter(|c| c.kind == kind && c.lane == lane)
                        .count();
                    let name = match kind {
                        ClipKind::Video => format!("V{}", lane + 1),
                        ClipKind::Audio => format!("A{}", lane + 1),
                    };
                    ui.label(egui::RichText::new(crate::tf!(k::tl::lane::TITLE, name)).strong());
                    let del_label = if n > 0 {
                        crate::tf!(k::tl::lane::DELETE_N, n)
                    } else {
                        tr(k::tl::lane::DELETE).to_string()
                    };
                    let lanes_of_kind = match kind {
                        ClipKind::Video => video_lanes,
                        ClipKind::Audio => audio_lanes,
                    };
                    let lk = match kind {
                        ClipKind::Video => LaneKind::Video,
                        ClipKind::Audio => LaneKind::Audio,
                    };
                    if ui.add_enabled(lanes_of_kind > 1, egui::Button::new(del_label)).clicked() {
                        self.delete_lane(kind, lane);
                        kt_model::lane_order_remove(&mut self.lane_order, lk, lane);
                        ui.close();
                    }
                    if ui.button(tr(k::tl::lane::RENAME)).clicked() {
                    let lk = match kind {
                        ClipKind::Video => LaneKind::Video,
                        ClipKind::Audio => LaneKind::Audio,
                    };
                    let cur = kt_model::lane_label(&self.lane_names, lk, lane);
                    self.lane_edit = Some(crate::app::LaneEdit { kind: lk, lane, buf: cur });
                    self.lane_edit_focus = true;
                    ui.close();
                }
                if ui.button(tr(k::tl::lane::INSERT_ABOVE)).clicked() {
                        self.insert_lane_at(lk, lane);
                        ui.close();
                    }
                    if ui.button(tr(k::tl::lane::INSERT_BELOW)).clicked() {
                        self.insert_lane_at(lk, lane + 1);
                        ui.close();
                    }
                }
                if let Some(CtxTarget::MorphLane(lane)) = self.ctx_target {
                    let n =
                        self.model.project().morphers.iter().filter(|m| m.lane == lane).count();
                    ui.label(egui::RichText::new(crate::tf!(k::tl::lane::MORPH_TITLE, lane + 1)).strong());
                    let del_label = if n > 0 {
                        crate::tf!(k::tl::lane::DELETE_MORPH_N, n)
                    } else {
                        tr(k::tl::lane::DELETE).to_string()
                    };
                    if ui.button(del_label).clicked() {
                        if self.apply(Command::RemoveMorphLane { at: lane }) {
                            kt_model::lane_order_remove(
                                &mut self.lane_order,
                                LaneKind::Morph,
                                lane,
                            );
                            kt_model::lane_names_remove(
                                &mut self.lane_names,
                                LaneKind::Morph,
                                lane,
                            );
                        }
                        ui.close();
                    }
                    if ui.button(tr(k::tl::lane::INSERT_MORPH_BELOW)).clicked() {
                        self.insert_lane_at(LaneKind::Morph, lane + 1);
                        ui.close();
                    }
                }
            });
        }

        // ---- регион: заливка ---------------------------------------------------
        if let Some(r) = self.model.project().region {
            let x0 = tick_to_x(r.start.0).max(rect.left());
            let x1 = tick_to_x(r.end.0).min(rect.right());
            if x1 > x0 {
                // v1: центр альфа 14, боковые градиенты к краям альфа 58,
                // ширина фейда min(56, w/2)
                let base = |a: u8| egui::Color32::from_rgba_unmultiplied(84, 124, 255, a);
                let fade_w = ((x1 - x0) / 2.0).clamp(1.0, 56.0);
                let grad_h = |xa: f32, xb: f32, ca: egui::Color32, cb: egui::Color32| {
                    let mut mesh = egui::Mesh::default();
                    let i = mesh.vertices.len() as u32;
                    for (pos, col) in [
                        (egui::pos2(xa, rect.top()), ca),
                        (egui::pos2(xb, rect.top()), cb),
                        (egui::pos2(xb, rect.bottom()), cb),
                        (egui::pos2(xa, rect.bottom()), ca),
                    ] {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos,
                            uv: egui::epaint::WHITE_UV,
                            color: col,
                        });
                    }
                    mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
                    painter.add(egui::Shape::mesh(mesh));
                };
                grad_h(x0, x0 + fade_w, base(58), base(14));
                if x1 - fade_w > x0 + fade_w {
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(x0 + fade_w, rect.top()),
                            egui::pos2(x1 - fade_w, rect.bottom()),
                        ),
                        0.0,
                        base(14),
                    );
                }
                grad_h(x1, x1 - fade_w, base(58), base(14));
                for (edge_t, x) in
                    [(r.start.0, tick_to_x(r.start.0)), (r.end.0, tick_to_x(r.end.0))]
                {
                    let _ = edge_t;
                    if x >= rect.left() && x <= rect.right() {
                        painter.line_segment(
                            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(102, 152, 255)),
                        );
                    }
                }
            }
        }

        // ---- хит-тесты --------------------------------------------------------
        let pointer = response.interact_pointer_pos();
        let hover = response.hover_pos();
        self.snap_line = None;
        // магнит включён всегда; Ctrl его снимает (п.2 р.11)
        self.snap_off = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);

        let marker_at = |app: &KtApp, pos: egui::Pos2| -> Option<i64> {
            if pos.y > rect.top() + RULER_H {
                return None;
            }
            app.model
                .project()
                .markers
                .iter()
                .map(|m| m.t.0)
                .min_by_key(|m| (tick_to_x(*m) - pos.x).abs() as i64)
                .filter(|m| (tick_to_x(*m) - pos.x).abs() <= EDGE_GRAB_PX)
        };
        // конец диапазонной метки (Р.24): хват на той же полосе, что флажки
        let marker_end_at = |app: &KtApp, pos: egui::Pos2| -> Option<i64> {
            if pos.y > rect.top() + RULER_H {
                return None;
            }
            app.model
                .project()
                .markers
                .iter()
                .filter(|m| m.is_range())
                .map(|m| m.t.0)
                .find(|start| {
                    let end = start
                        + app.model.project().markers.iter().find(|m| m.t.0 == *start)
                            .map(|m| m.duration.0)
                            .unwrap_or(0);
                    (tick_to_x(end) - pos.x).abs() <= EDGE_GRAB_PX
                })
        };
        // границы региона хватаются на ВСЕЙ высоте (п.14): линии рисуются
        // на всю высоту и должны быть кликабельны везде
        let region_edge_at = |app: &KtApp, pos: egui::Pos2| -> Option<bool> {
            let r = app.model.project().region?;
            if (tick_to_x(r.start.0) - pos.x).abs() <= REGION_EDGE_PX {
                Some(true)
            } else if (tick_to_x(r.end.0) - pos.x).abs() <= REGION_EDGE_PX {
                Some(false)
            } else {
                None
            }
        };
        #[derive(PartialEq)]
        enum ClipZone {
            Body,
            TrimStart,
            TrimEnd,
            VolumeRail,
        }
        #[derive(PartialEq)]
        enum TransZone {
            EdgeLeft,
            EdgeRight,
            Joint,
        }
        // хват перехода приоритетнее клип-зон: его края лежат В ТЕЛАХ клипов
        let transition_at = |app: &KtApp, pos: egui::Pos2| -> Option<(TransitionId, TransZone)> {
            let p = app.model.project();
            for t in &p.transitions {
                // якорь фейда (Р.25) — его единственный живой конец
                let Some(a) = t.a.or(t.b).and_then(|id| p.clips.iter().find(|c| c.id == id))
                else {
                    continue;
                };
                let top = lane_top(a.kind, a.lane);
                if pos.y < top || pos.y > top + lane_h - 2.0 {
                    continue;
                }
                let Some(joint) = t.joint(p).map(|j| j.0) else { continue };
                let xj = tick_to_x(joint);
                if (pos.x - xj).abs() <= EDGE_GRAB_PX {
                    return Some((t.id, TransZone::Joint));
                }
                if (pos.x - tick_to_x(joint - t.in_offset.0)).abs() <= EDGE_GRAB_PX {
                    return Some((t.id, TransZone::EdgeLeft));
                }
                if (pos.x - tick_to_x(joint + t.out_offset.0)).abs() <= EDGE_GRAB_PX {
                    return Some((t.id, TransZone::EdgeRight));
                }
            }
            None
        };
        let clip_at = |app: &KtApp, pos: egui::Pos2| -> Option<(ClipId, ClipZone)> {
            for c in app.model.project().clips.iter().rev() {
                let top = lane_top(c.kind, c.lane);
                if pos.y < top || pos.y > top + lane_h - 2.0 {
                    continue;
                }
                let x0 = tick_to_x(c.start.0);
                let x1 = tick_to_x(c.start.0 + c.duration.0);
                if pos.x < x0 - 2.0 || pos.x > x1 + 2.0 {
                    continue;
                }
                // у узких клипов трим-зоны сжимаются — середина всегда
                // Body (п.17 р.6, тест 60: клип <14px нельзя было двигать)
                let grab = edge_grab_for_width(x1 - x0);
                let zone = if (pos.x - x0).abs() <= grab {
                    ClipZone::TrimStart
                } else if (pos.x - x1).abs() <= grab {
                    ClipZone::TrimEnd
                } else if c.kind == ClipKind::Audio
                    && x1 - x0 > 40.0
                    && pos.x >= x0 + EDGE_GRAB_PX + 3.0
                    && pos.x <= x0 + EDGE_GRAB_PX + 3.0 + VOLUME_RAIL_W
                {
                    ClipZone::VolumeRail
                } else {
                    ClipZone::Body
                };
                return Some((c.id, zone));
            }
            None
        };

        // курсор-иконка + КОНТЕКСТНАЯ ПОДСКАЗКА зоны (п.1 р.9: у таймлайна
        // не было ни одной подсказки — а это самая большая площадь окна)
        if self.drag.is_none()
            && let Some(pos) = hover
        {
            let mut zone_hint: Option<&str> = None;
            let mut hint_owned: Option<String> = None;
            if marker_at(self, pos).is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                zone_hint = Some(tr(k::tl::hint::MARKER));
            } else if region_edge_at(self, pos).is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                zone_hint = Some(tr(k::tl::hint::REGION_EDGE));
            } else if pos.y <= rect.top() + RULER_H {
                zone_hint = Some(tr(k::tl::hint::RULER));
            } else if pos.x < rect.left() {
                // полное имя дорожки — в подсказке (ярлык может быть усечён)
                if let Some((k, l)) = geom.key_at_y(pos.y) {
                    let name = kt_model::lane_label(&self.lane_names, k, l);
                    hint_owned = Some(crate::tf!(k::tl::hint::LANE_NAMED, name));
                } else {
                    zone_hint = Some(tr(k::tl::hint::LANE));
                }
            } else if let Some((_, tz)) = transition_at(self, pos) {
                ui.ctx().set_cursor_icon(match tz {
                    TransZone::Joint => egui::CursorIcon::Grab,
                    _ => egui::CursorIcon::ResizeHorizontal,
                });
                zone_hint = Some(tr(k::tl::hint::TRANSITION));
            } else {
                match clip_at(self, pos) {
                    Some((_, ClipZone::TrimStart)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        zone_hint = Some(tr(k::tl::hint::TRIM_START));
                    }
                    Some((_, ClipZone::TrimEnd)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        zone_hint = Some(tr(k::tl::hint::TRIM_END));
                    }
                    Some((_, ClipZone::VolumeRail)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        zone_hint = Some(tr(k::tl::hint::VOLUME));
                    }
                    Some((_, ClipZone::Body)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                        zone_hint = Some(
                            tr(k::tl::hint::CLIP),
                        );
                    }
                    None => {
                        zone_hint = Some(
                            tr(k::tl::hint::EMPTY),
                        );
                    }
                }
            }
            if let Some(h) = hint_owned {
                self.hover_hint = Some(h);
            } else if let Some(h) = zone_hint {
                self.hover_hint = Some(h.to_string());
            }
        }

        // right-click: захват цели контекст-меню
        if response.secondary_clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            // строка под кликом нужна и для пустого места: «добавить
            // дорожку» из меню встаёт СРАЗУ ПОД ней (п.5 р.16)
            self.ctx_row = geom.key_at_y(pos.y);
            self.ctx_target = Some(if let Some(m) = marker_at(self, pos) {
                CtxTarget::Marker(m)
            } else if let Some(start_edge) = region_edge_at(self, pos) {
                let r = self.model.project().region.unwrap();
                CtxTarget::RegionEdge(if start_edge { r.start.0 } else { r.end.0 })
            } else if let Some((id, _)) = clip_at(self, pos) {
                self.selected = Some(id);
                self.selected_morph = None;
                CtxTarget::Clip(id)
            } else if let Some(mid) = morpher_at(self, pos) {
                self.selected_morph = Some(mid);
                self.selected = None;
                CtxTarget::Morpher(mid)
            } else {
                CtxTarget::Empty
            });
        }

        // ---- press ------------------------------------------------------------
        // Зоны определяем по точке НАЖАТИЯ (press_origin), а не по текущему
        // указателю: drag_started приходит после сдвига в несколько px, и
        // указатель успевал выползти из узкой трим-зоны — трим превращался
        // в перенос клипа (п.28 «не могу изменить длину события»).
        let press_pos = ui.input(|i| i.pointer.press_origin());
        if (response.drag_started() || response.clicked())
            && let Some(pos) = press_pos.or(pointer)
        {
            if let Some(start) = marker_end_at(self, pos) {
                if response.drag_started() {
                    let end = self
                        .model
                        .project()
                        .markers
                        .iter()
                        .find(|m| m.t.0 == start)
                        .map(|m| m.end().0)
                        .unwrap_or(start);
                    self.drag = Some(Drag::MarkerEdge { start, left: false, cur: end });
                }
            } else if let Some(m) = marker_at(self, pos) {
                if response.drag_started() {
                    self.drag = Some(Drag::Marker { orig: m, cur: m });
                }
            } else if let Some(start_edge) = region_edge_at(self, pos) {
                if response.drag_started() {
                    self.drag = Some(Drag::RegionEdge { start_edge });
                }
            } else if pos.y <= rect.top() + RULER_H {
                // руллер: захват треугольника playhead = скраб; клик =
                // постановка курсора (снап к маркерам, п.29); драг =
                // выделение региона (решение пользователя, п.8).
                // Хит-зона — по НАРИСОВАННОМУ треугольнику (часы показа)
                let head_x = tick_to_x(self.player.video_playhead().0);
                if response.drag_started() && (pos.x - head_x).abs() <= 8.0 {
                    self.drag = Some(Drag::Scrub);
                } else if response.drag_started() {
                    // снап и при СОЗДАНИИ региона (п.13 р.6, тест 59)
                    let anchor = self.snap(x_to_tick(pos.x).max(0), None);
                    self.drag = Some(Drag::Region { anchor });
                } else {
                    let t = self.snap(x_to_tick(pos.x).max(0), None);
                    self.player.seek(Tick(t.max(0)));
                }
            } else if self.pending_morpher.is_some() && morph_lane_at_y(pos.y).is_some() {
                // режим размещения morpher'а (H1): клик по Morph-дорожке =
                // создать событие тут; прочие клики проходят как обычно
                if response.clicked()
                    && let Some(lane) = morph_lane_at_y(pos.y)
                {
                    let at = self.snap(x_to_tick(pos.x).max(0), None);
                    self.commit_morph_placement(lane, Tick(at), None);
                }
            } else if let Some(mid) = morpher_at(self, pos) {
                self.selected_morph = Some(mid);
                self.selected = None;
                if response.double_clicked() {
                    self.open_morph_editor(mid);
                } else if response.drag_started()
                    && let Some(orig) =
                        self.model.project().morphers.iter().find(|m| m.id == mid)
                {
                    self.drag = Some(Drag::Morph {
                        id: mid,
                        grab_dt: x_to_tick(pos.x) - orig.start.0,
                        cur_start: orig.start.0,
                        moved: false,
                    });
                }
            } else if let Some((tid, tz)) = transition_at(self, pos) {
                if response.drag_started() {
                    match tz {
                        TransZone::Joint => {
                            let at = {
                                let p = self.model.project();
                                p.transitions
                                    .iter()
                                    .find(|t| t.id == tid)
                                    .and_then(|t| t.joint(p))
                                    .map(|t| t.0)
                            };
                            if let Some(at) = at {
                                self.drag =
                                    Some(Drag::TransitionBody { id: tid, orig_at: at, cur_at: at });
                            }
                        }
                        TransZone::EdgeLeft | TransZone::EdgeRight => {
                            self.gesture_counter += 1;
                            self.drag = Some(Drag::TransitionEdge {
                                id: tid,
                                left: tz == TransZone::EdgeLeft,
                                gesture: self.gesture_counter,
                            });
                        }
                    }
                } else if response.clicked() {
                    let t = self.snap(x_to_tick(pos.x).max(0), None);
                    self.player.seek(Tick(t.max(0)));
                }
            } else if let Some((id, zone)) = clip_at(self, pos) {
                self.selected = Some(id);
                self.selected_morph = None;
                // клик по СОБЫТИЮ тоже ставит курсор (v1, п.7)
                if response.clicked() {
                    let t = self.snap(x_to_tick(pos.x).max(0), None);
                    self.player.seek(Tick(t.max(0)));
                }
                if response.double_clicked() {
                    // регион на span всей link-группы (порт double-click)
                    let group = self.model.clip(id).and_then(|c| c.link_group);
                    let members: Vec<&ClipEvent> = self
                        .model
                        .project()
                        .clips
                        .iter()
                        .filter(|c| c.id == id || (group.is_some() && c.link_group == group))
                        .collect();
                    let start = members.iter().map(|c| c.start).min().unwrap();
                    let end = members.iter().map(|c| c.start + c.duration).max().unwrap();
                    self.model.set_region(Region::normalized(start, end));
                    self.on_region_changed();
                } else if response.drag_started() {
                    match zone {
                        ClipZone::VolumeRail => {
                            self.drag = Some(Drag::Volume { id, moved: false });
                        }
                        zone => {
                            if let Some(orig) = self.model.clip(id).cloned() {
                                let mode = match zone {
                                    ClipZone::TrimStart => DragMode::TrimStart,
                                    ClipZone::TrimEnd => DragMode::TrimEnd,
                                    _ => DragMode::Move,
                                };
                                self.drag = Some(Drag::Clip(DragState {
                                    id,
                                    mode,
                                    grab_dt: x_to_tick(pos.x) - orig.start.0,
                                    cur_start: orig.start.0,
                                    cur_end: orig.start.0 + orig.duration.0,
                                    cur_lane: orig.lane,
                                    orig,
                                    moved: false,
                                }));
                            }
                        }
                    }
                }
            } else if response.clicked() && edge_heat(pos.x) != 0.0 {
                // клик в зоне прыжка: курсор СТАВИТСЯ (по позиции до сдвига),
                // и вью прыгает — «и то и то» (решение пользователя, п.10)
                let t = self.snap(x_to_tick(pos.x).max(0), None);
                self.player.seek(Tick(t.max(0)));
                let heat = edge_heat(pos.x);
                let vis = rect.width() as f64 * tpp;
                self.view_start = (self.view_start
                    + heat.signum() as f64 * (heat.abs() as f64).powi(2) * vis * 0.6)
                    .max(-(tpp * 40.0));
            } else if response.drag_started() {
                // пустое место: выделение региона (порт RegionSelect)
                // снап и при создании региона с пустого места (п.13 р.6)
                let anchor = self.snap(x_to_tick(pos.x).max(0), None);
                self.drag = Some(Drag::Region { anchor });
            } else {
                self.selected = None;
                let t = self.snap(x_to_tick(pos.x).max(0), None);
                self.player.seek(Tick(t.max(0)));
            }
        }

        // ---- drag move ----------------------------------------------------------
        if response.dragged()
            && let Some(pos) = pointer
        {
            match self.drag.take() {
                Some(Drag::LaneRow { from, cur }) => {
                    // обрабатывается gutter-областью; здесь просто держим
                    self.drag = Some(Drag::LaneRow { from, cur });
                }
                Some(Drag::Clip(mut d)) => {
                    d.moved = true;
                    match d.mode {
                        DragMode::Move => {
                            let raw = x_to_tick(pos.x) - d.grab_dt;
                            let dur = d.orig.duration.0;
                            let (snapped, line) = if self.snap_off {
                                (raw, None)
                            } else {
                                let targets = self.snap_targets(Some(d.id));
                                snap_move(
                                    raw,
                                    dur,
                                    &targets,
                                    snap_threshold(self.ticks_per_px),
                                )
                            };
                            d.cur_start = snapped.max(0);
                            // упор в ноль ломает примагничивание — линию гасим
                            self.snap_line = if d.cur_start == snapped { line } else { None };
                            d.cur_end = d.cur_start + dur;
                            // строки перемешаны (п.6): прилипаем к
                            // ближайшей строке СВОЕГО вида (тест 50)
                            let rel = pos.y - geom.lanes_top();
                            let row = (rel / lane_h).floor().max(0.0) as i64;
                            d.cur_lane = geom.nearest_lane(d.orig.kind, row);
                            // мульти-наезд/поглощение — кламп ещё в превью
                            // (реш. 10 Р.24); одиночный частичный наезд
                            // остаётся как есть (release сделает кроссфейд)
                            if let MovePlan::Clamped { start } =
                                plan_move(self.model.project(), d.id, d.cur_start, d.cur_lane)
                            {
                                if d.cur_lane != d.orig.lane {
                                    // вертикальный перенос сквозь занятое
                                    // не проталкиваем — возврат на строку
                                    d.cur_lane = d.orig.lane;
                                    if let MovePlan::Clamped { start } = plan_move(
                                        self.model.project(),
                                        d.id,
                                        d.cur_start,
                                        d.cur_lane,
                                    ) {
                                        d.cur_start = start;
                                    }
                                } else {
                                    d.cur_start = start;
                                }
                                d.cur_end = d.cur_start + dur;
                                self.snap_line = None;
                                self.hover_hint = Some(tr(k::tl::hint::CLAMPED).to_string());
                            }
                        }
                        DragMode::TrimStart => {
                            let raw = x_to_tick(pos.x);
                            let snapped = self.snap(raw, Some(d.id));
                            let max_start = d.cur_end - MIN_CLIP_DURATION.0;
                            let min_start = d.orig.start.0 - d.orig.source_in.0;
                            d.cur_start = snapped.clamp(min_start, max_start);
                        }
                        DragMode::TrimEnd => {
                            let raw = x_to_tick(pos.x);
                            let snapped = self.snap(raw, Some(d.id));
                            d.cur_end = snapped.max(d.cur_start + MIN_CLIP_DURATION.0);
                        }
                    }
                    self.drag = Some(Drag::Clip(d));
                }
                Some(Drag::Scrub) => {
                    let t = self.snap(x_to_tick(pos.x).max(0), None);
                    self.player.seek(Tick(t.max(0)));
                    self.drag = Some(Drag::Scrub);
                }
                Some(Drag::Region { anchor }) => {
                    let cur = self.snap(x_to_tick(pos.x).max(0), None);
                    self.model.set_region(Region::normalized(Tick(anchor), Tick(cur)));
                    self.drag = Some(Drag::Region { anchor });
                }
                Some(Drag::RegionEdge { start_edge }) => {
                    if let Some(r) = self.model.project().region {
                        let raw = x_to_tick(pos.x).max(0);
                        let snapped = self.snap(raw, None);
                        let newr = if start_edge {
                            Region::normalized(Tick(snapped), r.end)
                        } else {
                            Region::normalized(r.start, Tick(snapped))
                        };
                        self.model.set_region(newr.or(Some(r)));
                    }
                    self.drag = Some(Drag::RegionEdge { start_edge });
                }
                Some(Drag::Marker { orig, .. }) => {
                    let raw = x_to_tick(pos.x).max(0);
                    let cur = self.snap(raw, None);
                    self.drag = Some(Drag::Marker { orig, cur });
                }
                Some(Drag::MarkerEdge { start, left, .. }) => {
                    let raw = x_to_tick(pos.x).max(0);
                    let cur = self.snap(raw, None).max(if left { 0 } else { start });
                    self.drag = Some(Drag::MarkerEdge { start, left, cur });
                }
                Some(Drag::TransitionEdge { id, left, gesture }) => {
                    // live-ресайз окна: SetTransition с коалесингом (один undo)
                    let cmd = {
                        let p = self.model.project();
                        p.transitions.iter().find(|t| t.id == id).cloned().and_then(|mut t| {
                            let joint = t.joint(p)?.0;
                            let body = |side: Option<ClipId>| {
                                // у фейда сторона-пустота телом не ограничена
                                side.map_or(i64::MAX, |id| {
                                    p.clips
                                        .iter()
                                        .find(|c| c.id == id)
                                        .map_or(0, |c| c.duration.0)
                                })
                            };
                            let raw = x_to_tick(pos.x);
                            if left {
                                t.in_offset = Tick((joint - raw).clamp(0, body(t.a)));
                            } else {
                                t.out_offset = Tick((raw - joint).clamp(0, body(t.b)));
                            }
                            (t.in_offset + t.out_offset >= MIN_TRANSITION_DURATION)
                                .then_some(Command::SetTransition(t))
                        })
                    };
                    if let Some(cmd) = cmd && self.model.apply_coalesced(cmd, gesture).is_ok() {
                        self.dirty_content = true;
                    }
                    self.drag = Some(Drag::TransitionEdge { id, left, gesture });
                }
                Some(Drag::TransitionBody { id, orig_at, .. }) => {
                    // roll стыка: превью-риска, тримы на release
                    let cur_at = {
                        let p = self.model.project();
                        p.transitions
                            .iter()
                            .find(|t| t.id == id)
                            .and_then(|t| {
                                let a = p.clips.iter().find(|c| Some(c.id) == t.a)?;
                                let b = p.clips.iter().find(|c| Some(c.id) == t.b)?;
                                Some(x_to_tick(pos.x).clamp(
                                    a.start.0 + MIN_CLIP_DURATION.0,
                                    b.start.0 + b.duration.0 - MIN_CLIP_DURATION.0,
                                ))
                            })
                            .unwrap_or(orig_at)
                    };
                    self.drag = Some(Drag::TransitionBody { id, orig_at, cur_at });
                }
                Some(Drag::Morph { id, grab_dt, .. }) => {
                    let raw = x_to_tick(pos.x) - grab_dt;
                    let cur_start = self.snap(raw, None).max(0);
                    self.drag = Some(Drag::Morph { id, grab_dt, cur_start, moved: true });
                }
                Some(Drag::Volume { id, .. }) => {
                    // t: 0 = верх (+6 dB), 1 = низ (−6 dB) — порт рейла
                    if let Some(c) = self.model.clip(id) {
                        let top = lane_top(c.kind, c.lane) + 3.0;
                        let h = lane_h - 8.0;
                        let t = ((pos.y - top) / h).clamp(0.0, 1.0);
                        let gain_db = (0.5 - t) * RAIL_DB_SPAN;
                        // live-превью без записи в историю
                        let mut p = self.model.project().clone();
                        if let Some(cm) = p.clips.iter_mut().find(|c| c.id == id) {
                            cm.gain_db = gain_db;
                        }
                        self.player.set_project(p);
                        self.live_gain = Some((id, gain_db));
                    }
                    self.drag = Some(Drag::Volume { id, moved: true });
                }
                None => {}
            }
        }

        // ---- release -------------------------------------------------------------
        if response.drag_stopped() {
            match self.drag.take() {
                Some(Drag::Clip(d)) if d.moved => match d.mode {
                    DragMode::Move => {
                        // одиночный частичный наезд = кроссфейд (жест Р.24):
                        // клипы встык до C, окно перехода = бывший наезд —
                        // звук байт-идентичен старому перекрытию (т.136)
                        match plan_move(self.model.project(), d.id, d.cur_start, d.cur_lane) {
                            MovePlan::Crossfade { outgoing, incoming, joint, in_off, out_off } => {
                                let existing = self
                                    .model
                                    .project()
                                    .transitions
                                    .iter()
                                    .find(|t| (t.a, t.b) == (Some(outgoing), Some(incoming)))
                                    .map(|t| t.id);
                                let ev = TransitionEvent {
                                    id: existing
                                        .unwrap_or(TransitionId(self.next_transition_id())),
                                    a: Some(outgoing),
                                    b: Some(incoming),
                                    in_offset: Tick(in_off),
                                    out_offset: Tick(out_off),
                                    kind: TransitionKind::Crossfade,
                                };
                                let mut cmds = vec![
                                    Command::MoveClip {
                                        id: d.id,
                                        new_start: Tick(d.cur_start),
                                        new_lane: d.cur_lane,
                                    },
                                    Command::TrimEnd { id: outgoing, new_end: Tick(joint) },
                                    Command::TrimStart { id: incoming, new_start: Tick(joint) },
                                ];
                                cmds.push(if existing.is_some() {
                                    Command::SetTransition(ev)
                                } else {
                                    Command::AddTransition(ev)
                                });
                                // клип мог уйти ИЗ другого кроссфейда — его
                                // осиротевший сосед получает материал обратно
                                self.apply_batch_restoring(cmds);
                            }
                            _ => {
                                // растащили клипы (или увели на другую
                                // дорожку) — переход умирает, и срезанный
                                // при наезде материал возвращается (Р.26)
                                self.apply_batch_restoring(vec![Command::MoveClip {
                                    id: d.id,
                                    new_start: Tick(d.cur_start),
                                    new_lane: d.cur_lane,
                                }]);
                            }
                        }
                    }
                    DragMode::TrimStart => {
                        self.apply(Command::TrimStart { id: d.id, new_start: Tick(d.cur_start) });
                    }
                    DragMode::TrimEnd => {
                        self.apply(Command::TrimEnd { id: d.id, new_end: Tick(d.cur_end) });
                    }
                },
                Some(Drag::Marker { orig, cur }) => {
                    if orig != cur {
                        self.apply(Command::MoveMarker { from: Tick(orig), to: Tick(cur) });
                    }
                }
                Some(Drag::MarkerEdge { start, left: false, cur }) => {
                    // duration = 0 законна: схлопывание диапазона в точку
                    self.apply(Command::SetMarkerDuration {
                        at: Tick(start),
                        duration: Tick((cur - start).max(0)),
                    });
                }
                Some(Drag::TransitionBody { id, orig_at, cur_at }) if cur_at != orig_at => {
                    // roll: стык едет, окно едет с ним (офсеты не меняются),
                    // reconcile модели клампит вылезшее
                    let pair = self
                        .model
                        .project()
                        .transitions
                        .iter()
                        .find(|t| t.id == id)
                        .map(|t| (t.a, t.b));
                    // roll стыка бывает только у настоящей пары
                    if let Some((Some(a), Some(b))) = pair {
                        self.apply_batch(vec![
                            Command::TrimEnd { id: a, new_end: Tick(cur_at) },
                            Command::TrimStart { id: b, new_start: Tick(cur_at) },
                        ]);
                    }
                }
                Some(Drag::Volume { id, moved }) => {
                    if moved && let Some((gid, gain)) = self.live_gain.take_if(|(g, _)| *g == id) {
                        self.apply(Command::SetGain { id: gid, gain_db: gain });
                    }
                }
                Some(Drag::Region { .. }) | Some(Drag::RegionEdge { .. }) => {
                    self.on_region_changed();
                    // играем — курсор немедленно в начало нового региона (п.4)
                    self.on_region_released();
                }
                Some(Drag::Morph { id, cur_start, moved, .. }) => {
                    if moved
                        && let Some(mut ev) =
                            self.model.project().morphers.iter().find(|m| m.id == id).cloned()
                    {
                        ev.start = Tick(cur_start);
                        self.apply(Command::SetMorpher(ev));
                    }
                }
                _ => {}
            }
        }

        // ---- клипы -----------------------------------------------------------------
        // кэши по model_rev; Arc-снапшоты — клоны указателей (п.1 v1.4)
        let (media_names, suppress_name, media_paths) = self.names();
        let peaks = self.peaks.lock().unwrap();
        let offline = self.offline.lock().unwrap().clone();

        for c in &self.model.project().clips {
            let (start, end, lane) = match &self.drag {
                Some(Drag::Clip(d)) if d.id == c.id => (d.cur_start, d.cur_end, d.cur_lane),
                _ => (c.start.0, c.start.0 + c.duration.0, c.lane),
            };
            let x0 = tick_to_x(start);
            let x1 = tick_to_x(end);
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let top = lane_top(c.kind, lane);
            // клип тянется ПОД полосу имён дорожек — она полупрозрачная и
            // таймлайн просвечивает (п.4 р.7.1); имя рисуется от СТАРОЙ
            // границы (rect.left()), чтобы текст не лез под градиент
            let clip_rect = egui::Rect::from_min_max(
                egui::pos2(x0.max(full.left()), top + 1.0),
                egui::pos2(x1.min(rect.right()), top + lane_h - 3.0),
            );
            let selected = self.selected == Some(c.id);
            let dragging = matches!(&self.drag, Some(Drag::Clip(d)) if d.id == c.id);
            let is_offline = offline.contains(&c.media);

            let mut fill = media_paths
                .get(&c.media)
                .map(|p| clip_color(p, c.kind))
                .unwrap_or(egui::Color32::from_gray(80));
            // аудио-клипы темнее — волноформа должна читаться
            if c.kind == ClipKind::Audio {
                fill = fill.gamma_multiply(0.55);
            }
            if dragging {
                fill = fill.gamma_multiply(0.75);
            }
            let edge =
                if selected { egui::Color32::WHITE } else { egui::Color32::from_rgb(25, 25, 25) };
            painter.rect(
                clip_rect,
                4.0,
                fill,
                egui::Stroke::new(if selected { 2.0 } else { 1.0 }, edge),
                egui::StrokeKind::Inside,
            );
            // лёгкий вертикальный градиент (свет сверху)
            crate::toolbar::gradient_rect(
                &painter,
                egui::Rect::from_min_max(
                    clip_rect.left_top() + egui::vec2(1.0, 1.0),
                    egui::pos2(clip_rect.right() - 1.0, clip_rect.center().y),
                ),
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22),
                egui::Color32::TRANSPARENT,
            );
            let hovered =
                hover.map(|h| clip_rect.contains(h) && self.drag.is_none()).unwrap_or(false);
            if hovered && !selected {
                painter.rect(
                    clip_rect,
                    4.0,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 90),
                    ),
                    egui::StrokeKind::Inside,
                );
            }
            if is_offline {
                painter.rect_filled(
                    clip_rect,
                    4.0,
                    egui::Color32::from_rgba_unmultiplied(55, 25, 25, 110),
                );
                painter.rect(
                    clip_rect.shrink(1.5),
                    3.0,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 140, 140)),
                    egui::StrokeKind::Inside,
                );
            }

            // филмстрип видео-клипа: тумбы из индекс-БД/сервиса, отрисовка
            // никогда не блокируется и не трогает декодеры плеера
            if c.kind == ClipKind::Video && !is_offline && clip_rect.width() > 30.0 {
                let thumb_h = clip_rect.height() - 2.0;
                let thumb_w = (thumb_h * 16.0 / 9.0).max(28.0);
                let step = crate::thumbs::thumb_source_step(
                    Tick(end - start),
                    (x1 - x0).max(1.0) * 1.0,
                    thumb_w,
                );
                let mut x = clip_rect.left();
                while x < clip_rect.right() {
                    // момент СЕРЕДИНЫ колонки (v1: тумб центрируется на
                    // своей позиции, а не floor'ится в прошлое — п.3)
                    let tl = x_to_tick(x + thumb_w * 0.5) - start;
                    if tl >= 0 {
                        // бакет от НАЧАЛА КУСКА (source_in), не от нуля
                        // файла: у клипа с source_in=3c первая колонка
                        // показывала кадр за 3 секунды ДО начала куска
                        // квантование ОТ НУЛЯ ФАЙЛА — общий кэш с бином
                        // (п.11 р.6, тест 62)
                        let src_t = crate::thumbs::thumb_key_t(c.source_in.0, tl, step);
                        let key = crate::thumbs::ThumbKey { media: c.media, t: src_t };
                        let tex = match self.thumb_textures.get(&key) {
                            Some(t) => Some(t.clone()),
                            None => match self.thumbs.get(&key) {
                                Some(img) => {
                                    let t = ui.ctx().load_texture(
                                        format!("thumb-{}-{}", key.media.0, key.t),
                                        (*img).clone(),
                                        egui::TextureOptions::LINEAR,
                                    );
                                    self.thumb_textures.insert(key, t.clone());
                                    self.thumb_lru.retain(|k| *k != key);
                                    self.thumb_lru.push_back(key);
                                    // LRU вместо полного clear() (п.2: сброс
                                    // всех текстур давал шторм перезагрузок
                                    // при зуме)
                                    while self.thumb_textures.len() > crate::thumbs::MAX_READY
                                        && let Some(old) = self.thumb_lru.pop_front()
                                    {
                                        self.thumb_textures.remove(&old);
                                    }
                                    Some(t)
                                }
                                None => {
                                    self.thumbs.request(key);
                                    None
                                }
                            },
                        };
                        if let Some(t) = tex {
                            let dst = egui::Rect::from_min_size(
                                egui::pos2(x, clip_rect.top() + 1.0),
                                egui::vec2(thumb_w.min(clip_rect.right() - x), thumb_h),
                            );
                            let uv = egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2((dst.width() / thumb_w).min(1.0), 1.0),
                            );
                            painter.image(t.id(), dst, uv, egui::Color32::WHITE);
                            // кадры филмстрипа: тонкая рамка + зазор
                            // рамка 1px + зазор между превью — перфорация
                            // убрана (п.21 р.6: ~42 лишних шейпа на кадр)
                            painter.rect(
                                dst,
                                0.0,
                                egui::Color32::TRANSPARENT,
                                egui::Stroke::new(1.0, crate::theme::THUMB_FRAME),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                    x += thumb_w + 3.0;
                }
            }

            // стерео-волноформа
            if c.kind == ClipKind::Audio
                && !is_offline
                && let Some(levels) = peaks.get(&c.media)
                && !levels.is_empty()
            {
                let frames_per_px = tpp * 48_000.0 / 90_000.0;
                let mut level = 0usize;
                let mut block = kt_media::PEAK_BLOCK_FRAMES as f64;
                while block * 2.0 <= frames_per_px && level + 1 < levels.len() {
                    level += 1;
                    block *= 2.0;
                }
                let (left_ch, right_ch) = &levels[level];
                let mid = clip_rect.center().y;
                let half = (clip_rect.height() - 6.0) * 0.5;
                let eff_src_in = c.source_in.0 + (start - c.start.0).max(0);
                let src_in_frames = Tick(eff_src_in.max(0)).to_audio_frames() as f64;
                let col_l = egui::Color32::from_rgba_unmultiplied(170, 210, 255, 230);
                let col_r = egui::Color32::from_rgba_unmultiplied(230, 190, 255, 230);
                // один меш на клип вместо 2 line_segment на ПИКСЕЛЬ
                // (п.1 v1.4: тысячи шейпов за кадр на широких клипах)
                let mut mesh = egui::Mesh::default();
                let mut x = clip_rect.left();
                while x < clip_rect.right() {
                    let tl0 = x_to_tick(x) - start;
                    let f0 = src_in_frames + tl0 as f64 * 48_000.0 / 90_000.0;
                    if f0 >= 0.0 {
                        let bi = (f0 / block) as usize;
                        if let Some(&(mn, mx)) = left_ch.get(bi) {
                            let amp = (mx.abs().max(mn.abs())) as f32 / 32767.0;
                            mesh.add_colored_rect(
                                egui::Rect::from_min_max(
                                    egui::pos2(x, mid - (amp * half).max(0.5)),
                                    egui::pos2(x + 1.0, mid),
                                ),
                                col_l,
                            );
                        }
                        if let Some(&(mn, mx)) = right_ch.get(bi) {
                            let amp = (mx.abs().max(mn.abs())) as f32 / 32767.0;
                            mesh.add_colored_rect(
                                egui::Rect::from_min_max(
                                    egui::pos2(x, mid),
                                    egui::pos2(x + 1.0, mid + (amp * half).max(0.5)),
                                ),
                                col_r,
                            );
                        }
                    }
                    x += 1.0;
                }
                if !mesh.is_empty() {
                    painter.add(egui::Shape::mesh(mesh));
                }
            }

            // volume rail (аудио, ширина > 40)
            if c.kind == ClipKind::Audio && clip_rect.width() > 40.0 {
                let rail = egui::Rect::from_min_size(
                    egui::pos2(x0.max(rect.left()) + EDGE_GRAB_PX + 3.0, top + 3.0),
                    egui::vec2(VOLUME_RAIL_W, lane_h - 8.0),
                );
                painter.rect_filled(rail, 2.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 70));
                let gain = match self.live_gain {
                    Some((id, g)) if id == c.id => g,
                    _ => c.gain_db,
                };
                let t = (0.5 - gain / RAIL_DB_SPAN).clamp(0.0, 1.0);
                let y = rail.top() + rail.height() * t;
                painter.line_segment(
                    [egui::pos2(rail.left(), y), egui::pos2(rail.right(), y)],
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 220, 120)),
                );
            }

            // имя клипа на плашке
            if clip_rect.width() > 40.0 && !suppress_name.contains(&c.id) {
                let mut label = media_names.get(&c.id).cloned().unwrap_or_default();
                if !c.fx.is_empty() {
                    label.push(' ');
                    label.push_str(crate::icons::FX);
                }
                if c.reversed {
                    label.push_str(" ◀");
                }
                let galley = painter.layout_no_wrap(
                    label,
                    egui::FontId::proportional(10.5),
                    egui::Color32::WHITE,
                );
                let plate = egui::Rect::from_min_size(
                    egui::pos2(clip_rect.left().max(rect.left()), clip_rect.top())
                        + egui::vec2(4.0, 2.0),
                    galley.size() + egui::vec2(8.0, 3.0),
                );
                painter.rect_filled(
                    plate.intersect(clip_rect),
                    3.0,
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 140),
                );
                painter.galley(plate.min + egui::vec2(4.0, 1.5), galley, egui::Color32::WHITE);
            }

            // трим-ручки на выделенном
            if selected && clip_rect.width() > 20.0 {
                for x in [clip_rect.left() + 2.0, clip_rect.right() - 4.0] {
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(x, clip_rect.center().y - 7.0),
                            egui::vec2(2.5, 14.0),
                        ),
                        1.0,
                        egui::Color32::from_gray(235),
                    );
                }
            }

            // coverage-полоска из кэша (инвариант №4)
            let cov_kind = match c.kind {
                ClipKind::Audio => BurstKind::Audio,
                ClipKind::Video => BurstKind::video(Some(self.player.preview_quality())),
            };
            let (b0, b1) = match c.kind {
                ClipKind::Audio => {
                    let f0 = c.source_in.to_audio_frames();
                    let f1 = (c.source_in + c.duration).to_audio_frames();
                    (
                        f0.div_euclid(AUDIO_BURST_FRAMES as i64),
                        (f1 - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1,
                    )
                }
                ClipKind::Video => (c.source_in.0, c.source_in.0 + c.duration.0),
            };
            let _ = (cov_kind, b0, b1);
            for run in coverage_snapshot.get(&c.id).map(|v| v.as_slice()).unwrap_or(&[]) {
                let (t0, t1) = match c.kind {
                    ClipKind::Audio => (
                        c.start.0
                            + Tick::from_audio_frames(
                                run.start * AUDIO_BURST_FRAMES as i64
                                    - c.source_in.to_audio_frames(),
                            )
                            .0,
                        c.start.0
                            + Tick::from_audio_frames(
                                run.end * AUDIO_BURST_FRAMES as i64 - c.source_in.to_audio_frames(),
                            )
                            .0,
                    ),
                    ClipKind::Video => {
                        (c.start.0 + run.start - c.source_in.0, c.start.0 + run.end - c.source_in.0)
                    }
                };
                painter.line_segment(
                    [
                        egui::pos2(tick_to_x(t0).max(clip_rect.left()), clip_rect.bottom() - 1.5),
                        egui::pos2(tick_to_x(t1).min(clip_rect.right()), clip_rect.bottom() - 1.5),
                    ],
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(110, 235, 110)),
                );
            }
        }
        drop(peaks);

        // ---- morph-блоки (H1, порт v1: фиолетовый скруглённый блок) ---------
        for m in &self.model.project().morphers {
            let start = match &self.drag {
                Some(Drag::Morph { id, cur_start, .. }) if *id == m.id => *cur_start,
                _ => m.start.0,
            };
            let x0 = tick_to_x(start);
            let x1 = tick_to_x(start + m.duration.0).max(x0 + 8.0);
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let top = morph_lane_top(m.lane);
            let r = egui::Rect::from_min_max(
                egui::pos2(x0.max(rect.left()), top + 3.0),
                egui::pos2(x1.min(rect.right()), top + lane_h - 5.0),
            );
            let selected = self.selected_morph == Some(m.id);
            painter.rect(
                r,
                4.0,
                egui::Color32::from_rgba_unmultiplied(140, 90, 200, 200),
                egui::Stroke::new(
                    if selected { 2.0 } else { 1.0 },
                    if selected { egui::Color32::WHITE } else { egui::Color32::from_rgb(40, 30, 55) },
                ),
                egui::StrokeKind::Inside,
            );
            if r.width() > 46.0 {
                let label = match &m.kind {
                    MorphKind::EffectBatch { .. } => crate::tf!(k::tl::morph::BATCH, m.targets.len()),
                    MorphKind::Replace { .. } => crate::tf!(k::tl::morph::REPLACE, m.targets.len()),
                };
                painter.text(
                    egui::pos2(r.left() + 6.0, r.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(11.0),
                    egui::Color32::WHITE,
                );
            }
        }
        // режим размещения: подсветка morph-дорожек + строка-подсказка (v1)
        if self.pending_morpher.is_some() {
            for lane in 0..morph_lanes {
                let y = morph_lane_top(lane);
                let lr = egui::Rect::from_min_max(
                    egui::pos2(rect.left(), y),
                    egui::pos2(rect.right(), y + lane_h - 2.0),
                );
                painter.rect(
                    lr,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(96, 200, 140, 52),
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(140, 235, 170, 200)),
                    egui::StrokeKind::Inside,
                );
            }
        }

        // ---- силуэт дропа файла (п.2 v1.4): подсветка дорожки + призрак --
        // Тот же силуэт обслуживает перетаскивание карточки ИЗ БИНА
        // (п.5 р.8): payload от egui-драга даёт позицию сам.
        let bin_drag_pos = egui::DragAndDrop::payload::<crate::bin_panel::BinDrag>(ui.ctx())
            .map(|p| *p)
            .and_then(|p| ui.input(|i| i.pointer.hover_pos()).map(|pos| (p.0, pos)));
        if let Some((media, pos)) = bin_drag_pos
            && let Some(g) = self.tl_geom.clone()
            && g.in_lanes(pos)
            && let Some((kind, lane)) = g.lane_at_y(pos.y)
        {
            self.dnd_hover = Some(pos);
            if ui.input(|i| i.pointer.any_released()) {
                let _ = egui::DragAndDrop::take_payload::<crate::bin_panel::BinDrag>(ui.ctx());
                let at = self.snap(x_to_tick(pos.x).max(0), None);
                self.place_media_at(media, Tick(at), Some((kind, lane)));
                self.dnd_hover = None;
            }
        }
        if let Some(pos) = self.dnd_hover
            && self.pending_morpher.is_none()
            && let Some(g) = self.tl_geom.clone()
            && g.in_lanes(pos)
            && let Some((kind, lane)) = g.lane_at_y(pos.y)
        {
            let top = g.lane_top(kind, lane);
            let lane_rect = egui::Rect::from_min_max(
                egui::pos2(rect.left(), top),
                egui::pos2(rect.right(), top + lane_h - 2.0),
            );
            painter.rect(
                lane_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(96, 200, 140, 36),
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(140, 235, 170, 160)),
                egui::StrokeKind::Inside,
            );
            let at = self.snap(g.x_to_tick(pos.x).max(0), None);
            let x0 = g.tick_to_x(at);
            let ghost_w = (((90_000.0 * 3.0) / self.ticks_per_px) as f32).max(3.0); // ~3 c, мин 3 px
            let ghost = egui::Rect::from_min_max(
                egui::pos2(x0, top + 1.0),
                egui::pos2((x0 + ghost_w).min(rect.right()), top + lane_h - 3.0),
            );
            painter.rect(
                ghost,
                4.0,
                egui::Color32::from_rgba_unmultiplied(140, 235, 170, 44),
                egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(160, 240, 180, 220)),
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [egui::pos2(x0, rect.top() + RULER_H), egui::pos2(x0, rect.bottom())],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(160, 240, 180, 180)),
            );
        }

        // ---- вертикальный скроллбар (п.12 р.6) -----------------------------
        if vl.scroll_max > 0.0 {
            let track = egui::Rect::from_min_max(
                egui::pos2(rect.right() - 7.0, rect.top() + RULER_H + 2.0),
                egui::pos2(rect.right() - 1.0, rect.bottom() - 2.0),
            );
            let usable = track.height();
            let frac_vis = (usable / vl.content_h).clamp(0.05, 1.0);
            let knob_h = (usable * frac_vis).max(24.0);
            let frac_pos = (self.tl_scroll / vl.scroll_max).clamp(0.0, 1.0);
            let knob_y = track.top() + frac_pos * (usable - knob_h);
            let knob = egui::Rect::from_min_size(
                egui::pos2(track.left(), knob_y),
                egui::vec2(track.width(), knob_h),
            );
            let resp = ui.interact(
                track,
                ui.id().with("tl-vscroll"),
                egui::Sense::click_and_drag(),
            );
            if resp.dragged()
                && let Some(p) = resp.interact_pointer_pos()
            {
                let f = ((p.y - track.top() - knob_h / 2.0) / (usable - knob_h).max(1.0))
                    .clamp(0.0, 1.0);
                self.tl_scroll = f * vl.scroll_max;
            }
            painter.rect_filled(track, 3.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 90));
            painter.rect_filled(
                knob,
                3.0,
                if resp.hovered() || resp.dragged() {
                    egui::Color32::from_gray(120)
                } else {
                    egui::Color32::from_gray(80)
                },
            );
        }

        // ---- переходы (Р.24, п.140): подложка + X + риска стыка ---------------
        {
            let p = self.model.project();
            for t in &p.transitions {
                // якорь фейда (Р.25) — единственный живой конец, стык у него
                let Some(a) = t.a.or(t.b).and_then(|id| p.clips.iter().find(|c| c.id == id))
                else {
                    continue;
                };
                let Some(base_joint) = t.joint(p).map(|j| j.0) else { continue };
                let joint = match &self.drag {
                    Some(Drag::TransitionBody { id, cur_at, .. }) if *id == t.id => *cur_at,
                    _ => base_joint,
                };
                let x0 = tick_to_x(joint - t.in_offset.0);
                let x1 = tick_to_x(joint + t.out_offset.0);
                if x1 < rect.left() || x0 > rect.right() {
                    continue;
                }
                let top = lane_top(a.kind, a.lane);
                let r = transition_rect(x0, x1, top, lane_h);
                // ручки (Р.26): материал соседей продолжается за стык и
                // никуда не делся — растащишь клипы, и он вернётся
                let xj = tick_to_x(joint);
                let (left, right) = transition_handle_rects(x0, xj, x1, top, lane_h);
                let handle_fill = |id: Option<ClipId>| {
                    let c = id.and_then(|i| p.clips.iter().find(|c| c.id == i))?;
                    let base = media_paths
                        .get(&c.media)
                        .map(|path| clip_color(path, c.kind))
                        .unwrap_or(egui::Color32::from_gray(80));
                    Some(egui::Color32::from_rgba_unmultiplied(
                        base.r(),
                        base.g(),
                        base.b(),
                        70,
                    ))
                };
                // слева от стыка просвечивает ВХОДЯЩИЙ, справа — уходящий
                if let Some(fill) = handle_fill(t.b) {
                    painter.rect_filled(left, 2.0, fill);
                }
                if let Some(fill) = handle_fill(t.a) {
                    painter.rect_filled(right, 2.0, fill);
                }
                painter.rect_filled(
                    r,
                    2.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22),
                );
                let st =
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 130));
                // кроссфейд — X (две встречные диагонали), фейд — ОДНА
                // диагональ по направлению нарастания (Р.25)
                match (t.a.is_some(), t.b.is_some()) {
                    (true, true) => {
                        painter.line_segment([r.left_bottom(), r.right_top()], st);
                        painter.line_segment([r.left_top(), r.right_bottom()], st);
                    }
                    // фейд-ин: пустота слева, картинка проявляется вправо
                    (false, true) => {
                        painter.line_segment([r.left_bottom(), r.right_top()], st);
                    }
                    // фейд-аут: гаснет вправо
                    _ => {
                        painter.line_segment([r.left_top(), r.right_bottom()], st);
                    }
                }
                painter.line_segment(
                    [egui::pos2(xj, r.top()), egui::pos2(xj, r.bottom())],
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 80)),
                );
            }
        }

        // ---- маркеры ----------------------------------------------------------------
        for m in &self.model.project().markers {
            let mt = match &self.drag {
                Some(Drag::Marker { orig, cur }) if *orig == m.t.0 => *cur,
                _ => m.t.0,
            };
            let mdur = match &self.drag {
                // правый край диапазона тянется — живой предпросмотр
                Some(Drag::MarkerEdge { start, left: false, cur }) if *start == m.t.0 => {
                    (*cur - m.t.0).max(0)
                }
                _ => m.duration.0,
            };
            let x = tick_to_x(mt);
            let xe = tick_to_x(mt + mdur);
            if xe < rect.left() || x > rect.right() {
                continue;
            }
            let amber = egui::Color32::from_rgb(255, 195, 0);
            let top = rect.top() + TIME_TOP;
            // диапазонная метка (Р.24, marked_range): лента на строке
            // подписей времени (нового места не занимаем, п.13 чек-листа) +
            // полупрозрачная граница конца
            if mdur > 0 {
                full_painter.rect_filled(
                    marker_band_rect(x, xe, top),
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 195, 0, 70),
                );
                full_painter.line_segment(
                    [egui::pos2(xe, top), egui::pos2(xe, rect.bottom())],
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 195, 0, 120)),
                );
            }
            full_painter.line_segment(
                [egui::pos2(x, top), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.0, amber),
            );
            full_painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(x - 5.0, top),
                    egui::pos2(x + 5.0, top),
                    egui::pos2(x, top + 8.0),
                ],
                amber,
                egui::Stroke::NONE,
            ));
            // имя метки — НА ТОЙ ЖЕ строке, что подписи времени, правее
            // флажка (п.3 р.12: отдельная полоса под метки пустовала).
            // Рисуется после линейки, поэтому янтарное имя перекрывает
            // серую цифру времени, а не наоборот.
            if !m.name.is_empty() {
                let font = egui::FontId::proportional(10.5);
                let g = full_painter.layout_no_wrap(m.name.clone(), font, amber);
                let at = egui::pos2(x + 7.0, top + 1.0);
                // подложка: имя ложится на цифру времени, и без неё они
                // сливаются в кашу (проверено живьём, п.3 р.12)
                full_painter.rect_filled(
                    egui::Rect::from_min_size(at, g.size()).expand2(egui::vec2(2.0, 0.0)),
                    2.0,
                    egui::Color32::from_rgba_unmultiplied(12, 13, 16, 235),
                );
                full_painter.galley(at, g, amber);
            }
        }

        // ---- инлайн-ввод имени метки (п.1 р.10) --------------------------
        if let Some(ed) = self.marker_edit.clone() {
            let x = tick_to_x(ed.at);
            let mut buf = ed.buf.clone();
            let mut done: Option<bool> = None; // Some(true) — сохранить
            egui::Area::new(egui::Id::new("marker-name-edit"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(x + 6.0, rect.top() + TIME_TOP))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut buf)
                                .hint_text(if ed.is_new {
                                    tr(k::tl::field::MARKER_NAME_NEW)
                                } else {
                                    tr(k::tl::field::MARKER_NAME)
                                })
                                .desired_width(140.0),
                        );
                        if !r.has_focus() && self.marker_edit_focus {
                            r.request_focus();
                            self.marker_edit_focus = false;
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            done = Some(true);
                        } else if ui.input(|i| i.key_pressed(egui::Key::Escape))
                            || (r.lost_focus() && !r.has_focus())
                        {
                            // клик мимо/Esc: метка просто остаётся без имени
                            done = Some(false);
                        }
                    });
                });
            match done {
                Some(true) => {
                    let name = buf.trim().to_string();
                    self.marker_edit = None;
                    if !name.is_empty() {
                        self.apply(Command::SetMarkerName { at: Tick(ed.at), name });
                    }
                }
                Some(false) => {
                    // новая метка остаётся без имени; при переименовании
                    // Esc/клик мимо — просто отмена, старое имя на месте
                    self.marker_edit = None;
                }
                None => self.marker_edit = Some(crate::app::MarkerEdit { buf, ..ed }),
            }
        }

        // ---- инлайн-ввод имени дорожки (п.2 р.10) ------------------------
        if let Some(ed) = self.lane_edit.clone() {
            let y = geom
                .rows
                .iter()
                .position(|&(k, l)| k == ed.kind && l == ed.lane)
                .map(|row| geom.row_top(row))
                .unwrap_or(rect.top());
            let mut buf = ed.buf.clone();
            let mut done: Option<bool> = None;
            egui::Area::new(egui::Id::new("lane-name-edit"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(full.left() + 2.0, y + 2.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut buf)
                                .hint_text(tr(k::tl::field::LANE_NAME))
                                .desired_width(120.0),
                        );
                        if !r.has_focus() && self.lane_edit_focus {
                            r.request_focus();
                            self.lane_edit_focus = false;
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            done = Some(true);
                        } else if ui.input(|i| i.key_pressed(egui::Key::Escape))
                            || (r.lost_focus() && !r.has_focus())
                        {
                            done = Some(false);
                        }
                    });
                });
            match done {
                Some(true) => {
                    let name = buf.trim().to_string();
                    self.lane_names.retain(|(k, l, _)| !(*k == ed.kind && *l == ed.lane));
                    if !name.is_empty() {
                        self.lane_names.push((ed.kind, ed.lane, name));
                    }
                    self.dirty_ui = true;
                    self.lane_edit = None;
                }
                Some(false) => self.lane_edit = None,
                None => self.lane_edit = Some(crate::app::LaneEdit { buf, ..ed }),
            }
        }

        // ---- heat-зоны прыжка/edge-scroll: градиент при наведении ------------------
        if let Some(pos) = hover {
            let boost = if self.drag.is_some() { 1.0f32 } else { 0.6 };
            let margin = edge_margin;
            let warm = |heat: f32, zone: egui::Rect, from_left: bool| {
                if heat > 0.02 {
                    let a = ((36.0 + heat * 174.0) * boost) as u8;
                    let (c0, c1) = if from_left {
                        (
                            egui::Color32::from_rgba_unmultiplied(255, 178, 100, a),
                            egui::Color32::TRANSPARENT,
                        )
                    } else {
                        (
                            egui::Color32::TRANSPARENT,
                            egui::Color32::from_rgba_unmultiplied(255, 178, 100, a),
                        )
                    };
                    let mut mesh = egui::Mesh::default();
                    for (p, c) in [
                        (zone.left_top(), c0),
                        (zone.right_top(), c1),
                        (zone.right_bottom(), c1),
                        (zone.left_bottom(), c0),
                    ] {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos: p,
                            uv: egui::epaint::WHITE_UV,
                            color: c,
                        });
                    }
                    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                    full_painter.add(egui::Shape::mesh(mesh));
                }
            };
            let lh = ((rect.left() + margin - pos.x) / margin).clamp(0.0, 1.0);
            let rh = ((pos.x - (rect.right() - margin)) / margin).clamp(0.0, 1.0);
            // левая зона начинается от КРАЯ панели (через gutter): горячий
            // конец градиента упирается в физическую кромку окна, а не
            // обрывается на шве gutter'а (п.20 р.6)
            warm(
                lh,
                egui::Rect::from_min_max(
                    egui::pos2(full.left(), rect.top() + RULER_H),
                    egui::pos2(rect.left() + margin * 0.6, rect.bottom()),
                ),
                true,
            );
            warm(
                rh,
                egui::Rect::from_min_max(
                    egui::pos2(rect.right() - margin * 0.6, rect.top() + RULER_H),
                    egui::pos2(rect.right(), rect.bottom()),
                ),
                false,
            );
        }

        // ---- полоса имён дорожек: полупрозрачный градиент ПОВЕРХ всего --
        // (пп.4-5 р.7.1) Рисуется ПОСЛЕ клипов и heat-зон: таймлайн и
        // полоски кэша просвечивают, heat-градиент втекает под неё слева
        // без резкой границы; сами имена — сплошные, читаемые.
        {
            let g = crate::theme::GUTTER;
            let solid = egui::Color32::from_rgba_unmultiplied(g.r(), g.g(), g.b(), 165);
            let mid = egui::Color32::from_rgba_unmultiplied(g.r(), g.g(), g.b(), 85);
            let split = gutter.left() + gutter.width() * 0.45;
            crate::theme::hgrad(
                &full_painter,
                egui::Rect::from_min_max(gutter.min, egui::pos2(split, gutter.bottom())),
                solid,
                mid,
            );
            crate::theme::hgrad(
                &full_painter,
                egui::Rect::from_min_max(egui::pos2(split, gutter.top()), gutter.max),
                mid,
                egui::Color32::TRANSPARENT,
            );
            // п.10 р.11: переименованная дорожка в 34-пиксельный жёлоб не
            // влезает — раньше текст центрировался в нём и вытекал В ОБЕ
            // стороны: слева обрезался краем окна, справа лез на клипы.
            // Теперь: влезает — как раньше; не влезает — ярлык от левого
            // края поверх НАЧАЛА дорожки, на своей подложке, с усечением.
            let col = egui::Color32::from_gray(175);
            let font = egui::FontId::proportional(12.0);
            for (label, cy, row_top) in &gutter_labels {
                let fit = full_painter.layout_no_wrap(label.clone(), font.clone(), col);
                if fit.size().x <= gutter.width() - 4.0 {
                    full_painter.text(
                        egui::pos2(gutter.left() + gutter.width() * 0.42, *cy),
                        egui::Align2::CENTER_CENTER,
                        label,
                        font.clone(),
                        col,
                    );
                    continue;
                }
                let mut job = egui::text::LayoutJob::single_section(
                    label.clone(),
                    egui::TextFormat { font_id: font.clone(), color: col, ..Default::default() },
                );
                job.wrap = egui::text::TextWrapping {
                    max_width: LANE_LABEL_MAX_W,
                    max_rows: 1,
                    break_anywhere: true,
                    overflow_character: Some('…'),
                };
                let galley = full_painter.layout_job(job);
                let x = gutter.left() + 3.0;
                // строкой НИЖЕ имени клипа; на низкой дорожке прижимаемся к
                // её низу, но всё равно под именем клипа (п.2 р.13)
                let y = lane_label_y(*row_top, lane_h, galley.size().y);
                let plate = egui::Rect::from_min_size(
                    egui::pos2(x - 3.0, y - 1.0),
                    egui::vec2(galley.size().x + 12.0, galley.size().y + 2.0),
                );
                crate::theme::hgrad(&full_painter, plate, solid, egui::Color32::TRANSPARENT);
                full_painter.galley(egui::pos2(x, y), galley, col);
            }
        }

        // ---- снап-линия / playhead -----------------------------------------------
        if let Some(t) = self.snap_line {
            let x = tick_to_x(t);
            full_painter.line_segment(
                [egui::pos2(x, rect.top() + RULER_H), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 220, 120)),
            );
        }
        // курсор — потребитель позиции для ПОКАЗА: часы показа, как превью и
        // таймкод тулбара (инвариант 12; сырой transport.playhead() при игре
        // прыгает квантами колбэка и расходится со слышимым на latency)
        let ph = self.player.video_playhead();
        let x = tick_to_x(ph.0);
        if x >= rect.left() && x <= rect.right() {
            // Свечение курсора (п.4 р.5, переделано п.5 р.13): НЕ заходит на
            // полоски кэша (начинается с полосы времени) и светит ТОЛЬКО со
            // стороны движения — шлейф позади курсора: вперёд слева, в
            // реверсе справа. На паузе сторона берётся от последнего знака
            // скорости, чтобы не мигала при остановке.
            let glow_half = 12.0f32;
            let mut mesh = egui::Mesh::default();
            let c0 = egui::Color32::from_rgba_unmultiplied(255, 60, 60, 0);
            let c1 = egui::Color32::from_rgba_unmultiplied(255, 60, 60, 90);
            let top_y = rect.top() + TIME_TOP;
            let trail = if self.play_dir_back {
                [(x, x + glow_half, c1, c0)]
            } else {
                [(x - glow_half, x, c0, c1)]
            };
            for (xa, xb, ca, cb) in trail {
                let i = mesh.vertices.len() as u32;
                for (px, col) in [
                    (egui::pos2(xa, top_y), ca),
                    (egui::pos2(xb, top_y), cb),
                    (egui::pos2(xb, rect.bottom()), cb),
                    (egui::pos2(xa, rect.bottom()), ca),
                ] {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: px,
                        uv: egui::epaint::WHITE_UV,
                        color: col,
                    });
                }
                mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
            }
            full_painter.add(egui::Shape::mesh(mesh));
            full_painter.line_segment(
                [egui::pos2(x, rect.top() + TIME_TOP), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 100, 100)),
            );
            full_painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(x - 6.0, rect.top() + TIME_TOP),
                    egui::pos2(x + 6.0, rect.top() + TIME_TOP),
                    egui::pos2(x, rect.top() + TIME_TOP + 9.0),
                ],
                egui::Color32::from_rgb(255, 80, 80),
                egui::Stroke::NONE,
            ));
        }

        // ---- контекст-меню ----------------------------------------------------------
        let target = self.ctx_target;
        response.context_menu(|ui| match target {
            Some(CtxTarget::Clip(id)) => {
                let (is_audio, has_group) = self
                    .model
                    .clip(id)
                    .map(|c| (c.kind == ClipKind::Audio, c.link_group.is_some()))
                    .unwrap_or((false, false));
                if ui.button(tr(k::tl::menu::COPY)).clicked() {
                    self.selected = Some(id);
                    self.copy_selection();
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::PASTE)).clicked() {
                    self.paste_at_cursor();
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::DELETE)).clicked() {
                    self.selected = Some(id);
                    // именно ЭТОТ клип (+группа), а не содержимое региона (п.10)
                    self.delete_selected();
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::SPLIT)).clicked() {
                    self.selected = Some(id);
                    self.cut_action();
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::FX)).clicked() {
                    self.open_fx_editor(id);
                    ui.close();
                }
                let is_rev = self.model.clip(id).map(|c| c.reversed).unwrap_or(false);
                let rev_label = if is_rev {
                    format!("{} {}", tr(k::tl::menu::REVERSE), crate::icons::CHECK)
                } else {
                    tr(k::tl::menu::REVERSE).to_string()
                };
                if ui.button(rev_label).clicked() {
                    self.apply(Command::SetReversed { id, reversed: !is_rev });
                    ui.close();
                }
                ui.separator();
                if has_group {
                    if ui.button(tr(k::tl::menu::UNLINK)).clicked() {
                        self.selected = Some(id);
                        self.unlink_selected();
                        ui.close();
                    }
                } else if ui.button(tr(k::tl::menu::LINK)).clicked() {
                    self.selected = Some(id);
                    self.magnetize_selected();
                    ui.close();
                }
                if is_audio && ui.button(tr(k::tl::menu::RESET_GAIN)).clicked() {
                    self.apply(Command::SetGain { id, gain_db: 0.0 });
                    ui.close();
                }
                // переходы (Р.24): кроссфейд со встык-соседом / убрать
                {
                    let p = self.model.project();
                    let me = p.clips.iter().find(|c| c.id == id).cloned();
                    if let Some(me) = me {
                        let neighbor = |incoming: bool| {
                            p.clips
                                .iter()
                                .find(|c| {
                                    c.kind == me.kind
                                        && c.lane == me.lane
                                        && if incoming {
                                            c.start == me.start + me.duration
                                        } else {
                                            c.start + c.duration == me.start
                                        }
                                })
                                .map(|c| c.id)
                        };
                        let mut items: Vec<(bool, ClipId)> = Vec::new();
                        if let Some(n) = neighbor(true)
                            && !p.transitions.iter().any(|t| (t.a, t.b) == (Some(id), Some(n)))
                        {
                            items.push((true, n));
                        }
                        if let Some(n) = neighbor(false)
                            && !p.transitions.iter().any(|t| (t.a, t.b) == (Some(n), Some(id)))
                        {
                            items.push((false, n));
                        }
                        let existing: Vec<TransitionId> = p
                            .transitions
                            .iter()
                            .filter(|t| t.touches(id))
                            .map(|t| t.id)
                            .collect();
                        // фейд у свободного края клипа (Р.25): соседа нет —
                        // картинка/звук проявляется из пустоты или гаснет в неё
                        let fades: Vec<bool> = [true, false]
                            .into_iter()
                            .filter(|&incoming| {
                                let occupied = p.transitions.iter().any(|t| {
                                    if incoming { t.a == Some(id) } else { t.b == Some(id) }
                                });
                                neighbor(incoming).is_none() && !occupied
                            })
                            .collect();
                        for (next, n) in items {
                            let label = if next {
                                tr(k::tl::menu::CROSSFADE_NEXT)
                            } else {
                                tr(k::tl::menu::CROSSFADE_PREV)
                            };
                            if ui.button(label).clicked() {
                                let (a, b) = if next { (id, n) } else { (n, id) };
                                let clip = |i: ClipId| {
                                    self.model.project().clips.iter().find(|c| c.id == i).cloned()
                                };
                                if let (Some(ca), Some(cb)) = (clip(a), clip(b)) {
                                    // окно по умолчанию: до 0.5 c на сторону,
                                    // клампится телами соседей
                                    let half = Tick(45_000)
                                        .min(Tick(ca.duration.0 / 2))
                                        .min(Tick(cb.duration.0 / 2))
                                        .max(MIN_TRANSITION_DURATION);
                                    self.apply(Command::AddTransition(TransitionEvent {
                                        id: TransitionId(self.next_transition_id()),
                                        a: Some(a),
                                        b: Some(b),
                                        in_offset: half,
                                        out_offset: half,
                                        kind: TransitionKind::Crossfade,
                                    }));
                                }
                                ui.close();
                            }
                        }
                        for incoming in fades {
                            let label = if incoming {
                                tr(k::tl::menu::FADE_OUT)
                            } else {
                                tr(k::tl::menu::FADE_IN)
                            };
                            if ui.button(label).clicked() {
                                let half = Tick(45_000)
                                    .min(Tick(me.duration.0 / 2))
                                    .max(MIN_TRANSITION_DURATION);
                                let (a, b) =
                                    if incoming { (Some(id), None) } else { (None, Some(id)) };
                                self.apply(Command::AddTransition(TransitionEvent {
                                    id: TransitionId(self.next_transition_id()),
                                    a,
                                    b,
                                    in_offset: half,
                                    out_offset: half,
                                    kind: TransitionKind::Crossfade,
                                }));
                                ui.close();
                            }
                        }
                        for tid in existing {
                            if ui.button(tr(k::tl::menu::REMOVE_TRANSITION)).clicked() {
                                self.apply(Command::RemoveTransition { id: tid });
                                ui.close();
                            }
                        }
                    }
                }
                ui.separator();
                // morpher (H1, порт v1): цели = регион (приоритет) или клип+группа
                if ui.button(tr(k::tl::menu::ADD_MORPHER)).clicked() {
                    self.selected = Some(id);
                    self.begin_morph_placement(false);
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::ADD_FX_BATCH)).clicked() {
                    self.selected = Some(id);
                    self.begin_morph_placement(true);
                    ui.close();
                }
            }
            Some(CtxTarget::Morpher(mid)) => {
                let is_batch = self
                    .model
                    .project()
                    .morphers
                    .iter()
                    .find(|m| m.id == mid)
                    .map(|m| matches!(m.kind, MorphKind::EffectBatch { .. }))
                    .unwrap_or(false);
                let bake_label = if is_batch {
                    tr(k::tl::menu::BAKE_BATCH)
                } else {
                    tr(k::tl::menu::BAKE_MORPH)
                };
                if ui.button(bake_label).clicked() {
                    if self.apply(Command::BakeMorpher { id: mid }) {
                        self.selected_morph = None;
                    }
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::REMOVE_MORPHER)).clicked() {
                    if self.apply(Command::RemoveMorpher { id: mid }) {
                        self.selected_morph = None;
                    }
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::MORPH_EDITOR)).clicked() {
                    self.open_morph_editor(mid);
                    ui.close();
                }
            }
            Some(CtxTarget::RegionEdge(t)) => {
                if ui.button(tr(k::tl::menu::MARKER_HERE)).clicked() {
                    self.apply(Command::AddMarker { at: Tick(t), duration: Tick::ZERO });
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::REGION_CLEAR)).clicked() {
                    self.clear_region();
                    ui.close();
                }
            }
            Some(CtxTarget::Marker(m)) => {
                if ui.button(tr(k::tl::menu::RENAME)).clicked() {
                    let name = self
                        .model
                        .project()
                        .markers
                        .iter()
                        .find(|k| k.t.0 == m)
                        .map(|k| k.name.clone())
                        .unwrap_or_default();
                    self.marker_edit =
                        Some(crate::app::MarkerEdit { at: m, buf: name, is_new: false });
                    self.marker_edit_focus = true;
                    ui.close();
                }
                let is_range = self
                    .model
                    .project()
                    .markers
                    .iter()
                    .any(|k| k.t.0 == m && k.is_range());
                if is_range && ui.button(tr(k::tl::menu::CLEAR_RANGE)).clicked() {
                    self.apply(Command::SetMarkerDuration { at: Tick(m), duration: Tick::ZERO });
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::REMOVE_MARKER)).clicked() {
                    self.apply(Command::RemoveMarker { at: Tick(m) });
                    ui.close();
                }
            }
            _ => {
                if !self.clipboard.clips.is_empty() && ui.button(tr(k::tl::menu::PASTE)).clicked() {
                    self.paste_at_cursor();
                    ui.close();
                }
                if self.model.project().region.is_some() {
                    if ui.button(tr(k::tl::menu::DELETE_IN_REGION)).clicked() {
                        self.delete_action();
                        ui.close();
                    }
                    if ui.button(tr(k::tl::menu::PUNCH_OUT)).clicked() {
                        self.punch_out_action();
                        ui.close();
                    }
                    ui.separator();
                    // v1: «Add morpher (clips in region)…» / FX batch
                    if ui.button(tr(k::tl::menu::ADD_MORPHER_REGION)).clicked() {
                        self.begin_morph_placement(false);
                        ui.close();
                    }
                    if ui.button(tr(k::tl::menu::ADD_FX_BATCH_REGION)).clicked() {
                        self.begin_morph_placement(true);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(tr(k::tl::menu::REGION_CLEAR)).clicked() {
                        self.clear_region();
                        ui.close();
                    }
                } else if self.clipboard.clips.is_empty() {
                    ui.weak(tr(k::tl::menu::PICK_SOMETHING));
                }
                // Дорожки добавляются ПОД кликнутой строкой (п.5 р.16):
                // «не в самый низ, а после той дорожки по которой кликнули».
                // Если кликнули по пустому месту — в конец своего вида.
                ui.separator();
                let row = self.ctx_row;
                for (label, kind) in [
                    (tr(k::tl::lane::ADD_VIDEO), LaneKind::Video),
                    (tr(k::tl::lane::ADD_AUDIO), LaneKind::Audio),
                    (tr(k::tl::lane::ADD_MORPH), LaneKind::Morph),
                ] {
                    if ui.button(label).clicked() {
                        self.insert_lane_after_row(kind, row);
                        ui.close();
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod geom_tests {
    use super::*;

    /// Тест 61 (п.12 р.6): вертикальная раскладка — сжатие панели
    /// поджимает строки до 30, рост числа дорожек даёт скролл, v_zoom
    /// уважает клампы.
    #[test]
    fn t61_vertical_layout() {
        // 3 дорожки в высокой панели: высота из панели, скролла нет
        let v = vertical_layout(400.0, 3, None);
        assert_eq!(v.lane_h, 84.0, "кламп сверху");
        assert_eq!(v.scroll_max, 0.0);
        // сжатие панели поджимает (как раньше)
        let v = vertical_layout(160.0, 3, None);
        assert!(v.lane_h < 84.0 && v.lane_h >= 30.0);
        // много дорожек: строки НЕ тоньше 30 — появляется скролл
        let v = vertical_layout(300.0, 12, None);
        assert_eq!(v.lane_h, 30.0);
        assert!(v.scroll_max > 0.0);
        let v13 = vertical_layout(300.0, 13, None);
        assert!(v13.scroll_max > v.scroll_max, "ещё дорожка — больше скролл");
        // v_zoom задаёт высоту явно, с клампами
        assert_eq!(vertical_layout(300.0, 12, Some(50.0)).lane_h, 50.0);
        assert_eq!(vertical_layout(300.0, 12, Some(4.0)).lane_h, 16.0);
        assert_eq!(vertical_layout(300.0, 12, Some(500.0)).lane_h, 120.0);
        // при v_zoom, помещающемся целиком, скролла нет
        assert_eq!(vertical_layout(800.0, 3, Some(40.0)).scroll_max, 0.0);
    }

    /// Тест 60 (п.17 р.6): глубокий зум-аут — лестница руллера, формат
    /// времени, пропорциональный порог снапа, зоны узкого клипа.
    #[test]
    fn t60_zoom_out_support() {
        // лестница: на всём диапазоне minor >= 7px, label >= 70px
        for tpp in [30.0f64, 900.0, 9_000.0, 90_000.0, 500_000.0, 2_000_000.0] {
            let (minor, label) = ruler_steps(tpp);
            assert!(minor as f64 / tpp >= 7.0 || minor == 324_000_000, "minor@{tpp}");
            assert!(label as f64 / tpp >= 70.0 || label == 324_000_000, "label@{tpp}");
            assert!(label % minor == 0, "label кратен minor@{tpp}");
        }
        // на мелком зуме — секунды, как раньше
        assert_eq!(ruler_steps(900.0).0, 90_000);
        // формат: мм:сс до часа, ч:мм:сс дальше
        assert_eq!(format_ruler_time(0), "0:00");
        assert_eq!(format_ruler_time(59 * 90_000), "0:59");
        assert_eq!(format_ruler_time(3599 * 90_000), "59:59");
        assert_eq!(format_ruler_time(3600 * 90_000), "1:00:00");
        // порог снапа растёт с зумом, без мёртвого потолка
        assert!(snap_threshold(30.0) >= 1_080);
        assert!(snap_threshold(2_000_000.0) > snap_threshold(90_000.0));
        assert_eq!(snap_threshold(90_000.0), 900_000, "10px при 90k тиков/px");
        // узкий клип: середина всегда Body
        assert_eq!(edge_grab_for_width(100.0), EDGE_GRAB_PX);
        let g = edge_grab_for_width(10.0);
        assert!((2.0..=2.5).contains(&g), "узкий клип: захват ~w/4, got {g}");
        // 30-мин клип при узком окне получает крупную ступень тумбов
        let step = crate::thumbs::thumb_source_step(
            kt_time::Tick(30 * 60 * 90_000),
            600.0,
            150.0,
        );
        assert_eq!(step, 10_800_000, "2-минутная ступень");
    }

    /// Тест 72 (п.30 р.7): авто-восстановление follow — только когда
    /// играет, регион ЦЕЛИКОМ видим, курсор внутри него и юзер перестал
    /// крутить (grace).
    #[test]
    fn t72_follow_rearm_rules() {
        use super::follow_rearm;
        let reg = Some((1000.0, 2000.0));
        // всё сошлось → true
        assert!(follow_rearm(true, 2.0, 1.5, 1500.0, reg, 500.0, 2000.0));
        // не играет / рано после скролла / нет региона
        assert!(!follow_rearm(false, 2.0, 1.5, 1500.0, reg, 500.0, 2000.0));
        assert!(!follow_rearm(true, 0.4, 1.5, 1500.0, reg, 500.0, 2000.0));
        assert!(!follow_rearm(true, 2.0, 1.5, 1500.0, None, 500.0, 2000.0));
        // регион виден не целиком (обрезан слева/справа)
        assert!(!follow_rearm(true, 2.0, 1.5, 1500.0, reg, 1200.0, 2000.0));
        assert!(!follow_rearm(true, 2.0, 1.5, 1500.0, reg, 500.0, 1400.0));
        // курсор вне региона
        assert!(!follow_rearm(true, 2.0, 1.5, 800.0, reg, 500.0, 2000.0));
        assert!(!follow_rearm(true, 2.0, 1.5, 2000.0, reg, 500.0, 2000.0), "end исключён");
        // пустой регион игнорируется
        assert!(!follow_rearm(true, 2.0, 1.5, 1000.0, Some((1000.0, 1000.0)), 0.0, 5000.0));
    }

    /// Тест 59 (п.14 р.6): follow-правила возврата окна к playhead.
    #[test]
    fn t59_follow_view_rules() {
        // без follow — никогда
        assert_eq!(follow_view(false, true, false, 0.0, 1000.0, 100.0), None);
        // пауза/драг — никогда
        assert_eq!(follow_view(true, false, false, 0.0, 1000.0, 100.0), None);
        assert_eq!(follow_view(true, true, true, 0.0, 1000.0, 100.0), None);
        // внутри зоны [2%, 85%] — не трогаем
        assert_eq!(follow_view(true, true, false, 1050.0, 1000.0, 100.0), None);
        // вышел вправо за 85% → окно на ph - 15%
        assert_eq!(follow_view(true, true, false, 1090.0, 1000.0, 100.0), Some(1090.0 - 15.0));
        // вышел влево за 2% → тоже
        assert_eq!(follow_view(true, true, false, 1001.0, 1000.0, 100.0), Some(1001.0 - 15.0));
    }

    /// Тест 47 (п.2 v1.4): геометрия таймлайна для дропа — lane_at_y по
    /// display-строкам, x_to_tick round-trip, руллер/gutter/morph → None.
    #[test]
    fn t47_tl_geom_mapping() {
        use kt_model::LaneKind;
        let g = TlGeom {
            rect: egui::Rect::from_min_max(egui::pos2(52.0, 100.0), egui::pos2(1052.0, 500.0)),
            lane_h: 50.0,
            rows: std::sync::Arc::new(vec![
                (LaneKind::Video, 0),
                (LaneKind::Video, 1),
                (LaneKind::Audio, 0),
                (LaneKind::Audio, 1),
                (LaneKind::Morph, 0),
            ]),
            view_top: 0.0,
            view_start: 90_000.0,
            tpp: 900.0,
        };
        for t in [90_000i64, 180_000, 900_000] {
            let x = g.tick_to_x(t);
            assert!((g.x_to_tick(x) - t).abs() <= (g.tpp as i64), "round-trip тика {t}");
        }
        let lanes_top = g.rect.top() + RULER_H + 3.0;
        assert_eq!(g.lane_at_y(lanes_top + 10.0), Some((ClipKind::Video, 0)));
        assert_eq!(g.lane_at_y(lanes_top + 60.0), Some((ClipKind::Video, 1)));
        assert_eq!(g.lane_at_y(lanes_top + 110.0), Some((ClipKind::Audio, 0)));
        assert_eq!(g.lane_at_y(lanes_top + 160.0), Some((ClipKind::Audio, 1)));
        assert_eq!(g.lane_at_y(lanes_top + 210.0), None, "morph-строка — не цель дропа");
        assert_eq!(g.morph_lane_at_y(lanes_top + 210.0), Some(0));
        assert_eq!(g.lane_at_y(g.rect.top() + 5.0), None);
        assert!(!g.in_lanes(egui::pos2(500.0, g.rect.top() + 5.0)));
        assert!(g.in_lanes(egui::pos2(500.0, lanes_top + 10.0)));
        assert!(!g.in_lanes(egui::pos2(10.0, lanes_top + 10.0)), "gutter левее rect");
        assert_eq!(g.lane_at_y(g.lane_top(ClipKind::Audio, 1) + 1.0), Some((ClipKind::Audio, 1)));
    }

    /// Тест 50 (п.6 v1.4): произвольный порядок строк — биекция
    /// row_of/key_at_y, прилипание drag'а клипа к ближайшей строке
    /// своего вида.
    #[test]
    fn t50_mixed_rows_bijection_and_nearest() {
        use kt_model::LaneKind;
        let rows = vec![
            (LaneKind::Video, 0),
            (LaneKind::Audio, 0),
            (LaneKind::Video, 1),
            (LaneKind::Morph, 0),
            (LaneKind::Audio, 1),
        ];
        let g = TlGeom {
            rect: egui::Rect::from_min_max(egui::pos2(52.0, 100.0), egui::pos2(1052.0, 500.0)),
            lane_h: 40.0,
            rows: std::sync::Arc::new(rows.clone()),
            view_top: 0.0,
            view_start: 0.0,
            tpp: 900.0,
        };
        // биекция row_of ↔ key_at_y по центру каждой строки
        for (row, &key) in rows.iter().enumerate() {
            assert_eq!(g.row_of(key), Some(row));
            let y = g.row_top(row) + 5.0;
            assert_eq!(g.key_at_y(y), Some(key), "строка {row}");
        }
        // lane_top отражает перемешанный порядок
        assert_eq!(g.lane_top(ClipKind::Audio, 0), g.row_top(1));
        assert_eq!(g.lane_top(ClipKind::Video, 1), g.row_top(2));
        // прилипание: видео-клип над morph-строкой (row 3) едет на V1 (row 2)
        assert_eq!(g.nearest_lane(ClipKind::Video, 3), 1);
        // аудио-клип над видео-строкой row 0 липнет к A0 (row 1)
        assert_eq!(g.nearest_lane(ClipKind::Audio, 0), 0);
        // аудио внизу (row 4) — A1
        assert_eq!(g.nearest_lane(ClipKind::Audio, 4), 1);
    }

    /// Тест 95 (п.2 р.11): выбор кандидата магнита при переносе клипа.
    #[test]
    fn t95_snap_move_candidates() {
        let dur = 100;
        let th = 10;
        // цель только для НАЧАЛА — раньше побеждал «не двигать» со
        // смещением 0, и магнит молча не работал
        assert_eq!(snap_move(104, dur, &[100], th), (100, Some(100)));
        // цель только для КОНЦА: конец клипа тянется к 500
        assert_eq!(snap_move(404, dur, &[500], th), (400, Some(500)));
        // обе цели: побеждает меньшее смещение (конец ближе)
        assert_eq!(snap_move(105, dur, &[100, 208], th), (108, Some(208)));
        assert_eq!(snap_move(103, dur, &[100, 208], th), (100, Some(100)));
        // ни одной цели в пороге — ничего не двигаем и линии нет
        assert_eq!(snap_move(300, dur, &[100, 500], th), (300, None));
        // ровно на пороге цель ещё считается, за порогом — уже нет
        assert_eq!(snap_move(110, dur, &[100], th), (100, Some(100)));
        assert_eq!(snap_move(111, dur, &[100], th), (111, None));
        // одиночная точка: ближайшая из нескольких
        assert_eq!(snap_point(104, &[100, 107, 300], th), Some(107));
        assert_eq!(snap_point(104, &[300], th), None);
    }
}
