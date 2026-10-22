//! Timeline (OTIO) ↔ kt_model::Project + Retained (passthrough remainders).
//!
//! Mapping (R.24, test 152): the video lane is inverted into the track order
//! (OTIO composites later tracks ON TOP of earlier ones, while our lane 0 is the top);
//! gaps → Gap; FX → standard Effect.1 with effect_name “kt:<kind>” and
//! parameters in metadata.kt; reversed → LinearTimeWarp{-1} first;
//! transitions → Transition.1 (SMPTE_Dissolve); markers → Marker.2 of the root
//! Stack; everything outside the model — the metadata.kt envelope (meta.rs). Foreign data is kept
//! in Retained and written back: clips — by ClipId, tracks/gaps/transitions —
//! by position at load time (a documented v1 limitation).

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

/// Import remark — shown to the user as a note
pub type Notice = String;

/// (extra, metadata) of an object — a passthrough pair
pub type ExtraMeta = (Map<String, Value>, Map<String, Value>);
/// (transition_type, metadata, extra) of a foreign transition
pub type TransitionExtra = (String, Map<String, Value>, Map<String, Value>);

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ClipExtra {
    pub name: String,
    pub extra: Map<String, Value>,
    pub meta_foreign: Map<String, Value>,
    /// Foreign effects (including the not-understood time warps) with their original index —
    /// are re-interleaved into the chain on writing
    pub effects_foreign: Vec<(usize, EffectNode)>,
    /// Markers hanging on a clip (we do not create our own — passthrough)
    pub markers: Vec<MarkerNode>,
    /// ALL media_references of the clip as in the file (including the active one and `null`:
    /// a clip without a media binding is legal). On writing the active key
    /// is replaced with our target_url — but only if the material exists; otherwise
    /// the map is written untouched (a foreign `null` does not turn into a reference).
    pub media_refs: IndexMap<String, Option<MediaRefNode>>,
    pub active_key: String,
    /// Unknown fields of OUR clip envelope kt (future versions, R.29).
    pub kt_extra: Map<String, Value>,
}

/// Media references are a property of the MEDIA, not the clip. Key `Retained.media`.
///
/// Why separate from `ClipExtra` (R.49, item 356). `available_range` describes
/// the file: where its start is in its own coordinates. For broadcast and
/// cinema files it is not at zero but in the burned-in timecode (for Sol Levante — exactly
/// one hour). A clip BORN in the editor (the right half of a split, a bake piece,
/// a paste, an event from the bin) has no `ClipExtra` of its own — and on writing its
/// `source_in` went to the file without this hour, and on the next load the hour
/// was subtracted once more. The media level fixes this whole half at once.
/// Foreign data on a marker: what is absent from `kt_model::Marker`.
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
    /// extra/metadata of gaps by their ordinal number in the track.
    pub gaps: Vec<(usize, ExtraMeta)>,
    /// Foreign data hanging on the TRACK itself (R.49, item 358). We have no track effects
    /// or markers of our own, but the neighbours do — and the round trip lost them.
    pub effects: Vec<EffectNode>,
    pub markers: Vec<MarkerNode>,
    /// A disabled track. We do not support this and do not invent it: the value
    /// goes back to the file as it is, and the user gets a notice —
    /// otherwise they look at a track that cannot be heard in the foreign editor.
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

/// Passthrough remainders of the loaded file. Outside undo (not editable);
/// for a new project — Default.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Retained {
    pub timeline_name: String,
    pub timeline_extra: Map<String, Value>,
    /// Foreign metadata namespaces of the Timeline (order preserved, kt excluded).
    pub timeline_meta_foreign: Map<String, Value>,
    pub stack_name: String,
    pub stack_extra: Map<String, Value>,
    pub stack_metadata: Map<String, Value>,
    pub global_start_time: Option<RationalTime>,
    // HashMap fields (R.70): in the project file — PAIRS, sorted by key.
    // JSON cannot have non-string keys, and HashMap iteration order is random —
    // without sorting every save would give a spurious diff in git.
    #[serde(with = "sorted_pairs")]
    pub tracks: HashMap<(ClipKind, u32), TrackExtra>,
    #[serde(with = "sorted_pairs")]
    pub clips: HashMap<ClipId, ClipExtra>,
    /// Media references by MEDIA are shared by all its clips (item 356).
    #[serde(with = "sorted_pairs")]
    pub media: HashMap<kt_model::MediaId, MediaRefs>,
    /// For a pair of clips at a joint.
    /// The key is the pair of transition ends; for a fade (R.25) one end is None.
    #[serde(with = "sorted_pairs")]
    pub transitions: HashMap<(Option<ClipId>, Option<ClipId>), TransitionExtra>,
    /// Raw data of foreign placeholders by ForeignEvent.key.
    #[serde(with = "sorted_pairs")]
    pub foreign: HashMap<u64, RawObject>,
    /// Foreign markers of the root Stack (ours come from the model).
    pub stack_markers_foreign: Vec<MarkerNode>,
    /// Foreign data on OUR markers of the root Stack: comment, metadata, extra
    /// (R.49, item 358). The model does not know them — for us a marker is time, name and
    /// colour — and the round trip lost them: a foreign editor put a comment
    /// and its own namespace there, and after our save they disappeared.
    /// The key is the ordinal number of the marker in OUR list at read time.
    pub stack_marker_extras: Vec<(usize, MarkerExtra)>,
    /// Foreign objects at the level of the CHILDREN of the root Stack — not `Track`.
    ///
    /// Before R.50 they went into `foreign` by key, but the writer never
    /// retrieved them FROM THERE: raw data is returned only by
    /// `ForeignEvent` placeholders living ON tracks, while the root
    /// child has no track. The round trip lost such an object silently. We store it with
    /// its POSITION among the children and return it to that position.
    pub root_foreign: Vec<(usize, RawObject)>,
    /// Unknown fields of OUR kt envelope (future versions, R.29):
    /// passthrough — a v2 build does not lose the additions of v3.
    pub timeline_kt_extra: Map<String, Value>,
}

impl Retained {
    /// Reorder the raw data of foreign TRACKS after the renumbering of tracks
    /// (R.56, item 449). The key is a semantic pair (view, number), and with
    /// “number = row rank” it changes on reading: had we not reordered
    /// the map, the metadata of a foreign track would silently move to a neighbouring track.
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
    // The content of the root is written by the SAME code as the content of a nested
    // reel (R.50, item 382): previously these were three closures, hard-wired
    // to `p`.
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
    // foreign root children return to THEIR positions (R.50): previously they
    // went to `foreign` by a key that the writer never gets
    let mut children = children;
    for (pos, raw) in &retained.root_foreign {
        let at = (*pos).min(children.len());
        children.insert(at, Composable::Unknown(raw.clone()));
    }

