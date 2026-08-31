//! Timeline (OTIO) ↔ kt_model::Project + Retained (passthrough-остатки).
//!
//! Маппинг (Р.24, тест 152): видео-lane инвертируется в порядок треков
//! (OTIO композитит поздние треки ПОВЕРХ ранних, у нас lane 0 — верхний);
//! зазоры → Gap; FX → стандартные Effect.1 с effect_name "kt:<kind>" и
//! параметрами в metadata.kt; reversed → LinearTimeWarp{-1} первым;
//! переходы → Transition.1 (SMPTE_Dissolve); метки → Marker.2 корневого
//! Stack; всё внемодельное — конверт metadata.kt (meta.rs). Чужое хранится
//! в Retained и пишется обратно: клипы — по ClipId, дорожки/gap'ы/переходы —
//! по позиции на момент загрузки (документированное ограничение v1).

use indexmap::IndexMap;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;

use crate::meta::{
    KT_META_VERSION, KT_NS, KtBinMedia, KtClipMeta, KtSettings, KtTimelineMeta, KtTransport,
    kt_get, kt_set,
};
use crate::schema::{
    Clip, Composable, Effect, EffectNode, ExternalReference, Gap, LinearTimeWarp, Marker,
    MarkerNode, MediaRefNode, RationalTime, RawObject, Stack, TimeRange, Timeline, Track,
};
use crate::{DEFAULT_MEDIA_KEY, Error};
use kt_model::{
    ClipEvent, ClipId, ClipKind, ForeignEvent, GroupId, MediaId, MediaRef, Project,
    ProjectSettings, TransitionEvent, TransitionId, TransitionKind, UiState,
};
use kt_time::Tick;

/// Замечание импорта — показывается пользователю note'ом.
pub type Notice = String;

/// (extra, metadata) объекта — passthrough-пара.
pub type ExtraMeta = (Map<String, Value>, Map<String, Value>);
/// (transition_type, metadata, extra) чужого перехода.
pub type TransitionExtra = (String, Map<String, Value>, Map<String, Value>);

#[derive(Debug, Clone, Default)]
pub struct ClipExtra {
    pub name: String,
    pub extra: Map<String, Value>,
    pub meta_foreign: Map<String, Value>,
    /// Чужие эффекты (включая непонятые тайм-варпы) с исходным индексом —
    /// на записи реинтерливятся в цепочку.
    pub effects_foreign: Vec<(usize, EffectNode)>,
    /// Маркеры, висящие на клипе (мы своих не создаём — passthrough).
    pub markers: Vec<MarkerNode>,
    /// Прочие media_references (кроме активного) + активный целиком
    /// (available_range/metadata/extra восстанавливаются, target_url наш).
    pub media_refs: IndexMap<String, MediaRefNode>,
    pub active_key: String,
    pub active_ref: Option<ExternalReference>,
}

#[derive(Debug, Clone, Default)]
pub struct TrackExtra {
    pub name_foreign: Option<String>,
    pub extra: Map<String, Value>,
    pub metadata: Map<String, Value>,
    /// extra/metadata gap'ов по порядковому номеру в дорожке.
    pub gaps: Vec<(usize, ExtraMeta)>,
}

/// Passthrough-остатки загруженного файла. Вне undo (нередактируемы);
/// для нового проекта — Default.
#[derive(Debug, Clone, Default)]
pub struct Retained {
    pub timeline_name: String,
    pub timeline_extra: Map<String, Value>,
    /// Чужие неймспейсы metadata Timeline (порядок сохранён, kt исключён).
    pub timeline_meta_foreign: Map<String, Value>,
    pub stack_name: String,
    pub stack_extra: Map<String, Value>,
    pub stack_metadata: Map<String, Value>,
    pub global_start_time: Option<RationalTime>,
    pub tracks: HashMap<(ClipKind, u32), TrackExtra>,
    pub clips: HashMap<ClipId, ClipExtra>,
    /// По паре клипов стыка.
    pub transitions: HashMap<(ClipId, ClipId), TransitionExtra>,
    /// Сырьё foreign-плейсхолдеров по ForeignEvent.key.
    pub foreign: HashMap<u64, RawObject>,
    /// Чужие маркеры корневого Stack (наши — из модели).
    pub stack_markers_foreign: Vec<MarkerNode>,
}

