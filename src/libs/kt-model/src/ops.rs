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
    /// Камеры, чьё окно пересекло окно блока (Р.55, С3). Правило то же,
    /// что у морфер-слоя в режиме «всё, что ниже»: камера — тоже строка
    /// над монтажом, и без неё свёрнутый кусок теряет движение, которое
    /// человек ему поставил.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<CameraEvent>,
}

impl ClipBlock {
    /// Наименьший и наибольший номер дорожки этого вида в блоке.
    pub fn lane_span(&self, k: LaneKind) -> Option<(u32, u32)> {
        let lanes = self
            .clips
            .iter()
            .filter(|c| lane_key(c).0 == k)
            .map(|c| c.lane)
            .chain(self.morphers.iter().filter(|_| k == LaneKind::Morph).map(|m| m.lane))
            .chain(self.cameras.iter().filter(|_| k == LaneKind::Camera).map(|c| c.lane));
        lanes.fold(None, |acc: Option<(u32, u32)>, l| {
            Some(acc.map_or((l, l), |(a, b)| (a.min(l), b.max(l))))
        })
    }

    /// Сдвинуть ВСЕ виды на одну дельту, клампя КАЖДЫЙ вид так, чтобы его
    /// нижний номер не ушёл за ноль (Р.56, п.450).
    ///
    /// Кламп именно по виду, а не по событию: клампуй мы каждое событие,
    /// два события разных дорожек слиплись бы на нулевой — форма блока
    /// важнее попадания в точку. Возвращает виды, которым сдвиг урезали:
    /// об этом говорят человеку, а не доправляют молча (правило 16).
    pub fn shift_lanes(&mut self, delta: i64) -> Vec<LaneKind> {
        let mut clamped = Vec::new();
        let mut d = [0i64; 4];
        for (i, k) in
            [LaneKind::Video, LaneKind::Audio, LaneKind::Morph, LaneKind::Camera].iter().enumerate()
        {
            let Some((lo, _)) = self.lane_span(*k) else { continue };
            d[i] = delta.max(-(lo as i64));
            if d[i] != delta {
                clamped.push(*k);
            }
        }
        let of = |k: LaneKind| match k {
            LaneKind::Video => d[0],
            LaneKind::Audio => d[1],
            LaneKind::Morph => d[2],
            LaneKind::Camera => d[3],
        };
        let mv = |lane: &mut u32, k: LaneKind| {
            *lane = (*lane as i64 + of(k)).max(0) as u32;
        };
        for c in &mut self.clips {
            let k = lane_key(c).0;
            mv(&mut c.lane, k);
        }
        for m in &mut self.morphers {
            mv(&mut m.lane, LaneKind::Morph);
        }
        for c in &mut self.cameras {
            mv(&mut c.lane, LaneKind::Camera);
        }
        clamped
    }

    /// Привести блок к СТРОКЕ, по которой кликнули (Р.56, п.450).
    ///
    /// Вид строки есть в блоке — совмещаем его верхнее событие с этой
    /// строкой, остальные виды едут на ту же дельту (пара видео+аудио
    /// сохраняет взаимное положение). Вида нет (кликнули по камерной
    /// строке блоком без камер) — не двигаем вовсе: угадывать нечего.
    pub fn shifted_to_row(mut self, row: Option<(LaneKind, u32)>) -> (ClipBlock, Vec<LaneKind>) {
        let Some((k, lane)) = row else { return (self, Vec::new()) };
        let Some((lo, _)) = self.lane_span(k) else { return (self, Vec::new()) };
        let clamped = self.shift_lanes(lane as i64 - lo as i64);
        (self, clamped)
    }

    /// Перевести блок из РАБОЧЕГО ВИДА в координаты ленты полосы: номера
    /// дорожек полосы лежат со смещением `base[вид]` (Р.55, п.441), и
    /// смещения у видов РАЗНЫЕ. Чужие события выбрасываются со счётом.
    ///
    /// Без этого клипборд хранил номера с тремя разными базами, а вставка
    /// повторяла их как есть: видео оказывалось у потолка, аудио внизу, а
    /// морфер — в самом низу, где под него заводилась новая строка
    /// (претензия владельца п.4 Р.56).
    pub fn localize(&mut self, base: LaneBases, len: LaneBases) -> usize {
        let owns =
            |k: LaneKind, lane: u32| lane >= base.get(k) && lane < base.get(k) + len.get(k).max(1);
        let before = self.clips.len() + self.morphers.len() + self.cameras.len();
        self.clips.retain(|c| owns(lane_key(c).0, c.lane));
        self.morphers.retain(|m| owns(LaneKind::Morph, m.lane));
        self.cameras.retain(|c| owns(LaneKind::Camera, c.lane));
        let kept: Vec<ClipId> = self.clips.iter().map(|c| c.id).collect();
        self.transitions.retain(|t| {
            t.a.is_none_or(|id| kept.contains(&id)) && t.b.is_none_or(|id| kept.contains(&id))
        });
        for c in &mut self.clips {
            c.lane -= base.get(lane_key(c).0);
        }
        for m in &mut self.morphers {
            m.lane -= base.morph;
        }
        for c in &mut self.cameras {
            c.lane -= base.camera;
        }
        before - (self.clips.len() + self.morphers.len() + self.cameras.len())
    }

