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
/// Полоса хвата контура именованной области (Р.38 п.1). Шире рисунка
/// (линия в 1 px), но уже региона: контур области пересекает дорожки, и
/// широкая полоса воровала бы клики по событиям.
pub const AREA_GRAB_PX: f32 = 5.0;
/// Ширина хвата ручки фейда у верхнего угла события (Р.28, п.12 — жест
/// Vegas: «fade можно у событий сделать по краям без еще одних событий —
/// просто потянув за верхний уголок»).
pub const FADE_HANDLE_PX: f32 = 12.0;
/// Полувысота хвата линии уровня (непрозрачность видео).
pub const LEVEL_GRAB_PX: f32 = 5.0;
/// ±6.02 dB на весь ход линии уровня у аудио: gain = 2^((0.5−t)·2) (порт
/// рейла v1 — диапазон сохранён, сменилось только место элемента).
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
    /// `blocker` — тики того, ВО ЧТО упёрлись: подсвечивается на таймлайне
    /// и называется в подсказке (п.5 Р.28 — «я пока сам туда не промотал —
    /// не понял что оказывается там еще событие есть в конце которое мешает»).
    Clamped { start: i64, blocker: (i64, i64) },
    /// Частичный наезд на ОДНОГО соседа: жест кроссфейда — клипы встык до
    /// `joint` (середина наезда с клампами тел), окно перехода = наезд.
    Crossfade { outgoing: ClipId, incoming: ClipId, joint: i64, in_off: i64, out_off: i64 },
}

/// Высота верхней полосы события, где живут жесты Vegas (ручки фейда и
/// линия уровня). Ниже неё всё по-старому: тело, тримы, окна переходов.
pub(crate) fn top_strip_h(lane_h: f32) -> f32 {
    (lane_h * 0.35).min(12.0)
}

/// Длина «уголкового» фейда у края клипа в тиках: у такого фейда внешний
/// офсет НУЛЕВОЙ — он целиком внутри события, как в Vegas. Переход с
/// живым соседом (кроссфейд) уголком не считается: у него свои ручки.
/// Чистая функция — тест 173.
pub(crate) fn corner_fade_len(p: &Project, id: ClipId, left: bool) -> Option<i64> {
    p.transitions.iter().find_map(|t| {
        if !t.is_fade() {
            return None;
        }
        match (left, t.a, t.b) {
            // фейд в начале: пустота слева, живой конец — мы
            (true, None, Some(b)) if b == id && t.in_offset.0 == 0 => Some(t.out_offset.0),
            // фейд в конце: пустота справа
            (false, Some(a), None) if a == id && t.out_offset.0 == 0 => Some(t.in_offset.0),
            _ => None,
        }
    })
}

/// Позиция ручки фейда (тик): у края события, когда фейда нет, и на
/// внутреннем конце фейда, когда он есть. Чистая функция — тест 173.
pub(crate) fn fade_handle_tick(start: i64, end: i64, len: i64, left: bool) -> i64 {
    if left { start + len.max(0) } else { end - len.max(0) }
}

/// Непрозрачность из положения мыши в строке дорожки (верх = 1.0).
/// Чистая функция — тест 173.
pub(crate) fn level_from_y(y: f32, top: f32, lane_h: f32) -> f32 {
    let h = (lane_h - 8.0).max(1.0);
    (1.0 - (y - (top + 3.0)) / h).clamp(0.0, 1.0)
}

/// Уровень события 0..1 для линии: у видео это непрозрачность, у аудио —
/// громкость в диапазоне рейла v1 (±6.02 dB; 0 dB — середина). Один
/// элемент на оба вида (требование «фейды у аудио и видео одинаково
/// выглядящие»: вертикальный рейл стоял внутри зоны фейда).
pub(crate) fn clip_level(c: &kt_model::ClipEvent) -> f32 {
    if c.kind == ClipKind::Audio { gain_to_level(c.gain_db) } else { c.opacity }
}

pub(crate) fn gain_to_level(gain_db: f32) -> f32 {
    (0.5 + gain_db / RAIL_DB_SPAN).clamp(0.0, 1.0)
}

pub(crate) fn level_to_gain(level: f32) -> f32 {
    (level.clamp(0.0, 1.0) - 0.5) * RAIL_DB_SPAN
}

/// Обратное преобразование: y линии уровня.
pub(crate) fn level_y(opacity: f32, top: f32, lane_h: f32) -> f32 {
    let h = (lane_h - 8.0).max(1.0);
    top + 3.0 + (1.0 - opacity.clamp(0.0, 1.0)) * h
}

/// Препятствие текущего жеста (п.5 Р.28): что мешает и где оно лежит.
/// Живёт один кадр — ставится обработчиком драга, гасится в начале
/// следующего прохода.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Blocked {
    /// Тики препятствия [start, end).
    pub span: (i64, i64),
    pub kind: ClipKind,
    pub lane: u32,
}

/// Видно ли препятствие в текущем окне и, если нет, куда смотреть.
/// Чистая функция — тест 172.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum BlockView {
    Visible,
    /// Препятствие левее вида; тики до его ПРАВОГО края.
    Left(i64),
    /// Препятствие правее вида; тики до его ЛЕВОГО края.
    Right(i64),
}

pub(crate) fn block_view(span: (i64, i64), view: (i64, i64)) -> BlockView {
    if span.1 <= view.0 {
        BlockView::Left(view.0 - span.1)
    } else if span.0 >= view.1 {
        BlockView::Right(span.0 - view.1)
    } else {
        BlockView::Visible
    }
}

/// Сдвиг материала внутри куска для ПРЕВЬЮ жеста (волноформа и филмстрип).
/// Его даёт ТОЛЬКО трим левого края: там кусок стоит на месте, а край едет
/// по материалу. При переносе клип едет целиком — материал обязан ехать
/// вместе с ним (п.2 Р.28: «перетаскиваю событие вглубь предыдущего — превью
/// звука прокручивается влево, хотя при воспроизведении сам звук не
/// меняется»). Трим правого края левой границы не двигает. Тест 169.
pub(crate) fn preview_src_shift(drag: Option<&Drag>, id: ClipId, clip_start: i64) -> i64 {
    match drag {
        Some(Drag::Clip(d)) if d.id == id && d.mode == DragMode::TrimStart => {
            d.cur_start - clip_start
        }
        _ => 0,
    }
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
    let spans: Vec<(i64, i64)> = hits
        .iter()
        .map(|c| (c.start.0, c.start.0 + c.duration.0))
        .chain(foreign.iter().copied())
        .collect();
    // препятствие — то, что первым встретилось ПО ХОДУ движения
    let blocker = if want >= orig {
        spans.iter().copied().min_by_key(|(s, _)| *s)
    } else {
        spans.iter().copied().max_by_key(|(_, e)| *e)
    };
    let start = match blocker {
        Some((s, _)) if want >= orig => (s - dur).clamp(orig.min(want), want),
        Some((_, e)) => e.clamp(want, orig),
        None => want,
    };
    MovePlan::Clamped { start, blocker: blocker.unwrap_or((want, want + dur)) }
}

/// Метка-диапазон под кликом в руллере (Р.33 п.8). Сперва флажок/имя в
/// пределах `tol` — «по названию региона», как просил пользователь; иначе
/// покрывающий диапазон с БЛИЖАЙШИМ флажком слева: при наложении
/// нескольких берётся тот, чью подпись видно в этом месте ленты.
/// Возвращает начало метки (её идентичность в модели). Чистая — тест 207.
pub(crate) fn pick_range_marker(markers: &[Marker], click: i64, tol: i64) -> Option<i64> {
    let by_flag = markers
        .iter()
        .filter(|m| m.is_range() && (m.t.0 - click).abs() <= tol)
        .min_by_key(|m| (m.t.0 - click).abs());
    if let Some(m) = by_flag {
        return Some(m.t.0);
    }
    markers
        .iter()
        .filter(|m| m.is_range() && m.t.0 <= click && click <= m.end().0)
        .max_by_key(|m| m.t.0)
        .map(|m| m.t.0)
}

/// План переноса ПАЧКИ событий (Р.33 п.7): общий сдвиг по времени и
/// препятствие, если блок в него упёрся. `delta == want` — путь свободен.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BlockPlan {
    pub delta: i64,
    pub blocker: Option<((i64, i64), ClipKind, u32)>,
}

