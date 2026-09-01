//! Чистые операции редактирования (без UI): буфер обмена, регион, трим групп.
//! Семантика перенесена из старого timelineeditor.cpp (copy/paste/punch-out).
//! Все функции возвращают команды для `TimelineModel::apply_batch` —
//! один undo-снапшот на жест, атомарность бесплатно.

use crate::model::{Command, MIN_CLIP_DURATION};
use crate::project::*;
use kt_time::Tick;

/// Блок обмена (Р.24): клипы + переходы, у которых ОБА конца попали в блок
/// и стык пережил кроп. Времена клипов ребейзнуты к нулю блока.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipBlock {
    pub clips: Vec<ClipEvent>,
    pub transitions: Vec<TransitionEvent>,
}

/// Переходы, пережившие копирование: живые концы в блоке, стык цел (кропы
/// региона могли съесть край), офсеты клампятся новыми телами. Фейд (Р.25)
/// переезжает вместе со своим единственным клипом.
fn surviving_transitions(p: &Project, block_clips: &[ClipEvent]) -> Vec<TransitionEvent> {
    let find = |id: ClipId| block_clips.iter().find(|c| c.id == id);
    let side = |id: Option<ClipId>| match id {
        None => Some(None),
        Some(id) => find(id).map(Some),
    };
    p.transitions
        .iter()
        .filter_map(|t| {
            let (a, b) = (side(t.a)?, side(t.b)?);
            if let (Some(a), Some(b)) = (a, b)
                && a.start + a.duration != b.start
            {
                return None;
            }
            let mut t = t.clone();
            if let Some(a) = a {
                t.in_offset = t.in_offset.min(a.duration);
            }
            if let Some(b) = b {
                t.out_offset = t.out_offset.min(b.duration);
            }
            (t.in_offset + t.out_offset >= MIN_TRANSITION_DURATION).then_some(t)
        })
        .collect()
}

/// Copy с регионом: пересекающиеся клипы кропаются по региону и ребейзятся
/// к нулю (source_in сдвигается на срез слева). Блок готов для paste_block.
pub fn copy_region_block(p: &Project, r: Region) -> ClipBlock {
    let mut clips = Vec::new();
    for c in &p.clips {
        if !r.intersects_clip(c) {
            continue;
        }
        let t0 = c.start.max(r.start);
        let t1 = (c.start + c.duration).min(r.end);
        if t1 - t0 < MIN_CLIP_DURATION {
            continue;
        }
        let mut copy = c.clone();
        // реверс (п.16): кроп слева таймлайна режет ХВОСТ источника,
        // source_in двигается только от кропа СПРАВА
        copy.source_in = if c.reversed {
            c.source_in + ((c.start + c.duration) - t1)
        } else {
            c.source_in + (t0 - c.start)
        };
        copy.start = t0 - r.start; // rebase к нулю региона
        copy.duration = t1 - t0;
        clips.push(copy);
    }
    let transitions = surviving_transitions(p, &clips);
    ClipBlock { clips, transitions }
}

/// Copy без региона: выбранный клип со всей его link-группой,
/// ребейз к старту самого раннего.
pub fn copy_group(p: &Project, id: ClipId) -> ClipBlock {
    let Some(sel) = p.clips.iter().find(|c| c.id == id) else { return ClipBlock::default() };
    let group = sel.link_group;
    let members: Vec<ClipEvent> = p
        .clips
        .iter()
        .filter(|c| c.id == id || (group.is_some() && c.link_group == group))
        .cloned()
        .collect();
    let base = members.iter().map(|c| c.start).min().unwrap_or(Tick::ZERO);
    let clips: Vec<ClipEvent> = members
        .into_iter()
        .map(|mut c| {
            c.start = c.start - base;
            c
        })
        .collect();
    let transitions = surviving_transitions(p, &clips);
    ClipBlock { clips, transitions }
}

