//! Чистые операции редактирования (без UI): буфер обмена, регион, трим групп.
//! Семантика перенесена из старого timelineeditor.cpp (copy/paste/punch-out).
//! Все функции возвращают команды для `TimelineModel::apply_batch` —
//! один undo-снапшот на жест, атомарность бесплатно.

use crate::model::{Command, MIN_CLIP_DURATION};
use crate::project::*;
use kt_time::{Tick, TickRange};

/// Блок обмена (Р.24): клипы + переходы, у которых ОБА конца попали в блок
/// и стык пережил кроп. Времена клипов ребейзнуты к нулю блока.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClipBlock {
    pub clips: Vec<ClipEvent>,
    pub transitions: Vec<TransitionEvent>,
    /// Морферы, чьи цели попали в блок (Р.35 п.4). Цели за пределами
    /// блока выброшены ещё при захвате.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morphers: Vec<MorpherEvent>,
    /// Именованные области, пересёкшие окно блока (кроп + ребейз).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<MarkedArea>,
    /// Метки: диапазонные — по пересечению с кропом, точечные — если
    /// внутри окна.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Marker>,
}

/// Свежие id для вставки — растущий список аргументов `paste_block`
/// перестал читаться уже на четвёртом.
pub struct PasteIds<'a> {
    pub clip: &'a mut u64,
    pub group: &'a mut u64,
    pub transition: &'a mut u64,
    pub morph: &'a mut u64,
}

/// Как называть вставленное именованное (Р.35 п.4: «Имена — уникальные,
/// и можно попросить пользователя ввести префикс, а можно и сгенерировать
/// в поле его»; Р.38 п.19 — умолчание стало СУФФИКСОМ). `taken` — занятые
/// имена ПРОЕКТА.
#[derive(Default)]
pub struct PasteNames {
    /// Слово, которым помечается копия.
    pub affix: String,
    /// Ставить его СПЕРЕДИ. Умолчание — сзади (Р.38 п.19, дословно: «при
    /// копировании элементов - надо по дефолту проставлять не префикс а
    /// суффикс»): так копии сортируются рядом с оригиналом, а не
    /// собираются в отдельную кучу по первой букве.
    pub before: bool,
    pub taken: Vec<String>,
}

/// Что вставка пропустила. Батч атомарен, поэтому один плохой элемент
/// уронил бы ВСЮ вставку — а выглядело бы это как «копипаст не работает».
/// Пропущенное считается и уходит в сообщение, а не «доправляется»
/// (правило 20).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PasteSkipped {
    /// Морферы, у которых не уцелело ни одной цели.
    pub morphers: usize,
    /// Метки на уже занятый тик.
    pub markers: usize,
}

/// Длина блока по клипам (окно, в котором ловятся области и метки).
fn block_len(clips: &[ClipEvent]) -> Tick {
    clips.iter().map(|c| c.start + c.duration).max().unwrap_or(Tick::ZERO)
}

/// Морферы, области и метки, попавшие в блок (Р.35 п.4, дословно: «При
/// выделении на таймлайне всего подряд и ctrl+c ctrl+v я хочу чтобы у
/// меня проставлялись связи и для морферов тоже… и marked_area-ы тоже
/// при выделении и копировании копируются, и marked_region тоже вместе с
/// теми что внутри него»).
///
/// `base` — начало окна на таймлайне (для ребейза), `len` — его длина.
/// Морфер едет, если в блоке ХОТЯ БЫ ОДНА его цель; цели снаружи
/// выбрасываются. Область и метка-диапазон — по пересечению, с кропом;
/// точечная метка — если внутри окна.
fn block_extras(p: &Project, clips: &[ClipEvent], base: Option<Tick>, len: Tick) -> ClipBlock {
    let base = base.unwrap_or(Tick::ZERO);
    let win = kt_time::TickRange::new(base, base + len);
    let ids: Vec<ClipId> = clips.iter().map(|c| c.id).collect();
    let morphers = p
        .morphers
        .iter()
        .filter_map(|m| {
            let kept: Vec<ClipId> = match &m.targets {
                MorphTargets::Clips { ids: t } => {
                    t.iter().copied().filter(|i| ids.contains(i)).collect()
                }
                // слой и ссылки целей не морозят: они едут, если сам блок
                // морфера попал в окно
                _ => {
                    return m
                        .window()
                        .overlaps(&win)
                        .then(|| rebase_morpher(m, base));
                }
            };
            (!kept.is_empty()).then(|| {
                let mut copy = rebase_morpher(m, base);
                copy.targets = MorphTargets::Clips { ids: kept };
                copy
            })
        })
        .collect();
    let areas = p
        .areas
        .iter()
        .filter_map(|a| {
            let r = a.range().intersect(&win);
            (!r.is_empty()).then(|| MarkedArea {
                start: r.start - base,
                duration: r.len(),
                ..a.clone()
            })
        })
        .collect();
    let markers = p
        .markers
        .iter()
        .filter_map(|m| {
            if m.duration.0 > 0 {
                let r = kt_time::TickRange::new(m.t, m.t + m.duration).intersect(&win);
                (!r.is_empty()).then(|| Marker {
                    t: r.start - base,
                    duration: r.len(),
                    ..m.clone()
                })
            } else {
                win.contains(m.t).then(|| Marker { t: m.t - base, ..m.clone() })
            }
        })
        .collect();
    ClipBlock { morphers, areas, markers, ..Default::default() }
}