    /// Обратный перевод: в координаты полосы рабочего вида.
    pub fn offset_lanes(&mut self, base: LaneBases) {
        for c in &mut self.clips {
            c.lane += base.get(lane_key(c).0);
        }
        for m in &mut self.morphers {
            m.lane += base.morph;
        }
        for c in &mut self.cameras {
            c.lane += base.camera;
        }
    }

    /// Сколько дорожек каждого вида нужно, чтобы блок поместился.
    pub fn lanes_needed(&self) -> LaneBases {
        let mut n = LaneBases::default();
        for k in [LaneKind::Video, LaneKind::Audio, LaneKind::Morph, LaneKind::Camera] {
            if let Some((_, hi)) = self.lane_span(k) {
                *n.get_mut(k) = hi + 1;
            }
        }
        n
    }
}

/// Свежие id для вставки — растущий список аргументов `paste_block`
/// перестал читаться уже на четвёртом.
pub struct PasteIds<'a> {
    pub clip: &'a mut u64,
    pub group: &'a mut u64,
    pub transition: &'a mut u64,
    pub morph: &'a mut u64,
    pub camera: &'a mut u64,
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
                    return m.window().overlaps(&win).then(|| rebase_morpher(m, base));
                }
            };
            (!kept.is_empty()).then(|| {
                let mut copy = rebase_morpher(m, base);
                copy.targets = MorphTargets::Clips { ids: kept };
                copy
            })
        })
        .collect();
    // камеры — по ПЕРЕСЕЧЕНИЮ окна с окном блока, с ребейзом: у камеры
    // нет «целей», она действует на то, что под ней по времени
    let cameras = p
        .cameras
        .iter()
        .filter(|c| kt_time::TickRange::new(c.start, c.start + c.duration).overlaps(&win))
        .map(|c| CameraEvent { start: c.start - base, ..c.clone() })
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
    ClipBlock { morphers, areas, markers, cameras, ..Default::default() }
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
            if let (Some(a), Some(b)) = (a, b) {
                let (a_end, b_end) = (a.start + a.duration, b.start + b.duration);
                if a_end != b.start {
                    // наложенный переход (Р.51) выживает, если оба клипа в
                    // блоке лежат той же «лесенкой»; окно проверит
                    // check_transition_full при вставке
                    let stag = a.start < b.start && b.start < a_end && a_end < b_end;
                    return (stag && a_end - b.start >= MIN_TRANSITION_DURATION).then(|| t.clone());
                }
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
    let PasteIds {
        clip: next_clip,
        group: next_group,
        transition: next_transition,
        morph,
        camera: next_camera,
    } = ids;
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
    // камеры — то же правило, что у морферов: свежий адрес, ребейз на
    // точку вставки. Целей у камеры нет, поэтому и пропускать нечего.
    for c in &block.cameras {
        let mut copy = c.clone();
        copy.id = CameraId(*next_camera);
        *next_camera += 1;
        copy.start = at + c.start;
        // имя камеры НЕ уникализируется: оно подпись, а не адрес —
        // ссылок на него нет (в отличие от области и метки, на которые
        // смотрит морфер по ссылке)
        end = end.max(copy.start + copy.duration);
        cmds.push(Command::AddCamera(copy));
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
        // наложенный переход (Р.51): у него ничего не срезалось при
        // наезде — «возврат ручек» дал бы ложные тримы
        if t.overlap_pair(before).is_some() {
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
            obstacles(p, m)
                .filter(|(s, _)| *s >= end)
                .map(|(s, _)| s - end)
                .min()
                .unwrap_or(i64::MAX)
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
        let joint = Tick((ov0.0 + (ov1.0 - ov0.0) / 2).clamp(
            (a0.start + MIN_CLIP_DURATION).0,
            (b0.start + b0.duration - MIN_CLIP_DURATION).0,
        ));
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
            invert: false,
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
        let keeper_lane = p.clips.iter().find(|c| c.id == mover).map(|c| c.lane).unwrap_or(0);
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
    let mut spans: Vec<(&ClipEvent, Tick)> =
        p.clips.iter().filter(|c| c.kind == kind).map(|c| (c, c.start + c.duration)).collect();
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
            source: Default::default(),
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
        let mat =
            |t: Tick| crate::clip_material_at(c.source_in, c.start, c.duration, c.reversed, 1, t);
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
        auto[slot].push(ParamCurve {
            param: "db".into(),
            ease: true,
            keys,
            source: Default::default(),
        });
        if fx.len() != c.fx.len() {
            out.push(Command::SetFx { id: *id, fx });
        }
        out.push(Command::SetFxAuto { id: *id, auto });
    }
    out
}

/// Совместить событие `b` с событием `a` по найденному сдвигу звука
/// (Р.49, п.375).
///
/// `offset_ticks` — «материал B отстаёт от материала A на столько тиков».
///
/// **Не тримит и не линкует.** Синхронизация отвечает на вопрос «где», а не
/// «как долго» и не «вместе ли»: магнит подрезает соседа — это его свойство,
/// а link-группа это отдельное решение монтажёра. Сдвинуть и остановиться —
/// ровно то, о чём просили.
///
/// **Перевод «материал → таймлайн» — только через `clip_timeline_at`**
/// (правило единственного дома, t273): своя арифметика по `source_in` здесь
/// уехала бы на реверсе и на подрезанном начале.
pub fn sync_by_audio_commands(p: &Project, a: ClipId, b: ClipId, offset_ticks: Tick) -> SyncPlan {
    let Some(ca) = p.clips.iter().find(|c| c.id == a) else { return SyncPlan::AlreadyAligned };
    let Some(cb) = p.clips.iter().find(|c| c.id == b) else { return SyncPlan::AlreadyAligned };
    let step = kt_time::FpsGrid::new(p.settings.fps).frame_step();
    // точка материала A, которая сейчас стоит в начале события A
    let mat_a = clip_material_at(ca.source_in, ca.start, ca.duration, ca.reversed, step, ca.start);
    // тот же момент реального мира в материале B
    let mat_b = Tick(mat_a.0 + offset_ticks.0);
    // где он сейчас на таймлайне у события B
    let now = clip_timeline_at(cb.source_in, cb.start, cb.duration, cb.reversed, step, mat_b);
    let delta = ca.start.0 - now.0;
    if delta == 0 {
        return SyncPlan::AlreadyAligned;
    }
    let want = cb.start.0 + delta;
    if want < 0 {
        // КЛАМПИТЬ НЕЛЬЗЯ: событие, упёршееся в ноль, окажется НЕ совмещено,
        // а выглядеть будет как успешная синхронизация. Отказ честнее.
        return SyncPlan::OutOfRange;
    }
    SyncPlan::Move(vec![Command::MoveClip { id: b, new_start: Tick(want), new_lane: cb.lane }])
}

/// Чем кончилось планирование синхронизации.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncPlan {
    /// Двигаем.
    Move(Vec<Command>),
    /// Уже совмещено — работы нет.
    AlreadyAligned,
    /// Совмещение уводит событие ЗА НАЧАЛО проекта. Кламп к нулю тут
    /// недопустим: он выглядит как успех, а материалы остаются вразнобой.
    OutOfRange,
}

