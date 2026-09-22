//! Связь: значение ведёт ДРУГОЕ свойство (Р.61).
//!
//! **Решение владельца:** адресуется ЛЮБОЕ событие проекта, а не только
//! своё. Из инвариантов выводится ровно одно ограничение, и оно не сужает
//! решение, а объясняет, где у «проекта» край: **связь не пересекает границу
//! ленты**. Наружу — потому что лента, читающая событие снаружи, перестаёт
//! быть самодостаточной, два её экземпляра дали бы разные картинки, и закон
//! Э (вложение тождественно развёрнутому содержимому) перестал бы
//! выполняться. Внутрь — потому что «снаружи задать значение внутри» уже
//! сделано и называется публичными параметрами композиции (Р.59).
//!
//! **Время между РАЗНЫМИ событиями — тик ТАЙМЛАЙНА.** Домен ключа — материал
//! своего события, и у двух событий в разных местах монтажа общего
//! материального времени нет. Перевод идёт через `clip_timeline_at` своего
//! события и `clip_material_at` источника — ту же пару, которой пользуется
//! вся программа; второго правила перевода не заводим. Вне тела источника
//! берётся его крайнее значение.

use crate::project::*;
use crate::published::ParamOwner;
use kt_time::Tick;

/// Разобранный адрес ведущего свойства.
///
/// Текстовая форма `"<событие>/<владелец>/<параметр>"`:
/// `"12/fx0/radius"`, `"12/gen/size"`, `"12/clip/opacity"`. Событие — число
/// `ClipId`; `self` означает «это же событие» и не требует его знать.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkPath {
    /// `None` — своё событие.
    pub clip: Option<ClipId>,
    pub owner: ParamOwner,
    pub param: String,
}

impl LinkPath {
    /// Разобрать текст адреса. `None` — адрес не по форме.
    pub fn parse(s: &str) -> Option<LinkPath> {
        let mut it = s.split('/');
        let who = it.next()?;
        let owner = it.next()?;
        let param = it.next()?.to_string();
        if it.next().is_some() || param.is_empty() {
            return None;
        }
        let clip = if who == "self" { None } else { Some(ClipId(who.parse().ok()?)) };
        let owner = match owner {
            "gen" => ParamOwner::Generator,
            "clip" => ParamOwner::ClipAttr,
            o if o.starts_with("fx") => ParamOwner::FxSlot { slot: o[2..].parse().ok()? },
            _ => return None,
        };
        Some(LinkPath { clip, owner, param })
    }

    pub fn to_text(&self) -> String {
        let who = match self.clip {
            None => "self".to_string(),
            Some(c) => c.0.to_string(),
        };
        let owner = match &self.owner {
            ParamOwner::Generator => "gen".to_string(),
            ParamOwner::ClipAttr => "clip".to_string(),
            ParamOwner::FxSlot { slot } => format!("fx{slot}"),
            _ => "?".to_string(),
        };
        format!("{who}/{owner}/{}", self.param)
    }
}

/// Кто спрашивает: событие-потребитель и его геометрия.
#[derive(Debug, Clone, Copy)]
pub struct LinkCtx {
    pub source_in: Tick,
    pub start: Tick,
    pub duration: Tick,
    pub reversed: bool,
    pub frame_step: i64,
}

impl LinkCtx {
    pub fn of(c: &ClipEvent, frame_step: i64) -> LinkCtx {
        LinkCtx {
            source_in: c.source_in,
            start: c.start,
            duration: c.duration,
            reversed: c.reversed,
            frame_step,
        }
    }
}

/// Значение ведущего свойства в материальной позиции ПОТРЕБИТЕЛЯ.
///
/// Внутри ОДНОГО события перевод не нужен — домен общий. Между разными
/// событиями считается общий знаменатель: своя материальная позиция → тик
/// таймлайна → материал источника.
pub fn read_link(reel: &dyn Reel, me: &LinkCtx, path: &LinkPath, at: Tick) -> Option<f64> {
    let clips = reel.clips();
    let src = match path.clip {
        None => return read_at(reel, None, path, at),
        Some(id) => clips.iter().find(|c| crate::same_clip(c.id, id))?,
    };
    // общий знаменатель — ТАЙМЛАЙН
    let tl = clip_timeline_at(
        me.source_in,
        me.start,
        me.duration,
        me.reversed,
        me.frame_step,
        at,
    );
    // вне тела источника берётся его крайнее значение: клампим тик, а не
    // молчим — иначе связь «пропадала» бы на краях и это выглядело бы
    // случайной поломкой
    // границы ВКЛЮЧАЮТ конец события: последний ключ кривой стоит ровно
    // там, и кламп на тик раньше терял бы его
    let lo = src.start.0;
    let hi = src.start.0 + src.duration.0.max(1);
    let tl = Tick(tl.0.clamp(lo, hi));
    let mat = clip_material_at(
        src.source_in,
        src.start,
        src.duration,
        src.reversed,
        me.frame_step,
        tl,
    );
    read_at(reel, Some(src), path, mat)
}