fn rebase_morpher(m: &MorpherEvent, base: Tick) -> MorpherEvent {
    MorpherEvent { start: m.start - base, ..m.clone() }
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
        // кроп — тот же кусок, что у Split и punch-out (Р.39): реверс,
        // материальный якорь и сдвиг ключей считает `clip_slice`
        let mut copy = clip_slice(c, TickRange::new(t0, t1), c.id);
        copy.start = t0 - r.start; // rebase к нулю региона
        clips.push(copy);
    }
    let transitions = surviving_transitions(p, &clips);
    let extras = block_extras(p, &clips, Some(r.start), r.end - r.start);
    ClipBlock { clips, transitions, ..extras }
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
    let extras = block_extras(p, &clips, Some(base), block_len(&clips));
    ClipBlock { clips, transitions, ..extras }
}

/// Copy множества выделенных событий (Р.33): каждое тянет за собой свою
/// link-группу, ребейз к старту самого раннего. `copy_group` — частный
/// случай с одним id; отдельная функция нужна, чтобы блок из нескольких
/// событий сохранял их взаимные СДВИГИ (вставка кладёт их той же фигурой).
pub fn copy_clips(p: &Project, ids: &[ClipId]) -> ClipBlock {
    let groups: Vec<GroupId> =
        p.clips.iter().filter(|c| ids.contains(&c.id)).filter_map(|c| c.link_group).collect();
    let members: Vec<ClipEvent> = p
        .clips
        .iter()
        .filter(|c| ids.contains(&c.id) || c.link_group.is_some_and(|g| groups.contains(&g)))
        .cloned()
        .collect();
    if members.is_empty() {
        return ClipBlock::default();
    }
    let base = members.iter().map(|c| c.start).min().unwrap_or(Tick::ZERO);
    let clips: Vec<ClipEvent> = members
        .into_iter()
        .map(|mut c| {
            c.start = c.start - base;
            c
        })
        .collect();
    let transitions = surviving_transitions(p, &clips);
    let extras = block_extras(p, &clips, Some(base), block_len(&clips));
    ClipBlock { clips, transitions, ..extras }
}