// ---- вложенные секвенции (Р.50, п.380) --------------------------------------

/// Свежие адреса для операций над лентами. Тот же приём, что `PasteIds`:
/// растущий список аргументов перестаёт читаться на четвёртом.
pub struct NestIds<'a> {
    pub media: &'a mut u64,
    pub clip: &'a mut u64,
    pub group: &'a mut u64,
    pub transition: &'a mut u64,
    pub morph: &'a mut u64,
    pub camera: &'a mut u64,
}

/// Счётчики свежих адресов, посчитанные ОТ ПРОЕКТА.
///
/// Заведены отдельной структурой, потому что операции над лентами раздают
/// адреса пяти видов сразу, и call-site, который выпишет их руками, рано или
/// поздно выпишет их по дереву не целиком: `ClipId` уникален по ВСЕМУ
/// документу, и «максимум по `p.clips`» мимо лент — это молчаливое
/// столкновение адресов.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NestCounters {
    pub media: u64,
    pub clip: u64,
    pub group: u64,
    pub transition: u64,
    pub morph: u64,
    pub camera: u64,
}

impl NestCounters {
    pub fn of(p: &Project) -> NestCounters {
        let reels = || std::iter::once(&p.clips).chain(p.sequences.iter().map(|s| &s.clips));
        let max = |it: &mut dyn Iterator<Item = u64>| it.max().unwrap_or(0) + 1;
        NestCounters {
            media: max(&mut p
                .media
                .iter()
                .map(|m| m.id.0)
                .chain(p.sequences.iter().map(|s| s.id.0))),
            clip: max(&mut reels().flat_map(|cs| cs.iter().map(|c| c.id.0))),
            group: max(
                &mut reels().flat_map(|cs| cs.iter().filter_map(|c| c.link_group.map(|g| g.0)))
            ),
            transition: max(&mut std::iter::once(&p.transitions)
                .chain(p.sequences.iter().map(|s| &s.transitions))
                .flat_map(|ts| ts.iter().map(|t| t.id.0))),
            morph: max(&mut std::iter::once(&p.morphers)
                .chain(p.sequences.iter().map(|s| &s.morphers))
                .flat_map(|ms| ms.iter().map(|m| m.id.0))),
            camera: max(&mut std::iter::once(&p.cameras)
                .chain(p.sequences.iter().map(|s| &s.cameras))
                .flat_map(|cs| cs.iter().map(|c| c.id.0))),
        }
    }

    pub fn ids(&mut self) -> NestIds<'_> {
        NestIds {
            media: &mut self.media,
            clip: &mut self.clip,
            group: &mut self.group,
            transition: &mut self.transition,
            morph: &mut self.morph,
            camera: &mut self.camera,
        }
    }
}

