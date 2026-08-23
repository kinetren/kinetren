//! Чистые операции редактирования (без UI): буфер обмена, регион, трим групп.
//! Семантика перенесена из старого timelineeditor.cpp (copy/paste/punch-out).
//! Все функции возвращают команды для `TimelineModel::apply_batch` —
//! один undo-снапшот на жест, атомарность бесплатно.

use crate::model::{Command, MIN_CLIP_DURATION};
use crate::project::*;
use kt_time::Tick;

/// Copy с регионом: пересекающиеся клипы кропаются по региону и ребейзятся
/// к нулю (source_in сдвигается на срез слева). Блок готов для paste_block.
pub fn copy_region_block(p: &Project, r: Region) -> Vec<ClipEvent> {
    let mut out = Vec::new();
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
        out.push(copy);
    }
    out
}

/// Copy без региона: выбранный клип со всей его link-группой,
/// ребейз к старту самого раннего.
pub fn copy_group(p: &Project, id: ClipId) -> Vec<ClipEvent> {
    let Some(sel) = p.clips.iter().find(|c| c.id == id) else { return Vec::new() };
    let group = sel.link_group;
    let members: Vec<ClipEvent> = p
        .clips
        .iter()
        .filter(|c| c.id == id || (group.is_some() && c.link_group == group))
        .cloned()
        .collect();
    let base = members.iter().map(|c| c.start).min().unwrap_or(Tick::ZERO);
    members
        .into_iter()
        .map(|mut c| {
            c.start = c.start - base;
            c
        })
        .collect()
}

/// Paste блока в позицию `at`: свежие id, каждая старая группа → одна новая.
/// Возвращает (команды, конец блока) — курсор прыгает в конец для chain-paste.
pub fn paste_block(
    block: &[ClipEvent],
    at: Tick,
    next_clip: &mut u64,
    next_group: &mut u64,
) -> (Vec<Command>, Tick) {
    let mut group_map: std::collections::HashMap<GroupId, GroupId> =
        std::collections::HashMap::new();
    let mut cmds = Vec::new();
    let mut end = at;
    for c in block {
        let mut copy = c.clone();
        copy.id = ClipId(*next_clip);
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