/// Paste блока в позицию `at`: свежие id, каждая старая группа → одна новая,
/// переходы ретаргетятся на новые id. Возвращает (команды, конец блока) —
/// курсор прыгает в конец для chain-paste. Вставка с коллизией на дорожке
/// теперь честно проваливается жёсткой валидацией apply_batch (п.139).
pub fn paste_block(
    block: &ClipBlock,
    at: Tick,
    ids: PasteIds<'_>,
    names: &PasteNames,
) -> (Vec<Command>, Tick, PasteSkipped) {
    let PasteIds { clip: next_clip, group: next_group, transition: next_transition, morph } = ids;
    let mut skipped = PasteSkipped::default();
    let mut taken = names.taken.clone();
    // имена вставленного: приставка + уникализация ТОЙ ЖЕ функцией, что у
    // команд — разойдись они, батч упал бы на собственных суффиксах
    let mut name_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut rename = |old: &str| -> String {
        if let Some(n) = name_map.get(old) {
            return n.clone();
        }
        let a = names.affix.trim();
        let wanted = if a.is_empty() {
            old.to_string()
        } else if names.before {
            format!("{a} {old}")
        } else {
            format!("{old} {a}")
        };
        let fresh = unique_name(&taken, &wanted, "copy");
        taken.push(fresh.clone());
        name_map.insert(old.to_string(), fresh.clone());
        fresh
    };
    let mut group_map: std::collections::HashMap<GroupId, GroupId> =
        std::collections::HashMap::new();
    let mut clip_map: std::collections::HashMap<ClipId, ClipId> = std::collections::HashMap::new();
    let mut cmds = Vec::new();
    let mut end = at;
    for c in &block.clips {
        let mut copy = c.clone();
        copy.id = ClipId(*next_clip);
        clip_map.insert(c.id, copy.id);
        // родство — для конверта (п.357): вставленная копия обязана унести
        // имя, чужие эффекты и metadata оригинала
        cmds.push(Command::NoteBirth { child: copy.id, parent: c.id });
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
    // области и метки: имена переименовываются ПЕРВЫМИ — ссылки морферов
    // ниже смотрят в уже готовую карту
    for a in &block.areas {
        let mut copy = a.clone();
        copy.name = rename(&a.name);
        copy.start = at + a.start;
        cmds.push(Command::AddArea(copy));
    }
    for m in &block.markers {
        let t = at + m.t;
        if block.markers.iter().any(|o| !std::ptr::eq(o, m) && at + o.t == t) {
            skipped.markers += 1;
            continue;
        }
        cmds.push(Command::AddMarker { at: t, duration: m.duration });
        if !m.name.is_empty() {
            cmds.push(Command::SetMarkerName { at: t, name: rename(&m.name) });
        }
    }
    for m in &block.morphers {
        let mut copy = m.clone();
        copy.id = MorphId(*morph);
        copy.start = at + m.start;
        match &mut copy.targets {
            MorphTargets::Clips { ids } => {
                // цель, не пережившая копирование, тянет за собой весь
                // морфер: батч атомарен, а морфер без целей бессмыслен
                ids.retain(|i| clip_map.contains_key(i));
                if ids.is_empty() {
                    skipped.morphers += 1;
                    continue;
                }
                for i in ids.iter_mut() {
                    *i = clip_map[i];
                }
            }
            // ссылка на СКОПИРОВАННОЕ имя переезжает на копию, на
            // нескопированное — остаётся на оригинале (он никуда не делся)
            MorphTargets::Areas { names } | MorphTargets::Marks { names } => {
                for n in names.iter_mut() {
                    if let Some(new) = name_map.get(n.as_str()) {
                        *n = new.clone();
                    }
                }
            }
            MorphTargets::Below => {}
        }
        *morph += 1;
        end = end.max(copy.start + copy.duration);
        cmds.push(Command::AddMorpher(copy));
    }
    (cmds, end, skipped)
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
        // Оба остатка — через ОДНО правило куска (Р.39). Раньше здесь была
        // своя копия линейной формулы (по ветке на каждый остаток), и на
        // растянутом клипе она уезжала так же, как уезжал `Split`.
        // Реверс, `source_in` и сдвиг ключей — забота `clip_slice`.
        if r.start - c.start >= MIN_CLIP_DURATION {
            let mut left = clip_slice(c, TickRange::new(c.start, r.start), ClipId(*next_clip));
            *next_clip += 1;
            left.link_group = None;
            cmds.push(Command::NoteBirth { child: left.id, parent: c.id });
            cmds.push(Command::AddClip(left));
        }
        if end - r.end >= MIN_CLIP_DURATION {
            let mut right = clip_slice(c, TickRange::new(r.end, end), ClipId(*next_clip));
            *next_clip += 1;
            right.link_group = None;
            cmds.push(Command::NoteBirth { child: right.id, parent: c.id });
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
        // предел трима считает та же функция, что и команда: разойдись
        // они — жест то падал бы, то молча не доезжал (Р.39)
        .filter(|c| slice_fits_source(c, TickRange::new(at, c.start + c.duration)))
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
    *c = clip_slice(c, TickRange::new(c.start, new_end), c.id);
}

fn raw_trim_start(c: &mut ClipEvent, new_start: Tick) {
    *c = clip_slice(c, TickRange::new(new_start, c.start + c.duration), c.id);
}

/// Конвертация legacy-проекта с перекрытиями в Transition-модель (этап 9).
/// Идемпотентна: на проекте без перекрытий отчёт пуст. После вызова
/// проект обязан проходить validate().
pub fn upgrade_legacy(p: &mut Project) -> UpgradeReport {
    let mut report = UpgradeReport::default();

    // кейфреймы slowed&reverb старого образца — в общий механизм (Р.33):
    // легаси-проект их точно несёт в собственном домене
    for c in &mut p.clips {
        crate::project::migrate_legacy_slowed(c);
        crate::project::migrate_legacy_cammotion(c);
    }

    // морферы старого образца — прежняя семантика (патч на цели целиком,
    // вне времени): слоем они не становятся сами (Р.35)
    for m in &mut p.morphers {
        crate::project::migrate_legacy_morpher(m);
    }

    // Переслаивание (Р.28): старый проект открывается КАК БЫЛ — перекрытия
    // остаются перекрытиями, дорожки не плодятся, ничего не режется.
    // Решение пользователя после того, как его настоящий проект (1952 клипа,
    // 76 перекрытий) разворачивало в ~40 видео-дорожек: «как было, с
    // перекрытиями». Конверсия в переходы/развод по дорожкам осталась для
    // режима БЕЗ переслаивания.
    if p.overlap_layers {
        p.normalize();
        return report;
    }

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
            audio: Default::default(),
            auto: vec![],
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

// ---- буфер обмена FX (Р.33) --------------------------------------------
// Требование пользователя: «Не хватает копипастинга — как отдельных
// эффектов так и fx-chain. так и автоматизации». Блоки живут в модели, а
// не в UI: их же формой пользуется системный буфер (JSON), поэтому копия
// переживает перезапуск и переезжает между окнами программы.

/// Один слот цепочки со всем, что к нему относится.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FxSlotBlock {
    pub fx: FxInstance,
    #[serde(default)]
    pub bypass: bool,
    #[serde(default)]
    pub curves: Vec<ParamCurve>,
}

/// Цепочка целиком либо её огибающие. `src_in` — точка входа события, из
/// которого копировали: ключи лежат в тиках МАТЕРИАЛА, и при вставке в
/// другое событие их надо перевесить относительно его собственной точки
/// входа («точка входа к точке входа», как в Premiere).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FxChainBlock {
    pub slots: Vec<FxSlotBlock>,
    pub src_in: Tick,
    #[serde(default)]
    pub fx_off: bool,
}

/// Огибающие одного слота (кривые без самого эффекта).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnvelopeBlock {
    /// Тег варианта эффекта-источника: вставка в чужой вид эффекта
    /// бессмысленна (кривые адресуются именами его полей).
    pub fx_tag: String,
    pub curves: Vec<ParamCurve>,
    pub src_in: Tick,
}