pub struct ProjectBundle {
    pub project: Project,
    pub retained: Retained,
    pub notices: Vec<Notice>,
}

fn rt(t: Tick) -> RationalTime {
    RationalTime::from_ticks(t)
}

fn range(start: Tick, dur: Tick) -> TimeRange {
    TimeRange { start_time: rt(start), duration: rt(dur) }
}

fn fx_tag(fx: &kt_model::FxInstance) -> String {
    let v = serde_json::to_value(fx).expect("FxInstance serializes");
    v.get("kind").and_then(Value::as_str).unwrap_or("unknown").to_string()
}

// ---- Project → Timeline -----------------------------------------------------

pub fn project_to_timeline(p: &Project, retained: &Retained, base_dir: Option<&Path>) -> Timeline {
    let url_of = |path: &str| -> String {
        match base_dir {
            Some(dir) => crate::url::media_target_url(dir, path),
            None => path.replace('\\', "/"),
        }
    };
    let media_path =
        |id: MediaId| p.media.iter().find(|m| m.id == id).map(|m| m.path.clone()).unwrap_or_default();

    // переходы по паре и по incoming-клипу
    let trans_by_b: HashMap<ClipId, &TransitionEvent> =
        p.transitions.iter().map(|t| (t.b, t)).collect();

    let mut children: Vec<Composable> = Vec::new();
    let mut emit_track = |kind: ClipKind, lane: u32| {
        let tx = retained.tracks.get(&(kind, lane)).cloned().unwrap_or_default();
        let mut items: Vec<(Tick, Tick, Result<&ClipEvent, &ForeignEvent>)> = p
            .clips
            .iter()
            .filter(|c| c.kind == kind && c.lane == lane)
            .map(|c| (c.start, c.start + c.duration, Ok(c)))
            .chain(
                p.foreign
                    .iter()
                    .filter(|f| f.kind == kind && f.lane == lane)
                    .map(|f| (f.start, f.start + f.duration, Err(f))),
            )
            .collect();
        items.sort_by_key(|(s, e, _)| (s.0, e.0));
        let mut kids: Vec<Composable> = Vec::new();
        let mut cursor = Tick::ZERO;
        let mut gap_ordinal = 0usize;
        for (start, end, item) in items {
            if start > cursor {
                let (extra, metadata) = tx
                    .gaps
                    .iter()
                    .find(|(i, _)| *i == gap_ordinal)
                    .map(|(_, em)| em.clone())
                    .unwrap_or_default();
                kids.push(Composable::Gap(Gap {
                    name: String::new(),
                    source_range: Some(range(Tick::ZERO, start - cursor)),
                    effects: vec![],
                    markers: vec![],
                    enabled: true,
                    metadata,
                    extra,
                }));
                gap_ordinal += 1;
            }
            match item {
                Ok(c) => {
                    if let Some(t) = trans_by_b.get(&c.id)
                        && p.clips.iter().any(|a| a.id == t.a)
                    {
                        let (ttype, meta, extra) = retained
                            .transitions
                            .get(&(t.a, t.b))
                            .cloned()
                            .unwrap_or(("SMPTE_Dissolve".into(), Map::new(), Map::new()));
                        kids.push(Composable::Transition(crate::schema::Transition {
                            name: String::new(),
                            transition_type: ttype,
                            in_offset: rt(t.in_offset),
                            out_offset: rt(t.out_offset),
                            metadata: meta,
                            extra,
                        }));
                    }
                    kids.push(Composable::Clip(clip_to_otio(
                        c,
                        retained,
                        &url_of(&media_path(c.media)),
                    )));
                }
                Err(f) => {
                    if let Some(raw) = retained.foreign.get(&f.key) {
                        kids.push(Composable::Unknown(raw.clone()));
                    }
                }
            }
            cursor = end;
        }
        // только ЯВНОЕ имя дорожки (дефолтные ярлыки V1/A1 в файл не пишем —
        // иначе чужой/новый проект после круга обрастает фиктивными именами)
        let lk = match kind {
            ClipKind::Video => kt_model::LaneKind::Video,
            ClipKind::Audio => kt_model::LaneKind::Audio,
        };
        let name = p
            .ui
            .as_ref()
            .and_then(|u| {
                u.lane_names.iter().find(|(k, l, _)| *k == lk && *l == lane).map(|(_, _, n)| n.clone())
            })
            .unwrap_or_default();
        children.push(Composable::Track(Track {
            name: if name.is_empty() { tx.name_foreign.clone().unwrap_or_default() } else { name },
            kind: match kind {
                ClipKind::Video => "Video".into(),
                ClipKind::Audio => "Audio".into(),
            },
            children: kids,
            source_range: None,
            effects: vec![],
            markers: vec![],
            enabled: true,
            metadata: tx.metadata.clone(),
            extra: tx.extra.clone(),
        }));
    };
    // видео снизу вверх (OTIO: поздний трек поверх), затем аудио по порядку
    for lane in (0..p.video_lanes).rev() {
        emit_track(ClipKind::Video, lane);
    }
    for lane in 0..p.audio_lanes {
        emit_track(ClipKind::Audio, lane);
    }

    // метки — на корневом Stack (наши + чужие passthrough)
    let mut markers: Vec<MarkerNode> = p
        .markers
        .iter()
        .map(|m| {
            MarkerNode::Marker(Marker {
                name: m.name.clone(),
                color: m.color.clone().unwrap_or_else(Marker::default_color),
                marked_range: range(m.t, m.duration),
                comment: String::new(),
                metadata: Map::new(),
                extra: Map::new(),
            })
        })
        .collect();
    markers.extend(retained.stack_markers_foreign.iter().cloned());

    // metadata.kt таймлайна
    let mut meta = retained.timeline_meta_foreign.clone();
    let mut morphers = p.morphers.clone();
    morphers.sort_by_key(|m| m.id);
    let used: std::collections::HashSet<MediaId> = p.clips.iter().map(|c| c.media).collect();
    let kt = KtTimelineMeta {
        v: KT_META_VERSION,
        settings: Some(KtSettings {
            fps: p.settings.fps,
            width: p.settings.width,
            height: p.settings.height,
            sample_rate: p.settings.sample_rate,
        }),
        transport: (p.region.is_some() || p.loop_enabled).then(|| KtTransport {
            region: p.region.map(|r| (r.start.0, r.end.0)),
            loop_enabled: p.loop_enabled,
        }),
        ui: p.ui.clone(),
        bin_media: p
            .media
            .iter()
            .filter(|m| !used.contains(&m.id))
            .map(|m| KtBinMedia { id: m.id.0, path: url_of(&m.path) })
            .collect(),
        bin_groups: p.ui.as_ref().map(|u| u.bin_groups.clone()).unwrap_or_default(),
        bin_layout: p.ui.as_ref().map(|u| u.bin_layout.clone()).unwrap_or_default(),
        bin_free: p.ui.as_ref().map(|u| u.bin_free).unwrap_or(false),
        morph_lanes: p.morph_lanes,
        morphers,
    };
    kt_set(&mut meta, &kt);

    Timeline {
        name: retained.timeline_name.clone(),
        global_start_time: retained.global_start_time,
        metadata: meta,
        tracks: Stack {
            name: retained.stack_name.clone(),
            children,
            source_range: None,
            effects: vec![],
            markers,
            enabled: true,
            metadata: retained.stack_metadata.clone(),
            extra: retained.stack_extra.clone(),
        },
        extra: retained.timeline_extra.clone(),
    }
}