/// Что сворачивание изменило, кроме монтажа. Возвращается, а не
/// «доправляется»: человеку надо сказать (правило 16), а не догадываться.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CollapseReport {
    /// Адрес заведённой ленты.
    pub seq: Option<MediaId>,
    /// Переходы, у которых один конец остался СНАРУЖИ: они перевешены на
    /// экземпляр (кроссфейд со свёрнутым куском остаётся кроссфейдом).
    pub rehung: usize,
    /// Морфер-слои, накрывавшие свёрнутое СНАРУЖИ. Они остаются снаружи и
    /// теперь бьют по СОБРАННОЙ картинке, а не по каждому куску: картинка
    /// законно меняется, и об этом обязано быть сказано.
    pub outer_layers: usize,
    /// Слои, уехавшие ВНУТРЬ ленты (и убранные из корня).
    pub moved_layers: usize,
    /// Камеры, уехавшие внутрь.
    pub moved_cameras: usize,
    /// Слои по ОБЛАСТИ/МЕТКЕ, оставленные снаружи. Ссылочные режимы
    /// внутри ленты запрещены (`MorphRefInsideSequence`): область и метка
    /// — навигация ПО ПРОЕКТУ, они живут в корне. Утащи такой слой
    /// внутрь — и `validate` отвергнет ВЕСЬ батч, то есть свернуть
    /// выделение под ним стало бы нельзя вообще.
    pub kept_ref_layers: usize,
}