/// Тег варианта (`kind` в файле) — по нему проверяется совместимость
/// огибающих при вставке.
pub fn fx_tag(f: &FxInstance) -> String {
    serde_json::to_value(f)
        .ok()
        .and_then(|v| v.get("kind").and_then(|k| k.as_str().map(str::to_string)))
        .unwrap_or_default()
}

/// Сдвиг ключей при переносе в другое событие: материал адресуется от
/// точки входа, поэтому кривая встаёт «на то же место куска».
pub fn rebase_curves(curves: &[ParamCurve], from: Tick, to: Tick) -> Vec<ParamCurve> {
    let d = to.0 - from.0;
    curves
        .iter()
        .map(|c| ParamCurve {
            param: c.param.clone(),
            ease: c.ease,
            keys: c.keys.iter().map(|k| AutoKey { t: Tick((k.t.0 + d).max(0)), v: k.v }).collect(),
        })
        .collect()
}

/// Дописать блок цепочки в конец приёмника (Р.48): ЕДИНСТВЕННАЯ правда
/// вставки — её зовут и копипаст полосы, и вставка пресета.
///
/// Ключи переезжают «вход к входу»: `d = dst_src_in − block.src_in`. У
/// события точка входа — `source_in` (тики материала), у морфера — ноль
/// (время блока), поэтому одна формула обслуживает оба носителя и оба
/// направления переноса. Ключи, ушедшие левее входа, клампятся в ноль —
/// как и в `rebase_curves`.
///
/// `fx_off` блока НЕ применяется намеренно: дописывание не должно
/// выключать цепочку приёмника (мастер-выключатель — свойство события, а
/// не переносимого куска).
pub fn append_chain_block(
    fx: &mut Vec<FxInstance>,
    bypass: &mut Vec<bool>,
    auto: &mut Vec<Vec<ParamCurve>>,
    block: &FxChainBlock,
    dst_src_in: Tick,
) {
    bypass.resize(fx.len(), false);
    auto.resize(fx.len(), Vec::new());
    for slot in &block.slots {
        fx.push(slot.fx.clone());
        bypass.push(slot.bypass);
        auto.push(rebase_curves(&slot.curves, block.src_in, dst_src_in));
    }
}

