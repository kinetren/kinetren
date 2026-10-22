//! An editable model: commands + undo.
//!
//! Undo — snapshots of the whole `Project` (simple and reliable; the depth is limited).
//! A command is VALIDATED BEFORE the snapshot: an impossible command returns Err
//! and does not leave no-op entries in the history (defect q of the old
//! TimelineEditor, where push went before the checks).

use crate::genmedia::GeneratorKind;
use crate::project::*;
use crate::published::*;
use kt_time::{Tick, TickRange};

pub const MAX_UNDO: usize = 80;

/// Minimum clip duration. 1 ms, as in the old editor.
/// Align the warp SPEED of link partners (R.37 item 2, the user's decision:
/// “an effect for audio/video can be combined right away”).
///
/// The rule lives in the MODEL, not in the FX window, because `SetFx` also arrives
/// from pasting a chain from the buffer, from a morpher bake, from the format reader and
/// from kt-cli. “The A/V pair consumes the source at one speed” is an invariant of the
/// edit: if the speeds diverge, the picture and the sound diverge.
/// The precedent is `reconcile_transitions` after each geometric mutation. Inside
/// `mutate_on` the mirroring lands in the SAME undo step by construction, even under
/// `apply_coalesced`.
///
/// Synchronous: the presence of the slot, `rate` and its curve. Each one's own:
/// reverb parameters, frame mode, slot bypass (the checkbox on the video slot —
/// that is exactly “disable the picture slowdown”).
/// Generator values — shared by a link pair A/V ON ONE MEDIA (R.59,
/// stage 2).
///
/// The precedent and the rationale are the same as for `sync_warp_to_link_group`:
/// a sounding generator (SMPTE with tone, countdown with beep) puts on the
/// timeline a PAIR of events on one media, and they have one kind. If the
/// values diverged, the picture would show bars of one level while the tone sounded
/// from another, and there would be nothing to explain it with. The rule lives IN THE MODEL,
/// because commands come not only from the window: they are sent by chain pasting,
/// “apply to all” and the format reader.
///
/// Different media are not synchronized: they have different kinds and different source
/// addresses — this is no longer a pair, but two independent generators.
fn sync_gen_to_link_group(p: &mut Project, id: ClipId) {
    let Some(src) = p.clips.iter().find(|c| c.id == id) else { return };
    let Some(group) = src.link_group else { return };
    let (media, kind, auto) = (src.media, src.r#gen.clone(), src.gen_auto.clone());
    for c in &mut p.clips {
        if c.id != id && c.link_group == Some(group) && c.media == media {
            c.r#gen = kind.clone();
            c.gen_auto = auto.clone();
        }
    }
}

/// Bring the generator's values into agreement with the event's MEDIA (R.59).
///
/// The only place where values appear or vanish on their own: a clip has landed on
/// a generator — take the kind from the media path (the values of older files live there too,
/// so migration is free).
/// A clip that has moved to a file cannot have values.
fn fit_gen_to_media(p: &Project, c: &mut ClipEvent) {
    match p.default_gen_for(c.media) {
        Some(k) => {
            let same = c.r#gen.as_ref().is_some_and(|g| g.tag() == k.tag());
            if !same {
                c.r#gen = Some(k);
                c.gen_auto.clear();
            }
        }
        None => {
            c.r#gen = None;
            c.gen_auto.clear();
        }
    }
}

/// A consistency check of the generator's values for a command — with THE SAME
/// code as the file validation.
fn kt_check(p: &Project, c: &ClipEvent) -> Result<(), CommandError> {
    check_clip_gen(c, p.gen_media_path(c.media)).map_err(CommandError::InvalidClip)
}

fn sync_warp_to_link_group(p: &mut Project, id: ClipId) {
    let Some(src) = p.clips.iter().find(|c| c.id == id) else { return };
    let Some(group) = src.link_group else { return };
    // the slot is looked up BY KIND, not by index: audio may have
    // [Gain, Slowed], video [Blur, Slowed, Mirror]
    let warp_slot = |c: &ClipEvent| c.fx.iter().position(|f| f.warps_time());
    let mine = warp_slot(src);
    let (tmpl, rate_curve) = match mine {
        Some(i) => (
            Some(src.fx[i].clone()),
            src.fx_auto.get(i).and_then(|cs| cs.iter().find(|c| c.param == "rate").cloned()),
        ),
        None => (None, None),
    };
    let partners: Vec<ClipId> = p
        .clips
        .iter()
        .filter(|c| c.id != id && c.link_group == Some(group))
        .map(|c| c.id)
        .collect();
    for pid in partners {
        let Some(c) = p.clips.iter_mut().find(|c| c.id == pid) else { continue };
        match (warp_slot(c), &tmpl) {
            // both have it — we carry over ONLY the speed and its curve
            (Some(j), Some(t)) => {
                if let (
                    FxInstance::SlowedReverb { rate, .. },
                    FxInstance::SlowedReverb { rate: r0, .. },
                ) = (&mut c.fx[j], t)
                {
                    *rate = *r0;
                }
                if c.fx_auto.len() <= j {
                    c.fx_auto.resize(j + 1, Vec::new());
                }
                c.fx_auto[j].retain(|cu| cu.param != "rate");
                if let Some(rc) = &rate_curve {
                    c.fx_auto[j].push(rc.clone());
                }
            }
            // it appeared on one — we create it on the partner (one addition puts the
            // instance on BOTH events of the pair)
            (None, Some(t)) => {
                c.fx.push(t.clone());
                let mut curves = Vec::new();
                if let Some(rc) = &rate_curve {
                    curves.push(rc.clone());
                }
                c.fx_auto.resize(c.fx.len(), Vec::new());
                // the slot has just been added — last_mut exists by construction
                if let Some(slot) = c.fx_auto.last_mut() {
                    *slot = curves;
                }
            }
            // it disappeared from one — we remove it from the partner along with its curves
            (Some(j), None) => {
                c.fx.remove(j);
                if j < c.fx_auto.len() {
                    c.fx_auto.remove(j);
                }
                if j < c.fx_bypass.len() {
                    c.fx_bypass.remove(j);
                }
            }
            (None, None) => {}
        }
    }
}

pub const MIN_CLIP_DURATION: Tick = Tick(90);

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddMedia {
        id: MediaId,
        path: String,
    },
    /// A change of media path (R.52): editing the generator's parameters is a change of its
    /// `gen:`-path, and all clips of this media are updated (Vegas semantics: two clips of
    /// one generator share the kind). The path is opaque to the model, as in `AddMedia`,
    /// so the command also suits a future relink. Re-creating the source in the player
    /// is the concern of the call site
    /// (`KtApp::sync_media_sources`), as it is for `AddMedia`.
    SetMediaPath {
        id: MediaId,
        path: String,
    },
    AddClip(ClipEvent),
    RemoveClip {
        id: ClipId,
    },
    /// Moves the clip (and its whole link group — by the same offset).
    MoveClip {
        id: ClipId,
        new_start: Tick,
        new_lane: u32,
    },
    /// Trims the start: start/duration/source_in move consistently.
    TrimStart {
        id: ClipId,
        new_start: Tick,
    },
    TrimEnd {
        id: ClipId,
        new_end: Tick,
    },
    /// A note on KINSHIP: `child` — an event born of `parent` in this same
    /// packet (R.49, item 357). It does not change the project state at all.
    ///
    /// Why as a command. Derived events — the right half of a cut, the leftovers of a crop and
    /// punch-out, an inserted copy, pieces of a bake — are born INSIDE `ops::*`, and from outside
    /// their parent can no longer be computed: after renumbering it would be a guess by matching
    /// material and window. Whoever cuts knows the parent exactly — let him name it.
    /// The kinship carrier must survive undo together with the edit itself,
    /// and therefore must live in the command packet.
    NoteBirth {
        child: ClipId,
        parent: ClipId,
    },
    /// A cut at point `at` (absolute tick). The right half gets a new id
    /// and a new link group (the right halves of an A/V pair remain linked).
    Split {
        id: ClipId,
        at: Tick,
        new_id: ClipId,
        new_group: Option<GroupId>,
    },
    /// The option “trim stretches the animation together with the event” (R.40).
    /// Off by default: the key stays on its frame of the material,
    /// wherever the event's edge goes.
    SetTrimStretchesAuto {
        on: bool,
    },
    SetGain {
        id: ClipId,
        gain_db: f32,
    },
    /// Level (opacity) of a video event 0..1 — R.28, item 12.
    SetOpacity {
        id: ClipId,
        opacity: f32,
    },
    /// The leading “Geometry” node of the event and its position in the chain (R.62).
    SetGeometry {
        id: ClipId,
        geometry: crate::EventGeometry,
        pos: usize,
        auto: Vec<ParamCurve>,
    },
    /// The master switch of the event's FX chain (R.33). It does not touch the slot
    /// state: switch it back on and the chain is the same.
    SetFxOff {
        id: ClipId,
        off: bool,
    },
    /// Reverse of the event (item 16). Undo-able.
    SetReversed {
        id: ClipId,
        reversed: bool,
    },
    /// Bypass flags of the FX slots (item 2, v1.3). Undo-able.
    SetFxBypass {
        id: ClipId,
        bypass: Vec<bool>,
    },
    SetFx {
        id: ClipId,
        fx: Vec<FxInstance>,
    },
    /// Automation of video-FX parameters (R.30): a full replacement of the clip's curves
    /// (in parallel with fx, like fx_bypass). The UI sends it as a coalesced gesture.
    SetFxAuto {
        id: ClipId,
        auto: Vec<Vec<ParamCurve>>,
    },
    /// Generator values ON THE EVENT (R.59, stage 2).
    ///
    /// Does not touch the frame plan (`touches_render_plan() == false`): the source's address
    /// is computed from the event, not from the values, and the values reach the LIVE source
    /// through the `retune` strip — exactly the purpose for which that strip
    /// was introduced in R.53.
    /// Otherwise every keystroke in a title would stop the audio render stream.
    SetClipGen {
        id: ClipId,
        kind: Option<GeneratorKind>,
    },
    /// Curves of the generator's parameters (domain — the material tick, like fx_auto).
    SetClipGenAuto {
        id: ClipId,
        auto: Vec<ParamCurve>,
    },
    /// “Edit in all places at once” is a SEPARATE command (the owner's decision in R.59):
    /// editing one event no longer touches its neighbors, while “places: N” remains available
    /// and fits into one history step.
    ApplyGenToAll {
        media: MediaId,
        kind: GeneratorKind,
        auto: Vec<ParamCurve>,
    },
    LinkClips {
        ids: Vec<ClipId>,
        group: GroupId,
    },
    UnlinkClip {
        id: ClipId,
    },
    /// A transition at the junction of two adjacent clips on one track (R.24). Undo-able.
    AddTransition(TransitionEvent),
    RemoveTransition {
        id: TransitionId,
    },
    /// A full replacement of a transition (dragging edges/body — batches and apply_coalesced).
    SetTransition(TransitionEvent),
    /// A marker; duration > 0 = a range marker (R.24, marked_range).
    AddMarker {
        at: Tick,
        duration: Tick,
    },
    RemoveMarker {
        at: Tick,
    },
    /// Set/change the marker's name (item 1, round 10). A separate command: creation
    /// with a name = batch [AddMarker, SetMarkerName] (one undo step), live
    /// typing — apply_coalesced by the gesture token.
    SetMarkerName {
        at: Tick,
        name: String,
    },
    MoveMarker {
        from: Tick,
        to: Tick,
    },
    /// The marker's duration (0 = collapse to a point marker). The reel edges — by gestures.
    SetMarkerDuration {
        at: Tick,
        duration: Tick,
    },
    /// Insert an empty track of the kind `kind` with rank `at` (R.62: one
    /// command for all four kinds); the content numbers of the kind ≥ at shift
    /// by +1, the row is placed right above the former `at` (at the end, if there is none).
    InsertTrack {
        kind: LaneKind,
        at: u32,
    },
    /// Delete a track together with its content; the numbers above shift by −1.
    /// The last video or audio track is not deleted (`LastTrack`).
    RemoveTrack {
        kind: LaneKind,
        at: u32,
    },
    /// Track properties in one piece: name, height, switches, level,
    /// overlay mode (R.62). The chain — `SetTrackFx`.
    SetTrackProps {
        id: TrackId,
        props: TrackProps,
    },
    /// The track's FX chain: slots and bypass. A track has no curves — its
    /// key is the timeline tick, and that is a second automation domain (the next
    /// change), and the command rejects them rather than accepting them silently.
    SetTrackFx {
        id: TrackId,
        fx: Vec<FxInstance>,
        bypass: Vec<bool>,
    },
    SetTrackFxOff {
        id: TrackId,
        off: bool,
    },
    RemoveMedia {
        id: MediaId,
    },
    SetSettings {
        settings: ProjectSettings,
    },
    /// Morpher (v1 port). Unlike v1, ALL operations are undo-able (defect D6).
    AddMorpher(MorpherEvent),
    RemoveMorpher {
        id: MorphId,
    },
    /// A full replacement of the event (editor window; in the UI — a coalesced gesture).
    SetMorpher(MorpherEvent),
    /// Bake: destructively write the patch into the target clips and delete the event.
    /// The render result does not change: effective_clips already applied the same.
    BakeMorpher {
        id: MorphId,
    },
    /// The order of track rows from top to bottom (R.35). With the morpher's move into the
    /// layer it affects the PICTURE, so it lives in the model and goes through
    /// undo: drag a track — you changed the edit. A full replacement; the vector is
    /// normalized by the mutation, there is no need to check it.
    SetLaneOrder {
        order: Vec<(LaneKind, u32)>,
    },
    /// Named regions (R.35 item 2). Addressing BY NAME: indices
    /// shift under undo/redo, while the name is the address.
    AddArea(MarkedArea),
    RemoveArea {
        name: String,
    },
    /// Renaming also fixes the morpher REFERENCES — in one history step.
    RenameArea {
        from: String,
        to: String,
    },
    SetAreaRange {
        name: String,
        start: Tick,
        duration: Tick,
    },
    SetAreaLanes {
        name: String,
        lanes: Vec<(LaneKind, u32)>,
    },
    /// Cameras (R.29, 3D compositing) — modeled on the morpher commands.
    AddCamera(CameraEvent),
    /// A full replacement of the event (parameters window; in the UI — a coalesced gesture).
    SetCamera(CameraEvent),
    RemoveCamera {
        id: CameraId,
    },
    // --- nested sequences (R.50) ---
    /// Create an empty reel. The content arrives in a separate command:
    /// folding is the package “create, fill, remove the originals,
    /// place an instance”, and it must be ONE history step.
    AddSequence {
        id: MediaId,
        name: String,
    },
    /// Remove a reel. Refused while any event of the tree still points at it:
    /// silently orphaning clips costs more than saying “busy”.
    RemoveSequence {
        id: MediaId,
    },
    RenameSequence {
        id: MediaId,
        name: String,
    },
    /// The reel’s own raster and fps (R.62); `None` means the same as the project.
    SetSequenceSettings {
        id: MediaId,
        raster: Option<(u32, u32)>,
        fps: Option<u32>,
    },
    /// Full replacement of the reel’s CONTENT (Box, so that `Command` does not
    /// grow by size for the sake of one variant).
    SetSequenceContent {
        id: MediaId,
        content: Box<Sequence>,
    },
    /// Change the source of an event: a file or a reel. The carrying command of
    /// both folding and “make independent”.
    SetClipMedia {
        id: ClipId,
        media: MediaId,
    },
    // --- public parameters of a composition (R.59, stage 3) ---
    /// Declare a property of a NAMED reel outward.
    ///
    /// **The reel is named EXPLICITLY (R.63 item 11), and this is not decoration.** Before
    /// this round the command followed the rule “the reel you are standing in”, and
    /// publishing from an expanded band required entering the reel FIRST:
    /// “if we publish a parameter without entering the composition fully, then why
    /// do we automatically enter the composition fully” (the owner). The address in the
    /// command itself removes both the entry and the dependence on context: the handle
    /// lands in the named reel wherever the person stands, and a nonexistent reel is an
    /// honest refusal.
    PublishParam {
        seq: MediaId,
        param: Box<PublishedParam>,
    },
    /// Remove the handle. Overrides pointing at it disappear in the same history step
    /// (`reconcile_published`), so no dead handle is left outside.
    UnpublishParam {
        seq: MediaId,
        id: PubId,
    },
    /// Renaming and narrowing the limits is a full replacement of the declaration.
    SetPublished {
        seq: MediaId,
        param: Box<PublishedParam>,
    },
    /// The order of handles in the consumer’s window is part of the author’s interface
    /// (Resolve 21 had it as a separate feature, reconnaissance F-TL-012).
    ReorderPublished {
        seq: MediaId,
        order: Vec<PubId>,
    },
    /// Override on an INSTANCE. A full replacement of one handle.
    SetOverride {
        id: ClipId,
        ov: Box<Override>,
    },
    ClearOverride {
        id: ClipId,
        pub_id: PubId,
    },
}