fn clip_to_otio(c: &ClipEvent, retained: &Retained, target_url: &str) -> Clip {
    let ex = retained.clips.get(&c.id).cloned().unwrap_or_default();
    // эффекты: LTW(-1) при реверсе первым, затем наша цепочка, чужие —
    // по исходным индексам
    let mut effects: Vec<EffectNode> = Vec::new();
    if c.reversed {
        effects.push(EffectNode::LinearTimeWarp(LinearTimeWarp {
            name: String::new(),
            effect_name: "LinearTimeWarp".into(),
            time_scalar: -1.0,
            enabled: true,
            metadata: Map::new(),
            extra: Map::new(),
        }));
    }
    for (i, fx) in c.fx.iter().enumerate() {
        let mut md = Map::new();
        kt_set(&mut md, fx);
        effects.push(EffectNode::Effect(Effect {
            name: String::new(),
            effect_name: format!("kt:{}", fx_tag(fx)),
            enabled: !c.fx_bypass.get(i).copied().unwrap_or(false),
            metadata: md,
            extra: Map::new(),
        }));
    }
    for (i, e) in &ex.effects_foreign {
        let at = (*i).min(effects.len());
        effects.insert(at, e.clone());
    }

    let mut media_refs = ex.media_refs.clone();
    let mut active = ex.active_ref.clone().unwrap_or(ExternalReference {
        name: String::new(),
        target_url: String::new(),
        available_range: None,
        available_image_bounds: None,
        metadata: Map::new(),
        extra: Map::new(),
    });
    active.target_url = target_url.to_string();
    let active_key =
        if ex.active_key.is_empty() { DEFAULT_MEDIA_KEY.to_string() } else { ex.active_key };
    media_refs.insert(active_key.clone(), MediaRefNode::External(active));

    let mut meta = ex.meta_foreign.clone();
    kt_set(
        &mut meta,
        &KtClipMeta {
            id: c.id.0,
            media: c.media.0,
            link_group: c.link_group.map(|g| g.0),
            gain: c.gain_db,
        },
    );

    Clip {
        name: ex.name,
        source_range: Some(range(c.source_in, c.duration)),
        media_references: media_refs,
        active_media_reference_key: active_key,
        effects,
        markers: ex.markers,
        enabled: true,
        metadata: meta,
        extra: ex.extra,
    }
}