/// Автодакинг (Р.48, п.341): кривая громкости по интервалам, в которых
/// звучит дорожка-источник. Интервалы считает движок (`kt_engine::
/// ducking::duck_spans`) честным рендером, здесь — только правка модели.
///
/// Почему кривой, а не живым сайдчейном: дакинг остаётся ОБЫЧНОЙ правкой
/// проекта — виден на глаз, правится руками, откатывается одним undo и
/// не делает звук события функцией чужой дорожки. Цена: поменялась
/// музыка — команду надо позвать снова (сам он не «отрастает»).
///
/// Ключей не больше четырёх на интервал: вниз за `attack`, назад вверх
/// за `release`, перекрытия соседних спанов сливаются. Существующая
/// кривая `db` слота Gain ЗАМЕНЯЕТСЯ (повторный вызов перегенерирует,
/// а не наслаивает), остальные кривые слота живут.
pub fn duck_curve_commands(
    p: &Project,
    targets: &[ClipId],
    spans: &[std::ops::Range<i64>],
    depth_db: f64,
    attack_ms: f64,
    release_ms: f64,
) -> Vec<Command> {
    let att = Tick::from_audio_frames((attack_ms.max(1.0) / 1000.0 * 48_000.0) as i64);
    let rel = Tick::from_audio_frames((release_ms.max(1.0) / 1000.0 * 48_000.0) as i64);
    let mut out = Vec::new();
    for id in targets {
        let Some(c) = p.clips.iter().find(|c| c.id == *id) else { continue };
        if c.kind != ClipKind::Audio {
            continue;
        }
        let clip_end = c.start + c.duration;
        // слот громкости: свой, иначе дописываем в конец цепочки
        let (slot, fx) = match c.fx.iter().position(|f| matches!(f, FxInstance::Gain { .. })) {
            Some(i) => (i, c.fx.clone()),
            None => {
                let mut fx = c.fx.clone();
                fx.push(FxInstance::gain(0.0));
                (fx.len() - 1, fx)
            }
        };
        let base = match &fx[slot] {
            FxInstance::Gain { db, .. } => *db as f64,
            _ => 0.0,
        };
        // ключи в тиках МАТЕРИАЛА — тем же правилом, что у редактора
        // кривых (реверс и обрезка учтены одной функцией)
        let mat = |t: Tick| {
            crate::clip_material_at(c.source_in, c.start, c.duration, c.reversed, 1, t)
        };
        let mut keys: Vec<AutoKey> = Vec::new();
        let push = |t: Tick, v: f64, keys: &mut Vec<AutoKey>| {
            let t = mat(t.clamp(c.start, clip_end));
            match keys.last_mut() {
                // на реверсе ключи идут в обратном порядке — сортировка ниже
                Some(k) if k.t == t => k.v = v,
                _ => keys.push(AutoKey { t, v }),
            }
        };
        let ducked = base - depth_db;
        for sp in spans {
            let (s, e) = (Tick::from_audio_frames(sp.start), Tick::from_audio_frames(sp.end));
            if e <= c.start || s >= clip_end {
                continue; // спан не задевает событие
            }
            push(s - att, base, &mut keys);
            push(s, ducked, &mut keys);
            push(e, ducked, &mut keys);
            push(e + rel, base, &mut keys);
        }
        if keys.is_empty() {
            continue;
        }
        keys.sort_by_key(|k| k.t.0);
        keys.dedup_by(|a, b| a.t == b.t);
        let mut auto = c.fx_auto.clone();
        auto.resize(fx.len(), Vec::new());
        auto[slot].retain(|cur| cur.param != "db");
        auto[slot].push(ParamCurve { param: "db".into(), ease: true, keys });
        if fx.len() != c.fx.len() {
            out.push(Command::SetFx { id: *id, fx });
        }
        out.push(Command::SetFxAuto { id: *id, auto });
    }
    out
}
