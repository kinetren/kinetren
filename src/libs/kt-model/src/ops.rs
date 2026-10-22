//! Pure editing operations (no UI): clipboard, region, group trims.
//! The semantics are carried over from the old timelineeditor.cpp (copy/paste/punch-out).
//! All functions return commands for `TimelineModel::apply_batch` —
//! one undo snapshot per gesture, atomicity for free.

use crate::model::{Command, MIN_CLIP_DURATION};
use crate::project::*;
use crate::published as kt_published;
use kt_time::{Tick, TickRange};

/// The exchange block (R.24): clips + transitions whose BOTH ends fell into the block
/// and whose joint survived the crop. The clip times are rebased to the zero of the block.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClipBlock {
    pub clips: Vec<ClipEvent>,
    pub transitions: Vec<TransitionEvent>,
    /// Morphers whose targets fell into the block (R.35 item 4). Targets outside
    /// the block are discarded already at capture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morphers: Vec<MorpherEvent>,
    /// Named regions that crossed the window of the block (crop + rebase).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<MarkedArea>,
    /// Markers: range ones — by intersection with the crop, point ones — if
    /// they are inside the window.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Marker>,
    /// Cameras whose window crossed the window of the block (R.55, C3). The rule is the same
    /// as for the morpher layer in the “everything below” mode: a camera is also a row
    /// above the edit, and without it a collapsed piece loses the movement that
    /// the person set for it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<CameraEvent>,
}

impl ClipBlock {
    /// The smallest and the largest track number of this kind in the block.
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

    /// Shift ALL kinds by one delta, clamping EACH kind so that its
    /// lower number does not go below zero (R.56, item 450).
    ///
    /// The clamp is per kind, not per event: were we to clamp each event, the
    /// events of different tracks would stick together at zero — the shape of the block
    /// matters more than hitting the point. Returns the kinds whose shift was cut:
    /// the person is told about it, rather than being silently patched up (rule 16).
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

    /// Bring the block to the ROW that was clicked (R.56, item 450).
    ///
    /// The kind of the row is present in the block — we align its top event with this
    /// row, and the other kinds move by the same delta (the video+audio pair
    /// keeps its mutual position). If the kind is absent (a block clicked on the camera
    /// row without cameras) — we do not move it at all: there is nothing to guess.
    pub fn shifted_to_row(mut self, row: Option<(LaneKind, u32)>) -> (ClipBlock, Vec<LaneKind>) {
        let Some((k, lane)) = row else { return (self, Vec::new()) };
        let Some((lo, _)) = self.lane_span(k) else { return (self, Vec::new()) };
        let clamped = self.shift_lanes(lane as i64 - lo as i64);
        (self, clamped)
    }

    /// Translate the block from the WORKING VIEW into the coordinates of the band’s reel: the track numbers
    /// of the band lie with an offset `base[kind]` (R.55, item 441), and
    /// the offsets of the kinds are DIFFERENT. Foreign events are thrown out and counted.
    ///
    /// Without this the clipboard kept the numbers with three different bases, and the paste
    /// repeated them as they were: video ended up at the ceiling, audio at the bottom, and
    /// the morpher at the very bottom, where a new row was laid out for it
    /// (the owner’s claim, item 4 of R.56).
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

    /// The reverse translation: into the coordinates of the band in the working view.
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

    /// How many tracks of each kind are needed for the block to fit.
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

/// Fresh ids for the paste — the growing list of arguments of `paste_block`
/// stopped being readable already at the fourth one.
pub struct PasteIds<'a> {
    pub clip: &'a mut u64,
    pub group: &'a mut u64,
    pub transition: &'a mut u64,
    pub morph: &'a mut u64,
    pub camera: &'a mut u64,
}

/// How to name the pasted named item (R.35 item 4: “Names are unique,
/// and the user can be asked to enter a prefix, or it can be generated
/// in the field for him”; R.38 item 19 — the default became a SUFFIX). `taken` is the
/// set of names taken in the PROJECT.
#[derive(Default)]
pub struct PasteNames {
    /// The word with which a copy is marked.
    pub affix: String,
    /// Put it IN FRONT. The default is at the back (R.38 item 19, verbatim:
    /// “when copying items — the default should be a suffix, not a prefix”):
    /// this way copies sort next to the original,
    /// rather than gathering into a separate heap by the first letter.
    pub before: bool,
    pub taken: Vec<String>,
}

/// What the paste skipped. The batch is atomic, so one bad item
/// would bring down the WHOLE paste — and it would look like “copy-paste does not work”.
/// What was skipped is counted and goes into the message, rather than being “patched up”
/// (rule 20).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PasteSkipped {
    /// Morphers that kept not a single target.
    pub morphers: usize,
    /// Markers on an already occupied tick.
    pub markers: usize,
}

/// The length of the block by clips (the window in which regions and markers are caught).
fn block_len(clips: &[ClipEvent]) -> Tick {
    clips.iter().map(|c| c.start + c.duration).max().unwrap_or(Tick::ZERO)
}