/// Paste блока в позицию `at`: свежие id, каждая старая группа → одна новая,
/// переходы ретаргетятся на новые id. Возвращает (команды, конец блока) —
/// курсор прыгает в конец для chain-paste. Вставка с коллизией на дорожке
/// теперь честно проваливается жёсткой валидацией apply_batch (п.139).
pub fn paste_block(
    block: &ClipBlock,
    at: Tick,
    next_clip: &mut u64,
    next_group: &mut u64,
    next_transition: &mut u64,
) -> (Vec<Command>, Tick) {
    let mut group_map: std::collections::HashMap<GroupId, GroupId> =
        std::collections::HashMap::new();
    let mut clip_map: std::collections::HashMap<ClipId, ClipId> = std::collections::HashMap::new();
    let mut cmds = Vec::new();
    let mut end = at;
    for c in &block.clips {
        let mut copy = c.clone();
        copy.id = ClipId(*next_clip);
        clip_map.insert(c.id, copy.id);
        *next_clip += 1;
        copy.link_group = c.link_group.map(|g| {
            *group_map.entry(g).or_insert_with(|| {
                let ng = GroupId(*next_group);
                *next_group += 1;
                ng
            })
        });
        copy.start = at + c.start;
        end = end.max(copy.start + copy.duration);
        cmds.push(Command::AddClip(copy));
    }
    for t in &block.transitions {
        let remap = |id: Option<ClipId>| match id {
            None => Some(None),
            Some(id) => clip_map.get(&id).copied().map(Some),
        };
        let (Some(a), Some(b)) = (remap(t.a), remap(t.b)) else { continue };
        let mut copy = t.clone();
        copy.id = TransitionId(*next_transition);
        *next_transition += 1;
        copy.a = a;
        copy.b = b;
        cmds.push(Command::AddTransition(copy));
    }
    (cmds, end)
}

/// Punch-out (старый splitDeleteRegionInterior): у каждого пересекающего
/// клипа вырезается интерьер региона; остатки получают новые id и ТЕРЯЮТ
/// link-группу; правый остаток сохраняет выравнивание источника. Без ripple.
pub fn punch_out_region(p: &Project, r: Region, next_clip: &mut u64) -> Vec<Command> {
    let mut cmds = Vec::new();
    for c in &p.clips {
        if !r.intersects_clip(c) {
            continue;
        }
        cmds.push(Command::RemoveClip { id: c.id });
        let end = c.start + c.duration;
        // левый остаток
        if r.start - c.start >= MIN_CLIP_DURATION {
            let mut left = c.clone();
            left.id = ClipId(*next_clip);
            *next_clip += 1;
            left.link_group = None;
            left.duration = r.start - c.start;
            if c.reversed {
                // реверс (п.16): левый кусок играет хвост источника
                left.source_in = c.source_in + (end - r.start);
            }
            cmds.push(Command::AddClip(left));
        }
        // правый остаток
        if end - r.end >= MIN_CLIP_DURATION {
            let mut right = c.clone();
            right.id = ClipId(*next_clip);
            *next_clip += 1;
            right.link_group = None;
            if !c.reversed {
                right.source_in = c.source_in + (r.end - c.start);
            } // reversed: правый кусок играет НАЧАЛО источника — source_in как есть
            right.start = r.end;
            right.duration = end - r.end;
            cmds.push(Command::AddClip(right));
        }
    }
    cmds
}

/// Del с регионом: пересекающиеся клипы И их linked-партнёры.
pub fn delete_region(p: &Project, r: Region) -> Vec<Command> {
    let mut doomed: std::collections::HashSet<ClipId> = std::collections::HashSet::new();
    let mut groups: std::collections::HashSet<GroupId> = std::collections::HashSet::new();
    for c in &p.clips {
        if r.intersects_clip(c) {
            doomed.insert(c.id);
            if let Some(g) = c.link_group {
                groups.insert(g);
            }
        }
    }
    for c in &p.clips {
        if let Some(g) = c.link_group
            && groups.contains(&g)
        {
            doomed.insert(c.id);
        }
    }
    let mut ids: Vec<ClipId> = doomed.into_iter().collect();
    ids.sort();
    ids.into_iter().map(|id| Command::RemoveClip { id }).collect()
}