/// Кламп переноса пачки: блок едет ЦЕЛИКОМ, взаимные сдвиги внутри него не
/// меняются (требование п.6 Р.28 — «ничего не отрастает и не отрубается»),
/// поэтому упор считается по первому контакту любого участника с чужим
/// событием своей дорожки. При переслаивании (умолчание проекта) мешают
/// только чужие плейсхолдеры — их накладывать нельзя никогда.
pub(crate) fn plan_move_block(p: &Project, moving: &[ClipId], want: i64) -> BlockPlan {
    let block: Vec<&ClipEvent> = p.clips.iter().filter(|c| moving.contains(&c.id)).collect();
    if block.is_empty() {
        return BlockPlan { delta: want, blocker: None };
    }
    // за ноль блок не уезжает
    let min_start = block.iter().map(|c| c.start.0).min().unwrap_or(0);
    let mut delta = want.max(-min_start);
    let mut blocker = None;
    for m in &block {
        let (s, e) = (m.start.0, m.start.0 + m.duration.0);
        let others = p
            .clips
            .iter()
            .filter(|c| !moving.contains(&c.id) && c.kind == m.kind && c.lane == m.lane)
            .map(|c| (c.start.0, c.start.0 + c.duration.0, false))
            .chain(
                p.foreign
                    .iter()
                    .filter(|f| f.kind == m.kind && f.lane == m.lane)
                    .map(|f| (f.start.0, (f.start + f.duration).0, true)),
            );
        for (o0, o1, foreign) in others {
            if p.overlap_layers && !foreign {
                continue; // перекрытия законны — упора нет
            }
            if delta > 0 && o0 >= e {
                let cap = o0 - e;
                if cap < delta {
                    delta = cap;
                    blocker = Some(((o0, o1), m.kind, m.lane));
                }
            } else if delta < 0 && o1 <= s {
                let cap = o1 - s;
                if cap > delta {
                    delta = cap;
                    blocker = Some(((o0, o1), m.kind, m.lane));
                }
            }
        }
    }
    BlockPlan { delta, blocker }
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
    /// Прочие выделенные события (Р.33 п.7): id и ИСХОДНЫЙ старт. Пусто —
    /// обычный одиночный драг со всей его семантикой (кроссфейд из наезда,
    /// перенос по дорожкам). Непусто — блок едет только по времени.
    pub extra: Vec<(ClipId, i64)>,
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
    /// Ручка фейда у края события (Р.28, п.12): длина в тиках, живой
    /// предпросмотр; на отпускании — AddTransition/SetTransition/Remove.
    Fade {
        id: ClipId,
        left: bool,
        len: i64,
        moved: bool,
    },
    /// Линия уровня видео — непрозрачность (Р.28, п.12).
    Level {
        id: ClipId,
        moved: bool,
    },
    /// Нижняя кромка строки в gutter'е (Р.38 п.11): индивидуальная
    /// высота дорожки, как в Vegas. `cur` — высота в пикселях.
    LaneHeight {
        row: usize,
        cur: f32,
    },
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
    /// Рамка выделения событий (Р.33 п.7): Ctrl+драг с пустого места, а в
    /// режиме-инструменте — обычный драг. `add` = добавлять к выделению,
    /// иначе рамка заменяет его целиком.
    Rubber {
        anchor: egui::Pos2,
        add: bool,
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
    /// Перенос morph-блока по времени (H1; в v1 блок был недвижим).
    Morph {
        id: MorphId,
        grab_dt: i64,
        cur_start: i64,
        moved: bool,
    },
    /// Край morph-блока (Р.38 п.9, дословно: «я не понимаю как менять
    /// длинну у морфер события»). Левый двигает начало при стоящем
    /// конце, правый меняет длительность — как у метки-диапазона.
    MorphEdge {
        id: MorphId,
        left: bool,
        cur: i64,
    },
    /// Край камеры-события (Р.38 п.5): до этого раунда длительность
    /// камеры была read-only подписью в окне и менять её было нечем.
    CameraEdge {
        id: kt_model::CameraId,
        left: bool,
        cur: i64,
    },
    /// Перенос камеры-события (Р.34 К3) — по образцу Morph.
    Camera {
        id: kt_model::CameraId,
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
    /// Правый клик по заголовку камерной дорожки (Р.29).
    CameraLane(u32),
    /// Правый клик по камере-событию (Р.34 К3).
    Camera(kt_model::CameraId),
    /// Правый клик по именованной области (Р.38 п.1). Индекс в
    /// `project().areas`, а не имя: цель обязана оставаться `Copy`.
    /// Читается через `get` — область могли удалить из-под меню.
    Area(usize),
    Empty,
}

/// Сущность таймлайна, которой пользователь управляет мышью (Р.38 п.253).
///
/// Существует ради одной цели: заставить КОМПИЛЯТОР спросить про новую
/// сущность «а какими глаголами ей управляют?». Массив `ALL` фиксированной
/// длины ломает СБОРКУ при добавлении варианта, а `verbs()` в тесте 253 —
/// исчерпывающий `match`, поэтому строку про глаголы не пропустить молча.
/// Повод — Р.35: именованные области нарисовали, а удалить и переименовать
/// их было нечем, и все тесты были зелёными.
// Тип живёт ради КОМПИЛЯТОРА (см. доккомментарий) и читается тестом
// достижимости 253 — в продуктовом пути его никто не зовёт, и это норма.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum TlEntity {
    Clip = 0,
    Marker,
    Area,
    Morpher,
    Camera,
    ClipLane,
    MorphLane,
    CameraLane,
    Region,
}

#[allow(dead_code)]
impl TlEntity {
    pub const N: usize = TlEntity::Region as usize + 1;
    /// Длина массива задана `N`: добавил вариант — литерал перестал
    /// соответствовать длине, и не собирается ВСЁ приложение, а не только
    /// тесты.
    pub const ALL: [TlEntity; Self::N] = [
        TlEntity::Clip,
        TlEntity::Marker,
        TlEntity::Area,
        TlEntity::Morpher,
        TlEntity::Camera,
        TlEntity::ClipLane,
        TlEntity::MorphLane,
        TlEntity::CameraLane,
        TlEntity::Region,
    ];
}

#[allow(dead_code)]
impl CtxTarget {
    /// Какой сущности принадлежит цель меню. Match исчерпывающий: завёл
    /// новую цель — обязан назвать её сущность (или явно сказать «ничья»).
    pub fn entity(self) -> Option<TlEntity> {
        match self {
            CtxTarget::Clip(_) => Some(TlEntity::Clip),
            CtxTarget::Marker(_) => Some(TlEntity::Marker),
            CtxTarget::RegionEdge(_) => Some(TlEntity::Region),
            CtxTarget::Lane { .. } => Some(TlEntity::ClipLane),
            CtxTarget::Morpher(_) => Some(TlEntity::Morpher),
            CtxTarget::MorphLane(_) => Some(TlEntity::MorphLane),
            CtxTarget::CameraLane(_) => Some(TlEntity::CameraLane),
            CtxTarget::Camera(_) => Some(TlEntity::Camera),
            CtxTarget::Area(_) => Some(TlEntity::Area),
            CtxTarget::Empty => None,
        }
    }
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
    /// Высота строки КЛИПОВ (видео/аудио). Служебные строки ниже втрое.
    pub base_h: f32,
    pub content_h: f32,
    pub scroll_max: f32,
}

/// Во столько раз служебная строка (morph/camera) ниже строки клипов —
/// просьба пользователя Р.35 п.1: «чтобы не терять место — треки-морферы
/// можно уменьшить по высоте в три раза».
pub const THIN_ROW_DIV: f32 = 3.0;
/// Делитель МОРФ-строки (Р.38 п.10, дословно: «увеличь высоту морфер
/// трека по дефолту на 2»). Морф-блок несёт цепочку эффектов и подпись,
/// камерный — только имя, поэтому втрое ниже осталась камерная строка.
/// Индивидуальную высоту можно задать каждой дорожке (п.11).
pub const MORPH_ROW_DIV: f32 = 1.5;
/// Пол высоты служебной строки: 84/3 = 28 нормально, а 16/3 = 5 — в такую
/// строку мышью не попасть (п.1 чек-листа UI).
pub const THIN_ROW_MIN: f32 = 10.0;

/// Потолок индивидуальной высоты строки (Р.38 п.11). Предел есть у
/// всего, что тянется мышью (правило 6 чек-листа).
pub const ROW_H_MAX: f32 = 400.0;
/// Полоса хвата нижней кромки строки в gutter'е. Шире рисунка (правило 1).
pub const ROW_EDGE_PX: f32 = 5.0;

/// Высота строки по её виду. ЕДИНСТВЕННОЕ место, где живёт это правило.
pub fn row_height(kind: LaneKind, base_h: f32) -> f32 {
    match kind {
        LaneKind::Video | LaneKind::Audio => base_h,
        LaneKind::Morph => (base_h / MORPH_ROW_DIV).max(THIN_ROW_MIN),
        LaneKind::Camera => (base_h / THIN_ROW_DIV).max(THIN_ROW_MIN),
    }
}

/// Высота строки с учётом ИНДИВИДУАЛЬНОЙ (Р.38 п.11, дословно: «разреши
/// менять высоту каждого трека индивидуально»). Переопределение — в
/// пикселях и абсолютное: вертикальный зум его не трогает, иначе
/// «поставил дорожке высоту» переставало держаться при первом же зуме.
pub fn row_height_of(
    kind: LaneKind,
    lane: u32,
    base_h: f32,
    over: &[(LaneKind, u32, f32)],
) -> f32 {
    match over.iter().find(|(k, l, _)| *k == kind && *l == lane) {
        Some((_, _, h)) => h.clamp(THIN_ROW_MIN, ROW_H_MAX),
        None => row_height(kind, base_h),
    }
}

/// Префиксные суммы верхов строк; длина — `rows.len() + 1`, последний
/// элемент = полная высота содержимого.
pub fn row_offsets(
    rows: &[(LaneKind, u32)],
    base_h: f32,
    over: &[(LaneKind, u32, f32)],
) -> Vec<f32> {
    let mut out = Vec::with_capacity(rows.len() + 1);
    let mut y = 0.0;
    out.push(0.0);
    for (kind, lane) in rows {
        y += row_height_of(*kind, *lane, base_h, over);
        out.push(y);
    }
    out
}

/// `clip_lanes` — строки клипов, `thin_lanes` — служебные (они дают
/// втрое меньший вклад, иначе десяток морф-дорожек плющил бы видео).
pub fn vertical_layout(
    avail_h: f32,
    clip_lanes: usize,
    morph_lanes: usize,
    camera_lanes: usize,
    v_zoom: Option<f32>,
) -> VLayout {
    let clip_lanes = clip_lanes.max(1) as f32;
    let usable = (avail_h - RULER_H - 6.0).max(0.0);
    // вес служебных строк в делителе — их доля от строки клипов; у морф-
    // и камерных строк доли РАЗНЫЕ (п.10 Р.38)
    let weight = clip_lanes
        + morph_lanes as f32 / MORPH_ROW_DIV
        + camera_lanes as f32 / THIN_ROW_DIV;
    let base_h = match v_zoom {
        Some(z) => z.clamp(16.0, 120.0),
        None => (usable / weight).clamp(30.0, 84.0),
    };
    let content_h = clip_lanes * base_h
        + morph_lanes as f32 * (base_h / MORPH_ROW_DIV).max(THIN_ROW_MIN)
        + camera_lanes as f32 * (base_h / THIN_ROW_DIV).max(THIN_ROW_MIN);
    VLayout { base_h, content_h, scroll_max: (content_h - usable).max(0.0) }
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
                    "{{\"kind\":\"{}\",\"lane\":{lane},\"top\":{:.1},\"center\":{:.1},\"h\":{:.1}}}",
                    match kind {
                        kt_model::LaneKind::Video => "video",
                        kt_model::LaneKind::Audio => "audio",
                        kt_model::LaneKind::Morph => "morph",
                        kt_model::LaneKind::Camera => "camera",
                    },
                    geom.row_top(i),
                    geom.row_center_y(i),
                    geom.row_h(i),
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
            geom.base_h,
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
    /// Высота строки КЛИПОВ. Высота КОНКРЕТНОЙ строки — `row_h`:
    /// служебные (morph/camera) втрое ниже (Р.35 п.1).
    pub base_h: f32,
    /// Display-порядок строк сверху вниз (нормализован).
    pub rows: std::sync::Arc<Vec<(LaneKind, u32)>>,
    /// Верхи строк (префиксные суммы). Приватное: разойтись с `rows` оно
    /// физически не может — обе заполняет `new`.
    offs: std::sync::Arc<Vec<f32>>,
    /// Высоты строк с учётом индивидуальных (Р.38 п.11). Тоже приватное и
    /// тоже из `new`: «сколько строка занимает» обязано быть согласовано
    /// с её верхом.
    hs: std::sync::Arc<Vec<f32>>,
    /// Вертикальный скролл-офсет зоны дорожек (п.12 р.6).
    pub view_top: f32,
    pub view_start: f64,
    pub tpp: f64,
}

impl TlGeom {
    pub fn new(
        rect: egui::Rect,
        base_h: f32,
        rows: Vec<(LaneKind, u32)>,
        over: &[(LaneKind, u32, f32)],
        view_top: f32,
        view_start: f64,
        tpp: f64,
    ) -> TlGeom {
        let offs = row_offsets(&rows, base_h, over);
        let hs = rows.iter().map(|(k, l)| row_height_of(*k, *l, base_h, over)).collect();
        TlGeom {
            rect,
            base_h,
            rows: std::sync::Arc::new(rows),
            offs: std::sync::Arc::new(offs),
            hs: std::sync::Arc::new(hs),
            view_top,
            view_start,
            tpp,
        }
    }

    /// Высота строки по индексу.
    pub fn row_h(&self, row: usize) -> f32 {
        self.hs.get(row).copied().unwrap_or(self.base_h)
    }

    /// Высота строки дорожки клипов (её вид — всегда базовый).
    pub fn lane_h(&self) -> f32 {
        self.base_h
    }

    /// Середина строки — куда садятся подписи gutter'а.
    pub fn row_center_y(&self, row: usize) -> f32 {
        self.row_top(row) + self.row_h(row) * 0.5
    }

    /// Точка внутри строки дорожки клипов в долях её высоты. Тесты и
    /// живые сценарии обязаны считать y ИМЕННО так: своя формула
    /// «lane_top + lane_h*f» ломается на строках разной высоты.
    /// (Отрисовка держит `top` и `lane_h` в локальных переменных, поэтому
    /// в продуктовом коде вызовов нет — метод существует для тех, кто
    /// геометрию ЧИТАЕТ снаружи.)
    #[allow(dead_code)]
    pub fn lane_y(&self, kind: ClipKind, lane: u32, frac: f32) -> f32 {
        let lk = match kind {
            ClipKind::Video => LaneKind::Video,
            ClipKind::Audio => LaneKind::Audio,
        };
        let h = self.row_of((lk, lane)).map(|r| self.row_h(r)).unwrap_or(self.base_h);
        self.lane_top(kind, lane) + h * frac
    }

    /// Индекс строки под y БЕЗ ограничения снизу: строки перебираются
    /// сверху вниз, а всё, что ниже последней, продолжается её высотой.
    /// Нужен драгу клипа — там «сколько строк проехали» считалось
    /// делением на `lane_h`, и на строках разной высоты оно врало.
    pub fn row_at_y_unbounded(&self, y: f32) -> i64 {
        let rel = y - self.lanes_top();
        if rel < 0.0 {
            let last = self.base_h.max(1.0);
            return (rel / last).floor() as i64;
        }
        match self.offs.partition_point(|&o| o <= rel) {
            0 => 0,
            i if i <= self.rows.len() => i as i64 - 1,
            _ => {
                let over = rel - self.offs[self.rows.len()];
                self.rows.len() as i64 + (over / self.base_h.max(1.0)).floor() as i64
            }
        }
    }

    /// Строка, в которую упирается вертикальный зум (якорь под курсором),
    /// в ДОЛЯХ строк: целая часть — индекс, дробная — место внутри.
    pub fn anchor_at(&self, y: f32) -> f32 {
        let rel = (y - self.lanes_top()).max(0.0);
        let i = self.row_at_y_unbounded(y).max(0) as usize;
        let top = self.offs.get(i).copied().unwrap_or(rel);
        let h = self.row_h(i).max(1.0);
        i as f32 + (rel - top) / h
    }
    pub fn x_to_tick(&self, x: f32) -> i64 {
        (self.view_start + (x - self.rect.left()) as f64 * self.tpp) as i64
    }

    pub fn tick_to_x(&self, t: i64) -> f32 {
        self.rect.left() + ((t as f64 - self.view_start) / self.tpp) as f32
    }

    pub fn lanes_top(&self) -> f32 {
        self.rect.top() + RULER_H + 3.0 - self.view_top
    }

    /// Индекс строки под y (в пределах rows).
    pub fn row_at_y(&self, y: f32) -> Option<usize> {
        let rel = y - self.lanes_top();
        if rel < 0.0 || rel >= *self.offs.last().unwrap_or(&0.0) {
            return None;
        }
        Some(self.offs.partition_point(|&o| o <= rel).saturating_sub(1))
    }

    pub fn key_at_y(&self, y: f32) -> Option<(LaneKind, u32)> {
        self.rows.get(self.row_at_y(y)?).copied()
    }

    /// Дорожка КЛИПОВ под y (morph/camera-строка/руллер → None).
    pub fn lane_at_y(&self, y: f32) -> Option<(ClipKind, u32)> {
        match self.key_at_y(y)? {
            (LaneKind::Video, l) => Some((ClipKind::Video, l)),
            (LaneKind::Audio, l) => Some((ClipKind::Audio, l)),
            (LaneKind::Morph | LaneKind::Camera, _) => None,
        }
    }

    pub fn morph_lane_at_y(&self, y: f32) -> Option<u32> {
        match self.key_at_y(y)? {
            (LaneKind::Morph, l) => Some(l),
            _ => None,
        }
    }

    pub fn camera_lane_at_y(&self, y: f32) -> Option<u32> {
        match self.key_at_y(y)? {
            (LaneKind::Camera, l) => Some(l),
            _ => None,
        }
    }

    /// Прямоугольник именованной области на экране. Одна формула на
    /// отрисовку И хит-тест: разойдись они — контур рисуется в одном
    /// месте, а хватается в другом (правило «геометрия только через
    /// TlGeom»). `None` — область целиком за пределами вида.
    pub fn area_rect(&self, a: &kt_model::MarkedArea) -> Option<egui::Rect> {
        let x0 = self.tick_to_x(a.start.0);
        let x1 = self.tick_to_x((a.start + a.duration).0).max(x0 + 6.0);
        if x1 < self.rect.left() || x0 > self.rect.right() {
            return None;
        }
        let keys: Vec<(LaneKind, u32)> =
            if a.lanes.is_empty() { self.rows.to_vec() } else { a.lanes.clone() };
        let (mut top, mut bot) = (f32::MAX, f32::MIN);
        for k in &keys {
            if let Some(row) = self.row_of(*k) {
                top = top.min(self.row_top(row));
                bot = bot.max(self.row_top(row) + self.row_h(row));
            }
        }
        if top > bot {
            return None;
        }
        Some(egui::Rect::from_min_max(
            egui::pos2(x0.max(self.rect.left()), top + 1.0),
            egui::pos2(x1.min(self.rect.right()), bot - 1.0),
        ))
    }

    pub fn row_of(&self, key: (LaneKind, u32)) -> Option<usize> {
        self.rows.iter().position(|&k| k == key)
    }

    pub fn row_top(&self, row: usize) -> f32 {
        let off = match self.offs.get(row) {
            Some(o) => *o,
            // за последней строкой продолжаем базовой высотой
            None => {
                self.offs.last().copied().unwrap_or(0.0)
                    + (row.saturating_sub(self.rows.len())) as f32 * self.base_h
            }
        };
        self.lanes_top() + off
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

    /// Ключи строк, задетых вертикальным отрезком [y0, y1].
    pub fn rows_in_band(&self, y0: f32, y1: f32) -> Vec<(LaneKind, u32)> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                let top = self.row_top(*i);
                top < y1 && top + self.row_h(*i) > y0
            })
            .map(|(_, k)| *k)
            .collect()
    }

    /// Точка внутри полосы дорожек (не gutter/не руллер)?
    pub fn in_lanes(&self, pos: egui::Pos2) -> bool {
        self.rect.contains(pos) && pos.y > self.rect.top() + RULER_H
    }
}