impl Command {
    /// Does the command change what the renderers COUNT (R.53, item 412).
    ///
    /// Law P: the media path is a property of the SOURCE, not of the plan. The renderers
    /// know only `MediaId`, so `SetMediaPath` has no right to clone the project
    /// and rebuild the graphs: its addressee is the worker of this media
    /// (`Player::retune_media`). Before this item, every keystroke in a title’s text
    /// field stopped the audio render thread.
    ///
    /// The match is EXHAUSTIVE and without `_ =>`: a new command will not compile until
    /// it is answered whether it touches the plan. A precedent of the same kind is
    /// `on_region_changed` without `set_project` (R.43 item 296, guard t286).
    /// The event that the command edits, if it has one and it EXISTS.
    ///
    /// Introduced for the sake of one question: “in which REEL is this edit” (R.55, C5).
    /// Enumerating the variants in the caller is not allowed: that would be a second place
    /// that knows how commands are built, and it would drift apart from the first.
    pub fn target_clip(&self) -> Option<ClipId> {
        match self {
            Command::AddClip(c) => Some(c.id),
            Command::RemoveClip { id }
            | Command::MoveClip { id, .. }
            | Command::TrimStart { id, .. }
            | Command::TrimEnd { id, .. }
            | Command::Split { id, .. }
            | Command::SetGain { id, .. }
            | Command::SetOpacity { id, .. }
            | Command::SetGeometry { id, .. }
            | Command::SetFxOff { id, .. }
            | Command::SetReversed { id, .. }
            | Command::SetFxBypass { id, .. }
            | Command::SetFx { id, .. }
            | Command::SetFxAuto { id, .. }
            | Command::SetClipGen { id, .. }
            | Command::SetClipGenAuto { id, .. }
            | Command::UnlinkClip { id }
            | Command::SetOverride { id, .. }
            | Command::ClearOverride { id, .. }
            | Command::SetClipMedia { id, .. } => Some(*id),
            _ => None,
        }
    }

    /// ROWS whose content the command edits (R.62, change B) is the ONLY function
    /// by which the track lock is checked.
    ///
    /// The match is EXHAUSTIVE and without `_ =>`: a new command will not compile until
    /// it is assigned to one of the two halves, “touches events” (then name the rows) or
    /// “does not touch” (an empty list). This is the same technique as `Band::after_batch`,
    /// and for the same reason: a missing row here means a gesture that passed through the
    /// lock silently.
    ///
    /// The properties of the track ITSELF (`SetTrackProps`) do not fall under the lock,
    /// otherwise a locked track could not be unlocked.
    pub fn target_rows(&self, p: &Project) -> Vec<(LaneKind, u32)> {
        let of_clip =
            |id: ClipId| p.clips.iter().find(|c| c.id == id).map(|c| (lane_key_of(c.kind), c.lane));
        let mut out: Vec<(LaneKind, u32)> = Vec::new();
        let mut add = |row: Option<(LaneKind, u32)>| {
            if let Some(r) = row
                && !out.contains(&r)
            {
                out.push(r);
            }
        };
        match self {
            // clip events: both where they are taken from and where they are put
            Command::AddClip(c) => add(Some((lane_key_of(c.kind), c.lane))),
            Command::MoveClip { id, new_lane, .. } => {
                add(of_clip(*id));
                add(of_clip(*id).map(|(k, _)| (k, *new_lane)));
            }
            Command::RemoveClip { id }
            | Command::TrimStart { id, .. }
            | Command::TrimEnd { id, .. }
            | Command::Split { id, .. }
            | Command::SetGain { id, .. }
            | Command::SetOpacity { id, .. }
            | Command::SetGeometry { id, .. }
            | Command::SetFxOff { id, .. }
            | Command::SetReversed { id, .. }
            | Command::SetFxBypass { id, .. }
            | Command::SetFx { id, .. }
            | Command::SetFxAuto { id, .. }
            | Command::SetClipGen { id, .. }
            | Command::SetClipGenAuto { id, .. }
            | Command::UnlinkClip { id }
            | Command::SetOverride { id, .. }
            | Command::ClearOverride { id, .. }
            | Command::SetClipMedia { id, .. } => add(of_clip(*id)),
            Command::LinkClips { ids, .. } => {
                for id in ids {
                    add(of_clip(*id));
                }
            }
            // “in all places at once”: the rows of ALL events of this media
            Command::ApplyGenToAll { media, .. } => {
                for c in p.clips.iter().filter(|c| c.media == *media) {
                    add(Some((lane_key_of(c.kind), c.lane)));
                }
            }
            // transitions live AT the joint of two events; locking either of the rows locks it
            Command::AddTransition(t) | Command::SetTransition(t) => {
                add(t.a.and_then(of_clip));
                add(t.b.and_then(of_clip));
            }
            Command::RemoveTransition { id } => {
                if let Some(t) = p.transitions.iter().find(|t| t.id == *id) {
                    add(t.a.and_then(of_clip));
                    add(t.b.and_then(of_clip));
                }
            }
            // morphers and cameras are events of their own service rows
            Command::AddMorpher(m) | Command::SetMorpher(m) => add(Some((LaneKind::Morph, m.lane))),
            Command::RemoveMorpher { id } | Command::BakeMorpher { id } => {
                add(p.morphers.iter().find(|m| m.id == *id).map(|m| (LaneKind::Morph, m.lane)));
            }
            Command::AddCamera(c) | Command::SetCamera(c) => add(Some((LaneKind::Camera, c.lane))),
            Command::RemoveCamera { id } => {
                add(p.cameras.iter().find(|c| c.id == *id).map(|c| (LaneKind::Camera, c.lane)));
            }
            // deleting a track takes its events with it; this is also an edit of the content
            Command::RemoveTrack { kind, at } => add(Some((*kind, *at))),
            // the CONTENT OF ROWS IS NOT TOUCHED — listed by name
            Command::NoteBirth { .. }
            | Command::AddMedia { .. }
            | Command::SetMediaPath { .. }
            | Command::RemoveMedia { .. }
            | Command::SetSettings { .. }
            | Command::SetTrimStretchesAuto { .. }
            | Command::AddMarker { .. }
            | Command::RemoveMarker { .. }
            | Command::SetMarkerName { .. }
            | Command::MoveMarker { .. }
            | Command::SetMarkerDuration { .. }
            | Command::InsertTrack { .. }
            | Command::SetTrackProps { .. }
            | Command::SetTrackFx { .. }
            | Command::SetTrackFxOff { .. }
            | Command::SetLaneOrder { .. }
            | Command::AddArea(_)
            | Command::RemoveArea { .. }
            | Command::RenameArea { .. }
            | Command::SetAreaRange { .. }
            | Command::SetAreaLanes { .. }
            | Command::AddSequence { .. }
            | Command::RemoveSequence { .. }
            | Command::RenameSequence { .. }
            | Command::SetSequenceSettings { .. }
            | Command::SetSequenceContent { .. }
            | Command::PublishParam { .. }
            | Command::UnpublishParam { .. }
            | Command::SetPublished { .. }
            | Command::ReorderPublished { .. } => {}
        }
        out
    }