/// Trim-to-cursor для всей link-группы (как в старом редакторе: партнёры
/// получают тот же срез). Пустой результат = курсор вне клипа.
pub fn trim_group_start_to(p: &Project, sel: ClipId, at: Tick) -> Vec<Command> {
    group_members(p, sel)
        .into_iter()
        .filter(|c| c.start < at && at < c.start + c.duration - MIN_CLIP_DURATION + Tick(1))
        .filter(|c| c.reversed || (c.source_in + (at - c.start)).0 >= 0)
        .map(|c| Command::TrimStart { id: c.id, new_start: at })
        .collect()
}

pub fn trim_group_end_to(p: &Project, sel: ClipId, at: Tick) -> Vec<Command> {
    group_members(p, sel)
        .into_iter()
        .filter(|c| at - c.start >= MIN_CLIP_DURATION && at < c.start + c.duration)
        .map(|c| Command::TrimEnd { id: c.id, new_end: at })
        .collect()
}

/// Возврат «ручек» умерших кроссфейдов (Р.26, решение пользователя: «в sony
/// vegas если растащить перекрывающиеся события — остаётся то, что
/// перекрывалось, без обрезки»).
///
/// При создании кроссфейда клипы режутся до стыка, и офсеты перехода
/// ЧИСЛЕННО РАВНЫ срезанному: outgoing потерял справа `out_offset`, incoming
/// слева — `in_offset` (так считает и plan_move в UI, и upgrade_legacy при
/// импорте .ktproj). Поэтому разрыв перехода восстановим точно, без хранения
/// лишнего состояния.
///
/// `before` — проект до жеста (там переходы ещё живы), `after` — его
/// результат (`TimelineModel::preview`). Возврат клампится освободившимся
/// местом, поэтому батч не может провалиться на перекрытии.
///
/// Фейды пропускаются: у них ничего не срезалось (заводятся явной командой).
pub fn restore_handles(before: &Project, after: &Project) -> Vec<Command> {
    let mut cmds = Vec::new();
    for t in &before.transitions {
        if t.is_fade() || after.transitions.iter().any(|x| x.id == t.id) {
            continue;
        }
        let (Some(a_id), Some(b_id)) = (t.a, t.b) else { continue };
        let a = after.clips.iter().find(|c| c.id == a_id);
        let b = after.clips.iter().find(|c| c.id == b_id);
        let mut want_a = if a.is_some() { t.out_offset.0 } else { 0 };
        let mut want_b = if b.is_some() { t.in_offset.0 } else { 0 };
        // место до ближайшего препятствия, с учётом ВСЕХ членов link-группы:
        // трим двигает группу целиком, и упереться может любой её член
        let room_a = a.map_or(0, |c| room_right(after, c));
        let room_b = b.map_or(0, |c| room_left(after, c));
        want_a = want_a.min(room_a);
        want_b = want_b.min(room_b);
        // материал за краем не бесконечен: у reversed TrimEnd тянет source_in
        // вниз, у обычного TrimStart — тоже, и check_on отвергает уход ниже
        // нуля. Кламп ОБЯЗАН быть здесь: иначе подарок уронит весь жест
        // (перенос/удаление откатится вместе с ним).
        if let Some(a) = a
            && a.reversed
        {
            want_a = want_a.min(a.source_in.0);
        }
        if let Some(b) = b
            && !b.reversed
        {
            want_b = want_b.min(b.source_in.0);
        }
        // клипы пары могли остаться соседями (растащили чуть-чуть): тогда
        // общий зазор делится пропорционально тому, кто сколько потерял
        if let (Some(a), Some(b)) = (a, b)
            && a.kind == b.kind
            && a.lane == b.lane
        {
            let gap = b.start.0 - (a.start + a.duration).0;
            if gap >= 0 && want_a + want_b > gap {
                let total = want_a + want_b;
                let share = if total > 0 { gap * want_a / total } else { 0 };
                want_a = share;
                want_b = gap - share;
            }
        }
        if let Some(a) = a
            && want_a > 0
        {
            cmds.push(Command::TrimEnd { id: a.id, new_end: a.start + a.duration + Tick(want_a) });
        }
        if let Some(b) = b
            && want_b > 0
        {
            cmds.push(Command::TrimStart { id: b.id, new_start: b.start - Tick(want_b) });
        }
    }
    cmds
}