// ---- Timeline → Project -----------------------------------------------------

pub fn timeline_to_project(
    tl: &Timeline,
    base_dir: Option<&Path>,
) -> Result<ProjectBundle, Error> {
    crate::meta::check_kt_version(tl)?;
    let mut notices: Vec<Notice> = Vec::new();
    let kt: Option<KtTimelineMeta> = kt_get(&tl.metadata);
    let is_ours = kt.is_some();
    let settings = kt
        .as_ref()
        .and_then(|k| k.settings)
        .map(|s| ProjectSettings {
            fps: s.fps,
            width: s.width,
            height: s.height,
            sample_rate: s.sample_rate,
        })
        .unwrap_or_else(|| {
            notices.push("imported with default project settings".into());
            ProjectSettings { fps: guess_fps(tl), width: 1920, height: 1080, sample_rate: 48000 }
        });
    let mut p = Project::new(settings);
    let mut retained = Retained {
        timeline_name: tl.name.clone(),
        timeline_extra: tl.extra.clone(),
        timeline_meta_foreign: {
            let mut m = tl.metadata.clone();
            m.shift_remove(KT_NS);
            m
        },
        stack_name: tl.tracks.name.clone(),
        stack_extra: tl.tracks.extra.clone(),
        stack_metadata: tl.tracks.metadata.clone(),
        global_start_time: tl.global_start_time,
        ..Default::default()
    };

    // маркеры корневого Stack
    for m in &tl.tracks.markers {
        match m {
            MarkerNode::Marker(mk) => p.markers.push(kt_model::Marker {
                t: mk.marked_range.start_time.to_ticks(),
                duration: Tick(mk.marked_range.duration.to_ticks().0.max(0)),
                name: mk.name.clone(),
                color: Some(mk.color.clone()),
            }),
            MarkerNode::Unknown(_) => retained.stack_markers_foreign.push(m.clone()),
        }
    }

    // дорожки: видео нумеруются с ВЕРХНЕЙ (последний видео-трек = lane 0)
    let video_total =
        tl.tracks.children.iter().filter(|c| track_kind(c) == Some(ClipKind::Video)).count() as u32;
    let mut seen_video = 0u32;
    let mut seen_audio = 0u32;
    let mut next_clip_id = 1u64;
    let mut next_foreign_key = 1u64;
    let mut media_by_url: HashMap<String, MediaId> = HashMap::new();
    let mut next_media_id = 1u64;

    for child in &tl.tracks.children {
        let Some(kind) = track_kind(child) else {
            notices.push(format!(
                "unsupported track-level object was kept untouched: {}",
                composable_tag(child)
            ));
            // хребет типизирован — перекодируем в сырьё для Retained
            let raw = to_raw(child);
            retained.foreign.insert(next_foreign_key, raw);
            next_foreign_key += 1;
            continue;
        };
        let Composable::Track(track) = child else { unreachable!() };
        let lane = match kind {
            ClipKind::Video => {
                seen_video += 1;
                video_total - seen_video
            }
            ClipKind::Audio => {
                seen_audio += 1;
                seen_audio - 1
            }
        };
        let mut tx = TrackExtra {
            name_foreign: (!track.name.is_empty()).then(|| track.name.clone()),
            extra: track.extra.clone(),
            metadata: track.metadata.clone(),
            gaps: Vec::new(),
        };
        if !track.name.is_empty() {
            let ui = p.ui.get_or_insert_with(UiState::default);
            let lk = match kind {
                ClipKind::Video => kt_model::LaneKind::Video,
                ClipKind::Audio => kt_model::LaneKind::Audio,
            };
            ui.lane_names.push((lk, lane, track.name.clone()));
        }
        let mut cursor = Tick::ZERO;
        let mut gap_ordinal = 0usize;
        let mut pending_transition: Option<(Tick, Tick, TransitionExtra)> = None;
        let mut prev_clip: Option<ClipId> = None;
        for item in &track.children {
            match item {
                Composable::Gap(g) => {
                    let dur = item_duration(&g.source_range).ok_or_else(|| {
                        Error::UnsupportedSchema("Gap without source_range".into())
                    })?;
                    if !g.extra.is_empty() || !g.metadata.is_empty() {
                        tx.gaps.push((gap_ordinal, (g.extra.clone(), g.metadata.clone())));
                    }
                    gap_ordinal += 1;
                    cursor = cursor + dur;
                    prev_clip = None;
                }
                Composable::Transition(t) => {
                    pending_transition = Some((
                        t.in_offset.to_ticks(),
                        t.out_offset.to_ticks(),
                        (t.transition_type.clone(), t.metadata.clone(), t.extra.clone()),
                    ));
                }
                Composable::Clip(c) if c.enabled => {
                    let dur = item_duration(&c.source_range).ok_or_else(|| {
                        Error::UnsupportedSchema("Clip without source_range".into())
                    })?;
                    let (ev, ex) = clip_from_otio(
                        c,
                        kind,
                        lane,
                        cursor,
                        dur,
                        is_ours,
                        &mut next_clip_id,
                        &mut media_by_url,
                        &mut next_media_id,
                        base_dir,
                        &mut p,
                        &mut notices,
                    );
                    if let Some((in_off, out_off, (ttype, meta, extra))) = pending_transition.take() {
                        if let Some(a) = prev_clip {
                            let id = p.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
                            if ttype != "SMPTE_Dissolve" {
                                notices
                                    .push(format!("transition '{ttype}' imported as crossfade"));
                            }
                            retained.transitions.insert((a, ev.id), (ttype, meta, extra));
                            p.transitions.push(TransitionEvent {
                                id: TransitionId(id),
                                a,
                                b: ev.id,
                                in_offset: in_off,
                                out_offset: out_off,
                                kind: TransitionKind::Crossfade,
                            });
                        } else {
                            notices.push(
                                "transition at a track edge is not supported yet; kept none"
                                    .into(),
                            );
                        }
                    }
                    prev_clip = Some(ev.id);
                    retained.clips.insert(ev.id, ex);
                    p.clips.push(ev);
                    cursor = cursor + dur;
                }
                other => {
                    // выключенный клип / неизвестная схема / вложенный
                    // Stack|Track → серый плейсхолдер (Р.24 п.5)
                    let dur = composable_duration(other).ok_or_else(|| {
                        Error::UnsupportedSchema(format!(
                            "{} without readable duration",
                            composable_tag(other)
                        ))
                    })?;
                    if pending_transition.take().is_some() {
                        notices.push("transition next to a foreign object was dropped".into());
                    }
                    p.foreign.push(ForeignEvent {
                        key: next_foreign_key,
                        kind,
                        lane,
                        start: cursor,
                        duration: dur,
                    });
                    retained.foreign.insert(next_foreign_key, to_raw(other));
                    next_foreign_key += 1;
                    prev_clip = None;
                    cursor = cursor + dur;
                }
            }
        }
        if pending_transition.is_some() {
            notices.push("transition at a track edge is not supported yet; kept none".into());
        }
        retained.tracks.insert((kind, lane), tx);
    }

    // конверт kt: транспорт/ui/бин/морферы
    if let Some(kt) = kt {
        if let Some(tr) = kt.transport {
            p.region = tr.region.and_then(|(s, e)| {
                kt_model::Region::normalized(Tick(s), Tick(e))
            });
            p.loop_enabled = tr.loop_enabled;
        }
        if let Some(mut ui) = kt.ui {
            // имена дорожек v/a приезжают из Track.name; morph — из ui
            let mut names = p.ui.take().map(|u| u.lane_names).unwrap_or_default();
            ui.lane_names.retain(|(k, _, _)| *k == kt_model::LaneKind::Morph);
            names.extend(ui.lane_names.clone());
            ui.lane_names = names;
            ui.bin_groups = kt.bin_groups;
            ui.bin_layout = kt.bin_layout;
            ui.bin_free = kt.bin_free;
            p.ui = Some(ui);
        }
        for bm in kt.bin_media {
            let path = resolve_url(base_dir, &bm.path);
            p.media.push(MediaRef { id: MediaId(bm.id), path });
        }
        p.morph_lanes = kt.morph_lanes;
        p.morphers = kt.morphers;
    }

    // детерминированный порядок независимо от порядка обхода дорожек
    // (низ-вверх для видео): save→load→save стабилен, PartialEq честен
    p.clips.sort_by_key(|c| c.id);
    p.foreign.sort_by_key(|f| f.key);
    p.media.sort_by_key(|m| m.id);
    p.normalize();
    p.validate().map_err(|e| Error::UnsupportedSchema(format!("invalid timeline: {e}")))?;
    Ok(ProjectBundle { project: p, retained, notices })
}

