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
    KT_NS, KtBinMedia, KtClipMeta, KtSettings, KtTimelineMeta, KtTransport, kt_get, kt_get_checked,
    kt_set,
};
use crate::schema::{
    Clip, Composable, Effect, EffectNode, ExternalReference, Gap, GeneratorReference,
    LinearTimeWarp, Marker, MarkerNode, MediaRefNode, RationalTime, RawObject, Stack, TimeRange,
    Timeline, Track,
};
use crate::{DEFAULT_MEDIA_KEY, Error};
use kt_model::{
    AudioCurve, ClipEvent, ClipId, ClipKind, ForeignEvent, GroupId, MediaId, MediaRef, Project,
    ProjectSettings, Sequence, TransitionEvent, TransitionId, TransitionKind, UiState,
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

/// Ссылки на материал — свойство МЕДИА, а не клипа. Ключ `Retained.media`.
///
/// Зачем отдельно от `ClipExtra` (Р.49, п.356). `available_range` описывает
/// файл: где у него начало в его собственных координатах. У вещательных и
/// киношных файлов оно не в нуле, а во вшитом таймкоде (у Sol Levante — ровно
/// час). Клип, РОЖДЁННЫЙ в редакторе (правая половина разреза, кусок bake,
/// вставка, событие из бина), своего `ClipExtra` не имеет — и на записи его
/// `source_in` уходил в файл без этого часа, а при следующей загрузке час
/// вычитался ещё раз. Медиа-уровень чинит всю эту половину разом.
/// Чужое у метки: то, чего нет в `kt_model::Marker`.
#[derive(Debug, Clone, Default)]
pub struct MarkerExtra {
    pub comment: String,
    pub metadata: Map<String, Value>,
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default)]
pub struct MediaRefs {
    pub media_refs: IndexMap<String, Option<MediaRefNode>>,
    pub active_key: String,
}

#[derive(Debug, Clone)]
pub struct TrackExtra {
    pub name_foreign: Option<String>,
    pub extra: Map<String, Value>,
    pub metadata: Map<String, Value>,
    /// extra/metadata gap'ов по порядковому номеру в дорожке.
    pub gaps: Vec<(usize, ExtraMeta)>,
    /// Чужое, что висит на САМОЙ дорожке (Р.49, п.358). Своих дорожечных
    /// эффектов и маркеров у нас нет, а у соседей есть — и круг их терял.
    pub effects: Vec<EffectNode>,
    pub markers: Vec<MarkerNode>,
    /// Выключенная дорожка. Мы такого не умеем и не выдумываем: значение
    /// возвращается в файл как есть, а пользователь получает notice —
    /// иначе он смотрит на дорожку, которой в чужом редакторе не слышно.
    pub enabled: bool,
}

impl Default for TrackExtra {
    fn default() -> TrackExtra {
        TrackExtra {
            name_foreign: None,
            extra: Map::new(),
            metadata: Map::new(),
            gaps: Vec::new(),
            effects: Vec::new(),
            markers: Vec::new(),
            enabled: true,
        }
    }
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
    /// Ссылки на материал по МЕДИА — общие для всех его клипов (п.356).
    pub media: HashMap<kt_model::MediaId, MediaRefs>,
    /// По паре клипов стыка.
    /// Ключ — пара концов перехода; у фейда (Р.25) один конец None.
    pub transitions: HashMap<(Option<ClipId>, Option<ClipId>), TransitionExtra>,
    /// Сырьё foreign-плейсхолдеров по ForeignEvent.key.
    pub foreign: HashMap<u64, RawObject>,
    /// Чужие маркеры корневого Stack (наши — из модели).
    pub stack_markers_foreign: Vec<MarkerNode>,
    /// Чужое у НАШИХ меток корневого Stack: комментарий, metadata, extra
    /// (Р.49, п.358). Модель их не знает — метка у нас это время, имя и
    /// цвет, — а круг их терял: чужой редактор клал комментарий и
    /// собственный неймспейс, и после нашего сохранения они исчезали.
    /// Ключ — порядковый номер метки в НАШЕМ списке на момент чтения.
    pub stack_marker_extras: Vec<(usize, MarkerExtra)>,
    /// Чужие объекты на уровне ДЕТЕЙ корневого Stack — не `Track`.
    ///
    /// До Р.50 они попадали в `foreign` по ключу, но писателем не
    /// доставались ОТТУДА никогда: сырьё возвращают только
    /// `ForeignEvent`-плейсхолдеры, живущие НА дорожках, а у корневого
    /// ребёнка дорожки нет. Круг терял такой объект молча. Храним его с
    /// ПОЗИЦИЕЙ среди детей и возвращаем на неё.
    pub root_foreign: Vec<(usize, RawObject)>,
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
    // Содержимое корня пишется ТЕМ ЖЕ кодом, что содержимое вложенной
    // ленты (Р.50, п.382): раньше это были три замыкания, намертво
    // привязанные к `p`.
    let timeline_end = p.timeline_end.unwrap_or(Tick::ZERO);
    let children = reel_children(
        &ReelWrite {
            clips: &p.clips,
            foreign: &p.foreign,
            transitions: &p.transitions,
            video_lanes: p.video_lanes,
            audio_lanes: p.audio_lanes,
            timeline_end,
            lane_names: p.ui.as_ref().map(|u| u.lane_names.as_slice()).unwrap_or(&[]),
            retained,
        },
        p,
        base_dir,
        0,
    );
    // чужие корневые дети возвращаются на СВОИ позиции (Р.50): раньше они
    // уезжали в `foreign` по ключу, который писателю не достаётся
    let mut children = children;
    for (pos, raw) in &retained.root_foreign {
        let at = (*pos).min(children.len());
        children.insert(at, Composable::Unknown(raw.clone()));
    }