/// Сколько тиков клип (со всей link-группой) может вырасти вправо, не задев
/// соседей и чужие плейсхолдеры. Край таймлайна справа не ограничивает.
fn room_right(p: &Project, c: &ClipEvent) -> i64 {
    group_members(p, c.id)
        .iter()
        .map(|m| {
            let end = (m.start + m.duration).0;
            obstacles(p, m).filter(|(s, _)| *s >= end).map(|(s, _)| s - end).min().unwrap_or(i64::MAX)
        })
        .min()
        .unwrap_or(0)
        .max(0)
}

/// То же влево; слева таймлайн ограничен нулём.
fn room_left(p: &Project, c: &ClipEvent) -> i64 {
    group_members(p, c.id)
        .iter()
        .map(|m| {
            let start = m.start.0;
            obstacles(p, m)
                .filter(|(_, e)| *e <= start)
                .map(|(_, e)| start - e)
                .min()
                .unwrap_or(start)
                .min(start)
        })
        .min()
        .unwrap_or(0)
        .max(0)
}

/// Занятые интервалы на дорожке клипа: чужие клипы (вне его link-группы) и
/// плейсхолдеры чужих конструкций — инвариант «без перекрытий» включает их.
fn obstacles<'a>(p: &'a Project, c: &'a ClipEvent) -> impl Iterator<Item = (i64, i64)> + 'a {
    let group = c.link_group;
    p.clips
        .iter()
        .filter(move |o| {
            o.kind == c.kind
                && o.lane == c.lane
                && o.id != c.id
                && (group.is_none() || o.link_group != group)
        })
        .map(|o| (o.start.0, (o.start + o.duration).0))
        .chain(
            p.foreign
                .iter()
                .filter(move |f| f.kind == c.kind && f.lane == c.lane)
                .map(|f| (f.start.0, (f.start + f.duration).0)),
        )
}

fn group_members(p: &Project, sel: ClipId) -> Vec<ClipEvent> {
    let Some(s) = p.clips.iter().find(|c| c.id == sel) else { return Vec::new() };
    let group = s.link_group;
    p.clips
        .iter()
        .filter(|c| c.id == sel || (group.is_some() && c.link_group == group))
        .cloned()
        .collect()
}

/// Магнетизация A/V-пары (п.6): партнёр противоположного типа с ближайшим
/// стартом подтягивается к выделенному и линкуется. None — партнёра нет;
/// Some(vec![]) — уже слинкованы и выровнены (no-op). Партнёр
/// предварительно отвязывается — иначе MoveClip двигал ВСЮ его группу
/// (включая сам выделенный) и пара никогда не сходилась.
pub fn magnetize_commands(
    p: &Project,
    sel: ClipId,
    threshold: Tick,
    next_group: &mut u64,
) -> Option<Vec<Command>> {
    let c = p.clips.iter().find(|c| c.id == sel)?;
    let partner = p
        .clips
        .iter()
        .filter(|o| o.kind != c.kind)
        .min_by_key(|o| (o.start.0 - c.start.0).abs())?;
    if (partner.start.0 - c.start.0).abs() > threshold.0 {
        return None;
    }
    let aligned = partner.start == c.start && partner.duration == c.duration;
    let linked = c.link_group.is_some() && partner.link_group == c.link_group;
    if aligned && linked {
        return Some(Vec::new()); // уже намагничены
    }
    let group = c.link_group.unwrap_or_else(|| {
        let g = GroupId(*next_group);
        *next_group += 1;
        g
    });
    let mut cmds = Vec::new();
    if partner.link_group.is_some() {
        cmds.push(Command::UnlinkClip { id: partner.id });
    }
    cmds.push(Command::MoveClip { id: partner.id, new_start: c.start, new_lane: partner.lane });
    cmds.push(Command::TrimEnd { id: partner.id, new_end: c.start + c.duration });
    cmds.push(Command::LinkClips { ids: vec![c.id, partner.id], group });
    Some(cmds)
}