#[allow(clippy::too_many_arguments)]
fn clip_from_otio(
    c: &Clip,
    kind: ClipKind,
    lane: u32,
    start: Tick,
    dur: Tick,
    is_ours: bool,
    next_clip_id: &mut u64,
    media_by_url: &mut HashMap<String, MediaId>,
    next_media_id: &mut u64,
    base_dir: Option<&Path>,
    p: &mut Project,
    notices: &mut Vec<Notice>,
) -> (ClipEvent, ClipExtra) {
    let meta: Option<KtClipMeta> = kt_get(&c.metadata);
    let url = match c.active_media() {
        Some(MediaRefNode::External(er)) => er.target_url.clone(),
        _ => String::new(),
    };
    let media = match meta {
        Some(m) if is_ours => MediaId(m.media),
        _ => *media_by_url.entry(url.clone()).or_insert_with(|| {
            let id = MediaId(*next_media_id);
            *next_media_id += 1;
            id
        }),
    };
    if !p.media.iter().any(|m| m.id == media) {
        p.media.push(MediaRef { id: media, path: resolve_url(base_dir, &url) });
    }
    let id = match meta {
        Some(m) if is_ours => ClipId(m.id),
        _ => {
            let id = ClipId(*next_clip_id);
            *next_clip_id += 1;
            id
        }
    };

    // эффекты: kt:* → цепочка, LTW ±1 → reversed/no-op, прочее → foreign
    let mut fx = Vec::new();
    let mut bypass = Vec::new();
    let mut reversed = false;
    let mut ex = ClipExtra {
        name: c.name.clone(),
        extra: c.extra.clone(),
        meta_foreign: {
            let mut m = c.metadata.clone();
            m.shift_remove(KT_NS);
            m
        },
        markers: c.markers.clone(),
        media_refs: {
            let mut refs = c.media_references.clone();
            refs.shift_remove(&c.active_media_reference_key);
            refs
        },
        active_key: c.active_media_reference_key.clone(),
        active_ref: match c.active_media() {
            Some(MediaRefNode::External(er)) => Some(er.clone()),
            _ => None,
        },
        ..Default::default()
    };
    for (i, e) in c.effects.iter().enumerate() {
        match e {
            EffectNode::Effect(eff) if eff.effect_name.starts_with("kt:") => {
                match kt_get::<kt_model::FxInstance>(&eff.metadata) {
                    Some(inst) => {
                        fx.push(inst);
                        bypass.push(!eff.enabled);
                    }
                    None => {
                        notices.push(format!("unreadable effect {} kept as foreign", eff.effect_name));
                        ex.effects_foreign.push((i, e.clone()));
                    }
                }
            }
            EffectNode::LinearTimeWarp(w) if w.time_scalar == -1.0 => reversed = true,
            EffectNode::LinearTimeWarp(w) if w.time_scalar == 1.0 => {}
            EffectNode::LinearTimeWarp(w) => {
                notices.push(format!(
                    "time warp x{} is not rendered (kept untouched)",
                    w.time_scalar
                ));
                ex.effects_foreign.push((i, e.clone()));
            }
            other => ex.effects_foreign.push((i, other.clone())),
        }
    }
    while bypass.last() == Some(&false) {
        bypass.pop(); // хвост «включено» не сериализуем (форма модели)
    }

    let src_in = c
        .source_range
        .as_ref()
        .map(|r| Tick(r.start_time.to_ticks().0.max(0)))
        .unwrap_or(Tick::ZERO);
    let ev = ClipEvent {
        id,
        link_group: meta.and_then(|m| m.link_group).map(GroupId),
        kind,
        lane,
        start,
        duration: dur,
        source_in: src_in,
        media,
        gain_db: meta.map(|m| m.gain).unwrap_or(0.0),
        reversed,
        fx,
        fx_bypass: bypass,
    };
    (ev, ex)
}