/// Свернуть выделение в ленту и поставить на его место экземпляр.
///
/// Тело набирает `copy_clips` — второго правила «что едет с выделением»
/// заводить нельзя: разойдись они, копипаст и сворачивание брали бы разное,
/// и объяснить это было бы нечем.
///
/// **`ClipId` сохраняются.** Те же события переехали в другой контейнер, а не
/// родились заново: поэтому переходы, link-группы и замороженные цели
/// морферов переезжают verbatim, а FX-состояние движка (ключ — `ClipId`)
/// живёт дальше и хвосты ревербов не обрываются (инвариант 3).
pub fn collapse_into_sequence(
    p: &Project,
    ids: &[ClipId],
    name: &str,
    n: NestIds<'_>,
) -> (Vec<Command>, CollapseReport) {
    let block = copy_clips(p, ids);
    if block.clips.is_empty() {
        return (Vec::new(), CollapseReport::default());
    }
    let members: Vec<ClipId> = block.clips.iter().map(|c| c.id).collect();
    let base = p
        .clips
        .iter()
        .filter(|c| members.contains(&c.id))
        .map(|c| c.start)
        .min()
        .unwrap_or(Tick::ZERO);
    let len = block_len(&block.clips);
    let seq_id = MediaId(*n.media);
    *n.media += 1;

    // Содержимое ленты: события, переходы и морферы блока. Области и метки
    // ОСТАЮТСЯ В КОРНЕ: это навигация по проекту, человек поставил их там, и
    // утащить их внутрь значило бы «отрубить само» (правило 20).
    let mut content = Sequence::new(seq_id, name);
    content.clips = block.clips.clone();
    content.transitions = block.transitions.clone();
    // Слой по ОБЛАСТИ или МЕТКЕ внутрь не едет: `validate_reel` такие
    // внутри ленты запрещает (`MorphRefInsideSequence`), и утащенный
    // слой уронил бы ВЕСЬ батч — свернуть выделение под ссылочным слоем
    // стало бы нельзя вообще, с английской ошибкой в статусбаре.
    let refs_stay = |m: &MorpherEvent| {
        matches!(m.targets, MorphTargets::Areas { .. } | MorphTargets::Marks { .. })
    };
    content.morphers = block.morphers.iter().filter(|m| !refs_stay(m)).cloned().collect();
    content.cameras = block.cameras.clone();
    content.video_lanes = lanes_needed(&content.clips, ClipKind::Video).max(1);
    content.audio_lanes = lanes_needed(&content.clips, ClipKind::Audio).max(1);
    content.morph_lanes = content.morphers.iter().map(|m| m.lane + 1).max().unwrap_or(0);
    content.camera_lanes = content.cameras.iter().map(|c| c.lane + 1).max().unwrap_or(0);
    // ПОРЯДОК СТРОК едет вместе с содержимым (Р.55, С3). Без него у ленты
    // порядок дефолтный — видео, аудио, морферы, камеры, — а в нём
    // морф-строки идут ПОСЛЕДНИМИ. Слой в режиме «всё, что ниже» требует
    // цели НИЖЕ себя, и под ним не оказывается ничего: слой уехал внутрь
    // и там умер. Ровно это владелец и увидел («морфер не попадает туда»).
    content.lane_order = p
        .rows()
        .into_iter()
        .filter(|&(k, l)| match k {
            LaneKind::Video => l < content.video_lanes,
            LaneKind::Audio => l < content.audio_lanes,
            LaneKind::Morph => l < content.morph_lanes,
            LaneKind::Camera => l < content.camera_lanes,
        })
        .collect();
    content.overlap_layers = p.overlap_layers;

    let mut report = CollapseReport {
        seq: Some(seq_id),
        moved_layers: content.morphers.len(),
        moved_cameras: content.cameras.len(),
        kept_ref_layers: block.morphers.iter().filter(|m| refs_stay(m)).count(),
        ..Default::default()
    };
    report.outer_layers = p
        .morphers
        .iter()
        .filter(|m| !block.morphers.iter().any(|b| b.id == m.id))
        .filter(|m| m.window().overlaps(&TickRange::new(base, base + len)))
        .count();

    let mut cmds = vec![
        Command::AddSequence { id: seq_id, name: name.to_string() },
        Command::SetSequenceContent { id: seq_id, content: Box::new(content) },
    ];
    for id in &members {
        cmds.push(Command::RemoveClip { id: *id });
    }
    // Уехавшее ВНУТРЬ убирается из КОРНЯ. Без этого слой и камера жили бы
    // в двух местах сразу: внутри — на каждом куске, снаружи — на
    // собранной картинке. Одно и то же наложилось бы дважды, и объяснить
    // это было бы нечем.
    let moved_morphs: Vec<MorphId> = p
        .morphers
        .iter()
        .filter(|m| block.morphers.iter().any(|b| b.id == m.id) && !refs_stay(m))
        .map(|m| m.id)
        .collect();
    for id in moved_morphs {
        cmds.push(Command::RemoveMorpher { id });
    }
    let moved_cams: Vec<CameraId> = p
        .cameras
        .iter()
        .filter(|c| block.cameras.iter().any(|b| b.id == c.id))
        .map(|c| c.id)
        .collect();
    for id in moved_cams {
        cmds.push(Command::RemoveCamera { id });
    }

    // Экземпляр — ОБЫЧНЫЙ клип, и он ПАРА (видео + аудио) в одной
    // link-группе: тогда «смонтируй, потом замедли целиком» получается даром
    // через существующий `sync_warp_to_link_group`.
    let kinds: Vec<ClipKind> = [ClipKind::Video, ClipKind::Audio]
        .into_iter()
        .filter(|k| block.clips.iter().any(|c| c.kind == *k))
        .collect();
    let group = (kinds.len() > 1).then(|| {
        let g = GroupId(*n.group);
        *n.group += 1;
        g
    });
    let mut instances: Vec<(ClipKind, ClipId)> = Vec::new();
    for k in kinds {
        // самое верхнее из свёрнутого: экземпляр встаёт туда, куда человек
        // смотрел, а не на нулевую дорожку
        let lane = p
            .clips
            .iter()
            .filter(|c| members.contains(&c.id) && c.kind == k)
            .map(|c| c.lane)
            .min()
            .unwrap_or(0);
        let id = ClipId(*n.clip);
        *n.clip += 1;
        instances.push((k, id));
        let first = p
            .clips
            .iter()
            .filter(|c| members.contains(&c.id) && c.kind == k)
            .min_by_key(|c| (c.start, c.id))
            .map(|c| c.id);
        if let Some(parent) = first {
            // конверт экземпляра — от первого свёрнутого события его вида
            cmds.push(Command::NoteBirth { child: id, parent });
        }
        cmds.push(Command::AddClip(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            id,
            link_group: group,
            kind: k,
            lane,
            start: base,
            duration: len,
            source_in: Tick::ZERO,
            media: seq_id,
            gain_db: 0.0,
            reversed: false,
            fx: Vec::new(),
            fx_bypass: Vec::new(),
            fx_auto: Vec::new(),
            opacity: 1.0,
            fx_off: false,
        }));
    }

    // Переход, у которого ОДИН конец остался снаружи, перевешивается на
    // экземпляр: кроссфейд со свёрнутым куском обязан остаться кроссфейдом,
    // а не умереть от `reconcile_transitions` вместе с исчезнувшим концом.
    for t in &p.transitions {
        let inside = |id: Option<ClipId>| id.is_some_and(|i| members.contains(&i));
        let (ia, ib) = (inside(t.a), inside(t.b));
        if ia == ib {
            continue; // оба внутри (уехал в ленту) или оба снаружи (цел)
        }
        let outer = if ia { t.b } else { t.a };
        let kind =
            outer.and_then(|i| p.clips.iter().find(|c| c.id == i)).map(|c| c.kind).or_else(|| {
                let side = if ia { t.a } else { t.b };
                side.and_then(|i| p.clips.iter().find(|c| c.id == i)).map(|c| c.kind)
            });
        let Some(inst) = kind.and_then(|k| instances.iter().find(|(x, _)| *x == k)) else {
            continue;
        };
        let mut copy = t.clone();
        if ia {
            copy.a = Some(inst.1);
        } else {
            copy.b = Some(inst.1);
        }
        cmds.push(Command::SetTransition(copy));
        report.rehung += 1;
    }
    (cmds, report)
}

fn lanes_needed(clips: &[ClipEvent], k: ClipKind) -> u32 {
    clips.iter().filter(|c| c.kind == k).map(|c| c.lane + 1).max().unwrap_or(1)
}