    pub fn touches_render_plan(&self) -> bool {
        match self {
            // Generator values are a property of the SOURCE, not of the plan (the same
            // law P as the media path). Their addressee is the worker of this event,
            // `Player::update_gen`.
            Command::SetMediaPath { .. }
            | Command::SetClipGen { .. }
            | Command::SetClipGenAuto { .. } => false,
            Command::NoteBirth { .. }
            | Command::AddMedia { .. }
            | Command::AddClip(_)
            | Command::RemoveClip { .. }
            | Command::MoveClip { .. }
            | Command::TrimStart { .. }
            | Command::TrimEnd { .. }
            | Command::Split { .. }
            | Command::SetTrimStretchesAuto { .. }
            | Command::SetGain { .. }
            | Command::SetOpacity { .. }
            | Command::SetGeometry { .. }
            | Command::SetFxOff { .. }
            | Command::SetReversed { .. }
            | Command::SetFxBypass { .. }
            | Command::SetFx { .. }
            | Command::SetFxAuto { .. }
            | Command::ApplyGenToAll { .. }
            | Command::LinkClips { .. }
            | Command::UnlinkClip { .. }
            | Command::AddTransition(_)
            | Command::RemoveTransition { .. }
            | Command::SetTransition(_)
            | Command::AddMarker { .. }
            | Command::RemoveMarker { .. }
            | Command::SetMarkerName { .. }
            | Command::MoveMarker { .. }
            | Command::SetMarkerDuration { .. }
            | Command::InsertTrack { .. }
            | Command::RemoveTrack { .. }
            | Command::SetTrackProps { .. }
            | Command::SetTrackFx { .. }
            | Command::SetTrackFxOff { .. }
            | Command::RemoveMedia { .. }
            | Command::SetSettings { .. }
            | Command::AddMorpher(_)
            | Command::RemoveMorpher { .. }
            | Command::SetMorpher(_)
            | Command::BakeMorpher { .. }
            | Command::SetLaneOrder { .. }
            | Command::AddArea(_)
            | Command::RemoveArea { .. }
            | Command::RenameArea { .. }
            | Command::SetAreaRange { .. }
            | Command::SetAreaLanes { .. }
            | Command::AddCamera(_)
            | Command::SetCamera(_)
            | Command::RemoveCamera { .. }
            | Command::AddSequence { .. }
            | Command::RemoveSequence { .. }
            | Command::RenameSequence { .. }
            | Command::SetSequenceSettings { .. }
            | Command::SetSequenceContent { .. }
            // Override and publication change the CONTENT of the reel as the engine sees it:
            // the edit becomes a different one, and the plan must be rebuilt.
            // Law P does not extend here:
            // a `retune` band cannot carry such a change.
            | Command::PublishParam { .. }
            | Command::UnpublishParam { .. }
            | Command::SetPublished { .. }
            | Command::ReorderPublished { .. }
            | Command::SetOverride { .. }
            | Command::ClearOverride { .. }
            | Command::SetClipMedia { .. } => true,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CommandError {
    #[error("clip {0:?} not found")]
    ClipNotFound(ClipId),
    #[error("media {0:?} not found")]
    MediaNotFound(MediaId),
    #[error("id {0:?} already in use")]
    IdInUse(u64),
    #[error("result would violate minimal duration")]
    TooShort,
    #[error("marked area {0:?} not found")]
    AreaNotFound(String),
    #[error("marked area: name must not be empty")]
    AreaUnnamed,
    #[error("name {0:?} is already taken (areas and named ranges share one namespace)")]
    NameTaken(String),
    #[error("split point outside clip")]
    SplitOutside,
    #[error("published parameter {0:?} not found")]
    PublishedNotFound(PubId),
    #[error("the new order is not a permutation of the published set")]
    PublishedOrderBroken,
    #[error("invalid clip: {0}")]
    InvalidClip(#[from] ProjectError),
    #[error("transition {0:?} not found")]
    TransitionNotFound(TransitionId),
    #[error("layout invariant violated: {0}")]
    InvalidLayout(ProjectError),
    #[error("invalid transition: {0}")]
    InvalidTransition(ProjectError),
    #[error("marker at {0:?} not found")]
    MarkerNotFound(Tick),
    #[error("marker at {0:?} already exists")]
    MarkerExists(Tick),
    #[error("media {0:?} is used by clips")]
    MediaInUse(MediaId),
    #[error("sidechain key {0:?} is not an audio track of this reel (or is the track itself)")]
    SidechainKeyInvalid(TrackId),
    #[error("sidechain key would make a cycle through track {0:?}")]
    SidechainCycle(TrackId),
    #[error("sidechain key belongs on a track chain, not on a clip")]
    SidechainOnClip,
    #[error("lane count would orphan clips")]
    LanesTooFew,
    #[error("{0:?} track {1} does not exist")]
    TrackMissing(LaneKind, u32),
    #[error("the last {0:?} track cannot be removed")]
    LastTrack(LaneKind),
    #[error("track {0:?} not found")]
    TrackNotFound(TrackId),
    #[error("track {0:?}: property out of range")]
    TrackPropOutOfRange(TrackId),
    #[error("track {0:?}: curves on a track are not supported yet")]
    TrackCurvesUnsupported(TrackId),
    #[error("track {0:?} is locked")]
    TrackLocked(TrackId),
    #[error("morpher {0:?} not found")]
    MorpherNotFound(MorphId),
    #[error("morpher needs at least one target clip")]
    MorpherNoTargets,
    #[error("morph lane {0} not found")]
    MorphLaneNotFound(u32),
    #[error("camera {0:?} not found")]
    CameraNotFound(CameraId),
    #[error("invalid camera: {0}")]
    InvalidCamera(ProjectError),
    #[error("camera lane {0} not found")]
    CameraLaneNotFound(u32),
    #[error("sequence {0:?} not found")]
    SequenceNotFound(MediaId),
    #[error("sequence {0:?} is still used by {1} clip(s)")]
    SequenceInUse(MediaId, usize),
    #[error("row order and lane count cannot change while a reel is expanded in place")]
    BandsOpen,
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("nothing to redo")]
    NothingToRedo,
}

/// A model with history. The engine reads `project()` (a snapshot clone on rebuild).
#[derive(Debug, Clone)]
pub struct TimelineModel {
    /// CANON: the real layout without expanded bands. It goes into the file,
    /// the player and the export, and only it exists in actual fact.
    project: Project,
    /// DERIVED: the canon plus the reels expanded in place (R.55, C5).
    ///
    /// It is returned by `project()`, that is, the whole interface works with it —
    /// drawing, hit testing, snapping, selection, windows. None of these hands
    /// knows that part of the events on screen lives in another reel: they physically
    /// lie in the root vectors, only with an offset.
    /// With empty `bands` this is a byte-for-byte copy of the canon, that is, the former
    /// behavior without a single extra branch.
    bench: Project,
    /// Expanded bands. A working place, not work: it is not written to the file
    /// (rule t390-t391), but EVERY undo snapshot remembers its own — otherwise Ctrl+Z
    /// inside an expansion would look like “pressed, and everything vanished”.
    bands: Vec<Band>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// The token of the last coalesced gesture (see apply_coalesced).
    last_gesture: Option<u64>,
    /// The reel that is being edited now (R.50, B1). `None` is the root.
    ///
    /// It lives IN THE MODEL, not in the project: this is not editing but the place where the
    /// person stands. It is not written to the file (the view inside a reel is a working place,
    /// not work) and is not part of the undo snapshot — but EVERY snapshot remembers
    /// in what context the edit was made.
    editing: Option<MediaId>,
}

/// History snapshot: the project and the context in which the edit was made (R.50, B1).
///
/// Without the context, Ctrl+Z inside a reel would look like “pressed — nothing
/// happened”: the edit in some other place, which is not visible, would be undone.
#[derive(Debug, Clone)]
struct Snapshot {
    project: Project,
    scope: Option<MediaId>,
    /// Which bands were expanded at the moment of the edit (R.55, C5).
    bands: Vec<Band>,
}

impl Band {
    /// Recalculate the band after a batch of commands (R.55, C5).
    ///
    /// **The single place of the construction where correctness rests on the person,
    /// not on the build.** That is why the `match` is EXHAUSTIVE, without `_ =>`: a new
    /// command will not compile until its row is filled in — the same technique as
    /// `Command::touches_render_plain` and the table of t253.
    ///
    /// Exactly six commands change the track numbering. The band lies at the TAIL
    /// of the numeric space, so the rule is simple: a track inserted or removed
    /// BELOW the start of the band — the whole band moves (`base`); inside the band
    /// — its size changes (`len`).
    ///
    /// The error here does not fire any assert by itself: the band “drifts”, and
    /// `unmerge_reel` starts rejecting batches — or, with a symmetric error, silently puts
    /// an event into the wrong reel. Guard — t568.
    pub fn after_batch(&mut self, cmds: &[Command]) {
        for cmd in cmds {
            match cmd {
                Command::InsertTrack { kind, at } => self.lane_inserted(*kind, *at),
                // `SetLanes` does NOT MOVE THE BAND.
                // This is a decision, not an oversight.
                // The command sets the number of tracks as a whole, and the clips are not
                // renumbered at the same time — so the band boundary (`base`) must remain
                // where the events lie.
                //
                // The only sender of it is track deletion, and it is accompanied by `MoveClip`
                // for everyone above. That is why the application does not allow deleting a track
                // while a band is expanded: first it folds the expansion and says so.
                //
                // `SetLaneOrder` from R.56 renumbers the content (track number = rank of the row,
                // item 449) — on the bench this would move the events of the band
                // out of the TAIL of the numeric space, and `unmerge_reel` would sort them
                // into the wrong reels. Both commands are rejected by `simulate` while a band
                // is open (`CommandError::BandsOpen`),
                // so they do not reach here.
                Command::RemoveTrack { .. } | Command::SetLaneOrder { .. } => {
                    debug_assert!(
                        false,
                        "RemoveTrack/SetLaneOrder are refused while a band is open"
                    )
                }
                // Everything else does not move track numbers. They are listed
                // by name ON PURPOSE: `_ =>` would hide the next command
                // that starts moving them.
                Command::NoteBirth { .. }
                | Command::AddMedia { .. }
                | Command::SetMediaPath { .. }
                | Command::AddClip(_)
                | Command::RemoveClip { .. }
                | Command::MoveClip { .. }
                | Command::TrimStart { .. }
                | Command::TrimEnd { .. }
                | Command::Split { .. }
                | Command::SetTrimStretchesAuto { .. }
                | Command::SetGain { .. }
                | Command::SetOpacity { .. }
                | Command::SetGeometry { .. }
                | Command::SetFxOff { .. }
                | Command::SetReversed { .. }
                | Command::SetFxBypass { .. }
                | Command::SetFx { .. }
                | Command::SetFxAuto { .. }
                | Command::SetClipGen { .. }
                | Command::SetClipGenAuto { .. }
                | Command::ApplyGenToAll { .. }
                | Command::LinkClips { .. }
                | Command::UnlinkClip { .. }
                | Command::AddTransition(_)
                | Command::RemoveTransition { .. }
                | Command::SetTransition(_)
                | Command::AddMarker { .. }
                | Command::RemoveMarker { .. }
                | Command::SetMarkerName { .. }
                | Command::MoveMarker { .. }
                | Command::SetMarkerDuration { .. }
                | Command::RemoveMedia { .. }
                | Command::SetSettings { .. }
                | Command::AddMorpher(_)
                | Command::RemoveMorpher { .. }
                | Command::SetMorpher(_)
                | Command::BakeMorpher { .. }
                | Command::AddArea(_)
                | Command::RemoveArea { .. }
                | Command::RenameArea { .. }
                | Command::SetAreaRange { .. }
                | Command::SetAreaLanes { .. }
                | Command::SetTrackProps { .. }
                | Command::SetTrackFx { .. }
                | Command::SetTrackFxOff { .. }
                | Command::AddCamera(_)
                | Command::SetCamera(_)
                | Command::RemoveCamera { .. }
                | Command::AddSequence { .. }
                | Command::RemoveSequence { .. }
                | Command::RenameSequence { .. }
                | Command::SetSequenceSettings { .. }
                | Command::SetSequenceContent { .. }
                | Command::PublishParam { .. }
                | Command::UnpublishParam { .. }
                | Command::SetPublished { .. }
                | Command::ReorderPublished { .. }
                | Command::SetOverride { .. }
                | Command::ClearOverride { .. }
                | Command::SetClipMedia { .. } => {}
            }
        }
    }

    fn lane_inserted(&mut self, kind: LaneKind, at: u32) {
        let base = self.base.get(kind);
        if at <= base {
            *self.base.get_mut(kind) += 1;
        } else if at <= base + self.len.get(kind) {
            // The TAIL of the band belongs to the BAND (R.56): by construction nothing
            // lies behind it, and an insertion exactly at `base + len` is
            // “the reel needs one more track” (an insertion in the expansion).
            // Before R.56 such a track belonged to nobody,
            // and an event on it silently drifted into the root.
            *self.len.get_mut(kind) += 1;
        }
    }
}

/// Who was born from whom within a batch of commands (R.49, item 357). The kinship is
/// reported by the MODEL, it is not guessed by the caller: for `Split` it is directly in
/// the arguments, for the other derived ones it is in the note `NoteBirth` that the one who
/// cut leaves behind. A guess by matching material and window would be exactly a guess:
/// after renumbering there is no way to tell “the half of a cut” from “a neighboring event of
/// the same file”.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Births {
    /// Pairs “new event ← parent”, in the order of birth.
    pub pairs: Vec<(ClipId, ClipId)>,
}

impl Births {
    fn of(cmds: &[Command]) -> Births {
        let mut pairs = Vec::new();
        for c in cmds {
            match c {
                Command::Split { id, new_id, .. } => pairs.push((*new_id, *id)),
                Command::NoteBirth { child, parent } => pairs.push((*child, *parent)),
                _ => {}
            }
        }
        Births { pairs }
    }

    pub fn parent_of(&self, child: ClipId) -> Option<ClipId> {
        self.pairs.iter().find(|(c, _)| *c == child).map(|(_, p)| *p)
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

impl TimelineModel {
    pub fn new(project: Project) -> TimelineModel {
        TimelineModel {
            bench: project.clone(),
            project,
            bands: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            last_gesture: None,
            editing: None,
        }
    }

    /// Rebuild the working view from the canon. Called AFTER every edit of the canon —
    /// including edits of fields outside history (region, loop, ui).
    fn rebuild_bench(&mut self) {
        self.bench = self.project.clone();
        for b in &self.bands {
            self.bench.merge_reel(b);
        }
    }

    /// Expanded bands.
    pub fn bands(&self) -> &[Band] {
        &self.bands
    }

    /// Expand an instance’s reel IN PLACE (R.55, C5). `false` means it is not
    /// an attachment, or it is already expanded.
    pub fn expand_band(&mut self, inst: ClipId) -> bool {
        if self.bands.iter().any(|b| b.inst == inst) {
            return false;
        }
        // the band is counted from the CANON: `base` must be the number of root
        // tracks, not “how many there are now on screen together with someone else’s band”.
        // INSIDE a full entry (R.58, item 460) the canon is the current reel as the root
        // (`swap_reel`), and `Band::of` counts the bases from it too; no
        // special branch is needed. Expanding the very reel in which we stand is impossible by
        // cycles (`validate`); the guard is for the case of a manual build of the project
        // bypassing validation.
        let Some(b) = Band::of(&self.project, inst) else { return false };
        debug_assert!(
            Some(b.seq) != self.editing,
            "expanding the reel being edited inside itself: a cycle escaped validation"
        );
        self.bands.push(b);
        self.rebuild_bench();
        true
    }

    /// Fold the expansion back.
    pub fn collapse_band(&mut self, inst: ClipId) -> bool {
        let Some(i) = self.bands.iter().position(|b| b.inst == inst) else { return false };
        self.bands.remove(i);
        self.rebuild_bench();
        true
    }

    pub fn collapse_all_bands(&mut self) {
        if self.bands.is_empty() {
            return;
        }
        self.bands.clear();
        self.rebuild_bench();
    }

    /// Whose row this is: the root’s or the expanded band’s.
    pub fn band_of_row(&self, kind: LaneKind, lane: u32) -> Option<&Band> {
        self.bands.iter().find(|b| b.owns(kind, lane))
    }

    /// The reel that is being edited now. `None` is the root.
    pub fn editing(&self) -> Option<MediaId> {
        self.editing
    }

    /// Enter the reel (or exit to the root). A nonexistent address is a
    /// silent exit to the root: the reel could have disappeared under undo.
    pub fn set_editing(&mut self, seq: Option<MediaId>) {
        let want = seq.filter(|id| {
            // the address is looked up in the DOCUMENT: standing inside X, X itself in
            // `project.sequences` holds the content of the root, but its id is the former one —
            // and that is enough
            self.project.sequence(*id).is_some()
        });
        if want == self.editing {
            return;
        }
        // A full entry and an in-place expansion are TWO ways of looking inside, and together
        // they are meaningless: the band was counted from the root, and the root
        // would become a different reel.
        self.bands.clear();
        if let Some(cur) = self.editing {
            self.project.swap_reel(cur);
        }
        if let Some(next) = want {
            self.project.swap_reel(next);
        }
        self.editing = want;
        self.last_gesture = None;
        self.rebuild_bench();
    }

    /// The document in which the CURRENT reel stands as the ROOT (R.50, B1).
    ///
    /// It is needed by offline paths that count by the time of what is at hand:
    /// loudness, ducking, material preview. The working view must not be given to them —
    /// in it the reel inside which we stand holds the content of the root and looks
    /// nested inside itself.
    pub fn reel_document(&self) -> Project {
        let mut p = self.project.clone();
        if let Some(cur) = self.editing {
            p.sequences.retain(|s| s.id != cur);
        }
        p
    }

    /// The project AS THE PERSON SEES IT: the content of the reel in which he now
    /// stands lies in place of the root (R.50, B1), and the reels expanded in place are laid out
    /// on tracks with an offset (R.55, C5).
    ///
    /// The whole interface — drawing, hit testing, gesture planning, snapping,
    /// selection, the FX and morpher windows — works with this view and knows nothing
    /// about attachments. Whoever needs the DOCUMENT (engine, saving, export)
    /// needs `document()`; the guard is the scanning test t456b.
    ///
    /// The name is NOT renamed to `bench` on purpose: `project()` is called
    /// 182 times, and all these calls are correct — they need precisely the working view.
    /// A renaming would clarify nothing, and the difference “canon versus view” lives in the
    /// doc of the fields and in t456b.
    #[allow(clippy::misnamed_getters)]
    pub fn project(&self) -> &Project {
        &self.bench
    }

    /// The DOCUMENT in its real layout: the root as the root, reels as reels.
    ///
    /// It is exactly this that goes into the player, into the file and into the export. Handing
    /// the working view there would mean writing to disk a project whose root and reel
    /// contents have swapped places — a loss of work that the person would notice
    /// only at the next opening.
    pub fn document(&self) -> Project {
        let mut p = self.project.clone();
        if let Some(seq) = self.editing {
            p.swap_reel(seq);
        }
        p
    }

    /// The event BY WHAT THE PERSON SEES (R.55, C5).
    ///
    /// It searches in the working view, not in the canon: the interface asks about what
    /// was clicked, and an event of an expanded band lies inside the reel in the canon.
    /// If it searched the canon here, every gesture on the band would answer
    /// “there is no such event”, and silently so.
    pub fn clip(&self, id: ClipId) -> Option<&ClipEvent> {
        self.bench.clips.iter().find(|c| c.id == id)
    }

    /// Half-open interval of a clip on the timeline.
    pub fn clip_range(c: &ClipEvent) -> TickRange {
        TickRange::new(c.start, c.start + c.duration)
    }

    pub fn apply(&mut self, cmd: Command) -> Result<(), CommandError> {
        self.apply_batch(vec![cmd]).map(|_| ())
    }

    /// A batch of commands = ONE undo snapshot per gesture (split of an A/V
    /// pair, deletion of a group, paste, punch-out). Atomic: validation of the whole series on a clone,
    /// a failure of any command = zero changes and zero entries in history.
    pub fn apply_batch(&mut self, cmds: Vec<Command>) -> Result<Births, CommandError> {
        if cmds.is_empty() {
            return Ok(Births::default());
        }
        let scratch = self.simulate(&cmds)?;
        let births = Births::of(&cmds);
        self.push_undo();
        self.project = scratch;
        // BANDS could have survived an edit with other track numbers
        // (`InsertLane` at the root moves the whole band down): the recalculation is in
        // the same place where the result is applied, and only once.
        for b in &mut self.bands {
            b.after_batch(&cmds);
        }
        self.bands.retain(|b| self.project.clips.iter().any(|c| c.id == b.inst));
        self.rebuild_bench();
        self.last_gesture = None;
        Ok(births)
    }

    /// The project as it WILL BE after the batch — without writing to undo (R.26).
    /// It is needed by call sites that must count something by the future
    /// state: the return of the crossfade “handles” counts the free space already
    /// after the clip has moved (ops::restore_handles).
    pub fn preview(&self, cmds: &[Command]) -> Result<Project, CommandError> {
        // The WORKING VIEW of the future, not the canon: the caller compares it with
        // `project()` (the return of the crossfade “handles”, R.26). Were it to compare the canon
        // with the bench, material would be quietly and undoably trimmed off other people’s events,
        // and with undo.
        let canon = self.simulate(cmds)?;
        let mut bench = canon;
        for b in &self.bands {
            bench.merge_reel(b);
        }
        Ok(bench)
    }

    fn simulate(&self, cmds: &[Command]) -> Result<Project, CommandError> {
        // THE ORDER AND THE NUMBER OF ROWS do not change while a band is open (R.56,
        // item 449). Since R.56 `SetLaneOrder` renumbers the content, and the expansion band
        // deliberately lives at the TAIL of the numeric space:
        // if we renumbered the bench, `unmerge_reel` would sort the events
        // into the wrong reels, and SILENTLY so.
        // The application folds the expansion itself and says so.
        if !self.bands.is_empty()
            && cmds
                .iter()
                .any(|c| matches!(c, Command::SetLaneOrder { .. } | Command::RemoveTrack { .. }))
        {
            return Err(CommandError::BandsOpen);
        }
        // A scratch is already in the WORKING view: the content of the current reel stands in
        // place of the root (R.50, B1), and the command edits it without knowing
        // about attachments at all.
        let mut scratch = self.bench.clone();
        // WHERE WE STAND: needed only by the address-based publication commands — the set
        // of the reel in which we stand lies in the root of the working view, and the
        // others lie in `sequences` (R.63 item 11)
        let scope = self.editing;
        let run = |scratch: &mut Project| -> Result<(), CommandError> {
            for cmd in cmds {
                Self::check_on(scratch, scope, cmd)?;
                Self::mutate_on(scratch, scope, cmd.clone());
            }
            Ok(())
        };
        let res = run(&mut scratch);
        // reconcile — ONCE for the whole batch: the gesture is atomic, and a transition
        // must not die in a transitional state between commands (the roll of a joint =
        // TrimEnd+TrimStart: between them the joint is formally broken)
        Self::reconcile_transitions(&mut scratch);
        res?;
        // EXPANDED BANDS are collected back into their reels (R.55, C5) —
        // in reverse order, because each next one was expanded
        // over the already expanded ones. An error here means
        // that the gesture carried an event across the boundary of reels: the batch is rejected
        // ENTIRELY, rather than “patched up”.
        let mut bands = self.bands.clone();
        for b in &mut bands {
            b.after_batch(cmds);
        }
        for b in bands.iter().rev() {
            scratch.unmerge_reel(b).map_err(CommandError::InvalidLayout)?;
        }
        // The fate of publications — by the SAME history step and on the same pattern
        // as for transitions: the target has disappeared — the handle disappears too,
        // and all its overrides in all instances of the tree. Otherwise a handle remains
        // outside that does nothing, and the consumer finds out about it
        // (minus reconnaissance F-TL-012).
        //
        // AFTER the assembly of bands, and this is not order for the sake of order:
        // while a band is expanded, the content of the reel physically lies in the root,
        // and its publications remain in `sequences` — each target would look missing,
        // and expanding would quietly wipe out the entire public set.
        Self::reconcile_published(&mut scratch, self.editing);
        // Validation — BY DOCUMENT: in the working view the instance of the reel in which we stand
        // lies inside itself and looks like a cycle.
        if let Some(seq) = self.editing {
            scratch.swap_reel(seq);
        }
        // Hard validation of the result (R.24, stage 6): overlaps after group moves/trims/inserts
        // are cheaper to catch with one check than to duplicate their logic
        // in check_on. A failure = zero changes.
        let ok = scratch.validate().map_err(CommandError::InvalidLayout);
        if let Some(seq) = self.editing {
            scratch.swap_reel(seq);
        }
        ok?;
        Ok(scratch)
    }

    /// Like apply, but a series of calls with one `gesture` token is merged into
    /// one undo entry (live editing of FX parameters from the window).
    pub fn apply_coalesced(&mut self, cmd: Command, gesture: u64) -> Result<(), CommandError> {
        let scratch = self.simulate(std::slice::from_ref(&cmd))?;
        if self.last_gesture != Some(gesture) {
            self.push_undo();
        }
        self.project = scratch;
        for b in &mut self.bands {
            b.after_batch(std::slice::from_ref(&cmd));
        }
        self.bands.retain(|b| self.project.clips.iter().any(|c| c.id == b.inst));
        self.rebuild_bench();
        self.last_gesture = Some(gesture);
        Ok(())
    }

    fn push_undo(&mut self) {
        self.undo.push(Snapshot {
            project: self.project.clone(),
            scope: self.editing,
            bands: self.bands.clone(),
        });
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// The region is OUTSIDE the undo history (the semantics of the old editor).
    pub fn set_region(&mut self, r: Option<Region>) {
        self.project.region = r;
        self.bench.region = r;
    }

    pub fn set_loop_enabled(&mut self, on: bool) {
        self.project.loop_enabled = on;
        self.bench.loop_enabled = on;
    }

    /// The MEASURED length of the material — outside undo (R.63 item 2).
    ///
    /// This is not an edit but a FACT about the file: whoever opened the source brings it,
    /// and brings it asynchronously (background media sync, return of a vanished
    /// file). It cannot be a command, for two reasons: there is nothing to roll back in
    /// “we learned that the file lasts 226 s”, and the asterisk “project changed” from
    /// opening a source is a direct violation of the rule that “conveniences do not pretend
    /// the user changed something” (item 5 of round 11).
    /// The neighbors on this shelf are the region, loop and viewport.
    ///
    /// Returns `true` if the value has REALLY changed: the sync is called on every
    /// frame, and a silent “the same” must not move
    /// either the revision or the autosave.
    pub fn note_media_length(&mut self, id: crate::MediaId, duration: kt_time::Tick) -> bool {
        let set = |p: &mut crate::Project| -> bool {
            match p.media.iter_mut().find(|m| m.id == id) {
                Some(m) if m.duration != Some(duration) && duration.0 > 0 => {
                    m.duration = Some(duration);
                    true
                }
                _ => false,
            }
        };
        // the length lives with the media of the PROJECT (the media list is one per document), and the
        // working view and the canon are different projects: we edit both, the same as the region
        let a = set(&mut self.project);
        let b = set(&mut self.bench);
        a || b
    }

    /// The viewport (outside undo, like the region) — saved to the project file.
    pub fn set_ui_state(&mut self, ui: Option<crate::UiState>) {
        self.project.ui = ui.clone();
        self.bench.ui = ui;
    }

    pub fn undo(&mut self) -> Result<(), CommandError> {
        let mut prev = self.undo.pop().ok_or(CommandError::NothingToUndo)?;
        // region/loop are not rolled back — we carry over the current values
        prev.project.region = self.project.region;
        prev.project.loop_enabled = self.project.loop_enabled;
        // The CONTEXT travels together with the EDIT, not with the place where the person
        // stood at the moment of undo (R.50, B1): what he did THERE is undone,
        // so that place must be shown too. And the redo entry remembers the SAME context,
        // otherwise the reverse step would lead to the wrong place.
        let here = Snapshot {
            project: self.project.clone(),
            scope: prev.scope,
            bands: self.bands.clone(),
        };
        self.project = prev.project;
        // The snapshot already carries the layout of ITS OWN context — an exchange here would be a
        // second one, and the content would move to the wrong place.
        self.editing = prev.scope.filter(|id| self.project.sequence(*id).is_some());
        // BANDS travel together with the context for the same reason: an edit made IN THE EXPANSION
        // is undone, so the expansion must be shown too,
        // otherwise Ctrl+Z looks like “pressed, and everything vanished”.
        self.bands = prev.bands;
        self.bands.retain(|b| self.project.clips.iter().any(|c| c.id == b.inst));
        self.rebuild_bench();
        self.redo.push(here);
        self.last_gesture = None;
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), CommandError> {
        let mut next = self.redo.pop().ok_or(CommandError::NothingToRedo)?;
        next.project.region = self.project.region;
        next.project.loop_enabled = self.project.loop_enabled;
        let here = Snapshot {
            project: self.project.clone(),
            scope: next.scope,
            bands: self.bands.clone(),
        };
        self.project = next.project;
        self.editing = next.scope.filter(|id| self.project.sequence(*id).is_some());
        self.bands = next.bands;
        self.bands.retain(|b| self.project.clips.iter().any(|c| c.id == b.inst));
        self.rebuild_bench();
        self.undo.push(here);
        self.last_gesture = None;
        Ok(())
    }

    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// The set of publications of a NAMED reel in the WORKING view (R.63 item 11).
    ///
    /// Entering a reel is done by EXCHANGING the content, so the set of the reel
    /// in which you stand lies in the root, and the set of all the others lies
    /// in `sequences`. An expanded band moves the content into the root
    /// and leaves the reel’s set in place (t672) — and this is exactly the case
    /// for which the address was introduced.
    fn published_slot(
        p: &Project,
        scope: Option<MediaId>,
        seq: MediaId,
    ) -> Option<&Vec<PublishedParam>> {
        if scope == Some(seq) {
            Some(&p.published)
        } else {
            p.sequence(seq).map(|s| &s.published)
        }
    }

    fn published_slot_mut(
        p: &mut Project,
        scope: Option<MediaId>,
        seq: MediaId,
    ) -> Option<&mut Vec<PublishedParam>> {
        if scope == Some(seq) {
            Some(&mut p.published)
        } else {
            p.sequence_mut(seq).map(|s| &mut s.published)
        }
    }

    fn check_on(p: &Project, scope: Option<MediaId>, cmd: &Command) -> Result<(), CommandError> {
        // TRACK LOCK (R.62): one check for the whole registry of commands.
        // The properties of the track itself do not fall here — a locked track
        // must be unlockable by something.
        for (k, l) in cmd.target_rows(p) {
            if p.track_at(k, l).is_some_and(|t| t.lock) {
                return Err(CommandError::TrackLocked(
                    p.track_at(k, l).map(|t| t.id).unwrap_or_default(),
                ));
            }
        }
        let find =
            |id: ClipId| p.clips.iter().find(|c| c.id == id).ok_or(CommandError::ClipNotFound(id));
        match cmd {
            // the note about kinship requires nothing of the project: it is about the batch
            Command::NoteBirth { .. } => {}
            Command::AddMedia { id, .. } => {
                if p.media.iter().any(|m| m.id == *id) {
                    return Err(CommandError::IdInUse(id.0));
                }
            }
            Command::SetMediaPath { id, .. } => {
                if !p.media.iter().any(|m| m.id == *id) {
                    return Err(CommandError::MediaNotFound(*id));
                }
            }
            Command::AddClip(c) => {
                if p.clips.iter().any(|x| x.id == c.id) {
                    return Err(CommandError::IdInUse(c.id.0));
                }
                // from a CLIP to the source — only through `Project::source()`:
                // it looks both into the files and into the nested reels (R.50)
                if p.source(c.media).is_none() {
                    return Err(CommandError::MediaNotFound(c.media));
                }
                if c.duration < MIN_CLIP_DURATION {
                    return Err(CommandError::TooShort);
                }
                if c.source_in.0 < 0 {
                    return Err(CommandError::InvalidClip(ProjectError::NegativeSourceIn(c.id)));
                }
            }
            Command::RemoveClip { id }
            | Command::SetGain { id, .. }
            | Command::SetOpacity { id, .. }
            | Command::SetFxOff { id, .. }
            | Command::SetReversed { id, .. }
            | Command::SetFxBypass { id, .. }
            | Command::UnlinkClip { id }
            | Command::MoveClip { id, .. } => {
                find(*id)?;
            }
            Command::SetFx { id, fx } => {
                find(*id)?;
                // the sidechain lives ONLY in the track’s chain (the owner’s decision,
                // R.65): an event has neither its own key window nor
                // a place in the order of stages
                if crate::sidechain_keys(fx).next().is_some() {
                    return Err(CommandError::SidechainOnClip);
                }
            }
            Command::SetGeometry { id, pos, .. } => {
                let c = find(*id)?;
                if *pos > c.fx.len() {
                    return Err(CommandError::InvalidClip(ProjectError::BadGeometryPos(*id)));
                }
            }
            Command::SetFxAuto { id, auto } => {
                find(*id)?;
                check_fx_auto(*id, auto).map_err(CommandError::InvalidClip)?;
            }
            Command::TrimStart { id, new_start } => {
                let c = find(*id)?;
                let end = c.start + c.duration;
                if end - *new_start < MIN_CLIP_DURATION {
                    return Err(CommandError::TooShort);
                }
                // source_in cannot go negative (for reversed, the left trim
                // does not touch source_in — item 16). The SAME function computes it as the mutation:
                // otherwise validation and the edit would diverge on a stretched
                // clip (R.39).
                if !slice_fits_source(c, TickRange::new(*new_start, end)) {
                    return Err(CommandError::TooShort);
                }
            }
            Command::TrimEnd { id, new_end } => {
                let c = find(*id)?;
                if *new_end - c.start < MIN_CLIP_DURATION {
                    return Err(CommandError::TooShort);
                }
                // for reversed, extending to the right pulls source_in down (item 16)
                if c.reversed && (c.source_in + (c.start + c.duration - *new_end)).0 < 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::Split { id, at, new_id, .. } => {
                let c = find(*id)?;
                if p.clips.iter().any(|x| x.id == *new_id) {
                    return Err(CommandError::IdInUse(new_id.0));
                }
                let r = Self::clip_range(c);
                if !(r.contains(*at)
                    && (*at - r.start) >= MIN_CLIP_DURATION
                    && (r.end - *at) >= MIN_CLIP_DURATION)
                {
                    return Err(CommandError::SplitOutside);
                }
            }
            Command::LinkClips { ids, .. } => {
                for id in ids {
                    find(*id)?;
                }
            }
            Command::AddTransition(t) => {
                if p.transitions.iter().any(|x| x.id == t.id) {
                    return Err(CommandError::IdInUse(t.id.0));
                }
                check_transition_full(p, t, None).map_err(CommandError::InvalidTransition)?;
            }
            Command::RemoveTransition { id } => {
                if !p.transitions.iter().any(|t| t.id == *id) {
                    return Err(CommandError::TransitionNotFound(*id));
                }
            }
            Command::SetTransition(t) => {
                if !p.transitions.iter().any(|x| x.id == t.id) {
                    return Err(CommandError::TransitionNotFound(t.id));
                }
                check_transition_full(p, t, Some(t.id)).map_err(CommandError::InvalidTransition)?;
            }
            Command::AddMarker { at, duration } => {
                if p.markers.iter().any(|m| m.t == *at) {
                    return Err(CommandError::MarkerExists(*at));
                }
                if duration.0 < 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::RemoveMarker { at } => {
                if !p.markers.iter().any(|m| m.t == *at) {
                    return Err(CommandError::MarkerNotFound(*at));
                }
            }
            Command::SetMarkerDuration { at, duration } => {
                if !p.markers.iter().any(|m| m.t == *at) {
                    return Err(CommandError::MarkerNotFound(*at));
                }
                if duration.0 < 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::SetMarkerName { at, name } => {
                // one namespace with the areas (R.35 item 2)
                if !name.is_empty()
                    && p.markers.iter().any(|m| m.t == *at && m.duration.0 > 0)
                    && p.taken_names().iter().any(|t| t == name)
                    && !p.markers.iter().any(|m| m.t == *at && m.name == *name)
                {
                    return Err(CommandError::NameTaken(name.clone()));
                }
                if !p.markers.iter().any(|m| m.t == *at) {
                    return Err(CommandError::MarkerNotFound(*at));
                }
            }
            Command::MoveMarker { from, to } => {
                if !p.markers.iter().any(|m| m.t == *from) {
                    return Err(CommandError::MarkerNotFound(*from));
                }
                if p.markers.iter().any(|m| m.t == *to) && from != to {
                    return Err(CommandError::MarkerExists(*to));
                }
            }
            Command::InsertTrack { .. } => {}
            Command::RemoveTrack { kind, at } => {
                let n = p.lanes(*kind);
                if *at >= n {
                    return Err(CommandError::TrackMissing(*kind, *at));
                }
                if matches!(kind, LaneKind::Video | LaneKind::Audio) && n <= 1 {
                    return Err(CommandError::LastTrack(*kind));
                }
            }
            Command::SetTrackFx { id, fx, bypass } => {
                if p.track(*id).is_none() {
                    return Err(CommandError::TrackNotFound(*id));
                }
                if bypass.len() > fx.len() {
                    return Err(CommandError::TrackPropOutOfRange(*id));
                }
                // the track’s chain is ONLY audio or only video, by its kind: a video effect on an
                // audio track would be a dead slot
                let kind = p.track(*id).map(|t| t.kind);
                let wrong = fx.iter().any(|f| match kind {
                    Some(LaneKind::Audio) => f.class() != FxClass::Audio,
                    Some(LaneKind::Video) => f.class() == FxClass::Audio,
                    _ => true,
                });
                if wrong {
                    return Err(CommandError::TrackPropOutOfRange(*id));
                }
                // SIDECHAIN KEY (R.65): only to ITS OWN reel, only an audio track,
                // not to itself and without a cycle. The refusal is named:
                // a silent “the key did not work” would be worse than an error.
                for k in crate::sidechain_keys(fx) {
                    let ok = k != *id
                        && p.track(k).is_some_and(|t| t.kind == LaneKind::Audio)
                        && p.tracks.iter().any(|t| t.id == k);
                    if !ok {
                        return Err(CommandError::SidechainKeyInvalid(k));
                    }
                }
                if crate::sidechain_cycle(&p.tracks, *id, fx) {
                    return Err(CommandError::SidechainCycle(*id));
                }
            }
            Command::SetTrackFxOff { id, .. } => {
                if p.track(*id).is_none() {
                    return Err(CommandError::TrackNotFound(*id));
                }
            }
            Command::SetTrackProps { id, props } => {
                if p.track(*id).is_none() {
                    return Err(CommandError::TrackNotFound(*id));
                }
                if !props.opacity.is_finite()
                    || !(0.0..=1.0).contains(&props.opacity)
                    || !props.gain_db.is_finite()
                    || !(TRACK_GAIN_MIN_DB..=TRACK_GAIN_MAX_DB).contains(&props.gain_db)
                    || !props.height.is_finite()
                    || props.height < 0.0
                {
                    return Err(CommandError::TrackPropOutOfRange(*id));
                }
            }

            Command::RemoveMedia { id } => {
                if !p.media.iter().any(|m| m.id == *id) {
                    return Err(CommandError::MediaNotFound(*id));
                }
                // ACROSS THE WHOLE TREE, not only the root (R.50): a file that stands
                // only inside a reel would be deleted silently, and the events would hang
                // on a nonexistent address — it would surface a week later
                if p.sequence_uses(*id) > 0 {
                    return Err(CommandError::MediaInUse(*id));
                }
                // a morpher with a substitution on this media also holds it
                if p.morphers
                    .iter()
                    .chain(p.sequences.iter().flat_map(|s| s.morphers.iter()))
                    .any(|m| m.media_refs().any(|x| x == *id))
                {
                    return Err(CommandError::MediaInUse(*id));
                }
            }
            Command::AddSequence { id, name } => {
                if p.source(*id).is_some() {
                    return Err(CommandError::IdInUse(id.0));
                }
                if name.trim().is_empty() {
                    return Err(CommandError::InvalidClip(ProjectError::SequenceUnnamed(*id)));
                }
            }
            Command::RemoveSequence { id } => {
                if p.sequence(*id).is_none() {
                    return Err(CommandError::SequenceNotFound(*id));
                }
                let used = p.sequence_uses(*id);
                if used > 0 {
                    return Err(CommandError::SequenceInUse(*id, used));
                }
            }
            Command::RenameSequence { id, name } => {
                if p.sequence(*id).is_none() {
                    return Err(CommandError::SequenceNotFound(*id));
                }
                if name.trim().is_empty() {
                    return Err(CommandError::InvalidClip(ProjectError::SequenceUnnamed(*id)));
                }
            }
            Command::SetSequenceSettings { id, raster, fps } => {
                let Some(s) = p.sequence(*id) else {
                    return Err(CommandError::SequenceNotFound(*id));
                };
                let mut probe = s.clone();
                probe.raster = *raster;
                probe.fps = *fps;
                if !probe.settings_ok() {
                    return Err(CommandError::InvalidClip(ProjectError::BadSequenceSettings(*id)));
                }
            }
            Command::SetSequenceContent { id, content } => {
                if p.sequence(*id).is_none() {
                    return Err(CommandError::SequenceNotFound(*id));
                }
                if content.id != *id {
                    return Err(CommandError::SequenceNotFound(content.id));
                }
                // the number of tracks in a reel is not less than what its events occupy:
                // “SetLanes will not orphan clips” must hold here too
                let need = |k: ClipKind| {
                    content.clips.iter().filter(|c| c.kind == k).map(|c| c.lane + 1).max()
                };
                if content.lanes(LaneKind::Video) < need(ClipKind::Video).unwrap_or(1).max(1)
                    || content.lanes(LaneKind::Audio) < need(ClipKind::Audio).unwrap_or(1).max(1)
                {
                    return Err(CommandError::LanesTooFew);
                }
            }
            Command::SetClipMedia { id, media } => {
                if !p.clips.iter().any(|c| c.id == *id) {
                    return Err(CommandError::ClipNotFound(*id));
                }
                if p.source(*media).is_none() {
                    return Err(CommandError::MediaNotFound(*media));
                }
            }
            // Publication is checked by the SAME code as the validation of the file:
            // there cannot be a second rule for “what counts as a live target”.
            Command::PublishParam { seq, param } => {
                let set = Self::published_slot(p, scope, *seq)
                    .ok_or(CommandError::SequenceNotFound(*seq))?;
                if set.iter().any(|x| x.id == param.id) {
                    return Err(CommandError::IdInUse(param.id.0));
                }
                Self::check_published_on(p, scope, *seq, param)?;
            }
            Command::SetPublished { seq, param } => {
                let set = Self::published_slot(p, scope, *seq)
                    .ok_or(CommandError::SequenceNotFound(*seq))?;
                if !set.iter().any(|x| x.id == param.id) {
                    return Err(CommandError::PublishedNotFound(param.id));
                }
                Self::check_published_on(p, scope, *seq, param)?;
            }
            Command::UnpublishParam { seq, id } => {
                let set = Self::published_slot(p, scope, *seq)
                    .ok_or(CommandError::SequenceNotFound(*seq))?;
                if !set.iter().any(|x| x.id == *id) {
                    return Err(CommandError::PublishedNotFound(*id));
                }
            }
            Command::ReorderPublished { seq, order } => {
                // A PERMUTATION, not an edit of the set: the order must be a permutation of exactly
                // what is declared, otherwise the command would silently lose or duplicate
                // handles.
                let set = Self::published_slot(p, scope, *seq)
                    .ok_or(CommandError::SequenceNotFound(*seq))?;
                let mut a: Vec<u64> = order.iter().map(|x| x.0).collect();
                let mut b: Vec<u64> = set.iter().map(|x| x.id.0).collect();
                a.sort_unstable();
                b.sort_unstable();
                if a != b {
                    return Err(CommandError::PublishedOrderBroken);
                }
            }
            Command::SetOverride { id, ov } => {
                let c = find(*id)?;
                let mut probe = c.clone();
                probe.overrides.retain(|x| x.pub_id != ov.pub_id);
                probe.overrides.push((**ov).clone());
                p.validate_overrides_for(&probe).map_err(CommandError::InvalidClip)?;
            }
            Command::ClearOverride { id, .. } => {
                find(*id)?;
            }
            // The generator values are checked by the SAME code as the validation
            // of the file (`check_clip_gen`): there cannot be a second rule for “what
            // counts as consistent”; were they to diverge, the window
            // would set a state that the project
            // would then refuse to save.
            Command::SetClipGen { id, kind } => {
                let c = find(*id)?;
                let mut probe = c.clone();
                probe.r#gen = kind.clone();
                if kind.is_none() {
                    probe.gen_auto.clear();
                }
                kt_check(p, &probe)?;
            }
            Command::SetClipGenAuto { id, auto } => {
                let c = find(*id)?;
                let mut probe = c.clone();
                probe.gen_auto = auto.clone();
                kt_check(p, &probe)?;
            }
            Command::ApplyGenToAll { media, kind, auto } => {
                if p.gen_media_path(*media).is_none() {
                    return Err(CommandError::MediaNotFound(*media));
                }
                let mut probe = ClipEvent {
                    r#gen: Some(kind.clone()),
                    gen_auto: auto.clone(),
                    media: *media,
                    ..p.clips
                        .iter()
                        .find(|c| c.media == *media)
                        .cloned()
                        .ok_or(CommandError::MediaNotFound(*media))?
                };
                probe.media = *media;
                kt_check(p, &probe)?;
            }
            Command::SetSettings { settings } => {
                if settings.fps == 0 || kt_time::TICK_HZ % settings.fps as i64 != 0 {
                    return Err(CommandError::InvalidClip(ProjectError::BadFps));
                }
            }
            Command::AddMorpher(m) => {
                if p.morphers.iter().any(|x| x.id == m.id) {
                    return Err(CommandError::IdInUse(m.id.0));
                }
                Self::check_morpher_on(p, m)?;
            }
            Command::SetMorpher(m) => {
                if !p.morphers.iter().any(|x| x.id == m.id) {
                    return Err(CommandError::MorpherNotFound(m.id));
                }
                Self::check_morpher_on(p, m)?;
            }
            Command::RemoveMorpher { id } | Command::BakeMorpher { id } => {
                if !p.morphers.iter().any(|x| x.id == *id) {
                    return Err(CommandError::MorpherNotFound(*id));
                }
            }
            Command::SetLaneOrder { .. } | Command::SetTrimStretchesAuto { .. } => {}
            Command::AddArea(a) => {
                if a.name.trim().is_empty() {
                    return Err(CommandError::AreaUnnamed);
                }
                if p.taken_names().contains(&a.name) {
                    return Err(CommandError::NameTaken(a.name.clone()));
                }
                if a.duration.0 <= 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::RemoveArea { name } | Command::SetAreaLanes { name, .. } => {
                if p.area(name).is_none() {
                    return Err(CommandError::AreaNotFound(name.clone()));
                }
            }
            Command::SetAreaRange { name, duration, .. } => {
                if p.area(name).is_none() {
                    return Err(CommandError::AreaNotFound(name.clone()));
                }
                if duration.0 <= 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::RenameArea { from, to } => {
                if p.area(from).is_none() {
                    return Err(CommandError::AreaNotFound(from.clone()));
                }
                if to.trim().is_empty() {
                    return Err(CommandError::AreaUnnamed);
                }
                if to != from && p.taken_names().contains(to) {
                    return Err(CommandError::NameTaken(to.clone()));
                }
            }
            Command::AddCamera(c) => {
                if p.cameras.iter().any(|x| x.id == c.id) {
                    return Err(CommandError::IdInUse(c.id.0));
                }
                Self::check_camera_on(c)?;
            }
            Command::SetCamera(c) => {
                if !p.cameras.iter().any(|x| x.id == c.id) {
                    return Err(CommandError::CameraNotFound(c.id));
                }
                Self::check_camera_on(c)?;
            }
            Command::RemoveCamera { id } => {
                if !p.cameras.iter().any(|x| x.id == *id) {
                    return Err(CommandError::CameraNotFound(*id));
                }
            }
        }
        Ok(())
    }

    fn check_morpher_on(p: &Project, m: &MorpherEvent) -> Result<(), CommandError> {
        // an empty list is forbidden ONLY in the mode of frozen targets: for a
        // layer (“everything below”) and for references an empty set is legal —
        // it is simply that right now there is nothing under the morpher
        if let MorphTargets::Clips { ids } = &m.targets {
            if ids.is_empty() {
                return Err(CommandError::MorpherNoTargets);
            }
            for t in ids {
                if !p.clips.iter().any(|c| c.id == *t) {
                    return Err(CommandError::ClipNotFound(*t));
                }
            }
        }
        for mid in m.media_refs() {
            if !p.media.iter().any(|x| x.id == mid) {
                return Err(CommandError::MediaNotFound(mid));
            }
        }
        Ok(())
    }

    /// The check of the DECLARATION of a public parameter by the same code as
    /// the validation of the file (R.59, stage 3).
    ///
    /// The reel is NAMED (R.63 item 11), and the target is searched for in it. The content of
    /// the reel in which you stand lies in the root of the working view — as does the content of the reel
    /// EXPANDED as a BAND, although its set of publications remained in `sequences`. Therefore the search
    /// for the target falls back to the root:
    /// one cannot get this wrong — the resulting document still passes
    /// `validate`, and a foreign target is rejected there.
    fn check_published_on(
        p: &Project,
        scope: Option<MediaId>,
        seq: MediaId,
        param: &PublishedParam,
    ) -> Result<(), CommandError> {
        if param.name.trim().is_empty() {
            return Err(CommandError::InvalidClip(ProjectError::PublishedUnnamed(None, param.id)));
        }
        let in_reel = (scope != Some(seq)).then_some(seq);
        let found = p.addr_type(in_reel, &param.target).or_else(|| p.addr_type(None, &param.target));
        let Some(ty) = found else {
            return Err(CommandError::InvalidClip(ProjectError::PublishedTargetMissing(
                None, param.id,
            )));
        };
        if !ty.same_family(&param.ty) {
            return Err(CommandError::InvalidClip(ProjectError::PublishedTypeMismatch(
                None, param.id,
            )));
        }
        Ok(())
    }

    /// The check of a camera event by the same rules as validate() (R.29):
    /// the command has no right to put into the project what the project would then reject.
    fn check_camera_on(c: &CameraEvent) -> Result<(), CommandError> {
        if c.duration.0 <= 0 {
            return Err(CommandError::InvalidCamera(ProjectError::CameraBadDuration(c.id)));
        }
        let fovs = std::iter::once(c.cam.fov_deg).chain(c.keyframes.iter().map(|k| k.cam.fov_deg));
        for fov in fovs {
            if !(fov > 0.0 && fov < 180.0) {
                return Err(CommandError::InvalidCamera(ProjectError::CameraBadFov(c.id)));
            }
        }
        let sorted = c.keyframes.windows(2).all(|w| w[0].t <= w[1].t);
        if !sorted || c.keyframes.iter().any(|k| !(0.0..=1.0).contains(&k.t)) {
            return Err(CommandError::InvalidCamera(ProjectError::CameraBadKeyframes(c.id)));
        }
        Ok(())
    }

    fn mutate_on(p: &mut Project, scope: Option<MediaId>, cmd: Command) {
        match cmd {
            Command::NoteBirth { .. } => {}
            Command::AddMedia { id, path } => {
                // The generator path carries ONLY the KIND (R.59, stage 2) — and
                // is brought to the canon in the same two places as in
                // `SetMediaPath`. The values arrive separately, on the EVENT:
                // otherwise the very first check would stumble on “the kind of values
                // is not the one in the path”.
                let path = match crate::genmedia::parse_gen_path(&path) {
                    Some(k) => crate::genmedia::gen_kind_path(&k),
                    None => path,
                };
                p.media.push(MediaRef::new(id, path));
            }
            Command::SetMediaPath { id, path } => {
                // The generator path carries ONLY the KIND (R.59, stage 2), and this place brings it
                // to the canon: the command remained the door of relink,
                // and for it the path is still opaque,
                // therefore the window may send the full old path as well.
                // The events on this media must remain consistent —
                // otherwise the very first edit would run into `GenKindMismatch`.
                let canon = match crate::genmedia::parse_gen_path(&path) {
                    Some(k) => crate::genmedia::gen_kind_path(&k),
                    None => path,
                };
                if let Some(m) = p.media.iter_mut().find(|m| m.id == id) {
                    m.path = canon;
                }
                let mut touched: Vec<ClipEvent> = std::mem::take(&mut p.clips);
                for c in touched.iter_mut().filter(|c| c.media == id) {
                    fit_gen_to_media(p, c);
                }
                p.clips = touched;
                for i in 0..p.sequences.len() {
                    let mut cs = std::mem::take(&mut p.sequences[i].clips);
                    for c in cs.iter_mut().filter(|c| c.media == id) {
                        fit_gen_to_media(p, c);
                    }
                    p.sequences[i].clips = cs;
                }
            }
            Command::AddClip(mut c) => {
                p.ensure_tracks(lane_key_of(c.kind), c.lane + 1);
                fit_gen_to_media(p, &mut c);
                p.clips.push(c)
            }
            Command::RemoveClip { id } => {
                p.clips.retain(|c| c.id != id);
                // D5 (v1): we clean the ids out of the frozen targets of the morphers
                for m in &mut p.morphers {
                    if let Some(ids) = m.targets.clip_ids_mut() {
                        ids.retain(|t| *t != id);
                    }
                }
            }
            Command::MoveClip { id, new_start, new_lane } => {
                let (group, delta, kind) = {
                    let c = p.clips.iter().find(|c| c.id == id).unwrap();
                    (c.link_group, new_start - c.start, c.kind)
                };
                p.ensure_tracks(lane_key_of(kind), new_lane + 1);
                for c in &mut p.clips {
                    if c.id == id {
                        c.start = new_start;
                        c.lane = new_lane;
                    } else if group.is_some() && c.link_group == group {
                        // A/V link partners move by the same offset
                        c.start = c.start + delta;
                    }
                }
            }
            // Trims are applied to the WHOLE link group (v1; item 6 “the magnet falls apart”: a trim of a video edge
            // left the audio partner at its former length).
            // The partners are clamped to their own limits.
            // Reverse (item 16): for a reversed clip source_in lives “from the other end” — TrimStart does
            // NOT touch it, TrimEnd shifts it.
            Command::TrimStart { id, new_start } => {
                // the option “stretch the animation together with the event” (R.40):
                // OFF by default — the key stays on its frame
                let stretch = p.trim_stretches_auto;
                let group = p.clips.iter().find(|c| c.id == id).and_then(|c| c.link_group);
                for c in &mut p.clips {
                    if c.id != id && (group.is_none() || c.link_group != group) {
                        continue;
                    }
                    let max_start = c.start + c.duration - MIN_CLIP_DURATION;
                    let ns = Tick(new_start.0.clamp(trim_min_start(c).0, max_start.0));
                    // geometry and material — by ONE rule (R.39):
                    // for a stretched clip, `source_in` moves along the integral
                    // of the speed, not by the length of the piece
                    let (span_old, anchor_old) = (crate::clip_material_span(c), c.source_in);
                    *c = clip_slice(c, TickRange::new(ns, c.start + c.duration), c.id);
                    if stretch {
                        let span_new = crate::clip_material_span(c);
                        crate::stretch_auto_span(c, anchor_old, span_old, span_new);
                    }
                }
            }
            Command::TrimEnd { id, new_end } => {
                let stretch = p.trim_stretches_auto;
                let group = p.clips.iter().find(|c| c.id == id).and_then(|c| c.link_group);
                // we take media lengths BEFORE the loop: inside `p.clips` is borrowed
                // mutably, and `media_duration` looks into the same project
                let lens: Vec<(ClipId, Option<Tick>)> = p
                    .clips
                    .iter()
                    .filter(|c| c.id == id || (group.is_some() && c.link_group == group))
                    .map(|c| (c.id, p.media_duration(c.media)))
                    .collect();
                for c in &mut p.clips {
                    if c.id != id && (group.is_none() || c.link_group != group) {
                        continue;
                    }
                    let media_len =
                        lens.iter().find(|(cid, _)| *cid == c.id).and_then(|(_, l)| *l);
                    let min_end = c.start + MIN_CLIP_DURATION;
                    // The edge of the material is the same stop as its start (R.63).
                    // The limit is NOT LESS than the current end:
                    // an event that is already longer than the material
                    // (files before R.63) is moved within what has been
                    // made and is trimmed freely — the event itself is not shortened (rule 20); only
                    // it is no longer allowed to grow further.
                    let was_end = (c.start + c.duration).0;
                    let limit = trim_max_end_in(c, media_len).0.max(was_end);
                    let ne = Tick(new_end.0.max(min_end.0).min(limit));
                    let (span_old, anchor_old) = (crate::clip_material_span(c), c.source_in);
                    *c = clip_slice(c, TickRange::new(c.start, ne), c.id);
                    if stretch {
                        let span_new = crate::clip_material_span(c);
                        crate::stretch_auto_span(c, anchor_old, span_old, span_new);
                    }
                }
            }
            Command::Split { id, at, new_id, new_group } => {
                // the pieces rule is ONE for the whole project — `clip_slice`
                // (reverse, item 16: the left half plays the TAIL of the source)
                let c = Self::clip_mut_on(p, id).clone();
                let body = TickRange::new(c.start, c.start + c.duration);
                *Self::clip_mut_on(p, id) = clip_slice(&c, TickRange::new(body.start, at), id);
                let mut right = clip_slice(&c, TickRange::new(at, body.end), new_id);
                right.link_group = new_group;
                p.clips.push(right);
                // D5 (v1): the right half inherits the targeting of the morphers
                for m in &mut p.morphers {
                    if let Some(ids) = m.targets.clip_ids_mut()
                        && ids.contains(&id)
                    {
                        ids.push(new_id);
                    }
                }
                // the transition where the clip was OUTGOING (the joint at the former END) moves
                // to the right half; the incoming joint (the start) stays
                // with the left one — it kept the id
                for t in &mut p.transitions {
                    if t.a == Some(id) {
                        t.a = Some(new_id);
                    }
                }
            }
            Command::SetTrimStretchesAuto { on } => p.trim_stretches_auto = on,
            Command::SetGain { id, gain_db } => Self::clip_mut_on(p, id).gain_db = gain_db,
            Command::SetOpacity { id, opacity } => {
                Self::clip_mut_on(p, id).opacity = opacity.clamp(0.0, 1.0)
            }
            Command::SetGeometry { id, geometry, pos, auto } => {
                let c = Self::clip_mut_on(p, id);
                c.geometry = geometry;
                c.geometry_pos = pos.min(c.fx.len());
                c.geometry_auto = auto;
            }
            Command::SetFxOff { id, off } => Self::clip_mut_on(p, id).fx_off = off,
            Command::SetReversed { id, reversed } => Self::clip_mut_on(p, id).reversed = reversed,
            Command::SetFxBypass { id, bypass } => {
                Self::clip_mut_on(p, id).fx_bypass = bypass;
                sync_warp_to_link_group(p, id);
            }
            Command::SetFx { id, fx } => {
                let c = Self::clip_mut_on(p, id);
                // the chain has changed — the curves of the extra slots drop out (R.30);
                // the editor itself handles the reordering of slots and sends SetFxAuto
                // within the same gesture
                c.fx_auto.truncate(fx.len());
                // the leading “Geometry” node (R.62) stood behind the end of
                // the shortened chain — it is pressed against its end
                c.geometry_pos = c.geometry_pos.min(fx.len());
                c.fx = fx;
                sync_warp_to_link_group(p, id);
            }
            Command::SetFxAuto { id, auto } => {
                Self::clip_mut_on(p, id).fx_auto = auto;
                sync_warp_to_link_group(p, id);
            }
            Command::SetClipGen { id, kind } => {
                let c = Self::clip_mut_on(p, id);
                if kind.is_none() {
                    c.gen_auto.clear();
                }
                c.r#gen = kind;
                sync_gen_to_link_group(p, id);
            }
            Command::SetClipGenAuto { id, auto } => {
                Self::clip_mut_on(p, id).gen_auto = auto;
                sync_gen_to_link_group(p, id);
            }
            Command::ApplyGenToAll { media, kind, auto } => {
                let apply = |clips: &mut Vec<ClipEvent>| {
                    for c in clips {
                        if c.media == media {
                            c.r#gen = Some(kind.clone());
                            c.gen_auto = auto.clone();
                        }
                    }
                };
                apply(&mut p.clips);
                for s in &mut p.sequences {
                    apply(&mut s.clips);
                }
            }
            Command::LinkClips { ids, group } => {
                for c in &mut p.clips {
                    if ids.contains(&c.id) {
                        c.link_group = Some(group);
                    }
                }
            }
            Command::UnlinkClip { id } => Self::clip_mut_on(p, id).link_group = None,
            Command::AddTransition(t) => p.transitions.push(t),
            Command::RemoveTransition { id } => p.transitions.retain(|t| t.id != id),
            Command::SetTransition(t) => {
                if let Some(slot) = p.transitions.iter_mut().find(|x| x.id == t.id) {
                    *slot = t;
                }
            }
            Command::AddMarker { at, duration } => {
                let i = p.markers.partition_point(|m| m.t < at);
                p.markers.insert(i, Marker { duration, ..Marker::at(at) });
            }
            Command::RemoveMarker { at } => p.markers.retain(|m| m.t != at),
            Command::SetMarkerName { at, name } => {
                if let Some(m) = p.markers.iter_mut().find(|m| m.t == at) {
                    m.name = name;
                }
            }
            Command::MoveMarker { from, to } => {
                // name, duration and color move TOGETHER with the marker
                let moved =
                    p.markers.iter().find(|m| m.t == from).cloned().unwrap_or(Marker::at(from));
                p.markers.retain(|m| m.t != from);
                let i = p.markers.partition_point(|m| m.t < to);
                p.markers.insert(i, Marker { t: to, ..moved });
            }
            Command::SetMarkerDuration { at, duration } => {
                if let Some(m) = p.markers.iter_mut().find(|m| m.t == at) {
                    m.duration = duration;
                }
            }
            Command::InsertTrack { kind, at } => {
                p.insert_track(kind, at);
            }
            Command::RemoveTrack { kind, at } => p.remove_track(kind, at),
            Command::SetTrackProps { id, props } => {
                if let Some(t) = p.track_mut(id) {
                    t.set_props(&props);
                }
            }
            Command::SetTrackFx { id, fx, bypass } => {
                if let Some(t) = p.track_mut(id) {
                    t.fx = fx;
                    t.fx_bypass = bypass;
                }
            }
            Command::SetTrackFxOff { id, off } => {
                if let Some(t) = p.track_mut(id) {
                    t.fx_off = off;
                }
            }
            Command::SetLaneOrder { order } => {
                p.set_lane_order(&order);
            }
            Command::RemoveMedia { id } => p.media.retain(|m| m.id != id),
            Command::AddSequence { id, name } => {
                p.sequences.push(Sequence::new(id, name));
                p.assign_track_ids();
            }
            Command::RemoveSequence { id } => p.sequences.retain(|s| s.id != id),
            Command::SetSequenceSettings { id, raster, fps } => {
                if let Some(s) = p.sequence_mut(id) {
                    s.raster = raster;
                    s.fps = fps;
                }
            }
            Command::RenameSequence { id, name } => {
                if let Some(s) = p.sequence_mut(id) {
                    s.name = name;
                }
            }
            Command::SetSequenceContent { id, content } => {
                if let Some(slot) = p.sequences.iter_mut().find(|s| s.id == id) {
                    *slot = *content;
                }
            }
            Command::SetClipMedia { id, media } => {
                let mut c = Self::clip_mut_on(p, id).clone();
                c.media = media;
                fit_gen_to_media(p, &mut c);
                // The instance moved to ANOTHER reel — someone else’s
                // overrides on it mean nothing.
                c.overrides.clear();
                *Self::clip_mut_on(p, id) = c;
            }
            Command::PublishParam { seq, param } => {
                if let Some(set) = Self::published_slot_mut(p, scope, seq) {
                    set.push(*param);
                }
            }
            Command::SetPublished { seq, param } => {
                if let Some(slot) = Self::published_slot_mut(p, scope, seq)
                    .and_then(|set| set.iter_mut().find(|x| x.id == param.id))
                {
                    *slot = *param;
                }
            }
            Command::UnpublishParam { seq, id } => {
                // The overrides of a removed handle are deleted by `reconcile_published`
                // in the SAME history step — across the whole document at once,
                // not only where this command reached.
                if let Some(set) = Self::published_slot_mut(p, scope, seq) {
                    set.retain(|x| x.id != id);
                }
            }
            Command::ReorderPublished { seq, order } => {
                if let Some(set) = Self::published_slot_mut(p, scope, seq) {
                    let mut out = Vec::with_capacity(set.len());
                    for id in &order {
                        if let Some(x) = set.iter().find(|x| x.id == *id) {
                            out.push(x.clone());
                        }
                    }
                    *set = out;
                }
            }
            Command::SetOverride { id, ov } => {
                let c = Self::clip_mut_on(p, id);
                c.overrides.retain(|x| x.pub_id != ov.pub_id);
                c.overrides.push(*ov);
            }
            Command::ClearOverride { id, pub_id } => {
                Self::clip_mut_on(p, id).overrides.retain(|x| x.pub_id != pub_id);
            }
            Command::SetSettings { settings } => p.settings = settings,
            Command::AddMorpher(m) => {
                p.ensure_tracks(LaneKind::Morph, m.lane + 1);
                p.morphers.push(m);
            }
            Command::SetMorpher(m) => {
                p.ensure_tracks(LaneKind::Morph, m.lane + 1);
                if let Some(slot) = p.morphers.iter_mut().find(|x| x.id == m.id) {
                    *slot = m;
                }
            }
            Command::RemoveMorpher { id } => p.morphers.retain(|m| m.id != id),
            Command::BakeMorpher { id } => {
                let Some(pos) = p.morphers.iter().position(|m| m.id == id) else { return };
                // “bake == what was drawn” — so the CUT is materialized too. We count exactly
                // this morpher: the others
                // remain alive and keep cutting as before.
                let baked = {
                    let mut q = p.clone();
                    // Attachments are NOT expanded on bake (R.50): the morpher
                    // materializes its cut over the clips of THIS reel, and
                    // the attachment instance is the same kind of clip. Expanding
                    // would replace it with its content and lose the
                    // event itself.
                    q.sequences.clear();
                    q.morphers.retain(|m| m.id == id);
                    q.effective_clips()
                };
                p.morphers.remove(pos);
                // synthetic ids of the pieces → real ones; piece No. 0 keeps the id
                // of the original (the `Split` rule), the rest get max+1…
                let mut next = p.clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 1;
                // (original, new id, end of the piece) — for re-hanging
                let mut born: Vec<(ClipId, ClipId, Tick)> = Vec::new();
                let mut clips = Vec::with_capacity(baked.len());
                for mut c in baked {
                    if let Some(origin) = slice_origin(c.id) {
                        c.id = ClipId(next);
                        next += 1;
                        born.push((origin, c.id, c.start + c.duration));
                    }
                    clips.push(c);
                }
                let ends: Vec<(ClipId, Tick)> =
                    p.clips.iter().map(|c| (c.id, c.start + c.duration)).collect();
                p.clips = clips;
                if born.is_empty() {
                    return;
                }
                // a transition where the cut clip was OUTGOING moves to the
                // piece that got the joint (the former END of the body)
                for t in &mut p.transitions {
                    let Some(a) = t.a else { continue };
                    let Some(&(_, end)) = ends.iter().find(|(i, _)| *i == a) else { continue };
                    if let Some(&(_, new, _)) = born.iter().find(|(o, _, e)| *o == a && *e == end) {
                        t.a = Some(new);
                    }
                }
                // the pieces inherit the targeting of the OTHER morphers (D5 v1)
                for m in &mut p.morphers {
                    if let Some(ids) = m.targets.clip_ids_mut() {
                        for (origin, new, _) in &born {
                            if ids.contains(origin) {
                                ids.push(*new);
                            }
                        }
                    }
                }
            }
            Command::AddArea(a) => p.areas.push(a),
            Command::RemoveArea { name } => {
                p.areas.retain(|a| a.name != name);
                // a reference to a deleted region stays in the file and
                // resolves to nothing: it cannot be silently dropped —
                // the user will restore the region under the same name (rule 20)
            }
            Command::RenameArea { from, to } => {
                if let Some(a) = p.areas.iter_mut().find(|a| a.name == from) {
                    a.name = to.clone();
                }
                for m in &mut p.morphers {
                    if let MorphTargets::Areas { names } = &mut m.targets {
                        for n in names.iter_mut().filter(|n| **n == from) {
                            *n = to.clone();
                        }
                    }
                }
            }
            Command::SetAreaRange { name, start, duration } => {
                if let Some(a) = p.areas.iter_mut().find(|a| a.name == name) {
                    a.start = Tick(start.0.max(0));
                    a.duration = duration;
                }
            }
            Command::SetAreaLanes { name, lanes } => {
                if let Some(a) = p.areas.iter_mut().find(|a| a.name == name) {
                    a.lanes = lanes;
                }
            }
            Command::AddCamera(c) => {
                // the row MUST exist, otherwise the camera would land on
                // an invisible track: it can be created, it cannot be controlled.
                // The same rule as for `MoveClip` of video and audio.
                p.ensure_tracks(LaneKind::Camera, c.lane + 1);
                p.cameras.push(c);
            }
            Command::SetCamera(c) => {
                if let Some(x) = p.cameras.iter_mut().find(|x| x.id == c.id) {
                    *x = c;
                }
            }
            Command::RemoveCamera { id } => p.cameras.retain(|c| c.id != id),
        }
    }

    /// The fate of transitions after edits of clip geometry (R.24, item 135):
    /// the clip has vanished or the joint is broken (end of `a` != start of `b`, track/view
    /// diverged) — the transition dies; a trim shortened the neighbor — the offsets are clamped
    /// by the bodies, and a window that collapsed below the minimum dies. A group move
    /// of both neighbors keeps the joint — the transition lives (identity by ClipId,
    /// the position is not stored).
    /// Transitions are checked against the content of EACH reel (R.50): a transition lives
    /// on the reel where its ends lie, and checking it against foreign clips
    /// would mean killing it on the very first command.
    fn reconcile_transitions(p: &mut Project) {
        let Project { clips, transitions, sequences, .. } = p;
        reconcile_reel(clips, transitions);
        for s in sequences {
            reconcile_reel(&s.clips, &mut s.transitions);
        }
    }

    /// The fate of publications and overrides after a batch of commands (R.59,
    /// stage 3) — the pattern of `reconcile_transitions`.
    ///
    /// `root_is` is whose reel stands NOW in the place of the root (the entry inside
    /// is done by exchanging the content). Without this address it is impossible to tell whose
    /// overrides became orphaned when the handle was removed from inside the reel.
    ///
    /// A target that changed its TYPE counts as vanished: a slot whose effect kind
    /// was substituted is a different slot, and a handle on it does
    /// nothing anymore.
    fn reconcile_published(p: &mut Project, root_is: Option<MediaId>) {
        // Fast path, first line: a project without publications and without
        // overrides — the vast majority — pays one walk
        // and not a single allocation.
        let none = p.published.is_empty()
            && p.sequences.iter().all(|s| s.published.is_empty())
            && p.clips
                .iter()
                .chain(p.sequences.iter().flat_map(|s| s.clips.iter()))
                .all(|c| c.overrides.is_empty());
        if none {
            return;
        }
        // UP TO A FIXED POINT. Removing a handle of an inner reel kills
        // the handle of the outer one that referred to it (`Nested`) — and that one
        // could be the target of yet another one a floor higher. A single pass would leave
        // a dead reference until the end of the batch, and `validate` would reject the WHOLE
        // gesture: the edit would be legal, but would not go through.
        for _ in 0..=max_publish_cascade() {
            if !Self::reconcile_published_once(p, root_is) {
                break;
            }
        }
    }

    /// One pass of reconciliation. Returns `true` if something was removed —
    /// which means it must be run once more.
    fn reconcile_published_once(p: &mut Project, root_is: Option<MediaId>) -> bool {
        let root_keep: Vec<bool> = {
            let alive = |seq: Option<MediaId>, prm: &PublishedParam| {
                p.addr_type(seq, &prm.target).is_some_and(|t| t.same_family(&prm.ty))
            };
            p.published.iter().map(|x| alive(None, x)).collect()
        };
        let seq_keep: Vec<Vec<bool>> = p
            .sequences
            .iter()
            .map(|s| {
                s.published
                    .iter()
                    .map(|x| {
                        p.addr_type(Some(s.id), &x.target).is_some_and(|t| t.same_family(&x.ty))
                    })
                    .collect()
            })
            .collect();
        let removed = root_keep.iter().any(|k| !k) || seq_keep.iter().flatten().any(|k| !k);
        let mut it = root_keep.into_iter();
        p.published.retain(|_| it.next().unwrap_or(true));
        for (s, keeps) in p.sequences.iter_mut().zip(seq_keep) {
            let mut it = keeps.into_iter();
            s.published.retain(|_| it.next().unwrap_or(true));
        }
        // Live handles by the reel address. The order matters: the reel that stands
        // in place of the root holds its own set in `p.published`, while its record in
        // `sequences` is someone else’s content.
        let mut sets: std::collections::HashMap<MediaId, std::collections::HashSet<PubId>> =
            std::collections::HashMap::new();
        if let Some(rid) = root_is {
            sets.insert(rid, p.published.iter().map(|x| x.id).collect());
        }
        for s in &p.sequences {
            sets.entry(s.id).or_insert_with(|| s.published.iter().map(|x| x.id).collect());
        }
        let prune = |c: &mut ClipEvent| {
            if c.overrides.is_empty() {
                return;
            }
            match sets.get(&c.media) {
                Some(ids) => c.overrides.retain(|o| ids.contains(&o.pub_id)),
                // the event moved to a file — there is nothing left to override
                None => c.overrides.clear(),
            }
        };
        for c in &mut p.clips {
            prune(c);
        }
        for s in &mut p.sequences {
            for c in &mut s.clips {
                prune(c);
            }
        }
        removed
    }

    fn clip_mut_on(p: &mut Project, id: ClipId) -> &mut ClipEvent {
        p.clips.iter_mut().find(|c| c.id == id).unwrap()
    }
}

/// The maximum number of cascading removals of a publication: a chain
/// of `Nested` references is no longer than the tree of reels.
fn max_publish_cascade() -> usize {
    crate::MAX_NEST_DEPTH
}

fn reconcile_reel(clips: &[ClipEvent], transitions: &mut Vec<TransitionEvent>) {
    transitions.retain_mut(|t| {
        let side = |id: Option<ClipId>| match id {
            // the empty side of a fade (R.25) is legal and stays empty
            None => Some(None),
            // the clip of a side has vanished: the transition dies entirely (fades
            // are set explicitly, not left over from a deletion)
            Some(id) => clips.iter().find(|c| c.id == id).map(Some),
        };
        let (Some(mut a), Some(mut b)) = (side(t.a), side(t.b)) else { return false };
        if let (Some(ca), Some(cb)) = (a, b) {
            // canonicalization of roles (R.51, item 399): Split and moves could have changed
            // the order of the pair — “a” is always the earlier one. The directed view is
            // mirrored together with the roles; the death of the transition would be worse
            // (rule 20)
            if cb.start < ca.start {
                std::mem::swap(&mut t.a, &mut t.b);
                std::mem::swap(&mut a, &mut b);
            }
            let (ca, cb) = (a.unwrap(), b.unwrap());
            let (a_end, b_end) = (ca.start + ca.duration, cb.start + cb.duration);
            if ca.kind != cb.kind || ca.lane != cb.lane {
                return false;
            }
            if a_end != cb.start {
                // not joined: it lives only as an OVERLAPPED transition (R.51) —
                // a plain “staircase” with a window not shorter than the minimum and without
                // a third clip in it (a third one crept in by a move →
                // the transition dies, the batch does not fall — the precedent of a broken joint)
                let stag = ca.start < cb.start && cb.start < a_end && a_end < b_end;
                if !stag || a_end - cb.start < MIN_TRANSITION_DURATION {
                    return false;
                }
                let busy = clips.iter().any(|c| {
                    c.id != ca.id
                        && c.id != cb.id
                        && c.kind == ca.kind
                        && c.lane == ca.lane
                        && c.start < a_end
                        && cb.start < c.start + c.duration
                });
                // the offsets of an overlapped one are not clamped — they are dead weight and
                // come back to life when the pair is joined again
                return !busy;
            }
        }
        if let Some(a) = a {
            t.in_offset = t.in_offset.min(a.duration);
        }
        if let Some(b) = b {
            t.out_offset = t.out_offset.min(b.duration);
        }
        t.in_offset + t.out_offset >= MIN_TRANSITION_DURATION
    });
}