/// Значение свойства владельца в ЕГО материальной позиции.
fn read_at(
    reel: &dyn Reel,
    src: Option<&ClipEvent>,
    path: &LinkPath,
    mat: Tick,
) -> Option<f64> {
    let c = match src {
        Some(c) => c,
        None => reel.clips().iter().find(|c| c.id == path.clip.unwrap_or(c.id))?,
    };
    let curves: &[ParamCurve] = match &path.owner {
        ParamOwner::FxSlot { slot } => c.fx_auto.get(*slot).map(|v| &v[..]).unwrap_or(&[]),
        ParamOwner::Generator => &c.gen_auto,
        _ => &[],
    };
    // живая кривая ведущего свойства главнее ручки — та же семантика, что
    // у остальной программы
    if let Some(cur) = curves.iter().find(|x| x.param == path.param)
        && !cur.keys.is_empty()
        && cur.source.is_keys()
    {
        return Some(curve_value(&cur.keys, mat));
    }
    match &path.owner {
        ParamOwner::FxSlot { slot } => c.fx.get(*slot)?.get_param(&path.param),
        ParamOwner::Generator => c.r#gen.as_ref()?.get_param(&path.param),
        ParamOwner::ClipAttr => match path.param.as_str() {
            "opacity" => Some(c.opacity as f64),
            "gain_db" => Some(c.gain_db as f64),
            _ => None,
        },
        _ => None,
    }
}

/// Значение НЕЧИСЛОВОЙ цели по отметкам оси значения (Р.61).
///
/// Между отметками держится ПРЕДЫДУЩИЙ снимок, ниже первой — первый: рига
/// переключает вариант ступенькой, «половины» между вариантами нет.
pub fn stop_value(stops: &[LinkStop], driver: f64) -> Option<&ParamValue> {
    if stops.is_empty() {
        return None;
    }
    let mut best = &stops[0];
    for s in stops {
        if driver >= s.at && s.at >= best.at {
            best = s;
        }
    }
    // ниже первой отметки — первый снимок
    let first = stops.iter().min_by(|a, b| a.at.total_cmp(&b.at))?;
    Some(if driver < first.at { &first.value } else { &best.value })
}

// ---- граница ленты и циклы (Р.61) ------------------------------------------

/// Проверить связи ОДНОЙ ленты: адреса разрешимы, границу не пересекают,
/// циклов нет.
///
/// Цикл ищется обходом со ВСЕХ вершин, а не от корня: пара свойств, ведущих
/// друг друга и никем не читаемых, всё равно цикл — файл с ней сохранился
/// бы, а открылся бы с ошибкой. Тот же приём, что у цикла лент.
pub fn check_links(reel: &dyn Reel) -> Result<(), LinkError> {
    let clips = reel.clips();
    // вершина графа — «свойство»: (событие, владелец-в-тексте, параметр)
    let mut edges: Vec<(String, String)> = Vec::new();
    for c in clips {
        for (slot, curves) in c.fx_auto.iter().enumerate() {
            for cur in curves {
                collect(c, &ParamOwner::FxSlot { slot }, cur, clips, &mut edges)?;
            }
        }
        for cur in &c.gen_auto {
            collect(c, &ParamOwner::Generator, cur, clips, &mut edges)?;
        }
    }
    if let Some(path) = find_cycle(&edges) {
        return Err(LinkError::Cycle(path));
    }
    Ok(())
}

fn node_name(clip: ClipId, owner: &ParamOwner, param: &str) -> String {
    LinkPath { clip: Some(clip), owner: owner.clone(), param: param.to_string() }.to_text()
}