    // метки — на корневом Stack (наши + чужие passthrough)
    let mut markers: Vec<MarkerNode> = p
        .markers
        .iter()
        .enumerate()
        .map(|(i, m)| {
            // чужое у метки возвращается на своё место (п.358)
            let ex = retained
                .stack_marker_extras
                .iter()
                .find(|(n, _)| *n == i)
                .map(|(_, e)| e.clone())
                .unwrap_or_default();
            MarkerNode::Marker(Marker {
                name: m.name.clone(),
                color: m.color.clone().unwrap_or_else(Marker::default_color),
                marked_range: range(m.t, m.duration),
                comment: ex.comment,
                metadata: ex.metadata,
                extra: ex.extra,
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
    // «использовано» считается ПО ВСЕМУ ДЕРЕВУ (Р.50): файл, стоящий лишь
    // внутри ленты, попал бы и в bin_media, и в клипы этой ленты — на
    // чтении получился бы дубль адреса и отказ загрузки.
    let used: std::collections::HashSet<MediaId> = p
        .clips
        .iter()
        .chain(p.sequences.iter().flat_map(|s| s.clips.iter()))
        .map(|c| c.media)
        .collect();
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
            let has_off = p.clips.iter().any(|c| c.fx_off) || p.morphers.iter().any(|m| !m.enabled);
            // вид перехода и кривая аудио — тоже v3: старая сборка
            // прочитала бы вайп как кроссфейд и молча испортила бы монтаж
            let has_tr = p.transitions.iter().any(|t| {
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
            // кривые слоя (Р.38 п.13) и его сила с фейдами (п.12): старая
            // сборка отрендерила бы слой с постоянными параметрами вместо
            // анимированных, а «слой на 30 %» — целиком
            let has_morph_v5 = p.morphers.iter().any(|m| {
                m.mixes()
                    || m.patch
                        .video_auto
                        .iter()
                        .chain(m.patch.audio_auto.iter())
                        .any(|a| !a.is_empty())
            });
            // переходы Р.51 (инверсия, наложенные) — по ВСЕМ лентам
            // документа: старая сборка сыграла бы переход неинвертированным
            // (а наложенный — жёстким срезом) и сохранила бы конверт
            // нетронутым — порча незаметнее некуда
            let v7_reel = |reel: &dyn kt_model::Reel| {
                reel.transitions().iter().any(|t| t.invert || t.overlap_pair(reel).is_some())
            };
            let has_v7 =
                v7_reel(p) || p.sequences.iter().any(|s| v7_reel(s as &dyn kt_model::Reel));
            // вложенные ленты (Р.50): старая сборка прочла бы вложение
            // серым плейсхолдером (п.158) и при первой перезаписи вернула
            // бы живой монтаж в файл мёртвым сырьём
            if has_v7 {
                crate::meta::KT_META_V7
            } else if !p.sequences.is_empty() {
                crate::meta::KT_META_V6
            } else if has_morph_v5 {
                crate::meta::KT_META_V5
            } else if has_layer {
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
            // дефолт в файл не пишется (skip_serializing_if)
            pitch_engine: (p.settings.pitch_engine != Default::default())
                .then_some(p.settings.pitch_engine),
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
            let lo = |k, l| {
                layers_of(&p.clips, &p.foreign, retained.tracks.contains_key(&(k, l)), k, l) as u32
            };
            let v: Vec<u32> = (0..p.video_lanes).map(|l| lo(ClipKind::Video, l)).collect();
            let a: Vec<u32> = (0..p.audio_lanes).map(|l| lo(ClipKind::Audio, l)).collect();
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
        sequences: {
            let mut v: Vec<crate::meta::KtSequence> = p
                .sequences
                .iter()
                .map(|s| {
                    let mut morphers = s.morphers.clone();
                    morphers.sort_by_key(|m| m.id);
                    let mut cameras = s.cameras.clone();
                    cameras.sort_by_key(|c| c.id);
                    crate::meta::KtSequence {
                        id: s.id.0,
                        name: s.name.clone(),
                        video_lanes: s.video_lanes,
                        audio_lanes: s.audio_lanes,
                        morph_lanes: s.morph_lanes,
                        camera_lanes: s.camera_lanes,
                        lane_order: s.lane_order.clone(),
                        morphers,
                        cameras,
                        overlap_layers: s.overlap_layers,
                        layers: {
                            let lo = |k, l| layers_of(&s.clips, &[], false, k, l) as u32;
                            let v: Vec<u32> =
                                (0..s.video_lanes).map(|l| lo(ClipKind::Video, l)).collect();
                            let a: Vec<u32> =
                                (0..s.audio_lanes).map(|l| lo(ClipKind::Audio, l)).collect();
                            // как у корня: пишем, только когда слой не один
                            let multi = v.iter().chain(a.iter()).any(|n| *n > 1);
                            (multi || !s.overlap_layers).then(|| crate::meta::KtLayers {
                                off: !s.overlap_layers,
                                video: if multi { v } else { Vec::new() },
                                audio: if multi { a } else { Vec::new() },
                            })
                        },
                        overlap_transitions: overlap_transition_list(s),
                    }
                })
                .collect();
            v.sort_by_key(|s| s.id);
            v
        },
        overlap_transitions: overlap_transition_list(p),
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

/// Сколько OTIO-треков занимает одна НАША дорожка (Р.28, расслоение).
///
/// Дорожка без содержимого, которой не было в исходном файле, не пишется
/// вовсе (Р.25) — иначе карта слоёв не сойдётся с файлом и при чтении будет
/// отброшена целиком.
fn layers_of(
    clips: &[ClipEvent],
    foreign: &[ForeignEvent],
    known: bool,
    kind: ClipKind,
    lane: u32,
) -> usize {
    let has_content = clips.iter().any(|c| c.kind == kind && c.lane == lane)
        || foreign.iter().any(|f| f.kind == kind && f.lane == lane);
    if !known && !has_content {
        return 0;
    }
    let mut spans: Vec<(i64, i64)> = clips
        .iter()
        .filter(|c| c.kind == kind && c.lane == lane)
        .map(|c| (c.start.0, (c.start + c.duration).0))
        .collect();
    spans.sort_unstable();
    let mut ends: Vec<i64> = Vec::new();
    for (s, e) in spans {
        match ends.iter_mut().find(|end| **end <= s) {
            Some(end) => *end = e,
            None => ends.push(e),
        }
    }
    ends.len().max(1)
}

/// Эффекты события наружу: реверс — LTW(-1) первым, затем наша цепочка с
/// кривыми в своём конверте, чужие — по исходным индексам. ОДИН код на
/// обычное событие и на вложение (у вложения чужих эффектов не бывает).
fn clip_effects(c: &ClipEvent, foreign: &[(usize, EffectNode)]) -> Vec<EffectNode> {
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
    for (i, e) in foreign {
        let at = (*i).min(effects.len());
        effects.insert(at, e.clone());
    }
    effects
}

/// Содержимое ОДНОЙ ленты (Р.50, п.382): корень проекта и вложенная
/// секвенция пишутся ТЕМ ЖЕ кодом.
///
/// До Р.50 это были три замыкания внутри `project_to_timeline`, намертво
/// привязанные к `&Project`. Ленты у нас две разновидности, а правило
/// расслоения, гэпов, переходов и имён дорожек — одно; выписать его второй
/// раз значило бы завести вторую правду о формате.
/// Чтение наложенных переходов ленты из её конверта (Р.51, п.399):
/// возвращает события и замечания. Запись с неизвестным клипом — notice
/// и пропуск (паттерн KtLayers: не сошлось — чтение как есть, не порча);
/// геометрию пары дальше проверит validate.
fn read_overlap_transitions(
    list: &[crate::meta::KtOverlapTransition],
    clips: &[ClipEvent],
    next_id: &mut u64,
) -> (Vec<TransitionEvent>, Vec<Notice>) {
    let mut out = Vec::new();
    let mut notices = Vec::new();
    for ot in list {
        let have = |id: u64| clips.iter().any(|c| c.id.0 == id);
        if !have(ot.a) || !have(ot.b) {
            notices.push(format!(
                "overlap transition {}->{} references unknown clips, dropped",
                ot.a, ot.b
            ));
            continue;
        }
        *next_id += 1;
        out.push(TransitionEvent {
            id: TransitionId(*next_id),
            a: Some(ClipId(ot.a)),
            b: Some(ClipId(ot.b)),
            in_offset: Tick::ZERO,
            out_offset: Tick::ZERO,
            kind: ot.kind.clone(),
            audio: ot.audio,
            auto: ot.auto.clone(),
            invert: ot.invert,
        });
    }
    (out, notices)
}

/// Наложенные переходы ленты — в её конверт (Р.51, п.399): пара живёт в
/// разных соседних Track и ребёнком последовательного трека невыразима.
/// Сортировка по паре — детерминизм записи (т.144).
fn overlap_transition_list(reel: &dyn kt_model::Reel) -> Vec<crate::meta::KtOverlapTransition> {
    let mut v: Vec<crate::meta::KtOverlapTransition> = reel
        .transitions()
        .iter()
        .filter(|t| t.overlap_pair(reel).is_some())
        .map(|t| crate::meta::KtOverlapTransition {
            a: t.a.map(|c| c.0).unwrap_or(0),
            b: t.b.map(|c| c.0).unwrap_or(0),
            kind: t.kind.clone(),
            audio: t.audio,
            auto: t.auto.clone(),
            invert: t.invert,
        })
        .collect();
    v.sort_by_key(|t| (t.a, t.b));
    v
}

struct ReelWrite<'a> {
    clips: &'a [ClipEvent],
    foreign: &'a [ForeignEvent],
    transitions: &'a [TransitionEvent],
    video_lanes: u32,
    audio_lanes: u32,
    timeline_end: Tick,
    /// Имена дорожек. У вложенной ленты своего `ui` нет — пусто.
    lane_names: &'a [(kt_model::LaneKind, u32, String)],
    /// Чужое сырьё. У наших лент его не бывает — пустой `Retained`.
    retained: &'a Retained,
}

fn reel_children(
    r: &ReelWrite<'_>,
    p: &Project,
    base_dir: Option<&Path>,
    depth: usize,
) -> Vec<Composable> {
    let url_of = |path: &str| -> String {
        match base_dir {
            Some(dir) => crate::url::media_target_url(dir, path),
            None => path.replace('\\', "/"),
        }
    };
    let media_path = |id: MediaId| {
        p.media.iter().find(|m| m.id == id).map(|m| m.path.clone()).unwrap_or_default()
    };
    // переход пишется ПЕРЕД своим incoming-клипом; фейд-аут (правого клипа
    // нет) — ПОСЛЕ своего outgoing (Р.25). НАЛОЖЕННЫЙ переход (Р.51) в
    // детей трека не пишется вовсе: его пара лежит в разных Track — он
    // едет в конверте ленты (`overlap_transitions`).
    let staggered = |t: &TransitionEvent| -> bool {
        let find = |id: Option<ClipId>| id.and_then(|i| r.clips.iter().find(|c| c.id == i));
        let (Some(a), Some(b)) = (find(t.a), find(t.b)) else { return false };
        let (ae, be) = (a.start + a.duration, b.start + b.duration);
        a.kind == b.kind && a.lane == b.lane && a.start < b.start && b.start < ae && ae < be
    };
    let trans_before: HashMap<ClipId, &TransitionEvent> =
        r.transitions.iter().filter(|t| !staggered(t)).filter_map(|t| Some((t.b?, t))).collect();
    let trans_after: HashMap<ClipId, &TransitionEvent> = r
        .transitions
        .iter()
        .filter(|t| t.b.is_none() && !staggered(t))
        .filter_map(|t| Some((t.a?, t)))
        .collect();

    // Переслаивание (Р.28): в OTIO позиция клипа определяется ПОРЯДКОМ детей
    // трека, поэтому перекрытые клипы одной нашей дорожки невыразимы в одном
    // Track — их пришлось бы писать подряд, и чужой редактор увидел бы
    // разъехавшийся монтаж. Раскладываем жадно по слоям: клип идёт в первый
    // слой, где ни с кем не пересекается; каждый слой пишется отдельным
    // Track, а номер НАШЕЙ дорожки едет в `metadata.kt.lane` трека — по нему
    // мы собираем дорожку обратно (решение пользователя Р.28).
    let layers_of = |kind: ClipKind, lane: u32| -> usize {
        layers_of(r.clips, r.foreign, r.retained.tracks.contains_key(&(kind, lane)), kind, lane)
    };
    let layer_of = |c: &ClipEvent, kind: ClipKind, lane: u32| -> usize {
        let mut ends: Vec<i64> = Vec::new();
        let mut mine = 0usize;
        let mut spans: Vec<(i64, i64, ClipId)> = r
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
    let emit_track = |children: &mut Vec<Composable>, kind: ClipKind, lane: u32, layer: usize| {
        // чужие extra/metadata/gap'ы принадлежат ИСХОДНОМУ треку — их несёт
        // слой 0, иначе расслоение продублировало бы их на каждый слой
        let tx = if layer == 0 {
            r.retained.tracks.get(&(kind, lane)).cloned().unwrap_or_default()
        } else {
            TrackExtra::default()
        };
        let known = r.retained.tracks.contains_key(&(kind, lane));
        let has_content = r.clips.iter().any(|c| c.kind == kind && c.lane == lane)
            || r.foreign.iter().any(|f| f.kind == kind && f.lane == lane);
        if !known && !has_content {
            return; // пустая дорожка, которой не было в исходном файле
        }
        let mut items: Vec<(Tick, Tick, Result<&ClipEvent, &ForeignEvent>)> = r
            .clips
            .iter()
            .filter(|c| c.kind == kind && c.lane == lane && layer_of(c, kind, lane) == layer)
            .map(|c| (c.start, c.start + c.duration, Ok(c)))
            .chain(
                r.foreign
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
                        let (ttype, mut meta, extra) = r
                            .retained
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
                            || t.invert
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
                            // умолчание false не пишется (байт-стабильность)
                            if t.invert {
                                e.insert("invert".into(), Value::Bool(true));
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
                        && t.a.is_none_or(|a| r.clips.iter().any(|x| x.id == a))
                    {
                        emit_transition(&mut kids, t);
                    }
                    // Вложение пишется `Stack`-ом ребёнком `Track` с
                    // `source_range` — точно тем же приёмом, каким это
                    // делают Premiere и FCP. Тело инлайнится в КАЖДЫЙ
                    // экземпляр: объектной идентичности в JSON OTIO нет,
                    // и чужой редактор обязан увидеть монтаж, а не ссылку.
                    match p.sequence(c.media) {
                        Some(seq) => {
                            kids.push(Composable::Stack(nest_to_otio(c, seq, p, base_dir, depth)))
                        }
                        None => kids.push(Composable::Clip(clip_to_otio(
                            c,
                            r.retained,
                            &url_of(&media_path(c.media)),
                        ))),
                    }
                    if let Some(t) = trans_after.get(&c.id) {
                        emit_transition(&mut kids, t);
                    }
                }
                Err(f) => {
                    if let Some(raw) = r.retained.foreign.get(&f.key) {
                        kids.push(Composable::Unknown(raw.clone()));
                    }
                }
            }
            cursor = end;
        }
        if r.timeline_end > cursor {
            kids.push(Composable::Gap(Gap {
                name: String::new(),
                source_range: Some(range(Tick::ZERO, r.timeline_end - cursor)),
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
        let name = r
            .lane_names
            .iter()
            .find(|(k, l, _)| *k == lk && *l == lane)
            .map(|(_, _, n)| n.clone())
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
            // чужое у дорожки возвращается на место (п.358); на слоях
            // расслоения его не дублируем — оно принадлежит слою 0
            effects: tx.effects.clone(),
            markers: tx.markers.clone(),
            enabled: tx.enabled,
            metadata: tx.metadata.clone(),
            extra: tx.extra.clone(),
        }));
    };
    // Монтажная длина (Р.25): дорожка короче конца ленты дописывается
    // хвостовым Gap'ом — так чужой файл задаёт длину и так его прочтут
    // обратно. Дорожку, которой в файле не было и в которой ничего нет,
    // не пишем вовсе (число дорожек живёт в metadata.kt).
    // Видео снизу вверх (OTIO: поздний трек поверх), затем аудио по порядку.
    let mut children: Vec<Composable> = Vec::new();
    for lane in (0..r.video_lanes).rev() {
        for layer in 0..layers_of(ClipKind::Video, lane) {
            emit_track(&mut children, ClipKind::Video, lane, layer);
        }
    }
    for lane in 0..r.audio_lanes {
        for layer in 0..layers_of(ClipKind::Audio, lane) {
            emit_track(&mut children, ClipKind::Audio, lane, layer);
        }
    }
    children
}

/// Вложение как `Stack` ребёнком `Track` (Р.50).
///
/// Окно экземпляра — `source_range` стека, ровно как у клипа: OTIO трактует
/// его одинаково, и чужой редактор покажет подрезанное вложение правильно
/// без единой нашей строки.
fn nest_to_otio(
    c: &ClipEvent,
    seq: &Sequence,
    p: &Project,
    base_dir: Option<&Path>,
    depth: usize,
) -> Stack {
    let empty = Retained::default();
    let children = if depth + 1 > kt_model::MAX_NEST_DEPTH {
        // глубже предела не пишем ВОВСЕ: файл, который мы не сможем
        // прочитать обратно, хуже отсутствующего вложения. Модель такого
        // проекта не пропустит (validate), так что сюда попасть нечем —
        // это страховка на случай ручной правки .otio.
        Vec::new()
    } else {
        reel_children(
            &ReelWrite {
                clips: &seq.clips,
                foreign: &[],
                transitions: &seq.transitions,
                video_lanes: seq.video_lanes,
                audio_lanes: seq.audio_lanes,
                timeline_end: Tick::ZERO,
                lane_names: &[],
                retained: &empty,
            },
            p,
            base_dir,
            depth + 1,
        )
    };
    let mut meta = Map::new();
    kt_set(
        &mut meta,
        &KtClipMeta {
            id: c.id.0,
            media: c.media.0,
            link_group: c.link_group.map(|g| g.0),
            gain: c.gain_db,
            opacity: c.opacity,
            fx_off: c.fx_off,
            extra: Map::new(),
        },
    );
    // адрес ленты — вот по этому ключу экземпляры схлопываются на чтении
    crate::meta::kt_entry_mut(&mut meta).insert("seq".into(), Value::from(seq.id.0));
    Stack {
        name: seq.name.clone(),
        children,
        source_range: Some(range(c.source_in, c.duration)),
        effects: clip_effects(c, &[]),
        markers: vec![],
        enabled: true,
        metadata: meta,
        extra: Map::new(),
    }
}

/// `GeneratorReference` (наш, `kt.<тег>`) → канонический `gen:`-путь.
/// Незнакомый тег или битые параметры — `None` (совместимость только
/// вперёд: заглушка вместо генератора из будущей сборки).
fn generator_ref_path(gr: &GeneratorReference) -> Option<String> {
    let tag = gr
        .generator_kind
        .strip_prefix("kt.")
        .or_else(|| crate::generator_alias(&gr.generator_kind))?;
    let mut fields = gr.parameters.clone();
    fields.insert("kind".into(), Value::String(tag.to_string()));
    let kind: kt_model::GeneratorKind = serde_json::from_value(Value::Object(fields)).ok()?;
    Some(kt_model::format_gen_path(&kind))
}

/// Поля `GeneratorKind` словарём `parameters` (без служебного `kind` —
/// он едет строкой `generator_kind`). Порядок ключей детерминирован
/// порядком полей структуры.
fn generator_parameters(kind: &kt_model::GeneratorKind) -> Map<String, Value> {
    let Ok(Value::Object(mut fields)) = serde_json::to_value(kind) else {
        unreachable!("GeneratorKind serializes to an object");
    };
    fields.shift_remove("kind");
    fields
}

fn clip_to_otio(c: &ClipEvent, retained: &Retained, target_url: &str) -> Clip {
    let mut ex = retained.clips.get(&c.id).cloned().unwrap_or_default();
    // Клип, рождённый в редакторе, конверта не имеет — ссылки на материал
    // берём у МЕДИА (п.356). Иначе потерялся бы `available_range`, а вместе
    // с ним стартовый таймкод файла: `source_in` уезжал бы на час.
    if ex.media_refs.is_empty()
        && let Some(m) = retained.media.get(&c.media)
    {
        ex.media_refs = m.media_refs.clone();
        ex.active_key = m.active_key.clone();
    }
    let effects = clip_effects(c, &ex.effects_foreign);

    let mut media_refs = ex.media_refs.clone();
    let active_key =
        if ex.active_key.is_empty() { DEFAULT_MEDIA_KEY.to_string() } else { ex.active_key };
    if !target_url.is_empty() {
        if let Some(kind) = kt_model::parse_gen_path(target_url) {
            // генератор пишется ПЕРВОКЛАССНОЙ схемой OTIO (Р.52):
            // name/metadata/extra прежнего узла сохраняются, вид и
            // параметры перезаписываются каноном. `available_range`
            // ОБЯЗАТЕЛЕН (контракт схемы — фикстура SMPTE несёт 50 кадров
            // при том, что длительности у генератора не существует):
            // пишем ровно потреблённое ЭТИМ клипом — идемпотентно.
            let mut active = match media_refs.get(&active_key) {
                Some(Some(MediaRefNode::Generator(gr))) => gr.clone(),
                _ => GeneratorReference {
                    name: String::new(),
                    generator_kind: String::new(),
                    available_range: None,
                    available_image_bounds: None,
                    parameters: Map::new(),
                    metadata: Map::new(),
                    extra: Map::new(),
                },
            };
            active.generator_kind = format!("kt.{}", kind.tag());
            active.parameters = generator_parameters(&kind);
            active.available_range = Some(range(Tick(0), c.source_in + c.duration));
            media_refs.insert(active_key.clone(), Some(MediaRefNode::Generator(active)));
        } else {
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
    }

    // начало available_range активной ссылки — тот самый таймкод, который
    // был вычтен на чтении (п.180)
    let media_origin = media_refs
        .get(&active_key)
        .and_then(|n| n.as_ref())
        .and_then(|n| match n {
            MediaRefNode::External(er) => er.available_range,
            MediaRefNode::Missing(mr) => mr.available_range,
            // у генератора начало материала всегда 0 — таймкода файла нет
            MediaRefNode::Generator(_) => None,
            MediaRefNode::Unknown(raw) => {
                raw.0.get("available_range").and_then(|v| serde_json::from_value(v.clone()).ok())
            }
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

    // Маркеры раскладываются по ОКНУ МАТЕРИАЛА (Р.49, п.357). Производное
    // событие наследует конверт родителя ЦЕЛИКОМ — иначе undo вернул бы
    // родителя без маркеров, уехавших к детям. Значит выбирает писатель: в
    // файл идут только те, что пересекают окно этого события. Чужие маркеры
    // неизвестной схемы пропускаются как есть: их диапазон нам не прочесть,
    // а выбрасывать чужое нельзя.
    let win_start = c.source_in.0 + media_origin;
    let win_end = win_start + c.duration.0;
    let markers: Vec<MarkerNode> = ex
        .markers
        .into_iter()
        .filter(|m| match m {
            MarkerNode::Marker(mk) => {
                let a = mk.marked_range.start_time.to_ticks().0;
                let b = a + mk.marked_range.duration.to_ticks().0.max(0);
                // полуинтервал: точечный маркер ровно на конце — уже следующий
                a < win_end && b >= win_start
            }
            MarkerNode::Unknown(_) => true,
        })
        .collect();

    Clip {
        name: ex.name,
        // таймкод материала возвращается на место (п.180): чужой файл
        // обязан прочитаться другим редактором так же, как пришёл
        source_range: Some(range(Tick(win_start), c.duration)),
        media_references: media_refs,
        active_media_reference_key: active_key,
        effects,
        markers,
        enabled: true,
        metadata: meta,
        extra: ex.extra,
    }
}

// ---- Timeline → Project -----------------------------------------------------

pub fn timeline_to_project(tl: &Timeline, base_dir: Option<&Path>) -> Result<ProjectBundle, Error> {
    crate::meta::check_kt_version(tl)?;
    let mut notices: Vec<Notice> = Vec::new();
    // нечитаемые кривые автоматизации: копим и валим загрузку в конце
    let mut bad_auto: Vec<String> = Vec::new();
    // адреса клипов с испорченной НАШЕЙ секцией (п.358)
    let mut bad_kt: Vec<String> = Vec::new();
    // битая kt-секция — ОШИБКА, а не «чужой файл»: молчаливое выбрасывание
    // конверта уносило с собой настройки, дорожки, морферы и раскладку
    let kt: Option<KtTimelineMeta> =
        kt_get_checked(&tl.metadata).map_err(|e| Error::KtMetaBroken(e.to_string()))?;
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
            pitch_engine: s.pitch_engine.unwrap_or_default(),
        })
        .unwrap_or_else(|| {
            notices.push("imported with default project settings".into());
            ProjectSettings {
                fps: guess_fps(tl),
                width: 1920,
                height: 1080,
                sample_rate: 48000,
                pitch_engine: Default::default(),
            }
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
            MarkerNode::Marker(mk) => {
                if !mk.comment.is_empty() || !mk.metadata.is_empty() || !mk.extra.is_empty() {
                    retained.stack_marker_extras.push((
                        p.markers.len(),
                        MarkerExtra {
                            comment: mk.comment.clone(),
                            metadata: mk.metadata.clone(),
                            extra: mk.extra.clone(),
                        },
                    ));
                }
                p.markers.push(kt_model::Marker {
                    t: mk.marked_range.start_time.to_ticks(),
                    duration: Tick(mk.marked_range.duration.to_ticks().0.max(0)),
                    name: mk.name.clone(),
                    color: Some(mk.color.clone()),
                })
            }
            MarkerNode::Unknown(_) => retained.stack_markers_foreign.push(m.clone()),
        }
    }

    // Карта слоёв (Р.28): перекрытые клипы одной НАШЕЙ дорожки лежат в
    // соседних Track'ах — здесь они собираются обратно. Карта берётся из
    // нашего конверта; если она не сходится с файлом (чужой редактор
    // пересобрал треки), она игнорируется и дорожки читаются один к одному.
    let root_layers = kt.as_ref().and_then(|k| k.layers.clone());
    let mut next_clip_id = 1u64;
    let mut next_foreign_key = 1u64;
    let mut track_extras: HashMap<(ClipKind, u32), TrackExtra> = HashMap::new();
    let mut media_by_url: HashMap<String, MediaId> = HashMap::new();
    let mut next_media_id = 1u64;

    let mut root = ReelRead::default();
    {
        let mut cx = ReadCtx {
            base_dir,
            is_ours,
            file_v,
            next_clip_id: &mut next_clip_id,
            next_foreign_key: &mut next_foreign_key,
            next_media_id: &mut next_media_id,
            media_by_url: &mut media_by_url,
            media: &mut p.media,
            retained: &mut retained,
            notices: &mut notices,
            bad_auto: &mut bad_auto,
            bad_kt: &mut bad_kt,
            track_extras: Some(&mut track_extras),
            seqs: Vec::new(),
            pending: Vec::new(),
            seq_layers: kt
                .as_ref()
                .map(|k| {
                    k.sequences.iter().filter_map(|s| s.layers.clone().map(|l| (s.id, l))).collect()
                })
                .unwrap_or_default(),
        };
        read_reel(&tl.tracks.children, root_layers.as_ref(), &mut root, &mut cx, 0)?;
        // аудио-половины чужих вложений, встреченных на видео-дорожках
        // (и наоборот) — они рождаются вне обхода своей дорожки
        root.clips.extend(std::mem::take(&mut cx.pending));
        for (id, mut seq) in std::mem::take(&mut cx.seqs) {
            // сайдкар (Р.50): дорожки, порядок строк, морферы и камеры
            // ленты — то, чего OTIO не выражает. Не сошлось с файлом —
            // notice и чтение как есть, а не порча (паттерн KtLayers).
            match kt.as_ref().and_then(|k| k.sequences.iter().find(|s| s.id == id.0)) {
                Some(meta) => {
                    seq.name = meta.name.clone();
                    seq.video_lanes = meta.video_lanes.max(1);
                    seq.audio_lanes = meta.audio_lanes.max(1);
                    seq.morph_lanes = meta.morph_lanes;
                    seq.camera_lanes = meta.camera_lanes;
                    seq.lane_order = meta.lane_order.clone();
                    seq.morphers = meta.morphers.clone();
                    seq.cameras = meta.cameras.clone();
                    seq.overlap_layers = meta.overlap_layers;
                }
                None => cx
                    .notices
                    .push(format!("nested reel '{}' has no kt sidecar — read as is", seq.name)),
            }
            // дорожек не меньше, чем занято содержимым: сайдкар мог
            // разойтись с файлом, а validate этого не простит
            for (k, n) in
                [(ClipKind::Video, &mut seq.video_lanes), (ClipKind::Audio, &mut seq.audio_lanes)]
            {
                let need =
                    seq.clips.iter().filter(|c| c.kind == k).map(|c| c.lane + 1).max().unwrap_or(1);
                *n = (*n).max(need).max(1);
            }
            p.sequences.push(seq);
        }
    }
    p.clips = root.clips;
    p.transitions = root.transitions;
    p.foreign = root.foreign;
    if root.end > p.timeline_end.unwrap_or(Tick::ZERO) {
        p.timeline_end = Some(root.end);
    }
    if !root.lane_names.is_empty() {
        p.ui.get_or_insert_with(UiState::default).lane_names.extend(root.lane_names);
    }
    retained.tracks = track_extras;

    // конверт kt: транспорт/ui/бин/морферы/камеры
    if let Some(kt) = kt {
        // неизвестные kt-поля будущих версий — passthrough (Р.29)
        retained.timeline_kt_extra = kt.extra.clone();
        // наложенные переходы (Р.51, п.399): id назначаются заново —
        // идентичность задаёт пара, номера сквозные по документу (п.394)
        let mut next_tid = p.max_transition_id();
        let read = read_overlap_transitions(&kt.overlap_transitions, &p.clips, &mut next_tid);
        p.transitions.extend(read.0);
        notices.extend(read.1);
        for s in &mut p.sequences {
            let Some(meta) = kt.sequences.iter().find(|m| m.id == s.id.0) else { continue };
            let read =
                read_overlap_transitions(&meta.overlap_transitions, &s.clips, &mut next_tid);
            s.transitions.extend(read.0);
            notices.extend(read.1);
        }
        if let Some(tr) = kt.transport {
            p.region = tr.region.and_then(|(s, e)| kt_model::Region::normalized(Tick(s), Tick(e)));
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
            // адрес мог уже приехать с событием ленты — дубля быть не должно
            if !p.media.iter().any(|m| m.id == MediaId(bm.id)) {
                p.media.push(MediaRef { id: MediaId(bm.id), path });
            }
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
    // адрес КАЖДОГО битого клипа: чинить файл вслепую нечем, а исходник мы
    // не трогаем — отказ читать безопаснее молчаливой порчи связей
    if !bad_kt.is_empty() {
        return Err(Error::KtMetaBroken(bad_kt.join("; ")));
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
) -> (TransitionKind, AudioCurve, Vec<kt_model::ParamCurve>, bool) {
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
    let audio =
        kt.shift_remove("audio").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    // кривые параметров вида (Р.34 К4) — тот же конверт
    let auto: Vec<kt_model::ParamCurve> = match kt.shift_remove("auto") {
        Some(v) => serde_json::from_value(v).unwrap_or_else(|e| {
            notices.push(format!("unreadable transition curves ({e})"));
            Vec::new()
        }),
        None => Vec::new(),
    };
    // инверсия (Р.51, п.398); не-булево — notice + false, файл не валится
    let invert = match kt.shift_remove("invert") {
        Some(Value::Bool(b)) => b,
        Some(_) => {
            notices.push("unreadable transition invert flag, kept off".to_string());
            false
        }
        None => false,
    };
    if kt.is_empty() {
        meta.shift_remove(crate::meta::KT_NS);
    }
    (kind, audio, auto, invert)
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
/// Куда читается ОДНА лента.
#[derive(Default)]
struct ReelRead {
    clips: Vec<ClipEvent>,
    transitions: Vec<TransitionEvent>,
    foreign: Vec<ForeignEvent>,
    /// Конец содержимого: у корня из него получается `timeline_end`.
    end: Tick,
    lane_names: Vec<(kt_model::LaneKind, u32, String)>,
}

/// Общее на весь документ при чтении: счётчики адресов, накопитель чужого
/// сырья, замечания. Вынесено в структуру, потому что читатель стал
/// рекурсивным (Р.50) и тащить полтора десятка `&mut` через рекурсию —
/// это не читается.
struct ReadCtx<'a> {
    base_dir: Option<&'a Path>,
    is_ours: bool,
    file_v: u32,
    next_clip_id: &'a mut u64,
    next_foreign_key: &'a mut u64,
    next_media_id: &'a mut u64,
    media_by_url: &'a mut HashMap<String, MediaId>,
    media: &'a mut Vec<MediaRef>,
    retained: &'a mut Retained,
    notices: &'a mut Vec<Notice>,
    bad_auto: &'a mut Vec<String>,
    bad_kt: &'a mut Vec<String>,
    /// Чужое у ДОРОЖЕК возвращается на место только у корня: внутри ленты
    /// ключ `(kind, lane)` столкнулся бы с корневым. Наши ленты чужого
    /// сырья не несут вовсе, у чужих вложений оно теряется — цена,
    /// принятая вместе с решением «читать чужие Stack настоящими лентами».
    track_extras: Option<&'a mut HashMap<(ClipKind, u32), TrackExtra>>,
    /// Прочитанные ленты: адрес → содержимое.
    seqs: Vec<(MediaId, Sequence)>,
    /// Карты слоёв вложенных лент из сайдкара `kt.sequences`.
    seq_layers: HashMap<u64, crate::meta::KtLayers>,
    /// События, рождённые ВНЕ обхода своей дорожки: аудио-половина чужого
    /// вложения, встреченного на видео-дорожке (и наоборот).
    pending: Vec<ClipEvent>,
}

/// Прочитать детей одного `Stack` — треки одной ленты.
///
/// `layers` — карта слоёв (Р.28): перекрытые клипы одной НАШЕЙ дорожки
/// лежат соседними треками, и без карты они разъехались бы по разным
/// дорожкам. Карта, не сошедшаяся с файлом (чужой редактор пересобрал
/// треки), ИГНОРИРУЕТСЯ с замечанием — читаем один к одному.
fn read_reel(
    children: &[Composable],
    layers: Option<&crate::meta::KtLayers>,
    out: &mut ReelRead,
    cx: &mut ReadCtx<'_>,
    depth: usize,
) -> Result<(), Error> {
    let total_of =
        |k: ClipKind| children.iter().filter(|c| track_kind(c) == Some(k)).count() as u32;
    let video_total = total_of(ClipKind::Video);
    let layer_plan = |kind: ClipKind| -> Option<Vec<u32>> {
        let l = layers?;
        let counts = match kind {
            ClipKind::Video => &l.video,
            ClipKind::Audio => &l.audio,
        };
        let total = total_of(kind);
        if counts.is_empty() || counts.iter().sum::<u32>() != total {
            return None;
        }
        // видео пишется от НИЖНЕЙ дорожки к верхней — карта разворачивается
        let order: Vec<u32> = match kind {
            ClipKind::Video => (0..counts.len() as u32).rev().collect(),
            ClipKind::Audio => (0..counts.len() as u32).collect(),
        };
        let mut lanes: Vec<u32> = Vec::with_capacity(total as usize);
        for lane in order {
            for _ in 0..counts[lane as usize] {
                lanes.push(lane);
            }
        }
        Some(lanes)
    };
    let plan = (layer_plan(ClipKind::Video), layer_plan(ClipKind::Audio));
    if layers.is_some() && (plan.0.is_none() || plan.1.is_none()) {
        cx.notices.push("layer map does not match the tracks — tracks kept separate".into());
    }
    let mut seen_video = 0u32;
    let mut seen_audio = 0u32;
    for (pos, child) in children.iter().enumerate() {
        let Some(kind) = track_kind(child) else {
            cx.notices.push(format!(
                "unsupported track-level object was kept untouched: {}",
                composable_tag(child)
            ));
            // хребет типизирован — перекодируем в сырьё для Retained.
            // У КОРНЯ сырьё хранится с позицией и возвращается на неё: до
            // Р.50 оно уезжало в `foreign` по ключу, который писателю не
            // достаётся никогда, и круг терял объект молча.
            let raw = to_raw(child);
            if depth == 0 {
                cx.retained.root_foreign.push((pos, raw));
            } else {
                cx.retained.foreign.insert(*cx.next_foreign_key, raw);
                *cx.next_foreign_key += 1;
            }
            continue;
        };
        let Composable::Track(track) = child else { unreachable!() };
        let lane = match kind {
            ClipKind::Video => {
                let idx = seen_video;
                seen_video += 1;
                match &plan.0 {
                    Some(plan) => plan[idx as usize],
                    None => video_total - seen_video,
                }
            }
            ClipKind::Audio => {
                let idx = seen_audio;
                seen_audio += 1;
                match &plan.1 {
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
            effects: track.effects.clone(),
            markers: track.markers.clone(),
            enabled: track.enabled,
        };
        if !track.enabled {
            // правило 16: невозможность/необычность — вслух, а не молча
            cx.notices.push(format!(
                "track '{}' is disabled in the file; Kinotrain has no per-track mute — \
                 the flag is kept on save",
                if track.name.is_empty() { "<unnamed>" } else { &track.name }
            ));
        }
        if !track.name.is_empty() {
            let lk = match kind {
                ClipKind::Video => kt_model::LaneKind::Video,
                ClipKind::Audio => kt_model::LaneKind::Audio,
            };
            out.lane_names.push((lk, lane, track.name.clone()));
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
                    if let Some((in_off, out_off, (ttype, mut meta, extra))) =
                        pending_transition.take()
                        && let Some(a) = prev_clip
                    {
                        let id = out.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
                        let (kind, audio, auto, invert) = take_transition_kt(&mut meta, cx.notices);
                        cx.retained.transitions.insert((Some(a), None), (ttype, meta, extra));
                        out.transitions.push(TransitionEvent {
                            id: TransitionId(id),
                            a: Some(a),
                            b: None,
                            in_offset: in_off,
                            out_offset: out_off,
                            kind,
                            audio,
                            auto,
                            invert,
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
                        cx.is_ours,
                        cx.file_v,
                        cx.next_clip_id,
                        cx.media_by_url,
                        cx.next_media_id,
                        cx.base_dir,
                        cx.media,
                        cx.notices,
                        cx.bad_auto,
                        cx.bad_kt,
                    );
                    // переход перед клипом: с живым соседом слева —
                    // кроссфейд, без него (край дорожки или Gap) — фейд-ин
                    // (Р.25; эталон трактует так же — dissolve с Gap)
                    if let Some((in_off, out_off, (ttype, mut meta, extra))) =
                        pending_transition.take()
                    {
                        let id = out.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
                        let (kind, audio, auto, invert) = take_transition_kt(&mut meta, cx.notices);
                        if ttype != "SMPTE_Dissolve" {
                            cx.notices.push(format!("transition '{ttype}' imported as crossfade"));
                        }
                        cx.retained
                            .transitions
                            .insert((prev_clip, Some(ev.id)), (ttype, meta, extra));
                        out.transitions.push(TransitionEvent {
                            id: TransitionId(id),
                            a: prev_clip,
                            b: Some(ev.id),
                            in_offset: in_off,
                            out_offset: out_off,
                            kind,
                            audio,
                            auto,
                            invert,
                        });
                    }
                    prev_clip = Some(ev.id);
                    // ссылки на материал — и на уровне медиа: ими будут
                    // пользоваться клипы, рождённые в редакторе (п.356)
                    if !ex.media_refs.is_empty() {
                        cx.retained.media.entry(ev.media).or_insert_with(|| MediaRefs {
                            media_refs: ex.media_refs.clone(),
                            active_key: ex.active_key.clone(),
                        });
                    }
                    cx.retained.clips.insert(ev.id, ex);
                    out.clips.push(ev);
                    cursor = cursor + dur;
                }
                // Вложение: наше (`kt.seq`) — схлопывается по адресу; чужое
                // (`Stack` или `Track` внутри `Track`) — становится НОВОЙ
                // лентой тем же кодом. Глубже предела — старый путь
                // плейсхолдера ниже: он милосерднее отказа, круг цел.
                Composable::Stack(_) | Composable::Track(_) if depth < kt_model::MAX_NEST_DEPTH => {
                    let dur = composable_duration(item).ok_or_else(|| {
                        Error::UnsupportedSchema(format!(
                            "nested {} without readable duration",
                            composable_tag(item)
                        ))
                    })?;
                    let ev = match item {
                        Composable::Stack(st)
                            if cx.is_ours && crate::meta::nest_seq_id(&st.metadata).is_some() =>
                        {
                            read_nest(st, kind, lane, cursor, dur, cx, depth)?
                        }
                        other => read_foreign_nest(other, kind, lane, cursor, dur, cx, depth)?,
                    };
                    if let Some((in_off, out_off, (ttype, mut meta, extra))) =
                        pending_transition.take()
                    {
                        let id = out.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
                        let (kind, audio, auto, invert) = take_transition_kt(&mut meta, cx.notices);
                        cx.retained
                            .transitions
                            .insert((prev_clip, Some(ev.id)), (ttype, meta, extra));
                        out.transitions.push(TransitionEvent {
                            id: TransitionId(id),
                            a: prev_clip,
                            b: Some(ev.id),
                            in_offset: in_off,
                            out_offset: out_off,
                            kind,
                            audio,
                            auto,
                            invert,
                        });
                    }
                    prev_clip = Some(ev.id);
                    out.clips.push(ev);
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
                    if let Some((in_off, out_off, (ttype, mut meta, extra))) =
                        pending_transition.take()
                    {
                        if let Some(a) = prev_clip {
                            let id = out.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
                            let (kind, audio, auto, invert) = take_transition_kt(&mut meta, cx.notices);
                            cx.retained.transitions.insert((Some(a), None), (ttype, meta, extra));
                            out.transitions.push(TransitionEvent {
                                id: TransitionId(id),
                                a: Some(a),
                                b: None,
                                in_offset: in_off,
                                out_offset: out_off,
                                kind,
                                audio,
                                auto,
                                invert,
                            });
                        } else {
                            cx.notices
                                .push("transition between foreign objects was dropped".into());
                        }
                    }
                    out.foreign.push(ForeignEvent {
                        key: *cx.next_foreign_key,
                        kind,
                        lane,
                        start: cursor,
                        duration: dur,
                    });
                    cx.retained.foreign.insert(*cx.next_foreign_key, to_raw(other));
                    *cx.next_foreign_key += 1;
                    prev_clip = None;
                    cursor = cursor + dur;
                }
            }
        }
        // переход последним ребёнком дорожки — фейд-аут последнего клипа
        if let Some((in_off, out_off, (ttype, mut meta, extra))) = pending_transition.take() {
            if let Some(a) = prev_clip {
                let id = out.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
                let (kind, audio, auto, invert) = take_transition_kt(&mut meta, cx.notices);
                cx.retained.transitions.insert((Some(a), None), (ttype, meta, extra));
                out.transitions.push(TransitionEvent {
                    id: TransitionId(id),
                    a: Some(a),
                    b: None,
                    in_offset: in_off,
                    out_offset: out_off,
                    kind,
                    audio,
                    auto,
                    invert,
                });
            } else {
                cx.notices.push("transition on an empty track was dropped".into());
            }
        }
        // монтажная длина: хвостовой Gap чужого файла (Р.25) — конец
        // дорожки правее последнего клипа задаёт длину таймлайна
        out.end = out.end.max(cursor);
        if let Some(tracks) = cx.track_extras.as_deref_mut() {
            tracks.insert((kind, lane), tx);
        }
    }
    Ok(())
}

/// Прочитать ЧУЖОЕ вложение — `Stack` или `Track` внутри `Track` (Р.50, А5).
///
/// До этого раунда оно становилось серым плейсхолдером: «melt раскрывает
/// вложенные композиции и рисует их содержимое, а у нас там пустота»
/// (п.158). Теперь — настоящая лента, прочитанная ТЕМ ЖЕ `read_reel`.
/// Цена принята владельцем вместе с решением: импорт чужих файлов даёт
/// другую модель, чем давал раньше.
///
/// Два случая, и оба есть в `nested_example.otio`:
///
/// - дети — `Track`'и: обычная многодорожечная лента;
/// - дети — сразу клипы (`Stack` без треков, либо `Track` внутри `Track`):
///   лента из ОДНОЙ дорожки, и вид у неё — вид РОДИТЕЛЬСКОГО трека, потому
///   что своего у неё нет.
fn read_foreign_nest(
    item: &Composable,
    kind: ClipKind,
    lane: u32,
    start: Tick,
    dur: Tick,
    cx: &mut ReadCtx<'_>,
    depth: usize,
) -> Result<ClipEvent, Error> {
    let (name, children, source_range, effects) = match item {
        Composable::Stack(st) => (&st.name, &st.children, st.source_range, &st.effects),
        Composable::Track(t) => (&t.name, &t.children, t.source_range, &t.effects),
        _ => unreachable!("called on Stack/Track only"),
    };
    let has_tracks = children.iter().any(|c| track_kind(c).is_some());
    let mut body = ReelRead::default();
    if has_tracks {
        read_reel(children, None, &mut body, cx, depth + 1)?;
    } else {
        // Оборачиваем содержимое в ОДИН трек своего вида: `read_reel` умеет
        // читать только треки, а правило «клип за клипом с гэпами» у нас
        // одно и второй раз его выписывать нельзя.
        let synth = Composable::Track(Track {
            name: String::new(),
            kind: match item {
                Composable::Track(t) => t.kind.clone(),
                _ => match kind {
                    ClipKind::Video => "Video".into(),
                    ClipKind::Audio => "Audio".into(),
                },
            },
            children: children.clone(),
            source_range: None,
            effects: vec![],
            markers: vec![],
            enabled: true,
            metadata: Map::new(),
            extra: Map::new(),
        });
        read_reel(std::slice::from_ref(&synth), None, &mut body, cx, depth + 1)?;
    }
    if !effects.is_empty() {
        cx.notices.push(format!(
            "effects on the nested '{}' are not ours and were dropped",
            if name.is_empty() { "<unnamed>" } else { name }
        ));
    }
    let addr = {
        let free = (*cx.next_media_id)
            .max(cx.media.iter().map(|m| m.id.0 + 1).max().unwrap_or(0))
            .max(cx.seqs.iter().map(|(id, _)| id.0 + 1).max().unwrap_or(0));
        *cx.next_media_id = free + 1;
        MediaId(free)
    };
    let mut seq = Sequence::new(
        addr,
        if name.trim().is_empty() { format!("Nested {}", addr.0) } else { name.clone() },
    );
    for (k, n) in [(ClipKind::Video, &mut seq.video_lanes), (ClipKind::Audio, &mut seq.audio_lanes)]
    {
        *n =
            body.clips.iter().filter(|c| c.kind == k).map(|c| c.lane + 1).max().unwrap_or(1).max(1);
    }
    let both = body.clips.iter().any(|c| c.kind == ClipKind::Video)
        && body.clips.iter().any(|c| c.kind == ClipKind::Audio);
    seq.clips = body.clips;
    seq.transitions = body.transitions;
    cx.seqs.push((addr, seq));

    let id = ClipId(*cx.next_clip_id);
    *cx.next_clip_id += 1;
    let source_in = source_range.map(|r| r.start_time.to_ticks()).unwrap_or(Tick::ZERO);
    // Чужое вложение со звуком И картинкой внутри: у нас это ДВА события в
    // одной link-группе — своей «дорожки обоих видов» у нас нет. Молчать
    // тут нельзя: человек увидит на таймлайне на одно событие больше, чем
    // было в чужом файле.
    let group = if both {
        let seq_name = cx.seqs.last().map(|(_, s)| s.name.clone()).unwrap_or_default();
        cx.notices.push(format!(
            "nested reel '{seq_name}' carries both video and audio — it became a linked pair"
        ));
        let g = GroupId(*cx.next_clip_id);
        *cx.next_clip_id += 1;
        Some(g)
    } else {
        None
    };
    let make = |id: ClipId, kind: ClipKind, lane: u32| ClipEvent {
        id,
        link_group: group,
        kind,
        lane,
        start,
        duration: dur,
        source_in,
        media: addr,
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    if both {
        let other = match kind {
            ClipKind::Video => ClipKind::Audio,
            ClipKind::Audio => ClipKind::Video,
        };
        // партнёр встаёт на НУЛЕВУЮ дорожку своего вида: переслаивание
        // включено, наложение законно, а угадывать свободную нечем
        let pid = ClipId(*cx.next_clip_id);
        *cx.next_clip_id += 1;
        cx.pending.push(make(pid, other, 0));
    }
    Ok(make(id, kind, lane))
}

/// Прочитать вложение: `Stack` ребёнком `Track` (Р.50).
///
/// Тело инлайнится в КАЖДЫЙ экземпляр (объектной идентичности в JSON OTIO
/// нет), поэтому экземпляры схлопываются по `kt.seq`. Если тела разошлись —
/// чужой редактор правил один экземпляр и не правил другой — второе тело
/// становится ОТДЕЛЬНОЙ лентой с notice: тихий выбор одного из двух есть
/// потеря чужой работы.
/// Эффекты вложения: та же раскладка, что у клипа, но чужого у нас тут не
/// бывает — вложение мы пишем сами. Кривые читаются в домене v3+ (тики
/// материала): файл с вложением не может быть старше v6.
fn nest_effects(
    st: &Stack,
    cx: &mut ReadCtx<'_>,
) -> (Vec<kt_model::FxInstance>, Vec<bool>, Vec<Vec<kt_model::ParamCurve>>, bool) {
    let mut fx = Vec::new();
    let mut bypass = Vec::new();
    let mut auto: Vec<Vec<kt_model::ParamCurve>> = Vec::new();
    let mut reversed = false;
    for e in &st.effects {
        match e {
            EffectNode::Effect(eff) if eff.effect_name.starts_with("kt:") => {
                match kt_get::<kt_model::FxInstance>(&eff.metadata) {
                    Some(inst) => {
                        fx.push(inst);
                        bypass.push(!eff.enabled);
                        auto.push(
                            eff.metadata
                                .get(crate::meta::KT_NS)
                                .and_then(|kt| kt.get("auto"))
                                .and_then(|v| serde_json::from_value(v.clone()).ok())
                                .unwrap_or_default(),
                        );
                    }
                    None => cx.notices.push(format!(
                        "unreadable effect {} on a nested reel was dropped",
                        eff.effect_name
                    )),
                }
            }
            EffectNode::LinearTimeWarp(w) if w.time_scalar == -1.0 => reversed = true,
            EffectNode::LinearTimeWarp(w) if w.time_scalar == 1.0 => {}
            other => cx.notices.push(format!(
                "{} on a nested reel is not ours and was dropped",
                match other {
                    EffectNode::LinearTimeWarp(w) => format!("time warp x{}", w.time_scalar),
                    _ => "an effect".to_string(),
                }
            )),
        }
    }
    while bypass.last() == Some(&false) {
        bypass.pop();
    }
    while auto.last().is_some_and(|v| v.is_empty()) {
        auto.pop();
    }
    (fx, bypass, auto, reversed)
}

fn read_nest(
    st: &Stack,
    kind: ClipKind,
    lane: u32,
    start: Tick,
    dur: Tick,
    cx: &mut ReadCtx<'_>,
    depth: usize,
) -> Result<ClipEvent, Error> {
    let seq_id = MediaId(crate::meta::nest_seq_id(&st.metadata).unwrap_or(0));
    let meta: Option<KtClipMeta> = kt_get_checked(&st.metadata)
        .map_err(|e| Error::KtMetaBroken(format!("nested reel '{}': {e}", st.name)))?;
    let mut body = ReelRead::default();
    if depth < kt_model::MAX_NEST_DEPTH {
        let layers = cx.seq_layers.get(&seq_id.0).cloned();
        read_reel(&st.children, layers.as_ref(), &mut body, cx, depth + 1)?;
    } else {
        cx.notices.push(format!(
            "nested reel '{}' is deeper than {} levels — its content was dropped",
            st.name,
            kt_model::MAX_NEST_DEPTH
        ));
    }
    // тот же адрес уже читался — сверяем тела
    let mut addr = seq_id;
    match cx.seqs.iter().find(|(id, _)| *id == seq_id) {
        Some((_, prev)) if prev.clips == body.clips && prev.transitions == body.transitions => {}
        Some(_) => {
            // тела разошлись: вторая версия живёт своей лентой, и её
            // содержимое получает СВЕЖИЕ адреса (ClipId уникален по
            // документу)
            // свободный адрес ищется по ВСЕМУ уже прочитанному: счётчик
            // `next_media_id` раздаёт адреса только файлам, найденным по
            // URL, а адреса из нашего конверта приезжают готовыми
            let free = (*cx.next_media_id)
                .max(cx.media.iter().map(|m| m.id.0 + 1).max().unwrap_or(0))
                .max(cx.seqs.iter().map(|(id, _)| id.0 + 1).max().unwrap_or(0));
            addr = MediaId(free);
            *cx.next_media_id = free + 1;
            let mut map: HashMap<ClipId, ClipId> = HashMap::new();
            for c in &mut body.clips {
                let fresh = ClipId(*cx.next_clip_id);
                *cx.next_clip_id += 1;
                map.insert(c.id, fresh);
                c.id = fresh;
            }
            for t in &mut body.transitions {
                t.a = t.a.and_then(|x| map.get(&x).copied());
                t.b = t.b.and_then(|x| map.get(&x).copied());
            }
            cx.notices.push(format!(
                "two different bodies for nested reel '{}' — the second became its own reel",
                st.name
            ));
            let mut seq = Sequence::new(addr, st.name.clone());
            seq.clips = std::mem::take(&mut body.clips);
            seq.transitions = std::mem::take(&mut body.transitions);
            cx.seqs.push((addr, seq));
        }
        None => {
            let mut seq = Sequence::new(seq_id, st.name.clone());
            seq.clips = std::mem::take(&mut body.clips);
            seq.transitions = std::mem::take(&mut body.transitions);
            cx.seqs.push((seq_id, seq));
        }
    }
    let id = match meta.as_ref().map(|m| m.id) {
        Some(v) if v != 0 => ClipId(v),
        _ => {
            let v = ClipId(*cx.next_clip_id);
            *cx.next_clip_id += 1;
            v
        }
    };
    *cx.next_clip_id = (*cx.next_clip_id).max(id.0 + 1);
    let (fx, fx_bypass, fx_auto, reversed) = nest_effects(st, cx);
    Ok(ClipEvent {
        id,
        link_group: meta.as_ref().and_then(|m| m.link_group).map(GroupId),
        kind,
        lane,
        start,
        duration: dur,
        source_in: st.source_range.map(|r| r.start_time.to_ticks()).unwrap_or(Tick::ZERO),
        media: addr,
        gain_db: meta.as_ref().map(|m| m.gain).unwrap_or(0.0),
        reversed,
        fx,
        fx_bypass,
        fx_auto,
        opacity: meta.as_ref().map(|m| m.opacity).unwrap_or(1.0),
        fx_off: meta.as_ref().map(|m| m.fx_off).unwrap_or(false),
    })
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
    media_list: &mut Vec<MediaRef>,
    notices: &mut Vec<Notice>,
    bad_auto: &mut Vec<String>,
    bad_kt: &mut Vec<String>,
) -> (ClipEvent, ClipExtra) {
    // Битая НАША секция у клипа — ОШИБКА, а не «чужой файл» (Р.49, п.358;
    // близнец t251 у Timeline). `kt_get` с `.ok()` выбрасывал конверт молча:
    // клип получал НОВЫЙ номер, а с ним рвались link-группы A/V и ссылки
    // морферов, терялись громкость, прозрачность и выключенная цепочка.
    // Notice тут не спасает: переприсвоенный номер необратим.
    let meta: Option<KtClipMeta> = match kt_get_checked(&c.metadata) {
        Ok(v) => v,
        Err(e) => {
            bad_kt.push(format!(
                "clip '{}' at {:?} lane {lane} ({}): broken kt section ({e})",
                c.name, kind, start.0
            ));
            None
        }
    };
    let url = match c.active_media() {
        Some(MediaRefNode::External(er)) => er.target_url.clone(),
        // наш генератор: канонический gen:-путь из generator_kind +
        // parameters; непрочитавшийся (kt-вид новее сборки) — пустой url,
        // то есть заметная заглушка, а узел уезжает обратно без потерь
        Some(MediaRefNode::Generator(gr)) => generator_ref_path(gr).unwrap_or_default(),
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
    if !media_list.iter().any(|m| m.id == media) {
        media_list.push(MediaRef { id: media, path: resolve_url(base_dir, &url) });
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
                        notices
                            .push(format!("unreadable effect {} kept as foreign", eff.effect_name));
                        ex.effects_foreign.push((i, e.clone()));
                    }
                }
            }
            EffectNode::LinearTimeWarp(w) if w.time_scalar == -1.0 => reversed = true,
            EffectNode::LinearTimeWarp(w) if w.time_scalar == 1.0 => {}
            EffectNode::LinearTimeWarp(w) => {
                notices
                    .push(format!("time warp x{} is not rendered (kept untouched)", w.time_scalar));
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