/// Morphers, regions and markers that fell into the block (R.35 item 4, verbatim: “When selecting
/// everything in a row on the timeline and ctrl+c ctrl+v, I want the links to be set for
/// morphers too… and marked_area too
/// are copied when selecting and copying, and marked_region together with
/// those inside it”).
///
/// `base` is the start of the window on the timeline (for the rebase), `len` is its length.
/// A morpher moves if the block contains AT LEAST ONE of its targets; the targets outside
/// are discarded. A region and a range marker — by intersection, with the crop;
/// a point marker — if it is inside the window.
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
                // the layer and the target references are not frozen: they move if the block itself
                // of the morpher fell into the window
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
    // cameras — by INTERSECTION of the window with the window of the block, with a rebase: a camera
    // has no “targets”, it acts on what lies under it in time
    let cameras = p
        .cameras
        .iter()
        .filter(|c| kt_time::TickRange::new(c.start, c.start + c.duration).overlaps(&win))
        .map(|c| CameraEvent { ease: true, start: c.start - base, ..c.clone() })
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

/// Transitions that survived the copy: live ends in the block, the joint intact (the crops
/// of a region could have eaten an edge), the offsets are clamped by the new bodies. A fade (R.25)
/// moves together with its only clip.
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
                    // an overlapped transition (R.51) survives if both clips in the
                    // block lie in the same “staircase”; the window is checked by
                    // check_transition_full on insertion
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

/// Copy with a region: the crossing clips are cropped to the region and rebased
/// to zero (source_in shifts by the slice on the left). The block is ready for paste_block.
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
        // the crop is the same piece as in Split and punch-out (R.39): reverse,
        // the material anchor and the shift of keys are computed by `clip_slice`
        let mut copy = clip_slice(c, TickRange::new(t0, t1), c.id);
        copy.start = t0 - r.start; // rebase to the zero of the region
        clips.push(copy);
    }
    let transitions = surviving_transitions(p, &clips);
    let extras = block_extras(p, &clips, Some(r.start), r.end - r.start);
    ClipBlock { clips, transitions, ..extras }
}

/// Copy without a region: the selected clip with its whole link group,
/// rebased to the start of the earliest one.
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

/// Copy of a set of selected events (R.33): each drags its own
/// link group along, rebased to the start of the earliest one. `copy_group` is a
/// special case with one id; a separate function is needed so that a block of several
/// events keeps their mutual SHIFTS (the paste puts them in the same shape).
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

/// Paste of a block at position `at`: fresh ids, each old group → one new one,
/// transitions are retargeted to the new ids. Returns (commands, end of the block) —
/// the cursor jumps to the end for chain-paste. A paste with a collision on a track
/// now honestly fails by the hard validation of apply_batch (item 139).
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
    // names of the pasted items: a prefix + uniqueness by THE SAME function as
    // the commands — were they to diverge, the batch would fall over its own suffixes
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
        // kinship — for the envelope (item 357): the pasted copy must carry
        // the name, the foreign effects and the metadata of the original
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
    // regions and markers: the names are renamed FIRST — the morpher references
    // below look into the already finished map
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
                // a target that did not survive the copy drags the whole
                // morpher along: the batch is atomic, and a morpher without targets is meaningless
                ids.retain(|i| clip_map.contains_key(i));
                if ids.is_empty() {
                    skipped.morphers += 1;
                    continue;
                }
                for i in ids.iter_mut() {
                    *i = clip_map[i];
                }
            }
            // a reference to a COPIED name moves to the copy, to a non-copied one —
            // it stays on the original (it went nowhere)
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
    // cameras — the same rule as for morphers: a fresh address, a rebase to the
    // insertion point. A camera has no targets, so there is nothing to skip.
    for c in &block.cameras {
        let mut copy = c.clone();
        copy.id = CameraId(*next_camera);
        *next_camera += 1;
        copy.start = at + c.start;
        // the camera name is NOT made unique: it is a label, not an address —
        // there are no references to it (unlike a region and a marker, which
        // the morpher looks at by reference)
        end = end.max(copy.start + copy.duration);
        cmds.push(Command::AddCamera(copy));
    }
    (cmds, end, skipped)
}