/// Что разворачивание сделало сверх переноса событий.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExpandReport {
    /// Экземпляр нёс цепочку эффектов, и она СНЯТА: снаружи она
    /// применялась к собранной картинке, внутри такого места нет. Молча
    /// раскидать её по кускам — значит посчитать другое (правило 16).
    pub chain_dropped: bool,
    /// Экземпляр нёс уровень (непрозрачность или громкость) — снят.
    pub level_dropped: bool,
    /// Экземпляр был ЗЕРКАЛЬНЫМ, и реверс снят (Р.55, С4).
    ///
    /// Назван отдельно, потому что это РЕВЕРС-АНАЛОГ ЗАКОНА Г: «реверс
    /// собранного куска ≠ реверс каждого куска». Перевернуть порядок
    /// кусков и выставить каждому `reversed` эквивалентно только там, где
    /// внутри нет ни перекрытий, ни переходов, ни потоковых цепочек, — то
    /// есть в частном случае. Тихо раскидать реверс по кускам значит
    /// посчитать другое.
    pub reverse_dropped: bool,
    /// Экземпляр играл со СКОРОСТЬЮ, и она снята — по той же причине:
    /// ресемплируется СОБРАННЫЙ микс со сшитыми хвостами, а не каждый
    /// кусок со своим оборванным.
    pub warp_dropped: bool,
    /// Сколько дорожек пришлось вставить под содержимое.
    pub lanes_inserted: usize,
}

/// Развернуть экземпляр обратно на дорожки.
///
/// Содержимое подрезается окном экземпляра ОДНИМ правилом куска
/// (`clip_slice`) и переносится на свежие id через `paste_block`: тело ленты
/// остаётся на месте — та же лента может стоять ещё в пяти местах.
pub fn expand_instance(p: &Project, id: ClipId, n: NestIds<'_>) -> (Vec<Command>, ExpandReport) {
    let Some(inst) = p.clips.iter().find(|c| c.id == id) else {
        return (Vec::new(), ExpandReport::default());
    };
    let Some(seq) = p.sequence(inst.media) else { return (Vec::new(), ExpandReport::default()) };
    let mut report = ExpandReport {
        chain_dropped: !inst.fx.is_empty(),
        level_dropped: inst.opacity != 1.0 || inst.gain_db != 0.0,
        reverse_dropped: inst.reversed,
        warp_dropped: crate::chain_warps_time(&inst.active_fx(), &inst.active_fx_auto()),
        lanes_inserted: 0,
    };
    // окно экземпляра в координатах содержимого
    let win = TickRange::new(inst.source_in, inst.source_in + inst.duration);
    let mut block = ClipBlock::default();
    for c in &seq.clips {
        let body = TickRange::new(c.start, c.start + c.duration);
        let cut = body.intersect(&win);
        if cut.is_empty() || cut.len() < MIN_CLIP_DURATION {
            continue;
        }
        // id тут временный: `paste_block` ниже выдаст свежие и ретаргетит
        // переходы на них. Важно, что материальная точка входа куска
        // считается ЕДИНСТВЕННЫМ правилом проекта.
        let mut s = clip_slice(c, cut, c.id);
        s.start = cut.start - win.start;
        block.clips.push(s);
    }
    if block.clips.is_empty() {
        return (Vec::new(), report);
    }
    block.transitions = seq
        .transitions
        .iter()
        .filter(|t| {
            let ok = |x: Option<ClipId>| x.is_none_or(|i| block.clips.iter().any(|c| c.id == i));
            ok(t.a) && ok(t.b)
        })
        .cloned()
        .collect();

    // Дорожки ВСТАВЛЯЮТСЯ под содержимое, а не занимаются чужие: иначе
    // разворачивание молча меняет z-order соседей — «отросло само».
    let mut cmds = Vec::new();
    let partners: Vec<&ClipEvent> = p
        .clips
        .iter()
        .filter(|c| c.id == id || (inst.link_group.is_some() && c.link_group == inst.link_group))
        .collect();
    let mut lane_of: std::collections::HashMap<ClipKind, u32> = std::collections::HashMap::new();
    for c in &partners {
        lane_of.insert(c.kind, c.lane);
    }
    for k in [ClipKind::Video, ClipKind::Audio] {
        let need = block.clips.iter().filter(|c| c.kind == k).map(|c| c.lane + 1).max();
        let (Some(need), Some(&at)) = (need, lane_of.get(&k)) else { continue };
        for _ in 1..need {
            cmds.push(Command::InsertLane { kind: k, at: at + 1 });
            report.lanes_inserted += 1;
        }
        for c in block.clips.iter_mut().filter(|c| c.kind == k) {
            c.lane += at;
        }
    }
    for c in &partners {
        cmds.push(Command::RemoveClip { id: c.id });
    }
    let (paste, _, _) = paste_block(
        &block,
        inst.start,
        PasteIds {
            clip: n.clip,
            group: n.group,
            transition: n.transition,
            morph: n.morph,
            camera: n.camera,
        },
        &PasteNames::default(),
    );
    cmds.extend(paste);
    (cmds, report)
}