    // markers — on the root Stack (ours + foreign passthrough)
    let mut markers: Vec<MarkerNode> = p
        .markers
        .iter()
        .enumerate()
        .map(|(i, m)| {
            // foreign data on a marker returns to its place (item 358)
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

    // the timeline’s metadata.kt
    let mut meta = retained.timeline_meta_foreign.clone();
    let mut morphers = p.morphers.clone();
    morphers.sort_by_key(|m| m.id);
    let mut cameras = p.cameras.clone();
    cameras.sort_by_key(|c| c.id);
    // “used” is counted over the WHOLE TREE (R.50): a file sitting only inside a reel
    // would get into both bin_media and the clips of that reel — on reading this would give
    // a duplicate address and a load rejection.
    let used: std::collections::HashSet<MediaId> = p
        .clips
        .iter()
        .chain(p.sequences.iter().flat_map(|s| s.clips.iter()))
        .map(|c| c.media)
        .collect();
    let kt = KtTimelineMeta {
        // version BY CONTENT (R.29/R.30): cameras and parameter automation raise it to v2
        // (an old reader would silently lose them on rewrite),
        // without them — v1
        v: {
            let has_auto = p
                .clips
                .iter()
                .any(|c| c.fx_auto.iter().any(|v| v.iter().any(|c| !c.keys.is_empty())))
                // camera curves — the same mechanism and the same version
                || cameras.iter().any(|c| !c.auto.is_empty());
            // the toggles are v3 too: an old build does not know about them and
            // would play the disabled one (the “silent loss” rule, R.29)
            let has_off = p.clips.iter().any(|c| c.fx_off) || p.morphers.iter().any(|m| !m.enabled);
            // the transition kind and the audio curve are v3 too: an old build
            // would read a wipe as a crossfade and silently spoil the edit
            let has_tr = p.transitions.iter().any(|t| {
                t.kind != TransitionKind::Crossfade
                    || t.audio != AudioCurve::default()
                    || !t.auto.is_empty()
            });
            let has_cams = !cameras.is_empty() || p.lanes(kt_model::LaneKind::Camera) > 0;
            // version BY CONTENT: the curves of the new domain are v3, cameras —
            // v2, otherwise v1 (a file without our novelties opens in old
            // builds and stays byte-stable)
            // morpher layer (R.35): acts by time or aims at
            // not a frozen list — an old build would play the wrong thing
            let has_layer = !p.areas.is_empty()
                || p.morphers
                    .iter()
                    .any(|m| m.timed || !matches!(m.targets, kt_model::MorphTargets::Clips { .. }));
            // reel curves (R.38 item 13) and its strength with fades (item 12): an old
            // build would render the layer with constant parameters instead of the
            // animated ones, and a “layer at 30%” as a whole
            let has_morph_v5 = p.morphers.iter().any(|m| {
                m.mixes()
                    || m.patch
                        .video_auto
                        .iter()
                        .chain(m.patch.audio_auto.iter())
                        .any(|a| !a.is_empty())
            });
            // R.51 transitions (inversion, overlapping) — across ALL reels
            // of the document: an old build would play the transition uninverted
            // (and the overlapping one as a hard cut) and would keep the envelope
            // untouched — the corruption could not be less noticeable
            let v7_reel = |reel: &dyn kt_model::Reel| {
                reel.transitions().iter().any(|t| t.invert || t.overlap_pair(reel).is_some())
            };
            let has_v7 =
                v7_reel(p) || p.sequences.iter().any(|s| v7_reel(s as &dyn kt_model::Reel));
            // an event left of zero (R.58, item 462): an old build would put it
            // at zero and shift all the reel’s content after it — silently
            let has_v8 = p
                .clips
                .iter()
                .chain(p.sequences.iter().flat_map(|s| s.clips.iter()))
                .any(|c| c.start < Tick::ZERO);
            // nested reels (R.50): an old build would read an attachment
            // as a grey placeholder (item 158) and on the first rewrite would return
            // the live edit to the file as dead raw data
            // generator parameters on an EVENT (R.59, stage 2): an old
            // build would look for them in the media path, where they no longer exist,
            // and would show an empty title — and in all places at once
            //
            // Public composition parameters and per-instance overrides (R.59, stage 3):
            // the SAME version — the round’s decision “one version per round, raised BY CONTENT” —
            // and the predicate must be ONE place. An old build would play
            // every instance with template values: ten news editions
            // with one headline
            // and not a single visible sign.
            let has_v9 = p
                .clips
                .iter()
                .chain(p.sequences.iter().flat_map(|s| s.clips.iter()))
                .any(|c| c.r#gen.is_some() || !c.overrides.is_empty())
                || !p.published.is_empty()
                || p.sequences.iter().any(|s| !s.published.is_empty());
            // a segment shape not expressible by the former boolean field (R.61):
            // a uniform “third order” or “straight” does NOT move the version —
            // they are written with the same `ease` as before, byte for byte (D1b).
            // Everything else an old build would read as uniform: a zigzag
            // would become straight, a step a smooth move, and not a single
            // sign of corruption.
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
            // v11 (R.62): the project rule is not “fit”, explicit settings,
            // a leading node with a non-default value or not the first one, a “Geometry” node
            // in the chain — an old build would lose or fail to read them
            let clip_needs_v11 = |c: &kt_model::ClipEvent| {
                !c.geometry.is_default()
                    || c.geometry_pos != 0
                    || !c.geometry_auto.is_empty()
                    || c.fx.iter().any(|f| matches!(f, kt_model::FxInstance::Geometry { .. }))
            };
            // a published control of the LEADING GEOMETRY (R.63) — the same
            // novelty as R.62: the address `owner: "geometry"` an old build
            // would not parse at all, and the version must say so, not a
            // parse error
            let reel_needs_v11 = |reel: &dyn kt_model::Reel| {
                reel.clips().iter().any(clip_needs_v11)
                    || reel
                        .published()
                        .iter()
                        .any(|p| matches!(p.target.owner, kt_model::ParamOwner::Geometry))
            };
            // a track-object with any non-default value — also a novelty (R.62,
            // change B): an old build would silently lose the mute, level, mode and
            // the track chain
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
            // v12 (R.65): the assigned sidechain KEY in the track chain
            let track_needs_v12 =
                |t: &kt_model::Track| kt_model::sidechain_keys(&t.fx).next().is_some();
            let has_v12 = p.tracks.iter().any(track_needs_v12)
                || p.sequences.iter().any(|s| s.tracks.iter().any(track_needs_v12));
            // v13 (R.67): segment formula, layer over the keys, `k` in the formula
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
            // a default is not written to the file (skip_serializing_if)
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
        // track names since R.62 live on the tracks themselves (the file’s `Track.name`
        // and `kt.tracks` for service rows), they are no longer in `ui`; bin fields
        // too, separately (kt.bin_*) — one home for each fact
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
        // length beyond the material (empty tracks are not written to the file)
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
            // we write only when there is something to say: without overlaps and with the
            // option enabled the file stays byte-for-byte as before
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
                            // as for the root: we write only when the layer is not single
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

/// How many OTIO tracks ONE of OUR tracks occupies (R.28, layering).
///
/// A track without content, which was not in the source file, is not written
/// at all (R.25) — otherwise the layer map would not match the file and on reading
/// would be discarded as a whole.
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

/// Event effects outward: reverse — LTW(-1) first, then our chain with
/// curves in its own envelope, foreign ones — by their original indices. ONE code for a
/// regular event and for an attachment (an attachment has no foreign effects).
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
        // automation (R.30): slot curves travel under the key "auto" inside the
        // effect’s kt envelope; FxInstance ignores unknown fields, an old reader
        // would lose them silently → the file rises to v2
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

/// The content of ONE reel (R.50, item 382): the project root and a nested
/// sequence are written by the SAME code.
///
/// Before R.50 these were three closures inside `project_to_timeline`, hard-wired
/// to `&Project`. Our reels come in two kinds, while the rule of
/// layering, gaps, transitions and track names is one; writing it out a second
/// time would mean a second truth about the format.
/// Reading the overlapping transitions of a reel from its envelope (R.51, item 399):
/// returns the events and remarks. Writing with an unknown clip — a notice
/// and a skip (the KtLayers pattern: no match — read as is, not corruption);
/// the pair’s geometry is checked further by validate.
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

/// Overlapping transitions of a reel — into its envelope (R.51, item 399): the pair lives in
/// different neighbouring Tracks and cannot be expressed by a child of a sequential track.
/// Sorting by pair — write determinism (t144).
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
    /// Track-objects of a reel (R.62): name, toggles and chain travel to
    /// the OTIO track itself, the order and the rest — into the envelope.
    tracks: &'a [kt_model::Track],
    timeline_end: Tick,
    /// Foreign raw data. Our reels have none of it — an empty `Retained`.
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

/// The FX chain into standard `Effect.1` — ONE code for an event, an attachment and
/// a TRACK (R.62): if they diverged, the track chain would be read differently.
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

/// Parse the track’s `Effect.1`: our `kt:` effects into the chain, foreign ones —
/// back to the caller (they return to the file untouched, item 358).
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

/// The envelope of a reel’s tracks (R.62). Empty — nothing to write: the order
/// is natural and all tracks have no properties, so the file stays byte-for-byte as before.
fn track_list(tracks: &[kt_model::Track]) -> Vec<crate::meta::KtTrack> {
    let natural = {
        let mut v: Vec<kt_model::LaneKind> = Vec::new();
        for k in kt_model::LaneKind::ALL {
            v.extend(tracks.iter().filter(|t| t.kind == k).map(|t| t.kind));
        }
        v
    };
    let order_natural = tracks.iter().map(|t| t.kind).eq(natural.iter().copied());
    // the name and chain of VIDEO/AUDIO live on the file’s own tracks; service
    // rows have no track of their own, so their name is here —
    // there is no need to write the envelope if the order is natural and there is
    // nothing to say about tracks: the video/audio name lives on the file’s own track
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
                // video/audio name — in the file’s `Track.name` (one home for the fact)
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
    // a transition is written BEFORE its incoming clip; a fade-out (the right clip
    // is absent) — AFTER its outgoing one (R.25). An OVERLAPPING transition (R.51) is not
    // written into the track’s children at all: its pair lies in different Tracks — it
    // travels in the reel’s envelope (`overlap_transitions`).
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

    // Layering (R.28): in OTIO a clip’s position is set by the ORDER of the track’s
    // children, so overlapping clips of one of our tracks cannot be expressed in one
    // Track — they would have to be written in a row, and a foreign editor would see
    // a scrambled edit. We distribute greedily by layers: a clip goes into the first
    // layer where it intersects no one; each layer is written as a separate
    // Track, and the number of OUR track travels in `metadata.kt.lane` of the track — by it
    // we assemble the track back (the owner’s decision, R.28).
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
        // foreign extra/metadata/gaps belong to the SOURCE track — they are carried by
        // layer 0, otherwise layering would duplicate them onto every layer
        let tx = if layer == 0 {
            r.retained.tracks.get(&(kind, lane)).cloned().unwrap_or_default()
        } else {
            TrackExtra::default()
        };
        let known = r.retained.tracks.contains_key(&(kind, lane));
        let has_content = r.clips.iter().any(|c| c.kind == kind && c.lane == lane)
            || r.foreign.iter().any(|f| f.kind == kind && f.lane == lane);
        if !known && !has_content {
            return; // an empty track which was not in the source file
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
        // `cursor` — the position IN THE OTIO TRACK (sequential), not the model time.
        // Usually they coincide: a gap is appended exactly up to
        // `start`. They diverge for an event LEFT OF ZERO inside a reel (R.58,
        // item 462) and for everything that follows it and would land on the track cursor:
        // such an event is written AT the cursor, while the true `start` travels into the
        // envelope (`kt.start`), from which our reader takes it.
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
                        // The transition kind outward (R.33): a crossfade is the standard
                        // SMPTE_Dissolve, wipes are SMPTE_Wipe, the rest —
                        // Custom. A foreign kind that arrived in Retained is not
                        // touched: the rule “what we do not understand we return as
                        // it was” (t152).
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
                        // our parameters — into their own envelope; defaults are not
                        // written, so a plain crossfade is byte-stable
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
                            // a default of false is not written (byte stability)
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
                    // An attachment is written by a `Stack` as a child of a `Track` with a
                    // `source_range` — exactly the same technique as
                    // Premiere and FCP do. The body is inlined into EVERY
                    // instance: there is no object identity in OTIO JSON,
                    // and a foreign editor must see the edit, not a reference.
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
            // the OTIO track advances by the LENGTH of the element from its place in the
            // track; when it matches the model, this is the former `cursor = end`
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
        // only an EXPLICIT track name (the default V1/A1 labels are not written to the file —
        // otherwise a foreign/new project would grow fictitious names after a round trip)
        let lk = kt_model::lane_key_of(kind);
        let t = r.track(lk, lane);
        let name = t.map(|t| t.name.clone()).unwrap_or_default();
        // THE CHAIN of a track — as standard `Effect.1` by the same code as for
        // a clip (R.62); foreign effects return to their place (item 358).
        // A layering layer carries only foreign data: the chain belongs to layer 0.
        let mut effects = tx.effects.clone();
        if layer == 0
            && let Some(t) = t
        {
            effects.extend(chain_effects(&t.fx, &t.fx_bypass, &t.fx_auto));
        }
        children.push(Composable::Track(Track {
            name: if layer > 0 {
                String::new() // the track name is carried by layer 0
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
            // `enabled = !mute` (R.62): a foreign reader sees a muted
            // track honestly, rather than “Kinetren has no per-track mute”
            enabled: tx.enabled && !t.is_some_and(|t| t.mute),
            metadata: tx.metadata.clone(),
            extra: tx.extra.clone(),
        }));
    };
    // Edit length (R.25): a track shorter than the end of the reel is padded
    // with a trailing Gap — so a foreign file sets the length and so it is read
    // back. A track that was not in the file and in which there is nothing
    // is not written at all (the number of tracks lives in metadata.kt).
    // Video bottom to top (OTIO: a later track on top), then audio in order.
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

/// An attachment as a `Stack`, a child of a `Track` (R.50).
///
/// The instance window is the stack’s `source_range`, exactly as for a clip: OTIO treats
/// it the same way, and a foreign editor shows the trimmed attachment correctly
/// without a single line of ours.
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
        // deeper than the limit we do not write AT ALL: a file we could not
        // read back is worse than a missing attachment. The model would not pass
        // such a project (validate), so there is no way to get here —
        // this is a safety net for manual edits of the .otio.
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
    // the reel’s address — this is the key by which instances collapse on reading
    crate::meta::kt_entry_mut(&mut meta).insert("seq".into(), Value::from(seq.id.0));
    // the reel’s own raster and fps (R.62) — only when they are set
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

/// `GeneratorReference` (ours, `kt.<tag>`) → the canonical `gen:` path.
/// An unknown tag or broken parameters — `None` (compatibility
/// is forward-only: a stub instead of a generator from a future build).
fn generator_ref_path(gr: &GeneratorReference) -> Option<String> {
    let tag = gr
        .generator_kind
        .strip_prefix("kt.")
        .or_else(|| crate::generator_alias(&gr.generator_kind))?;
    let mut fields = gr.parameters.clone();
    fields.insert("kind".into(), Value::String(tag.to_string()));
    // LEGACY title-animation mode (R.53) — via the fields `anim`/`anim_secs`.
    // The kind no longer carries them, and serde would simply ignore them, so
    // they move into the path as the tokens `am`/`ams`: from there the
    // SINGLE migration (`Project::migrate_gen_to_events`) picks them up, and
    // turns the mode into curves. There is never a second copy of these formulas.
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

/// The legacy mode’s name from the file (`typewriter`) → path token (`tw`).
fn legacy_anim_token(name: &str) -> Option<&'static str> {
    kt_model::TextAnim::ALL
        .into_iter()
        .find(|a| format!("{a:?}").to_lowercase() == name)
        .map(|a| a.token())
}

/// The `GeneratorKind` fields as a `parameters` dictionary (without the service `kind` —
/// it travels as the `generator_kind` string). The key order is deterministic,
/// by the order of the struct’s fields.
fn generator_parameters(kind: &kt_model::GeneratorKind) -> Map<String, Value> {
    let Ok(Value::Object(mut fields)) = serde_json::to_value(kind) else {
        unreachable!("GeneratorKind serializes to an object");
    };
    fields.shift_remove("kind");
    fields
}

/// `kt.start` is written ONLY when the place in the track (`at`) does not equal the model
/// time (R.58, item 462). An ordinary edit — `None`, and files without the field
/// are byte-stable; the guard — t638 and the golden fixtures.
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
    // A clip born in the editor has no envelope — its media references
    // are taken from the MEDIA (item 356). Otherwise `available_range` would be lost, and along
    // with it the file’s starting timecode: `source_in` would drift by an hour.
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
        // The values come FROM THE EVENT (R.59, stage 2); the path carries only the kind,
        // and it is parsed only for a clip without values (a foreign
        // import, a broken kt section).
        if let Some(kind) = c.r#gen.clone().or_else(|| kt_model::parse_gen_path(target_url)) {
            // a generator is written by the FIRST-CLASS OTIO schema (R.52):
            // the name/metadata/extra of the former node are kept, the kind and
            // parameters are overwritten by the canon. `available_range`
            // is MANDATORY (the schema contract — the SMPTE fixture carries 50 frames
            // although a generator has no duration): we write exactly what THIS clip
            // consumes — idempotently.
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
            // available_range/metadata/extra of the former reference are kept,
            // only the media address is replaced
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
            // The MEDIA LENGTH — into the standard OTIO field (R.63 item 2), not into
            // our envelope: `available_range` is exactly about this, and a foreign
            // reader understands it without us. We write ONLY when the field
            // was absent: what came from a foreign file is passthrough (we already read its start
            // as the material’s timecode), and rewriting it with our own measurement
            // would mean “understanding half-way”.
            if active.available_range.is_none()
                && let Some(len) = media_len.filter(|l| l.0 > 0)
            {
                active.available_range = Some(range(Tick(0), len));
            }
            media_refs.insert(active_key.clone(), Some(MediaRefNode::External(active)));
        }
    }

    // the start of the active reference’s available_range — the very timecode
    // that was subtracted on reading (item 180)
    let media_origin = media_refs
        .get(&active_key)
        .and_then(|n| n.as_ref())
        .and_then(|n| match n {
            MediaRefNode::External(er) => er.available_range,
            MediaRefNode::Missing(mr) => mr.available_range,
            // a generator’s material start is always 0 — there is no file timecode
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

    // Markers are laid out by the WINDOW OF THE MATERIAL (R.49, item 357). A derived
    // event inherits the parent’s envelope WHOLESALE — otherwise undo would return the
    // parent without the markers that moved to the children. So the writer chooses: only those
    // that intersect the window of this event go into the file. Foreign markers of an
    // unknown schema are passed through as they are: we cannot read their range,
    // and foreign data must not be discarded.
    let win_start = c.source_in.0 + media_origin;
    let win_end = win_start + c.duration.0;
    let markers: Vec<MarkerNode> = ex
        .markers
        .into_iter()
        .filter(|m| match m {
            MarkerNode::Marker(mk) => {
                let a = mk.marked_range.start_time.to_ticks().0;
                let b = a + mk.marked_range.duration.to_ticks().0.max(0);
                // half-open interval: a point marker exactly at the end is already the next one
                a < win_end && b >= win_start
            }
            MarkerNode::Unknown(_) => true,
        })
        .collect();

    Clip {
        name: ex.name,
        // the material timecode returns to its place (item 180): a foreign file
        // must be read by another editor exactly as it arrived
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
    // unreadable automation curves: we collect them and fail the load at the end
    let mut bad_auto: Vec<String> = Vec::new();
    // the addresses of clips with a broken OUR section (item 358)
    let mut bad_kt: Vec<String> = Vec::new();
    // a broken kt section — an ERROR, not a “foreign file”: silently discarding
    // the envelope carried away the settings, tracks, morphers and layout
    let kt: Option<KtTimelineMeta> =
        kt_get_checked(&tl.metadata).map_err(|e| Error::KtMetaBroken(e.to_string()))?;
    let is_ours = kt.is_some();
    // the file’s envelope version: up to v3 the automation curves lay in fractions
    // of the event length, now in material ticks (R.33)
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

    // markers of the root Stack
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

    // Layer map (R.28): overlapping clips of one OUR track lie in
    // neighbouring Tracks — here they are assembled back. The map is taken from
    // our envelope; if it does not match the file (a foreign editor
    // rebuilt the tracks), it is ignored and the tracks are read one to one.
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
        // the audio halves of foreign attachments met on video tracks
        // (and vice versa) — they are born outside the walk of their own track
        root.clips.extend(std::mem::take(&mut cx.pending));
        let mut seq_facts = std::mem::take(&mut cx.seq_facts);
        for (id, mut seq) in std::mem::take(&mut cx.seqs) {
            // Sidecar (R.50): the tracks, the row order, the morphers and cameras of the
            // reel — what OTIO does not express. If it does not match the file —
            // a notice and reading as is, not corruption (the KtLayers pattern).
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

    // kt envelope: transport/ui/bin/morphers/cameras
    let mut kt_tracks: Vec<crate::meta::KtTrack> = Vec::new();
    let mut kt_legacy: (u32, u32, u32, u32, Vec<(kt_model::LaneKind, u32)>) =
        (0, 0, 0, 0, Vec::new());
    let mut kt_names: Vec<(kt_model::LaneKind, u32, String)> = Vec::new();
    let mut kt_heights: Vec<(kt_model::LaneKind, u32, f32)> = Vec::new();
    if let Some(kt) = kt {
        // unknown kt fields of future versions — passthrough (R.29)
        retained.timeline_kt_extra = kt.extra.clone();
        // overlapping transitions (R.51, item 399): ids are assigned anew —
        // identity is set by the pair, the numbers are continuous across the document (item 394)
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
        // LEGACY row order: its own field, and from `ui.lane_order` — the upgrade
        // of an even older file (both read-only, R.62)
        let mut legacy_order = kt.lane_order.clone();
        if legacy_order.is_empty()
            && let Some(u) = &kt.ui
        {
            legacy_order = u.lane_order.clone();
        }
        let legacy_names = kt.ui.as_ref().map(|u| u.lane_names.clone()).unwrap_or_default();
        let legacy_heights = kt.ui.as_ref().map(|u| u.lane_heights.clone()).unwrap_or_default();
        if let Some(mut ui) = kt.ui {
            // track names and heights since R.62 live on the track-objects
            ui.lane_names = Vec::new();
            ui.lane_heights = Vec::new();
            ui.bin_groups = kt.bin_groups;
            ui.bin_layout = kt.bin_layout;
            ui.bin_free = kt.bin_free;
            p.ui = Some(ui);
        }
        for bm in kt.bin_media {
            let path = resolve_url(base_dir, &bm.path);
            // the address could have already arrived with a reel event — there must be no duplicate
            if !p.media.iter().any(|m| m.id == MediaId(bm.id)) {
                p.media.push(MediaRef::new(MediaId(bm.id), path));
            }
        }
        p.published = kt.published;
        p.morphers = kt.morphers;
        // the morphers of a file BEFORE v4 — the former semantics: the patch on the whole target,
        // block time does not act (R.35, one-way upgrade)
        if file_v < crate::meta::KT_META_V4 {
            for m in &mut p.morphers {
                kt_model::migrate_legacy_morpher(m);
            }
        }
        p.cameras = kt.cameras;
        // camera keyframes of the old domain → shared curves (R.34 K3)
        for c in &mut p.cameras {
            kt_model::migrate_legacy_camera(c);
        }
        if let Some(end) = kt.end {
            p.timeline_end = Some(Tick(end));
        }
        if let Some(l) = &kt.layers {
            p.overlap_layers = !l.off;
        }
        // ROOT TRACKS (R.62): the envelope (order, properties, service
        // rows) + the facts of the file’s own tracks (name, `enabled`, chain);
        // neither one nor the other is there — the legacy counters and `lane_order`.
        let (lv, la) = kt.lanes.unwrap_or((0, 0));
        kt_tracks = kt.tracks.clone();
        kt_legacy = (lv, la, kt.morph_lanes, kt.camera_lanes, legacy_order.clone());
        kt_names = legacy_names;
        kt_heights = legacy_heights;
    }

    // TRACKS (R.62) — OUTSIDE the envelope: a foreign file has no envelope at all,
    // but it does have tracks (name, `enabled`, foreign effects on the track).
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
    // LEGACY ROW ORDER (before R.62 — its own field or `ui.lane_order`):
    // applied by the SINGLE reorder door, and its reordering is
    // needed by the foreign raw track data (it is addressed by the pair “view, number”)
    if kt_tracks.is_empty() && !kt_legacy.4.is_empty() {
        let perm = p.set_lane_order(&kt_legacy.4.clone());
        retained.remap_tracks(&perm);
    }
    // names and heights of the old file (`ui`) — onto the tracks, one-way
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

    // a deterministic order regardless of the track walk order
    // (bottom-up for video): save→load→save is stable, PartialEq is honest
    p.clips.sort_by_key(|c| c.id);
    p.foreign.sort_by_key(|f| f.key);
    p.media.sort_by_key(|m| m.id);
    if let Some(u) = &mut p.ui {
        // the order is independent of the track walk (video runs bottom-up)
        u.lane_names.sort_by_key(|(k, l, _)| (*k as u8, *l));
    }
    // the edit length is stored ONLY when it is longer than the material: otherwise
    // our own file would grow an end field after each round trip (t151 byte stability)
    let material = p.clips.iter().map(|c| c.start + c.duration).max().unwrap_or(Tick::ZERO);
    if p.timeline_end.is_some_and(|e| e <= material) {
        p.timeline_end = None;
    }
    // The SIDECHAIN KEY (R.65) is read TOLERANTLY: a key on a track that is absent
    // from the file, and a key on an EVENT — is “no key”, not a refusal to read.
    // The file could have come from a foreign hand or from a future schema; the project must
    // open, and the device sounds as an ordinary compressor.
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

    // TRACK NUMBER = ROW RANK (R.56, item 449). Foreign tracks are addressed by
    // the pair (view, number), so their raw data is permuted by the SAME
    // mapping: otherwise the metadata of a foreign track would move to a neighbour.
    let perm = p.normalize();
    retained.remap_tracks(&perm);
    p.validate().map_err(|e| Error::UnsupportedSchema(format!("invalid timeline: {e}")))?;
    if let Some(first) = bad_auto.first() {
        return Err(Error::UnsupportedSchema(first.clone()));
    }
    // the address of EVERY broken clip: there is no way to repair the file blindly, and the source we
    // do not touch — refusing to read is safer than silent corruption of links
    if !bad_kt.is_empty() {
        return Err(Error::KtMetaBroken(bad_kt.join("; ")));
    }
    Ok(ProjectBundle { project: p, retained, notices })
}

/// Our transition fields from its metadata: kind and audio curve. From the envelope they
/// are CUT OUT — the writer writes them back from the model, otherwise after a change of kind
/// a stale value would travel into the file. An empty “kt” envelope is removed
/// entirely: a foreign file without our keys must stay byte-stable.
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
    // parameter curves of the kind (R.34 K4) — the same envelope
    let auto: Vec<kt_model::ParamCurve> = match kt.shift_remove("auto") {
        Some(v) => serde_json::from_value(v).unwrap_or_else(|e| {
            notices.push(format!("unreadable transition curves ({e})"));
            Vec::new()
        }),
        None => Vec::new(),
    };
    // inversion (R.51, item 398); non-boolean — notice + false, the file does not fail
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

/// Curves of a file BEFORE v3: the keys lie in fractions of the event length (the R.30 domain),
/// we convert them into material ticks by the clip’s geometry. A fractional value does not
/// parse into the new schema at all, so we parse the old form with an explicit
/// structure — the same way the other legacy forms are read (markers).
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
            // the former boolean smoothing is expanded into ONE form for
            // all segments (R.61, D1b): the values after reading must
            // match the former ones bit for bit. The rule is ONE for both readers
            // (this one and `Deserialize for ParamCurve`) and knows about the speed
            // of a warp — R.63, see `implied_shape_of`.
            let shape = kt_model::ParamCurve::implied_shape_of(&c.param, c.ease);
            kt_model::ParamCurve {
                param: c.param,
                keys: c
                    .keys
                    .into_iter()
                    .map(|k| {
                        // the old domain of video curves was computed by the progress of the
                        // TIMELINE — for a reversed clip it is mirrored relative to the
                        // material
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
/// Where ONE reel is read into.
#[derive(Default)]
struct ReelRead {
    clips: Vec<ClipEvent>,
    transitions: Vec<TransitionEvent>,
    foreign: Vec<ForeignEvent>,
    /// The end of the content: for the root it gives `timeline_end`.
    end: Tick,
    /// What the file’s own tracks said: name, `enabled`, chain (R.62).
    track_facts: Vec<(kt_model::LaneKind, u32, TrackFacts)>,
}

/// Shared across the whole document on reading: address counters, the accumulator of foreign
/// raw data, remarks. Moved into a structure because the reader became
/// recursive (R.50), and dragging a dozen and a half `&mut` through the recursion —
/// is unreadable.
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
    /// Foreign data of TRACKS returns to its place only at the root: inside a reel the
    /// key `(kind, lane)` would collide with the root’s. Our reels carry no foreign
    /// raw data at all, and foreign attachments lose it — the price accepted together with the decision
    /// “read foreign Stacks as real reels”.
    track_extras: Option<&'a mut HashMap<(ClipKind, u32), TrackExtra>>,
    /// Read reels: address → content.
    seqs: Vec<(MediaId, Sequence)>,
    /// What the file’s tracks said about the tracks of EACH nested reel (R.62):
    /// the reel’s sidecar arrives later, while the facts are born during the walk.
    seq_facts: HashMap<MediaId, Vec<(kt_model::LaneKind, u32, TrackFacts)>>,
    /// Layer maps of nested reels from the sidecar `kt.sequences`.
    seq_layers: HashMap<u64, crate::meta::KtLayers>,
    /// Events born OUTSIDE the walk of their own track: the audio half of a foreign
    /// attachment met on a video track (and vice versa).
    pending: Vec<ClipEvent>,
}

/// Read the children of one `Stack` — the tracks of one reel.
///
/// `layers` — the layer map (R.28): overlapping clips of one OUR track
/// lie in neighbouring tracks, and without the map they would drift onto different
/// tracks. A map that did not match the file (a foreign editor rebuilt the
/// tracks) is IGNORED with a remark — we read one to one.
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
        // video is written from the BOTTOM track to the top — the map is unfolded
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
            // the spine is typed — we re-encode it into raw data for Retained.
            // At the ROOT the raw data is stored with its position and returns to it: before
            // R.50 it went to `foreign` by a key that the writer
            // never gets, and the round trip lost the object silently.
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
        // R.62: `enabled: false` is the track’s MUTE, not “we cannot do this”; the track chain is
        // read by the same code as for a clip, and
        // foreign effects stay in `Retained` and return to the file.
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
                    // a transition before a Gap — a fade-out of the previous clip (Gap =
                    // emptiness; the reference treats the track edge the same way)
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
                    // a transition before a clip: with a live neighbour on the left —
                    // a crossfade; without one (the track edge or a Gap) — a fade-in
                    // (R.25; the reference treats it the same way — a dissolve with a Gap)
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
                    // media references — and at the media level: they will be
                    // used by clips born in the editor (item 356)
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
                // An attachment: ours (`kt.seq`) — collapses by address; foreign
                // (a `Stack` or a `Track` inside a `Track`) — becomes a NEW
                // reel by the same code. Deeper than the limit — the old path of the
                // placeholder below: it is more merciful than a refusal, the round trip intact.
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
                    // a disabled clip / unknown schema / a nested
                    // Stack|Track → grey placeholder (R.24 item 5)
                    let dur = composable_duration(other).ok_or_else(|| {
                        Error::UnsupportedSchema(format!(
                            "{} without readable duration",
                            composable_tag(other)
                        ))
                    })?;
                    // a transition before a foreign placeholder: we cannot render its pair,
                    // but the fade-out of the previous clip is honest
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
        // a transition as the last child of a track — the fade-out of the last clip
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
        // edit length: a foreign file’s trailing Gap (R.25) — the end of the
        // track to the right of the last clip sets the timeline length
        out.end = out.end.max(cursor);
        if let Some(tracks) = cx.track_extras.as_deref_mut() {
            tracks.insert((kind, lane), tx);
        }
    }
    Ok(())
}

/// Read a FOREIGN attachment — a `Stack` or a `Track` inside a `Track` (R.50, A5).
///
/// Until this round it became a grey placeholder: “melt expands
/// nested compositions and draws their content, while for us there is emptiness”
/// (item 158). Now it is a real reel, read by the SAME `read_reel`.
/// The price was accepted by the owner together with the decision: importing foreign
/// files gives a different model from what it gave before.
///
/// Two cases, and both are in `nested_example.otio`:
///
/// - children are `Track`s: an ordinary multi-track reel;
/// - children are clips right away (a `Stack` without tracks, or a `Track` inside a `Track`):
///   a reel of ONE track, and its kind is that of the PARENT track, because
///   it has none of its own.
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
        // We wrap the content into ONE track of its kind: `read_reel` can
        // read only tracks, and the rule “clip after clip with gaps” is ours,
        // and writing it out a second time is not allowed.
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
    // A foreign attachment with sound AND a picture inside: for us this is TWO events in
    // one link group — we have no “track of both kinds” of our own. Silence
    // here is not allowed: a person would see one more event on the timeline than
    // there was in the foreign file.
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
        // the partner stands on the ZERO track of its kind: layering
        // is on, overlap is legal, and there is nothing to guess a free one with
        let pid = ClipId(*cx.next_clip_id);
        *cx.next_clip_id += 1;
        cx.pending.push(make(pid, other, 0));
    }
    Ok(make(id, kind, lane))
}

/// The former effect `kt:pan` (abolished R.62) → an additional “Geometry” node
/// on the same slot: slot indices do not move, the curves
/// `pan_x`/`pan_y` move onto `pos_x`/`pos_y` (recomputation `pan_to_pos`
/// at the base zoom), `zoom` remains. The mode of the former frame is not carried over:
/// all three gave one frame (F-MED-004).
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

/// Read the attachment: a `Stack` as a child of a `Track` (R.50).
///
/// The body is inlined into EVERY instance (there is no object identity in OTIO JSON),
/// so the instances collapse by `kt.seq`. If the bodies diverged —
/// a foreign editor edited one instance and did not edit the other — the second body
/// becomes a SEPARATE reel with a notice: silently picking one of the two is
/// the loss of someone else’s work.
/// The attachment’s effects: the same layout as for a clip, but we have no foreign ones
/// here — we write attachments ourselves. The curves are read in the v3+ domain (material
/// ticks): a file with an attachment cannot be older than v6.
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

/// TRACKS of a reel from the envelope and from the file’s own tracks (R.62).
///
/// The envelope is authoritative for OUR files: it carries the row order, service
/// rows and properties that OTIO does not have. A foreign file has no envelope —
/// then the tracks are built from the legacy counters, while the name and `enabled` are taken from
/// the tracks (`enabled: false` is read as mute: a foreign disabled track
/// must stay disabled, not “switch on” silently).
fn build_tracks(
    list: &[crate::meta::KtTrack],
    legacy: (u32, u32, u32, u32, &[(kt_model::LaneKind, u32)]),
    per_track: &[(kt_model::LaneKind, u32, TrackFacts)],
    need: [u32; 4],
) -> Vec<kt_model::Track> {
    use kt_model::LaneKind as K;
    let mut out: Vec<kt_model::Track> = if list.is_empty() {
        // a foreign or old file: the natural order from the counters
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
    // facts from the file’s own tracks: name, `enabled`, and the video/audio chain
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
    // tracks no fewer than the content occupies
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

/// How many tracks of each kind the reel’s content requires.
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

/// What the OTIO track itself said about the track.
#[derive(Default, Clone)]
struct TrackFacts {
    name: String,
    enabled: bool,
    fx: Vec<kt_model::FxInstance>,
    bypass: Vec<bool>,
    auto: Vec<Vec<kt_model::ParamCurve>>,
}

/// The reel’s own raster and fps from the `kt` envelope (R.62); no keys — the project.
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
    // the same address was already read — we compare the bodies
    let mut addr = seq_id;
    match cx.seqs.iter().find(|(id, _)| *id == seq_id) {
        Some((_, prev)) if prev.clips == body.clips && prev.transitions == body.transitions => {}
        Some(_) => {
            // the bodies diverged: the second version lives as its own reel, and its
            // content gets FRESH addresses (ClipId is unique across the
            // document)
            // a free address is searched across EVERYTHING already read: the counter
            // `next_media_id` hands out addresses only to files found by
            // URL, while addresses from our envelope arrive ready-made
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
    // the true time — from the envelope, if the track could not express it (R.58)
    let start = meta.as_ref().and_then(|m| m.start).map(Tick).unwrap_or(start);
    Ok(ClipEvent {
        r#gen: None,
        // Overrides of the reel’s INSTANCE — from our envelope (R.59,
        // stage 3). A foreign file does not carry them and cannot.
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
    // A broken OUR section of a clip — an ERROR, not a “foreign file” (R.49, item 358;
    // a twin of t251 in Timeline). `kt_get` with `.ok()` silently discarded the envelope:
    // the clip got a NEW number, and with it the A/V link groups and the morpher references
    // were torn, the volume, opacity and the disabled chain were lost.
    // A notice does not help here: the reassigned number is irreversible.
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
        // our generator: the canonical gen: path from generator_kind +
        // parameters; one that could not be read (a kt kind newer than the build) — an empty url,
        // that is, a visible stub, while the node travels back without loss
        Some(MediaRefNode::Generator(gr)) => generator_ref_path(gr).unwrap_or_default(),
        _ => String::new(),
    };
    let media = match &meta {
        Some(m) if is_ours => MediaId(m.media),
        // a clip without a reference (Missing/null) does not share a source with other such
        // ones: they have no common material, and deduplication by an empty URL would glue
        // independent clips into one bin record
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
        // The MATERIAL LENGTH — from the standard `available_range` of the external
        // reference (R.63 item 2). A generator has none and cannot have one: there
        // `available_range` is written as the consumed piece, not as the length of the
        // source, and taking it for the length would mean setting a trim limit for the
        // generator out of nowhere.
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

    // effects: kt:* → chain, LTW ±1 → reversed/no-op, other → foreign
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
                // the former Pan (R.62) — a “Geometry” node on the same slot
                if let Some((inst, curves)) = migrate_pan_effect(&eff.metadata) {
                    fx.push(inst);
                    fx_auto.push(serde_json::to_value(curves).ok());
                    bypass.push(!eff.enabled);
                }
            }
            EffectNode::Effect(eff) if eff.effect_name.starts_with("kt:") => {
                match kt_get::<kt_model::FxInstance>(&eff.metadata) {
                    Some(inst) => {
                        // automation: the key "auto" next to the parameters.
                        // We parse AFTER the loop: the key domain for files before
                        // v3 depends on source_in and reversed, and the reverse
                        // arrives as a separate effect below
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
        bypass.pop(); // the “enabled” tail is not serializable (the model’s form)
    }

    // MATERIAL START TIMECODE (item 180). In OTIO `source_range` lives in
    // the coordinate system of the material itself — the same as `available_range`.
    // For broadcast and cinema files it starts not at zero but from the
    // burned-in timecode (for Sol Levante — exactly one hour, 86160 frames). Our
    // `source_in` is an offset from the file’s ZERO frame, so the start of
    // available_range has to be subtracted; otherwise the engine drifts by an hour into a
    // 162-second file and renders emptiness.
    let origin = c.active_available_range().map(|r| r.start_time.to_ticks().0).unwrap_or(0);
    let src_in = c
        .source_range
        .as_ref()
        .map(|r| r.start_time.to_ticks().0 - origin)
        // without a source_range the clip takes the material whole — from its start
        .unwrap_or(0);
    let src_in = Tick(src_in.max(0));
    // the key domain: v3+ — material ticks as they are, older — fractions of the
    // event length, which we convert by the geometry of THIS clip (R.33)
    let mut fx_auto: Vec<Vec<kt_model::ParamCurve>> = fx_auto
        .iter()
        .map(|raw| match raw {
            None => Vec::new(),
            Some(v) if file_v >= crate::meta::KT_META_V3 => {
                // broken curves are NOT swallowed: silently losing animation is
                // worse than a refusal (the R.24 rule “do not break others’ work and
                // do not lose your own”)
                serde_json::from_value(v.clone()).unwrap_or_else(|e| {
                    bad_auto.push(format!("clip {}: unreadable automation ({e})", id.0));
                    Vec::new()
                })
            }
            Some(v) => curves_from_progress(v, src_in, dur, reversed),
        })
        .collect();
    while fx_auto.last().is_some_and(|v| v.is_empty()) {
        fx_auto.pop(); // a tail without curves is not serializable (the model’s form)
    }
    // the true time of the event — from the envelope, if the track could not express it
    // (R.58, item 462); to a foreign file we trust only the position in the track
    let start = match &meta {
        Some(m) if is_ours => m.start.map(Tick).unwrap_or(start),
        _ => start,
    };
    let mut ev = ClipEvent {
        // There is nothing to override on an ordinary event (`validate` requires this
        // too): the set is empty by construction.
        overrides: Vec::new(),
        // The generator’s values — from OUR envelope, if there is one (R.59,
        // stage 2). If there is none, the clip stays empty, and `normalize()`
        // takes the values from the media path: the R.52..R.58 files kept them
        // exactly there, and the migration lives in ONE place.
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
    // slowed&reverb keyframes of the old pattern — into the shared mechanism (R.33):
    // they no longer have their own domain
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

/// Clip duration: `source_range`, and without it — the whole `available_range`
/// of the active reference (OTIO spec; this is how Clip-004 lives in the upstream simple_cut.otio).
/// The corresponding `source_in` is taken in `clip_from_otio`.
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

/// Heuristic for the fps of a foreign file: the most frequent rate among the source_ranges,
/// brought to the 90 kHz grid; otherwise 30.
///
/// **NTSC series** (item 178): 23.976/29.97/59.94 do not fit the 90 kHz grid
/// (90000/23.976 = 3753.75), and before, such a file silently became
/// a 30 fps project — even though the real rate is closer to 24.
/// Now a fractional rate is rounded to the nearest whole one that divides
/// 90000: 23.976→24, 29.97→30, 59.94→60. The residual drift of 0.1 % (pulldown)
/// goes nowhere — the splice positions are stored in TICKS and are exact, only
/// the frame grid “drifts”.
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

/// The frame rate that fits the 90 kHz grid, nearest to `rate`.
/// None — if none was found (too large/zero, or far from a whole one).
pub fn grid_fps(rate: f64) -> Option<u32> {
    if !(rate.is_finite() && rate > 0.0 && rate <= 240.0) {
        return None;
    }
    let r = rate.round();
    // further than half a frame from a whole one — this is not the “NTSC version” of a whole rate
    if (rate - r).abs() > 0.5 {
        return None;
    }
    let r = r as u32;
    (r > 0 && kt_time::TICK_HZ % r as i64 == 0).then_some(r)
}

// ---- Retained ↔ JSON (R.70) ---------------------------------------------------
//
// The project file is its own, and the foreign passthrough of an imported exchange file travels in
// it as a separate field and returns on export: foreign work cannot be lost
// silently. The conversion goes through the serde bridges of the nodes (`node_serde!`), which
// already write and read OTIO objects — there is no layer of its own between `Retained` and JSON.

/// `HashMap` in JSON — PAIRS `[key, value]`, sorted by key.
///
/// Two reasons, and both are mandatory: a JSON object cannot have non-string keys
/// (`(ClipKind, u32)`, `(Option<ClipId>, Option<ClipId>)`), and the iteration order of a
/// `HashMap` is random — without sorting the same model would give different
/// bytes, and every save would be a spurious diff in git. Sorting by the
/// SERIALIZED key: `Ord` is not required of the keys, and their JSON
/// is deterministic.
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
    /// Passthrough as an opaque JSON for the project file. Deterministic: the
    /// document maps are insertion-ordered (as in the source), and `HashMap` fields —
    /// as sorted pairs.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("Retained is made of JSON values and schema nodes")
    }

    /// Back from the project file. A broken field is an error, not an empty passthrough:
    /// silently losing foreign work is worse than refusing.
    pub fn from_json(v: &Value) -> Result<Retained, Error> {
        Ok(serde_json::from_value(v.clone())?)
    }

    /// There is nothing to return: the project was never an exchange file (or there was no
    /// foreign data in it). Such a project carries no `otio` field in the file — the file
    /// does not grow for nothing, and a project born in the editor has no diff
    /// from the passthrough at all.
    pub fn is_empty(&self) -> bool {
        self.to_json() == Retained::default().to_json()
    }
}
