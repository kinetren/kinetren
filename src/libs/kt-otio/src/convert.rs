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
    ProjectSettings, Sequence, TransitionEvent, TransitionId, TransitionKind,
};
use kt_time::Tick;

/// Замечание импорта — показывается пользователю note'ом.
pub type Notice = String;

/// (extra, metadata) объекта — passthrough-пара.
pub type ExtraMeta = (Map<String, Value>, Map<String, Value>);
/// (transition_type, metadata, extra) чужого перехода.
pub type TransitionExtra = (String, Map<String, Value>, Map<String, Value>);

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MarkerExtra {
    pub comment: String,
    pub metadata: Map<String, Value>,
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MediaRefs {
    pub media_refs: IndexMap<String, Option<MediaRefNode>>,
    pub active_key: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Retained {
    pub timeline_name: String,
    pub timeline_extra: Map<String, Value>,
    /// Чужие неймспейсы metadata Timeline (порядок сохранён, kt исключён).
    pub timeline_meta_foreign: Map<String, Value>,
    pub stack_name: String,
    pub stack_extra: Map<String, Value>,
    pub stack_metadata: Map<String, Value>,
    pub global_start_time: Option<RationalTime>,
    // HashMap-поля (Р.70): в файле проекта — ПАРЫ, отсортированные по ключу.
    // JSON не умеет нестроковых ключей, а порядок обхода HashMap случаен —
    // без сортировки каждое сохранение давало бы ложный дифф в git.
    #[serde(with = "sorted_pairs")]
    pub tracks: HashMap<(ClipKind, u32), TrackExtra>,
    #[serde(with = "sorted_pairs")]
    pub clips: HashMap<ClipId, ClipExtra>,
    /// Ссылки на материал по МЕДИА — общие для всех его клипов (п.356).
    #[serde(with = "sorted_pairs")]
    pub media: HashMap<kt_model::MediaId, MediaRefs>,
    /// По паре клипов стыка.
    /// Ключ — пара концов перехода; у фейда (Р.25) один конец None.
    #[serde(with = "sorted_pairs")]
    pub transitions: HashMap<(Option<ClipId>, Option<ClipId>), TransitionExtra>,
    /// Сырьё foreign-плейсхолдеров по ForeignEvent.key.
    #[serde(with = "sorted_pairs")]
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

impl Retained {
    /// Переставить сырьё чужих ТРЕКОВ вслед за перенумерацией дорожек
    /// (Р.56, п.449). Ключ — семантическая пара (вид, номер), и при
    /// «номер = ранг строки» она меняется на чтении: не переставь мы
    /// карту, metadata чужого трека уехала бы на соседний трек МОЛЧА.
    pub fn remap_tracks(&mut self, perm: &kt_model::LanePerm) {
        if perm.is_identity() {
            return;
        }
        self.tracks =
            self.tracks.drain().map(|((k, l), v)| ((k, perm.map_clip(k, l)), v)).collect();
    }
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
            tracks: &p.tracks,
            timeline_end,
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
            let has_cams = !cameras.is_empty() || p.lanes(kt_model::LaneKind::Camera) > 0;
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
            // событие левее нуля (Р.58, п.462): старая сборка положила бы
            // его в ноль и сдвинула бы за ним всё содержимое ленты — молча
            let has_v8 = p
                .clips
                .iter()
                .chain(p.sequences.iter().flat_map(|s| s.clips.iter()))
                .any(|c| c.start < Tick::ZERO);
            // вложенные ленты (Р.50): старая сборка прочла бы вложение
            // серым плейсхолдером (п.158) и при первой перезаписи вернула
            // бы живой монтаж в файл мёртвым сырьём
            // параметры генератора на СОБЫТИИ (Р.59, стадия 2): старая
            // сборка полезла бы за ними в путь медиа, где их больше нет,
            // и показала бы пустой титр — причём во всех местах разом
            //
            // Публичные параметры композиции и переопределения у
            // экземпляра (Р.59, стадия 3) — ТА ЖЕ версия: решение раунда
            // «одна версия на раунд, поднимается ПО СОДЕРЖИМОМУ», и
            // предикат обязан быть ОДНИМ местом. Старая сборка сыграла бы
            // все экземпляры шаблонными значениями — десять выпусков
            // новостей с одним заголовком и ни одного видимого признака.
            let has_v9 = p
                .clips
                .iter()
                .chain(p.sequences.iter().flat_map(|s| s.clips.iter()))
                .any(|c| c.r#gen.is_some() || !c.overrides.is_empty())
                || !p.published.is_empty()
                || p.sequences.iter().any(|s| !s.published.is_empty());
            // форма отрезка, не выразимая прежним булевым полем (Р.61):
            // однородная «третий порядок» или «прямая» версию НЕ двигают —
            // они пишутся тем же `ease`, что и раньше, байт в байт (D1b).
            // Всё остальное старая сборка прочла бы однородным: зигзаг
            // стал бы прямой, ступень — плавным переездом, и ни одного
            // признака порчи.
            let curve_needs_v10 = |c: &kt_model::ParamCurve| {
                let ease = !c.keys.iter().all(|k| k.shape == kt_model::SegShape::Linear);
                let implied =
                    if ease { kt_model::SegShape::DEFAULT } else { kt_model::SegShape::Linear };
                c.keys.iter().any(|k| k.shape != implied)
            };
            let clip_needs_v10 = |c: &kt_model::ClipEvent| {
                c.fx_auto.iter().flatten().chain(c.gen_auto.iter()).any(curve_needs_v10)
            };
            let reel_needs_v10 = |reel: &dyn kt_model::Reel| {
                reel.clips().iter().any(clip_needs_v10)
                    || reel.transitions().iter().any(|t| t.auto.iter().any(curve_needs_v10))
                    || reel.cameras().iter().any(|c| c.auto.iter().any(curve_needs_v10))
            };
            let has_v10 = reel_needs_v10(p)
                || p.sequences.iter().any(|s| reel_needs_v10(s as &dyn kt_model::Reel))
                || p.morphers.iter().any(|m| {
                    m.patch
                        .video_auto
                        .iter()
                        .chain(m.patch.audio_auto.iter())
                        .flatten()
                        .any(curve_needs_v10)
                })
                || p.settings.default_shape != kt_model::SegShape::DEFAULT;
            // v11 (Р.62): правило проекта не «вписать», явные настройки,
            // ведущий узел с не-умолчанием или не первым, узел «Геометрия»
            // в цепочке — старая сборка потеряла бы или не прочла
            let clip_needs_v11 = |c: &kt_model::ClipEvent| {
                !c.geometry.is_default()
                    || c.geometry_pos != 0
                    || !c.geometry_auto.is_empty()
                    || c.fx.iter().any(|f| matches!(f, kt_model::FxInstance::Geometry { .. }))
            };
            // опубликованная ручка ВЕДУЩЕЙ ГЕОМЕТРИИ (Р.63) — то же
            // новшество Р.62: адрес `owner: "geometry"` старая сборка не
            // разберёт вовсе, и сказать об этом обязана версия, а не
            // ошибка разбора
            let reel_needs_v11 = |reel: &dyn kt_model::Reel| {
                reel.clips().iter().any(clip_needs_v11)
                    || reel
                        .published()
                        .iter()
                        .any(|p| matches!(p.target.owner, kt_model::ParamOwner::Geometry))
            };
            // дорожка-объект с любым не-умолчанием — тоже новшество (Р.62,
            // change B): старая сборка потеряла бы mute, уровень, режим и
            // цепочку дорожки молча
            let track_needs_v11 = |t: &kt_model::Track| t.has_novelty();
            let has_v11 = reel_needs_v11(p)
                || p.tracks.iter().any(track_needs_v11)
                || p.sequences.iter().any(|s| s.tracks.iter().any(track_needs_v11))
                || p.sequences.iter().any(|s| reel_needs_v11(s as &dyn kt_model::Reel))
                || p.morphers.iter().any(|m| {
                    m.patch
                        .video_fx
                        .iter()
                        .any(|f| matches!(f, kt_model::FxInstance::Geometry { .. }))
                })
                || p.settings.conform != Default::default()
                || p.settings.settings_explicit
                || p.sequences.iter().any(|s| s.raster.is_some() || s.fps.is_some());
            // v12 (Р.65): назначенный КЛЮЧ сайдчейна в цепочке дорожки
            let track_needs_v12 =
                |t: &kt_model::Track| kt_model::sidechain_keys(&t.fx).next().is_some();
            let has_v12 = p.tracks.iter().any(track_needs_v12)
                || p.sequences.iter().any(|s| s.tracks.iter().any(track_needs_v12));
            // v13 (Р.67): формула отрезка, слой над ключами, `k` в формуле
            let clip_needs_v13 = |c: &kt_model::ClipEvent| {
                c.fx_auto
                    .iter()
                    .flatten()
                    .chain(c.gen_auto.iter())
                    .chain(c.geometry_auto.iter())
                    .chain(c.overrides.iter().flat_map(|o| o.curves.iter()))
                    .any(kt_model::ParamCurve::needs_v13)
            };
            let reel_needs_v13 = |reel: &dyn kt_model::Reel| {
                reel.clips().iter().any(clip_needs_v13)
                    || reel.transitions().iter().any(|t| t.auto.iter().any(kt_model::ParamCurve::needs_v13))
                    || reel.cameras().iter().any(|c| c.auto.iter().any(kt_model::ParamCurve::needs_v13))
            };
            let has_v13 = reel_needs_v13(p)
                || p.sequences.iter().any(|s| reel_needs_v13(s as &dyn kt_model::Reel))
                || p.morphers.iter().any(|m| {
                    m.patch
                        .video_auto
                        .iter()
                        .chain(m.patch.audio_auto.iter())
                        .flatten()
                        .any(kt_model::ParamCurve::needs_v13)
                });
            if has_v13 {
                crate::meta::KT_META_V13
            } else if has_v12 {
                crate::meta::KT_META_V12
            } else if has_v11 {
                crate::meta::KT_META_V11
            } else if has_v10 {
                crate::meta::KT_META_V10
            } else if has_v9 {
                crate::meta::KT_META_V9
            } else if has_v8 {
                crate::meta::KT_META_V8
            } else if has_v7 {
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
            default_shape: (p.settings.default_shape != Default::default())
                .then_some(p.settings.default_shape),
            conform: (p.settings.conform != Default::default()).then_some(p.settings.conform),
            settings_explicit: p.settings.settings_explicit,
        }),
        transport: (p.region.is_some() || p.loop_enabled).then(|| KtTransport {
            region: p.region.map(|r| (r.start.0, r.end.0)),
            loop_enabled: p.loop_enabled,
        }),
        // имена дорожек с Р.62 живут на самих дорожках (`Track.name` файла
        // и `kt.tracks` у служебных строк), в `ui` их больше нет; bin-поля
        // тоже отдельно (kt.bin_*) — один дом у каждого факта
        ui: p.ui.clone().map(|mut u| {
            u.lane_names = Vec::new();
            u.lane_heights = Vec::new();
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
        morph_lanes: p.lanes(kt_model::LaneKind::Morph),
        lane_order: Vec::new(),
        tracks: track_list(&p.tracks),
        areas: p.areas.clone(),
        // длина сверх материала (пустые дорожки в файл не пишем)
        end: p.timeline_end.map(|t| t.0),
        lanes: Some((p.lanes(kt_model::LaneKind::Video), p.lanes(kt_model::LaneKind::Audio))),
        layers: {
            let lo = |k, l| {
                layers_of(&p.clips, &p.foreign, retained.tracks.contains_key(&(k, l)), k, l) as u32
            };
            let v: Vec<u32> =
                (0..p.lanes(kt_model::LaneKind::Video)).map(|l| lo(ClipKind::Video, l)).collect();
            let a: Vec<u32> =
                (0..p.lanes(kt_model::LaneKind::Audio)).map(|l| lo(ClipKind::Audio, l)).collect();
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
        camera_lanes: p.lanes(kt_model::LaneKind::Camera),
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
                        video_lanes: s.lanes(kt_model::LaneKind::Video),
                        audio_lanes: s.lanes(kt_model::LaneKind::Audio),
                        morph_lanes: s.lanes(kt_model::LaneKind::Morph),
                        camera_lanes: s.lanes(kt_model::LaneKind::Camera),
                        lane_order: Vec::new(),
                        tracks: track_list(&s.tracks),
                        morphers,
                        cameras,
                        overlap_layers: s.overlap_layers,
                        layers: {
                            let lo = |k, l| layers_of(&s.clips, &[], false, k, l) as u32;
                            let v: Vec<u32> = (0..s.lanes(kt_model::LaneKind::Video))
                                .map(|l| lo(ClipKind::Video, l))
                                .collect();
                            let a: Vec<u32> = (0..s.lanes(kt_model::LaneKind::Audio))
                                .map(|l| lo(ClipKind::Audio, l))
                                .collect();
                            // как у корня: пишем, только когда слой не один
                            let multi = v.iter().chain(a.iter()).any(|n| *n > 1);
                            (multi || !s.overlap_layers).then(|| crate::meta::KtLayers {
                                off: !s.overlap_layers,
                                video: if multi { v } else { Vec::new() },
                                audio: if multi { a } else { Vec::new() },
                            })
                        },
                        overlap_transitions: overlap_transition_list(s),
                        published: s.published.clone(),
                    }
                })
                .collect();
            v.sort_by_key(|s| s.id);
            v
        },
        overlap_transitions: overlap_transition_list(p),
        published: p.published.clone(),
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
    /// Дорожки-объекты ленты (Р.62): имя, выключатели и цепочка едут на
    /// сам OTIO-трек, порядок и остальное — в конверт.
    tracks: &'a [kt_model::Track],
    timeline_end: Tick,
    /// Чужое сырьё. У наших лент его не бывает — пустой `Retained`.
    retained: &'a Retained,
}

impl ReelWrite<'_> {
    fn lanes(&self, kind: kt_model::LaneKind) -> u32 {
        self.tracks.iter().filter(|t| t.kind == kind).count() as u32
    }

    fn track(&self, kind: kt_model::LaneKind, lane: u32) -> Option<&kt_model::Track> {
        kt_model::track_index(self.tracks, kind, lane).map(|i| &self.tracks[i])
    }
}

/// Цепочка FX в стандартные `Effect.1` — ОДИН код для события, вложения и
/// ДОРОЖКИ (Р.62): расходись они, дорожечная цепочка читалась бы иначе.
fn chain_effects(
    fx: &[kt_model::FxInstance],
    bypass: &[bool],
    auto: &[Vec<kt_model::ParamCurve>],
) -> Vec<EffectNode> {
    let mut out = Vec::new();
    for (i, f) in fx.iter().enumerate() {
        let mut md = Map::new();
        kt_set(&mut md, f);
        if let Some(curves) = auto.get(i).filter(|v| !v.is_empty()) {
            crate::meta::kt_entry_mut(&mut md).insert(
                "auto".into(),
                serde_json::to_value(curves).expect("curves are serializable"),
            );
        }
        out.push(EffectNode::Effect(Effect {
            name: String::new(),
            effect_name: format!("kt:{}", fx_tag(f)),
            enabled: !bypass.get(i).copied().unwrap_or(false),
            metadata: md,
            extra: Map::new(),
        }));
    }
    out
}

/// Разобрать `Effect.1` дорожки: наши `kt:`-эффекты в цепочку, чужие —
/// обратно вызывающему (они возвращаются в файл нетронутыми, п.358).
fn read_chain_effects(
    effects: &[EffectNode],
) -> (Vec<kt_model::FxInstance>, Vec<bool>, Vec<Vec<kt_model::ParamCurve>>, Vec<EffectNode>) {
    let mut fx: Vec<kt_model::FxInstance> = Vec::new();
    let mut bypass: Vec<bool> = Vec::new();
    let mut auto: Vec<Vec<kt_model::ParamCurve>> = Vec::new();
    let mut rest: Vec<EffectNode> = Vec::new();
    for e in effects {
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
                    None => rest.push(e.clone()),
                }
            }
            other => rest.push(other.clone()),
        }
    }
    while bypass.last() == Some(&false) {
        bypass.pop();
    }
    while auto.last().is_some_and(|v| v.is_empty()) {
        auto.pop();
    }
    (fx, bypass, auto, rest)
}