fn collect(
    c: &ClipEvent,
    owner: &ParamOwner,
    cur: &ParamCurve,
    clips: &[ClipEvent],
    edges: &mut Vec<(String, String)>,
) -> Result<(), LinkError> {
    let Some(text) = cur.source.link_path() else {
        // у формулы адресов нет: имена в ней — переменные, их разрешает
        // тот, кто строит план (стадия 4)
        return Ok(());
    };
    let path = LinkPath::parse(text).ok_or_else(|| LinkError::BadPath(text.to_string()))?;
    // адрес обязан указывать на существующее событие ЭТОЙ ленты: пересечь
    // границу нельзя ни наружу, ни внутрь — см. шапку модуля
    if let Some(id) = path.clip
        && !clips.iter().any(|x| crate::same_clip(x.id, id))
    {
        return Err(LinkError::OutsideReel { from: c.id, to: id });
    }
    let to = node_name(path.clip.unwrap_or(c.id), &path.owner, &path.param);
    edges.push((node_name(c.id, owner, &cur.param), to));
    Ok(())
}

/// Обход СО ВСЕХ вершин: пара, ведущая друг друга и никем не читаемая, —
/// тоже цикл.
fn find_cycle(edges: &[(String, String)]) -> Option<Vec<String>> {
    let mut stack: Vec<String> = Vec::new();
    let mut done: Vec<String> = Vec::new();
    let starts: Vec<&String> = edges.iter().map(|(a, _)| a).collect();
    for s in starts {
        if done.contains(s) {
            continue;
        }
        if let Some(p) = walk(s, edges, &mut stack, &mut done) {
            return Some(p);
        }
    }
    None
}

fn walk(
    at: &str,
    edges: &[(String, String)],
    stack: &mut Vec<String>,
    done: &mut Vec<String>,
) -> Option<Vec<String>> {
    if let Some(i) = stack.iter().position(|x| x == at) {
        let mut p = stack[i..].to_vec();
        p.push(at.to_string());
        return Some(p);
    }
    if done.iter().any(|x| x == at) {
        return None;
    }
    stack.push(at.to_string());
    for (a, b) in edges {
        if a == at
            && let Some(p) = walk(b, edges, stack, done)
        {
            return Some(p);
        }
    }
    stack.pop();
    done.push(at.to_string());
    None
}

/// Чем связь может быть незаконна.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LinkError {
    #[error("link address {0:?} is not of the form <clip>/<owner>/<param>")]
    BadPath(String),
    #[error(
        "clip {from:?} links to clip {to:?}, which is outside its reel — a reel that reads \
         events from outside stops being self-contained; publish a composition parameter instead"
    )]
    OutsideReel { from: ClipId, to: ClipId },
    #[error("properties drive each other in a circle: {}", .0.join(" -> "))]
    Cycle(Vec<String>),
}

// ---- разрешение производных источников (Р.61) ------------------------------