// ---- импорт легаси-ktproj: перекрытия → переходы/развод (Р.24, п.143) ------

/// Отчёт конвертации старых перекрытий. Показывается пользователю note'ом.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UpgradeReport {
    /// Аудио-пары, ставшие стыком с переходом (звук байт-идентичен, т.136).
    pub transitions_created: usize,
    /// Аудио-клипы, разведённые по дорожкам (3+/FX-перекрытия: точной
    /// компенсации 1/N не существует — решение пользователя п.9 Р.24,
    /// громкость в зоне бывшего 1/N становится суммой).
    pub audio_moved: usize,
    /// Видео-клипы, демоцированные на вставленную дорожку ниже (старое
    /// видео перекрытия не смешивало — композит байт-идентичен тайбрейку).
    pub video_moved: usize,
}

impl UpgradeReport {
    pub fn is_empty(&self) -> bool {
        *self == UpgradeReport::default()
    }
}

/// Зеркала мутаций тримов (model.rs) для прямого применения к клипу:
/// у reversed TrimEnd сдвигает source_in, TrimStart — не трогает (п.16).
fn raw_trim_end(c: &mut ClipEvent, new_end: Tick) {
    let old = c.duration;
    c.duration = new_end - c.start;
    if c.reversed {
        c.source_in = c.source_in + (old - c.duration);
    }
}

fn raw_trim_start(c: &mut ClipEvent, new_start: Tick) {
    let delta = new_start - c.start;
    c.start = new_start;
    c.duration = c.duration - delta;
    if !c.reversed {
        c.source_in = c.source_in + delta;
    }
}

/// Конвертация legacy-проекта с перекрытиями в Transition-модель (этап 9).
/// Идемпотентна: на проекте без перекрытий отчёт пуст. После вызова
/// проект обязан проходить validate().
pub fn upgrade_legacy(p: &mut Project) -> UpgradeReport {
    let mut report = UpgradeReport::default();

    // 1) аудио-пары: ровно два клипа в зоне наезда, оба без активных FX —
    //    трим до середины наезда + переход тем же окном (байт-идентично)
    while let Some((a_id, b_id)) = find_audio_pair(p) {
        let a0 = p.clips.iter().find(|c| c.id == a_id).unwrap().clone();
        let b0 = p.clips.iter().find(|c| c.id == b_id).unwrap().clone();
        let (ov0, ov1) = (b0.start, a0.start + a0.duration);
        let joint = Tick(
            (ov0.0 + (ov1.0 - ov0.0) / 2).clamp(
                (a0.start + MIN_CLIP_DURATION).0,
                (b0.start + b0.duration - MIN_CLIP_DURATION).0,
            ),
        );
        for c in p.clips.iter_mut() {
            if c.id == a_id {
                raw_trim_end(c, joint);
            } else if c.id == b_id {
                raw_trim_start(c, joint);
            }
        }
        let id = p.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
        p.transitions.push(TransitionEvent {
            id: TransitionId(id),
            a: Some(a_id),
            b: Some(b_id),
            in_offset: joint - ov0,
            out_offset: ov1 - joint,
            kind: TransitionKind::Crossfade,
        });
        report.transitions_created += 1;
    }

    // 2) оставшиеся аудио-перекрытия (3+, FX, невозможная пара) — развод:
    //    ранний остаётся, конфликтующий уезжает на свободную дорожку ниже
    //    (новая в конце при нехватке)
    while let Some(mover) = find_overlap_mover(p, ClipKind::Audio) {
        let lane = free_audio_lane_below(p, mover);
        // счётчик дорожек бампается СРАЗУ: иначе следующий mover считает
        // добавленную дорожку несуществующей и летит на неё же (вечный цикл)
        p.audio_lanes = p.audio_lanes.max(lane + 1);
        if let Some(c) = p.clips.iter_mut().find(|c| c.id == mover) {
            c.lane = lane;
        }
        report.audio_moved += 1;
    }

    // 3) видео-перекрытия: демоция позднего на ВСТАВЛЕННУЮ дорожку сразу
    //    ниже раннего — старый тайбрейк (сверху тот, у кого start меньше)
    //    сохраняется байт-в-байт
    while let Some(mover) = find_overlap_mover(p, ClipKind::Video) {
        let keeper_lane =
            p.clips.iter().find(|c| c.id == mover).map(|c| c.lane).unwrap_or(0);
        for c in p.clips.iter_mut() {
            if c.kind == ClipKind::Video && c.lane > keeper_lane {
                c.lane += 1;
            }
        }
        if let Some(c) = p.clips.iter_mut().find(|c| c.id == mover) {
            c.lane = keeper_lane + 1;
        }
        report.video_moved += 1;
    }

    p.normalize();
    report
}