/// События, попавшие в рамку выделения (Р.33 п.7). Чистая функция: рамка
/// в координатах окна, геометрия — только через `TlGeom`, как требует
/// инвариант маппинга. Событие берётся при ЛЮБОМ пересечении с рамкой —
/// целиться в него целиком пользователь не обязан (п.1 чек-листа).
pub fn rubber_hits(geom: &TlGeom, p: &Project, band: egui::Rect) -> Vec<ClipId> {
    p.clips
        .iter()
        .filter(|c| {
            let top = geom.lane_top(c.kind, c.lane);
            let r = egui::Rect::from_min_max(
                egui::pos2(geom.tick_to_x(c.start.0), top),
                egui::pos2(geom.tick_to_x((c.start + c.duration).0), top + geom.lane_h()),
            );
            band.intersects(r)
        })
        .map(|c| c.id)
        .collect()
}

impl KtApp {
    /// Все события, которые едут с этим драгом: якорь, его прочие
    /// выделенные соседи и link-партнёры всех перечисленных.
    pub(crate) fn moving_block_ids(&self, d: &DragState) -> Vec<ClipId> {
        let p = self.model.project();
        let seeds: Vec<ClipId> =
            std::iter::once(d.id).chain(d.extra.iter().map(|(id, _)| *id)).collect();
        let groups: Vec<GroupId> =
            p.clips.iter().filter(|c| seeds.contains(&c.id)).filter_map(|c| c.link_group).collect();
        p.clips
            .iter()
            .filter(|c| seeds.contains(&c.id) || c.link_group.is_some_and(|g| groups.contains(&g)))
            .map(|c| c.id)
            .collect()
    }

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
        // ДВА счётчика: строки клипов и служебные (они втрое ниже).
        // Раньше камерные дорожки тут не считались вовсе, а в layout
        // считались — вертикальный зум колесом брал не ту высоту.
        let (clip_hint, morph_hint, cam_hint) = {
            let (v, a) = self.lane_counts();
            let p = self.model.project();
            ((v + a).max(1) as usize, p.morph_lanes as usize, p.camera_lanes as usize)
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
                        vertical_layout(rect.height(), clip_hint, morph_hint, cam_hint, None).base_h
                    });
                let factor = if raw_y > 0.0 { 1.15 } else { 1.0 / 1.15 };
                let new_h = (cur_h * factor).clamp(16.0, 120.0);
                // якорь — строка ПОД КУРСОРОМ в долях строк; делением на
                // одну высоту его не посчитать: строки разной высоты
                let row = self
                    .tl_geom
                    .as_ref()
                    .map(|g| g.anchor_at(pos.y))
                    .unwrap_or_else(|| {
                        ((pos.y - (rect.top() + RULER_H + 3.0) + self.tl_scroll) / cur_h).max(0.0)
                    })
                    .max(0.0);
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
        let camera_lanes = self.model.project().camera_lanes;
        let vl = vertical_layout(
            rect.height(),
            (video_lanes + audio_lanes).max(1) as usize,
            morph_lanes as usize,
            camera_lanes as usize,
            self.tl_v_zoom,
        );
        let lane_h = vl.base_h;
        // высота служебной строки (morph/camera): втрое меньше, но не ниже
        // пола — в строку в 5 px мышью не попасть
        let thin_h = row_height(LaneKind::Morph, lane_h);
        // кламп скролла каждый кадр (удаление дорожек/ресайз панели)
        self.tl_scroll = self.tl_scroll.clamp(0.0, vl.scroll_max);
        // display-порядок строк: с Р.35 он живёт в МОДЕЛИ (морфер-слой
        // действует «на всё, что ниже»), а `rows()` нормализует его к
        // фактическим счётчикам дорожек на каждый кадр
        let rows = self.model.project().rows();
        // кэш геометрии для маппинга вне draw_timeline (дроп п.2, строки п.6)
        let row_over = self.lane_heights.clone();
        let geom = TlGeom::new(
            rect,
            lane_h,
            rows,
            &row_over,
            self.tl_scroll,
            self.view_start,
            self.ticks_per_px,
        );
        self.tl_geom = Some(geom.clone());
        // KT_GEOM_DUMP=<файл> — геометрия таймлайна наружу для живых
        // сценариев (Р.27): без неё скрипт вынужден УГАДЫВАТЬ, где дорожка и
        // сколько пикселей в секунде, и ломается от любой правки вёрстки.
        // Только по env, не чаще 10 Гц, атомарной заменой файла.
        self.dump_geom(&geom, ui.ctx().content_rect());
        let lane_top = |kind: ClipKind, lane: u32| -> f32 { geom.lane_top(kind, lane) };
        let morph_lane_top = |lane: u32| -> f32 { geom.morph_lane_top(lane) };
        // (подпись, центр строки, верх строки) — верх нужен, чтобы длинный
        // ярлык встал ПОД именем клипа, а не поверх него
        let mut gutter_labels: Vec<(String, f32, f32, f32)> =
            Vec::with_capacity(geom.rows.len());
        for (row, &(kind, lane)) in geom.rows.iter().enumerate() {
            let y = geom.row_top(row);
            // высота ЭТОЙ строки: служебные втрое ниже (Р.35 п.1)
            let rh = geom.row_h(row);
            // фон дорожки тянется ПОД gutter — градиент ниже «растворяет»
            // номерную полосу в дорожках (п.20 р.6)
            let lane_rect = egui::Rect::from_min_max(
                egui::pos2(full.left(), y),
                egui::pos2(rect.right(), y + rh - 2.0),
            );
            // фоны дорожек в тонах старого: видео синеватый, аудио тёплый,
            // morph фиолетовый (темнее — тема р.6)
            let fill = match kind {
                LaneKind::Video => crate::theme::LANE_VIDEO,
                LaneKind::Audio => crate::theme::LANE_AUDIO,
                LaneKind::Morph => crate::theme::LANE_MORPH,
                LaneKind::Camera => crate::theme::LANE_CAMERA,
            };
            // подпись: пользовательское имя либо формула V1/A1/M1 (п.2 р.10)
            let label = kt_model::lane_label(&self.lane_names, kind, lane);
            painter.rect_filled(lane_rect, 0.0, fill);
            gutter_labels.push((label, y + rh * 0.5, y, rh));
        }
        // хит-тест morph-блока (нужен и press-обработке, и ПКМ ниже)
        // кромка morph-блока: та же зона хвата, что у тримов клипа
        let morph_edge_at = |slf: &KtApp, pos: egui::Pos2| -> Option<(MorphId, bool)> {
            let lane = geom.morph_lane_at_y(pos.y)?;
            slf.model.project().morphers.iter().rev().filter(|m| m.lane == lane).find_map(|m| {
                let x0 = tick_to_x(m.start.0);
                let x1 = tick_to_x((m.start + m.duration).0).max(x0 + 8.0);
                if (pos.x - x0).abs() <= EDGE_GRAB_PX {
                    Some((m.id, true))
                } else if (pos.x - x1).abs() <= EDGE_GRAB_PX {
                    Some((m.id, false))
                } else {
                    None
                }
            })
        };
        let camera_edge_at = |slf: &KtApp, pos: egui::Pos2| -> Option<(kt_model::CameraId, bool)> {
            let lane = geom.camera_lane_at_y(pos.y)?;
            slf.model.project().cameras.iter().rev().filter(|c| c.lane == lane).find_map(|c| {
                let x0 = tick_to_x(c.start.0);
                let x1 = tick_to_x((c.start + c.duration).0).max(x0 + 8.0);
                if (pos.x - x0).abs() <= EDGE_GRAB_PX {
                    Some((c.id, true))
                } else if (pos.x - x1).abs() <= EDGE_GRAB_PX {
                    Some((c.id, false))
                } else {
                    None
                }
            })
        };
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
        // хит-тест камеры-события (Р.34 К3): до этого раунда камеру нельзя
        // было ни выделить, ни перенести, ни открыть — она рисовалась и всё
        let camera_at = |slf: &KtApp, pos: egui::Pos2| -> Option<kt_model::CameraId> {
            let lane = geom.camera_lane_at_y(pos.y)?;
            slf.model
                .project()
                .cameras
                .iter()
                .rev()
                .filter(|c| c.lane == lane)
                .find(|c| {
                    let x0 = tick_to_x(c.start.0);
                    let x1 = tick_to_x((c.start + c.duration).0).max(x0 + 8.0);
                    pos.x >= x0 && pos.x <= x1
                })
                .map(|c| c.id)
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
            // нижняя кромка строки в gutter'е тянет её ВЫСОТУ (Р.38 п.11).
            // Проверяется РАНЬШЕ реордера: кромка узкая, и реордер, начатый
            // с неё, забрал бы жест себе.
            let row_edge_at = |g: &TlGeom, y: f32| -> Option<usize> {
                (0..g.rows.len()).find(|r| (g.row_top(*r) + g.row_h(*r) - y).abs() <= ROW_EDGE_PX)
            };
            if self.lane_edit.is_none()
                && let Some(pos) = gutter_resp.hover_pos()
                && row_edge_at(&geom, pos.y).is_some()
            {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                self.hover_hint = Some(tr(k::tl::hint::ROW_HEIGHT).to_string());
            }
            // Зона берётся по точке НАЖАТИЯ, а не по текущему указателю:
            // `drag_started` приходит уже после сдвига на порог, и указатель
            // успевает выползти из узкой полосы кромки — жест превращался бы
            // в реордер. Тот же урок, что у тримов клипа (п.28).
            let gutter_press = ui.input(|i| i.pointer.press_origin());
            if self.lane_edit.is_none()
                && gutter_resp.drag_started()
                && let Some(pos) = gutter_press.or_else(|| gutter_resp.interact_pointer_pos())
                && let Some(row) = row_edge_at(&geom, pos.y)
            {
                self.drag = Some(Drag::LaneHeight { row, cur: geom.row_h(row) });
            } else if self.lane_edit.is_none()
                && gutter_resp.drag_started()
                && let Some(pos) = gutter_press.or_else(|| gutter_resp.interact_pointer_pos())
                && let Some(row) = geom.row_at_y(pos.y)
            {
                self.drag = Some(Drag::LaneRow { from: row, cur: row });
            }
            if gutter_resp.dragged()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
                && let Some(Drag::LaneHeight { row, .. }) = self.drag
            {
                let cur = (pos.y - geom.row_top(row)).clamp(THIN_ROW_MIN, ROW_H_MAX);
                self.drag = Some(Drag::LaneHeight { row, cur });
            }
            if gutter_resp.drag_stopped()
                && let Some(Drag::LaneHeight { row, cur }) = self.drag.take()
                && let Some(key) = geom.rows.get(row).copied()
            {
                self.set_row_height(key, Some(cur));
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
                        egui::pos2(rect.right(), y + geom.row_h(cur) - 2.0),
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
                    Some((LaneKind::Camera, lane)) => {
                        self.ctx_target = Some(CtxTarget::CameraLane(lane));
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
                        ui.close();
                    }
                    if ui.button(tr(k::tl::lane::RENAME)).clicked() {
                    let cur = kt_model::lane_label(&self.lane_names, lk, lane);
                    self.lane_edit = Some(crate::app::LaneEdit { kind: lk, lane, buf: cur });
                    self.lane_edit_focus = true;
                    ui.close();
                }
                if self.lane_heights.iter().any(|(k, l, _)| (*k, *l) == (lk, lane))
                    && ui.button(tr(k::tl::lane::RESET_HEIGHT)).clicked()
                {
                    self.set_row_height((lk, lane), None);
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
                    // симметрично камерной дорожке (правило 17): парные
                    // способы одного действия ведут себя одинаково, а до
                    // Р.38 морфер на существующую дорожку положить было
                    // НЕЧЕМ — «add morpher layer» всегда заводил новую
                    if ui.button(tr(k::tl::lane::ADD_MORPHER_HERE)).clicked() {
                        self.add_morpher_at(lane);
                        ui.close();
                    }
                    if !self.lane_heights.iter().any(|(k, l, _)| (*k, *l) == (LaneKind::Morph, lane)) {
                        // сбрасывать нечего — пункт молча ничего не сделал бы
                    } else if ui.button(tr(k::tl::lane::RESET_HEIGHT)).clicked() {
                        self.set_row_height((LaneKind::Morph, lane), None);
                        ui.close();
                    }
                    // переименование — как у обычной дорожки (правило 17):
                    // двойной клик по заголовку переименовывает СТРОКУ
                    // ЛЮБОГО вида, а пункта меню у служебных не было
                    if ui.button(tr(k::tl::lane::RENAME)).clicked() {
                        let cur = kt_model::lane_label(&self.lane_names, LaneKind::Morph, lane);
                        self.lane_edit =
                            Some(crate::app::LaneEdit { kind: LaneKind::Morph, lane, buf: cur });
                        self.lane_edit_focus = true;
                        ui.close();
                    }
                    let del_label = if n > 0 {
                        crate::tf!(k::tl::lane::DELETE_MORPH_N, n)
                    } else {
                        tr(k::tl::lane::DELETE).to_string()
                    };
                    if ui.button(del_label).clicked() {
                        if self.apply(Command::RemoveMorphLane { at: lane }) {
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
                if let Some(CtxTarget::CameraLane(lane)) = self.ctx_target {
                    let n =
                        self.model.project().cameras.iter().filter(|c| c.lane == lane).count();
                    ui.label(
                        egui::RichText::new(crate::tf!(k::tl::lane::CAMERA_TITLE, lane + 1))
                            .strong(),
                    );
                    // камера ставится под курсор ВОСПРОИЗВЕДЕНИЯ (как метка)
                    if ui.button(tr(k::tl::lane::ADD_CAMERA_HERE)).clicked() {
                        self.add_camera_at_playhead(lane);
                        ui.close();
                    }
                    if !self.lane_heights.iter().any(|(k, l, _)| (*k, *l) == (LaneKind::Camera, lane)) {
                        // сбрасывать нечего — пункт молча ничего не сделал бы
                    } else if ui.button(tr(k::tl::lane::RESET_HEIGHT)).clicked() {
                        self.set_row_height((LaneKind::Camera, lane), None);
                        ui.close();
                    }
                    // переименование — как у обычной дорожки (правило 17):
                    // двойной клик по заголовку переименовывает СТРОКУ
                    // ЛЮБОГО вида, а пункта меню у служебных не было
                    if ui.button(tr(k::tl::lane::RENAME)).clicked() {
                        let cur = kt_model::lane_label(&self.lane_names, LaneKind::Camera, lane);
                        self.lane_edit =
                            Some(crate::app::LaneEdit { kind: LaneKind::Camera, lane, buf: cur });
                        self.lane_edit_focus = true;
                        ui.close();
                    }
                    let del_label = if n > 0 {
                        crate::tf!(k::tl::lane::DELETE_CAMERA_N, n)
                    } else {
                        tr(k::tl::lane::DELETE).to_string()
                    };
                    if ui.button(del_label).clicked() {
                        if self.apply(Command::RemoveCameraLane { at: lane }) {
                            kt_model::lane_names_remove(
                                &mut self.lane_names,
                                LaneKind::Camera,
                                lane,
                            );
                        }
                        ui.close();
                    }
                    if ui.button(tr(k::tl::lane::INSERT_CAMERA_BELOW)).clicked() {
                        self.insert_lane_at(LaneKind::Camera, lane + 1);
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
        // кэши по model_rev; Arc-снапшоты — клоны указателей (п.1 v1.4)
        let (media_names, suppress_name, media_paths, draw_order) = self.names();
        let pointer = response.interact_pointer_pos();
        let hover = response.hover_pos();
        self.snap_line = None;
        // препятствие живёт РОВНО пока жест в него упирается (п.5)
        self.blocked = None;
        // магнит включён всегда; Ctrl его снимает (п.2 р.11). Тот же Ctrl,
        // нажатый ДО захвата пустого места, начинает рамку выделения
        // (Р.33 п.7) — на переносе события магнит от этого не страдает
        self.snap_off = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
        let ctrl_held = self.snap_off;
        let shift_held = ui.input(|i| i.modifiers.shift);

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
        // ЛЕВЫЙ край диапазонной метки (Р.38, подметание). `Drag::MarkerEdge`
        // с `left: true` был описан в комментарии как рабочий, но никогда не
        // конструировался: жест уходил в `_ => {}` и пропадал молча.
        let marker_start_at = |app: &KtApp, pos: egui::Pos2| -> Option<i64> {
            if pos.y > rect.top() + RULER_H {
                return None;
            }
            app.model
                .project()
                .markers
                .iter()
                .filter(|m| m.is_range())
                .map(|m| m.t.0)
                .find(|start| (tick_to_x(*start) - pos.x).abs() <= EDGE_GRAB_PX)
        };
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
        // Именованная область (Р.38 п.1: «не понимаю как удалить saved
        // area region… пкм ничего не дает»). Хватается КОНТУР — тело
        // отдано событиям под ним, иначе область украла бы все клики на
        // своих дорожках. Полоса хвата шире рисунка (правило 1): линия
        // в 1 px мышью не ловится.
        let area_at = |app: &KtApp, pos: egui::Pos2| -> Option<(usize, bool)> {
            app.model.project().areas.iter().enumerate().find_map(|(i, a)| {
                let r = geom.area_rect(a)?;
                let band = r.expand(AREA_GRAB_PX);
                if !band.contains(pos) || r.shrink(AREA_GRAB_PX).contains(pos) {
                    return None;
                }
                // левая кромка или всё остальное (правая/верх/низ)
                Some((i, (pos.x - r.left()).abs() <= AREA_GRAB_PX))
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
            /// Ручка фейда у начала/конца события (п.12).
            FadeLeft,
            FadeRight,
            /// Линия уровня события: непрозрачность у видео, громкость у
            /// аудио (Р.28, п.12 + требование «фейды у аудио и видео
            /// одинаковые» — вертикальный рейл v1 стоял внутри зоны фейда).
            Level,
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
                // верхняя полоса «уголкового» фейда (п.12) принадлежит ручке
                // клипа: край окна клампится минимумом, а угол снимает фейд
                // совсем — иначе задвинуть его обратно было бы нечем.
                // Ниже полосы окно ловится как прежде (жест Р.25, тест 157).
                if t.is_fade()
                    && (t.in_offset.0 == 0 || t.out_offset.0 == 0)
                    && pos.y <= top + top_strip_h(lane_h)
                {
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
        // порядок хита — обратный порядку отрисовки: сначала то, что лежит
        // СВЕРХУ (поздний клип при наезде, п.177), иначе клик выбирал бы не
        // тот клип, который видно
        let order = draw_order.clone();
        let clip_at = move |app: &KtApp, pos: egui::Pos2| -> Option<(ClipId, ClipZone)> {
            for c in order.iter().rev().filter_map(|&i| app.model.project().clips.get(i)) {
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
                // Верхняя полоса события — жесты Vegas (п.12): ручки фейда
                // по углам и линия уровня. Они перекрывают трим-зоны, но
                // только в верхних пикселях — тело и края ниже свободны.
                let strip = top_strip_h(lane_h);
                let p = app.model.project();
                let fade_l = corner_fade_len(p, c.id, true).unwrap_or(0);
                let fade_r = corner_fade_len(p, c.id, false).unwrap_or(0);
                let hx_l = tick_to_x(fade_handle_tick(c.start.0, c.start.0 + c.duration.0, fade_l, true));
                let hx_r = tick_to_x(fade_handle_tick(c.start.0, c.start.0 + c.duration.0, fade_r, false));
                let in_strip = pos.y <= top + strip;
                let ly = level_y(clip_level(c), top, lane_h);
                let zone = if in_strip && (pos.x - hx_l).abs() <= FADE_HANDLE_PX {
                    ClipZone::FadeLeft
                } else if in_strip && (pos.x - hx_r).abs() <= FADE_HANDLE_PX {
                    ClipZone::FadeRight
                } else if x1 - x0 > 24.0 && (pos.y - ly).abs() <= LEVEL_GRAB_PX {
                    ClipZone::Level
                } else if (pos.x - x0).abs() <= grab {
                    ClipZone::TrimStart
                } else if (pos.x - x1).abs() <= grab {
                    ClipZone::TrimEnd
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
                    Some((_, ClipZone::FadeLeft)) | Some((_, ClipZone::FadeRight)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        zone_hint = Some(tr(k::tl::hint::FADE_HANDLE));
                    }
                    Some((id, ClipZone::Level)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                        let audio = self
                            .model
                            .clip(id)
                            .map(|c| c.kind == ClipKind::Audio)
                            .unwrap_or(false);
                        zone_hint =
                            Some(if audio { tr(k::tl::hint::VOLUME) } else { tr(k::tl::hint::LEVEL) });
                    }
                    Some((_, ClipZone::Body)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                        zone_hint = Some(
                            tr(k::tl::hint::CLIP),
                        );
                    }
                    None => {
                        // «пустое место» говорилось и НАД морф/камера-блоком:
                        // `clip_at` на служебной строке всегда None, а
                        // отдельной ветки не было. Подсказка, которая врёт,
                        // хуже отсутствующей (правило 10 чек-листа).
                        zone_hint = Some(if morph_edge_at(self, pos).is_some()
                            || camera_edge_at(self, pos).is_some()
                        {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            tr(k::tl::hint::MORPH_EDGE)
                        } else if area_at(self, pos).is_some() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            tr(k::tl::hint::AREA)
                        } else if morpher_at(self, pos).is_some() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                            tr(k::tl::hint::MORPHER)
                        } else if camera_at(self, pos).is_some() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                            tr(k::tl::hint::CAMERA)
                        } else {
                            tr(k::tl::hint::EMPTY)
                        });
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
            } else if let Some(mid) = morpher_at(self, pos) {
                // служебные блоки проверяются РАНЬШЕ границы региона:
                // линия региона идёт на всю высоту, и блок, чья кромка на
                // неё попала, иначе не схватить вовсе
                self.selected_morph = Some(mid);
                self.selected.clear();
                CtxTarget::Morpher(mid)
            } else if let Some(cid) = camera_at(self, pos) {
                self.selected_camera = Some(cid);
                CtxTarget::Camera(cid)
            } else if let Some(start_edge) = region_edge_at(self, pos) {
                let r = self.model.project().region.unwrap();
                CtxTarget::RegionEdge(if start_edge { r.start.0 } else { r.end.0 })
            } else if let Some((id, _)) = clip_at(self, pos) {
                // ПКМ по члену выделения не рушит пачку: меню про неё
                if !self.is_selected(id) {
                    self.select_only(id);
                }
                self.selected_morph = None;
                CtxTarget::Clip(id)
            } else if let Some((i, _)) = area_at(self, pos) {
                CtxTarget::Area(i)
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
            } else if let Some(start) = marker_start_at(self, pos) {
                if response.drag_started() {
                    self.drag = Some(Drag::MarkerEdge { start, left: true, cur: start });
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
                if response.double_clicked()
                    && let Some(start) = pick_range_marker(
                        &self.model.project().markers,
                        x_to_tick(pos.x),
                        (EDGE_GRAB_PX as f64 * tpp) as i64,
                    )
                {
                    // «при двойном клике на marked_region он выделяется весь»
                    // (Р.33 п.8): регион по границам метки + её события —
                    // сразу и рендер по имени, и операции над содержимым
                    self.region_from_marked_range(start);
                } else if response.drag_started() && (pos.x - head_x).abs() <= 8.0 {
                    self.drag = Some(Drag::Scrub);
                } else if response.drag_started() {
                    // снап и при СОЗДАНИИ региона (п.13 р.6, тест 59)
                    let anchor = self.snap(x_to_tick(pos.x).max(0), None);
                    self.drag = Some(Drag::Region { anchor });
                } else {
                    let t = self.snap(x_to_tick(pos.x).max(0), None);
                    self.player.seek(Tick(t.max(0)));
                }
            } else if let Some((cid, left)) =
                camera_edge_at(self, pos).filter(|_| !response.double_clicked())
                && response.drag_started()
            {
                self.selected_camera = Some(cid);
                self.selected.clear();
                self.selected_morph = None;
                if let Some(orig) = self.model.project().cameras.iter().find(|c| c.id == cid) {
                    let cur = if left { orig.start.0 } else { (orig.start + orig.duration).0 };
                    self.drag = Some(Drag::CameraEdge { id: cid, left, cur });
                }
            } else if let Some(cid) = camera_at(self, pos) {
                self.selected_camera = Some(cid);
                self.selected.clear();
                self.selected_morph = None;
                if response.double_clicked() {
                    self.open_camera_editor(cid);
                } else if response.drag_started()
                    && let Some(orig) =
                        self.model.project().cameras.iter().find(|c| c.id == cid)
                {
                    self.drag = Some(Drag::Camera {
                        id: cid,
                        grab_dt: x_to_tick(pos.x) - orig.start.0,
                        cur_start: orig.start.0,
                        moved: false,
                    });
                }
            } else if let Some((mid, left)) = morph_edge_at(self, pos).filter(|_| {
                // двойной клик по кромке — это всё-таки «открыть окно»
                !response.double_clicked()
            }) && response.drag_started()
            {
                self.selected_morph = Some(mid);
                self.selected.clear();
                if let Some(orig) = self.model.project().morphers.iter().find(|m| m.id == mid) {
                    let cur = if left { orig.start.0 } else { (orig.start + orig.duration).0 };
                    self.drag = Some(Drag::MorphEdge { id: mid, left, cur });
                }
            } else if let Some(mid) = morpher_at(self, pos) {
                self.selected_morph = Some(mid);
                self.selected.clear();
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
                if response.double_clicked() {
                    // двойной клик по окну перехода = его свойства
                    // (паритет со способом «двойной клик по событию =
                    // окно FX», п.17 р.28: парные способы ведут себя
                    // одинаково)
                    self.open_transition_editor(tid);
                } else if response.drag_started() {
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
                self.selected_morph = None;
                if ctrl_held && response.clicked() {
                    // Ctrl+клик набирает выделение: курсор при этом стоит на
                    // месте, иначе набор пачки утаскивал бы playhead за собой
                    self.toggle_selected(id);
                } else if response.clicked() {
                    self.select_only(id);
                    // клик по СОБЫТИЮ тоже ставит курсор (v1, п.7)
                    let t = self.snap(x_to_tick(pos.x).max(0), None);
                    self.player.seek(Tick(t.max(0)));
                } else if !self.is_selected(id) {
                    // драг по чужому событию берёт его одно; драг по члену
                    // пачки тащит всю пачку (выделение не рушим)
                    self.select_only(id);
                }
                if response.double_clicked() {
                    // двойной клик открывает FX — ровно то, что обещает
                    // подсказка клипа с Р.18 (до Р.33 она врала: клик ставил
                    // регион, а FX прятались в ПКМ-меню). Регион по событию
                    // остался на Shift+двойном клике и в меню
                    if shift_held {
                        self.region_from_event(id);
                    } else {
                        self.open_fx_editor(id);
                    }
                } else if response.drag_started() {
                    match zone {
                        ClipZone::FadeLeft | ClipZone::FadeRight => {
                            let left = zone == ClipZone::FadeLeft;
                            let len = corner_fade_len(self.model.project(), id, left)
                                .unwrap_or(0);
                            self.drag = Some(Drag::Fade { id, left, len, moved: false });
                        }
                        ClipZone::Level => {
                            self.drag = Some(Drag::Level { id, moved: false });
                        }
                        zone => {
                            if let Some(orig) = self.model.clip(id).cloned() {
                                let mode = match zone {
                                    ClipZone::TrimStart => DragMode::TrimStart,
                                    ClipZone::TrimEnd => DragMode::TrimEnd,
                                    _ => DragMode::Move,
                                };
                                // пачка едет вместе с якорем: сюда попадают
                                // прочие выделенные (свои link-партнёры
                                // двигаются самой командой MoveClip)
                                let extra: Vec<(ClipId, i64)> = if self.selected.len() > 1 {
                                    let group = orig.link_group;
                                    self.model
                                        .project()
                                        .clips
                                        .iter()
                                        .filter(|c| {
                                            self.selected.contains(&c.id)
                                                && c.id != id
                                                && !(group.is_some() && c.link_group == group)
                                        })
                                        .map(|c| (c.id, c.start.0))
                                        .collect()
                                } else {
                                    Vec::new()
                                };
                                self.drag = Some(Drag::Clip(DragState {
                                    id,
                                    mode,
                                    extra,
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
                if ctrl_held || self.sel_tool {
                    // рамка выделения (Р.33 п.7): Ctrl+драг всегда, а в
                    // режиме-инструменте — обычный драг. Начинается с
                    // ПУСТОГО места: драг по событию остаётся переносом, и
                    // Ctrl в нём по-прежнему снимает магнит (п.2 р.11)
                    self.drag =
                        Some(Drag::Rubber { anchor: pos, add: ctrl_held && self.sel_tool });
                } else {
                    // пустое место: выделение региона (порт RegionSelect)
                    // снап и при создании региона с пустого места (п.13 р.6)
                    let anchor = self.snap(x_to_tick(pos.x).max(0), None);
                    self.drag = Some(Drag::Region { anchor });
                }
            } else {
                self.selected.clear();
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
                Some(Drag::LaneHeight { row, cur }) => {
                    // тоже жест gutter'а — держим, чтобы не потерять
                    self.drag = Some(Drag::LaneHeight { row, cur });
                }
                Some(Drag::Rubber { anchor, add }) => {
                    // рамка рисуется в слое клипов по текущему указателю,
                    // выделение забирается на отпускании
                    self.drag = Some(Drag::Rubber { anchor, add });
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
                            if d.extra.is_empty() {
                                // строки перемешаны (п.6): прилипаем к
                                // ближайшей строке СВОЕГО вида (тест 50)
                                // строк разной высоты деление не считает
                                let row = geom.row_at_y_unbounded(pos.y).max(0);
                                d.cur_lane = geom.nearest_lane(d.orig.kind, row);
                            } else {
                                // пачка едет только по времени: у её событий
                                // разные виды и дорожки, «ближайшая строка»
                                // для блока не определена
                                d.cur_lane = d.orig.lane;
                                let moving = self.moving_block_ids(&d);
                                let plan = plan_move_block(
                                    self.model.project(),
                                    &moving,
                                    d.cur_start - d.orig.start.0,
                                );
                                d.cur_start = d.orig.start.0 + plan.delta;
                                d.cur_end = d.cur_start + dur;
                                if let Some((span, kind, lane)) = plan.blocker {
                                    self.snap_line = None;
                                    self.blocked = Some(Blocked { span, kind, lane });
                                }
                            }
                            // мульти-наезд/поглощение — кламп ещё в превью
                            // (реш. 10 Р.24); одиночный частичный наезд
                            // остаётся как есть (release сделает кроссфейд)
                            if let (true, MovePlan::Clamped { start, blocker }) = (
                                d.extra.is_empty(),
                                plan_move(self.model.project(), d.id, d.cur_start, d.cur_lane),
                            ) {
                                let mut blocker = blocker;
                                if d.cur_lane != d.orig.lane {
                                    // вертикальный перенос сквозь занятое
                                    // не проталкиваем — возврат на строку
                                    d.cur_lane = d.orig.lane;
                                    if let MovePlan::Clamped { start, blocker: b } = plan_move(
                                        self.model.project(),
                                        d.id,
                                        d.cur_start,
                                        d.cur_lane,
                                    ) {
                                        d.cur_start = start;
                                        blocker = b;
                                    }
                                } else {
                                    d.cur_start = start;
                                }
                                d.cur_end = d.cur_start + dur;
                                self.snap_line = None;
                                // невозможность действия — ОШИБКА, а не
                                // блёклая подсказка (п.5): препятствие
                                // подсвечивается, а если оно за пределами
                                // вида — подсказка говорит, куда смотреть
                                self.blocked = Some(Blocked {
                                    span: blocker,
                                    kind: d.orig.kind,
                                    lane: d.cur_lane,
                                });
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
                            let was = (t.in_offset, t.out_offset);
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
                            // кадр без движения мыши команду не шлёт: иначе
                            // ревизия модели растёт каждый кадр драга и зря
                            // сбрасывает кэши, ключёванные по ней
                            ((t.in_offset, t.out_offset) != was
                                && t.in_offset + t.out_offset >= MIN_TRANSITION_DURATION)
                                .then_some(Command::SetTransition(t))
                        })
                    };
                    if let Some(cmd) = cmd {
                        self.apply_coalesced(cmd, gesture);
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
                Some(Drag::CameraEdge { id, left, .. }) => {
                    let cur = x_to_tick(pos.x).max(0);
                    self.drag = Some(Drag::CameraEdge { id, left, cur });
                }
                Some(Drag::MorphEdge { id, left, .. }) => {
                    let cur = x_to_tick(pos.x).max(0);
                    self.drag = Some(Drag::MorphEdge { id, left, cur });
                }
                Some(Drag::Morph { id, grab_dt, .. }) => {
                    let raw = x_to_tick(pos.x) - grab_dt;
                    let cur_start = self.snap(raw, None).max(0);
                    self.drag = Some(Drag::Morph { id, grab_dt, cur_start, moved: true });
                }
                Some(Drag::Camera { id, grab_dt, .. }) => {
                    let raw = x_to_tick(pos.x) - grab_dt;
                    let cur_start = self.snap(raw, None).max(0);
                    self.drag = Some(Drag::Camera { id, grab_dt, cur_start, moved: true });
                }
                Some(Drag::Fade { id, left, .. }) => {
                    // длина = расстояние от края события до курсора,
                    // клампится телом: «сколько выдвигал — столько и фейд»
                    if let Some(c) = self.model.clip(id) {
                        let (s0, e0) = (c.start.0, c.start.0 + c.duration.0);
                        let raw = x_to_tick(pos.x).clamp(s0, e0);
                        let mut len = if left { raw - s0 } else { e0 - raw };
                        // мёртвая зона у самого угла: ручка «прилипает» к
                        // краю, и задвинутый обратно фейд исчезает целиком,
                        // а не оставляет невидимый огрызок в пару мс
                        let edge_x = tick_to_x(if left { s0 } else { e0 });
                        if (pos.x - edge_x).abs() <= 3.0 {
                            len = 0;
                        }
                        self.drag = Some(Drag::Fade {
                            id,
                            left,
                            len: len.clamp(0, c.duration.0),
                            moved: true,
                        });
                    }
                }
                Some(Drag::Level { id, .. }) => {
                    if let Some(c) = self.model.clip(id) {
                        let (kind, top) = (c.kind, lane_top(c.kind, c.lane));
                        let lv = level_from_y(pos.y, top, lane_h);
                        // live-превью без записи в историю: у аудио уровень
                        // это громкость (диапазон рейла v1), у видео —
                        // непрозрачность
                        let mut p = self.model.project().clone();
                        if let Some(cm) = p.clips.iter_mut().find(|c| c.id == id) {
                            if kind == ClipKind::Audio {
                                cm.gain_db = level_to_gain(lv);
                            } else {
                                cm.opacity = lv;
                            }
                        }
                        self.player.set_project(p);
                        if kind == ClipKind::Audio {
                            self.live_gain = Some((id, level_to_gain(lv)));
                        } else {
                            self.live_opacity = Some((id, lv));
                        }
                    }
                    self.drag = Some(Drag::Level { id, moved: true });
                }
                None => {}
            }
        }

        // ---- release -------------------------------------------------------------
        if response.drag_stopped() {
            match self.drag.take() {
                // пачка: общий сдвиг одним батчем, никаких кроссфейдов из
                // наезда — блок переезжает как есть (Р.33 п.7)
                Some(Drag::Clip(d)) if d.moved && !d.extra.is_empty() => {
                    let delta = d.cur_start - d.orig.start.0;
                    if delta != 0 {
                        let cmds: Vec<Command> = std::iter::once((d.id, d.orig.start.0))
                            .chain(d.extra.iter().copied())
                            .map(|(id, orig_start)| Command::MoveClip {
                                id,
                                // от ОРИГИНАЛЬНЫХ стартов: команда двигает всю
                                // link-группу, и повтор по её члену — no-op
                                new_start: Tick(orig_start + delta),
                                new_lane: self
                                    .model
                                    .clip(id)
                                    .map_or(d.orig.lane, |c| c.lane),
                            })
                            .collect();
                        self.apply_batch_restoring(cmds);
                    }
                }
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
                                    audio: Default::default(),
                                    auto: vec![],
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
                Some(Drag::MarkerEdge { start, left: true, cur }) => {
                    // конец стоит, начало едет: MoveMarker + новая длина
                    // одним шагом истории (правило 20 — ничего не отрастает)
                    let end = self
                        .model
                        .project()
                        .markers
                        .iter()
                        .find(|m| m.t.0 == start)
                        .map(|m| m.end().0)
                        .unwrap_or(start);
                    let ns = cur.clamp(0, end);
                    if ns != start {
                        self.apply_batch(vec![
                            Command::MoveMarker { from: Tick(start), to: Tick(ns) },
                            Command::SetMarkerDuration {
                                at: Tick(ns),
                                duration: Tick(end - ns),
                            },
                        ]);
                    }
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
                Some(Drag::Fade { id, left, len, moved }) => {
                    if moved {
                        self.commit_fade_handle(id, left, len);
                    }
                }
                Some(Drag::Level { id, moved }) => {
                    if moved {
                        if let Some((oid, op)) = self.live_opacity.take_if(|(o, _)| *o == id) {
                            self.apply(Command::SetOpacity { id: oid, opacity: op });
                        }
                        if let Some((gid, gain)) = self.live_gain.take_if(|(g, _)| *g == id) {
                            self.apply(Command::SetGain { id: gid, gain_db: gain });
                        }
                    }
                }
                Some(Drag::Rubber { anchor, add }) => {
                    // точка отпускания: interact_pointer_pos живёт до конца
                    // кадра релиза, hover — страховка
                    let band = egui::Rect::from_two_pos(anchor, pointer.or(hover).unwrap_or(anchor));
                    // рамка запоминается В ДОМЕНЕ МОДЕЛИ (время × ключи
                    // дорожек): из неё делается именованная область
                    // (Р.35 п.2) — пунктом меню или кнопкой, уже после
                    // того, как пользователь отпустил мышь
                    self.last_band = Some((
                        Tick(x_to_tick(band.left()).max(0)),
                        Tick(x_to_tick(band.right()).max(0)),
                        geom.rows_in_band(band.top(), band.bottom()),
                    ));
                    let hits = rubber_hits(&geom, self.model.project(), band);
                    if !add {
                        self.selected.clear();
                    }
                    if !hits.is_empty() {
                        self.selected_morph = None;
                    }
                    self.selected.extend(hits);
                }
                Some(Drag::Region { .. }) | Some(Drag::RegionEdge { .. }) => {
                    self.on_region_changed();
                    // играем — курсор немедленно в начало нового региона (п.4)
                    self.on_region_released();
                }
                Some(Drag::Camera { id, cur_start, moved, .. }) => {
                    // перенос камеры везёт её кривые с собой: их домен —
                    // тики ОТ НАЧАЛА события, трогать нечего (Р.34 К3)
                    if moved
                        && let Some(mut ev) =
                            self.model.project().cameras.iter().find(|c| c.id == id).cloned()
                        && ev.start.0 != cur_start
                    {
                        ev.start = Tick(cur_start);
                        self.apply(Command::SetCamera(ev));
                    }
                }
                Some(Drag::CameraEdge { id, left, cur }) => {
                    if let Some(mut ev) =
                        self.model.project().cameras.iter().find(|c| c.id == id).cloned()
                    {
                        let (s0, e0) = (ev.start.0, (ev.start + ev.duration).0);
                        let min = kt_model::MIN_CLIP_DURATION.0;
                        if left {
                            let ns = cur.min(e0 - min).max(0);
                            ev.start = Tick(ns);
                            ev.duration = Tick(e0 - ns);
                        } else {
                            ev.duration = Tick((cur - s0).max(min));
                        }
                        self.apply(Command::SetCamera(ev));
                    }
                }
                Some(Drag::MorphEdge { id, left, cur }) => {
                    if let Some(mut ev) =
                        self.model.project().morphers.iter().find(|m| m.id == id).cloned()
                    {
                        let (s0, e0) = (ev.start.0, (ev.start + ev.duration).0);
                        let min = kt_model::MIN_CLIP_DURATION.0;
                        if left {
                            let ns = cur.min(e0 - min).max(0);
                            ev.start = Tick(ns);
                            ev.duration = Tick(e0 - ns);
                        } else {
                            ev.duration = Tick((cur - s0).max(min));
                        }
                        self.apply(Command::SetMorpher(ev));
                    }
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
        // карты растяжения — раз на версию модели (Р.37 п.3)
        let warps = self.warp_maps();
        let peaks = self.peaks.lock().unwrap();
        let offline = self.offline.lock().unwrap().clone();

        // порядок = порядок КОМПОЗИЦИИ движка: поздний клип ложится поверх
        // раннего, как в превью (п.177 — до этого полоса рисовала в порядке
        // списка проекта и могла показывать не тот источник, что в кадре)
        for c in draw_order.iter().filter_map(|&i| self.model.project().clips.get(i)) {
            let (start, end, lane) = match &self.drag {
                Some(Drag::Clip(d)) if d.id == c.id => (d.cur_start, d.cur_end, d.cur_lane),
                // прочие события пачки едут тем же сдвигом (Р.33 п.7)
                Some(Drag::Clip(d))
                    if d.extra.iter().any(|(id, _)| *id == c.id) =>
                {
                    let delta = d.cur_start - d.orig.start.0;
                    (c.start.0 + delta, c.start.0 + c.duration.0 + delta, c.lane)
                }
                _ => (c.start.0, c.start.0 + c.duration.0, c.lane),
            };
            let x0 = tick_to_x(start);
            let x1 = tick_to_x(end);
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let src_shift = preview_src_shift(self.drag.as_ref(), c.id, c.start.0);
            let top = lane_top(c.kind, lane);
            // клип тянется ПОД полосу имён дорожек — она полупрозрачная и
            // таймлайн просвечивает (п.4 р.7.1); имя рисуется от СТАРОЙ
            // границы (rect.left()), чтобы текст не лез под градиент
            let clip_rect = egui::Rect::from_min_max(
                egui::pos2(x0.max(full.left()), top + 1.0),
                egui::pos2(x1.min(rect.right()), top + lane_h - 3.0),
            );
            let selected = self.is_selected(c.id);
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

            // Растяжение времени (Р.37 п.3): превью обязано считать
            // позицию материала ТОЙ ЖЕ картой, что рендер, иначе
            // замедленный клип рисует не то, что играет. Карта — из кэша
            // по model_rev; строить её на столбец нельзя (аллокация в
            // draw-пути, инвариант №12).
            let warp = warps.get(&c.id);
            let dur = c.duration.0.max(1);
            // тик таймлайна внутри события → СМЕЩЕНИЕ В МАТЕРИАЛЕ
            let material_off = |tl: i64| -> i64 {
                warp.map_or(tl, |w| w.material_off(c.duration, c.reversed, tl))
            };
            // фактически потреблённый пролёт источника: по нему выбирается
            // ступень филмстрипа, иначе замедленный клип берёт вдвое более
            // грубый шаг и вырождается в повторяющиеся бакеты
            let used_span = warp.map_or(dur, |w| w.used().max(1));

            // филмстрип видео-клипа: тумбы из индекс-БД/сервиса, отрисовка
            // никогда не блокируется и не трогает декодеры плеера
            if c.kind == ClipKind::Video && !is_offline && clip_rect.width() > 30.0 {
                let thumb_h = clip_rect.height() - 2.0;
                let thumb_w = (thumb_h * 16.0 / 9.0).max(28.0);
                let step = crate::thumbs::thumb_source_step(
                    Tick(used_span),
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
                        let src_t = crate::thumbs::thumb_key_t(
                            c.source_in.0 + src_shift,
                            material_off(tl),
                            step,
                        );
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
                let eff_src_in = c.source_in.0 + src_shift;
                let src_in_frames = Tick(eff_src_in.max(0)).to_audio_frames() as f64;
                let col_l = egui::Color32::from_rgba_unmultiplied(170, 210, 255, 230);
                let col_r = egui::Color32::from_rgba_unmultiplied(230, 190, 255, 230);
                // один меш на клип вместо 2 line_segment на ПИКСЕЛЬ
                // (п.1 v1.4: тысячи шейпов за кадр на широких клипах)
                let mut mesh = egui::Mesh::default();
                let mut x = clip_rect.left();
                while x < clip_rect.right() {
                    let tl0 = material_off(x_to_tick(x) - start);
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

            // линия уровня (п.12): у видео непрозрачность, у аудио
            // громкость — ОДИН элемент на оба вида, как в Vegas (требование
            // «фейды у аудио и видео одинаково выглядящие»: вертикальный
            // рейл v1 стоял ровно в зоне фейда и ломал картинку)
            if clip_rect.width() > 24.0 {
                let lv = if c.kind == ClipKind::Audio {
                    match self.live_gain {
                        Some((id, g)) if id == c.id => gain_to_level(g),
                        _ => gain_to_level(c.gain_db),
                    }
                } else {
                    match self.live_opacity {
                        Some((id, o)) if id == c.id => o,
                        _ => c.opacity,
                    }
                };
                let op = lv;
                let ly = level_y(lv, top, lane_h);
                // «не по умолчанию» — ярче: у видео это op<1, у аудио 0 dB
                // стоит посередине (диапазон рейла v1 ±6 dB)
                let dim = if c.kind == ClipKind::Audio { (op - 0.5).abs() > 1e-3 } else { op < 1.0 };
                painter.line_segment(
                    [egui::pos2(clip_rect.left(), ly), egui::pos2(clip_rect.right(), ly)],
                    egui::Stroke::new(
                        if dim { 2.0 } else { 1.0 },
                        egui::Color32::from_rgba_unmultiplied(255, 220, 120, if dim { 230 } else { 90 }),
                    ),
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

        // ---- именованные области (Р.35 п.2) --------------------------------
        // Своей полосы НЕ заводим (правило 13): область рисуется контуром
        // прямо по своим строкам, а имя — В РАЗРЫВЕ верхней линии (тот же
        // приём, что у имени группы бина).
        for a in &self.model.project().areas {
            let Some(r) = geom.area_rect(a) else { continue };
            let col = a
                .color
                .as_deref()
                .and_then(crate::theme::parse_hex)
                .unwrap_or(crate::theme::AREA);
            // разрыв верхней линии под имя
            let font = egui::FontId::proportional(11.0);
            let galley = painter.layout_no_wrap(a.name.clone(), font.clone(), col);
            let gap = (galley.size().x + 8.0).min((r.width() - 8.0).max(0.0));
            let stroke = egui::Stroke::new(1.0, col);
            painter.line_segment([r.left_bottom(), r.right_bottom()], stroke);
            painter.line_segment([r.left_top(), r.left_bottom()], stroke);
            painter.line_segment([r.right_top(), r.right_bottom()], stroke);
            painter.line_segment(
                [r.left_top(), egui::pos2((r.left() + 6.0).min(r.right()), r.top())],
                stroke,
            );
            painter.line_segment(
                [egui::pos2((r.left() + 6.0 + gap).min(r.right()), r.top()), r.right_top()],
                stroke,
            );
            if gap > 12.0 {
                painter.galley(egui::pos2(r.left() + 10.0, r.top() - galley.size().y * 0.5), galley, col);
            }
        }

        // ---- рамка выделения (Р.33 п.7): тот же вид, что в медиа-бине ------
        if let Some(Drag::Rubber { anchor, .. }) = &self.drag
            && let Some(cur) = pointer.or(hover)
        {
            let band = egui::Rect::from_two_pos(*anchor, cur);
            painter.rect_filled(
                band,
                0.0,
                egui::Color32::from_rgba_unmultiplied(90, 140, 220, 40),
            );
            painter.rect_stroke(
                band,
                0.0,
                egui::Stroke::new(1.0, crate::theme::ACCENT),
                egui::StrokeKind::Middle,
            );
        }

        // ---- morph-блоки (H1, порт v1: фиолетовый скруглённый блок) ---------
        for m in &self.model.project().morphers {
            let (start, end) = match &self.drag {
                Some(Drag::Morph { id, cur_start, .. }) if *id == m.id => {
                    (*cur_start, *cur_start + m.duration.0)
                }
                Some(Drag::MorphEdge { id, left, cur }) if *id == m.id => {
                    let min = kt_model::MIN_CLIP_DURATION.0;
                    let (s0, e0) = (m.start.0, (m.start + m.duration).0);
                    if *left { ((*cur).min(e0 - min).max(0), e0) } else { (s0, (*cur).max(s0 + min)) }
                }
                _ => (m.start.0, (m.start + m.duration).0),
            };
            let x0 = tick_to_x(start);
            let x1 = tick_to_x(end).max(x0 + 8.0);
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let top = morph_lane_top(m.lane);
            // служебная строка втрое ниже (Р.35 п.1): поля ужимаются, но
            // ХВАТ шире рисунка на 2 px наружу — правило 1 чек-листа
            let rh = thin_h;
            let pad = (rh * 0.12).clamp(1.0, 3.0);
            let r = egui::Rect::from_min_max(
                egui::pos2(x0.max(rect.left()), top + pad),
                egui::pos2(x1.min(rect.right()), top + rh - pad - 2.0),
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
            // подпись по РЕЖИМУ ЦЕЛЕЙ (Р.35): вида у морфера больше нет,
            // а «на кого действует» — единственное, чего не видно по
            // самому блоку. Не влезла — уходит в подсказку (правило 7).
            let label = match &m.targets {
                MorphTargets::Clips { ids } => crate::tf!(k::tl::morph::CLIPS, ids.len()),
                _ => tr(k::tl::morph::LAYER).to_string(),
            };
            let font = egui::FontId::proportional((rh * 0.5).clamp(8.0, 11.0));
            let galley = painter.layout_no_wrap(label.clone(), font.clone(), egui::Color32::WHITE);
            if galley.size().x + 10.0 <= r.width() && galley.size().y <= r.height() {
                painter.text(
                    egui::pos2(r.left() + 6.0, r.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    font,
                    egui::Color32::WHITE,
                );
            } else if ui.ctx().pointer_hover_pos().is_some_and(|p| r.contains(p)) {
                self.hover_hint = Some(label);
            }
        }
        // ---- камеры-события (Р.29): бирюзовый блок на камерной дорожке ------
        for c in &self.model.project().cameras {
            // живое превью жеста: до Р.38 камера при драге стояла на месте
            // до отпускания — жест выглядел мёртвым
            let (start, end) = match &self.drag {
                Some(Drag::Camera { id, cur_start, .. }) if *id == c.id => {
                    (*cur_start, *cur_start + c.duration.0)
                }
                Some(Drag::CameraEdge { id, left, cur }) if *id == c.id => {
                    let min = kt_model::MIN_CLIP_DURATION.0;
                    let (s0, e0) = (c.start.0, (c.start + c.duration).0);
                    if *left { ((*cur).min(e0 - min).max(0), e0) } else { (s0, (*cur).max(s0 + min)) }
                }
                _ => (c.start.0, (c.start + c.duration).0),
            };
            let x0 = tick_to_x(start);
            let x1 = tick_to_x(end).max(x0 + 8.0);
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let top = geom.row_top(geom.row_of((LaneKind::Camera, c.lane)).unwrap_or(0));
            let pad = (thin_h * 0.12).clamp(1.0, 3.0);
            let r = egui::Rect::from_min_max(
                egui::pos2(x0.max(rect.left()), top + pad),
                egui::pos2(x1.min(rect.right()), top + thin_h - pad - 2.0),
            );
            let selected = self.selected_camera == Some(c.id);
            painter.rect(
                r,
                4.0,
                egui::Color32::from_rgba_unmultiplied(60, 150, 160, 200),
                if selected {
                    egui::Stroke::new(2.0, crate::theme::ACCENT)
                } else {
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(24, 52, 58))
                },
                egui::StrokeKind::Inside,
            );
            if r.width() > 46.0 {
                let label = if c.name.is_empty() {
                    format!("C{}", c.id.0)
                } else {
                    c.name.clone()
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
                // ручка фейда тянется — окно рисуется по ЖИВОЙ длине (п.12)
                let (in_off, out_off) = match &self.drag {
                    Some(Drag::Fade { id, left, len, .. })
                        if t.is_fade()
                            && ((*left && t.b == Some(*id) && t.a.is_none())
                                || (!*left && t.a == Some(*id) && t.b.is_none())) =>
                    {
                        if *left { (0, *len) } else { (*len, 0) }
                    }
                    _ => (t.in_offset.0, t.out_offset.0),
                };
                let x0 = tick_to_x(joint - in_off);
                let x1 = tick_to_x(joint + out_off);
                if x1 < rect.left() || x0 > rect.right() {
                    continue;
                }
                let top = lane_top(a.kind, a.lane);
                let r = transition_rect(x0, x1, top, lane_h);
                // ручки (Р.26): материал соседей продолжается за стык и
                // никуда не делся — растащишь клипы, и он вернётся
                let xj = tick_to_x(joint);
                let (left, right) = transition_handle_rects(x0, xj, x1, top, lane_h);
                let _ = (in_off, out_off);
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
                // вид перехода виден НА ПОЛОСЕ (Р.33 С4): без этого 16
                // видов различались бы только по окну свойств. Имя
                // пишется в само окно перехода — своей полосы под него
                // не заводим (правило «не тратить место впустую»)
                if t.kind != TransitionKind::Crossfade {
                    let name = crate::transition_editor::kind_name(&t.kind);
                    let font = egui::FontId::proportional((lane_h * 0.32).clamp(8.0, 11.0));
                    let g = painter.layout_no_wrap(
                        name.to_string(),
                        font,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210),
                    );
                    if g.size().x + 6.0 <= r.width() {
                        // подложка под текстом: диагонали перехода иначе
                        // читаются сквозь буквы
                        let at = egui::pos2(
                            r.center().x - g.size().x / 2.0,
                            r.center().y - g.size().y / 2.0,
                        );
                        painter.rect_filled(
                            egui::Rect::from_min_size(at, g.size()).expand(2.0),
                            2.0,
                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 150),
                        );
                        painter.galley(at, g, egui::Color32::WHITE);
                    } else {
                        // не влезло — точка-метка «здесь не растворение»
                        painter.circle_filled(
                            r.center(),
                            2.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210),
                        );
                    }
                }
                painter.line_segment(
                    [egui::pos2(xj, r.top()), egui::pos2(xj, r.bottom())],
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 80)),
                );
            }
        }

        // предпросмотр ЕЩЁ НЕ созданного фейда и сами ручки (п.12)
        {
            let p = self.model.project();
            if let Some(Drag::Fade { id, left, len, moved: true }) = &self.drag
                && *len > 0
                && corner_fade_len(p, *id, *left).is_none()
                && let Some(c) = p.clips.iter().find(|c| c.id == *id)
            {
                let top = lane_top(c.kind, c.lane);
                let (s0, e0) = (c.start.0, c.start.0 + c.duration.0);
                let (a, b) = if *left { (s0, s0 + *len) } else { (e0 - *len, e0) };
                let r = transition_rect(tick_to_x(a), tick_to_x(b), top, lane_h);
                painter.rect_filled(r, 2.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22));
                let st = egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 130),
                );
                if *left {
                    painter.line_segment([r.left_bottom(), r.right_top()], st);
                } else {
                    painter.line_segment([r.left_top(), r.right_bottom()], st);
                }
            }
            // ручки видны у ВЫДЕЛЕННОГО события — иначе жест не найти
            // (п.14 чек-листа: элемент управления обязан быть заметен)
            if let Some(sel) = self.selected_one()
                && let Some(c) = p.clips.iter().find(|c| c.id == sel)
            {
                let top = lane_top(c.kind, c.lane);
                let (s0, e0) = (c.start.0, c.start.0 + c.duration.0);
                if tick_to_x(e0) - tick_to_x(s0) > 24.0 {
                    for left in [true, false] {
                        let len = corner_fade_len(p, sel, left).unwrap_or(0);
                        let x = tick_to_x(fade_handle_tick(s0, e0, len, left));
                        let y = top + 2.0;
                        let d = 5.0f32;
                        let dx = if left { d } else { -d };
                        painter.add(egui::Shape::convex_polygon(
                            vec![
                                egui::pos2(x, y),
                                egui::pos2(x + dx, y),
                                egui::pos2(x, y + d),
                            ],
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 190),
                            egui::Stroke::NONE,
                        ));
                    }
                }
            }
        }

        // ---- маркеры ----------------------------------------------------------------
        for m in &self.model.project().markers {
            let mt = match &self.drag {
                Some(Drag::Marker { orig, cur }) if *orig == m.t.0 => *cur,
                Some(Drag::MarkerEdge { start, left: true, cur }) if *start == m.t.0 => {
                    (*cur).clamp(0, m.end().0)
                }
                _ => m.t.0,
            };
            let mdur = match &self.drag {
                // край диапазона тянется — живой предпросмотр
                Some(Drag::MarkerEdge { start, left: false, cur }) if *start == m.t.0 => {
                    (*cur - m.t.0).max(0)
                }
                Some(Drag::MarkerEdge { start, left: true, cur }) if *start == m.t.0 => {
                    m.end().0 - (*cur).clamp(0, m.end().0)
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

        // ---- переименование области (Р.38 п.1) -----------------------------
        // Тот же виджет и то же поведение, что у имени дорожки: поле у
        // самой сущности, Enter применяет, Esc отменяет, фокус ОТДАЁТСЯ
        // при закрытии (правило 14 — иначе умрут горячие клавиши).
        if let Some(ed) = self.area_edit.clone() {
            let anchor = self
                .model
                .project()
                .areas
                .iter()
                .find(|a| a.name == ed.name)
                .and_then(|a| geom.area_rect(a))
                .map(|r| r.left_top())
                .unwrap_or(rect.left_top());
            let mut buf = ed.buf.clone();
            let mut done: Option<bool> = None;
            egui::Area::new(egui::Id::new("area-name-edit"))
                .order(egui::Order::Foreground)
                .fixed_pos(anchor + egui::vec2(2.0, 2.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut buf)
                                .hint_text(tr(k::tl::field::AREA_NAME))
                                .desired_width(140.0),
                        );
                        if !r.has_focus() && self.area_edit_focus {
                            r.request_focus();
                            self.area_edit_focus = false;
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
                    let to = buf.trim().to_string();
                    if !to.is_empty() && to != ed.name {
                        self.apply(Command::RenameArea { from: ed.name.clone(), to });
                    }
                    ui.ctx().memory_mut(|m| m.surrender_focus(egui::Id::new("area-name-edit")));
                    self.area_edit = None;
                }
                Some(false) => self.area_edit = None,
                None => self.area_edit = Some(crate::app::AreaEdit { buf, ..ed }),
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
            for (label, cy, row_top, row_h) in &gutter_labels {
                let font = if *row_h < 20.0 {
                    // тонкая служебная строка: шрифт по её высоте, иначе
                    // подпись не влезает и наезжает на соседнюю
                    egui::FontId::proportional((row_h * 0.62).clamp(7.0, 12.0))
                } else {
                    font.clone()
                };
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
                let y = lane_label_y(*row_top, *row_h, galley.size().y);
                let plate = egui::Rect::from_min_size(
                    egui::pos2(x - 3.0, y - 1.0),
                    egui::vec2(galley.size().x + 12.0, galley.size().y + 2.0),
                );
                crate::theme::hgrad(&full_painter, plate, solid, egui::Color32::TRANSPARENT);
                full_painter.galley(egui::pos2(x, y), galley, col);
            }
        }

        // ---- препятствие жеста (п.5) ----------------------------------------------
        // «я пока сам туда не промотал — не понял что оказывается там еще
        // событие есть в конце которое мешает»: то, во что упёрлись,
        // подсвечивается КРАСНЫМ, а если оно за пределами вида — на краю
        // строки рисуется стрелка в его сторону.
        if let Some(b) = self.blocked {
            let top = lane_top(b.kind, b.lane);
            let y0 = top + 1.0;
            let y1 = top + lane_h - 3.0;
            let err = egui::Color32::from_rgb(255, 105, 97);
            match block_view(b.span, (x_to_tick(rect.left()), x_to_tick(rect.right()))) {
                BlockView::Visible => {
                    let bx0 = tick_to_x(b.span.0).max(rect.left());
                    let bx1 = tick_to_x(b.span.1).min(rect.right());
                    let r = egui::Rect::from_min_max(egui::pos2(bx0, y0), egui::pos2(bx1, y1));
                    full_painter.rect_filled(
                        r,
                        4.0,
                        egui::Color32::from_rgba_unmultiplied(255, 105, 97, 48),
                    );
                    full_painter.rect(
                        r,
                        4.0,
                        egui::Color32::TRANSPARENT,
                        egui::Stroke::new(2.0, err),
                        egui::StrokeKind::Inside,
                    );
                }
                dir => {
                    // стрелка у края строки: куда смотреть
                    let left = matches!(dir, BlockView::Left(_));
                    let cy = (y0 + y1) * 0.5;
                    let tip = if left { rect.left() + 3.0 } else { rect.right() - 3.0 };
                    let back = if left { tip + 14.0 } else { tip - 14.0 };
                    full_painter.add(egui::Shape::convex_polygon(
                        vec![
                            egui::pos2(tip, cy),
                            egui::pos2(back, cy - 7.0),
                            egui::pos2(back, cy + 7.0),
                        ],
                        err,
                        egui::Stroke::NONE,
                    ));
                }
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
                    if !self.is_selected(id) {
                        self.select_only(id);
                    }
                    self.copy_selection();
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::PASTE)).clicked() {
                    self.paste_at_cursor();
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::DELETE)).clicked() {
                    if !self.is_selected(id) {
                        self.select_only(id);
                    }
                    // именно ЭТОТ клип (+группа), а не содержимое региона (п.10)
                    self.delete_selected();
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::SPLIT)).clicked() {
                    if !self.is_selected(id) {
                        self.select_only(id);
                    }
                    self.cut_action();
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::FX)).clicked() {
                    self.open_fx_editor(id);
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::REGION_FROM_EVENT)).clicked() {
                    self.region_from_event(id);
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
                        if !self.is_selected(id) {
                            self.select_only(id);
                        }
                        self.unlink_selected();
                        ui.close();
                    }
                } else if ui.button(tr(k::tl::menu::LINK)).clicked() {
                    if !self.is_selected(id) {
                        self.select_only(id);
                    }
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
                        // (id, фейд ли, я ли уходящая сторона) — по этой
                        // тройке строится ТОЧНОЕ название пункта (п.4)
                        let existing: Vec<(TransitionId, bool, bool)> = p
                            .transitions
                            .iter()
                            .filter(|t| t.touches(id))
                            .map(|t| (t.id, t.is_fade(), t.a == Some(id)))
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
                                tr(k::tl::menu::TRANSITION_NEXT)
                            } else {
                                tr(k::tl::menu::TRANSITION_PREV)
                            };
                            // ВИД выбирается сразу, а не «сначала fade,
                            // потом переделай» — претензия п.1 Р.33.
                            // Подменю: первым идёт растворение (прежний
                            // однокликовый путь остаётся коротким).
                            ui.menu_button(label, |ui| {
                                let mut pick: Option<TransitionKind> = None;
                                let mut cat: Option<crate::i18n::Key> = None;
                                for kind in TransitionKind::ALL {
                                    let c = crate::transition_editor::kind_category(kind);
                                    if cat != Some(c) {
                                        if cat.is_some() {
                                            ui.separator();
                                        }
                                        cat = Some(c);
                                    }
                                    if ui
                                        .button(crate::transition_editor::kind_name(kind))
                                        .clicked()
                                    {
                                        pick = Some(kind.clone());
                                    }
                                }
                                let Some(kind) = pick else { return };
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
                                        kind,
                                        audio: Default::default(),
                                        auto: vec![],
                                    }));
                                }
                                ui.close();
                            });
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
                                    audio: Default::default(),
                                    auto: vec![],
                                }));
                                ui.close();
                            }
                        }
                        for (tid, is_fade, i_am_outgoing) in existing {
                            // название говорит ЧТО и ГДЕ (п.4 пользователя:
                            // «Remove crossfade» одной надписью снимал и
                            // затухание с конца, и кроссфейд с соседом —
                            // «оказывается она fadeout с конца убирает»)
                            let label = match (is_fade, i_am_outgoing) {
                                (true, true) => tr(k::tl::menu::REMOVE_FADE_END),
                                (true, false) => tr(k::tl::menu::REMOVE_FADE_START),
                                (false, true) => tr(k::tl::menu::REMOVE_TRANSITION_NEXT),
                                (false, false) => tr(k::tl::menu::REMOVE_TRANSITION_PREV),
                            };
                            // свойства ПЕРЕД удалением: вид перехода
                            // выбирается здесь и двойным кликом по его
                            // окну (парные способы — п.17 р.28)
                            if ui.button(tr(k::tl::menu::TRANSITION_PROPS)).clicked() {
                                self.open_transition_editor(tid);
                                ui.close();
                            }
                            if ui.button(label).clicked() {
                                self.apply(Command::RemoveTransition { id: tid });
                                ui.close();
                            }
                        }
                    }
                }
                ui.separator();
                // морфер-слой (Р.35): один пункт вместо четырёх, целей
                // не спрашивает — действует на всё, что НИЖЕ
                if ui.button(tr(k::tl::menu::ADD_MORPHER)).clicked() {
                    self.add_morph_layer();
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::SAVE_AREA)).clicked() {
                    self.save_marked_area();
                    ui.close();
                }
            }
            Some(CtxTarget::Camera(cid)) => {
                // камера наконец редактируема (Р.34 К3): свойства и удаление
                if ui.button(tr(k::cam::PROPS)).clicked() {
                    self.open_camera_editor(cid);
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::DELETE)).clicked() {
                    self.apply(Command::RemoveCamera { id: cid });
                    ui.close();
                }
            }
            // Область (Р.38 п.1). До этого раунда меню у неё не было
            // ВООБЩЕ: команды RemoveArea/RenameArea/SetAreaRange/
            // SetAreaLanes лежали в модели написанными, покрытыми
            // юнит-тестами и не вызванными ни разу.
            Some(CtxTarget::Area(i)) => {
                let Some(a) = self.model.project().areas.get(i).cloned() else {
                    ui.close();
                    return;
                };
                ui.label(egui::RichText::new(crate::tf!(k::tl::menu::AREA_TITLE, a.name)).strong());
                if ui.button(tr(k::tl::menu::RENAME)).clicked() {
                    self.area_edit =
                        Some(crate::app::AreaEdit { name: a.name.clone(), buf: a.name.clone() });
                    self.area_edit_focus = true;
                    ui.close();
                }
                let has_region = self.model.project().region.is_some();
                if ui
                    .add_enabled(has_region, egui::Button::new(tr(k::tl::menu::AREA_FROM_REGION)))
                    .clicked()
                {
                    if let Some(r) = self.model.project().region {
                        self.apply(Command::SetAreaRange {
                            name: a.name.clone(),
                            start: r.start,
                            duration: r.end - r.start,
                        });
                    }
                    ui.close();
                }
                // дорожки области — по текущему региону выделения строк:
                // «если юзер передумал, он может поменять/удалить/добавить»
                if !self.last_band.as_ref().map(|b| b.2.is_empty()).unwrap_or(true)
                    && ui.button(tr(k::tl::menu::AREA_LANES_FROM_BAND)).clicked()
                {
                    let lanes = self.last_band.clone().map(|b| b.2).unwrap_or_default();
                    self.apply(Command::SetAreaLanes { name: a.name.clone(), lanes });
                    ui.close();
                }
                ui.menu_button(tr(k::tl::menu::AREA_COLOR), |ui| {
                    for (hex, col) in crate::theme::AREA_PALETTE {
                        let (rect, resp) =
                            ui.allocate_exact_size(egui::vec2(96.0, 16.0), egui::Sense::click());
                        ui.painter().rect_filled(rect.shrink(2.0), 3.0, col);
                        if resp.clicked() {
                            let mut na = a.clone();
                            na.color = Some(hex.to_string());
                            self.apply_batch(vec![
                                Command::RemoveArea { name: a.name.clone() },
                                Command::AddArea(na),
                            ]);
                            ui.close();
                        }
                    }
                });
                if ui.button(tr(k::tl::menu::DELETE)).clicked() {
                    self.apply(Command::RemoveArea { name: a.name.clone() });
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
                    .map(|m| !m.patch.video_fx.is_empty() || !m.patch.audio_fx.is_empty())
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
                // ПАРНОСТЬ с жестом M (п.11 пользователя): при выделенном
                // регионе оба способа дают маркер-ДИАПАЗОН, а не точку.
                // Название пункта говорит, что именно получится (п.4).
                let on_region = self
                    .model
                    .project()
                    .region
                    .map(|r| r.start.0 <= t && t <= r.end.0)
                    .unwrap_or(false);
                let label = if on_region {
                    tr(k::tl::menu::MARKER_REGION)
                } else {
                    tr(k::tl::menu::MARKER_HERE)
                };
                if ui.button(label).clicked() {
                    self.marker_here(Tick(t));
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
                    if ui.button(tr(k::tl::menu::SAVE_AREA)).clicked() {
                        self.save_marked_area();
                        ui.close();
                    }
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
                    (tr(k::tl::lane::ADD_CAMERA), LaneKind::Camera),
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
        // 3 дорожки клипов в высокой панели: высота из панели, скролла нет
        let v = vertical_layout(400.0, 3, 0, 0, None);
        assert_eq!(v.base_h, 84.0, "кламп сверху");
        assert_eq!(v.scroll_max, 0.0);
        // сжатие панели поджимает (как раньше)
        let v = vertical_layout(160.0, 3, 0, 0, None);
        assert!(v.base_h < 84.0 && v.base_h >= 30.0);
        // много дорожек: строки НЕ тоньше 30 — появляется скролл
        let v = vertical_layout(300.0, 12, 0, 0, None);
        assert_eq!(v.base_h, 30.0);
        assert!(v.scroll_max > 0.0);
        let v13 = vertical_layout(300.0, 13, 0, 0, None);
        assert!(v13.scroll_max > v.scroll_max, "ещё дорожка — больше скролл");
        // v_zoom задаёт высоту явно, с клампами
        assert_eq!(vertical_layout(300.0, 12, 0, 0, Some(50.0)).base_h, 50.0);
        assert_eq!(vertical_layout(300.0, 12, 0, 0, Some(4.0)).base_h, 16.0);
        assert_eq!(vertical_layout(300.0, 12, 0, 0, Some(500.0)).base_h, 120.0);
        // при v_zoom, помещающемся целиком, скролла нет
        assert_eq!(vertical_layout(800.0, 3, 0, 0, Some(40.0)).scroll_max, 0.0);
        // КАМЕРНЫЕ строки весят втрое меньше (Р.35 п.1): три камерные
        // дорожки отнимают у видео столько же, сколько одна дорожка клипов
        let a = vertical_layout(300.0, 4, 0, 3, None);
        let b = vertical_layout(300.0, 5, 0, 0, None);
        assert!((a.base_h - b.base_h).abs() < 0.01, "вес камерных строк = 1/3");
        assert!((a.content_h - b.content_h).abs() < 0.01, "три камерные == одна дорожка клипов");
        // морф-строка ВДВОЕ выше камерной (Р.38 п.10): полторы морф-дорожки
        // весят как одна дорожка клипов
        // сравнивать надо base_h, а не content_h: без клампа содержимое по
        // построению заполняет доступную высоту в ОБОИХ случаях
        let m = vertical_layout(300.0, 4, 3, 0, None);
        assert!(m.base_h < a.base_h, "морф-строки отнимают у клипов больше, чем камерные");
        assert!(
            (row_height(LaneKind::Morph, 60.0) / row_height(LaneKind::Camera, 60.0) - 2.0).abs()
                < 0.01,
            "морф-строка ровно вдвое выше камерной"
        );
        // и они не съедают высоту у видео: с тремя камерными строка
        // клипов ровно та же, что была бы с одной лишней дорожкой клипов
        let plain = vertical_layout(300.0, 4, 0, 0, None);
        assert!(a.base_h < plain.base_h, "служебные строки всё же место занимают");
        // пол высоты: на мелком зуме тонкая строка не тоньше 10 px
        assert_eq!(row_height(LaneKind::Morph, 12.0), THIN_ROW_MIN);
        assert_eq!(row_height(LaneKind::Camera, 16.0), THIN_ROW_MIN);
        assert_eq!(row_height(LaneKind::Video, 16.0), 16.0);
        assert!((row_height(LaneKind::Camera, 84.0) - 28.0).abs() < 0.01);
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
        let g = TlGeom::new(
            egui::Rect::from_min_max(egui::pos2(52.0, 100.0), egui::pos2(1052.0, 500.0)),
            50.0,
            vec![
                (LaneKind::Video, 0),
                (LaneKind::Video, 1),
                (LaneKind::Audio, 0),
                (LaneKind::Audio, 1),
                (LaneKind::Morph, 0),
            ],
            &[],
            0.0,
            90_000.0,
            900.0,
        );
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
        let g = TlGeom::new(
            egui::Rect::from_min_max(egui::pos2(52.0, 100.0), egui::pos2(1052.0, 500.0)),
            40.0,
            rows.clone(),
            &[],
            0.0,
            0.0,
            900.0,
        );
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