/// Разрешить связи и формулы ленты в ОБЫЧНЫЕ ключевые кривые.
///
/// Зовётся ОДИН раз при сборке рендерера — там же, где кривые превращаются в
/// профили звука; в аудио-колбэке вычислений не появляется.
///
/// **Ноль цены без использования.** Лента, где ни одного производного
/// источника нет, возвращается ЗАИМСТВОВАННОЙ: графа зависимостей не
/// строится вовсе, ни одного клона. Это не оптимизация, а требование
/// владельца «простой проект не лагает».
///
/// **Порядок — топологический.** Ведущее свойство обязано быть разрешено
/// раньше ведомого, иначе связь на связь прочитала бы сырой текст. Циклов
/// здесь уже быть не может: их отвергла валидация.
///
/// **Дискретизация — сетка кадров.** Производная кривая превращается в ключи
/// на той же сетке, по которой читается любая другая кривая: человеческий
/// ключ тоньше её тоже не поставит.
pub fn resolve_derived<'a>(
    clips: &'a [ClipEvent],
    frame_step: i64,
) -> std::borrow::Cow<'a, [ClipEvent]> {
    let any = clips.iter().any(|c| {
        c.fx_auto.iter().flatten().chain(c.gen_auto.iter()).any(|x| !x.source.is_keys())
    });
    if !any {
        return std::borrow::Cow::Borrowed(clips);
    }
    let mut out = clips.to_vec();
    // порядок: сперва события БЕЗ производных источников (они ведущие),
    // потом остальные. Цепочка длиннее двух звеньев разрешается повторными
    // проходами — их не больше, чем событий, а цикл уже отвергнут.
    for _ in 0..=out.len() {
        let snapshot = out.clone();
        let mut moved = false;
        for i in 0..out.len() {
            let me = LinkCtx::of(&snapshot[i], frame_step);
            let id = snapshot[i].id;
            for slot in 0..out[i].fx_auto.len() {
                for j in 0..out[i].fx_auto[slot].len() {
                    if resolve_one(&snapshot, &mut out[i].fx_auto[slot][j], &me, id, frame_step) {
                        moved = true;
                    }
                }
            }
            for j in 0..out[i].gen_auto.len() {
                if resolve_one(&snapshot, &mut out[i].gen_auto[j], &me, id, frame_step) {
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
    std::borrow::Cow::Owned(out)
}

/// Разрешить ОДНУ кривую. `true` — что-то изменилось.
fn resolve_one(
    clips: &[ClipEvent],
    cur: &mut ParamCurve,
    me: &LinkCtx,
    me_id: ClipId,
    frame_step: i64,
) -> bool {
    if cur.source.is_keys() {
        return false;
    }
    let expr = cur.source.expr().and_then(|e| crate::formula::Formula::parse(e).ok());
    let path = cur.source.link_path().and_then(LinkPath::parse);
    let step = frame_step.max(1);
    let from = me.source_in.0;
    let to = from + me.duration.0.max(step);
    let mut keys = Vec::new();
    let mut t = from;
    while t <= to {
        let at = Tick(t);
        let src = match &path {
            Some(p) => {
                let p = LinkPath { clip: p.clip.or(Some(me_id)), ..p.clone() };
                match read_link_in(clips, me, &p, at) {
                    Some(v) => v,
                    // ведущее ещё не разрешено — подождём следующего прохода
                    None => return false,
                }
            }
            None => 0.0,
        };
        let v = match &expr {
            Some(f) => {
                let mut vars = std::collections::BTreeMap::new();
                vars.insert("src".to_string(), src);
                vars.insert("t".to_string(), (t - from) as f64 / 90_000.0);
                match f.eval(&vars) {
                    Ok(v) => v,
                    // невычислимое — не тихий ноль: кривая остаётся сырой,
                    // и валидация уже сказала об этом громко
                    Err(_) => return false,
                }
            }
            None => src,
        };
        keys.push(AutoKey::shaped(at, v, SegShape::Linear));
        t += step;
    }
    if keys.is_empty() {
        return false;
    }
    cur.keys = keys;
    cur.source = CurveSource::Keys;
    true
}

/// То же, что `read_link`, но по срезу событий (лента уже разобрана).
fn read_link_in(
    clips: &[ClipEvent],
    me: &LinkCtx,
    path: &LinkPath,
    at: Tick,
) -> Option<f64> {
    let want = path.clip?;
    let src = clips.iter().find(|c| crate::same_clip(c.id, want))?;
    // ведущее свойство, само ещё не разрешённое, читать нельзя: получили бы
    // значение «до связи», то есть не то, что увидит человек
    let pending = match &path.owner {
        ParamOwner::FxSlot { slot } => src
            .fx_auto
            .get(*slot)
            .is_some_and(|v| v.iter().any(|c| c.param == path.param && !c.source.is_keys())),
        ParamOwner::Generator => {
            src.gen_auto.iter().any(|c| c.param == path.param && !c.source.is_keys())
        }
        _ => false,
    };
    if pending {
        return None;
    }
    let tl = clip_timeline_at(
        me.source_in,
        me.start,
        me.duration,
        me.reversed,
        me.frame_step,
        at,
    );
    let lo = src.start.0;
    let hi = src.start.0 + src.duration.0.max(1);
    let tl = Tick(tl.0.clamp(lo, hi));
    let mat = clip_material_at(
        src.source_in,
        src.start,
        src.duration,
        src.reversed,
        me.frame_step,
        tl,
    );
    read_at_clip(src, path, mat)
}

fn read_at_clip(c: &ClipEvent, path: &LinkPath, mat: Tick) -> Option<f64> {
    let curves: &[ParamCurve] = match &path.owner {
        ParamOwner::FxSlot { slot } => c.fx_auto.get(*slot).map(|v| &v[..]).unwrap_or(&[]),
        ParamOwner::Generator => &c.gen_auto,
        _ => &[],
    };
    if let Some(cur) = curves.iter().find(|x| x.param == path.param)
        && !cur.keys.is_empty()
        && cur.source.is_keys()
    {
        return Some(curve_value(&cur.keys, mat));
    }
    match &path.owner {
        ParamOwner::FxSlot { slot } => c.fx.get(*slot)?.get_param(&path.param),
        ParamOwner::Generator => c.r#gen.as_ref()?.get_param(&path.param),
        ParamOwner::ClipAttr => match path.param.as_str() {
            "opacity" => Some(c.opacity as f64),
            "gain_db" => Some(c.gain_db as f64),
            _ => None,
        },
        _ => None,
    }
}