fn track_kind(c: &Composable) -> Option<ClipKind> {
    match c {
        Composable::Track(t) if t.kind == "Video" => Some(ClipKind::Video),
        Composable::Track(t) if t.kind == "Audio" => Some(ClipKind::Audio),
        _ => None,
    }
}

fn composable_tag(c: &Composable) -> String {
    match c {
        Composable::Unknown(raw) => raw.schema_tag().unwrap_or("?").to_string(),
        Composable::Clip(_) => "Clip".into(),
        Composable::Gap(_) => "Gap".into(),
        Composable::Transition(_) => "Transition".into(),
        Composable::Stack(_) => "Stack".into(),
        Composable::Track(_) => "Track".into(),
    }
}

fn item_duration(r: &Option<TimeRange>) -> Option<Tick> {
    r.as_ref().map(|r| Tick(r.duration.to_ticks().0.max(0)))
}

fn composable_duration(c: &Composable) -> Option<Tick> {
    match c {
        Composable::Clip(x) => item_duration(&x.source_range),
        Composable::Gap(x) => item_duration(&x.source_range),
        Composable::Stack(x) => item_duration(&x.source_range),
        Composable::Track(x) => item_duration(&x.source_range),
        Composable::Unknown(raw) => {
            let r: Option<TimeRange> =
                raw.0.get("source_range").and_then(|v| serde_json::from_value(v.clone()).ok());
            r.map(|r| Tick(r.duration.to_ticks().0.max(0)))
        }
        Composable::Transition(_) => None,
    }
}

fn to_raw(c: &Composable) -> RawObject {
    match crate::schema::encode_composable(c) {
        Value::Object(m) => RawObject(m),
        _ => unreachable!(),
    }
}

fn resolve_url(base_dir: Option<&Path>, url: &str) -> String {
    match base_dir {
        Some(dir) => crate::url::resolve_media_path(dir, url).to_string_lossy().into_owned(),
        None => url.to_string(),
    }
}

/// Эвристика fps чужого файла: самый частый целый rate среди source_range,
/// делящий 90000; иначе 30.
fn guess_fps(tl: &Timeline) -> u32 {
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for child in &tl.tracks.children {
        if let Composable::Track(t) = child {
            for item in &t.children {
                if let Composable::Clip(c) = item
                    && let Some(r) = &c.source_range
                {
                    let rate = r.duration.rate;
                    if rate.fract() == 0.0 && rate > 0.0 {
                        let rate = rate as u32;
                        if rate <= 240 && kt_time::TICK_HZ % rate as i64 == 0 {
                            *counts.entry(rate).or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(r, _)| r).unwrap_or(30)
}