/// Пара для перехода: B — ближайший наезжающий на A, зона наезда покрыта
/// РОВНО ими двумя, наезд частичный, оба без активных FX, стык умещается.
fn find_audio_pair(p: &Project) -> Option<(ClipId, ClipId)> {
    for a in p.clips.iter().filter(|c| c.kind == ClipKind::Audio) {
        for b in p.clips.iter().filter(|c| c.kind == ClipKind::Audio) {
            if a.id == b.id || a.lane != b.lane {
                continue;
            }
            let (a1, b1) = (a.start + a.duration, b.start + b.duration);
            // частичный наезд B на хвост A
            if !(b.start > a.start && b.start < a1 && b1 > a1) {
                continue;
            }
            if !a.active_fx().is_empty() || !b.active_fx().is_empty() {
                continue;
            }
            // никто третий не задевает зону наезда
            let zone = (b.start, a1);
            let third = p.clips.iter().any(|c| {
                c.id != a.id
                    && c.id != b.id
                    && c.kind == ClipKind::Audio
                    && c.lane == a.lane
                    && c.start < zone.1
                    && c.start + c.duration > zone.0
            });
            if third {
                continue;
            }
            // окно и стык умещаются в тела
            if (a1 - b.start) < MIN_TRANSITION_DURATION
                || a.start + MIN_CLIP_DURATION > b1 - MIN_CLIP_DURATION
            {
                continue;
            }
            return Some((a.id, b.id));
        }
    }
    None
}

/// Первый конфликтующий клип вида `kind`: в каждой зоне перекрытия остаётся
/// самый ранний (по (start, id)), следующий — кандидат на переезд.
fn find_overlap_mover(p: &Project, kind: ClipKind) -> Option<ClipId> {
    let mut spans: Vec<(&ClipEvent, Tick)> = p
        .clips
        .iter()
        .filter(|c| c.kind == kind)
        .map(|c| (c, c.start + c.duration))
        .collect();
    spans.sort_by_key(|(c, _)| (c.lane, c.start, c.id));
    for w in spans.windows(2) {
        let ((a, a_end), (b, _)) = (&w[0], &w[1]);
        if a.lane == b.lane && b.start < *a_end {
            return Some(b.id);
        }
    }
    None
}

fn free_audio_lane_below(p: &Project, mover: ClipId) -> u32 {
    let me = p.clips.iter().find(|c| c.id == mover).unwrap();
    let (m0, m1) = (me.start, me.start + me.duration);
    for lane in (me.lane + 1)..p.audio_lanes {
        let busy = p.clips.iter().any(|c| {
            c.id != mover
                && c.kind == ClipKind::Audio
                && c.lane == lane
                && c.start < m1
                && c.start + c.duration > m0
        });
        if !busy {
            return lane;
        }
    }
    p.audio_lanes
}
