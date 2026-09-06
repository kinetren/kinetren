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
    KT_NS, KtBinMedia, KtClipMeta, KtSettings, KtTimelineMeta, KtTransport,
    kt_get, kt_set,
};
use crate::schema::{
    Clip, Composable, Effect, EffectNode, ExternalReference, Gap, LinearTimeWarp, Marker,
    MarkerNode, MediaRefNode, RationalTime, RawObject, Stack, TimeRange, Timeline, Track,
};
use crate::{DEFAULT_MEDIA_KEY, Error};
use kt_model::{
    ClipEvent, ClipId, ClipKind, ForeignEvent, GroupId, MediaId, MediaRef, Project,
    AudioCurve, ProjectSettings, TransitionEvent, TransitionId, TransitionKind, UiState,
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
    /// ВСЕ media_references клипа как в файле (включая активный и `null`:
    /// клип без привязки к материалу законен). На записи активный ключ
    /// подменяется нашим target_url — но только если материал есть; иначе
    /// карта пишется нетронутой (чужой `null` не превращается в ссылку).
    pub media_refs: IndexMap<String, Option<MediaRefNode>>,
    pub active_key: String,
    /// Неизвестные поля НАШЕГО клип-конверта kt (будущие версии, Р.29).
    pub kt_extra: Map<String, Value>,
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
    /// Ключ — пара концов перехода; у фейда (Р.25) один конец None.
    pub transitions: HashMap<(Option<ClipId>, Option<ClipId>), TransitionExtra>,
    /// Сырьё foreign-плейсхолдеров по ForeignEvent.key.
    pub foreign: HashMap<u64, RawObject>,
    /// Чужие маркеры корневого Stack (наши — из модели).
    pub stack_markers_foreign: Vec<MarkerNode>,
    /// Неизвестные поля НАШЕГО конверта kt (будущие версии, Р.29):
    /// passthrough — v2-сборка не теряет добавки v3.
    pub timeline_kt_extra: Map<String, Value>,
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

    // переход пишется ПЕРЕД своим incoming-клипом; фейд-аут (правого клипа
    // нет) — ПОСЛЕ своего outgoing (Р.25)
    let trans_before: HashMap<ClipId, &TransitionEvent> =
        p.transitions.iter().filter_map(|t| Some((t.b?, t))).collect();
    let trans_after: HashMap<ClipId, &TransitionEvent> = p
        .transitions
        .iter()
        .filter(|t| t.b.is_none())
        .filter_map(|t| Some((t.a?, t)))
        .collect();

    let mut children: Vec<Composable> = Vec::new();
    // Переслаивание (Р.28): в OTIO позиция клипа определяется ПОРЯДКОМ детей
    // трека, поэтому перекрытые клипы одной нашей дорожки невыразимы в одном
    // Track — их пришлось бы писать подряд, и чужой редактор увидел бы
    // разъехавшийся монтаж. Раскладываем жадно по слоям: клип идёт в первый
    // слой, где ни с кем не пересекается; каждый слой пишется отдельным
    // Track, а номер НАШЕЙ дорожки едет в `metadata.kt.lane` трека — по нему
    // мы собираем дорожку обратно (решение пользователя Р.28).
    let layers_of = |kind: ClipKind, lane: u32| -> usize {
        // дорожка без содержимого, которой не было в исходном файле, не
        // пишется вовсе (Р.25) — в карте слоёв у неё ноль треков, иначе
        // карта не сойдётся с файлом и при чтении будет отброшена
        let known = retained.tracks.contains_key(&(kind, lane));
        let has_content = p.clips.iter().any(|c| c.kind == kind && c.lane == lane)
            || p.foreign.iter().any(|f| f.kind == kind && f.lane == lane);
        if !known && !has_content {
            return 0;
        }
        let mut spans: Vec<(i64, i64)> = Vec::new();
        let mut ends: Vec<i64> = Vec::new();
        for c in p.clips.iter().filter(|c| c.kind == kind && c.lane == lane) {
            spans.push((c.start.0, (c.start + c.duration).0));
        }
        spans.sort_unstable();
        for (s, e) in spans {
            match ends.iter_mut().find(|end| **end <= s) {
                Some(end) => *end = e,
                None => ends.push(e),
            }
        }
        ends.len().max(1)
    };
    let layer_of = |c: &ClipEvent, kind: ClipKind, lane: u32| -> usize {
        let mut ends: Vec<i64> = Vec::new();
        let mut mine = 0usize;
        let mut spans: Vec<(i64, i64, ClipId)> = p
            .clips
            .iter()
            .filter(|x| x.kind == kind && x.lane == lane)
            .map(|x| (x.start.0, (x.start + x.duration).0, x.id))
            .collect();
        spans.sort_unstable();
        for (s, e, id) in spans {
            let idx = match ends.iter().position(|end| *end <= s) {
                Some(i) => {
                    ends[i] = e;
                    i
                }
                None => {
                    ends.push(e);
                    ends.len() - 1
                }
            };
            if id == c.id {
                mine = idx;
            }
        }
        mine
    };
    let mut emit_track = |kind: ClipKind, lane: u32, layer: usize, timeline_end: Tick| {
        // чужие extra/metadata/gap'ы принадлежат ИСХОДНОМУ треку — их несёт
        // слой 0, иначе расслоение продублировало бы их на каждый слой
        let tx = if layer == 0 {
            retained.tracks.get(&(kind, lane)).cloned().unwrap_or_default()
        } else {
            TrackExtra::default()
        };
        let known = retained.tracks.contains_key(&(kind, lane));
        let has_content = p.clips.iter().any(|c| c.kind == kind && c.lane == lane)
            || p.foreign.iter().any(|f| f.kind == kind && f.lane == lane);
        if !known && !has_content {
            return; // пустая дорожка, которой не было в исходном файле
        }
        let mut items: Vec<(Tick, Tick, Result<&ClipEvent, &ForeignEvent>)> = p
            .clips
            .iter()
            .filter(|c| c.kind == kind && c.lane == lane && layer_of(c, kind, lane) == layer)
            .map(|c| (c.start, c.start + c.duration, Ok(c)))
            .chain(
                p.foreign
                    .iter()
                    .filter(|f| f.kind == kind && f.lane == lane && layer == 0)
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
                    let emit_transition = |kids: &mut Vec<Composable>, t: &TransitionEvent| {
                        let (ttype, mut meta, extra) = retained
                            .transitions
                            .get(&(t.a, t.b))
                            .cloned()
                            .unwrap_or_else(|| (String::new(), Map::new(), Map::new()));
                        // Тип перехода наружу (Р.33): кроссфейд — стандартный
                        // SMPTE_Dissolve, вайпы — SMPTE_Wipe, остальное —
                        // Custom. Чужой тип, приехавший в Retained, не
                        // трогаем: правило «не понимаем — возвращаем как
                        // было» (т.152).
                        let ttype = if !ttype.is_empty() {
                            ttype
                        } else {
                            match &t.kind {
                                TransitionKind::Crossfade => "SMPTE_Dissolve".into(),
                                TransitionKind::Wipe { .. }
                                | TransitionKind::Iris { .. }
                                | TransitionKind::Clock { .. }
                                | TransitionKind::Barn { .. }
                                | TransitionKind::Luma { .. } => "SMPTE_Wipe".into(),
                                _ => "Custom".to_string(),
                            }
                        };
                        // наши параметры — в свой конверт; умолчания не
                        // пишутся, поэтому обычный кроссфейд байт-стабилен
                        if t.kind != TransitionKind::Crossfade
                            || t.audio != AudioCurve::EqualPower
                            || !t.auto.is_empty()
                        {
                            let e = crate::meta::kt_entry_mut(&mut meta);
                            e.insert(
                                "kind".into(),
                                serde_json::to_value(&t.kind).unwrap_or(Value::Null),
                            );
                            if t.audio != AudioCurve::EqualPower {
                                e.insert(
                                    "audio".into(),
                                    serde_json::to_value(t.audio).unwrap_or(Value::Null),
                                );
                            }
                            if !t.auto.is_empty() {
                                e.insert(
                                    "auto".into(),
                                    serde_json::to_value(&t.auto).unwrap_or(Value::Null),
                                );
                            }
                        }
                        kids.push(Composable::Transition(crate::schema::Transition {
                            name: String::new(),
                            transition_type: ttype,
                            in_offset: rt(t.in_offset),
                            out_offset: rt(t.out_offset),
                            metadata: meta,
                            extra,
                        }));
                    };
                    if let Some(t) = trans_before.get(&c.id)
                        && t.a.is_none_or(|a| p.clips.iter().any(|x| x.id == a))
                    {
                        emit_transition(&mut kids, t);
                    }
                    kids.push(Composable::Clip(clip_to_otio(
                        c,
                        retained,
                        &url_of(&media_path(c.media)),
                    )));
                    if let Some(t) = trans_after.get(&c.id) {
                        emit_transition(&mut kids, t);
                    }
                }
                Err(f) => {
                    if let Some(raw) = retained.foreign.get(&f.key) {
                        kids.push(Composable::Unknown(raw.clone()));
                    }
                }
            }
            cursor = end;
        }
        if timeline_end > cursor {
            kids.push(Composable::Gap(Gap {
                name: String::new(),
                source_range: Some(range(Tick::ZERO, timeline_end - cursor)),
                effects: vec![],
                markers: vec![],
                enabled: true,
                metadata: Map::new(),
                extra: Map::new(),
            }));
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
            name: if layer > 0 {
                String::new() // имя дорожки несёт слой 0
            } else if name.is_empty() {
                tx.name_foreign.clone().unwrap_or_default()
            } else {
                name
            },
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
    // Монтажная длина (Р.25): дорожка короче конца таймлайна дописывается
    // хвостовым Gap'ом — так чужой файл задаёт длину и так его прочтут
    // обратно. Дорожку, которой в файле не было и в которой ничего нет,
    // не пишем вовсе (число дорожек живёт в metadata.kt).
    let timeline_end = p.timeline_end.unwrap_or(Tick::ZERO);
    // видео снизу вверх (OTIO: поздний трек поверх), затем аудио по порядку
    for lane in (0..p.video_lanes).rev() {
        for layer in 0..layers_of(ClipKind::Video, lane) {
            emit_track(ClipKind::Video, lane, layer, timeline_end);
        }
    }
    for lane in 0..p.audio_lanes {
        for layer in 0..layers_of(ClipKind::Audio, lane) {
            emit_track(ClipKind::Audio, lane, layer, timeline_end);
        }
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
    let mut cameras = p.cameras.clone();
    cameras.sort_by_key(|c| c.id);
    let used: std::collections::HashSet<MediaId> = p.clips.iter().map(|c| c.media).collect();
    let kt = KtTimelineMeta {
        // версия ПО СОДЕРЖИМОМУ (Р.29/Р.30): камеры и автоматизация
        // параметров поднимают до v2 (старый ридер молча потерял бы их
        // при перезаписи), без них — v1
        v: {
            let has_auto = p
                .clips
                .iter()
                .any(|c| c.fx_auto.iter().any(|v| v.iter().any(|c| !c.keys.is_empty())))
                // кривые камер — тот же механизм и та же версия
                || cameras.iter().any(|c| !c.auto.is_empty());
            // выключатели тоже v3: старая сборка не знает про них и
            // сыграла бы выключенное (правило «молчаливой потери», Р.29)
            let has_off =
                p.clips.iter().any(|c| c.fx_off) || p.morphers.iter().any(|m| !m.enabled);
            // вид перехода и кривая аудио — тоже v3: старая сборка
            // прочитала бы вайп как кроссфейд и молча испортила бы монтаж
            let has_tr = p
                .transitions
                .iter()
                .any(|t| {
                    t.kind != TransitionKind::Crossfade
                        || t.audio != AudioCurve::default()
                        || !t.auto.is_empty()
                });
            let has_cams = !cameras.is_empty() || p.camera_lanes > 0;
            // версия ПО СОДЕРЖИМОМУ: кривые нового домена — v3, камеры —
            // v2, иначе v1 (файл без наших новшеств открывается старыми
            // сборками и остаётся байт-стабильным)
            // морфер-слой (Р.35): действует по времени или целится не
            // замороженным списком — старая сборка сыграла бы не то
            let has_layer = !p.areas.is_empty()
                || p.morphers
                    .iter()
                    .any(|m| m.timed || !matches!(m.targets, kt_model::MorphTargets::Clips { .. }));
            if has_layer {
                crate::meta::KT_META_V4
            } else if has_auto || has_off || has_tr {
                crate::meta::KT_META_V3
            } else if has_cams {
                crate::meta::KT_META_V2
            } else {
                crate::meta::KT_META_V1
            }
        },
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
        // в kt.ui — только morph/camera-имена (v/a живут в Track.name) и
        // без bin-полей (они в kt.bin_*): единственный источник каждого
        // факта
        ui: p.ui.clone().map(|mut u| {
            u.lane_names.retain(|(k, _, _)| {
                matches!(k, kt_model::LaneKind::Morph | kt_model::LaneKind::Camera)
            });
            u.bin_groups = Vec::new();
            u.bin_layout = Vec::new();
            u.bin_free = false;
            u
        }),
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
        lane_order: p.lane_order.clone(),
        areas: p.areas.clone(),
        // длина сверх материала и число дорожек (пустые в файл не пишем)
        end: p.timeline_end.map(|t| t.0),
        lanes: Some((p.video_lanes, p.audio_lanes)),
        layers: {
            let v: Vec<u32> =
                (0..p.video_lanes).map(|l| layers_of(ClipKind::Video, l) as u32).collect();
            let a: Vec<u32> =
                (0..p.audio_lanes).map(|l| layers_of(ClipKind::Audio, l) as u32).collect();
            // пишем, только когда есть что сказать: без перекрытий и с
            // включённой опцией файл остаётся байт-в-байт прежним
            let multi = v.iter().chain(a.iter()).any(|n| *n > 1);
            (multi || !p.overlap_layers).then(|| crate::meta::KtLayers {
                off: !p.overlap_layers,
                video: if multi { v } else { Vec::new() },
                audio: if multi { a } else { Vec::new() },
            })
        },
        morphers,
        camera_lanes: p.camera_lanes,
        cameras,
        extra: retained.timeline_kt_extra.clone(),
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
        // автоматизация (Р.30): кривые слота едут ключом "auto" внутри
        // kt-конверта эффекта; FxInstance незнакомые поля игнорирует,
        // старый ридер потерял бы их молча → файл поднимается до v2
        if let Some(curves) = c.fx_auto.get(i).filter(|v| !v.is_empty()) {
            crate::meta::kt_entry_mut(&mut md).insert(
                "auto".into(),
                serde_json::to_value(curves).expect("curves are serializable"),
            );
        }
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
    let active_key =
        if ex.active_key.is_empty() { DEFAULT_MEDIA_KEY.to_string() } else { ex.active_key };
    if !target_url.is_empty() {
        // available_range/metadata/extra прежней ссылки сохраняются,
        // подменяется только адрес материала
        let mut active = match media_refs.get(&active_key) {
            Some(Some(MediaRefNode::External(er))) => er.clone(),
            _ => ExternalReference {
                name: String::new(),
                target_url: String::new(),
                available_range: None,
                available_image_bounds: None,
                metadata: Map::new(),
                extra: Map::new(),
            },
        };
        active.target_url = target_url.to_string();
        media_refs.insert(active_key.clone(), Some(MediaRefNode::External(active)));
    }

    // начало available_range активной ссылки — тот самый таймкод, который
    // был вычтен на чтении (п.180)
    let media_origin = media_refs
        .get(&active_key)
        .and_then(|n| n.as_ref())
        .and_then(|n| match n {
            MediaRefNode::External(er) => er.available_range,
            MediaRefNode::Missing(mr) => mr.available_range,
            MediaRefNode::Unknown(raw) => raw
                .0
                .get("available_range")
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
        })
        .map(|r| r.start_time.to_ticks().0)
        .unwrap_or(0);

    let mut meta = ex.meta_foreign.clone();
    kt_set(
        &mut meta,
        &KtClipMeta {
            id: c.id.0,
            media: c.media.0,
            link_group: c.link_group.map(|g| g.0),
            gain: c.gain_db,
            opacity: c.opacity,
            fx_off: c.fx_off,
            extra: ex.kt_extra.clone(),
        },
    );

    Clip {
        name: ex.name,
        // таймкод материала возвращается на место (п.180): чужой файл
        // обязан прочитаться другим редактором так же, как пришёл
        source_range: Some(range(Tick(c.source_in.0 + media_origin), c.duration)),
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
    // нечитаемые кривые автоматизации: копим и валим загрузку в конце
    let mut bad_auto: Vec<String> = Vec::new();
    let kt: Option<KtTimelineMeta> = kt_get(&tl.metadata);
    let is_ours = kt.is_some();
    // версия конверта файла: до v3 кривые автоматизации лежали в долях
    // длины события, теперь — в тиках материала (Р.33)
    let file_v = crate::meta::kt_version(tl).unwrap_or(crate::meta::KT_META_V1);
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
    let audio_total =
        tl.tracks.children.iter().filter(|c| track_kind(c) == Some(ClipKind::Audio)).count() as u32;
    // Карта слоёв (Р.28): перекрытые клипы одной НАШЕЙ дорожки лежат в
    // соседних Track'ах — здесь они собираются обратно. Карта берётся из
    // нашего конверта; если она не сходится с файлом (чужой редактор
    // пересобрал треки), она игнорируется и дорожки читаются один к одному.
    let layer_plan = |kind: ClipKind| -> Option<Vec<u32>> {
        let l = kt.as_ref()?.layers.as_ref()?;
        let counts = match kind {
            ClipKind::Video => &l.video,
            ClipKind::Audio => &l.audio,
        };
        let total = match kind {
            ClipKind::Video => video_total,
            ClipKind::Audio => audio_total,
        };
        if counts.is_empty() || counts.iter().sum::<u32>() != total {
            return None;
        }
        // видео пишется от НИЖНЕЙ дорожки к верхней — карта разворачивается
        let mut lanes: Vec<u32> = Vec::with_capacity(total as usize);
        let order: Vec<u32> = match kind {
            ClipKind::Video => (0..counts.len() as u32).rev().collect(),
            ClipKind::Audio => (0..counts.len() as u32).collect(),
        };
        for lane in order {
            for _ in 0..counts[lane as usize] {
                lanes.push(lane);
            }
        }
        Some(lanes)
    };
    let video_lanes_plan = layer_plan(ClipKind::Video);
    let audio_lanes_plan = layer_plan(ClipKind::Audio);
    if kt.as_ref().and_then(|k| k.layers.as_ref()).is_some()
        && (video_lanes_plan.is_none() || audio_lanes_plan.is_none())
    {
        notices.push("layer map does not match the tracks — tracks kept separate".into());
    }
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
                let idx = seen_video;
                seen_video += 1;
                match &video_lanes_plan {
                    Some(plan) => plan[idx as usize],
                    None => video_total - seen_video,
                }
            }
            ClipKind::Audio => {
                let idx = seen_audio;
                seen_audio += 1;
                match &audio_lanes_plan {
                    Some(plan) => plan[idx as usize],
                    None => seen_audio - 1,
                }
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
                    // переход перед Gap — фейд-аут предыдущего клипа (Gap =
                    // пустота; эталон трактует край дорожки так же)
                    if let Some((in_off, out_off, (ttype, mut meta, extra))) = pending_transition.take()
                        && let Some(a) = prev_clip
                    {
                        let id = p.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
                        let (kind, audio, auto) = take_transition_kt(&mut meta, &mut notices);
                        retained.transitions.insert((Some(a), None), (ttype, meta, extra));
                        p.transitions.push(TransitionEvent {
                            id: TransitionId(id),
                            a: Some(a),
                            b: None,
                            in_offset: in_off,
                            out_offset: out_off,
                            kind,
                            audio,
                            auto,
                        });
                    }
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
                    let dur = clip_duration(c).ok_or_else(|| {
                        Error::UnsupportedSchema(
                            "Clip without source_range and without available_range".into(),
                        )
                    })?;
                    let (ev, ex) = clip_from_otio(
                        c,
                        kind,
                        lane,
                        cursor,
                        dur,
                        is_ours,
                        file_v,
                        &mut next_clip_id,
                        &mut media_by_url,
                        &mut next_media_id,
                        base_dir,
                        &mut p,
                        &mut notices,
                        &mut bad_auto,
                    );
                    // переход перед клипом: с живым соседом слева —
                    // кроссфейд, без него (край дорожки или Gap) — фейд-ин
                    // (Р.25; эталон трактует так же — dissolve с Gap)
                    if let Some((in_off, out_off, (ttype, mut meta, extra))) = pending_transition.take() {
                        let id = p.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
                        let (kind, audio, auto) = take_transition_kt(&mut meta, &mut notices);
                        if ttype != "SMPTE_Dissolve" {
                            notices.push(format!("transition '{ttype}' imported as crossfade"));
                        }
                        retained.transitions.insert((prev_clip, Some(ev.id)), (ttype, meta, extra));
                        p.transitions.push(TransitionEvent {
                            id: TransitionId(id),
                            a: prev_clip,
                            b: Some(ev.id),
                            in_offset: in_off,
                            out_offset: out_off,
                            kind,
                            audio,
                            auto,
                        });
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
                    // переход перед чужим плейсхолдером: рендерить его пару
                    // мы не можем, но фейд-аут предыдущего клипа честен
                    if let Some((in_off, out_off, (ttype, mut meta, extra))) = pending_transition.take() {
                        if let Some(a) = prev_clip {
                            let id = p.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
                            let (kind, audio, auto) = take_transition_kt(&mut meta, &mut notices);
                            retained.transitions.insert((Some(a), None), (ttype, meta, extra));
                            p.transitions.push(TransitionEvent {
                                id: TransitionId(id),
                                a: Some(a),
                                b: None,
                                in_offset: in_off,
                                out_offset: out_off,
                                kind,
                                audio,
                                auto,
                            });
                        } else {
                            notices.push("transition between foreign objects was dropped".into());
                        }
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
        // переход последним ребёнком дорожки — фейд-аут последнего клипа
        if let Some((in_off, out_off, (ttype, mut meta, extra))) = pending_transition.take() {
            if let Some(a) = prev_clip {
                let id = p.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
                let (kind, audio, auto) = take_transition_kt(&mut meta, &mut notices);
                retained.transitions.insert((Some(a), None), (ttype, meta, extra));
                p.transitions.push(TransitionEvent {
                    id: TransitionId(id),
                    a: Some(a),
                    b: None,
                    in_offset: in_off,
                    out_offset: out_off,
                    kind,
                    audio,
                    auto,
                });
            } else {
                notices.push("transition on an empty track was dropped".into());
            }
        }
        // монтажная длина: хвостовой Gap чужого файла (Р.25) — конец
        // дорожки правее последнего клипа задаёт длину таймлайна
        if cursor > p.timeline_end.unwrap_or(Tick::ZERO) {
            p.timeline_end = Some(cursor);
        }
        retained.tracks.insert((kind, lane), tx);
    }

    // конверт kt: транспорт/ui/бин/морферы/камеры
    if let Some(kt) = kt {
        // неизвестные kt-поля будущих версий — passthrough (Р.29)
        retained.timeline_kt_extra = kt.extra.clone();
        if let Some(tr) = kt.transport {
            p.region = tr.region.and_then(|(s, e)| {
                kt_model::Region::normalized(Tick(s), Tick(e))
            });
            p.loop_enabled = tr.loop_enabled;
        }
        // порядок строк: своё поле, а из ui.lane_order — только апгрейд
        // старого файла (односторонний, поле больше не пишется)
        p.areas = kt.areas;
        p.lane_order = kt.lane_order;
        if p.lane_order.is_empty()
            && let Some(u) = &kt.ui
        {
            p.lane_order = u.lane_order.clone();
        }
        if let Some(mut ui) = kt.ui {
            // имена дорожек v/a приезжают из Track.name; morph/camera — из ui
            let mut names = p.ui.take().map(|u| u.lane_names).unwrap_or_default();
            ui.lane_names.retain(|(k, _, _)| {
                matches!(k, kt_model::LaneKind::Morph | kt_model::LaneKind::Camera)
            });
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
        // морферы файла ДО v4 — прежняя семантика: патч на цели целиком,
        // время блока не действует (Р.35, односторонний апгрейд)
        if file_v < crate::meta::KT_META_V4 {
            for m in &mut p.morphers {
                kt_model::migrate_legacy_morpher(m);
            }
        }
        p.camera_lanes = kt.camera_lanes;
        p.cameras = kt.cameras;
        // кейфреймы камер старого домена → общие кривые (Р.34 К3)
        for c in &mut p.cameras {
            kt_model::migrate_legacy_camera(c);
        }
        if let Some(end) = kt.end {
            p.timeline_end = Some(Tick(end));
        }
        // число дорожек нашего проекта (пустые в файл не пишутся, Р.25);
        // normalize() ниже поднимет его, если клипы требуют больше
        if let Some(l) = &kt.layers {
            p.overlap_layers = !l.off;
        }
        if let Some((v, a)) = kt.lanes {
            p.video_lanes = p.video_lanes.max(v);
            p.audio_lanes = p.audio_lanes.max(a);
        }
    }

    // детерминированный порядок независимо от порядка обхода дорожек
    // (низ-вверх для видео): save→load→save стабилен, PartialEq честен
    p.clips.sort_by_key(|c| c.id);
    p.foreign.sort_by_key(|f| f.key);
    p.media.sort_by_key(|m| m.id);
    if let Some(u) = &mut p.ui {
        // порядок независим от обхода дорожек (видео идёт низ-вверх)
        u.lane_names.sort_by_key(|(k, l, _)| (*k as u8, *l));
    }
    // монтажная длина хранится, ТОЛЬКО когда она больше материала: иначе
    // наш же файл после круга обрастал бы полем end (t151 байт-стабильность)
    let material = p.clips.iter().map(|c| c.start + c.duration).max().unwrap_or(Tick::ZERO);
    if p.timeline_end.is_some_and(|e| e <= material) {
        p.timeline_end = None;
    }
    p.normalize();
    p.validate().map_err(|e| Error::UnsupportedSchema(format!("invalid timeline: {e}")))?;
    if let Some(first) = bad_auto.first() {
        return Err(Error::UnsupportedSchema(first.clone()));
    }
    Ok(ProjectBundle { project: p, retained, notices })
}

/// Наши поля перехода из его metadata: вид и кривая аудио. Из конверта они
/// ВЫРЕЗАЮТСЯ — обратно их пишет писатель из модели, иначе при смене вида
/// в файл уехало бы устаревшее значение. Пустой конверт «kt» убирается
/// целиком: чужой файл без наших ключей обязан остаться байт-стабильным.
fn take_transition_kt(
    meta: &mut Map<String, Value>,
    notices: &mut Vec<Notice>,
) -> (TransitionKind, AudioCurve, Vec<kt_model::ParamCurve>) {
    let Some(Value::Object(kt)) = meta.get_mut(crate::meta::KT_NS) else {
        return Default::default();
    };
    let kind = match kt.shift_remove("kind") {
        Some(v) => serde_json::from_value(v).unwrap_or_else(|e| {
            notices.push(format!("unreadable transition kind ({e}), kept as a crossfade"));
            TransitionKind::Crossfade
        }),
        None => TransitionKind::Crossfade,
    };
    let audio = kt
        .shift_remove("audio")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    // кривые параметров вида (Р.34 К4) — тот же конверт
    let auto: Vec<kt_model::ParamCurve> = match kt.shift_remove("auto") {
        Some(v) => serde_json::from_value(v).unwrap_or_else(|e| {
            notices.push(format!("unreadable transition curves ({e})"));
            Vec::new()
        }),
        None => Vec::new(),
    };
    if kt.is_empty() {
        meta.shift_remove(crate::meta::KT_NS);
    }
    (kind, audio, auto)
}

/// Кривые файла ДО v3: ключи лежат в долях длины события (домен Р.30),
/// переводим их в тики материала по геометрии клипа. Дробное значение в
/// новую схему не парсится вовсе, поэтому разбираем старую форму явной
/// структурой — так же, как читаются прочие легаси-формы (метки).
fn curves_from_progress(
    v: &serde_json::Value,
    source_in: Tick,
    duration: Tick,
    reversed: bool,
) -> Vec<kt_model::ParamCurve> {
    #[derive(serde::Deserialize)]
    struct LegacyKey {
        t: f64,
        v: f64,
    }
    #[derive(serde::Deserialize)]
    struct LegacyCurve {
        param: String,
        #[serde(default = "yes")]
        ease: bool,
        keys: Vec<LegacyKey>,
    }
    fn yes() -> bool {
        true
    }
    let legacy: Vec<LegacyCurve> = serde_json::from_value(v.clone()).unwrap_or_default();
    legacy
        .into_iter()
        .map(|c| kt_model::ParamCurve {
            param: c.param,
            ease: c.ease,
            keys: c
                .keys
                .into_iter()
                .map(|k| kt_model::AutoKey {
                    // старый домен видео-кривых считался по прогрессу
                    // ТАЙМЛАЙНА — у реверсированного клипа он зеркален
                    // материалу
                    t: kt_model::material_from_progress(k.t, source_in, duration, reversed),
                    v: k.v,
                })
                .collect(),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn clip_from_otio(
    c: &Clip,
    kind: ClipKind,
    lane: u32,
    start: Tick,
    dur: Tick,
    is_ours: bool,
    file_v: u32,
    next_clip_id: &mut u64,
    media_by_url: &mut HashMap<String, MediaId>,
    next_media_id: &mut u64,
    base_dir: Option<&Path>,
    p: &mut Project,
    notices: &mut Vec<Notice>,
    bad_auto: &mut Vec<String>,
) -> (ClipEvent, ClipExtra) {
    let meta: Option<KtClipMeta> = kt_get(&c.metadata);
    let url = match c.active_media() {
        Some(MediaRefNode::External(er)) => er.target_url.clone(),
        _ => String::new(),
    };
    let media = match &meta {
        Some(m) if is_ours => MediaId(m.media),
        // клип без ссылки (Missing/null) не делит источник с другими такими
        // же: общего материала у них нет, дедуп по пустому URL склеил бы
        // независимые клипы в одну запись бина
        _ if url.is_empty() => {
            let id = MediaId(*next_media_id);
            *next_media_id += 1;
            id
        }
        _ => *media_by_url.entry(url.clone()).or_insert_with(|| {
            let id = MediaId(*next_media_id);
            *next_media_id += 1;
            id
        }),
    };
    if !p.media.iter().any(|m| m.id == media) {
        p.media.push(MediaRef { id: media, path: resolve_url(base_dir, &url) });
    }
    let id = match &meta {
        Some(m) if is_ours => ClipId(m.id),
        _ => {
            let id = ClipId(*next_clip_id);
            *next_clip_id += 1;
            id
        }
    };

    // эффекты: kt:* → цепочка, LTW ±1 → reversed/no-op, прочее → foreign
    let mut fx = Vec::new();
    let mut fx_auto: Vec<Option<serde_json::Value>> = Vec::new();
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
        media_refs: c.media_references.clone(),
        active_key: c.active_media_reference_key.clone(),
        kt_extra: meta.as_ref().map(|m| m.extra.clone()).unwrap_or_default(),
        ..Default::default()
    };
    for (i, e) in c.effects.iter().enumerate() {
        match e {
            EffectNode::Effect(eff) if eff.effect_name.starts_with("kt:") => {
                match kt_get::<kt_model::FxInstance>(&eff.metadata) {
                    Some(inst) => {
                        // автоматизация: ключ "auto" рядом с параметрами.
                        // Разбираем ПОСЛЕ цикла: домен ключей у файлов до
                        // v3 зависит от source_in и reversed, а реверс
                        // приезжает отдельным эффектом ниже
                        fx.push(inst);
                        fx_auto.push(
                            eff.metadata
                                .get(crate::meta::KT_NS)
                                .and_then(|kt| kt.get("auto"))
                                .cloned(),
                        );
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

    // СТАРТОВЫЙ ТАЙМКОД МАТЕРИАЛА (п.180). В OTIO `source_range` живёт в
    // системе координат самого материала — той же, что `available_range`.
    // У вещательных и киношных файлов та начинается не с нуля, а со
    // вшитого таймкода (у Sol Levante — ровно час, 86160 кадров). Наш
    // `source_in` — смещение от НУЛЕВОГО кадра файла, поэтому начало
    // available_range надо вычесть; иначе движок уезжает на час в
    // 162-секундный файл и рендерит пустоту.
    let origin = c.active_available_range().map(|r| r.start_time.to_ticks().0).unwrap_or(0);
    let src_in = c
        .source_range
        .as_ref()
        .map(|r| r.start_time.to_ticks().0 - origin)
        // без source_range клип берёт материал целиком — от его начала
        .unwrap_or(0);
    let src_in = Tick(src_in.max(0));
    // домен ключей: v3+ — тики материала как есть, старее — доли длины
    // события, которые переводим по геометрии ЭТОГО клипа (Р.33)
    let mut fx_auto: Vec<Vec<kt_model::ParamCurve>> = fx_auto
        .iter()
        .map(|raw| match raw {
            None => Vec::new(),
            Some(v) if file_v >= crate::meta::KT_META_V3 => {
                // битые кривые НЕ проглатываем: молча потерять анимацию
                // хуже, чем отказать (правило Р.24 «не ломать чужое и не
                // терять своё»)
                serde_json::from_value(v.clone()).unwrap_or_else(|e| {
                    bad_auto.push(format!("clip {}: unreadable automation ({e})", id.0));
                    Vec::new()
                })
            }
            Some(v) => curves_from_progress(v, src_in, dur, reversed),
        })
        .collect();
    while fx_auto.last().is_some_and(|v| v.is_empty()) {
        fx_auto.pop(); // хвост без кривых не сериализуем (форма модели)
    }
    let mut ev = ClipEvent {
        id,
        link_group: meta.as_ref().and_then(|m| m.link_group).map(GroupId),
        kind,
        lane,
        start,
        duration: dur,
        source_in: src_in,
        media,
        gain_db: meta.as_ref().map(|m| m.gain).unwrap_or(0.0),
        opacity: meta.as_ref().map(|m| m.opacity).unwrap_or(1.0),
        fx_off: meta.as_ref().is_some_and(|m| m.fx_off),
        reversed,
        fx,
        fx_auto,
        fx_bypass: bypass,
    };
    // кейфреймы slowed&reverb старого образца — в общий механизм (Р.33):
    // собственного домена у них больше нет
    kt_model::migrate_legacy_slowed(&mut ev);
    kt_model::migrate_legacy_cammotion(&mut ev);
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

/// Длительность клипа: `source_range`, а без него — весь `available_range`
/// активной ссылки (спека OTIO; так живёт Clip-004 в simple_cut.otio
/// апстрима). Соответствующий `source_in` берётся в `clip_from_otio`.
fn clip_duration(c: &Clip) -> Option<Tick> {
    item_duration(&c.source_range).or_else(|| item_duration(&c.active_available_range()))
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

/// Эвристика fps чужого файла: самый частый rate среди source_range,
/// приведённый к сетке 90 кГц; иначе 30.
///
/// **NTSC-ряд** (п.178): 23.976/29.97/59.94 на сетку 90 кГц не ложатся
/// (90000/23.976 = 3753.75), и раньше такой файл молча становился
/// проектом на 30 fps — при том что настоящая частота ближе к 24.
/// Теперь дробная частота округляется до ближайшей целой, которая делит
/// 90000: 23.976→24, 29.97→30, 59.94→60. Остаточный дрейф 0.1 % (pulldown)
/// никуда не девается — позиции склеек хранятся в ТИКАХ и точны, «плывёт»
/// только сетка кадров.
fn guess_fps(tl: &Timeline) -> u32 {
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for child in &tl.tracks.children {
        if let Composable::Track(t) = child {
            for item in &t.children {
                if let Composable::Clip(c) = item
                    && let Some(r) = &c.source_range
                    && let Some(fps) = grid_fps(r.duration.rate)
                {
                    *counts.entry(fps).or_default() += 1;
                }
            }
        }
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(r, _)| r).unwrap_or(30)
}

/// Частота кадров, ложащаяся на сетку 90 кГц, ближайшая к `rate`.
/// None — если не нашлось (слишком большая/нулевая или далеко от целой).
pub fn grid_fps(rate: f64) -> Option<u32> {
    if !(rate.is_finite() && rate > 0.0 && rate <= 240.0) {
        return None;
    }
    let r = rate.round();
    // дальше половины кадра от целой — это не «NTSC-версия» целой частоты
    if (rate - r).abs() > 0.5 {
        return None;
    }
    let r = r as u32;
    (r > 0 && kt_time::TICK_HZ % r as i64 == 0).then_some(r)
}