/// Punch-out (the old splitDeleteRegionInterior): from each crossing
/// clip the interior of the region is cut out; the remainders get new ids and LOSE
/// their link group; the right remainder keeps the alignment of the source. Without ripple.
pub fn punch_out_region(p: &Project, r: Region, next_clip: &mut u64) -> Vec<Command> {
    let mut cmds = Vec::new();
    for c in &p.clips {
        if !r.intersects_clip(c) {
            continue;
        }
        cmds.push(Command::RemoveClip { id: c.id });
        let end = c.start + c.duration;
        // Both remainders go through ONE rule of the piece (R.39). Previously there was
        // its own copy of the linear formula here (a branch per remainder), and on
        // a stretched clip it drifted the same way `Split` drifted.
        // Reverse, `source_in` and the shift of keys are the business of `clip_slice`.
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

/// Del with a region: the crossing clips AND their linked partners.
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

/// Trim-to-cursor for the whole link group (as in the old editor: the partners
/// get the same slice). An empty result = the cursor is outside the clip.
pub fn trim_group_start_to(p: &Project, sel: ClipId, at: Tick) -> Vec<Command> {
    group_members(p, sel)
        .into_iter()
        .filter(|c| c.start < at && at < c.start + c.duration - MIN_CLIP_DURATION + Tick(1))
        // the trim limit is computed by the same function as the command: were
        // they to diverge — the gesture would sometimes fail, and sometimes silently not reach (R.39)
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

/// Return of the “handles” of dead crossfades (R.26, the user’s decision: “in Sony
/// Vegas, if you drag overlapping events apart, what overlapped remains without trimming”).
///
/// When a crossfade is created, the clips are cut to the joint,
/// and the offsets of the transition
/// are NUMERICALLY EQUAL to what was cut: outgoing lost `out_offset` on the right,
/// incoming lost `in_offset` on the left (this is how plan_move in the UI counts it, and
/// upgrade_legacy on import of .ktproj). Therefore the break of a transition can be restored exactly,
/// without storing extra state.
///
/// `before` is the project before the gesture (the transitions are still alive there),
/// `after` is its result (`TimelineModel::preview`). The return is clamped by the freed space,
/// so the batch cannot fall over an overlap.
///
/// Fades are skipped: nothing was cut from them (they are set by an explicit command).
pub fn restore_handles(before: &Project, after: &Project) -> Vec<Command> {
    let mut cmds = Vec::new();
    for t in &before.transitions {
        if t.is_fade() || after.transitions.iter().any(|x| x.id == t.id) {
            continue;
        }
        // an overlapped transition (R.51): nothing was cut from it when
        // overlapping — the “return of handles” would give false trims
        if t.overlap_pair(before).is_some() {
            continue;
        }
        let (Some(a_id), Some(b_id)) = (t.a, t.b) else { continue };
        let a = after.clips.iter().find(|c| c.id == a_id);
        let b = after.clips.iter().find(|c| c.id == b_id);
        let mut want_a = if a.is_some() { t.out_offset.0 } else { 0 };
        let mut want_b = if b.is_some() { t.in_offset.0 } else { 0 };
        // the space up to the nearest obstacle, taking into account ALL members of the link group:
        // a trim moves the group as a whole, and any of its members may run into something
        let room_a = a.map_or(0, |c| room_right(after, c));
        let room_b = b.map_or(0, |c| room_left(after, c));
        want_a = want_a.min(room_a);
        want_b = want_b.min(room_b);
        // the material beyond the edge is not infinite: for reversed, TrimEnd pulls source_in
        // down, and for a normal one, TrimStart does too, and check_on rejects going below
        // zero. The clamp MUST be here: otherwise a gift would bring down the whole gesture
        // (the move/delete would be rolled back together with it).
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
        // the clips of a pair could have remained neighbors (they were dragged apart a little): then
        // the common gap is divided in proportion to who lost how much
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

/// How many ticks the clip (with its whole link group) can grow to the right without touching
/// the neighbors and foreign placeholders. The right edge of the timeline does not limit it.
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

/// The same to the left; on the left the timeline is limited by zero.
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

/// The occupied intervals on the clip’s track: foreign clips (outside its link group) and
/// the placeholders of foreign constructions — the “no overlaps” invariant includes them.
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

/// Magnetization of an A/V pair (item 6): the partner of the opposite type with the nearest
/// start is pulled to the selected one and linked. None — there is no partner;
/// Some(vec![]) — already linked and aligned (no-op). The partner is unlinked beforehand —
/// otherwise MoveClip would move its WHOLE group (including the selected one itself),
/// and the pair would never converge.
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
        return Some(Vec::new()); // already magnetized
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

// ---- import of a legacy .ktproj: overlaps → transitions/split (R.24, item 143) ------

/// The report of the conversion of old overlaps. It is shown to the user as a note.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UpgradeReport {
    /// Audio pairs that became a joint with a transition (the sound is byte-identical, test 136).
    pub transitions_created: usize,
    /// Audio clips spread across tracks (3+/FX overlaps: an exact
    /// compensation 1/N does not exist — the user’s decision, item 9 of R.24,
    /// the loudness in the zone of the former 1/N becomes the sum).
    pub audio_moved: usize,
    /// Video clips demoted onto the inserted track below (the old video of the overlap did not mix —
    /// the composite is byte-identical to the tiebreak).
    pub video_moved: usize,
}

impl UpgradeReport {
    pub fn is_empty(&self) -> bool {
        *self == UpgradeReport::default()
    }
}

/// Mirrors of trim mutations (model.rs) for direct application to a clip:
/// for reversed, TrimEnd shifts source_in, TrimStart does not touch it (item 16).
fn raw_trim_end(c: &mut ClipEvent, new_end: Tick) {
    *c = clip_slice(c, TickRange::new(c.start, new_end), c.id);
}

fn raw_trim_start(c: &mut ClipEvent, new_start: Tick) {
    *c = clip_slice(c, TickRange::new(new_start, c.start + c.duration), c.id);
}

/// Conversion of a legacy project with overlaps into the Transition model (stage 9).
/// It is idempotent: on a project without overlaps the report is empty. After the call
/// the project MUST pass validate().
pub fn upgrade_legacy(p: &mut Project) -> UpgradeReport {
    let mut report = UpgradeReport::default();

    // keyframes of slowed&reverb of the old kind — into the common mechanism (R.33):
    // the legacy project certainly carries them in its own domain
    for c in &mut p.clips {
        crate::project::migrate_legacy_slowed(c);
        crate::project::migrate_legacy_cammotion(c);
    }

    // morphers of the old kind — the former semantics (a patch on the whole target,
    // outside time): they do not become a layer by themselves (R.35)
    for m in &mut p.morphers {
        crate::project::migrate_legacy_morpher(m);
    }

    // Re-layering (R.28): an old project opens AS IT WAS — overlaps
    // remain overlaps, tracks do not multiply, nothing is cut.
    // The user’s decision after his real project (1952 clips,
    // 76 overlaps) was unfolded into ~40 video tracks: “as it was, with
    // overlaps”. The conversion into transitions/split by tracks remained for
    // the mode WITHOUT re-layering.
    if p.overlap_layers {
        p.normalize();
        return report;
    }

    // 1) audio pairs: exactly two clips in the overlap zone, both without active FX —
    //    a trim to the middle of the overlap + a transition in the same window (byte-identical)
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

    // 2) the remaining audio overlaps (3+, FX, an impossible pair) — split:
    //    the early one stays, the conflicting one moves to a free track below
    //    (a new one at the end if there is a shortage)
    while let Some(mover) = find_overlap_mover(p, ClipKind::Audio) {
        let lane = free_audio_lane_below(p, mover);
        // the track counter is bumped AT ONCE: otherwise the next mover counts
        // the added track as nonexistent and flies onto it too (an endless loop)
        p.ensure_tracks(LaneKind::Audio, lane + 1);
        if let Some(c) = p.clips.iter_mut().find(|c| c.id == mover) {
            c.lane = lane;
        }
        report.audio_moved += 1;
    }

    // 3) video overlaps: demotion of the later one onto the INSERTED track right
    //    below the early one — the old tiebreak (on top is the one whose start is smaller)
    //    is kept byte for byte
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

/// A pair for a transition: B is the nearest one overlapping A, the overlap zone is covered
/// EXACTLY by the two of them, the overlap is partial, both without active FX, the joint fits.
fn find_audio_pair(p: &Project) -> Option<(ClipId, ClipId)> {
    for a in p.clips.iter().filter(|c| c.kind == ClipKind::Audio) {
        for b in p.clips.iter().filter(|c| c.kind == ClipKind::Audio) {
            if a.id == b.id || a.lane != b.lane {
                continue;
            }
            let (a1, b1) = (a.start + a.duration, b.start + b.duration);
            // a partial overlap of B onto the tail of A
            if !(b.start > a.start && b.start < a1 && b1 > a1) {
                continue;
            }
            if !a.active_fx().is_empty() || !b.active_fx().is_empty() {
                continue;
            }
            // no third one touches the overlap zone
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
            // the window and the joint fit into the bodies
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

/// The first conflicting clip of the kind `kind`: in each overlap zone the
/// earliest one (by (start, id)) stays, the next is a candidate for moving.
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
    for lane in (me.lane + 1)..p.lanes(LaneKind::Audio) {
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
    p.lanes(LaneKind::Audio)
}

// ---- FX clipboard (R.33) --------------------------------------------
// The user’s request: “Copy-paste is missing — for individual
// effects, for the fx-chain, and for automation.” The blocks live in the model,
// not in the UI: the system clipboard uses their very shape (JSON), so the copy
// survives a restart and moves between windows of the program.

/// One chain slot with everything that belongs to it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FxSlotBlock {
    pub fx: FxInstance,
    #[serde(default)]
    pub bypass: bool,
    #[serde(default)]
    pub curves: Vec<ParamCurve>,
}

/// The whole chain or its envelopes. `src_in` is the entry point of the event from
/// which it was copied: the keys lie in the ticks of the MATERIAL, and on insertion into
/// another event they must be re-hung relative to its own entry point
/// (“entry point to entry point”, as in Premiere).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FxChainBlock {
    pub slots: Vec<FxSlotBlock>,
    pub src_in: Tick,
    #[serde(default)]
    pub fx_off: bool,
}

/// The envelopes of one slot (the curves without the effect itself).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnvelopeBlock {
    /// The tag of the variant of the source effect: a paste into a foreign kind of effect
    /// is meaningless (the curves are addressed by the names of its fields).
    pub fx_tag: String,
    pub curves: Vec<ParamCurve>,
    pub src_in: Tick,
}

/// The variant tag (`kind` in the file) — by it the compatibility
/// of the envelopes on insertion is checked.
pub fn fx_tag(f: &FxInstance) -> String {
    serde_json::to_value(f)
        .ok()
        .and_then(|v| v.get("kind").and_then(|k| k.as_str().map(str::to_string)))
        .unwrap_or_default()
}

/// The shift of keys when moved to another event: the material is addressed from
/// the entry point, so the curve lands “at the same place of the piece”.
pub fn rebase_curves(curves: &[ParamCurve], from: Tick, to: Tick) -> Vec<ParamCurve> {
    let d = to.0 - from.0;
    curves
        .iter()
        .map(|c| ParamCurve {
            param: c.param.clone(),
            // the shape of the segment travels with the key: moving a curve to another
            // event has no right to change its kind
            keys: c
                .keys
                .iter()
                .map(|k| AutoKey::shaped(Tick((k.t.0 + d).max(0)), k.v, k.shape))
                .collect(),
            source: Default::default(),
        })
        .collect()
}

/// Append the chain block to the end of the receiver (R.48):
/// the ONLY truth of insertion —
/// both the lane copy-paste and the preset insertion call it.
///
/// Keys move “entry to entry”: `d = dst_src_in − block.src_in`. For an event the entry point is
/// `source_in` (ticks of the material), for a morpher it is zero (the time of the block),
/// so one formula serves both carriers and both directions of the transfer. Keys that went
/// to the left of the entry are clamped to zero — as in `rebase_curves`.
///
/// `fx_off` of the block is NOT applied on purpose: appending must not switch off
/// the receiver’s chain (the master switch is a property of the event,
/// not of the transferred piece).
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

/// Align event `b` with event `a` by the found audio offset
/// (R.49, item 375).
///
/// `offset_ticks` — “material B lags behind material A by this many ticks”.
///
/// **It does not trim and does not link.** Synchronization answers the question “where”, not
/// “how long” and not “together or not”: a magnet trims the neighbor — that is its property,
/// and a link group is a separate decision of the editor. To shift and stop is
/// exactly what was asked.
///
/// **The translation “material → timeline” — only through `clip_timeline_at`**
/// (the rule of the single home, t273): its own arithmetic on `source_in` here
/// would drift on reverse and on a trimmed start.
pub fn sync_by_audio_commands(p: &Project, a: ClipId, b: ClipId, offset_ticks: Tick) -> SyncPlan {
    let Some(ca) = p.clips.iter().find(|c| c.id == a) else { return SyncPlan::AlreadyAligned };
    let Some(cb) = p.clips.iter().find(|c| c.id == b) else { return SyncPlan::AlreadyAligned };
    let step = kt_time::FpsGrid::new(p.settings.fps).frame_step();
    // the point of material A that now stands at the start of event A
    let mat_a = clip_material_at(ca.source_in, ca.start, ca.duration, ca.reversed, step, ca.start);
    // the same moment of the real world in material B
    let mat_b = Tick(mat_a.0 + offset_ticks.0);
    // where it now is on the timeline of event B
    let now = clip_timeline_at(cb.source_in, cb.start, cb.duration, cb.reversed, step, mat_b);
    let delta = ca.start.0 - now.0;
    if delta == 0 {
        return SyncPlan::AlreadyAligned;
    }
    let want = cb.start.0 + delta;
    if want < 0 {
        // CLAMPING IS NOT ALLOWED: an event that runs into zero would end up NOT aligned,
        // and would look like a successful synchronization. A refusal is more honest.
        return SyncPlan::OutOfRange;
    }
    SyncPlan::Move(vec![Command::MoveClip { id: b, new_start: Tick(want), new_lane: cb.lane }])
}

/// How the sync planning ended.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncPlan {
    /// We move it.
    Move(Vec<Command>),
    /// Already aligned — no work to do.
    AlreadyAligned,
    /// Alignment would move the event BEFORE THE START of the project. Clamping to zero here
    /// is not allowed: it looks like a success, while the materials remain out of step.
    OutOfRange,
}

// ---- nested sequences (R.50, item 380) --------------------------------------

/// Fresh addresses for operations on reels. The same technique as `PasteIds`:
/// the growing list of arguments stops being readable at the fourth one.
pub struct NestIds<'a> {
    pub media: &'a mut u64,
    pub clip: &'a mut u64,
    pub group: &'a mut u64,
    pub transition: &'a mut u64,
    pub morph: &'a mut u64,
    pub camera: &'a mut u64,
}

/// Counters of fresh addresses, computed FROM THE PROJECT.
///
/// Introduced as a separate structure, because operations on reels hand out
/// addresses of five kinds at once, and a call site that would write them by hand
/// sooner or later would write them not for the whole tree: `ClipId` is unique across the WHOLE
/// document, and “the maximum over `p.clips`” that skips reels is a silent
/// collision of addresses.
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

/// What the folding changed, besides the edit. It is returned, rather than
/// “patched up”: the person must be told (rule 16), not left to guess.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CollapseReport {
    /// The address of the created reel.
    pub seq: Option<MediaId>,
    /// Transitions with one end left OUTSIDE: they are re-hung onto
    /// the instance (a crossfade with a folded piece remains a crossfade).
    pub rehung: usize,
    /// Morpher layers that covered the folded content from OUTSIDE. They remain outside and
    /// now act on the ASSEMBLED picture, not on each piece: the picture
    /// legitimately changes, and this must be said.
    pub outer_layers: usize,
    /// Layers that moved INSIDE the reel (and were removed from the root).
    pub moved_layers: usize,
    /// Cameras that moved inside.
    pub moved_cameras: usize,
    /// Layers by REGION/MARKER, left outside. Reference modes
    /// inside a reel are forbidden (`MorphRefInsideSequence`): a region and a marker
    /// are navigation BY PROJECT, they live in the root. Drag such a layer
    /// inside — and `validate` would reject the WHOLE batch, that is, folding
    /// the selection under it would become impossible altogether.
    pub kept_ref_layers: usize,
}

/// Fold the selection into a reel and put an instance in its place.
///
/// The body is gathered by `copy_clips` — a second rule of “what travels with the selection”
/// must not be introduced: were they to diverge, copy-paste and folding would take different things,
/// and there would be no way to explain it.
///
/// **`ClipId`s are preserved.** The same events moved into another container, rather than
/// being born again: therefore transitions, link groups and the frozen targets
/// of morphers move verbatim, and the engine’s FX state (key — `ClipId`)
/// lives on and the tails of reverbs are not cut (invariant 3).
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

    // The content of the reel: events, transitions and morphers of the block. Regions and markers
    // STAY IN THE ROOT: this is navigation through the project, the person placed them there, and
    // dragging them inside would mean “cutting off the very thing” (rule 20).
    let mut content = Sequence::new(seq_id, name);
    content.clips = block.clips.clone();
    content.transitions = block.transitions.clone();
    // A layer by REGION or MARKER does not move inside: `validate_reel` forbids such
    // inside a reel (`MorphRefInsideSequence`), and a dragged
    // layer would bring down the WHOLE batch — folding the selection under a reference layer
    // would become impossible altogether, with an English error in the status bar.
    let refs_stay = |m: &MorpherEvent| {
        matches!(m.targets, MorphTargets::Areas { .. } | MorphTargets::Marks { .. })
    };
    content.morphers = block.morphers.iter().filter(|m| !refs_stay(m)).cloned().collect();
    content.cameras = block.cameras.clone();
    // The TRACKS of the reel — those root tracks that the content needs, IN THE ORDER
    // OF THE ROWS of the root (R.55, C3; R.62 — objects). Without the order the reel’s
    // morph rows would go last, and the layer “everything below” would drift inside
    // and die there (“the morpher does not reach there”). The track properties
    // (name, lock, level) travel as a copy; the ids are assigned anew — the root
    // keeps its own.
    let need = LaneBases {
        video: lanes_needed(&content.clips, ClipKind::Video).max(1),
        audio: lanes_needed(&content.clips, ClipKind::Audio).max(1),
        morph: content.morphers.iter().map(|m| m.lane + 1).max().unwrap_or(0),
        camera: content.cameras.iter().map(|c| c.lane + 1).max().unwrap_or(0),
    };
    content.tracks = p
        .rows()
        .into_iter()
        .filter(|&(k, l)| l < need.get(k))
        .filter_map(|(k, l)| p.track_at(k, l).cloned())
        .map(|mut t| {
            t.id = TrackId(0);
            t
        })
        .collect();
    content.ensure_tracks(LaneKind::Video, need.video);
    content.ensure_tracks(LaneKind::Audio, need.audio);
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
    // What moved INSIDE is removed from the ROOT. Without this the layer and the camera would live
    // in two places at once: inside — on each piece, outside — on the
    // assembled picture. The same thing would be applied twice, and there would be no way
    // to explain it.
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

    // The instance is an ORDINARY clip, and it is a PAIR (video + audio) in one
    // link group: then “edit, then slow down as a whole” comes for free
    // through the existing `sync_warp_to_link_group`.
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
        // the topmost of the folded: the instance lands where the person
        // was looking, not on the zero track
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
            // the instance’s envelope — from the first folded event of its kind
            cmds.push(Command::NoteBirth { child: id, parent });
        }
        cmds.push(Command::AddClip(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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

    // A transition with ONE end left outside is re-hung onto
    // the instance: a crossfade with a folded piece must remain a crossfade,
    // rather than die from `reconcile_transitions` together with the vanished end.
    for t in &p.transitions {
        let inside = |id: Option<ClipId>| id.is_some_and(|i| members.contains(&i));
        let (ia, ib) = (inside(t.a), inside(t.b));
        if ia == ib {
            continue; // both inside (moved into the reel) or both outside (intact)
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

/// What the unfolding did beyond moving the events.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExpandReport {
    /// The instance carried an effect chain, and it is REMOVED: outside it
    /// applied to the assembled picture, inside there is no such place. Silently
    /// scattering it over the pieces would mean computing something else (rule 16).
    pub chain_dropped: bool,
    /// The instance carried a level (opacity or volume) — removed.
    pub level_dropped: bool,
    /// The instance was MIRRORED, and the reverse is removed (R.55, C4).
    ///
    /// Named separately, because this is the REVERSE ANALOGUE OF LAW G: “reverse of the
    /// assembled piece ≠ reverse of each piece”. Reversing the order of
    /// the pieces and setting `reversed` on each is equivalent only where
    /// there are no overlaps, no transitions and no streaming chains inside — that is,
    /// in the special case. Silently scattering the reverse over the pieces would mean
    /// computing something else.
    pub reverse_dropped: bool,
    /// The instance played at a SPEED, and it is removed — for the same reason:
    /// the ASSEMBLED mix with stitched tails is resampled, not each
    /// piece with its own cut-off one.
    pub warp_dropped: bool,
    /// How many tracks had to be inserted under the content.
    pub lanes_inserted: usize,
    /// How many overrides were BAKED into the unfolded content (R.59,
    /// stage 3). “What you saw is what you got”: outside, the instance was drawn
    /// with its own values, and the unfolded pieces must be drawn
    /// the same way. There is no reference to the reel after unfolding, so the handle has
    /// nowhere to live — that is why the number is named aloud.
    pub overrides_baked: usize,
}

/// Unfold the instance back onto the tracks.
///
/// The content is cut by the window of the instance by ONE rule of the piece
/// (`clip_slice`) and moved to fresh ids via `paste_block`: the body of the reel
/// stays in place — the same reel may still stand in five other places.
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
        overrides_baked: inst
            .overrides
            .iter()
            .filter(|o| seq.published.iter().any(|x| x.id == o.pub_id))
            .count(),
    };
    // OVERRIDES ARE BAKED IN (R.59, stage 3): the expanded band
    // shows the TEMPLATE (it is a reference, not a copy), and unfolding is
    // an irreversible action after which there is no reel under these pieces.
    // So the values must move into the pieces themselves.
    let mut patched;
    let seq = if report.overrides_baked == 0 {
        seq
    } else {
        patched = seq.clone();
        let published = patched.published.clone();
        let frame_step = 90_000 / p.settings.fps.max(1) as i64;
        let (clips, transitions, cameras) =
            (&mut patched.clips, &mut patched.transitions, &mut patched.cameras);
        kt_published::apply_overrides(
            &published,
            &inst.overrides,
            clips,
            transitions,
            cameras,
            frame_step,
            &|m| p.default_gen_for(m),
        );
        &patched
    };
    // the window of the instance in the coordinates of the content
    let win = TickRange::new(inst.source_in, inst.source_in + inst.duration);
    let mut block = ClipBlock::default();
    for c in &seq.clips {
        let body = TickRange::new(c.start, c.start + c.duration);
        let cut = body.intersect(&win);
        if cut.is_empty() || cut.len() < MIN_CLIP_DURATION {
            continue;
        }
        // the ids here are temporary: `paste_block` below will issue fresh ones and retarget
        // the transitions to them. It is important that the material entry point of the piece
        // is computed by the ONLY rule of the project.
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

    // Tracks are INSERTED under the content, rather than taking over foreign ones: otherwise
    // unfolding silently changes the z-order of the neighbors — “it grew on its own”.
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
            cmds.push(Command::InsertTrack { kind: lane_key_of(k), at: at + 1 });
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

/// Detach the instance from the other places: the content is copied into ITS OWN reel.
///
/// It is needed exactly when “an edit inside changes all places” ceases to be
/// what the person wants. It is not named “Unlink” — next to it there is already the break of the
/// A/V link, and one word for two different actions would be a lie (rule 18).
pub fn make_independent(p: &Project, id: ClipId, name: &str, n: NestIds<'_>) -> Vec<Command> {
    let Some(inst) = p.clips.iter().find(|c| c.id == id) else { return Vec::new() };
    let Some(seq) = p.sequence(inst.media) else { return Vec::new() };
    let new_id = MediaId(*n.media);
    *n.media += 1;
    let mut copy = seq.clone();
    copy.id = new_id;
    copy.name = name.to_string();
    // the ids of the content MUST be FRESH: `ClipId` is unique across the whole
    // document, otherwise the `Retained` maps in kt-otio would silently exchange material between
    // reels.
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
    // the link groups inside the reel remain as they are: they are addressed only
    // within their own reel and do not collide with foreign ones
    vec![
        Command::AddSequence { id: new_id, name: name.to_string() },
        Command::SetSequenceContent { id: new_id, content: Box::new(copy) },
        Command::SetClipMedia { id, media: new_id },
    ]
}

/// Reels that nobody looks at any more. Returns the commands, rather than
/// cleaning up itself: deletion is a history step, it must be possible to undo it.
pub fn remove_unused_sequences(p: &Project) -> Vec<Command> {
    p.sequences
        .iter()
        .filter(|s| p.sequence_uses(s.id) == 0)
        .map(|s| Command::RemoveSequence { id: s.id })
        .collect()
}

/// What the insertion of a foreign project changed beyond the appearance of the reel (R.55, C6).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportReport {
    /// The created root reel of the foreign project.
    pub seq: Option<MediaId>,
    /// How many of its nested reels came along with it.
    pub inner_seqs: usize,
    /// How many media were created anew.
    pub media_added: usize,
    /// How many media were RECOGNIZED among one’s own on the way and reused: the same
    /// file is not obliged to arrive as a second instance.
    pub media_reused: usize,
    /// What did NOT arrive, and why — the regions, markers and settings of the foreign
    /// project. They belong to the DOCUMENT, not to the edit, and putting them into
    /// one’s own project would mean replacing the person’s own ones (rule 20).
    pub dropped_extras: usize,
}

/// Insert a FOREIGN PROJECT as a composition (R.55, C6).
///
/// The owner’s claim, verbatim: “I drag a project file onto the timeline and
/// a new timeline opens, while I want it to give me a choice — open
/// as a project or insert as a composition”.
///
/// The root of the foreign project becomes a REEL, its nested reels arrive
/// along with it, and an instance stands at `at`. All addresses are renumbered:
/// `ClipId` is unique across the WHOLE document (R.50, t425), and a collision
/// of addresses would silently exchange material between reels through the `Retained` maps.
///
/// **Media are deduplicated by path.** The same file, already standing in the project,
/// need not arrive as a second instance: it would have its own worker, its own
/// cache row and its own thumbnails — that is, a double price for the same thing.
///
/// The regions, markers and settings of the foreign project do NOT come along: they are navigation and
/// properties of the DOCUMENT, not of the edit. The same rule as for folding
/// (`collapse_into_sequence`): “dragging them inside would mean cutting off the very thing”.
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
    // ---- media: our own are recognized by path, foreign ones are created -------------------
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
    // ---- reels: addresses first, so that the attachments find each other --------
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
    // the address of the media of the event: either a file or a nested reel
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
            // REFERENCE layers are not let inside the reel by `validate_reel`
            // (the region and the marker live in the root), and we do not carry regions at all
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
        // tracks — in the order of the source rows, the properties by copy, the ids anew
        let need = LaneBases {
            video: lanes_needed(&out.clips, ClipKind::Video).max(1),
            audio: lanes_needed(&out.clips, ClipKind::Audio).max(1),
            morph: out.morphers.iter().map(|m| m.lane + 1).max().unwrap_or(0),
            camera: out.cameras.iter().map(|c| c.lane + 1).max().unwrap_or(0),
        };
        out.tracks = from
            .rows()
            .into_iter()
            .filter(|&(k, l)| l < need.get(k))
            .filter_map(|(k, l)| {
                from.tracks().get(track_index(from.tracks(), k, l)?).cloned()
            })
            .map(|mut t| {
                t.id = TrackId(0);
                t
            })
            .collect();
        out.ensure_tracks(LaneKind::Video, need.video);
        out.ensure_tracks(LaneKind::Audio, need.audio);
        Some(out)
    };

    let mut n = n;
    for s in &src.sequences {
        let Some(mut body) = convert(s, &mut n) else { continue };
        let id = seqs[&s.id];
        body.id = id;
        body.name = s.name.clone();
        body.overlap_layers = s.overlap_layers;
        cmds.push(Command::AddSequence { id, name: body.name.clone() });
        cmds.push(Command::SetSequenceContent { id, content: Box::new(body) });
    }
    let Some(mut root) = convert(src, &mut n) else { return (Vec::new(), ImportReport::default()) };
    root.id = root_id;
    root.name = name.to_string();
    root.overlap_layers = src.overlap_layers;
    let len = block_len(&root.clips);
    cmds.push(Command::AddSequence { id: root_id, name: name.to_string() });
    cmds.push(Command::SetSequenceContent { id: root_id, content: Box::new(root) });
    rep.dropped_extras = src.areas.len() + src.markers.len();

    // the instance — an ORDINARY clip, a video+audio pair in one link group, as in
    // folding: “insert, then slow down as a whole” comes for free
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
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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

// ---- matching of fields on insertion of an envelope (R.61) -----------------------

/// The outcome of matching the buffer’s curves to the parameters of the TARGET.
///
/// The owner: “when I copy a curve from one effect, I want to paste it
/// into another, let the remapping at least suggest the fields”. Before R.61 the paste button
/// simply went out on a foreign kind of effect: the remapping was not absent, it was
/// FORBIDDEN.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnvelopeFit {
    /// Placed silently: the name AND the type matched.
    pub matched: Vec<ParamCurve>,
    /// Matched by nothing — their fate is decided by the person in the matching window.
    /// An empty remainder means the window must NOT be shown.
    pub leftover: Vec<ParamCurve>,
    /// The free parameters of the target onto which the remainder can be placed.
    pub free: Vec<String>,
}

/// Distribute the buffer’s curves over the parameters of the TARGET.
///
/// The rule is one: name AND type matched — we place it silently; not matched — into the remainder,
/// and NOT ONE curve is silently lost. A match by name alone, without the type,
/// does not go into the silent branch: `radius` of Blur is an integer number of pixels, while
/// `radius` of another kind could turn out to be a fraction, and “quietly placed” would give
/// an animation of the wrong scale.
pub fn fit_envelope<P: HasParams + ?Sized>(curves: &[ParamCurve], target: &P) -> EnvelopeFit {
    let specs = target.param_specs();
    let mut out = EnvelopeFit::default();
    let mut taken: Vec<String> = Vec::new();
    for c in curves {
        match specs.iter().find(|s| *s.name == c.param) {
            Some(_) => {
                taken.push(c.param.clone());
                out.matched.push(c.clone());
            }
            None => out.leftover.push(c.clone()),
        }
    }
    out.free = specs
        .iter()
        .filter(|s| !taken.iter().any(|t| t == &*s.name))
        .map(|s| s.name.to_string())
        .collect();
    out
}

/// Rename the curve to the target parameter chosen by the person.
///
/// The segment shape and the keys travel AS THEY ARE: the matching changes the address, not
/// the content of the animation.
pub fn remap_curve(mut c: ParamCurve, to: &str) -> ParamCurve {
    c.param = to.to_string();
    c
}