/// Отвязать экземпляр от остальных мест: содержимое копируется в СВОЮ ленту.
///
/// Нужно ровно тогда, когда «правка внутри меняет все места» перестаёт быть
/// тем, чего человек хочет. Названо не «Unlink» — рядом уже есть разрыв
/// A/V-связи, и одно слово на два разных действия врало бы (правило 18).
pub fn make_independent(p: &Project, id: ClipId, name: &str, n: NestIds<'_>) -> Vec<Command> {
    let Some(inst) = p.clips.iter().find(|c| c.id == id) else { return Vec::new() };
    let Some(seq) = p.sequence(inst.media) else { return Vec::new() };
    let new_id = MediaId(*n.media);
    *n.media += 1;
    let mut copy = seq.clone();
    copy.id = new_id;
    copy.name = name.to_string();
    // id содержимого обязаны быть СВЕЖИМИ: `ClipId` уникален по всему
    // документу, иначе карты `Retained` в kt-otio обменяют материал между
    // лентами молча
    let mut map: std::collections::HashMap<ClipId, ClipId> = std::collections::HashMap::new();
    for c in &mut copy.clips {
        let fresh = ClipId(*n.clip);
        *n.clip += 1;
        map.insert(c.id, fresh);
        c.id = fresh;
    }
    for t in &mut copy.transitions {
        t.id = TransitionId(*n.transition);
        *n.transition += 1;
        t.a = t.a.map(|x| map[&x]);
        t.b = t.b.map(|x| map[&x]);
    }
    for m in &mut copy.morphers {
        m.id = MorphId(*n.morph);
        *n.morph += 1;
        if let MorphTargets::Clips { ids } = &mut m.targets {
            ids.retain(|x| map.contains_key(x));
            for x in ids.iter_mut() {
                *x = map[x];
            }
        }
    }
    // link-группы внутри ленты остаются как есть: они адресуются только
    // внутри своей ленты и с чужими не сталкиваются
    vec![
        Command::AddSequence { id: new_id, name: name.to_string() },
        Command::SetSequenceContent { id: new_id, content: Box::new(copy) },
        Command::SetClipMedia { id, media: new_id },
    ]
}

/// Ленты, на которые больше никто не смотрит. Возвращает команды, а не
/// чистит сам: удаление — это шаг истории, его должно быть можно отменить.
pub fn remove_unused_sequences(p: &Project) -> Vec<Command> {
    p.sequences
        .iter()
        .filter(|s| p.sequence_uses(s.id) == 0)
        .map(|s| Command::RemoveSequence { id: s.id })
        .collect()
}

/// Что вставка чужого проекта изменила сверх появления ленты (Р.55, С6).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportReport {
    /// Заведённая лента-корень чужого проекта.
    pub seq: Option<MediaId>,
    /// Сколько его вложенных лент приехало вместе с ним.
    pub inner_seqs: usize,
    /// Сколько медиа заведено заново.
    pub media_added: usize,
    /// Сколько медиа УЗНАНО среди своих по пути и переиспользовано: тот же
    /// файл не обязан приезжать вторым экземпляром.
    pub media_reused: usize,
    /// Что НЕ приехало и почему — области, метки и настройки чужого
    /// проекта. Они принадлежат ДОКУМЕНТУ, а не монтажу, и класть их в
    /// свой проект значит подменить человеку его собственные (правило 20).
    pub dropped_extras: usize,
}