/// Конверт дорожек ленты (Р.62). Пусто — писать нечего: порядок
/// естественный и все дорожки без свойств, файл байт-в-байт прежний.
fn track_list(tracks: &[kt_model::Track]) -> Vec<crate::meta::KtTrack> {
    let natural = {
        let mut v: Vec<kt_model::LaneKind> = Vec::new();
        for k in kt_model::LaneKind::ALL {
            v.extend(tracks.iter().filter(|t| t.kind == k).map(|t| t.kind));
        }
        v
    };
    let order_natural = tracks.iter().map(|t| t.kind).eq(natural.iter().copied());
    // имя и цепочка ВИДЕО/АУДИО живут на самих треках файла; служебные
    // строки своего трека не имеют, поэтому их имя — здесь
    // писать конверт незачем, если порядок естественный и сказать о
    // дорожках нечего: имя видео/аудио живёт на самом треке файла
    let own_fact = |t: &kt_model::Track| {
        t.has_novelty()
            || t.height != 0.0
            || (matches!(t.kind, kt_model::LaneKind::Morph | kt_model::LaneKind::Camera)
                && !t.name.is_empty())
    };
    if order_natural && !tracks.iter().any(own_fact) {
        return Vec::new();
    }
    tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let lane = tracks[..i].iter().filter(|x| x.kind == t.kind).count() as u32;
            let on_track = matches!(t.kind, kt_model::LaneKind::Video | kt_model::LaneKind::Audio);
            let _ = lane;
            crate::meta::KtTrack {
                kind: t.kind,
                id: t.id.0,
                // имя видео/аудио — в `Track.name` файла (один дом факта)
                name: if on_track { String::new() } else { t.name.clone() },
                height: t.height,
                mute: t.mute,
                solo: t.solo,
                lock: t.lock,
                gain_db: t.gain_db,
                opacity: t.opacity,
                blend: t.blend,
                fx_off: t.fx_off,
            }
        })
        .collect()
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
        // `cursor` — позиция В ТРЕКЕ OTIO (последовательном), а не время
        // модели. Обычно они совпадают: зазор дописывается ровно до
        // `start`. Расходятся они у события ЛЕВЕЕ НУЛЯ внутри ленты (Р.58,
        // п.462) и у всего, что идёт за ним и легло бы на курсор трека:
        // такое событие пишется НА курсоре, а истинный `start` уезжает в
        // конверт (`kt.start`), откуда его берёт наш ридер.
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
                    let at = cursor.max(start);
                    match p.sequence(c.media) {
                        Some(seq) => kids
                            .push(Composable::Stack(nest_to_otio(c, seq, p, base_dir, depth, at))),
                        None => kids.push(Composable::Clip(clip_to_otio(
                            c,
                            r.retained,
                            &url_of(&media_path(c.media)),
                            at,
                            p.media_duration(c.media),
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
            // трек OTIO продвигается на ДЛИНУ элемента от его места в
            // треке; при совпадении с моделью это прежнее `cursor = end`
            cursor = cursor.max(start) + (end - start);
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
        let lk = kt_model::lane_key_of(kind);
        let t = r.track(lk, lane);
        let name = t.map(|t| t.name.clone()).unwrap_or_default();
        // ЦЕПОЧКА дорожки — стандартными `Effect.1` тем же кодом, что у
        // клипа (Р.62); чужие эффекты возвращаются на место (п.358).
        // Слой расслоения несёт только чужое: цепочка принадлежит слою 0.
        let mut effects = tx.effects.clone();
        if layer == 0
            && let Some(t) = t
        {
            effects.extend(chain_effects(&t.fx, &t.fx_bypass, &t.fx_auto));
        }
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
            effects,
            markers: tx.markers.clone(),
            // `enabled = !mute` (Р.62): чужой читатель видит заглушённую
            // дорожку честно, а не «у Kinotrain нет per-track mute»
            enabled: tx.enabled && !t.is_some_and(|t| t.mute),
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
    for lane in (0..r.lanes(kt_model::LaneKind::Video)).rev() {
        for layer in 0..layers_of(ClipKind::Video, lane) {
            emit_track(&mut children, ClipKind::Video, lane, layer);
        }
    }
    for lane in 0..r.lanes(kt_model::LaneKind::Audio) {
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
    at: Tick,
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
                tracks: &seq.tracks,
                timeline_end: Tick::ZERO,
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
            start: displaced_start(c, at),
            r#gen: c.r#gen.clone(),
            gen_auto: c.gen_auto.clone(),
            overrides: c.overrides.clone(),
            geometry: (!c.geometry.is_default()).then_some(c.geometry),
            geometry_pos: c.geometry_pos,
            geometry_auto: c.geometry_auto.clone(),
            extra: Map::new(),
        },
    );
    // адрес ленты — вот по этому ключу экземпляры схлопываются на чтении
    crate::meta::kt_entry_mut(&mut meta).insert("seq".into(), Value::from(seq.id.0));
    // свои растр и fps ленты (Р.62) — только когда заданы
    if let Some((w, h)) = seq.raster {
        crate::meta::kt_entry_mut(&mut meta).insert("raster".into(), serde_json::json!([w, h]));
    }
    if let Some(f) = seq.fps {
        crate::meta::kt_entry_mut(&mut meta).insert("fps".into(), Value::from(f));
    }
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
    // ЛЕГАСИ-режим анимации титра (Р.53) — полями `anim`/`anim_secs`.
    // Вид их больше не несёт, и serde их просто проигнорировал бы, поэтому
    // они переезжают в путь токенами `am`/`ams`: дальше их подберёт
    // ЕДИНСТВЕННАЯ миграция (`Project::migrate_gen_to_events`), которая и
    // превращает режим в кривые. Второй копии этих формул не бывает.
    let anim = fields.shift_remove("anim").and_then(|v| v.as_str().map(str::to_string));
    let anim_secs = fields.shift_remove("anim_secs").and_then(|v| v.as_f64());
    let kind: kt_model::GeneratorKind = serde_json::from_value(Value::Object(fields)).ok()?;
    let mut path = kt_model::format_gen_path(&kind);
    if let Some(a) = anim.as_deref().and_then(legacy_anim_token)
        && a != "no"
    {
        path.push_str(&format!(":am={a}"));
        if let Some(sec) = anim_secs.filter(|v| v.is_finite() && *v > 0.0) {
            path.push_str(&format!(":ams={sec}"));
        }
    }
    Some(path)
}

/// Имя легаси-режима из файла (`typewriter`) → токен пути (`tw`).
fn legacy_anim_token(name: &str) -> Option<&'static str> {
    kt_model::TextAnim::ALL
        .into_iter()
        .find(|a| format!("{a:?}").to_lowercase() == name)
        .map(|a| a.token())
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

/// `kt.start` пишется ТОЛЬКО когда место в треке (`at`) не равно времени
/// модели (Р.58, п.462). Обычный монтаж — `None`, и файлы без поля
/// байт-стабильны; сторож — t638 и golden-фикстуры.
fn displaced_start(c: &ClipEvent, at: Tick) -> Option<i64> {
    (c.start != at).then_some(c.start.0)
}

fn clip_to_otio(
    c: &ClipEvent,
    retained: &Retained,
    target_url: &str,
    at: Tick,
    media_len: Option<Tick>,
) -> Clip {
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
        // Значения — С СОБЫТИЯ (Р.59, стадия 2); путь несёт только вид,
        // и разбирать его приходится лишь у клипа без значений (чужой
        // импорт, битая kt-секция).
        if let Some(kind) = c.r#gen.clone().or_else(|| kt_model::parse_gen_path(target_url)) {
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
            // ДЛИНА МАТЕРИАЛА — в стандартное поле OTIO (Р.63 п.2), а не в
            // наш конверт: `available_range` ровно про это и есть, и чужой
            // читатель понимает его без нас. Пишем ТОЛЬКО когда поля не
            // было: пришедшее из чужого файла — passthrough (его начало
            // мы и так читаем как таймкод материала), и переписывать его
            // своим замером значило бы «понимать наполовину».
            if active.available_range.is_none()
                && let Some(len) = media_len.filter(|l| l.0 > 0)
            {
                active.available_range = Some(range(Tick(0), len));
            }
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
            start: displaced_start(c, at),
            r#gen: c.r#gen.clone(),
            gen_auto: c.gen_auto.clone(),
            overrides: c.overrides.clone(),
            geometry: (!c.geometry.is_default()).then_some(c.geometry),
            geometry_pos: c.geometry_pos,
            geometry_auto: c.geometry_auto.clone(),
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
            default_shape: s.default_shape.unwrap_or_default(),
            conform: s.conform.unwrap_or_default(),
            settings_explicit: s.settings_explicit,
        })
        .unwrap_or_else(|| {
            notices.push("imported with default project settings".into());
            ProjectSettings {
                fps: guess_fps(tl),
                width: 1920,
                height: 1080,
                sample_rate: 48000,
                pitch_engine: Default::default(),
                conform: Default::default(),
                settings_explicit: false,
                default_shape: Default::default(),
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
            seq_facts: HashMap::new(),
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
        let mut seq_facts = std::mem::take(&mut cx.seq_facts);
        for (id, mut seq) in std::mem::take(&mut cx.seqs) {
            // сайдкар (Р.50): дорожки, порядок строк, морферы и камеры
            // ленты — то, чего OTIO не выражает. Не сошлось с файлом —
            // notice и чтение как есть, а не порча (паттерн KtLayers).
            match kt.as_ref().and_then(|k| k.sequences.iter().find(|s| s.id == id.0)) {
                Some(meta) => {
                    seq.name = meta.name.clone();
                    seq.morphers = meta.morphers.clone();
                    seq.cameras = meta.cameras.clone();
                    seq.overlap_layers = meta.overlap_layers;
                    seq.published = meta.published.clone();
                    let facts = seq_facts.remove(&id).unwrap_or_default();
                    seq.tracks = build_tracks(
                        &meta.tracks,
                        (
                            meta.video_lanes,
                            meta.audio_lanes,
                            meta.morph_lanes,
                            meta.camera_lanes,
                            &meta.lane_order,
                        ),
                        &facts,
                        reel_need(&seq),
                    );
                }
                None => {
                    cx.notices
                        .push(format!("nested reel '{}' has no kt sidecar — read as is", seq.name));
                    let facts = seq_facts.remove(&id).unwrap_or_default();
                    seq.tracks = build_tracks(&[], (0, 0, 0, 0, &[]), &facts, reel_need(&seq));
                }
            }
            p.sequences.push(seq);
        }
    }
    p.clips = std::mem::take(&mut root.clips);
    p.transitions = std::mem::take(&mut root.transitions);
    p.foreign = std::mem::take(&mut root.foreign);
    if root.end > p.timeline_end.unwrap_or(Tick::ZERO) {
        p.timeline_end = Some(root.end);
    }
    retained.tracks = track_extras;

    // конверт kt: транспорт/ui/бин/морферы/камеры
    let mut kt_tracks: Vec<crate::meta::KtTrack> = Vec::new();
    let mut kt_legacy: (u32, u32, u32, u32, Vec<(kt_model::LaneKind, u32)>) =
        (0, 0, 0, 0, Vec::new());
    let mut kt_names: Vec<(kt_model::LaneKind, u32, String)> = Vec::new();
    let mut kt_heights: Vec<(kt_model::LaneKind, u32, f32)> = Vec::new();
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
            let read = read_overlap_transitions(&meta.overlap_transitions, &s.clips, &mut next_tid);
            s.transitions.extend(read.0);
            notices.extend(read.1);
        }
        if let Some(tr) = kt.transport {
            p.region = tr.region.and_then(|(s, e)| kt_model::Region::normalized(Tick(s), Tick(e)));
            p.loop_enabled = tr.loop_enabled;
        }
        p.areas = kt.areas;
        // ЛЕГАСИ-порядок строк: своё поле, а из `ui.lane_order` — апгрейд
        // ещё более старого файла (оба только на чтение, Р.62)
        let mut legacy_order = kt.lane_order.clone();
        if legacy_order.is_empty()
            && let Some(u) = &kt.ui
        {
            legacy_order = u.lane_order.clone();
        }
        let legacy_names = kt.ui.as_ref().map(|u| u.lane_names.clone()).unwrap_or_default();
        let legacy_heights = kt.ui.as_ref().map(|u| u.lane_heights.clone()).unwrap_or_default();
        if let Some(mut ui) = kt.ui {
            // имена и высоты дорожек с Р.62 живут на дорожках-объектах
            ui.lane_names = Vec::new();
            ui.lane_heights = Vec::new();
            ui.bin_groups = kt.bin_groups;
            ui.bin_layout = kt.bin_layout;
            ui.bin_free = kt.bin_free;
            p.ui = Some(ui);
        }
        for bm in kt.bin_media {
            let path = resolve_url(base_dir, &bm.path);
            // адрес мог уже приехать с событием ленты — дубля быть не должно
            if !p.media.iter().any(|m| m.id == MediaId(bm.id)) {
                p.media.push(MediaRef::new(MediaId(bm.id), path));
            }
        }
        p.published = kt.published;
        p.morphers = kt.morphers;
        // морферы файла ДО v4 — прежняя семантика: патч на цели целиком,
        // время блока не действует (Р.35, односторонний апгрейд)
        if file_v < crate::meta::KT_META_V4 {
            for m in &mut p.morphers {
                kt_model::migrate_legacy_morpher(m);
            }
        }
        p.cameras = kt.cameras;
        // кейфреймы камер старого домена → общие кривые (Р.34 К3)
        for c in &mut p.cameras {
            kt_model::migrate_legacy_camera(c);
        }
        if let Some(end) = kt.end {
            p.timeline_end = Some(Tick(end));
        }
        if let Some(l) = &kt.layers {
            p.overlap_layers = !l.off;
        }
        // ДОРОЖКИ КОРНЯ (Р.62): конверт (порядок, свойства, служебные
        // строки) + факты самих треков файла (имя, `enabled`, цепочка);
        // ни того, ни другого нет — легаси-счётчики и `lane_order`.
        let (lv, la) = kt.lanes.unwrap_or((0, 0));
        kt_tracks = kt.tracks.clone();
        kt_legacy = (lv, la, kt.morph_lanes, kt.camera_lanes, legacy_order.clone());
        kt_names = legacy_names;
        kt_heights = legacy_heights;
    }

    // ДОРОЖКИ (Р.62) — ВНЕ конверта: чужой файл конверта не имеет вовсе,
    // а дорожки у него есть (имя, `enabled`, чужие эффекты на треке).
    let mut need = [0u32; 4];
    let kind_idx = |k: ClipKind| match k {
        ClipKind::Video => 0usize,
        ClipKind::Audio => 1usize,
    };
    for c in &p.clips {
        let i = kind_idx(c.kind);
        need[i] = need[i].max(c.lane + 1);
    }
    for f in &p.foreign {
        let i = kind_idx(f.kind);
        need[i] = need[i].max(f.lane + 1);
    }
    for m in &p.morphers {
        need[2] = need[2].max(m.lane + 1);
    }
    for c in &p.cameras {
        need[3] = need[3].max(c.lane + 1);
    }
    p.tracks = build_tracks(
        &kt_tracks,
        (kt_legacy.0, kt_legacy.1, kt_legacy.2, kt_legacy.3, &kt_legacy.4),
        &root.track_facts,
        need,
    );
    // ЛЕГАСИ-ПОРЯДОК строк (до Р.62 — своё поле либо `ui.lane_order`):
    // применяется ЕДИНСТВЕННОЙ дверью перестановки, и её перестановка
    // нужна чужому сырью треков (оно адресовано парой «вид, номер»)
    if kt_tracks.is_empty() && !kt_legacy.4.is_empty() {
        let perm = p.set_lane_order(&kt_legacy.4.clone());
        retained.remap_tracks(&perm);
    }
    // имена и высоты старого файла (`ui`) — на дорожки, односторонне
    for (k, l, name) in kt_names {
        if let Some(t) = p.track_at_mut(k, l)
            && t.name.is_empty()
        {
            t.name = name;
        }
    }
    for (k, l, h) in kt_heights {
        if let Some(t) = p.track_at_mut(k, l) {
            t.height = h;
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
    // КЛЮЧ САЙДЧЕЙНА (Р.65) читается ТЕРПИМО: ключ на дорожку, которой в
    // файле нет, и ключ на СОБЫТИИ — это «ключа нет», а не отказ читать.
    // Файл мог прийти из чужой руки или из будущей схемы; проект обязан
    // открыться, а прибор — звучать обычным компрессором.
    {
        let known: std::collections::HashSet<kt_model::TrackId> =
            p.tracks.iter().map(|t| t.id).collect();
        let clean = |fx: &mut Vec<kt_model::FxInstance>, allow: bool| {
            for f in fx.iter_mut() {
                if let kt_model::FxInstance::Compressor { key, .. } = f
                    && key.is_some_and(|k| !allow || !known.contains(&k))
                {
                    *key = None;
                }
            }
        };
        for t in p.tracks.iter_mut() {
            let audio = t.kind == kt_model::LaneKind::Audio;
            let mut fx = std::mem::take(&mut t.fx);
            clean(&mut fx, audio);
            t.fx = fx;
        }
        for c in p.clips.iter_mut() {
            clean(&mut c.fx, false);
        }
    }

    // НОМЕР ДОРОЖКИ = РАНГ СТРОКИ (Р.56, п.449). Чужие треки адресуются
    // парой (вид, номер), поэтому их сырьё переставляется ТЕМ ЖЕ
    // отображением: иначе metadata чужого трека уехала бы на соседний.
    let perm = p.normalize();
    retained.remap_tracks(&perm);
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
        .map(|c| {
            // прежнее булево сглаживание разворачивается в ОДНУ форму на
            // все отрезки (Р.61, D1b): значения после чтения обязаны
            // совпасть с прежними побитово. Правило ОДНО на оба читателя
            // (этот и `Deserialize for ParamCurve`) и знает про скорость
            // варпа — Р.63, см. `implied_shape_of`.
            let shape = kt_model::ParamCurve::implied_shape_of(&c.param, c.ease);
            kt_model::ParamCurve {
                param: c.param,
                keys: c
                    .keys
                    .into_iter()
                    .map(|k| {
                        // старый домен видео-кривых считался по прогрессу
                        // ТАЙМЛАЙНА — у реверсированного клипа он зеркален
                        // материалу
                        kt_model::AutoKey::shaped(
                            kt_model::material_from_progress(k.t, source_in, duration, reversed),
                            k.v,
                            shape,
                        )
                    })
                    .collect(),
                source: Default::default(),
            }
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
    /// Что сказали сами треки файла: имя, `enabled`, цепочка (Р.62).
    track_facts: Vec<(kt_model::LaneKind, u32, TrackFacts)>,
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
    /// Что сказали треки файла о дорожках КАЖДОЙ вложенной ленты (Р.62):
    /// сайдкар ленты приезжает позже, а факты рождаются при обходе.
    seq_facts: HashMap<MediaId, Vec<(kt_model::LaneKind, u32, TrackFacts)>>,
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
        // Р.62: `enabled: false` — это MUTE дорожки, а не «мы такого не
        // умеем»; цепочка дорожки читается тем же кодом, что у клипа, а
        // чужие эффекты остаются в `Retained` и вернутся в файл.
        let (fx, bypass, auto, foreign_fx) = read_chain_effects(&track.effects);
        tx.effects = foreign_fx;
        out.track_facts.push((
            kt_model::lane_key_of(kind),
            lane,
            TrackFacts { name: track.name.clone(), enabled: track.enabled, fx, bypass, auto },
        ));
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
                            let (kind, audio, auto, invert) =
                                take_transition_kt(&mut meta, cx.notices);
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
    let both = body.clips.iter().any(|c| c.kind == ClipKind::Video)
        && body.clips.iter().any(|c| c.kind == ClipKind::Audio);
    seq.clips = body.clips;
    seq.transitions = body.transitions;
    cx.seq_facts.insert(addr, std::mem::take(&mut body.track_facts));
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
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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

/// Прежний эффект `kt:pan` (упразднён Р.62) → дополнительный узел
/// «Геометрия» на том же слоте: индексы слотов не двигаются, кривые
/// `pan_x`/`pan_y` переезжают на `pos_x`/`pos_y` (пересчёт `pan_to_pos`
/// при зуме базы), `zoom` остаётся. Режим прежней рамки не переносится:
/// все три давали один кадр (F-MED-004).
pub(crate) fn migrate_pan_effect(
    metadata: &Map<String, Value>,
) -> Option<(kt_model::FxInstance, Vec<kt_model::ParamCurve>)> {
    let kt = metadata.get(crate::meta::KT_NS)?.as_object()?;
    let num = |k: &str, d: f64| kt.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
    let (px, py, z) = (num("pan_x", 0.0), num("pan_y", 0.0), num("zoom", 1.0));
    let inst = kt_model::FxInstance::geometry_at(
        kt_model::pan_to_pos(px, z),
        kt_model::pan_to_pos(py, z),
        z,
    );
    let mut auto: Vec<kt_model::ParamCurve> =
        kt.get("auto").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    for c in auto.iter_mut() {
        let flip = match c.param.as_str() {
            "pan_x" => Some("pos_x"),
            "pan_y" => Some("pos_y"),
            _ => None,
        };
        if let Some(name) = flip {
            c.param = name.into();
            for k in c.keys.iter_mut() {
                k.v = kt_model::pan_to_pos(k.v, z);
            }
        }
    }
    Some((inst, auto))
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
            EffectNode::Effect(eff) if eff.effect_name == "kt:pan" => {
                if let Some((inst, curves)) = migrate_pan_effect(&eff.metadata) {
                    fx.push(inst);
                    bypass.push(!eff.enabled);
                    auto.push(curves);
                }
            }
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

/// ДОРОЖКИ ленты из конверта и из самих треков файла (Р.62).
///
/// Конверт авторитетен для НАШИХ файлов: он несёт порядок строк, служебные
/// строки и свойства, которых в OTIO нет. Чужой файл конверта не имеет —
/// тогда дорожки строятся из легаси-счётчиков, а имя и `enabled` берутся с
/// треков (`enabled: false` читается как mute: чужая выключенная дорожка
/// обязана остаться выключенной, а не «включиться» молча).
fn build_tracks(
    list: &[crate::meta::KtTrack],
    legacy: (u32, u32, u32, u32, &[(kt_model::LaneKind, u32)]),
    per_track: &[(kt_model::LaneKind, u32, TrackFacts)],
    need: [u32; 4],
) -> Vec<kt_model::Track> {
    use kt_model::LaneKind as K;
    let mut out: Vec<kt_model::Track> = if list.is_empty() {
        // чужой или старый файл: натуральный порядок из счётчиков
        let n = [
            legacy.0.max(need[0]).max(1),
            legacy.1.max(need[1]).max(1),
            legacy.2.max(need[2]),
            legacy.3.max(need[3]),
        ];
        let mut v = Vec::new();
        for (i, kind) in K::ALL.into_iter().enumerate() {
            for _ in 0..n[i] {
                v.push(kt_model::Track::new(kt_model::TrackId(0), kind));
            }
        }
        v
    } else {
        list.iter()
            .map(|t| {
                let mut x = kt_model::Track::new(kt_model::TrackId(t.id), t.kind);
                x.name = t.name.clone();
                x.height = t.height;
                x.mute = t.mute;
                x.solo = t.solo;
                x.lock = t.lock;
                x.gain_db = t.gain_db;
                x.opacity = t.opacity;
                x.blend = t.blend;
                x.fx_off = t.fx_off;
                x
            })
            .collect()
    };
    // факты с самих треков файла: имя, `enabled` и цепочка видео/аудио
    for (kind, lane, facts) in per_track {
        let mut rank = 0u32;
        for t in out.iter_mut() {
            if t.kind != *kind {
                continue;
            }
            if rank == *lane {
                if !facts.name.is_empty() {
                    t.name = facts.name.clone();
                }
                t.mute = t.mute || !facts.enabled;
                t.fx = facts.fx.clone();
                t.fx_bypass = facts.bypass.clone();
                t.fx_auto = facts.auto.clone();
                break;
            }
            rank += 1;
        }
    }
    // дорожек не меньше, чем занято содержимым
    for (i, kind) in K::ALL.into_iter().enumerate() {
        let have = out.iter().filter(|t| t.kind == kind).count() as u32;
        let want = need[i].max(if i < 2 { 1 } else { 0 });
        for _ in have..want {
            let at = kt_model::natural_insert_pos(&out, kind);
            out.insert(at, kt_model::Track::new(kt_model::TrackId(0), kind));
        }
    }
    out
}

/// Сколько дорожек каждого вида требует содержимое ленты.
fn reel_need(seq: &Sequence) -> [u32; 4] {
    let mut n = [0u32; 4];
    for c in &seq.clips {
        let i = match c.kind {
            ClipKind::Video => 0,
            ClipKind::Audio => 1,
        };
        n[i] = n[i].max(c.lane + 1);
    }
    for m in &seq.morphers {
        n[2] = n[2].max(m.lane + 1);
    }
    for c in &seq.cameras {
        n[3] = n[3].max(c.lane + 1);
    }
    n
}

/// Что о дорожке сказал сам OTIO-трек.
#[derive(Default, Clone)]
struct TrackFacts {
    name: String,
    enabled: bool,
    fx: Vec<kt_model::FxInstance>,
    bypass: Vec<bool>,
    auto: Vec<Vec<kt_model::ParamCurve>>,
}

/// Свои растр и fps ленты из конверта `kt` (Р.62); нет ключей — проект.
fn read_reel_settings(metadata: &Map<String, Value>, seq: &mut Sequence) {
    let Some(kt) = metadata.get(crate::meta::KT_NS).and_then(|v| v.as_object()) else {
        return;
    };
    if let Some(r) = kt.get("raster").and_then(|v| v.as_array())
        && r.len() == 2
        && let (Some(w), Some(h)) = (r[0].as_u64(), r[1].as_u64())
    {
        seq.raster = Some((w as u32, h as u32));
    }
    if let Some(f) = kt.get("fps").and_then(|v| v.as_u64()) {
        seq.fps = Some(f as u32);
    }
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
            read_reel_settings(&st.metadata, &mut seq);
            seq.clips = std::mem::take(&mut body.clips);
            seq.transitions = std::mem::take(&mut body.transitions);
            cx.seq_facts.insert(addr, std::mem::take(&mut body.track_facts));
            cx.seqs.push((addr, seq));
        }
        None => {
            let mut seq = Sequence::new(seq_id, st.name.clone());
            read_reel_settings(&st.metadata, &mut seq);
            seq.clips = std::mem::take(&mut body.clips);
            seq.transitions = std::mem::take(&mut body.transitions);
            cx.seq_facts.insert(seq_id, std::mem::take(&mut body.track_facts));
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
    // истинное время — из конверта, если трек его выразить не смог (Р.58)
    let start = meta.as_ref().and_then(|m| m.start).map(Tick).unwrap_or(start);
    Ok(ClipEvent {
        r#gen: None,
        // Переопределения ЭКЗЕМПЛЯРА ленты — из нашего конверта (Р.59,
        // стадия 3). Чужой файл их не несёт и нести не может.
        overrides: meta.as_ref().map(|m| m.overrides.clone()).unwrap_or_default(),
        gen_auto: Vec::new(),
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
        geometry: meta.as_ref().and_then(|m| m.geometry).unwrap_or_default(),
        geometry_pos: meta.as_ref().map(|m| m.geometry_pos).unwrap_or(0),
        geometry_auto: meta.as_ref().map(|m| m.geometry_auto.clone()).unwrap_or_default(),
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
        // ДЛИНА МАТЕРИАЛА — из стандартного `available_range` внешней
        // ссылки (Р.63 п.2). У генератора её нет и быть не может: там
        // `available_range` пишется потреблённым куском, а не длиной
        // источника, и принять его за длину значило бы поставить
        // генератору упор трима из ниоткуда.
        let len = match c.active_media() {
            Some(MediaRefNode::External(er)) => {
                er.available_range.map(|r| r.duration.to_ticks()).filter(|d| d.0 > 0)
            }
            _ => None,
        };
        media_list.push(match len {
            Some(d) => MediaRef::with_duration(media, resolve_url(base_dir, &url), d),
            None => MediaRef::new(media, resolve_url(base_dir, &url)),
        });
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
            EffectNode::Effect(eff) if eff.effect_name == "kt:pan" => {
                // прежний Pan (Р.62) — узел «Геометрия» на том же слоте
                if let Some((inst, curves)) = migrate_pan_effect(&eff.metadata) {
                    fx.push(inst);
                    fx_auto.push(serde_json::to_value(curves).ok());
                    bypass.push(!eff.enabled);
                }
            }
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
    // истинное время события — из конверта, если трек его выразить не смог
    // (Р.58, п.462); чужому файлу верим только позиции в треке
    let start = match &meta {
        Some(m) if is_ours => m.start.map(Tick).unwrap_or(start),
        _ => start,
    };
    let mut ev = ClipEvent {
        // Обычное событие переопределять нечего (`validate` это и
        // требует): набор пуст по построению.
        overrides: Vec::new(),
        // Значения генератора — из НАШЕГО конверта, если он есть (Р.59,
        // стадия 2). Нет его — клип остаётся пустым, и `normalize()`
        // достаёт значения из пути медиа: файлы Р.52..Р.58 держали их
        // именно там, и миграция живёт в ОДНОМ месте.
        r#gen: meta
            .as_ref()
            .filter(|_| is_ours && kt_model::parse_gen_path(&url).is_some())
            .and_then(|m| m.r#gen.clone()),
        gen_auto: meta
            .as_ref()
            .filter(|_| is_ours && kt_model::parse_gen_path(&url).is_some())
            .map(|m| m.gen_auto.clone())
            .unwrap_or_default(),
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
        geometry: meta.as_ref().and_then(|m| m.geometry).unwrap_or_default(),
        geometry_pos: meta.as_ref().map(|m| m.geometry_pos).unwrap_or(0),
        geometry_auto: meta.as_ref().map(|m| m.geometry_auto.clone()).unwrap_or_default(),
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

// ---- Retained ↔ JSON (Р.70) ---------------------------------------------------
//
// Файл проекта свой, а чужой passthrough импортированного файла обмена едет в
// нём отдельным полем и возвращается при экспорте: потерять чужую работу
// молча нельзя. Перевод идёт serde-мостами узлов (`node_serde!`), которые
// уже пишут и читают OTIO-объекты, — своего слоя между `Retained` и JSON нет.

/// `HashMap` в JSON — ПАРЫ `[ключ, значение]`, отсортированные по ключу.
///
/// Две причины, и обе обязательные: JSON-объект не умеет нестроковых ключей
/// (`(ClipKind, u32)`, `(Option<ClipId>, Option<ClipId>)`), а порядок обхода
/// `HashMap` случаен — без сортировки одна и та же модель давала бы разные
/// байты, и каждое сохранение было бы ложным диффом в git. Сортировка по
/// СЕРИАЛИЗОВАННОМУ ключу: `Ord` у ключей не требуется, а их JSON
/// детерминирован.
pub(crate) mod sorted_pairs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::HashMap;
    use std::hash::Hash;

    pub fn serialize<K, V, S>(m: &HashMap<K, V>, s: S) -> Result<S::Ok, S::Error>
    where
        K: Serialize,
        V: Serialize,
        S: Serializer,
    {
        let mut pairs: Vec<(&K, &V)> = m.iter().collect();
        pairs.sort_by_cached_key(|(k, _)| serde_json::to_string(k).unwrap_or_default());
        pairs.serialize(s)
    }

    pub fn deserialize<'de, K, V, D>(d: D) -> Result<HashMap<K, V>, D::Error>
    where
        K: Deserialize<'de> + Eq + Hash,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Ok(Vec::<(K, V)>::deserialize(d)?.into_iter().collect())
    }
}

impl Retained {
    /// Passthrough как непрозрачный JSON для файла проекта. Детерминирован:
    /// карты документа insertion-ordered (как в исходнике), `HashMap`-поля —
    /// отсортированными парами.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("Retained состоит из JSON-значений и узлов схемы")
    }

    /// Обратно из файла проекта. Битое поле — ошибка, а не пустой passthrough:
    /// молча потерять чужую работу хуже, чем отказать.
    pub fn from_json(v: &Value) -> Result<Retained, Error> {
        Ok(serde_json::from_value(v.clone())?)
    }
}