/// Вставить ЧУЖОЙ ПРОЕКТ как композицию (Р.55, С6).
///
/// Претензия владельца дословно: «я переношу файл проекта на таймлайн и
/// открывается новый таймлайн, а я хочу чтобы оно дало мне выбор — открыть
/// как проект или вставить как композицию».
///
/// Корень чужого проекта становится ЛЕНТОЙ, его вложенные ленты приезжают
/// вместе с ним, а на `at` встаёт экземпляр. Все адреса перенумеровываются:
/// `ClipId` уникален по ВСЕМУ документу (Р.50, t425), и столкновение
/// адресов молча обменяло бы материал между лентами через карты `Retained`.
///
/// **Медиа дедуплицируются по пути.** Тот же файл, уже стоящий в проекте,
/// не обязан приезжать вторым экземпляром: у него был бы свой воркер, своя
/// строка в кэше и свои тумбы — то есть двойная цена за одно и то же.
///
/// Области, метки и настройки чужого проекта НЕ едут: это навигация и
/// свойства ДОКУМЕНТА, а не монтажа. То же правило, что у сворачивания
/// (`collapse_into_sequence`): «утащить их внутрь значило бы отрубить само».
pub fn import_as_sequences(
    dst: &Project,
    src: &Project,
    name: &str,
    at: Tick,
    lane: u32,
    n: NestIds<'_>,
) -> (Vec<Command>, ImportReport) {
    let mut rep = ImportReport::default();
    if src.clips.is_empty() && src.sequences.is_empty() {
        return (Vec::new(), rep);
    }
    let mut cmds = Vec::new();
    // ---- медиа: свои узнаём по пути, чужие заводим -------------------
    let mut media: std::collections::HashMap<MediaId, MediaId> = std::collections::HashMap::new();
    for m in &src.media {
        match dst.media.iter().find(|x| x.path == m.path) {
            Some(mine) => {
                media.insert(m.id, mine.id);
                rep.media_reused += 1;
            }
            None => {
                let fresh = MediaId(*n.media);
                *n.media += 1;
                media.insert(m.id, fresh);
                rep.media_added += 1;
                cmds.push(Command::AddMedia { id: fresh, path: m.path.clone() });
            }
        }
    }
    // ---- ленты: адреса вперёд, чтобы вложения нашли друг друга -------
    let mut seqs: std::collections::HashMap<MediaId, MediaId> = std::collections::HashMap::new();
    for s in &src.sequences {
        let fresh = MediaId(*n.media);
        *n.media += 1;
        seqs.insert(s.id, fresh);
        rep.inner_seqs += 1;
    }
    let root_id = MediaId(*n.media);
    *n.media += 1;
    rep.seq = Some(root_id);
    // адрес медиа события: либо файл, либо вложенная лента
    let remap_media = |m: MediaId| -> Option<MediaId> {
        media.get(&m).copied().or_else(|| seqs.get(&m).copied())
    };

    let convert = |from: &dyn Reel, n: &mut NestIds<'_>| -> Option<Sequence> {
        let mut out = Sequence::new(MediaId(0), "");
        let mut map: std::collections::HashMap<ClipId, ClipId> = std::collections::HashMap::new();
        let mut groups: std::collections::HashMap<GroupId, GroupId> =
            std::collections::HashMap::new();
        for c in from.clips() {
            let mut copy = c.clone();
            copy.id = ClipId(*n.clip);
            *n.clip += 1;
            map.insert(c.id, copy.id);
            copy.media = remap_media(c.media)?;
            copy.link_group = c.link_group.map(|g| {
                *groups.entry(g).or_insert_with(|| {
                    let ng = GroupId(*n.group);
                    *n.group += 1;
                    ng
                })
            });
            out.clips.push(copy);
        }
        for t in from.transitions() {
            let mut copy = t.clone();
            copy.id = TransitionId(*n.transition);
            *n.transition += 1;
            copy.a = t.a.map(|x| map.get(&x).copied()).unwrap_or(None);
            copy.b = t.b.map(|x| map.get(&x).copied()).unwrap_or(None);
            out.transitions.push(copy);
        }
        for m in from.morphers() {
            // ССЫЛОЧНЫЕ слои внутрь ленты не пускает `validate_reel`
            // (область и метка живут в корне), а областей мы и не везём
            if matches!(m.targets, MorphTargets::Areas { .. } | MorphTargets::Marks { .. }) {
                continue;
            }
            let mut copy = m.clone();
            copy.id = MorphId(*n.morph);
            *n.morph += 1;
            if let MorphTargets::Clips { ids } = &mut copy.targets {
                ids.retain(|x| map.contains_key(x));
                for x in ids.iter_mut() {
                    *x = map[x];
                }
                if ids.is_empty() {
                    continue;
                }
            }
            out.morphers.push(copy);
        }
        for c in from.cameras() {
            let mut copy = c.clone();
            copy.id = CameraId(*n.camera);
            *n.camera += 1;
            out.cameras.push(copy);
        }
        out.video_lanes = lanes_needed(&out.clips, ClipKind::Video).max(1);
        out.audio_lanes = lanes_needed(&out.clips, ClipKind::Audio).max(1);
        out.morph_lanes = out.morphers.iter().map(|m| m.lane + 1).max().unwrap_or(0);
        out.camera_lanes = out.cameras.iter().map(|c| c.lane + 1).max().unwrap_or(0);
        Some(out)
    };

    let mut n = n;
    for s in &src.sequences {
        let Some(mut body) = convert(s, &mut n) else { continue };
        let id = seqs[&s.id];
        body.id = id;
        body.name = s.name.clone();
        body.overlap_layers = s.overlap_layers;
        body.lane_order = s.lane_order.clone();
        cmds.push(Command::AddSequence { id, name: body.name.clone() });
        cmds.push(Command::SetSequenceContent { id, content: Box::new(body) });
    }
    let Some(mut root) = convert(src, &mut n) else { return (Vec::new(), ImportReport::default()) };
    root.id = root_id;
    root.name = name.to_string();
    root.overlap_layers = src.overlap_layers;
    root.lane_order = src.lane_order.clone();
    let len = block_len(&root.clips);
    cmds.push(Command::AddSequence { id: root_id, name: name.to_string() });
    cmds.push(Command::SetSequenceContent { id: root_id, content: Box::new(root) });
    rep.dropped_extras = src.areas.len() + src.markers.len();

    // экземпляр — ОБЫЧНЫЙ клип, пара видео+аудио в одной link-группе, как
    // у сворачивания: «вставил, потом замедли целиком» получается даром
    let kinds: Vec<ClipKind> = [ClipKind::Video, ClipKind::Audio]
        .into_iter()
        .filter(|k| src.clips.iter().any(|c| c.kind == *k))
        .collect();
    let group = (kinds.len() > 1).then(|| {
        let g = GroupId(*n.group);
        *n.group += 1;
        g
    });
    for k in kinds {
        let id = ClipId(*n.clip);
        *n.clip += 1;
        cmds.push(Command::AddClip(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            id,
            link_group: group,
            kind: k,
            lane: if k == ClipKind::Video { lane } else { 0 },
            start: at,
            duration: len.max(crate::MIN_CLIP_DURATION),
            source_in: Tick::ZERO,
            media: root_id,
            gain_db: 0.0,
            reversed: false,
            fx: Vec::new(),
            fx_bypass: Vec::new(),
            fx_auto: Vec::new(),
            opacity: 1.0,
            fx_off: false,
        }));
    }
    (cmds, rep)
}
