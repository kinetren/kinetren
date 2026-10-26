//! Timeline: drawing and interactions. Parity with the old Qt timeline
//! (see the UI parity plan): markers, region,
//! volume rail, stereo waveforms, clip color from the path hash, offline tint,
//! edge-autoscroll, context menu, snapping to markers/region/clips.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use kt_cache::BurstKind;
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, Tick};

// The ruler = cache strips + ONE working row (item 6 round 11 + item 3 round 12).
// At first a separate 13 px band was set aside for the markers — and it stood empty
// in any project without markers. Now the marker name is written on the same row
// as the time labels (amber, over the digits): we waste no space, and nothing
// crowds the cache strips.
pub const STRIPES_H: f32 = 11.0; // 4+1+4+2: global cache strips
pub const TIME_H: f32 = 22.0; // ticks, time digits, flags and marker names
pub const TIME_TOP: f32 = STRIPES_H;
pub const RULER_H: f32 = STRIPES_H + TIME_H;
/// Track badge (R.62): hit zone, not a drawing. 14 px — the minimum of rule 1
/// (“hit it with the mouse the first time”); the drawing inside is smaller.
pub const BADGE_W: f32 = 14.0;

/// Header strip. The width is FIXED by the neighboring badges (M / S / lock /
/// FX) and does not change with state — rule 3 (“nothing jumps”): before
/// R.62 there were 34 px here for one label.
pub const GUTTER_W: f32 = 4.0 * BADGE_W + 6.0;

/// Which badges the row of this kind carries: for service rows (morph, camera) only the
/// lock of the toggles — there is nothing to mute or solo there, and a
/// dead control is forbidden by rule 9.
pub fn row_badges(kind: LaneKind) -> &'static [TrackBadge] {
    if kind.has_switches() {
        &[TrackBadge::Mute, TrackBadge::Solo, TrackBadge::Lock, TrackBadge::Fx]
    } else {
        &[TrackBadge::Lock]
    }
}

/// Badge in the track header (R.62).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackBadge {
    Mute,
    Solo,
    Lock,
    Fx,
}

impl TrackBadge {
    pub fn glyph(self) -> &'static str {
        match self {
            TrackBadge::Mute => "M",
            TrackBadge::Solo => "S",
            TrackBadge::Lock => crate::icons::LOCK,
            TrackBadge::Fx => crate::icons::FX,
        }
    }

    /// Whether the badge is lit on this track.
    pub fn on(self, t: &kt_model::Track) -> bool {
        match self {
            TrackBadge::Mute => t.mute,
            TrackBadge::Solo => t.solo,
            TrackBadge::Lock => t.lock,
            TrackBadge::Fx => !t.fx.is_empty() && !t.fx_off,
        }
    }

    /// Rectangles of the badge row in the row header: the row stands UNDER the
    /// name, in the already occupied place (rule 13), pressed to the bottom of the row.
    pub fn row_rects(kind: LaneKind, gutter_left: f32, row_top: f32, row_h: f32) -> Vec<(TrackBadge, egui::Rect)> {
        let badges = row_badges(kind);
        let h = BADGE_W.min(row_h - 2.0).max(8.0);
        let y = row_top + row_h - h - 1.0;
        badges
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let x = gutter_left + 2.0 + i as f32 * BADGE_W;
                (*b, egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(BADGE_W, h)))
            })
            .collect()
    }
}
/// Limit of the label of a long track name (item 10 round 11): beyond it — an
/// ellipsis; the full name is shown by a hint.
pub const LANE_LABEL_MAX_W: f32 = 160.0;
/// Height of the row taken by the CLIP's name on its label. The track label
/// is written one row BELOW (item 2 round 13): one may overlap onto the track, but letters
/// over letters are mush, which the user would not accept.
pub const CLIP_NAME_ROW_H: f32 = 19.0;

/// Y of the track label: a row below the clip name, but not below the track's bottom.
/// A pure function for the sake of test 102 — “below the clip name” is checked by a number,
/// not by eye.
/// The chord of the morpher menu's “Delete morpher” (menu-chords-out-of-text, t871).
/// The item deletes THIS morpher; Del deletes every selected morpher and camera, and a
/// right click on a member of a multi-selection keeps the selection. So the chord is
/// promised only when the morpher is the whole selection — the same rule as the event
/// menu's Delete (`del_accel`: a chord only where the key really does this).
pub fn morpher_del_accel(
    sel_morph: &std::collections::BTreeSet<MorphId>,
    sel_camera: &std::collections::BTreeSet<CameraId>,
    mid: MorphId,
) -> Option<&'static str> {
    (sel_camera.is_empty() && sel_morph.len() == 1 && sel_morph.contains(&mid)).then_some("Del")
}

pub fn lane_label_y(row_top: f32, lane_h: f32, galley_h: f32) -> f32 {
    let row_bottom = row_top + lane_h - 2.0;
    (row_top + CLIP_NAME_ROW_H).min(row_bottom - galley_h).max(row_top)
}
/// Unified zoom limits (item 17 round 6): 2e6 ticks/px = 22 s/px — a 30-minute
/// project entirely in a window of ~80 px... across the full width ≈ 1.5 px/s and finer.
pub const TPP_MIN: f64 = 30.0;
pub const TPP_MAX: f64 = 2_000_000.0;
/// Ruler step ladder, ticks: 1s/10s/1m/10m/1h.
/// Ruler step ladder, ticks. Since R.40 it starts at a HUNDREDTH OF A SECOND.
///
/// Owner's claim verbatim: “on the automation timeline, if the event/morpher
/// is small, the time on the ruler is marked only at 0:00, the very first one,
/// which is stupid… the ruler should show the next second and the ticks too
/// preferably (and there are no ticks on the main timeline either).”
///
/// Both problems have one cause: the ladder started at a SECOND. The label
/// is chosen by a 70 px threshold, the tick by 7 px, so at 1 s ≥ 70 px (and
/// the default zoom is 100 px/s) `minor` and `label` coincided, and minor
/// ticks were NOT A SINGLE ONE in either place. On the short axis of an event, exactly
/// one label — “0:00” — landed in the frame.
///
/// The fractions are chosen exact in ticks (90 000 is divisible by all of them): 0.01, 0.05, 0.1,
/// 0.2, 0.5 s. Then the former 1 s, 10 s, minute, 10 minutes, hour.
const RULER_LADDER: [i64; 10] =
    [900, 4_500, 9_000, 18_000, 45_000, 90_000, 900_000, 5_400_000, 54_000_000, 324_000_000];
pub const EDGE_GRAB_PX: f32 = 7.0;
pub const SNAP_PX: f64 = 10.0;
pub const REGION_EDGE_PX: f32 = 8.0;
/// Grab strip of the named area's outline (R.38 item 1). Wider than the drawing
/// (a 1 px line), but narrower than the region: the area's outline crosses tracks, and a
/// wide strip would steal clicks from events.
pub const AREA_GRAB_PX: f32 = 5.0;
/// Grab width of the fade handle at the event's top corner (R.28, item 12 — the Vegas
/// gesture: “fades can be made at the edges of events without yet more events —
/// just by dragging the top corner”).
pub const FADE_HANDLE_PX: f32 = 12.0;
/// Half-height of the level line's grab (video opacity).
pub const LEVEL_GRAB_PX: f32 = 5.0;
/// ±6.02 dB over the full travel of the level line for audio: gain = 2^((0.5−t)·2) (port
/// of the v1 rail — the range is kept, only the element's place changed).
pub const RAIL_DB_SPAN: f32 = 12.0412;

#[derive(Clone, Copy, PartialEq)]
pub enum DragMode {
    Move,
    TrimStart,
    TrimEnd,
}

/// Result of a Move drag (R.24, item 139). A pure function — test 139.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum MovePlan {
    /// No collision — an ordinary MoveClip.
    Free,
    /// Multi-overlap/absorption/overlap less than the minimum window: a stop at the joint
    /// (the user's decision, item 10 of R.24 — we do not silently destroy someone else's edit).
    /// `blocker` — the ticks of WHAT was hit: highlighted on the timeline
    /// and named in the hint (item 5 of R.28 — “I haven't scrolled there yet —
    /// didn't understand that there is still an event at the end in the way”).
    Clamped { start: i64, blocker: (i64, i64) },
    /// Partial overlap with ONE neighbor: the crossfade gesture — clips butt against
    /// `joint` (the middle of the overlap with the body clamps), the transition window = the overlap.
    Crossfade { outgoing: ClipId, incoming: ClipId, joint: i64, in_off: i64, out_off: i64 },
}

/// Height of the event's top strip where Vegas gestures live (fade handles and the
/// level line). Below it everything is as before: body, trims, transition windows.
pub(crate) fn top_strip_h(lane_h: f32) -> f32 {
    (lane_h * 0.35).min(12.0)
}

/// Length of the “corner” fade at the clip edge in ticks: such a fade has a ZERO external
/// offset — it lies entirely inside the event, as in Vegas. A transition with a live
/// neighbor (crossfade) is not counted as a corner: it has its own handles.
/// A pure function — test 173.
pub(crate) fn corner_fade_len(p: &Project, id: ClipId, left: bool) -> Option<i64> {
    p.transitions.iter().find_map(|t| {
        if !t.is_fade() {
            return None;
        }
        match (left, t.a, t.b) {
            // fade at the start: void on the left, live end — ours
            (true, None, Some(b)) if b == id && t.in_offset.0 == 0 => Some(t.out_offset.0),
            // fade at the end: void on the right
            (false, Some(a), None) if a == id && t.out_offset.0 == 0 => Some(t.in_offset.0),
            _ => None,
        }
    })
}

/// Position of the fade handle (tick): at the event's edge when there is no fade, and at
/// the inner end of the fade when there is one. A pure function — test 173.
pub(crate) fn fade_handle_tick(start: i64, end: i64, len: i64, left: bool) -> i64 {
    if left { start + len.max(0) } else { end - len.max(0) }
}

/// Opacity from the mouse position in the track row (top = 1.0).
/// A pure function — test 173.
pub(crate) fn level_from_y(y: f32, top: f32, lane_h: f32) -> f32 {
    let h = (lane_h - 8.0).max(1.0);
    (1.0 - (y - (top + 3.0)) / h).clamp(0.0, 1.0)
}

/// Event level 0..1 for the line: for video it is opacity, for audio —
/// loudness in the range of the v1 rail (±6.02 dB; 0 dB — the middle). One
/// element for both kinds (the requirement “fades for audio and video look alike”:
/// the vertical rail used to stand inside the fade zone).
pub(crate) fn clip_level(c: &kt_model::ClipEvent) -> f32 {
    if c.kind == ClipKind::Audio { gain_to_level(c.gain_db) } else { c.opacity }
}

pub(crate) fn gain_to_level(gain_db: f32) -> f32 {
    (0.5 + gain_db / RAIL_DB_SPAN).clamp(0.0, 1.0)
}

pub(crate) fn level_to_gain(level: f32) -> f32 {
    (level.clamp(0.0, 1.0) - 0.5) * RAIL_DB_SPAN
}

/// Inverse transform: y of the level line.
pub(crate) fn level_y(opacity: f32, top: f32, lane_h: f32) -> f32 {
    let h = (lane_h - 8.0).max(1.0);
    top + 3.0 + (1.0 - opacity.clamp(0.0, 1.0)) * h
}

/// Obstacle of the current gesture (item 5 of R.28): what is in the way and where it lies.
/// It lives for one frame — set by the drag handler, cleared at the start
/// of the next pass.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Blocked {
    /// Obstacle ticks [start, end).
    pub span: (i64, i64),
    pub kind: ClipKind,
    pub lane: u32,
}

/// Whether the obstacle is visible in the current window and, if not, where to look.
/// A pure function — test 172.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum BlockView {
    Visible,
    /// Obstacle to the left of the view; ticks up to its RIGHT edge.
    Left(i64),
    /// Obstacle to the right of the view; ticks up to its LEFT edge.
    Right(i64),
}

pub(crate) fn block_view(span: (i64, i64), view: (i64, i64)) -> BlockView {
    if span.1 <= view.0 {
        BlockView::Left(view.0 - span.1)
    } else if span.0 >= view.1 {
        BlockView::Right(span.0 - view.1)
    } else {
        BlockView::Visible
    }
}

/// Shift of the material inside the piece for the gesture PREVIEW (waveform and filmstrip).
/// It is given ONLY by the left-edge trim: there the piece stays in place, and the edge
/// moves over the material. When the clip is moved, it moves as a whole — the material must travel
/// with it (item 2 of R.28: “I drag an event deeper into the previous one — the sound preview
/// scrolls to the left, although during playback the sound itself is not
/// changed”). The right-edge trim does not move the left boundary. Test 169.
pub(crate) fn preview_src_shift(drag: Option<&Drag>, id: ClipId, clip_start: i64) -> i64 {
    match drag {
        Some(Drag::Clip(d)) if d.id == id && d.mode == DragMode::TrimStart => {
            d.cur_start - clip_start
        }
        _ => 0,
    }
}

pub(crate) fn plan_move(p: &Project, id: ClipId, want: i64, lane: u32) -> MovePlan {
    let Some(me) = p.clips.iter().find(|c| c.id == id) else { return MovePlan::Free };
    let dur = me.duration.0;
    let (w0, w1) = (want, want + dur);
    let group = me.link_group;
    let hits: Vec<&ClipEvent> = p
        .clips
        .iter()
        .filter(|c| {
            c.id != id
                && c.kind == me.kind
                && c.lane == lane
                && !(group.is_some() && c.link_group == group)
                && c.start.0 < w1
                && c.start.0 + c.duration.0 > w0
        })
        .collect();
    let foreign: Vec<(i64, i64)> = p
        .foreign
        .iter()
        .filter(|f| f.kind == me.kind && f.lane == lane)
        .map(|f| (f.start.0, (f.start + f.duration).0))
        .filter(|(s, e)| *s < w1 && *e > w0)
        .collect();
    if hits.is_empty() && foreign.is_empty() {
        return MovePlan::Free;
    }
    // Overlapping (R.28): clips may be laid on top of each other — the gesture
    // CUTS NOTHING and “corrects” nothing. The user's requirement: “what there was must
    // remain as much as there was… NOTHING ANYWHERE SHOULD GROW BACK/
    // BE CUT OFF”. A crossfade is heard from the overlap itself (v1 weights), so
    // neither trims nor a TransitionEvent are needed here. Foreign placeholders
    // still may not be laid over — their raw material is written back to OTIO, where a
    // track is strictly sequential.
    if p.overlap_layers && foreign.is_empty() {
        return MovePlan::Free;
    }
    // crossfade — only a single partial overlap on ITS OWN track
    // (a vertical move with a collision does not cut other clips with trims).
    if hits.len() == 1 && foreign.is_empty() && lane == me.lane {
        let n = hits[0];
        let (n0, n1) = (n.start.0, n.start.0 + n.duration.0);
        let mid = |ov0: i64, ov1: i64, lo: i64, hi: i64| -> Option<i64> {
            (lo <= hi).then(|| (ov0 + (ov1 - ov0) / 2).clamp(lo, hi))
        };
        if w0 > n0 && w1 > n1 && n1 - w0 >= MIN_TRANSITION_DURATION.0 {
            // we are incoming: our start is in its body, overlap [w0, n1)
            if let Some(j) = mid(w0, n1, n0 + MIN_CLIP_DURATION.0, w1 - MIN_CLIP_DURATION.0) {
                return MovePlan::Crossfade {
                    outgoing: n.id,
                    incoming: id,
                    joint: j,
                    in_off: j - w0,
                    out_off: n1 - j,
                };
            }
        } else if w0 < n0 && w1 < n1 && w1 - n0 >= MIN_TRANSITION_DURATION.0 {
            // we are outgoing: our end is in its body, overlap [n0, w1)
            if let Some(j) = mid(n0, w1, w0 + MIN_CLIP_DURATION.0, n1 - MIN_CLIP_DURATION.0) {
                return MovePlan::Crossfade {
                    outgoing: id,
                    incoming: n.id,
                    joint: j,
                    in_off: j - n0,
                    out_off: w1 - j,
                };
            }
        }
    }
    // a stop: roll back to the nearest joint in the direction of motion
    let orig = me.start.0;
    let spans: Vec<(i64, i64)> = hits
        .iter()
        .map(|c| (c.start.0, c.start.0 + c.duration.0))
        .chain(foreign.iter().copied())
        .collect();
    // obstacle — the one first met ALONG the direction of motion
    let blocker = if want >= orig {
        spans.iter().copied().min_by_key(|(s, _)| *s)
    } else {
        spans.iter().copied().max_by_key(|(_, e)| *e)
    };
    let start = match blocker {
        Some((s, _)) if want >= orig => (s - dur).clamp(orig.min(want), want),
        Some((_, e)) => e.clamp(want, orig),
        None => want,
    };
    MovePlan::Clamped { start, blocker: blocker.unwrap_or((want, want + dur)) }
}

/// Range marker under the click in the ruler (R.33 item 8). First a flag/name within
/// `tol` — “by the region's name”, as the user asked;
/// otherwise the covering range with the NEAREST flag to the left:
/// when several overlap, the one whose label is visible at this spot of the strip is taken.
/// Returns the start of the marker (its identity in the model). Pure — test 207.
pub(crate) fn pick_range_marker(markers: &[Marker], click: i64, tol: i64) -> Option<i64> {
    let by_flag = markers
        .iter()
        .filter(|m| m.is_range() && (m.t.0 - click).abs() <= tol)
        .min_by_key(|m| (m.t.0 - click).abs());
    if let Some(m) = by_flag {
        return Some(m.t.0);
    }
    markers
        .iter()
        .filter(|m| m.is_range() && m.t.0 <= click && click <= m.end().0)
        .max_by_key(|m| m.t.0)
        .map(|m| m.t.0)
}

/// Plan of moving a BATCH of events (R.33 item 7): a common time shift and
/// an obstacle, if the block ran into one. `delta == want` — the path is free.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BlockPlan {
    pub delta: i64,
    pub blocker: Option<((i64, i64), ClipKind, u32)>,
}

/// Clamp of the batch move: the block moves AS A WHOLE, the mutual shifts inside it
/// do not change (requirement of item 6 of R.28 — “nothing grows back and nothing is cut off”),
/// so the stop is computed from the first contact of any participant with a foreign
/// event on its track. With overlapping (the project default) only foreign placeholders
/// get in the way — they may never be laid over.
pub(crate) fn plan_move_block(p: &Project, moving: &[ClipId], want: i64) -> BlockPlan {
    let block: Vec<&ClipEvent> = p.clips.iter().filter(|c| moving.contains(&c.id)).collect();
    if block.is_empty() {
        return BlockPlan { delta: want, blocker: None };
    }
    // by zero the block does not move
    let min_start = block.iter().map(|c| c.start.0).min().unwrap_or(0);
    let mut delta = want.max(-min_start);
    let mut blocker = None;
    for m in &block {
        let (s, e) = (m.start.0, m.start.0 + m.duration.0);
        let others = p
            .clips
            .iter()
            .filter(|c| !moving.contains(&c.id) && c.kind == m.kind && c.lane == m.lane)
            .map(|c| (c.start.0, c.start.0 + c.duration.0, false))
            .chain(
                p.foreign
                    .iter()
                    .filter(|f| f.kind == m.kind && f.lane == m.lane)
                    .map(|f| (f.start.0, (f.start + f.duration).0, true)),
            );
        for (o0, o1, foreign) in others {
            if p.overlap_layers && !foreign {
                continue; // overlaps are legal — no stop
            }
            if delta > 0 && o0 >= e {
                let cap = o0 - e;
                if cap < delta {
                    delta = cap;
                    blocker = Some(((o0, o1), m.kind, m.lane));
                }
            } else if delta < 0 && o1 <= s {
                let cap = o1 - s;
                if cap > delta {
                    delta = cap;
                    blocker = Some(((o0, o1), m.kind, m.lane));
                }
            }
        }
    }
    BlockPlan { delta, blocker }
}

/// Rectangle of the transition window in the track row. Pure function — test 140.
pub(crate) fn transition_rect(x0: f32, x1: f32, lane_top: f32, lane_h: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(x0, lane_top + 1.0),
        egui::pos2(x1, lane_top + lane_h - 3.0),
    )
}

/// “Handles” in the transition window (R.26): to the left of the joint plays the continuation
/// of the INCOMING clip (its material before its own start), to the right — the tail
/// of the outgoing one. Exactly this is returned when pulled apart. Pure function —
/// test 160.
pub(crate) fn transition_handle_rects(
    x0: f32,
    xj: f32,
    x1: f32,
    lane_top: f32,
    lane_h: f32,
) -> (egui::Rect, egui::Rect) {
    let (top, bot) = (lane_top + 1.0, lane_top + lane_h - 3.0);
    (
        egui::Rect::from_min_max(egui::pos2(x0, top), egui::pos2(xj, bot)),
        egui::Rect::from_min_max(egui::pos2(xj, top), egui::pos2(x1, bot)),
    )
}

/// Lane of the range marker on the time-label row (R.24, item 142).
/// Pure function — test 142.
pub(crate) fn marker_band_rect(x0: f32, x1: f32, time_top: f32) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(x0, time_top + 9.0), egui::pos2(x1, time_top + 13.0))
}

pub struct DragState {
    pub id: ClipId,
    pub mode: DragMode,
    /// Other selected events (R.33 item 7): id and ORIGINAL start. Empty —
    /// an ordinary single drag with all its semantics (crossfade from overlap,
    /// moving across tracks). Non-empty — the block moves only in time.
    pub extra: Vec<(ClipId, i64)>,
    /// Ticks from the start of the clip to the grab point (for Move).
    pub grab_dt: i64,
    pub orig: ClipEvent,
    /// Current (preview) position.
    pub cur_start: i64,
    pub cur_end: i64,
    pub cur_lane: u32,
    pub moved: bool,
}

/// All kinds of drags on the timeline.
pub enum Drag {
    Clip(DragState),
    /// Fade handle at the event's edge (R.28, item 12): length in ticks, live
    /// preview; on release — AddTransition/SetTransition/Remove.
    Fade {
        id: ClipId,
        left: bool,
        len: i64,
        moved: bool,
    },
    /// Video level line — opacity (R.28, item 12).
    Level {
        id: ClipId,
        moved: bool,
    },
    /// Bottom edge of the row in the gutter (R.38 item 11): the individual
    /// track height, as in Vegas. `cur` — the height in pixels.
    LaneHeight {
        row: usize,
        cur: f32,
    },
    /// Dragging the header of a track ROW (reorder; item 6 of v1.4 —
    /// rows in display order; mixing V/A/M is allowed).
    LaneRow {
        from: usize,
        cur: usize,
    },
    /// Cursor scrub (grabbing the playhead triangle in the ruler).
    Scrub,
    /// Region selection from empty space.
    Region {
        anchor: i64,
    },
    /// Event selection frame (R.33 item 7): Ctrl+drag from empty space, and in
    /// tool mode — an ordinary drag. `add` = add to the selection;
    /// otherwise the frame replaces it entirely.
    Rubber {
        anchor: egui::Pos2,
        add: bool,
    },
    /// Dragging the region boundary (true = start).
    RegionEdge {
        start_edge: bool,
    },
    /// Dragging a marker: (original tick, current).
    Marker {
        orig: i64,
        cur: i64,
    },
    /// Edge of a range marker (R.24, item 142): the right one (left=false) changes
    /// the duration, the left one moves the start with the end fixed (batch).
    MarkerEdge {
        start: i64,
        left: bool,
        cur: i64,
    },
    /// Edge of the INSTANCE window inside the expanded strip (R.55, C5).
    ///
    /// The pointer is over a REEL row, but the ROOT event is edited — this is the only
    /// gesture like that, and so it is named as a separate variant rather than
    /// folded into the trims. The commands are ordinary: `TrimStart`/`TrimEnd` on the
    /// instance.
    ///
    /// The strip's content does NOT MOVE on screen at this point: the trim moves `start` and
    /// `source_in` by the same delta (`clip_slice`), so the strip's `dt` does not
    /// change, and the window slides over still material, like a shutter.
    NestWindow {
        inst: ClipId,
        left: bool,
        cur: i64,
    },
    /// Edge of the transition window (true = left/in_offset): live through
    /// apply_coalesced(SetTransition) — one undo per gesture (R.24, item 140).
    TransitionEdge {
        id: TransitionId,
        left: bool,
        gesture: u64,
    },
    /// Roll of a transition joint: preview position, a batch of trims on release.
    TransitionBody {
        id: TransitionId,
        orig_at: i64,
        cur_at: i64,
    },
    /// Moving a morph block in time (H1; in v1 the block was immovable).
    Morph {
        id: MorphId,
        grab_dt: i64,
        cur_start: i64,
        /// The row under the cursor (R.40 item 2): the morph block travels across tracks
        /// like a clip. Until this round the gesture's `lane` did not change at all, and
        /// there was nothing to lay the layers out by rows with.
        cur_lane: u32,
        moved: bool,
    },
    /// Edge of a morph block (R.38 item 9, verbatim: “I do not understand how to change
    /// the length of an event's morpher”). The left one moves the start with the
    /// end fixed, the right one changes the duration — as for a range marker.
    MorphEdge {
        id: MorphId,
        left: bool,
        cur: i64,
    },
    /// Edge of a camera event (R.38 item 5): until this round the camera's duration
    /// was a read-only label in the window and there was nothing to change it with.
    CameraEdge {
        id: kt_model::CameraId,
        left: bool,
        cur: i64,
    },
    /// Moving a camera event (R.34 K3) — modeled on Morph.
    Camera {
        id: kt_model::CameraId,
        grab_dt: i64,
        cur_start: i64,
        /// see `Drag::Morph::cur_lane`
        cur_lane: u32,
        moved: bool,
    },
}

/// Context-menu target, captured on right-click.
#[derive(Clone, Copy, PartialEq)]
pub enum CtxTarget {
    Clip(ClipId),
    Marker(i64),
    /// Right-click at the region boundary (the boundary's tick).
    RegionEdge(i64),
    /// Right-click on a track header (item 8).
    Lane {
        kind: ClipKind,
        lane: u32,
    },
    /// Right-click on a morph block (H1).
    Morpher(MorphId),
    /// Right-click on a morph track header (H1).
    MorphLane(u32),
    /// Right-click on a camera track header (R.29).
    CameraLane(u32),
    /// Right-click on a camera event (R.34 K3).
    Camera(kt_model::CameraId),
    /// Right-click on a NESTING event (R.50). A separate target, not
    /// `Clip`: it has its own verbs (enter, expand, unlink), and
    /// “there are no dead ends” checks them on its own row.
    Nest(ClipId),
    /// Right-click on a GENERATOR event (R.53, item 416). A separate target, for the same
    /// reason as `Nest`: it has an “open” verb (the view's parameters), which an event from
    /// a file does not have, and the “no dead ends” mechanism must ask about
    /// it by ITS OWN row.
    Generator(ClipId),
    /// Right-click on the TRANSITION WINDOW (R.49, item 361). Until this round the transition
    /// was the only entity of the timeline without its own menu: everything one could do with it
    /// lay in the menu of the NEIGHBORING CLIP — that is, a user aiming at the transition got
    /// the menu of the wrong object.
    Transition(TransitionId),
    /// Right-click on a named area (R.38 item 1). An index into
    /// `project().areas`, not a name: the target must remain `Copy`.
    /// Read through `get` — the area could have been deleted from under the menu.
    Area(usize),
    Empty,
}

/// A timeline entity that the user controls with the mouse (R.38 item 253).
///
/// It exists for one purpose: to make the COMPILER ask about a new entity:
/// “what verbs control it?”. A fixed-length `ALL` array breaks
/// the BUILD when a variant is added, and `verbs()` in test 253 is an
/// exhaustive `match`, so the verbs line cannot be skipped silently.
/// The occasion was R.35: named areas were drawn, but there was nothing to delete or rename
/// them with, and all the tests were green.
// The type lives for the sake of the COMPILER (see the doc comment) and is read by the
// reachability test 253 — no one calls it in the product path, and that is the norm.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum TlEntity {
    Clip = 0,
    /// Generator event (R.53): the same `ClipEvent`, but its “material” lives in the
    /// parameters of the media, not in a file — and they are edited in the window, one
    /// for all places of this generator.
    Generator,
    /// Nesting event (R.50): the same `ClipEvent`, but with it one can do what one
    /// cannot with an ordinary one — enter inside.
    Nest,
    Marker,
    Area,
    Morpher,
    Camera,
    ClipLane,
    MorphLane,
    CameraLane,
    Region,
    Transition,
}

#[allow(dead_code)]
impl TlEntity {
    pub const N: usize = TlEntity::Transition as usize + 1;
    /// The array length is set by `N`: add a variant — and the literal stops
    /// matching the length, and the WHOLE app stops building, not only the
    /// tests.
    pub const ALL: [TlEntity; Self::N] = [
        TlEntity::Clip,
        TlEntity::Generator,
        TlEntity::Nest,
        TlEntity::Marker,
        TlEntity::Area,
        TlEntity::Morpher,
        TlEntity::Camera,
        TlEntity::ClipLane,
        TlEntity::MorphLane,
        TlEntity::CameraLane,
        TlEntity::Region,
        TlEntity::Transition,
    ];
}

#[allow(dead_code)]
impl CtxTarget {
    /// Which entity the menu target belongs to. The match is exhaustive: add a
    /// new target — you must name its entity (or explicitly say “none”).
    pub fn entity(self) -> Option<TlEntity> {
        match self {
            CtxTarget::Clip(_) => Some(TlEntity::Clip),
            CtxTarget::Generator(_) => Some(TlEntity::Generator),
            CtxTarget::Nest(_) => Some(TlEntity::Nest),
            CtxTarget::Marker(_) => Some(TlEntity::Marker),
            CtxTarget::RegionEdge(_) => Some(TlEntity::Region),
            CtxTarget::Lane { .. } => Some(TlEntity::ClipLane),
            CtxTarget::Morpher(_) => Some(TlEntity::Morpher),
            CtxTarget::MorphLane(_) => Some(TlEntity::MorphLane),
            CtxTarget::CameraLane(_) => Some(TlEntity::CameraLane),
            CtxTarget::Camera(_) => Some(TlEntity::Camera),
            CtxTarget::Area(_) => Some(TlEntity::Area),
            CtxTarget::Transition(_) => Some(TlEntity::Transition),
            CtxTarget::Empty => None,
        }
    }
}

/// Clip color from the path hash (port of colorFromPath: r=80+h%120 ...).
///
/// For a GENERATOR only the look is hashed, without parameters (R.53, item 416):
/// the parameters live in the path, and from the hash of the full path the clip was recolored on
/// every keystroke in the window — flicker under one's hands where nothing was moved.
/// The look is one — so the color is one, and it also tells the generator apart
/// from a file with the same name.
pub fn clip_color(path: &str, kind: ClipKind) -> egui::Color32 {
    let path = match kt_model::parse_gen_path(path) {
        Some(kd) => return clip_color_raw(&format!("gen:{}", kd.tag()), kind),
        None => path,
    };
    clip_color_raw(path, kind)
}

fn clip_color_raw(path: &str, kind: ClipKind) -> egui::Color32 {
    let mut h: u64 = 1469598103934665603;
    let suffix = match kind {
        ClipKind::Video => ":v",
        ClipKind::Audio => ":a",
    };
    for b in path.bytes().chain(suffix.bytes()) {
        h = h.wrapping_mul(131).wrapping_add(b as u64);
    }
    egui::Color32::from_rgb(
        (80 + h % 120) as u8,
        (80 + (h / 7) % 120) as u8,
        (80 + (h / 17) % 120) as u8,
    )
}

/// Vertical layout of tracks (item 12 round 6, test 61): compressing the PANEL
/// squeezes the row height as before (down to 30), but growing the NUMBER of tracks
/// does not flatten rows — a vertical scroll appears. An explicit v_zoom
/// (Shift+Ctrl+wheel) sets the row height directly.
pub struct VLayout {
    /// Height of the CLIP rows (video/audio). Service rows below are a third.
    pub base_h: f32,
    pub content_h: f32,
    pub scroll_max: f32,
}

/// By how many times a service row (morph/camera) is lower than a clip row —
/// the user's request, R.35 item 1: “so as not to lose space — morpher tracks
/// can be reduced in height threefold.”
pub const THIN_ROW_DIV: f32 = 3.0;
/// Divisor of the MORPH row (R.38 item 10, verbatim: “increase the height of the
/// morpher track by default by 2”). A morph block carries the effect chain and a label,
/// the camera one only a name, so the camera row stayed threefold lower.
/// Each track can be given an individual height (item 11).
pub const MORPH_ROW_DIV: f32 = 1.5;
/// Floor of the height of a service row: 84/3 = 28 is normal, and 16/3 = 5 — a row like that
/// cannot be hit with the mouse (item 1 of the UI checklist).
pub const THIN_ROW_MIN: f32 = 10.0;

/// Ceiling of an individual row height (R.38 item 11). Everything dragged by mouse has a limit
/// (rule 6 of the checklist).
pub const ROW_H_MAX: f32 = 400.0;
/// Grab strip of the row's bottom edge in the gutter. Wider than the drawing (rule 1).
pub const ROW_EDGE_PX: f32 = 5.0;

/// Row height by its kind. The ONLY place where this rule lives.
pub fn row_height(kind: LaneKind, base_h: f32) -> f32 {
    match kind {
        LaneKind::Video | LaneKind::Audio => base_h,
        LaneKind::Morph => (base_h / MORPH_ROW_DIV).max(THIN_ROW_MIN),
        LaneKind::Camera => (base_h / THIN_ROW_DIV).max(THIN_ROW_MIN),
    }
}

/// Row height taking the INDIVIDUAL height into account (R.38 item 11, verbatim:
/// “allow changing the height of each track individually”). The override is in pixels
/// and absolute: vertical zoom does not touch it, otherwise
/// “set the track's height” stopped holding at the first zoom.
pub fn row_height_of(kind: LaneKind, lane: u32, base_h: f32, over: &[(LaneKind, u32, f32)]) -> f32 {
    match over.iter().find(|(k, l, _)| *k == kind && *l == lane) {
        Some((_, _, h)) => h.clamp(THIN_ROW_MIN, ROW_H_MAX),
        None => row_height(kind, base_h),
    }
}

/// Prefix sums of row tops; length — `rows.len() + 1`, the last
/// element = the full height of the content.
pub fn row_offsets(
    rows: &[(LaneKind, u32)],
    base_h: f32,
    over: &[(LaneKind, u32, f32)],
) -> Vec<f32> {
    let mut out = Vec::with_capacity(rows.len() + 1);
    let mut y = 0.0;
    out.push(0.0);
    for (kind, lane) in rows {
        y += row_height_of(*kind, *lane, base_h, over);
        out.push(y);
    }
    out
}

/// `clip_lanes` — clip rows, `thin_lanes` — service ones (they give a
/// three times smaller contribution, otherwise a dozen morph tracks would flatten the video).
pub fn vertical_layout(
    avail_h: f32,
    clip_lanes: usize,
    morph_lanes: usize,
    camera_lanes: usize,
    v_zoom: Option<f32>,
) -> VLayout {
    let clip_lanes = clip_lanes.max(1) as f32;
    let usable = (avail_h - RULER_H - 6.0).max(0.0);
    // weight of service rows in the divisor — their share of a clip row; for morph
    // and camera rows the shares are DIFFERENT (item 10 of R.38).
    let weight =
        clip_lanes + morph_lanes as f32 / MORPH_ROW_DIV + camera_lanes as f32 / THIN_ROW_DIV;
    let base_h = match v_zoom {
        Some(z) => z.clamp(16.0, 120.0),
        None => (usable / weight).clamp(30.0, 84.0),
    };
    let content_h = clip_lanes * base_h
        + morph_lanes as f32 * (base_h / MORPH_ROW_DIV).max(THIN_ROW_MIN)
        + camera_lanes as f32 * (base_h / THIN_ROW_DIV).max(THIN_ROW_MIN);
    VLayout { base_h, content_h, scroll_max: (content_h - usable).max(0.0) }
}

/// Ruler steps for zoom (item 17 round 6, test 60): minor ticks — the minimum ladder
/// rung with >= 7 px, labels — with >= 70 px. Previously the loop went BY
/// SECONDS and at deep zoom-out produced thousands of shapes per frame.
pub fn ruler_steps(tpp: f64) -> (i64, i64) {
    let minor = RULER_LADDER
        .iter()
        .copied()
        .find(|&st| st as f64 / tpp >= 7.0)
        .unwrap_or(RULER_LADDER[RULER_LADDER.len() - 1]);
    let label = RULER_LADDER
        .iter()
        .copied()
        .find(|&st| st >= minor && st as f64 / tpp >= 70.0)
        .unwrap_or(RULER_LADDER[RULER_LADDER.len() - 1]);
    (minor, label)
}

/// Label, PINNED TO THE LEFT EDGE of the ruler (R.40 item 14).
///
/// Owner's claim verbatim: “and if it zooms in, then this time flies off to the side.”
/// The label was drawn only at a tick falling in the window, and anchored LEFT_TOP —
/// on zoom the last one slid past the left edge, and NOT A SINGLE
/// time remained on the ruler. Hence the rule: the nearest label to the left is always visible.
///
/// A pure function: both the main timeline and the automation graph call it —
/// we do not create its own formula in two places.
pub fn ruler_pinned_label(left_tick: i64, label_step: i64) -> i64 {
    left_tick.div_euclid(label_step.max(1)) * label_step.max(1)
}

/// Ruler time label with precision BY THE LADDER STEP (R.40 item 14):
/// mm:ss up to an hour, h:mm:ss beyond, plus a fraction of a second at a sub-second step. At a sub-second
/// step whole seconds would repeat across neighboring labels — such a ruler
/// cannot be read. The number of digits is taken from the step, not from the value:
/// labels of one zoom must have one form, otherwise they jump in
/// width on scroll (rule 3 of the checklist).
pub fn format_ruler_time_step(ticks: i64, step: i64) -> String {
    // left of zero (the zone before zero inside the reel, R.58) — the sign and the fields from |t|
    if ticks < 0 {
        return format!("-{}", format_ruler_time_step(-ticks, step));
    }
    let s = ticks / 90_000;
    let head = if s < 3600 {
        format!("{}:{:02}", s / 60, s % 60)
    } else {
        format!("{}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
    };
    if step >= 90_000 {
        return head;
    }
    let digits = if step >= 9_000 {
        1
    } else if step >= 900 {
        2
    } else {
        3
    };
    let frac = (ticks % 90_000).abs() as f64 / 90_000.0;
    format!("{head}{}", format!("{frac:.digits$}", digits = digits).trim_start_matches('0'))
}

/// The CURSOR, region, marker and area FLOOR — always zero (R.58, item 462,
/// the owner's decision: “I do not need the zone before zero played back”). Gestures
/// over EVENTS are clamped not by it but by `KtApp::time_floor()`: inside a reel
/// an event legitimately slides left of zero. Bare `.max(0)` over the ticks of gestures
/// does not occur in this file — the guard is t637c.
pub const CURSOR_FLOOR: i64 = 0;

/// Snap threshold, ticks (item 17 round 6, test 60): proportional to zoom WITHOUT an upper
/// clamp — the former ceiling of 9000 ticks (0.1 s) made the snap dead
/// at deep zoom-out (0.005 px at tpp=2e6).
pub fn snap_threshold(tpp: f64) -> i64 {
    ((SNAP_PX * tpp) as i64).max(1_080)
}

/// The nearest magnetic target, if it is within the threshold.
pub fn snap_point(tick: i64, targets: &[i64], threshold: i64) -> Option<i64> {
    targets
        .iter()
        .copied()
        .map(|t| (t, (t - tick).abs()))
        .filter(|(_, d)| *d <= threshold)
        .min_by_key(|(t, d)| (*d, *t))
        .map(|(t, _)| t)
}

/// Clip move: we magnetize the START or the END, the smaller offset wins.
/// Returns the new start and the target for the yellow line.
///
/// Defect of item 2 round 11: previously the candidates were compared AFTER substitution of
/// “not found = leave as is”, and such a candidate had offset 0 —
/// that is, it always won, while the line was drawn. The snap fired
/// only when both edges hit at the same time, and the user saw
/// “the line is there, no magnet.”
pub fn snap_move(raw: i64, dur: i64, targets: &[i64], threshold: i64) -> (i64, Option<i64>) {
    let by_start = snap_point(raw, targets, threshold);
    let by_end = snap_point(raw + dur, targets, threshold);
    match (by_start, by_end) {
        (Some(s), Some(e)) => {
            if (s - raw).abs() <= (e - dur - raw).abs() {
                (s, Some(s))
            } else {
                (e - dur, Some(e))
            }
        }
        (Some(s), None) => (s, Some(s)),
        (None, Some(e)) => (e - dur, Some(e)),
        (None, None) => (raw, None),
    }
}

/// Clip zones by width (item 17 round 6, test 60): for narrow clips (< 3 grabs)
/// the trim zones shrink to w/4 (min 2 px) — previously a clip of 14 px was entirely
/// made of trim zones and could not be moved.
pub fn edge_grab_for_width(w: f32) -> f32 {
    if w >= 3.0 * EDGE_GRAB_PX { EDGE_GRAB_PX } else { (w / 4.0).max(2.0) }
}

/// Grab of the CORNER FADE HANDLE by the event's width (R.40 item 12).
///
/// Owner's claim verbatim: “on a small event I try to catch the opacity there but
/// it does not work”. The handle was caught within ±12 px of the corner ALWAYS — on an event
/// of 24 px both handles covered it entirely, and not a single pixel remained in the middle, where
/// the level line lives. Plus the level zone itself was locked behind a separate condition
/// `width > 24`.
///
/// The rule is the same as for the trim zones (`edge_grab_for_width`): on a narrow event
/// the zone shrinks, the MIDDLE stays free. A third of the width per handle —
/// means the middle third belongs to the level at any width.
pub fn fade_grab_for_width(w: f32) -> f32 {
    FADE_HANDLE_PX.min(w / 3.0).max(1.0)
}

/// Return of the window to the playhead during playback (item 14 round 6, test 59): only in
/// follow mode (Play turns it on, manual pan/zoom turns it off), not during a drag;
/// the window returns when the cursor has left the zone [2%, 85%].
pub fn follow_view(
    follow: bool,
    playing: bool,
    dragging: bool,
    ph: f64,
    view_start: f64,
    vis: f64,
) -> Option<f64> {
    if !follow || !playing || dragging {
        return None;
    }
    if ph < view_start + vis * 0.02 || ph > view_start + vis * 0.85 {
        Some(ph - vis * 0.15)
    } else {
        None
    }
}

/// Automatic restoration of follow mode (item 30 round 7): a manual scroll cancels
/// follow, but if a loop/region is playing, the region is ENTIRELY visible in the window, the cursor
/// is inside it and the user has stopped scrolling (grace pause) — following
/// switches back on by itself.
pub fn follow_rearm(
    playing: bool,
    since_scroll_s: f32,
    grace_s: f32,
    ph: f64,
    region: Option<(f64, f64)>,
    view_start: f64,
    vis: f64,
) -> bool {
    let Some((rs, re)) = region.filter(|(s, e)| e > s) else {
        return false;
    };
    playing
        && since_scroll_s >= grace_s
        && rs >= view_start
        && re <= view_start + vis
        && ph >= rs
        && ph < re
}

impl KtApp {
    /// State dump for live scenarios (R.27, `KT_GEOM_DUMP=<file>`):
    /// window, track zone, zoom and vertical row positions — so the script
    /// aims the mouse exactly, not by the heuristic “roughly a third from the bottom”, plus
    /// the cursor position and the playing flag — so that scenarios about playback
    /// (loop, wrap-around) read the truth from the program, not guess from the picture. It is written no more than 10 Hz and only when the
    /// env var is set: in an ordinary launch the branch costs one `Option` check.
    fn dump_geom(&mut self, geom: &TlGeom, screen: egui::Rect, windows: usize) {
        let Some(path) = self.geom_dump_path.clone() else { return };
        let now = std::time::Instant::now();
        // THE DUMP STAYS THROTTLED (10 Hz) AND UNDER THE BENCH (R.68). An attempt to
        // write it every frame — for the sake of an exact wait for a frame — cost
        // a round: serialization and an atomic file replacement in the DRAW PATH on
        // every frame noticeably slow the UI thread, and subtle gestures stop
        // landing in their frames (the live `corner` with a 3 px tolerance stopped
        // raising a fade at all). The frame wait works with 100 ms precision —
        // that is enough for everyone who asks for it.
        if self.geom_dumped_at.is_some_and(|t| now.duration_since(t).as_millis() < 100) {
            return;
        }
        self.geom_dumped_at = Some(now);
        let rows: Vec<String> = geom
            .rows
            .iter()
            .enumerate()
            .map(|(i, (kind, lane))| {
                format!(
                    "{{\"kind\":\"{}\",\"lane\":{lane},\"top\":{:.1},\"center\":{:.1},\"h\":{:.1}}}",
                    match kind {
                        kt_model::LaneKind::Video => "video",
                        kt_model::LaneKind::Audio => "audio",
                        kt_model::LaneKind::Morph => "morph",
                        kt_model::LaneKind::Camera => "camera",
                    },
                    geom.row_top(i),
                    geom.row_center_y(i),
                    geom.row_h(i),
                )
            })
            .collect();
        // FX window parameter fields — from the program ITSELF (R.43): a DragValue
        // has no node in the accessibility tree, and a scenario must not
        // guess its offset inside the window.
        let fields: Vec<String> = crate::widgets::field_probe::take()
            .into_iter()
            .map(|(name, r)| {
                format!(
                    "{{\"param\":\"{name}\",\"x\":{:.1},\"y\":{:.1},\"w\":{:.1},\"h\":{:.1}}}",
                    r.left(),
                    r.top(),
                    r.width(),
                    r.height()
                )
            })
            .collect();
        // starvation counters — here too (R.43): otherwise a live scenario has
        // no way to tell “playing evenly” from “playing evenly, but with silence”,
        // and after item 294 the clock runs under starvation and the symptom is invisible.
        let ord = std::sync::atomic::Ordering::Relaxed;
        // events of the LIVE model: a scenario that checks the unsaved-changes guard must NOT
        // save, otherwise it would check the save, not the guard
        // (R.49, item 354).
        let clips: Vec<String> = self
            .model
            .project()
            .clips
            .iter()
            .map(|c| {
                format!("{{\"id\":{},\"start\":{},\"dur\":{}}}", c.id.0, c.start.0, c.duration.0)
            })
            .collect();
        // WHERE WE STAND (R.50, B4): without this all future scenarios would silently
        // check the wrong list of events — `clips` above gives the content of the CURRENT reel,
        // and from outside that is the root, and inside it the reel.
        let nest = format!(
            "{{\"depth\":{},\"seq\":{},\"name\":\"{}\",\"uses\":{}}}",
            self.nest_path.len(),
            self.model.editing().map(|m| m.0 as i64).unwrap_or(-1),
            self.nest_crumbs().last().cloned().unwrap_or_default().replace('"', "'"),
            self.model.editing().map(|m| self.model.document().sequence_uses(m)).unwrap_or(0),
        );
        // the status line AS DRAWN (R.62): the conform scenario checks the hint about a mismatch,
        // and reading the text pixels is not allowed
        let status_shown: String = self
            .status_line_shown
            .0
            .chars()
            .map(|c| match c {
                '\\' => '/',
                '"' => '\'',
                c if c.is_control() => ' ',
                c => c,
            })
            .collect();
        // LIVE STAND (R.68): the number of the drawn frame (the harness waits for a frame
        // by it, not by the clock), the number of the executed stand command, the path of the
        // open project and the number of open windows — proof of cleanliness before
        // the first gesture of the next scenario.
        let project_shown = self
            .project_path
            .as_ref()
            .map(|p| p.to_string_lossy().replace('\\', "/").replace('"', "'"))
            .unwrap_or_default();
        let json = format!(
            "{{\"frame\":{},\"input\":{},\"bench_done\":{},\"windows\":{},\"project\":\"{}\",\
             \"playhead\":{},\"playing\":{},\"base_rate\":{:.3},\"fields\":[{}],\"clips\":[{}],\
             \"nest\":{},\"clipboard\":{},\"status\":\"{}\",\
             \"underruns\":{},\"callback_underruns\":{},\"dirty\":{},\"morphers\":{},\
             \"notices\":[{}],\
             \"window\":{{\"w\":{:.1},\"h\":{:.1}}},\
             \"bin\":{{\"left\":{:.1},\"right\":{:.1},\"top\":{:.1},\"bottom\":{:.1}}},\
             \"timeline\":{{\"left\":{:.1},\"right\":{:.1},\"top\":{:.1},\"bottom\":{:.1}}},\
             \"lane_h\":{:.1},\"ticks_per_px\":{:.3},\"view_start\":{:.1},\"rows\":[{}]}}\n",
            crate::bench::frames(),
            crate::bench::inputs(),
            crate::bench::done(),
            windows,
            project_shown,
            self.player.video_playhead().0,
            self.player.transport.is_playing(),
            // SPEED — from the program itself (R.57): it cannot be read off the strip's pixels,
            // and a scenario must assert the value, not
            // “something has moved”.
            self.base_rate,
            fields.join(","),
            clips.join(","),
            nest,
            self.clipboard.clips.len(),
            status_shown,
            self.player.underruns.load(ord),
            self.player.shared.callback_underruns.load(ord),
            // the asterisk and the number of morphers — for the live scenario of importing an old
            // version (R.67): “there are edits” and “the morphers arrived” are otherwise not visible.
            self.dirty_content,
            self.model.project().morphers.len(),
            // the last notification AS SHOWN (the bell): the status line
            // covers it with the hover hint.
            self.notices
                .iter()
                .rev()
                .take(5)
                // a multi-line notice (telemetry's lists) must not break the hand-built
                // JSON: line breaks become " | ", as in the journal (t843)
                .map(|n| {
                    let t = n.text.replace('\\', "/").replace('"', "'");
                    format!("\"{}\"", t.replace(['\n', '\r', '\t'], " | "))
                })
                .collect::<Vec<_>>()
                .join(","),
            screen.width(),
            screen.height(),
            self.bin_rect.left(),
            self.bin_rect.right(),
            self.bin_rect.top(),
            self.bin_rect.bottom(),
            geom.rect.left(),
            geom.rect.right(),
            geom.rect.top(),
            geom.rect.bottom(),
            geom.base_h,
            geom.tpp,
            geom.view_start,
            rows.join(",")
        );
        let tmp = format!("{path}.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// Timeline geometry of the last frame — the mapping of positions OUTSIDE
/// draw_timeline (file drop item 2, arbitrary track order item 6 of v1.4).
/// The rows are display order (LaneKind, lane); pure functions — tests 47/50.
#[derive(Clone)]
pub struct TlGeom {
    /// Track drawing zone (to the right of the gutter, including the ruler at the top).
    pub rect: egui::Rect,
    /// Height of a CLIP row. The height of a SPECIFIC row — `row_h`:
    /// service ones (morph/camera) are a third lower (R.35 item 1).
    pub base_h: f32,
    /// Display order of rows top to bottom (normalized).
    pub rows: std::sync::Arc<Vec<(LaneKind, u32)>>,
    /// Row tops (prefix sums). Private: it cannot physically
    /// diverge from `rows` — both are filled by `new`.
    offs: std::sync::Arc<Vec<f32>>,
    /// Row heights taking the individual ones into account (R.38 item 11). Also private,
    /// and also from `new`: “how much a row occupies” must be consistent
    /// with its top.
    hs: std::sync::Arc<Vec<f32>>,
    /// Vertical scroll offset of the track zone (item 12 round 6).
    pub view_top: f32,
    pub view_start: f64,
    pub tpp: f64,
}

impl TlGeom {
    pub fn new(
        rect: egui::Rect,
        base_h: f32,
        rows: Vec<(LaneKind, u32)>,
        over: &[(LaneKind, u32, f32)],
        view_top: f32,
        view_start: f64,
        tpp: f64,
    ) -> TlGeom {
        let offs = row_offsets(&rows, base_h, over);
        let hs = rows.iter().map(|(k, l)| row_height_of(*k, *l, base_h, over)).collect();
        TlGeom {
            rect,
            base_h,
            rows: std::sync::Arc::new(rows),
            offs: std::sync::Arc::new(offs),
            hs: std::sync::Arc::new(hs),
            view_top,
            view_start,
            tpp,
        }
    }

    /// Row height by index.
    pub fn row_h(&self, row: usize) -> f32 {
        self.hs.get(row).copied().unwrap_or(self.base_h)
    }

    /// Height of a clip track row — ITS OWN (R.40 item 11).
    ///
    /// Until R.40 this held `self.base_h`, and all event drawing went by
    /// the base height: a row could be stretched, while the event stayed
    /// the same. The individual height (R.38 item 11) reached only
    /// the row tops and the track background.
    pub fn lane_h_of(&self, kind: ClipKind, lane: u32) -> f32 {
        let lk = match kind {
            ClipKind::Video => LaneKind::Video,
            ClipKind::Audio => LaneKind::Audio,
        };
        self.row_of((lk, lane)).map(|r| self.row_h(r)).unwrap_or(self.base_h)
    }

    /// Height of a service row (morph/camera) — also its own.
    pub fn thin_h_of(&self, kind: LaneKind, lane: u32) -> f32 {
        self.row_of((kind, lane))
            .map(|r| self.row_h(r))
            .unwrap_or_else(|| crate::timeline::row_height(kind, self.base_h))
    }

    /// Middle of the row — where the gutter labels sit.
    pub fn row_center_y(&self, row: usize) -> f32 {
        self.row_top(row) + self.row_h(row) * 0.5
    }

    /// A point inside a clip track row in fractions of its height. Tests and
    /// live scenarios must compute y EXACTLY so: a home-made formula
    /// “lane_top + lane_h*f” breaks on rows of different heights.
    /// (Drawing holds `top` and `lane_h` in local variables, so
    /// there are no calls in product code — the method exists for those who
    /// READ the geometry from outside.)
    #[allow(dead_code)]
    pub fn lane_y(&self, kind: ClipKind, lane: u32, frac: f32) -> f32 {
        let lk = match kind {
            ClipKind::Video => LaneKind::Video,
            ClipKind::Audio => LaneKind::Audio,
        };
        let h = self.row_of((lk, lane)).map(|r| self.row_h(r)).unwrap_or(self.base_h);
        self.lane_top(kind, lane) + h * frac
    }

    /// Row index under y WITHOUT a lower bound: rows are iterated
    /// top to bottom, and everything below the last one continues at its height.
    /// A clip drag needs it — there “how many rows were passed” was computed
    /// by dividing by `lane_h`, and on rows of different height this lied.
    pub fn row_at_y_unbounded(&self, y: f32) -> i64 {
        let rel = y - self.lanes_top();
        if rel < 0.0 {
            let last = self.base_h.max(1.0);
            return (rel / last).floor() as i64;
        }
        match self.offs.partition_point(|&o| o <= rel) {
            0 => 0,
            i if i <= self.rows.len() => i as i64 - 1,
            _ => {
                let over = rel - self.offs[self.rows.len()];
                self.rows.len() as i64 + (over / self.base_h.max(1.0)).floor() as i64
            }
        }
    }

    /// The row at which vertical zoom is anchored (the anchor under the cursor),
    /// in ROW FRACTIONS: the integer part is the index, the fractional part the place within it.
    pub fn anchor_at(&self, y: f32) -> f32 {
        let rel = (y - self.lanes_top()).max(0.0);
        let i = self.row_at_y_unbounded(y).max(0) as usize;
        let top = self.offs.get(i).copied().unwrap_or(rel);
        let h = self.row_h(i).max(1.0);
        i as f32 + (rel - top) / h
    }
    pub fn x_to_tick(&self, x: f32) -> i64 {
        (self.view_start + (x - self.rect.left()) as f64 * self.tpp) as i64
    }

    pub fn tick_to_x(&self, t: i64) -> f32 {
        self.rect.left() + ((t as f64 - self.view_start) / self.tpp) as f32
    }

    pub fn lanes_top(&self) -> f32 {
        self.rect.top() + RULER_H + 3.0 - self.view_top
    }

    /// Row index under y (within rows).
    pub fn row_at_y(&self, y: f32) -> Option<usize> {
        let rel = y - self.lanes_top();
        if rel < 0.0 || rel >= *self.offs.last().unwrap_or(&0.0) {
            return None;
        }
        Some(self.offs.partition_point(|&o| o <= rel).saturating_sub(1))
    }

    pub fn key_at_y(&self, y: f32) -> Option<(LaneKind, u32)> {
        self.rows.get(self.row_at_y(y)?).copied()
    }

    /// Clip TRACK under y (morph/camera row/ruler → None).
    pub fn lane_at_y(&self, y: f32) -> Option<(ClipKind, u32)> {
        match self.key_at_y(y)? {
            (LaneKind::Video, l) => Some((ClipKind::Video, l)),
            (LaneKind::Audio, l) => Some((ClipKind::Audio, l)),
            (LaneKind::Morph | LaneKind::Camera, _) => None,
        }
    }

    pub fn morph_lane_at_y(&self, y: f32) -> Option<u32> {
        match self.key_at_y(y)? {
            (LaneKind::Morph, l) => Some(l),
            _ => None,
        }
    }

    pub fn camera_lane_at_y(&self, y: f32) -> Option<u32> {
        match self.key_at_y(y)? {
            (LaneKind::Camera, l) => Some(l),
            _ => None,
        }
    }

    /// Screen rectangle of a named area. One formula for drawing AND hit-test:
    /// if they diverged, the outline would be drawn in one
    /// place and grabbed in another (the rule “geometry only via
    /// TlGeom”). `None` — the area is entirely outside the view.
    pub fn area_rect(&self, a: &kt_model::MarkedArea) -> Option<egui::Rect> {
        let x0 = self.tick_to_x(a.start.0);
        let x1 = self.tick_to_x((a.start + a.duration).0).max(x0 + 6.0);
        if x1 < self.rect.left() || x0 > self.rect.right() {
            return None;
        }
        let keys: Vec<(LaneKind, u32)> =
            if a.lanes.is_empty() { self.rows.to_vec() } else { a.lanes.clone() };
        let (mut top, mut bot) = (f32::MAX, f32::MIN);
        for k in &keys {
            if let Some(row) = self.row_of(*k) {
                top = top.min(self.row_top(row));
                bot = bot.max(self.row_top(row) + self.row_h(row));
            }
        }
        if top > bot {
            return None;
        }
        Some(egui::Rect::from_min_max(
            egui::pos2(x0.max(self.rect.left()), top + 1.0),
            egui::pos2(x1.min(self.rect.right()), bot - 1.0),
        ))
    }

    pub fn row_of(&self, key: (LaneKind, u32)) -> Option<usize> {
        self.rows.iter().position(|&k| k == key)
    }

    pub fn row_top(&self, row: usize) -> f32 {
        let off = match self.offs.get(row) {
            Some(o) => *o,
            // beyond the last row we continue with the base height
            None => {
                self.offs.last().copied().unwrap_or(0.0)
                    + (row.saturating_sub(self.rows.len())) as f32 * self.base_h
            }
        };
        self.lanes_top() + off
    }

    pub fn lane_top(&self, kind: ClipKind, lane: u32) -> f32 {
        let key = match kind {
            ClipKind::Video => (LaneKind::Video, lane),
            ClipKind::Audio => (LaneKind::Audio, lane),
        };
        self.row_top(self.row_of(key).unwrap_or(0))
    }

    pub fn morph_lane_top(&self, lane: u32) -> f32 {
        self.row_top(self.row_of((LaneKind::Morph, lane)).unwrap_or(0))
    }

    /// The track of kind `kind` nearest to row `row` (dragging a clip across
    /// foreign rows “sticks” to its own kind, item 6 of v1.4, test 50).
    pub fn nearest_lane(&self, kind: ClipKind, row: i64) -> u32 {
        self.nearest_lane_of(
            match kind {
                ClipKind::Video => LaneKind::Video,
                ClipKind::Audio => LaneKind::Audio,
            },
            row,
        )
    }

    /// The same for ANY kind of row: morph and camera blocks move between
    /// their own tracks by exactly the same rule as clips (R.40 item 2).
    pub fn nearest_lane_of(&self, want: LaneKind, row: i64) -> u32 {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, e)| e.0 == want)
            .min_by_key(|(i, _)| (*i as i64 - row).abs())
            .map(|(_, e)| e.1)
            .unwrap_or(0)
    }

    /// Row keys touched by the vertical segment [y0, y1].
    pub fn rows_in_band(&self, y0: f32, y1: f32) -> Vec<(LaneKind, u32)> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                let top = self.row_top(*i);
                top < y1 && top + self.row_h(*i) > y0
            })
            .map(|(_, k)| *k)
            .collect()
    }

    /// Is the point inside the track strip (not the gutter, not the ruler)?
    pub fn in_lanes(&self, pos: egui::Pos2) -> bool {
        self.rect.contains(pos) && pos.y > self.rect.top() + RULER_H
    }
}

/// Events falling into the selection frame (R.33 item 7). A pure function: the frame
/// in window coordinates, geometry — only via `TlGeom`, as the mapping
/// invariant requires. An event is taken on ANY intersection with the frame —
/// the user need not aim at it entirely (item 1 of the checklist).
pub fn rubber_hits(geom: &TlGeom, p: &Project, band: egui::Rect) -> RubberHits {
    let hit = |top: f32, h: f32, a: i64, b: i64| {
        band.intersects(egui::Rect::from_min_max(
            egui::pos2(geom.tick_to_x(a), top),
            egui::pos2(geom.tick_to_x(b), top + h),
        ))
    };
    RubberHits {
        clips: p
            .clips
            .iter()
            .filter(|c| {
                hit(
                    geom.lane_top(c.kind, c.lane),
                    geom.lane_h_of(c.kind, c.lane),
                    c.start.0,
                    (c.start + c.duration).0,
                )
            })
            .map(|c| c.id)
            .collect(),
        // R.40, verbatim: “for some reason morphers are not selected via
        // area-select, you did not check any of that.” The frame caught ONLY clips,
        // and having caught them — it also CANCELLED the selected morpher.
        morphers: p
            .morphers
            .iter()
            .filter(|m| {
                hit(
                    geom.morph_lane_top(m.lane),
                    geom.thin_h_of(LaneKind::Morph, m.lane),
                    m.start.0,
                    (m.start + m.duration).0,
                )
            })
            .map(|m| m.id)
            .collect(),
        cameras: p
            .cameras
            .iter()
            .filter(|c| {
                let row = geom.row_of((LaneKind::Camera, c.lane));
                row.is_some_and(|r| {
                    hit(
                        geom.row_top(r),
                        geom.thin_h_of(LaneKind::Camera, c.lane),
                        c.start.0,
                        (c.start + c.duration).0,
                    )
                })
            })
            .map(|c| c.id)
            .collect(),
    }
}

/// The result of the selection frame. Previously it was a `Vec<ClipId>`, and the type itself
/// declared that the frame is about clips: a morpher and a camera could not get into it
/// (R.40).
#[derive(Default, Clone, Debug)]
pub struct RubberHits {
    pub clips: Vec<ClipId>,
    pub morphers: Vec<MorphId>,
    pub cameras: Vec<kt_model::CameraId>,
}

impl KtApp {
    /// All events that travel with this drag: the anchor, its other
    /// selected neighbors and the link partners of all of them listed.
    pub(crate) fn moving_block_ids(&self, d: &DragState) -> Vec<ClipId> {
        let p = self.model.project();
        let seeds: Vec<ClipId> =
            std::iter::once(d.id).chain(d.extra.iter().map(|(id, _)| *id)).collect();
        let groups: Vec<GroupId> =
            p.clips.iter().filter(|c| seeds.contains(&c.id)).filter_map(|c| c.link_group).collect();
        p.clips
            .iter()
            .filter(|c| seeds.contains(&c.id) || c.link_group.is_some_and(|g| groups.contains(&g)))
            .map(|c| c.id)
            .collect()
    }

    /// Magnetic targets: the edges of foreign clips, markers, region edges,
    /// playhead, zero (port of snapTimeMs).
    ///
    /// Not only the clip itself is excluded but its whole link group: the A/V partner
    /// sits on the same edges and would pull the clip back to the start point
    /// (item 2 of round 11).
    pub fn snap_targets(&self, exclude: Option<ClipId>) -> Vec<i64> {
        let p = self.model.project();
        let group =
            exclude.and_then(|id| p.clips.iter().find(|c| c.id == id)).and_then(|c| c.link_group);
        let mut t: Vec<i64> = vec![0, self.player.transport.playhead().0];
        for c in &p.clips {
            let same_group = group.is_some() && c.link_group == group;
            if Some(c.id) != exclude && !same_group {
                t.push(c.start.0);
                t.push(c.start.0 + c.duration.0);
            }
        }
        for m in &p.markers {
            t.push(m.t.0);
            if m.is_range() {
                t.push(m.end().0);
            }
        }
        if let Some(r) = p.region {
            t.push(r.start.0);
            t.push(r.end.0);
        }
        t
    }

    pub fn snap(&mut self, tick: i64, exclude: Option<ClipId>) -> i64 {
        if self.snap_off {
            return tick;
        }
        // v1 snapTimeMs: threshold 10 px, but not less than 12 ms and not more than 100 ms
        let threshold = snap_threshold(self.ticks_per_px);
        let targets = self.snap_targets(exclude);
        match snap_point(tick, &targets, threshold) {
            Some(t) => {
                self.snap_line = Some(t);
                t
            }
            None => tick,
        }
    }

    pub fn lane_counts(&self) -> (u32, u32) {
        let p = self.model.project();
        (p.lanes(kt_model::LaneKind::Video).max(1), p.lanes(kt_model::LaneKind::Audio).max(1))
    }

    /// TRACK VERBS in the row menu (R.62) — ONE body for all kinds of
    /// rows and for both menus (clip and service). The badge, the menu item and
    /// the hotkey lead to `toggle_track_badge`, because paired
    /// paths of one action must behave identically (rule 17):
    /// if they diverged, “muted from the menu” and “muted via the badge” would give
    /// different results on the same track.
    ///
    /// Mute, solo and chain are shown ONLY where they exist:
    /// service rows have nothing to mute or process, and a dead
    /// menu item is forbidden by rule 9, just as a dead badge is.
    fn track_menu_items(&mut self, ui: &mut egui::Ui, lk: LaneKind, lane: u32) {
        let Some(id) = self.model.project().track_at(lk, lane).map(|t| t.id) else { return };
        let r = ui.button(tr(k::track::MENU_PROPS));
        crate::widgets::field_probe::note("track.menu.props", r.rect);
        if r.clicked() {
            self.open_track_window(id);
            ui.close();
        }
        if lk.has_switches() {
            let r = ui.button(tr(k::track::MENU_MUTE));
            crate::widgets::field_probe::note("track.menu.mute", r.rect);
            if r.clicked() {
                self.toggle_track_badge(id, TrackBadge::Mute);
                ui.close();
            }
            let r = ui.button(tr(k::track::MENU_SOLO));
            crate::widgets::field_probe::note("track.menu.solo", r.rect);
            if r.clicked() {
                self.toggle_track_badge(id, TrackBadge::Solo);
                ui.close();
            }
        }
        let r = ui.button(tr(k::track::MENU_LOCK));
        crate::widgets::field_probe::note("track.menu.lock", r.rect);
        if r.clicked() {
            self.toggle_track_badge(id, TrackBadge::Lock);
            ui.close();
        }
        if lk.has_switches() {
            let r = ui.button(tr(k::track::MENU_FX));
            crate::widgets::field_probe::note("track.menu.fx", r.rect);
            if r.clicked() {
                self.open_track_fx(id);
                ui.close();
            }
        }
    }

    /// The CLIP row menu — one for the row header and for the track itself
    /// (R.40, rule C0). Previously it lived only in `gutter_resp`, and a person could
    /// rename or delete a track only from the strip
    /// 34 px wide.
    fn clip_lane_menu(&mut self, ui: &mut egui::Ui, video_lanes: u32, audio_lanes: u32) {
        if let Some(CtxTarget::Lane { kind, lane }) = self.ctx_target {
            let n = self
                .model
                .project()
                .clips
                .iter()
                .filter(|c| c.kind == kind && c.lane == lane)
                .count();
            let name = match kind {
                ClipKind::Video => format!("V{}", lane + 1),
                ClipKind::Audio => format!("A{}", lane + 1),
            };
            ui.label(egui::RichText::new(crate::tf!(k::tl::lane::TITLE, name)).strong());
            let del_label = if n > 0 {
                crate::tf!(k::tl::lane::DELETE_N, n)
            } else {
                tr(k::tl::lane::DELETE).to_string()
            };
            let lanes_of_kind = match kind {
                ClipKind::Video => video_lanes,
                ClipKind::Audio => audio_lanes,
            };
            let lk = match kind {
                ClipKind::Video => LaneKind::Video,
                ClipKind::Audio => LaneKind::Audio,
            };
            if ui.add_enabled(lanes_of_kind > 1, egui::Button::new(del_label)).clicked() {
                self.delete_lane(kind, lane);
                ui.close();
            }
            if ui.button(tr(k::tl::lane::RENAME)).clicked() {
                let cur = self.row_label(lk, lane);
                self.lane_edit = Some(crate::app::LaneEdit { kind: lk, lane, buf: cur });
                self.lane_edit_focus = true;
                ui.close();
            }
            self.track_menu_items(ui, lk, lane);
            if self.model.project().track_at(lk, lane).is_some_and(|t| t.height > 0.0)
                && ui.button(tr(k::tl::lane::RESET_HEIGHT)).clicked()
            {
                self.set_row_height((lk, lane), None);
                ui.close();
            }
            if ui.button(tr(k::tl::lane::INSERT_ABOVE)).clicked() {
                self.insert_lane_at(lk, lane);
                ui.close();
            }
            if ui.button(tr(k::tl::lane::INSERT_BELOW)).clicked() {
                self.insert_lane_at(lk, lane + 1);
                ui.close();
            }
        }
    }

    /// The EMPTY-SPACE timeline menu — insert, region operations,
    /// adding a track. It is appended to the row menu (R.40): a person
    /// who pressed RMB on a track now gets its verbs too, and these —
    /// previously he got ONLY these, and the row could not be renamed
    /// or deleted with anything except the 34-px header.
    fn empty_place_menu(&mut self, ui: &mut egui::Ui) {
        if !self.clipboard.clips.is_empty() && ui.button(tr(k::tl::menu::PASTE)).clicked() {
            self.paste_at_click();
            ui.close();
        }
        if self.model.project().region.is_some() {
            // the same `delete_action` as Del, and this item exists only with a region
            if crate::widgets::menu_item(ui, tr(k::tl::menu::DELETE_IN_REGION), Some("Del")) {
                self.delete_action();
                ui.close();
            }
            if ui.button(tr(k::tl::menu::PUNCH_OUT)).clicked() {
                self.punch_out_action();
                ui.close();
            }
            ui.separator();
            if ui.button(tr(k::tl::menu::SAVE_AREA)).clicked() {
                self.save_marked_area();
                ui.close();
            }
            if ui.button(tr(k::tl::menu::REGION_CLEAR)).clicked() {
                self.clear_region();
                ui.close();
            }
        } else if self.clipboard.clips.is_empty() {
            ui.weak(tr(k::tl::menu::PICK_SOMETHING));
        }
        // Tracks are added BELOW the clicked row (item 5 of round 16):
        // “not at the very bottom, but after the track that was clicked.”
        // If clicked on empty space — at the end of its own kind.
        ui.separator();
        let row = self.ctx_row;
        for (label, kind) in [
            (tr(k::tl::lane::ADD_VIDEO), LaneKind::Video),
            (tr(k::tl::lane::ADD_AUDIO), LaneKind::Audio),
            (tr(k::tl::lane::ADD_MORPH), LaneKind::Morph),
            (tr(k::tl::lane::ADD_CAMERA), LaneKind::Camera),
        ] {
            if ui.button(label).clicked() {
                self.insert_lane_after_row(kind, row);
                ui.close();
            }
        }
    }

    /// The SERVICE row menu (morph/camera) — ONE for both places from which it
    /// is opened: the row header in the gutter and the track itself (R.40 item 2).
    ///
    /// Until R.40 the block lived only inside `gutter_resp.context_menu`, that is,
    /// in the strip 34 px wide. The item “add morpher here” existed and
    /// was covered by a test — but a person pressing RMB on a track got the empty-space
    /// menu. Splitting this into two copies would mean creating a
    /// second source of truth exactly where one had already been burned.
    fn service_lane_menu(&mut self, ui: &mut egui::Ui) {
        if let Some(CtxTarget::MorphLane(lane)) = self.ctx_target {
            let n = self.model.project().morphers.iter().filter(|m| m.lane == lane).count();
            ui.label(egui::RichText::new(crate::tf!(k::tl::lane::MORPH_TITLE, lane + 1)).strong());
            // symmetrical to the camera track (rule 17): paired
            // ways of one action behave identically, and until
            // R.38 there was NOTHING to put a morpher onto an
            // existing track with — “add morpher layer” always created a new one
            if ui.button(tr(k::tl::lane::ADD_MORPHER_HERE)).clicked() {
                self.add_morpher_at(lane);
                ui.close();
            }
            if !self.model.project().track_at(LaneKind::Morph, lane).is_some_and(|t| t.height > 0.0)
            {
                // there is nothing to reset — the item would silently do nothing
            } else if ui.button(tr(k::tl::lane::RESET_HEIGHT)).clicked() {
                self.set_row_height((LaneKind::Morph, lane), None);
                ui.close();
            }
            // renaming — as for an ordinary track (rule 17):
            // a double-click on the header renames the ROW
            // of ANY kind, and the service rows had no menu item.
            if ui.button(tr(k::tl::lane::RENAME)).clicked() {
                let cur = self.row_label(LaneKind::Morph, lane);
                self.lane_edit =
                    Some(crate::app::LaneEdit { kind: LaneKind::Morph, lane, buf: cur });
                self.lane_edit_focus = true;
                ui.close();
            }
            let del_label = if n > 0 {
                crate::tf!(k::tl::lane::DELETE_MORPH_N, n)
            } else {
                tr(k::tl::lane::DELETE).to_string()
            };
            if ui.button(del_label).clicked() {
                self.apply(Command::RemoveTrack { kind: LaneKind::Morph, at: lane });
                ui.close();
            }
            if ui.button(tr(k::tl::lane::INSERT_MORPH_BELOW)).clicked() {
                self.insert_lane_at(LaneKind::Morph, lane + 1);
                ui.close();
            }
            self.track_menu_items(ui, LaneKind::Morph, lane);
        }
        if let Some(CtxTarget::CameraLane(lane)) = self.ctx_target {
            let n = self.model.project().cameras.iter().filter(|c| c.lane == lane).count();
            ui.label(egui::RichText::new(crate::tf!(k::tl::lane::CAMERA_TITLE, lane + 1)).strong());
            // the camera is placed at the PLAYBACK cursor (like a marker)
            if ui.button(tr(k::tl::lane::ADD_CAMERA_HERE)).clicked() {
                self.add_camera_at_playhead(lane);
                ui.close();
            }
            if !self
                .model
                .project()
                .track_at(LaneKind::Camera, lane)
                .is_some_and(|t| t.height > 0.0)
            {
                // there is nothing to reset — the item would silently do nothing
            } else if ui.button(tr(k::tl::lane::RESET_HEIGHT)).clicked() {
                self.set_row_height((LaneKind::Camera, lane), None);
                ui.close();
            }
            // renaming — as for an ordinary track (rule 17):
            // a double-click on the header renames the ROW
            // of ANY kind, and the service rows had no menu item.
            if ui.button(tr(k::tl::lane::RENAME)).clicked() {
                let cur = self.row_label(LaneKind::Camera, lane);
                self.lane_edit =
                    Some(crate::app::LaneEdit { kind: LaneKind::Camera, lane, buf: cur });
                self.lane_edit_focus = true;
                ui.close();
            }
            let del_label = if n > 0 {
                crate::tf!(k::tl::lane::DELETE_CAMERA_N, n)
            } else {
                tr(k::tl::lane::DELETE).to_string()
            };
            if ui.button(del_label).clicked() {
                self.apply(Command::RemoveTrack { kind: LaneKind::Camera, at: lane });
                ui.close();
            }
            if ui.button(tr(k::tl::lane::INSERT_CAMERA_BELOW)).clicked() {
                self.insert_lane_at(LaneKind::Camera, lane + 1);
                ui.close();
            }
            self.track_menu_items(ui, LaneKind::Camera, lane);
        }
    }

    pub fn draw_timeline(&mut self, ui: &mut egui::Ui) {
        // one pass of coverage polls per frame (cache by epoch, item 1);
        // Arc-snapshot — a pointer clone, not a map (item 1 of v1.4)
        let coverage_snapshot = self.coverage_runs();
        let full = ui.available_rect_before_wrap();
        let gutter =
            egui::Rect::from_min_max(full.min, egui::pos2(full.left() + GUTTER_W, full.bottom()));
        let rect = egui::Rect::from_min_max(egui::pos2(gutter.right(), full.top()), full.max);
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let painter = ui.painter_at(full);
        painter.rect_filled(full, 0.0, crate::theme::TL_BG);
        // INSIDE A REEL the background is tinted (R.50, B2): the only sign
        // visible at the edge of the eye, without reading the crumbs. A strip of its own or a
        // frame for this we do not create — rule 13.
        if self.model.editing().is_some() {
            painter.rect_filled(full, 0.0, egui::Color32::from_rgba_unmultiplied(80, 60, 120, 16));
        }
        // track headers are interactive: drag = reorder (item 7), RMB = the menu
        // of delete/insert (item 8)
        let gutter_resp =
            ui.interact(gutter, ui.id().with("lane-gutter"), egui::Sense::click_and_drag());

        // the number of rows in advance — vertical zoom by wheel needs, before layout,
        // TWO counters: clip rows and service ones (they are a third lower).
        // Previously camera tracks were not counted here at all, while in layout
        // they were — vertical zoom by wheel took the wrong height.
        let (clip_hint, morph_hint, cam_hint) = {
            let (v, a) = self.lane_counts();
            let p = self.model.project();
            (
                (v + a).max(1) as usize,
                p.lanes(kt_model::LaneKind::Morph) as usize,
                p.lanes(kt_model::LaneKind::Camera) as usize,
            )
        };
        // ---- zoom/pan + edge-autoscroll during a drag --------------------------
        let mut view_start = self.view_start;
        let mut tpp = self.ticks_per_px;
        // contains_pointer, NOT hover_pos: during a foreign egui drag, hovered is muted for
        // all other widgets — the wheel stopped working (item 28 round 7); contains_pointer is
        // not suppressed by an active drag
        if response.contains_pointer()
            && let Some(pos) = ui.input(|i| i.pointer.hover_pos())
        {
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            // IMPORTANT: egui reroutes Shift+wheel into scroll.x — for
            // vertical gestures (item 12 round 6) read the RAW wheel+modifiers:
            // the raw wheel: MouseWheel events BEFORE egui processing (Shift
            // reroutes smooth_scroll into X — and we need the vertical gesture)
            let raw_y: f32 = ui.input(|i| {
                i.raw
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::MouseWheel { unit, delta, .. } => Some(match unit {
                            egui::MouseWheelUnit::Point => delta.y,
                            egui::MouseWheelUnit::Line => delta.y * 20.0,
                            egui::MouseWheelUnit::Page => delta.y * 200.0,
                        }),
                        _ => None,
                    })
                    .sum()
            });
            let zoom = ui.input(|i| i.zoom_delta());
            let (ctrl, shift) =
                ui.input(|i| (i.modifiers.command || i.modifiers.ctrl, i.modifiers.shift));
            // Shift-vertical does NOT move the window — follow is not cancelled (item 30 round 7)
            let user_scrolled = (!shift && scroll != egui::Vec2::ZERO) || zoom != 1.0;
            if user_scrolled {
                // a manual window move disables following (item 14 round 6)
                self.follow_playhead = false;
                self.last_user_scroll = Some(std::time::Instant::now());
            }
            if shift && ctrl && raw_y != 0.0 {
                // Shift+Ctrl+wheel = vertical zoom, anchor — the row
                // under the cursor (item 12 round 6)
                let cur_h = self.tl_v_zoom.unwrap_or_else(|| {
                    vertical_layout(rect.height(), clip_hint, morph_hint, cam_hint, None).base_h
                });
                let factor = if raw_y > 0.0 { 1.15 } else { 1.0 / 1.15 };
                let new_h = (cur_h * factor).clamp(16.0, 120.0);
                // anchor — the row UNDER THE CURSOR in row fractions; dividing by
                // one height cannot compute it: rows have different heights
                let row = self
                    .tl_geom
                    .as_ref()
                    .map(|g| g.anchor_at(pos.y))
                    .unwrap_or_else(|| {
                        ((pos.y - (rect.top() + RULER_H + 3.0) + self.tl_scroll) / cur_h).max(0.0)
                    })
                    .max(0.0);
                self.tl_v_zoom = Some(new_h);
                self.tl_scroll = (row * new_h - (pos.y - (rect.top() + RULER_H + 3.0))).max(0.0);
            } else if shift && raw_y != 0.0 {
                // Shift+wheel = vertical scroll (clamp below, by layout)
                self.tl_scroll = (self.tl_scroll - raw_y).max(0.0);
            } else if ctrl && scroll.y != 0.0 || zoom != 1.0 {
                // zoom: Ctrl+wheel or pinch, anchor under the cursor (port of the old one)
                let factor = if zoom != 1.0 {
                    1.0 / zoom as f64
                } else if scroll.y > 0.0 {
                    0.8
                } else {
                    1.25
                };
                let anchor = view_start + (pos.x - rect.left()) as f64 * tpp;
                tpp = (tpp * factor).clamp(TPP_MIN, TPP_MAX);
                view_start = anchor - (pos.x - rect.left()) as f64 * tpp;
            } else if !shift && (scroll.x != 0.0 || scroll.y != 0.0) {
                // bare wheel (any axis) = scroll along the timeline (port of the old one)
                let delta = if scroll.x.abs() > scroll.y.abs() { scroll.x } else { scroll.y };
                view_start -= delta as f64 * tpp * 2.5;
            }
        }
        // “Jump” zones at the edges (port): heat = proximity to the edge [-1..1].
        let edge_margin = (rect.width() / 8.0).max(40.0);
        let edge_heat = |x: f32| -> f32 {
            if x < rect.left() + edge_margin {
                -((rect.left() + edge_margin - x) / edge_margin).clamp(0.0, 1.0)
            } else if x > rect.right() - edge_margin {
                ((x - (rect.right() - edge_margin)) / edge_margin).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        // during a drag — smooth autoscroll (heat^4.15, port)
        if self.drag.is_some()
            && let Some(pos) = response.hover_pos()
        {
            let heat = edge_heat(pos.x);
            if heat != 0.0 {
                let ms = (heat.abs().powf(4.15) * 0.18 * 240.0).clamp(1.0, 42.0) as f64;
                view_start += heat.signum() as f64 * ms * 90.0;
            }
        }
        // ensureCursorVisible during playback — ONLY in follow mode
        // (item 14 round 6): manual pan/zoom disables following until the next Play.
        // Auto-restoration (item 30 round 7): region visible + cursor in it +
        // the user has stopped scrolling ≥1.5 s → follow switches itself back on.
        if !self.follow_playhead
            && follow_rearm(
                self.player.transport.is_playing(),
                self.last_user_scroll.map(|t| t.elapsed().as_secs_f32()).unwrap_or(f32::MAX),
                1.5,
                self.player.transport.playhead().0 as f64,
                self.model.project().region.map(|r| (r.start.0 as f64, r.end.0 as f64)),
                view_start,
                rect.width() as f64 * tpp,
            )
        {
            self.follow_playhead = true;
        }
        // scrolling after a jump between stop points (item 11 round 9): the same
        // zones as follow_view, but without the “playing” requirement — otherwise while
        // paused Home/End left the window in place.
        if let Some(at) = self.pending_reveal.take()
            && let Some(vs) =
                follow_view(true, true, false, at as f64, view_start, rect.width() as f64 * tpp)
        {
            view_start = vs;
        }
        if let Some(vs) = follow_view(
            self.follow_playhead,
            self.player.transport.is_playing(),
            self.drag.is_some(),
            self.player.transport.playhead().0 as f64,
            view_start,
            rect.width() as f64 * tpp,
        ) {
            view_start = vs;
        }
        // scroll floor: at the root −40 px, inside a reel — down to the earliest
        // event (the zone before zero is shown, R.58 item 462)
        view_start = view_start.max(self.view_floor(tpp));
        self.view_start = view_start;
        self.ticks_per_px = tpp;
        let x_to_tick =
            move |x: f32| -> i64 { (view_start + (x - rect.left()) as f64 * tpp) as i64 };
        let tick_to_x =
            move |t: i64| -> f32 { rect.left() + ((t as f64 - view_start) / tpp) as f32 };

        // ---- global cache strips (port: cyan=audio, green=video) --------
        {
            let y0 = rect.top() + 1.0;
            let cyan = egui::Color32::from_rgb(114, 214, 255);
            let green = egui::Color32::from_rgb(72, 168, 96);
            // background underlays
            for (row, _) in [(0.0f32, cyan), (5.0, green)] {
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(full.left(), y0 + row),
                        egui::pos2(rect.right(), y0 + row + 4.0),
                    ),
                    0.0,
                    egui::Color32::from_gray(30),
                );
            }
            let q = self.player.preview_quality();
            for c in &self.model.project().clips {
                let (kind, row, col) = match c.kind {
                    ClipKind::Audio => (BurstKind::Audio, 0.0f32, cyan),
                    ClipKind::Video => (BurstKind::video(Some(q)), 5.0, green),
                };
                let (b0, b1) = match c.kind {
                    ClipKind::Audio => {
                        let f0 = c.source_in.to_audio_frames();
                        let f1 = (c.source_in + c.duration).to_audio_frames();
                        (
                            f0.div_euclid(AUDIO_BURST_FRAMES as i64),
                            (f1 - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1,
                        )
                    }
                    ClipKind::Video => (c.source_in.0, c.source_in.0 + c.duration.0),
                };
                let _ = (kind, b0, b1);
                for run in coverage_snapshot.get(&c.id).map(|v| v.as_slice()).unwrap_or(&[]) {
                    let (t0, t1) = match c.kind {
                        ClipKind::Audio => (
                            c.start.0
                                + Tick::from_audio_frames(
                                    run.start * AUDIO_BURST_FRAMES as i64
                                        - c.source_in.to_audio_frames(),
                                )
                                .0,
                            c.start.0
                                + Tick::from_audio_frames(
                                    run.end * AUDIO_BURST_FRAMES as i64
                                        - c.source_in.to_audio_frames(),
                                )
                                .0,
                        ),
                        ClipKind::Video => (
                            c.start.0 + run.start - c.source_in.0,
                            c.start.0 + run.end - c.source_in.0,
                        ),
                    };
                    // the strip goes left under the name strip, like the tracks
                    // (item 3 round 8) — the gradient on top of it shows through
                    let x0 = tick_to_x(t0).max(full.left());
                    let x1 = tick_to_x(t1).min(rect.right());
                    if x1 > x0 {
                        painter.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(x0, y0 + row),
                                egui::pos2(x1, y0 + row + 4.0),
                            ),
                            0.0,
                            col,
                        );
                    }
                }
            }
            // amber “head” of the loop region over the video strip (port of head)
            if let Some(r) = self.model.project().region
                && self.model.project().loop_enabled
            {
                let head_end = (r.start.0 + 90_000 / 2).min(r.end.0);
                let x0 = tick_to_x(r.start.0).max(full.left());
                let x1 = tick_to_x(head_end).min(rect.right());
                if x1 > x0 {
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(x0, y0 + 5.0),
                            egui::pos2(x1, y0 + 9.0),
                        ),
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(255, 210, 72, 150),
                    );
                }
            }
        }

        // ---- ruler: step ladder (item 17 round 6), ticks in one mesh ---------
        let (minor_step, label_step) = ruler_steps(tpp);
        let mut ruler_mesh = egui::Mesh::default();
        let tick_col = egui::Color32::from_gray(70);
        // to the left of zero the ticks and labels exist only inside the reel (the view's floor
        // there is below zero); `div_euclid` — so the grid does not go off on minus
        let t0 = x_to_tick(rect.left()).max(self.time_floor()).div_euclid(minor_step) * minor_step;
        let t1 = x_to_tick(rect.right()) + minor_step;
        let mut t = t0;
        while t <= t1 {
            let x = tick_to_x(t);
            let major = t % label_step == 0;
            ruler_mesh.add_colored_rect(
                egui::Rect::from_min_max(
                    egui::pos2(x, rect.top() + TIME_TOP),
                    egui::pos2(x + 1.0, rect.top() + if major { RULER_H } else { TIME_TOP + 6.0 }),
                ),
                tick_col,
            );
            if major {
                painter.text(
                    egui::pos2(x + 4.0, rect.top() + TIME_TOP + 1.0),
                    egui::Align2::LEFT_TOP,
                    format_ruler_time_step(t, label_step),
                    egui::FontId::proportional(10.5),
                    egui::Color32::from_gray(150),
                );
            }
            t += minor_step;
        }
        // A LABEL OF A TICK THAT HAS GONE LEFT is pinned to the edge (R.40 item 14,
        // verbatim: “and if it zooms in, then this time flies off to the side”).
        // The label was drawn only at a tick falling in the window, and anchored
        // LEFT_TOP — on zoom the last one slid past the left edge, and on the
        // ruler not A SINGLE time remained. Now the nearest label to the left
        // is always visible and always honest.
        let pinned = ruler_pinned_label(x_to_tick(rect.left()), label_step);
        if pinned >= 0 && tick_to_x(pinned) < rect.left() {
            let next = tick_to_x(pinned + label_step);
            let txt = format_ruler_time_step(pinned, label_step);
            let g = painter.layout_no_wrap(
                txt,
                egui::FontId::proportional(10.5),
                egui::Color32::from_gray(120),
            );
            // we do not run into the label of the NEXT tick: if it does not fit — we stay silent
            if g.size().x + 6.0 <= next - rect.left() {
                painter.galley(
                    egui::pos2(rect.left() + 2.0, rect.top() + TIME_TOP + 1.0),
                    g,
                    egui::Color32::from_gray(120),
                );
            }
        }
        if !ruler_mesh.is_empty() {
            painter.add(egui::Shape::mesh(ruler_mesh));
        }
        painter.line_segment(
            [
                egui::pos2(full.left(), rect.top() + RULER_H),
                egui::pos2(full.right(), rect.top() + RULER_H),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_gray(45)),
        );

        // the track zone is drawn with the clip BELOW the ruler: during vertical
        // scroll (item 12 round 6) content slides under the ruler, not over it
        let full_painter = painter.clone();
        let painter = ui.painter_at(egui::Rect::from_min_max(
            egui::pos2(full.left(), rect.top() + RULER_H + 1.0),
            full.max,
        ));

        // ---- tracks ---------------------------------------------------------
        let (video_lanes, audio_lanes) = self.lane_counts();
        let morph_lanes = self.model.project().lanes(kt_model::LaneKind::Morph);
        let camera_lanes = self.model.project().lanes(kt_model::LaneKind::Camera);
        let vl = vertical_layout(
            rect.height(),
            (video_lanes + audio_lanes).max(1) as usize,
            morph_lanes as usize,
            camera_lanes as usize,
            self.tl_v_zoom,
        );
        // HEIGHT — PER ROW (R.40 item 11). Until this round this held
        // `let lane_h = vl.base_h;` — one variable for all rows, with 36
        // uses: the event rectangle, zones, the top strip, the
        // handles, the level line, hit-test. The row grew — the event did not.
        let base_h = vl.base_h;
        // scroll clamp every frame (track removal / panel resize)
        self.tl_scroll = self.tl_scroll.clamp(0.0, vl.scroll_max);
        // display order of rows: since R.35 it lives in the MODEL (the morpher layer
        // acts “on everything below”), and `rows()` normalizes it to the
        // actual track counts on every frame
        let rows = self.model.project().rows();
        // geometry cache for mapping outside draw_timeline (drop item 2, rows item 6)
        let row_over = self.row_heights();
        let geom = TlGeom::new(
            rect,
            base_h,
            rows,
            &row_over,
            self.tl_scroll,
            self.view_start,
            self.ticks_per_px,
        );
        // heights are taken PER ROW via geometry: each track's
        // closures take the geometry BY REFERENCE — then they are `Copy` and
        // can be given to all `move` closures of the hit-tests
        let gref = &geom;
        let lane_h_at = move |kind: ClipKind, lane: u32| gref.lane_h_of(kind, lane);
        let thin_h_at = move |kind: LaneKind, lane: u32| gref.thin_h_of(kind, lane);
        self.tl_geom = Some(geom.clone());
        // KT_GEOM_DUMP=<file> — the timeline geometry exposed for live
        // scenarios (R.27): without it the script is forced to GUESS where the track is and
        // how many pixels per second, and breaks on any layout edit.
        // Only via env, no more than 10 Hz, by an atomic file replacement.
        // Modal windows the program names itself: they live in the status line layer,
        // and by layers they cannot be told apart from it.
        let modal = usize::from(self.ask.is_some() || self.recovery_open);
        self.dump_geom(&geom, ui.ctx().content_rect(), crate::bench::open_windows(ui.ctx()) + modal);
        let lane_top = |kind: ClipKind, lane: u32| -> f32 { geom.lane_top(kind, lane) };
        let morph_lane_top = |lane: u32| -> f32 { geom.morph_lane_top(lane) };
        // (label, row center, row top) — the top is needed so that a long
        // label lands UNDER the clip name, not over it
        let mut gutter_labels: Vec<(String, f32, f32, f32)> = Vec::with_capacity(geom.rows.len());
        for (row, &(kind, lane)) in geom.rows.iter().enumerate() {
            let y = geom.row_top(row);
            // height of THIS row: service ones are a third lower (R.35 item 1)
            let rh = geom.row_h(row);
            // the track background extends UNDER the gutter — the gradient below “dissolves”
            // the number strip into the tracks (item 20 round 6)
            let lane_rect = egui::Rect::from_min_max(
                egui::pos2(full.left(), y),
                egui::pos2(rect.right(), y + rh - 2.0),
            );
            // track backgrounds in the tones of the old one: video bluish, audio warm,
            // morph purple (darker — the round 6 theme)
            let fill = match kind {
                LaneKind::Video => crate::theme::LANE_VIDEO,
                LaneKind::Audio => crate::theme::LANE_AUDIO,
                LaneKind::Morph => crate::theme::LANE_MORPH,
                LaneKind::Camera => crate::theme::LANE_CAMERA,
            };
            // label: the user's name or the formula V1/A1/M1 (item 2 round 10).
            // For a row of an EXPANDED STRIP (R.55, C5) the names of the root tracks would
            // lie: this is a row of ANOTHER reel. The label gets ITS name,
            // and the number is counted from the start of the strip.
            let band = self.model.band_of_row(kind, lane).copied();
            let label = match &band {
                Some(b) => {
                    let name = self
                        .model
                        .project()
                        .sequence(b.seq)
                        .map(|s| s.name.clone())
                        .unwrap_or_default();
                    let local = lane - b.base.get(kind);
                    format!("{name} {}", kt_model::lane_label(kind, local))
                }
                None => self.row_label(kind, lane),
            };
            painter.rect_filled(lane_rect, 0.0, fill);
            // the STRIP BACKGROUND is tinted: two coordinate systems on one screen
            // must be distinguishable, otherwise a person edits not what he thinks
            if band.is_some() {
                painter.rect_filled(lane_rect, 0.0, crate::theme::BAND_TINT);
            }
            // An EMPTY service row itself says what may be placed on it
            // (R.40 item 5). We do not create a strip of its own for this — the inscription lives
            // INSIDE the empty row, which takes up space anyway
            // (rule 13 of the checklist); once an event appears — the inscription is gone.
            let empty_svc = match kind {
                LaneKind::Morph
                    if !self.model.project().morphers.iter().any(|m| m.lane == lane) =>
                {
                    Some(k::tl::lane::ADD_MORPHER_HERE)
                }
                LaneKind::Camera
                    if !self.model.project().cameras.iter().any(|c| c.lane == lane) =>
                {
                    Some(k::tl::lane::ADD_CAMERA_HERE)
                }
                _ => None,
            };
            if let Some(key) = empty_svc {
                let fs = (rh * 0.55).clamp(8.0, 12.0);
                painter.text(
                    egui::pos2(rect.left() + 6.0, y + rh * 0.5),
                    egui::Align2::LEFT_CENTER,
                    tr(key),
                    egui::FontId::proportional(fs),
                    egui::Color32::from_rgb(112, 118, 132),
                );
            }
            gutter_labels.push((label, y + rh * 0.5, y, rh));
        }
        // THE EDGE OF THE WINDOW of an expanded strip (R.55, C5). The grab zone is the same
        // as for the clip trims, but the priority is ABOVE the strip's events: the edge's
        // vertical line passes over them, and a person aims for it
        // just as he would at the edge itself.
        let band_edge_at = |slf: &KtApp, pos: egui::Pos2| -> Option<(ClipId, bool)> {
            let key = geom.key_at_y(pos.y)?;
            let b = slf.model.band_of_row(key.0, key.1)?;
            let win = b.window(slf.model.project())?;
            let (xa, xb) = (tick_to_x(win.start.0), tick_to_x(win.end.0));
            if (pos.x - xa).abs() <= EDGE_GRAB_PX {
                Some((b.inst, true))
            } else if (pos.x - xb).abs() <= EDGE_GRAB_PX {
                Some((b.inst, false))
            } else {
                None
            }
        };
        // hit-test of the morph block (needed by both press handling and RMB below)
        // the morph block's edge: the same grab zone as the clip trims
        let morph_edge_at = |slf: &KtApp, pos: egui::Pos2| -> Option<(MorphId, bool)> {
            let lane = geom.morph_lane_at_y(pos.y)?;
            slf.model.project().morphers.iter().rev().filter(|m| m.lane == lane).find_map(|m| {
                let x0 = tick_to_x(m.start.0);
                let x1 = tick_to_x((m.start + m.duration).0).max(x0 + 8.0);
                if (pos.x - x0).abs() <= EDGE_GRAB_PX {
                    Some((m.id, true))
                } else if (pos.x - x1).abs() <= EDGE_GRAB_PX {
                    Some((m.id, false))
                } else {
                    None
                }
            })
        };
        let camera_edge_at = |slf: &KtApp, pos: egui::Pos2| -> Option<(kt_model::CameraId, bool)> {
            let lane = geom.camera_lane_at_y(pos.y)?;
            slf.model.project().cameras.iter().rev().filter(|c| c.lane == lane).find_map(|c| {
                let x0 = tick_to_x(c.start.0);
                let x1 = tick_to_x((c.start + c.duration).0).max(x0 + 8.0);
                if (pos.x - x0).abs() <= EDGE_GRAB_PX {
                    Some((c.id, true))
                } else if (pos.x - x1).abs() <= EDGE_GRAB_PX {
                    Some((c.id, false))
                } else {
                    None
                }
            })
        };
        let morpher_at = |slf: &KtApp, pos: egui::Pos2| -> Option<MorphId> {
            let lane = geom.morph_lane_at_y(pos.y)?;
            slf.model
                .project()
                .morphers
                .iter()
                .rev()
                .filter(|m| m.lane == lane)
                .find(|m| {
                    let x0 = tick_to_x(m.start.0);
                    let x1 = tick_to_x((m.start + m.duration).0).max(x0 + 8.0);
                    pos.x >= x0 && pos.x <= x1
                })
                .map(|m| m.id)
        };
        // hit-test of a camera event (R.34 K3): until this round a camera could be
        // neither selected, moved, nor opened — it was drawn and that was all
        let camera_at = |slf: &KtApp, pos: egui::Pos2| -> Option<kt_model::CameraId> {
            let lane = geom.camera_lane_at_y(pos.y)?;
            slf.model
                .project()
                .cameras
                .iter()
                .rev()
                .filter(|c| c.lane == lane)
                .find(|c| {
                    let x0 = tick_to_x(c.start.0);
                    let x1 = tick_to_x((c.start + c.duration).0).max(x0 + 8.0);
                    pos.x >= x0 && pos.x <= x1
                })
                .map(|c| c.id)
        };

        // ---- tracks: reorder by drag and the menu (items 7-8; item 6 of v1.4: rows) ----
        {
            // THE TRACK BADGE (R.62) — before renaming and reordering:
            // the zone is narrow and specific, and a gesture started from it must
            // go to it. The row geometry is a PURE function of the row, not a
            // list filled in by drawing: headers are drawn LATER than
            // input handling, and such a list would lag by a frame.
            let badge_at = |g: &TlGeom, slf: &KtApp, pos: egui::Pos2| {
                let row = g.row_at_y(pos.y)?;
                let (kind, lane) = g.rows.get(row).copied()?;
                if slf.model.band_of_row(kind, lane).is_some() {
                    return None;
                }
                let t = slf.model.project().track_at(kind, lane)?;
                TrackBadge::row_rects(kind, gutter.left(), g.row_top(row), g.row_h(row))
                    .into_iter()
                    .find(|(_, r)| r.contains(pos))
                    .map(|(b, _)| (t.id, b))
            };
            if gutter_resp.clicked()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
                && let Some((id, badge)) = badge_at(&geom, self, pos)
            {
                self.toggle_track_badge(id, badge);
                self.drag = None;
            }
            // double-click on the track header — rename (item 2 round 10)
            else if gutter_resp.double_clicked()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
                && let Some((kind, lane)) = geom.key_at_y(pos.y)
            {
                let cur = self.row_label(kind, lane);
                self.lane_edit = Some(crate::app::LaneEdit { kind, lane, buf: cur });
                self.lane_edit_focus = true;
                self.drag = None;
            }
            // while a name is being typed — the drag-reorder is off, otherwise “clicked
            // in the field — the track moved away”
            // the bottom edge of the row in the gutter drags its HEIGHT (R.38 item 11).
            // It is checked BEFORE the reorder: the edge is narrow, and a reorder started
            // from it would take the gesture for itself.
            let row_edge_at = |g: &TlGeom, y: f32| -> Option<usize> {
                (0..g.rows.len()).find(|r| (g.row_top(*r) + g.row_h(*r) - y).abs() <= ROW_EDGE_PX)
            };
            if self.lane_edit.is_none()
                && let Some(pos) = gutter_resp.hover_pos()
                && row_edge_at(&geom, pos.y).is_some()
            {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                self.hover_hint = Some(tr(k::tl::hint::ROW_HEIGHT).to_string());
            }
            // The zone is taken by the POINT OF PRESS, not by the current pointer:
            // `drag_started` arrives only after a shift past the threshold, and the pointer
            // manages to crawl out of the narrow edge strip — the gesture would turn
            // into a reorder. The same lesson as the clip trims (item 28).
            let gutter_press = ui.input(|i| i.pointer.press_origin());
            if self.lane_edit.is_none()
                && gutter_resp.drag_started()
                && let Some(pos) = gutter_press.or_else(|| gutter_resp.interact_pointer_pos())
                && let Some(row) = row_edge_at(&geom, pos.y)
            {
                self.drag = Some(Drag::LaneHeight { row, cur: geom.row_h(row) });
            } else if self.lane_edit.is_none()
                && gutter_resp.drag_started()
                && let Some(pos) = gutter_press.or_else(|| gutter_resp.interact_pointer_pos())
                && let Some(row) = geom.row_at_y(pos.y)
            {
                self.drag = Some(Drag::LaneRow { from: row, cur: row });
            }
            if gutter_resp.dragged()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
                && let Some(Drag::LaneHeight { row, .. }) = self.drag
            {
                let cur = (pos.y - geom.row_top(row)).clamp(THIN_ROW_MIN, ROW_H_MAX);
                self.drag = Some(Drag::LaneHeight { row, cur });
            }
            if gutter_resp.drag_stopped()
                && let Some(Drag::LaneHeight { row, cur }) = self.drag.take()
                && let Some(key) = geom.rows.get(row).copied()
            {
                self.set_row_height(key, Some(cur));
            }
            if gutter_resp.dragged()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
                && let Some(Drag::LaneRow { from, .. }) = self.drag
                && let Some(cur) = geom.row_at_y(pos.y)
            {
                self.drag = Some(Drag::LaneRow { from, cur });
            }
            if gutter_resp.drag_stopped()
                && let Some(Drag::LaneRow { from, cur }) = self.drag.take()
                && from != cur
            {
                self.reorder_rows(from, cur);
            }
            // indicator of the row being dragged
            if let Some(Drag::LaneRow { cur, .. }) = self.drag {
                let y = geom.row_top(cur);
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(gutter.left(), y),
                        egui::pos2(rect.right(), y + geom.row_h(cur) - 2.0),
                    ),
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(120, 160, 255, 26),
                );
            }
            // RMB on the header — the track menu
            if gutter_resp.secondary_clicked()
                && let Some(pos) = gutter_resp.interact_pointer_pos()
            {
                self.ctx_panel = None; // targets cancel each other out (item 363)
                match geom.key_at_y(pos.y) {
                    Some((LaneKind::Video, lane)) => {
                        self.ctx_target = Some(CtxTarget::Lane { kind: ClipKind::Video, lane });
                    }
                    Some((LaneKind::Audio, lane)) => {
                        self.ctx_target = Some(CtxTarget::Lane { kind: ClipKind::Audio, lane });
                    }
                    Some((LaneKind::Morph, lane)) => {
                        self.ctx_target = Some(CtxTarget::MorphLane(lane));
                    }
                    Some((LaneKind::Camera, lane)) => {
                        self.ctx_target = Some(CtxTarget::CameraLane(lane));
                    }
                    None => {}
                }
            }
            gutter_resp.context_menu(|ui| {
                self.clip_lane_menu(ui, video_lanes, audio_lanes);
                self.service_lane_menu(ui);
            });
        }

        // ---- region: fill ---------------------------------------------------
        if let Some(r) = self.model.project().region {
            let x0 = tick_to_x(r.start.0).max(rect.left());
            let x1 = tick_to_x(r.end.0).min(rect.right());
            if x1 > x0 {
                // v1: center alpha 14, side gradients toward the edges alpha 58,
                // fade width min(56, w/2)
                let base = |a: u8| egui::Color32::from_rgba_unmultiplied(84, 124, 255, a);
                let fade_w = ((x1 - x0) / 2.0).clamp(1.0, 56.0);
                let grad_h = |xa: f32, xb: f32, ca: egui::Color32, cb: egui::Color32| {
                    let mut mesh = egui::Mesh::default();
                    let i = mesh.vertices.len() as u32;
                    for (pos, col) in [
                        (egui::pos2(xa, rect.top()), ca),
                        (egui::pos2(xb, rect.top()), cb),
                        (egui::pos2(xb, rect.bottom()), cb),
                        (egui::pos2(xa, rect.bottom()), ca),
                    ] {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos,
                            uv: egui::epaint::WHITE_UV,
                            color: col,
                        });
                    }
                    mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
                    painter.add(egui::Shape::mesh(mesh));
                };
                grad_h(x0, x0 + fade_w, base(58), base(14));
                if x1 - fade_w > x0 + fade_w {
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(x0 + fade_w, rect.top()),
                            egui::pos2(x1 - fade_w, rect.bottom()),
                        ),
                        0.0,
                        base(14),
                    );
                }
                grad_h(x1, x1 - fade_w, base(58), base(14));
                for (edge_t, x) in
                    [(r.start.0, tick_to_x(r.start.0)), (r.end.0, tick_to_x(r.end.0))]
                {
                    let _ = edge_t;
                    if x >= rect.left() && x <= rect.right() {
                        painter.line_segment(
                            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(102, 152, 255)),
                        );
                    }
                }
            }
        }

        // ---- hit-tests --------------------------------------------------------
        // caches by model_rev; Arc-snapshots — pointer clones (item 1 of v1.4)
        let (media_names, suppress_name, media_paths, draw_order) = self.names();
        let overlap_zones = self.overlap_zones();
        let pointer = response.interact_pointer_pos();
        let hover = response.hover_pos();
        // the row UNDER THE MOUSE lives exactly while the mouse is over the timeline (R.56,
        // item 450): Ctrl+V places a block here, and outside the timeline there is no row
        // and nothing to guess it by
        self.tl_hover_row = hover.and_then(|p| geom.key_at_y(p.y));
        self.snap_line = None;
        // the obstacle lives EXACTLY while the gesture runs into it (item 5)
        self.blocked = None;
        self.blocked_material = None;
        // the magnet is always on; Ctrl turns it off (item 2 round 11). The same Ctrl,
        // pressed BEFORE grabbing empty space, starts a selection frame
        // (R.33 item 7) — on moving an event the magnet does not suffer from this
        self.snap_off = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
        let ctrl_held = self.snap_off;
        let shift_held = ui.input(|i| i.modifiers.shift);

        let marker_at = |app: &KtApp, pos: egui::Pos2| -> Option<i64> {
            if pos.y > rect.top() + RULER_H {
                return None;
            }
            app.model
                .project()
                .markers
                .iter()
                .map(|m| m.t.0)
                .min_by_key(|m| (tick_to_x(*m) - pos.x).abs() as i64)
                .filter(|m| (tick_to_x(*m) - pos.x).abs() <= EDGE_GRAB_PX)
        };
        // the end of a range marker (R.24): grabbed on the same strip as the flags
        // the LEFT edge of a range marker (R.38, sweep). `Drag::MarkerEdge`
        // with `left: true` was described in a comment as working, but was never
        // constructed: the gesture went to `_ => {}` and vanished silently.
        let marker_start_at = |app: &KtApp, pos: egui::Pos2| -> Option<i64> {
            if pos.y > rect.top() + RULER_H {
                return None;
            }
            app.model
                .project()
                .markers
                .iter()
                .filter(|m| m.is_range())
                .map(|m| m.t.0)
                .find(|start| (tick_to_x(*start) - pos.x).abs() <= EDGE_GRAB_PX)
        };
        let marker_end_at = |app: &KtApp, pos: egui::Pos2| -> Option<i64> {
            if pos.y > rect.top() + RULER_H {
                return None;
            }
            app.model.project().markers.iter().filter(|m| m.is_range()).map(|m| m.t.0).find(
                |start| {
                    let end = start
                        + app
                            .model
                            .project()
                            .markers
                            .iter()
                            .find(|m| m.t.0 == *start)
                            .map(|m| m.duration.0)
                            .unwrap_or(0);
                    (tick_to_x(end) - pos.x).abs() <= EDGE_GRAB_PX
                },
            )
        };
        // A named area (R.38 item 1: “I don't understand how to delete a saved
        // area region… RMB gives nothing”). The OUTLINE is grabbed — the body
        // is given to the events under it, otherwise the area would steal all clicks on
        // its tracks. The grab strip is wider than the drawing (rule 1): a
        // 1 px line cannot be caught with a mouse.
        let area_at = |app: &KtApp, pos: egui::Pos2| -> Option<(usize, bool)> {
            app.model.project().areas.iter().enumerate().find_map(|(i, a)| {
                let r = geom.area_rect(a)?;
                let band = r.expand(AREA_GRAB_PX);
                if !band.contains(pos) || r.shrink(AREA_GRAB_PX).contains(pos) {
                    return None;
                }
                // left edge or everything else (right/top/bottom)
                Some((i, (pos.x - r.left()).abs() <= AREA_GRAB_PX))
            })
        };
        // region boundaries are grabbed over the WHOLE height (item 14): the lines are drawn
        // over the full height and must be clickable everywhere
        let region_edge_at = |app: &KtApp, pos: egui::Pos2| -> Option<bool> {
            let r = app.model.project().region?;
            if (tick_to_x(r.start.0) - pos.x).abs() <= REGION_EDGE_PX {
                Some(true)
            } else if (tick_to_x(r.end.0) - pos.x).abs() <= REGION_EDGE_PX {
                Some(false)
            } else {
                None
            }
        };
        #[derive(PartialEq)]
        enum ClipZone {
            Body,
            TrimStart,
            TrimEnd,
            /// Fade handle at the start/end of an event (item 12).
            FadeLeft,
            FadeRight,
            /// Level line of the event: opacity for video, volume for
            /// audio (R.28, item 12 + requirement “fades for audio and video
            /// are the same” — the vertical v1 rail sat inside the fade zone).
            Level,
        }
        #[derive(PartialEq)]
        enum TransZone {
            EdgeLeft,
            EdgeRight,
            Joint,
        }
        // the transition grab takes priority over clip zones: its edges lie INSIDE clip bodies
        let transition_at = |app: &KtApp, pos: egui::Pos2| -> Option<(TransitionId, TransZone)> {
            let p = app.model.project();
            for t in &p.transitions {
                // fade anchor (R.25) — its only live end
                let Some(a) = t.a.or(t.b).and_then(|id| p.clips.iter().find(|c| c.id == id)) else {
                    continue;
                };
                let top = lane_top(a.kind, a.lane);
                let lane_h = lane_h_at(a.kind, a.lane);
                if pos.y < top || pos.y > top + lane_h - 2.0 {
                    continue;
                }
                // the top strip of the “corner” fade (item 12) belongs to the clip handle:
                // the window edge is clamped to a minimum, and the corner removes the fade
                // entirely — otherwise there would be nothing to drag it back with.
                // Below the strip the window is caught as before (R.25 gesture, test 157).
                if t.is_fade()
                    && (t.in_offset.0 == 0 || t.out_offset.0 == 0)
                    && pos.y <= top + top_strip_h(lane_h)
                {
                    continue;
                }
                // at an OVERLAPPING transition (R.51) there are no drag zones: the window is the overlap zone itself
                // and follows the clip trims (a second handle subsystem is not added —
                // Kdenlive lesson, F-TL-010).
                if t.overlap_pair(p).is_some() {
                    continue;
                }
                let Some(joint) = t.joint(p).map(|j| j.0) else { continue };
                let xj = tick_to_x(joint);
                if (pos.x - xj).abs() <= EDGE_GRAB_PX {
                    return Some((t.id, TransZone::Joint));
                }
                if (pos.x - tick_to_x(joint - t.in_offset.0)).abs() <= EDGE_GRAB_PX {
                    return Some((t.id, TransZone::EdgeLeft));
                }
                if (pos.x - tick_to_x(joint + t.out_offset.0)).abs() <= EDGE_GRAB_PX {
                    return Some((t.id, TransZone::EdgeRight));
                }
            }
            None
        };
        // The transition window as a whole is for the CONTEXT MENU (R.49, item 361).
        // `transition_at` above catches only the handles and the seam (that is about the GESTURE,
        // ±6 px), while the right click targets what is VISIBLE: the window is drawn
        // on top of neighbouring clips, so the menu also comes from it. Rule 17:
        // “the transition wins the pixel over the clip” — as already in hover and press.
        let transition_body_at = |app: &KtApp, pos: egui::Pos2| -> Option<TransitionId> {
            let p = app.model.project();
            for t in &p.transitions {
                let Some(a) = t.a.or(t.b).and_then(|id| p.clips.iter().find(|c| c.id == id)) else {
                    continue;
                };
                // window of the overlapping transition (R.51) — the overlap zone
                let Some(w) = t.window(p) else { continue };
                let top = lane_top(a.kind, a.lane);
                let lane_h = lane_h_at(a.kind, a.lane);
                let r = transition_rect(tick_to_x(w.start.0), tick_to_x(w.end.0), top, lane_h);
                if r.contains(pos) {
                    return Some(t.id);
                }
            }
            None
        };
        // hit order is the reverse of the draw order: first what lies on TOP
        // (a later clip when overlapping, item 177), otherwise a click would select not
        // the clip that is visible
        let order = draw_order.clone();
        let clip_at = move |app: &KtApp, pos: egui::Pos2| -> Option<(ClipId, ClipZone)> {
            for c in order.iter().rev().filter_map(|&i| app.model.project().clips.get(i)) {
                let top = lane_top(c.kind, c.lane);
                let lane_h = lane_h_at(c.kind, c.lane);
                if pos.y < top || pos.y > top + lane_h - 2.0 {
                    continue;
                }
                let x0 = tick_to_x(c.start.0);
                let x1 = tick_to_x(c.start.0 + c.duration.0);
                if pos.x < x0 - 2.0 || pos.x > x1 + 2.0 {
                    continue;
                }
                // on narrow clips the trim zones shrink — the middle is always
                // Body (item 17 round 6, test 60: a clip <14px could not be moved)
                let grab = edge_grab_for_width(x1 - x0);
                // The top strip of an event — Vegas gestures (item 12): fade handles
                // at the corners and the level line. They overlap the trim zones, but
                // only in the top pixels — the body and the edges below are free.
                let strip = top_strip_h(lane_h);
                let p = app.model.project();
                let fade_l = corner_fade_len(p, c.id, true).unwrap_or(0);
                let fade_r = corner_fade_len(p, c.id, false).unwrap_or(0);
                let hx_l =
                    tick_to_x(fade_handle_tick(c.start.0, c.start.0 + c.duration.0, fade_l, true));
                let hx_r =
                    tick_to_x(fade_handle_tick(c.start.0, c.start.0 + c.duration.0, fade_r, false));
                let in_strip = pos.y <= top + strip;
                let ly = level_y(clip_level(c), top, lane_h);
                let fgrab = fade_grab_for_width(x1 - x0);
                let zone = if in_strip && (pos.x - hx_l).abs() <= fgrab {
                    ClipZone::FadeLeft
                } else if in_strip && (pos.x - hx_r).abs() <= fgrab {
                    ClipZone::FadeRight
                } else if (pos.y - ly).abs() <= LEVEL_GRAB_PX {
                    ClipZone::Level
                } else if (pos.x - x0).abs() <= grab {
                    ClipZone::TrimStart
                } else if (pos.x - x1).abs() <= grab {
                    ClipZone::TrimEnd
                } else {
                    ClipZone::Body
                };
                return Some((c.id, zone));
            }
            None
        };

        // cursor icon + CONTEXT HINT of the zone (item 1 round 9: the timeline had
        // not a single hint — and this is the largest area of the window)
        let mut hover_clip: Option<ClipId> = None;
        if self.drag.is_none()
            && let Some(pos) = hover
        {
            let mut zone_hint: Option<&str> = None;
            let mut hint_owned: Option<String> = None;
            if marker_at(self, pos).is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                zone_hint = Some(tr(k::tl::hint::MARKER));
            } else if region_edge_at(self, pos).is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                zone_hint = Some(tr(k::tl::hint::REGION_EDGE));
            } else if pos.y <= rect.top() + RULER_H {
                zone_hint = Some(tr(k::tl::hint::RULER));
            } else if pos.x < rect.left() {
                // full track name — in the hint (the label may be truncated)
                if let Some((k, l)) = geom.key_at_y(pos.y) {
                    let name = self.row_label(k, l);
                    hint_owned = Some(crate::tf!(k::tl::hint::LANE_NAMED, name));
                } else {
                    zone_hint = Some(tr(k::tl::hint::LANE));
                }
            } else if let Some((_, tz)) = transition_at(self, pos) {
                ui.ctx().set_cursor_icon(match tz {
                    TransZone::Joint => egui::CursorIcon::Grab,
                    _ => egui::CursorIcon::ResizeHorizontal,
                });
                zone_hint = Some(tr(k::tl::hint::TRANSITION));
            } else {
                // event under the pointer: its fade handles are drawn
                // (R.40 item 12) — a gesture that is not visible does not exist
                hover_clip = clip_at(self, pos).map(|(id, _)| id);
                match clip_at(self, pos) {
                    Some((_, ClipZone::TrimStart)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        zone_hint = Some(tr(k::tl::hint::TRIM_START));
                    }
                    Some((_, ClipZone::TrimEnd)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        zone_hint = Some(tr(k::tl::hint::TRIM_END));
                    }
                    Some((_, ClipZone::FadeLeft)) | Some((_, ClipZone::FadeRight)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        zone_hint = Some(tr(k::tl::hint::FADE_HANDLE));
                    }
                    Some((id, ClipZone::Level)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                        let audio =
                            self.model.clip(id).map(|c| c.kind == ClipKind::Audio).unwrap_or(false);
                        zone_hint = Some(if audio {
                            tr(k::tl::hint::VOLUME)
                        } else {
                            tr(k::tl::hint::LEVEL)
                        });
                    }
                    Some((id, ClipZone::Body)) => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                        // WHAT A PIN MEANS — is said right where it is
                        // drawn (R.59, stage 3): “how many of the declared handles
                        // this release changed”.
                        let ov = self
                            .model
                            .clip(id)
                            .filter(|c| !c.overrides.is_empty())
                            .map(|c| self.instance_override_counts(c));
                        match ov {
                            Some((n, total)) => {
                                hint_owned = Some(crate::tf!(k::pubp::OVERRIDDEN, n, total));
                            }
                            None => zone_hint = Some(tr(k::tl::hint::CLIP)),
                        }
                    }
                    None => {
                        // “empty space” was also said ABOVE the morph/camera block:
                        // `clip_at` on the service row always gives None, and
                        // there was no separate branch. A hint that lies is
                        // worse than none (checklist rule 10).
                        zone_hint = Some(
                            if morph_edge_at(self, pos).is_some()
                                || camera_edge_at(self, pos).is_some()
                            {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                                tr(k::tl::hint::MORPH_EDGE)
                            } else if area_at(self, pos).is_some() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                                tr(k::tl::hint::AREA)
                            } else if morpher_at(self, pos).is_some() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                                tr(k::tl::hint::MORPHER)
                            } else if camera_at(self, pos).is_some() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                                tr(k::tl::hint::CAMERA)
                            } else if let Some((lk @ (LaneKind::Video | LaneKind::Audio), l)) =
                                geom.key_at_y(pos.y)
                            {
                                // on a track the hint NAMES the track and speaks about its menu:
                                // since R.40 a right click here gives the row menu,
                                // not just the insert.
                                hint_owned =
                                    Some(crate::tf!(k::tl::hint::LANE_FREE, self.row_label(lk, l)));
                                ""
                            } else if geom.morph_lane_at_y(pos.y).is_some() {
                                // An EMPTY service row says WHAT to put on it (R.40 item 5, verbatim:
                                // “I do not understand how to add something to the camera track —
                                // what is it for?????”). Previously the empty-space hint here
                                // was about the region and the selection — things that never
                                // happen on this kind of row,
                                // so it was misleading.
                                tr(k::tl::hint::MORPH_LANE)
                            } else if geom.camera_lane_at_y(pos.y).is_some() {
                                tr(k::tl::hint::CAMERA_LANE)
                            } else {
                                tr(k::tl::hint::EMPTY)
                            },
                        );
                    }
                }
            }
            if let Some(h) = hint_owned {
                self.hover_hint = Some(h);
            } else if let Some(h) = zone_hint {
                self.hover_hint = Some(h.to_string());
            }
        }

        // right-click: capturing the context menu target
        if response.secondary_clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            // gesture token for EVERY menu opening: edits of a numeric field
            // inside one menu collapse into a single history step, and
            // the next opening starts a new one
            self.gesture_counter += 1;
            // the row under the click is needed for empty space too: “add
            // track” from the menu is placed RIGHT BELOW it (item 5 round 16)
            self.ctx_row = geom.key_at_y(pos.y);
            // and the TIME of the click: “Paste” places the block here (R.56, item 450)
            self.ctx_at = Some(kt_time::Tick(geom.x_to_tick(pos.x).max(self.time_floor())));
            // targets cancel each other: only one menu is on screen (item 363)
            self.ctx_panel = None;
            self.ctx_target = Some(if let Some(m) = marker_at(self, pos) {
                CtxTarget::Marker(m)
            } else if let Some(mid) = morpher_at(self, pos) {
                // service blocks are checked BEFORE the region boundary:
                // the region line runs the full height, and a block whose edge
                // falls on it could otherwise not be grabbed at all
                // RMB on a MEMBER of the selection does not break the group — as with clips
                if !self.selected_morph.contains(&mid) {
                    self.select_morph_only(mid);
                }
                CtxTarget::Morpher(mid)
            } else if let Some(cid) = camera_at(self, pos) {
                if !self.selected_camera.contains(&cid) {
                    self.select_camera_only(cid);
                }
                CtxTarget::Camera(cid)
            } else if let Some(start_edge) = region_edge_at(self, pos) {
                let r = self.model.project().region.unwrap();
                CtxTarget::RegionEdge(if start_edge { r.start.0 } else { r.end.0 })
            } else if let Some(tid) = transition_body_at(self, pos) {
                CtxTarget::Transition(tid)
            } else if let Some((id, _)) = clip_at(self, pos) {
                // RMB on a member of the selection does not break the group: the menu is about it
                if !self.is_selected(id) {
                    self.select_only(id);
                }
                self.selected_morph.clear();
                self.selected_camera.clear();
                // A NESTED REEL is its own entity (R.50): it has its own verbs, and
                // “there are no dead ends” asks about them along its own row
                // A NESTED REEL and a GENERATOR are their own entities (R.50, R.53): each
                // has a verb that an ordinary event does not have
                let media = self.model.clip(id).map(|c| c.media);
                if self.is_nest(id) {
                    CtxTarget::Nest(id)
                } else if media.is_some_and(|m| self.model.project().is_gen_media(m)) {
                    CtxTarget::Generator(id)
                } else {
                    CtxTarget::Clip(id)
                }
            } else if let Some((i, _)) = area_at(self, pos) {
                CtxTarget::Area(i)
            } else if let Some((LaneKind::Video | LaneKind::Audio, _)) = geom.key_at_y(pos.y) {
                // RMB on the clip track ITSELF (R.40, rule C0): previously
                // this was “empty space”, and a row could be renamed or deleted only from
                // its 34-pixel header. The empty-space items did not go anywhere —
                // they are lower, past the line.
                let (kind, lane) = match geom.key_at_y(pos.y) {
                    Some((LaneKind::Audio, l)) => (ClipKind::Audio, l),
                    _ => (ClipKind::Video, geom.key_at_y(pos.y).map(|k| k.1).unwrap_or(0)),
                };
                CtxTarget::Lane { kind, lane }
            } else if let Some(lane) = geom.morph_lane_at_y(pos.y) {
                // RMB on the morph track ITSELF (R.40 item 2). Before this round
                // the item “add morpher here” existed only in the menu of the
                // row HEADER — a 34 px strip — and a click on the track gave the
                // empty-space menu. The item existed, but a person could
                // not reach it.
                CtxTarget::MorphLane(lane)
            } else if let Some(lane) = geom.camera_lane_at_y(pos.y) {
                CtxTarget::CameraLane(lane)
            } else {
                CtxTarget::Empty
            });
        }

        // ---- press ------------------------------------------------------------
        // Zones are determined by the PRESS point (press_origin), not by the current
        // pointer: drag_started arrives after a shift of a few px, and
        // the pointer had time to slide out of the narrow trim zone — the trim turned
        // into moving the clip (item 28 “I cannot change the length of an event”).
        let press_pos = ui.input(|i| i.pointer.press_origin());
        if (response.drag_started() || response.clicked())
            && let Some(pos) = press_pos.or(pointer)
        {
            if let Some(start) = marker_end_at(self, pos) {
                if response.drag_started() {
                    let end = self
                        .model
                        .project()
                        .markers
                        .iter()
                        .find(|m| m.t.0 == start)
                        .map(|m| m.end().0)
                        .unwrap_or(start);
                    self.drag = Some(Drag::MarkerEdge { start, left: false, cur: end });
                }
            } else if let Some(start) = marker_start_at(self, pos) {
                if response.drag_started() {
                    self.drag = Some(Drag::MarkerEdge { start, left: true, cur: start });
                }
            } else if let Some(m) = marker_at(self, pos) {
                if response.drag_started() {
                    self.drag = Some(Drag::Marker { orig: m, cur: m });
                }
            } else if let Some(start_edge) = region_edge_at(self, pos) {
                if response.drag_started() {
                    self.drag = Some(Drag::RegionEdge { start_edge });
                }
            } else if pos.y <= rect.top() + RULER_H {
                // ruler: grabbing the playhead triangle = scrub; click =
                // placing the cursor (snap to markers, item 29); drag =
                // selecting a region (the owner’s decision, item 8).
                // The hit zone follows the DRAWN triangle (the display clock)
                let head_x = tick_to_x(self.player.video_playhead().0);
                if response.double_clicked()
                    && let Some(start) = pick_range_marker(
                        &self.model.project().markers,
                        x_to_tick(pos.x),
                        (EDGE_GRAB_PX as f64 * tpp) as i64,
                    )
                {
                    // “double-clicking a marked_region selects it entirely”
                    // (R.33 item 8): the region is taken from the marker boundaries plus its event —
                    // so both rendering by name and operations on its contents work at once
                    self.region_from_marked_range(start);
                } else if response.drag_started() && (pos.x - head_x).abs() <= 8.0 {
                    self.drag = Some(Drag::Scrub);
                } else if response.drag_started() {
                    // snap also when CREATING a region (item 13 round 6, test 59)
                    let anchor = self.snap(x_to_tick(pos.x).max(CURSOR_FLOOR), None);
                    self.drag = Some(Drag::Region { anchor });
                } else {
                    let t = self.snap(x_to_tick(pos.x).max(CURSOR_FLOOR), None);
                    self.player.seek(Tick(t.max(CURSOR_FLOOR)));
                }
            } else if let Some((inst, left)) =
                band_edge_at(self, pos).filter(|_| !response.double_clicked())
                && response.drag_started()
            {
                // The EDGE of the window of an expanded strip comes BEFORE all other targets in its rows:
                // the window edge line is drawn over the events, and it is the one aimed at,
                // not what is under it.
                let cur = self
                    .model
                    .project()
                    .clips
                    .iter()
                    .find(|c| c.id == inst)
                    .map(|c| if left { c.start.0 } else { (c.start + c.duration).0 });
                if let Some(cur) = cur {
                    self.select_only(inst);
                    self.drag = Some(Drag::NestWindow { inst, left, cur });
                }
            } else if let Some((cid, left)) =
                camera_edge_at(self, pos).filter(|_| !response.double_clicked())
                && response.drag_started()
            {
                self.select_camera_only(cid);
                if let Some(orig) = self.model.project().cameras.iter().find(|c| c.id == cid) {
                    let cur = if left { orig.start.0 } else { (orig.start + orig.duration).0 };
                    self.drag = Some(Drag::CameraEdge { id: cid, left, cur });
                }
            } else if let Some(cid) = camera_at(self, pos) {
                self.select_camera_only(cid);
                if response.double_clicked() {
                    self.open_camera_editor(cid);
                } else if response.drag_started()
                    && let Some(orig) = self.model.project().cameras.iter().find(|c| c.id == cid)
                {
                    self.drag = Some(Drag::Camera {
                        id: cid,
                        grab_dt: x_to_tick(pos.x) - orig.start.0,
                        cur_start: orig.start.0,
                        cur_lane: orig.lane,
                        moved: false,
                    });
                }
            } else if let Some((mid, left)) = morph_edge_at(self, pos).filter(|_| {
                // double-click on the edge is still “open window”
                !response.double_clicked()
            }) && response.drag_started()
            {
                self.select_morph_only(mid);
                if let Some(orig) = self.model.project().morphers.iter().find(|m| m.id == mid) {
                    let cur = if left { orig.start.0 } else { (orig.start + orig.duration).0 };
                    self.drag = Some(Drag::MorphEdge { id: mid, left, cur });
                }
            } else if let Some(mid) = morpher_at(self, pos) {
                self.select_morph_only(mid);
                if response.double_clicked() {
                    self.open_morph_editor(mid);
                } else if response.drag_started()
                    && let Some(orig) = self.model.project().morphers.iter().find(|m| m.id == mid)
                {
                    self.drag = Some(Drag::Morph {
                        id: mid,
                        grab_dt: x_to_tick(pos.x) - orig.start.0,
                        cur_start: orig.start.0,
                        cur_lane: orig.lane,
                        moved: false,
                    });
                }
            } else if response.double_clicked()
                && let Some(tid) = transition_body_at(self, pos)
            {
                // double-click on the transition window = its properties (parity with
                // the “double-click on an event = FX window” method, item 17 round 28).
                // The hit is on the BODY of the window: an overlapping transition
                // (R.51) has no drag zones at all, but it does have a window.
                self.open_transition_editor(tid);
            } else if let Some((tid, tz)) = transition_at(self, pos) {
                if response.double_clicked() {
                    // double-click on the handles/seam = the same properties window
                    self.open_transition_editor(tid);
                } else if response.drag_started() {
                    match tz {
                        TransZone::Joint => {
                            let at = {
                                let p = self.model.project();
                                p.transitions
                                    .iter()
                                    .find(|t| t.id == tid)
                                    .and_then(|t| t.joint(p))
                                    .map(|t| t.0)
                            };
                            if let Some(at) = at {
                                self.drag =
                                    Some(Drag::TransitionBody { id: tid, orig_at: at, cur_at: at });
                            }
                        }
                        TransZone::EdgeLeft | TransZone::EdgeRight => {
                            self.gesture_counter += 1;
                            self.drag = Some(Drag::TransitionEdge {
                                id: tid,
                                left: tz == TransZone::EdgeLeft,
                                gesture: self.gesture_counter,
                            });
                        }
                    }
                } else if response.clicked() {
                    let t = self.snap(x_to_tick(pos.x).max(CURSOR_FLOOR), None);
                    self.player.seek(Tick(t.max(CURSOR_FLOOR)));
                }
            } else if let Some((id, zone)) = clip_at(self, pos) {
                self.selected_morph.clear();
                self.selected_camera.clear();
                if ctrl_held && response.clicked() {
                    // Ctrl+click builds up the selection: the cursor stays in
                    // place, otherwise building the group would drag the playhead along
                    self.toggle_selected(id);
                } else if response.clicked() {
                    self.select_only(id);
                    // a click on an EVENT also places the cursor (v1, item 7)
                    let t = self.snap(x_to_tick(pos.x).max(CURSOR_FLOOR), None);
                    self.player.seek(Tick(t.max(CURSOR_FLOOR)));
                } else if !self.is_selected(id) {
                    // dragging a foreign event takes it alone; dragging a member
                    // of the group moves the whole group (the selection is not broken)
                    self.select_only(id);
                }
                if response.double_clicked() {
                    // double-click opens FX — exactly what the clip hint has promised since R.18
                    // (before R.33 it was misleading: a click set the region, and FX were hidden
                    // in the RMB menu). The region by event stayed on Shift+double-click
                    // and in the menu.
                    if shift_held {
                        self.region_from_event(id);
                    } else if zone == ClipZone::Level {
                        // …but ON THE LEVEL LINE it resets the level (R.49,
                        // item 362). The hint promised this from the start
                        // (“double-click — 0 dB”), and all this time a double-click
                        // on the line opened the FX window. There is one line for both
                        // kinds (item 176): for audio it is 0 dB, for video — full
                        // opacity.
                        if let Some(c) = self.model.clip(id).cloned() {
                            match c.kind {
                                ClipKind::Audio => {
                                    self.apply(Command::SetGain { id, gain_db: 0.0 })
                                }
                                ClipKind::Video => {
                                    self.apply(Command::SetOpacity { id, opacity: 1.0 })
                                }
                            };
                        }
                    } else if self.is_nest(id) {
                        // A NESTED REEL EXPANDS IN PLACE (R.55, C5) — this is
                        // the default mode, at the owner’s direct request:
                        // we do not leave the root, the whole program plays, and
                        // the composition can be adjusted while hearing the result
                        // in context. The FULL entry (view of the reel only)
                        // remains the menu item “Enter reel”.
                        //
                        // The gesture works REPEATEDLY (rule 14): a second
                        // double-click collapses the expansion.
                        self.expand_in_place(id);
                    } else {
                        self.open_fx_editor(id);
                    }
                } else if response.drag_started() {
                    match zone {
                        ClipZone::FadeLeft | ClipZone::FadeRight => {
                            let left = zone == ClipZone::FadeLeft;
                            let len = corner_fade_len(self.model.project(), id, left).unwrap_or(0);
                            self.drag = Some(Drag::Fade { id, left, len, moved: false });
                        }
                        ClipZone::Level => {
                            self.drag = Some(Drag::Level { id, moved: false });
                        }
                        zone => {
                            if let Some(orig) = self.model.clip(id).cloned() {
                                let mode = match zone {
                                    ClipZone::TrimStart => DragMode::TrimStart,
                                    ClipZone::TrimEnd => DragMode::TrimEnd,
                                    _ => DragMode::Move,
                                };
                                // the group travels together with the anchor: other selected items
                                // end up here (their own link partners move by the MoveClip
                                // command itself).
                                let extra: Vec<(ClipId, i64)> = if self.selected.len() > 1 {
                                    let group = orig.link_group;
                                    self.model
                                        .project()
                                        .clips
                                        .iter()
                                        .filter(|c| {
                                            self.selected.contains(&c.id)
                                                && c.id != id
                                                && !(group.is_some() && c.link_group == group)
                                        })
                                        .map(|c| (c.id, c.start.0))
                                        .collect()
                                } else {
                                    Vec::new()
                                };
                                self.drag = Some(Drag::Clip(DragState {
                                    id,
                                    mode,
                                    extra,
                                    grab_dt: x_to_tick(pos.x) - orig.start.0,
                                    cur_start: orig.start.0,
                                    cur_end: orig.start.0 + orig.duration.0,
                                    cur_lane: orig.lane,
                                    orig,
                                    moved: false,
                                }));
                            }
                        }
                    }
                }
            } else if response.clicked() && edge_heat(pos.x) != 0.0 {
                // a click in the jump zone: the cursor is PLACED (by position before the shift),
                // and the view jumps — “both at once” (the owner’s decision, item 10)
                let t = self.snap(x_to_tick(pos.x).max(CURSOR_FLOOR), None);
                self.player.seek(Tick(t.max(CURSOR_FLOOR)));
                let heat = edge_heat(pos.x);
                let vis = rect.width() as f64 * tpp;
                self.view_start = (self.view_start
                    + heat.signum() as f64 * (heat.abs() as f64).powi(2) * vis * 0.6)
                    .max(self.view_floor(tpp));
            } else if response.drag_started() {
                if ctrl_held || self.sel_tool {
                    // selection frame (R.33 item 7): Ctrl+drag always, and in
                    // the tool mode — ordinary drag. It starts from an EMPTY
                    // spot: a drag on an event stays a move, and Ctrl in it still
                    // removes the magnet (item 2 round 11)
                    self.drag = Some(Drag::Rubber { anchor: pos, add: ctrl_held && self.sel_tool });
                } else {
                    // empty spot: region selection (RegionSelect port)
                    // snap also when creating a region from an empty spot (item 13 round 6)
                    let anchor = self.snap(x_to_tick(pos.x).max(CURSOR_FLOOR), None);
                    self.drag = Some(Drag::Region { anchor });
                }
            } else {
                self.selected.clear();
                let t = self.snap(x_to_tick(pos.x).max(CURSOR_FLOOR), None);
                self.player.seek(Tick(t.max(CURSOR_FLOOR)));
            }
        }

        // ---- drag move ----------------------------------------------------------
        if response.dragged()
            && let Some(pos) = pointer
        {
            match self.drag.take() {
                Some(Drag::LaneRow { from, cur }) => {
                    // handled by the gutter area; here we just hold it
                    self.drag = Some(Drag::LaneRow { from, cur });
                }
                Some(Drag::LaneHeight { row, cur }) => {
                    // also the gutter's gesture — held so as not to lose it
                    self.drag = Some(Drag::LaneHeight { row, cur });
                }
                Some(Drag::Rubber { anchor, add }) => {
                    // the frame is drawn in the clip layer by the current pointer,
                    // the selection is taken on release
                    self.drag = Some(Drag::Rubber { anchor, add });
                }
                Some(Drag::Clip(mut d)) => {
                    d.moved = true;
                    match d.mode {
                        DragMode::Move => {
                            let raw = x_to_tick(pos.x) - d.grab_dt;
                            let dur = d.orig.duration.0;
                            let (snapped, line) = if self.snap_off {
                                (raw, None)
                            } else {
                                let targets = self.snap_targets(Some(d.id));
                                snap_move(raw, dur, &targets, snap_threshold(self.ticks_per_px))
                            };
                            d.cur_start = snapped.max(self.time_floor());
                            // a clamp at zero breaks the magnetism — the line is suppressed
                            self.snap_line = if d.cur_start == snapped { line } else { None };
                            d.cur_end = d.cur_start + dur;
                            if d.extra.is_empty() {
                                // rows are mixed (item 6): we snap to the
                                // nearest row of ITS OWN kind (test 50)
                                // division does not account for rows of different heights
                                let row = geom.row_at_y_unbounded(pos.y).max(0);
                                d.cur_lane = geom.nearest_lane(d.orig.kind, row);
                            } else {
                                // the group moves only in time: its events have
                                // different kinds and tracks, so the “nearest row”
                                // is not defined for a block
                                d.cur_lane = d.orig.lane;
                                let moving = self.moving_block_ids(&d);
                                let plan = plan_move_block(
                                    self.model.project(),
                                    &moving,
                                    d.cur_start - d.orig.start.0,
                                );
                                d.cur_start = d.orig.start.0 + plan.delta;
                                d.cur_end = d.cur_start + dur;
                                if let Some((span, kind, lane)) = plan.blocker {
                                    self.snap_line = None;
                                    self.blocked = Some(Blocked { span, kind, lane });
                                }
                            }
                            // multi-overlap/absorption — the clamp is still in the preview
                            // (decision 10, R.24); a single partial overlap
                            // stays as it is (release will make a crossfade)
                            if let (true, MovePlan::Clamped { start, blocker }) = (
                                d.extra.is_empty(),
                                plan_move(self.model.project(), d.id, d.cur_start, d.cur_lane),
                            ) {
                                let mut blocker = blocker;
                                if d.cur_lane != d.orig.lane {
                                    // a vertical move through an occupied spot
                                    // is not pushed through — it returns to the row
                                    d.cur_lane = d.orig.lane;
                                    if let MovePlan::Clamped { start, blocker: b } = plan_move(
                                        self.model.project(),
                                        d.id,
                                        d.cur_start,
                                        d.cur_lane,
                                    ) {
                                        d.cur_start = start;
                                        blocker = b;
                                    }
                                } else {
                                    d.cur_start = start;
                                }
                                d.cur_end = d.cur_start + dur;
                                self.snap_line = None;
                                // an impossible action is an ERROR, not a
                                // faded hint (item 5): the obstacle is highlighted
                                // and, if it lies outside the view, the hint says
                                // where to look.
                                self.blocked = Some(Blocked {
                                    span: blocker,
                                    kind: d.orig.kind,
                                    lane: d.cur_lane,
                                });
                            }
                        }
                        DragMode::TrimStart => {
                            let raw = x_to_tick(pos.x);
                            let snapped = self.snap(raw, Some(d.id));
                            let max_start = d.cur_end - MIN_CLIP_DURATION.0;
                            let min_start = d.orig.start.0 - d.orig.source_in.0;
                            d.cur_start = snapped.clamp(min_start, max_start);
                        }
                        DragMode::TrimEnd => {
                            let raw = x_to_tick(pos.x);
                            let snapped = self.snap(raw, Some(d.id));
                            let want = snapped.max(d.cur_start + MIN_CLIP_DURATION.0);
                            // END OF MATERIAL is the same stop as a foreign event
                            // (R.63 item 2): the limit is computed by the model
                            // (one formula for the gesture preview and for the command),
                            // and the preview must stop VISIBLY, otherwise the
                            // edge slides while the command trims it — “nothing
                            // happens”.
                            let limit = {
                                let p = self.model.project();
                                p.clips.iter().find(|c| c.id == d.id).map(|c| {
                                    let len = p.media_duration(c.media);
                                    kt_model::trim_max_end_in(c, len)
                                        .0
                                        .max((c.start + c.duration).0)
                                })
                            };
                            d.cur_end = match limit {
                                Some(l) if want > l => {
                                    self.blocked_material = Some(l);
                                    l
                                }
                                _ => want,
                            };
                        }
                    }
                    self.drag = Some(Drag::Clip(d));
                }
                Some(Drag::Scrub) => {
                    let t = self.snap(x_to_tick(pos.x).max(CURSOR_FLOOR), None);
                    self.player.seek(Tick(t.max(CURSOR_FLOOR)));
                    self.drag = Some(Drag::Scrub);
                }
                Some(Drag::Region { anchor }) => {
                    let cur = self.snap(x_to_tick(pos.x).max(CURSOR_FLOOR), None);
                    self.model.set_region(Region::normalized(Tick(anchor), Tick(cur)));
                    self.drag = Some(Drag::Region { anchor });
                }
                Some(Drag::RegionEdge { start_edge }) => {
                    if let Some(r) = self.model.project().region {
                        let raw = x_to_tick(pos.x).max(CURSOR_FLOOR);
                        let snapped = self.snap(raw, None);
                        let newr = if start_edge {
                            Region::normalized(Tick(snapped), r.end)
                        } else {
                            Region::normalized(r.start, Tick(snapped))
                        };
                        self.model.set_region(newr.or(Some(r)));
                    }
                    self.drag = Some(Drag::RegionEdge { start_edge });
                }
                Some(Drag::Marker { orig, .. }) => {
                    let raw = x_to_tick(pos.x).max(CURSOR_FLOOR);
                    let cur = self.snap(raw, None);
                    self.drag = Some(Drag::Marker { orig, cur });
                }
                Some(Drag::MarkerEdge { start, left, .. }) => {
                    let raw = x_to_tick(pos.x).max(CURSOR_FLOOR);
                    let cur = self.snap(raw, None).max(if left { 0 } else { start });
                    self.drag = Some(Drag::MarkerEdge { start, left, cur });
                }
                Some(Drag::TransitionEdge { id, left, gesture }) => {
                    // live resize of the window: SetTransition with coalescing (one undo)
                    let cmd = {
                        let p = self.model.project();
                        p.transitions.iter().find(|t| t.id == id).cloned().and_then(|mut t| {
                            let joint = t.joint(p)?.0;
                            let was = (t.in_offset, t.out_offset);
                            let body = |side: Option<ClipId>| {
                                // for a fade the empty side is not bounded by the body
                                side.map_or(i64::MAX, |id| {
                                    p.clips.iter().find(|c| c.id == id).map_or(0, |c| c.duration.0)
                                })
                            };
                            let raw = x_to_tick(pos.x);
                            if left {
                                t.in_offset = Tick((joint - raw).clamp(0, body(t.a)));
                            } else {
                                t.out_offset = Tick((raw - joint).clamp(0, body(t.b)));
                            }
                            // a frame without mouse movement sends no command: otherwise
                            // the model revision grows every frame of the drag and needlessly
                            // resets the caches keyed by it
                            ((t.in_offset, t.out_offset) != was
                                && t.in_offset + t.out_offset >= MIN_TRANSITION_DURATION)
                                .then_some(Command::SetTransition(t))
                        })
                    };
                    if let Some(cmd) = cmd {
                        self.apply_coalesced(cmd, gesture);
                    }
                    self.drag = Some(Drag::TransitionEdge { id, left, gesture });
                }
                Some(Drag::TransitionBody { id, orig_at, .. }) => {
                    // roll of a seam: a risk preview, trims on release
                    let cur_at = {
                        let p = self.model.project();
                        p.transitions
                            .iter()
                            .find(|t| t.id == id)
                            .and_then(|t| {
                                let a = p.clips.iter().find(|c| Some(c.id) == t.a)?;
                                let b = p.clips.iter().find(|c| Some(c.id) == t.b)?;
                                Some(x_to_tick(pos.x).clamp(
                                    a.start.0 + MIN_CLIP_DURATION.0,
                                    b.start.0 + b.duration.0 - MIN_CLIP_DURATION.0,
                                ))
                            })
                            .unwrap_or(orig_at)
                    };
                    self.drag = Some(Drag::TransitionBody { id, orig_at, cur_at });
                }
                Some(Drag::CameraEdge { id, left, .. }) => {
                    let cur = x_to_tick(pos.x).max(self.time_floor());
                    self.drag = Some(Drag::CameraEdge { id, left, cur });
                }
                Some(Drag::NestWindow { inst, left, .. }) => {
                    // the snap is the same as for trims: the window must sit on
                    // the same spots as any other edge
                    let cur = self.snap(x_to_tick(pos.x).max(self.time_floor()), Some(inst));
                    self.drag = Some(Drag::NestWindow { inst, left, cur });
                }
                Some(Drag::MorphEdge { id, left, .. }) => {
                    let cur = x_to_tick(pos.x).max(self.time_floor());
                    self.drag = Some(Drag::MorphEdge { id, left, cur });
                }
                Some(Drag::Morph { id, grab_dt, .. }) => {
                    let raw = x_to_tick(pos.x) - grab_dt;
                    let cur_start = self.snap(raw, None).max(self.time_floor());
                    // the row under the cursor — by the same rule as for a clip
                    let row = geom.row_at_y_unbounded(pos.y).max(0);
                    let cur_lane = geom.nearest_lane_of(LaneKind::Morph, row);
                    self.drag = Some(Drag::Morph { id, grab_dt, cur_start, cur_lane, moved: true });
                }
                Some(Drag::Camera { id, grab_dt, .. }) => {
                    let raw = x_to_tick(pos.x) - grab_dt;
                    let cur_start = self.snap(raw, None).max(self.time_floor());
                    let row = geom.row_at_y_unbounded(pos.y).max(0);
                    let cur_lane = geom.nearest_lane_of(LaneKind::Camera, row);
                    self.drag =
                        Some(Drag::Camera { id, grab_dt, cur_start, cur_lane, moved: true });
                }
                Some(Drag::Fade { id, left, .. }) => {
                    // length = distance from the event edge to the cursor,
                    // clamped by the body: “as far as it was pulled out — that much fade”
                    if let Some(c) = self.model.clip(id) {
                        let (s0, e0) = (c.start.0, c.start.0 + c.duration.0);
                        let raw = x_to_tick(pos.x).clamp(s0, e0);
                        let mut len = if left { raw - s0 } else { e0 - raw };
                        // a dead zone right at the corner: the handle “sticks” to the
                        // edge, and a fade pulled back disappears entirely,
                        // instead of leaving an invisible stub of a couple of ms
                        let edge_x = tick_to_x(if left { s0 } else { e0 });
                        if (pos.x - edge_x).abs() <= 3.0 {
                            len = 0;
                        }
                        self.drag = Some(Drag::Fade {
                            id,
                            left,
                            len: len.clamp(0, c.duration.0),
                            moved: true,
                        });
                    }
                }
                Some(Drag::Level { id, .. }) => {
                    if let Some(c) = self.model.clip(id) {
                        let (kind, top) = (c.kind, lane_top(c.kind, c.lane));
                        let lv = level_from_y(pos.y, top, lane_h_at(c.kind, c.lane));
                        // live preview without a history entry: for audio the level
                        // is the volume (the v1 rail range), for video —
                        // opacity
                        // The player plays what `player_project` gives
                        // (R.55, C1): at the root — the document, inside a reel —
                        // the reel ITSELF. In both cases the event the person
                        // drags lies at the ROOT of this project, so there is no longer
                        // any need to search for it through the tree.
                        let mut p = self.player_project();
                        if let Some(cm) = p.clips.iter_mut().find(|c| c.id == id) {
                            if kind == ClipKind::Audio {
                                cm.gain_db = level_to_gain(lv);
                            } else {
                                cm.opacity = lv;
                            }
                        }
                        self.player.set_project(p);
                        if kind == ClipKind::Audio {
                            self.live_gain = Some((id, level_to_gain(lv)));
                        } else {
                            self.live_opacity = Some((id, lv));
                        }
                    }
                    self.drag = Some(Drag::Level { id, moved: true });
                }
                None => {}
            }
        }

        // ---- release -------------------------------------------------------------
        if response.drag_stopped() {
            match self.drag.take() {
                // group: a common shift in one batch, no crossfades from an
                // overlap — the block moves as it is (R.33 item 7)
                Some(Drag::Clip(d)) if d.moved && !d.extra.is_empty() => {
                    let delta = d.cur_start - d.orig.start.0;
                    if delta != 0 {
                        let cmds: Vec<Command> = std::iter::once((d.id, d.orig.start.0))
                            .chain(d.extra.iter().copied())
                            .map(|(id, orig_start)| Command::MoveClip {
                                id,
                                // from the ORIGINAL starts: the command moves the whole
                                // link group, and a repeat on its member is a no-op
                                new_start: Tick(orig_start + delta),
                                new_lane: self.model.clip(id).map_or(d.orig.lane, |c| c.lane),
                            })
                            .collect();
                        self.apply_batch_restoring(cmds);
                    }
                }
                Some(Drag::Clip(d)) if d.moved => match d.mode {
                    DragMode::Move => {
                        // a single partial overlap = crossfade (R.24 gesture):
                        // the clips are butted up to C, the transition window = the former overlap —
                        // the audio is byte-identical to the old overlap (test 136)
                        match plan_move(self.model.project(), d.id, d.cur_start, d.cur_lane) {
                            MovePlan::Crossfade { outgoing, incoming, joint, in_off, out_off } => {
                                let existing = self
                                    .model
                                    .project()
                                    .transitions
                                    .iter()
                                    .find(|t| (t.a, t.b) == (Some(outgoing), Some(incoming)))
                                    .cloned();
                                let had = existing.is_some();
                                let ev = match existing {
                                    // a repeated overlap on a pair with a configured
                                    // transition moves ONLY the window: the view,
                                    // the curves and the audio stay (rule 20,
                                    // item 397)
                                    Some(mut t) => {
                                        t.in_offset = Tick(in_off);
                                        t.out_offset = Tick(out_off);
                                        t
                                    }
                                    None => TransitionEvent {
                                        id: TransitionId(self.next_transition_id()),
                                        a: Some(outgoing),
                                        b: Some(incoming),
                                        in_offset: Tick(in_off),
                                        out_offset: Tick(out_off),
                                        kind: TransitionKind::Crossfade,
                                        audio: Default::default(),
                                        auto: vec![],
                                        invert: false,
                                    },
                                };
                                let mut cmds = vec![
                                    Command::MoveClip {
                                        id: d.id,
                                        new_start: Tick(d.cur_start),
                                        new_lane: d.cur_lane,
                                    },
                                    Command::TrimEnd { id: outgoing, new_end: Tick(joint) },
                                    Command::TrimStart { id: incoming, new_start: Tick(joint) },
                                ];
                                cmds.push(if had {
                                    Command::SetTransition(ev)
                                } else {
                                    Command::AddTransition(ev)
                                });
                                // a clip could leave a DIFFERENT crossfade — its
                                // orphaned neighbour gets the material back
                                self.apply_batch_restoring(cmds);
                            }
                            _ => {
                                // the clips were pulled apart (or moved to another
                                // track) — the transition dies, and the material cut
                                // by the overlap returns (R.26)
                                self.apply_batch_restoring(vec![Command::MoveClip {
                                    id: d.id,
                                    new_start: Tick(d.cur_start),
                                    new_lane: d.cur_lane,
                                }]);
                            }
                        }
                    }
                    DragMode::TrimStart => {
                        self.apply(Command::TrimStart { id: d.id, new_start: Tick(d.cur_start) });
                    }
                    DragMode::TrimEnd => {
                        self.apply(Command::TrimEnd { id: d.id, new_end: Tick(d.cur_end) });
                    }
                },
                Some(Drag::Marker { orig, cur }) => {
                    if orig != cur {
                        self.apply(Command::MoveMarker { from: Tick(orig), to: Tick(cur) });
                    }
                }
                Some(Drag::MarkerEdge { start, left: false, cur }) => {
                    // duration = 0 is legitimate: collapsing the range to a point
                    self.apply(Command::SetMarkerDuration {
                        at: Tick(start),
                        duration: Tick((cur - start).max(0)),
                    });
                }
                Some(Drag::MarkerEdge { start, left: true, cur }) => {
                    // the end stays, the start moves: MoveMarker + new length
                    // in one history step (rule 20 — nothing grows back)
                    let end = self
                        .model
                        .project()
                        .markers
                        .iter()
                        .find(|m| m.t.0 == start)
                        .map(|m| m.end().0)
                        .unwrap_or(start);
                    let ns = cur.clamp(0, end);
                    if ns != start {
                        self.apply_batch(vec![
                            Command::MoveMarker { from: Tick(start), to: Tick(ns) },
                            Command::SetMarkerDuration { at: Tick(ns), duration: Tick(end - ns) },
                        ]);
                    }
                }
                Some(Drag::TransitionBody { id, orig_at, cur_at }) if cur_at != orig_at => {
                    // roll: the seam moves, the window moves with it (offsets do not change),
                    // the model reconcile clamps what has slid out
                    let pair = self
                        .model
                        .project()
                        .transitions
                        .iter()
                        .find(|t| t.id == id)
                        .map(|t| (t.a, t.b));
                    // a roll of a seam happens only for a real pair
                    if let Some((Some(a), Some(b))) = pair {
                        self.apply_batch(vec![
                            Command::TrimEnd { id: a, new_end: Tick(cur_at) },
                            Command::TrimStart { id: b, new_start: Tick(cur_at) },
                        ]);
                    }
                }
                Some(Drag::Fade { id, left, len, moved }) => {
                    if moved {
                        self.commit_fade_handle(id, left, len);
                    }
                }
                Some(Drag::Level { id, moved }) => {
                    if moved {
                        if let Some((oid, op)) = self.live_opacity.take_if(|(o, _)| *o == id) {
                            self.apply(Command::SetOpacity { id: oid, opacity: op });
                        }
                        if let Some((gid, gain)) = self.live_gain.take_if(|(g, _)| *g == id) {
                            self.apply(Command::SetGain { id: gid, gain_db: gain });
                        }
                    }
                }
                Some(Drag::Rubber { anchor, add }) => {
                    // the release point: interact_pointer_pos lives until the end of the
                    // release frame, hover is a safety net
                    let band =
                        egui::Rect::from_two_pos(anchor, pointer.or(hover).unwrap_or(anchor));
                    // the frame is remembered IN THE MODEL DOMAIN (time × track keys):
                    // a named area is made from it (R.35 item 2)
                    // by a menu item or a button, already after the user
                    // has released the mouse.
                    self.last_band = Some((
                        Tick(x_to_tick(band.left()).max(CURSOR_FLOOR)),
                        Tick(x_to_tick(band.right()).max(CURSOR_FLOOR)),
                        geom.rows_in_band(band.top(), band.bottom()),
                    ));
                    let hits = rubber_hits(&geom, self.model.project(), band);
                    if !add {
                        self.selected.clear();
                        self.selected_morph.clear();
                        self.selected_camera.clear();
                    }
                    // the line “caught clips → dim the morpher” no longer exists:
                    // the frame catches EVERYTHING under it, and dims only what
                    // did not fall under it (R.40)
                    self.selected.extend(hits.clips);
                    self.selected_morph.extend(hits.morphers);
                    self.selected_camera.extend(hits.cameras);
                }
                Some(Drag::Region { .. }) | Some(Drag::RegionEdge { .. }) => {
                    self.on_region_changed();
                    // playing — the cursor goes immediately to the start of the new region (item 4)
                    self.on_region_released();
                }
                Some(Drag::Camera { id, cur_start, cur_lane, moved, .. }) => {
                    // moving the camera carries its curves along: their domain is
                    // ticks FROM THE START of the event, nothing to touch (R.34 K3)
                    if moved
                        && let Some(mut ev) =
                            self.model.project().cameras.iter().find(|c| c.id == id).cloned()
                        && (ev.start.0 != cur_start || ev.lane != cur_lane)
                    {
                        ev.start = Tick(cur_start);
                        ev.lane = cur_lane;
                        self.apply(Command::SetCamera(ev));
                    }
                }
                Some(Drag::NestWindow { inst, left, cur }) => {
                    // ORDINARY COMMANDS on the instance: the gesture lives in the
                    // reel's rows, but edits the root event — and this is the only
                    // place where it is so. The model itself does the clamps (`check_on`
                    // of the trims), so there is no window arithmetic of its own here.
                    let cmd = if left {
                        Command::TrimStart { id: inst, new_start: Tick(cur) }
                    } else {
                        Command::TrimEnd { id: inst, new_end: Tick(cur) }
                    };
                    self.apply_batch_restoring(vec![cmd]);
                }
                Some(Drag::CameraEdge { id, left, cur }) => {
                    if let Some(mut ev) =
                        self.model.project().cameras.iter().find(|c| c.id == id).cloned()
                    {
                        let (s0, e0) = (ev.start.0, (ev.start + ev.duration).0);
                        let min = kt_model::MIN_CLIP_DURATION.0;
                        if left {
                            let ns = cur.min(e0 - min).max(self.time_floor());
                            ev.start = Tick(ns);
                            ev.duration = Tick(e0 - ns);
                        } else {
                            ev.duration = Tick((cur - s0).max(min));
                        }
                        self.apply(Command::SetCamera(ev));
                    }
                }
                Some(Drag::MorphEdge { id, left, cur }) => {
                    if let Some(mut ev) =
                        self.model.project().morphers.iter().find(|m| m.id == id).cloned()
                    {
                        let (s0, e0) = (ev.start.0, (ev.start + ev.duration).0);
                        let min = kt_model::MIN_CLIP_DURATION.0;
                        if left {
                            let ns = cur.min(e0 - min).max(self.time_floor());
                            ev.start = Tick(ns);
                            ev.duration = Tick(e0 - ns);
                        } else {
                            ev.duration = Tick((cur - s0).max(min));
                        }
                        self.apply(Command::SetMorpher(ev));
                    }
                }
                Some(Drag::Morph { id, cur_start, cur_lane, moved, .. }) => {
                    if moved
                        && let Some(mut ev) =
                            self.model.project().morphers.iter().find(|m| m.id == id).cloned()
                        && (ev.start.0 != cur_start || ev.lane != cur_lane)
                    {
                        ev.start = Tick(cur_start);
                        ev.lane = cur_lane;
                        self.apply(Command::SetMorpher(ev));
                    }
                }
                _ => {}
            }
        }

        // ---- clips -----------------------------------------------------------------
        // stretch maps — once per model version (R.37 item 3)
        let warps = self.warp_maps();
        // Previews of REELS (R.56, item 452). Requests are placed BEFORE the draw cycle and
        // by the key (reel, revision) — so neither zoom nor scroll
        // generates them (invariant 8), and a repeat request is removed by dedup.
        // The key is per INSTANCE (R.59, stage 3): an instance with
        // overrides has its own revision of the reel and its own frames.
        let reel_keys = self.reel_keys();
        let wanted: Vec<(crate::reel_preview::ReelKey, Vec<kt_model::Override>)> = self
            .model
            .project()
            .clips
            .iter()
            .filter_map(|c| Some((*reel_keys.get(&c.id)?, c.overrides.clone())))
            .collect();
        for (key, ov) in wanted {
            self.request_reel_preview(key, &ov);
        }
        let peaks = self.peaks.lock().unwrap();
        let offline = self.offline.lock().unwrap().clone();

        // order = the order of the ENGINE COMPOSITION: a later clip lies on top of an
        // earlier one, as in the preview (item 177 — until now the strip drew in
        // the project list order and could show a source other than the one in the frame)
        for c in draw_order.iter().filter_map(|&i| self.model.project().clips.get(i)) {
            let (start, end, lane) = match &self.drag {
                Some(Drag::Clip(d)) if d.id == c.id => (d.cur_start, d.cur_end, d.cur_lane),
                // the other events of the group move by the same shift (R.33 item 7)
                Some(Drag::Clip(d)) if d.extra.iter().any(|(id, _)| *id == c.id) => {
                    let delta = d.cur_start - d.orig.start.0;
                    (c.start.0 + delta, c.start.0 + c.duration.0 + delta, c.lane)
                }
                _ => (c.start.0, c.start.0 + c.duration.0, c.lane),
            };
            let x0 = tick_to_x(start);
            let x1 = tick_to_x(end);
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let src_shift = preview_src_shift(self.drag.as_ref(), c.id, c.start.0);
            let top = lane_top(c.kind, lane);
            let lane_h = lane_h_at(c.kind, lane);
            // the clip is dragged UNDER the track name strip — it is semi-transparent and
            // the timeline shows through (item 4 round 7.1); the name is drawn from the OLD
            // boundary (rect.left()), so the text does not run under the gradient
            let clip_rect = egui::Rect::from_min_max(
                egui::pos2(x0.max(full.left()), top + 1.0),
                egui::pos2(x1.min(rect.right()), top + lane_h - 3.0),
            );
            let selected = self.is_selected(c.id);
            let dragging = matches!(&self.drag, Some(Drag::Clip(d)) if d.id == c.id);
            let is_offline = offline.contains(&c.media);

            let mut fill = media_paths
                .get(&c.media)
                .map(|p| clip_color(p, c.kind))
                .unwrap_or(egui::Color32::from_gray(80));
            // audio clips are darker — the waveform must be readable
            if c.kind == ClipKind::Audio {
                fill = fill.gamma_multiply(0.55);
            }
            if dragging {
                fill = fill.gamma_multiply(0.75);
            }
            let edge =
                if selected { egui::Color32::WHITE } else { egui::Color32::from_rgb(25, 25, 25) };
            painter.rect(
                clip_rect,
                4.0,
                fill,
                egui::Stroke::new(if selected { 2.0 } else { 1.0 }, edge),
                egui::StrokeKind::Inside,
            );
            // a light vertical gradient (light from above)
            crate::toolbar::gradient_rect(
                &painter,
                egui::Rect::from_min_max(
                    clip_rect.left_top() + egui::vec2(1.0, 1.0),
                    egui::pos2(clip_rect.right() - 1.0, clip_rect.center().y),
                ),
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22),
                egui::Color32::TRANSPARENT,
            );
            let hovered =
                hover.map(|h| clip_rect.contains(h) && self.drag.is_none()).unwrap_or(false);
            if hovered && !selected {
                painter.rect(
                    clip_rect,
                    4.0,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 90),
                    ),
                    egui::StrokeKind::Inside,
                );
            }
            if is_offline {
                painter.rect_filled(
                    clip_rect,
                    4.0,
                    egui::Color32::from_rgba_unmultiplied(55, 25, 25, 110),
                );
                painter.rect(
                    clip_rect.shrink(1.5),
                    3.0,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 140, 140)),
                    egui::StrokeKind::Inside,
                );
            }

            // Time stretch (R.37 item 3): the preview must compute the
            // position of the material with THE SAME MAP as the render, otherwise
            // a slowed clip draws something other than what it plays. The map comes from
            // the cache by model_rev; it must not be built per column (allocation in the
            // draw path, invariant 12).
            let warp = warps.get(&c.id);
            let dur = c.duration.0.max(1);
            // timeline tick inside an event → OFFSET IN THE MATERIAL:
            // the map “event tick → material” lives in ONE place
            // (`thumbs::material_key_t`, R.54): the picker thumbnails ask it too,
            // and they cannot diverge — otherwise a cell would show
            // a frame that is not on the event.
            let material_off =
                |tl: i64| -> i64 { crate::thumbs::material_at(warp, c.duration, c.reversed, tl) };
            let key_at = |tl: i64, step: i64| -> i64 {
                crate::thumbs::material_key_t(
                    c.source_in.0 + src_shift,
                    warp,
                    c.duration,
                    c.reversed,
                    tl,
                    step,
                )
            };
            // the actually consumed span of the source: the filmstrip step is chosen by it,
            // otherwise a slowed clip takes a step twice as coarse and degenerates into
            // repeating buckets
            let used_span = warp.map_or(dur, |w| w.used().max(1));

            // A NESTED REEL. Since R.56 (item 452) it has both a filmstrip and
            // a waveform — “like ordinary events”, at the owner’s direct request.
            // The reel still has no FILE of its own: the frames and
            // the envelope come from an offline render of its contents by the key
            // (reel, revision), not from a thumbnail by path (invariant 8 intact —
            // the request is placed once per key, before the cycle).
            //
            // The mini-map remains a PLACEHOLDER: while the strip is being computed,
            // the person still sees what the reel consists of.
            let is_nest = self.model.project().sequence(c.media).is_some();
            let reel_key = reel_keys.get(&c.id).copied();
            let reel_prev = reel_key.and_then(|k| self.reel_previews.get(&k));
            let reel_strip = reel_prev.as_ref().and_then(|p| p.strip.clone());
            if is_nest && reel_strip.is_none() && clip_rect.width() > 20.0 {
                let seq = self.model.project().sequence(c.media).unwrap();
                let win = (c.source_in.0, c.source_in.0 + dur);
                let span = (win.1 - win.0).max(1) as f32;
                let mut mesh = egui::Mesh::default();
                let lanes = seq.lanes(kt_model::LaneKind::Video).max(1)
                    + seq.lanes(kt_model::LaneKind::Audio).max(1);
                let bar_h = (clip_rect.height() - 4.0) / lanes as f32;
                for ch in &seq.clips {
                    let (a, b) = (ch.start.0, (ch.start + ch.duration).0);
                    let (a, b) = (a.max(win.0), b.min(win.1));
                    if a >= b {
                        continue;
                    }
                    let row = match ch.kind {
                        ClipKind::Video => ch.lane,
                        ClipKind::Audio => seq.lanes(kt_model::LaneKind::Video).max(1) + ch.lane,
                    };
                    let y = clip_rect.top() + 2.0 + row as f32 * bar_h;
                    let r = egui::Rect::from_min_max(
                        egui::pos2(
                            clip_rect.left() + clip_rect.width() * (a - win.0) as f32 / span,
                            y,
                        ),
                        egui::pos2(
                            clip_rect.left() + clip_rect.width() * (b - win.0) as f32 / span,
                            y + (bar_h - 1.0).max(1.0),
                        ),
                    );
                    mesh.add_colored_rect(r, egui::Color32::from_white_alpha(40));
                }
                painter.add(egui::Shape::mesh(mesh));
            }
            // FILMSTRIP OF A REEL (R.56, item 452): the frames are taken from the strip of
            // offline previews, and the MOMENT — by the same map “event tick →
            // material” as for the file (t549). So a slowed or
            // mirrored instance pages correctly, and not one second
            // formula was introduced for this.
            if c.kind == ClipKind::Video
                && let Some(strip) = &reel_strip
                && let Some(key) = reel_key
                && clip_rect.width() > 30.0
            {
                let thumb_h = clip_rect.height() - 2.0;
                let thumb_w = (thumb_h * 16.0 / 9.0).max(28.0);
                let mut x = clip_rect.left();
                while x < clip_rect.right() {
                    let tl = x_to_tick(x + thumb_w * 0.5) - start;
                    if tl >= 0 {
                        let src_t = c.source_in.0 + src_shift + material_off(tl);
                        let idx = strip.index(src_t);
                        let tex = match self.reel_textures.get(&(key, idx)) {
                            Some(t) => Some(t.clone()),
                            None => strip.at(src_t).map(|img| {
                                let t = ui.ctx().load_texture(
                                    format!("reel-{}-{}", key.seq.0, idx),
                                    (**img).clone(),
                                    egui::TextureOptions::LINEAR,
                                );
                                self.reel_textures.insert((key, idx), t.clone());
                                t
                            }),
                        };
                        if let Some(t) = tex {
                            // thumbnail in the proportions of the SOURCE (R.62, t719):
                            // the vertical one is centred in the cell, margins on the
                            // sides — the event body; nothing is stretched
                            let [tw, th] = t.size();
                            let cell_w = thumb_w.min(clip_rect.right() - x);
                            let (off, fit_w) = crate::thumbs::thumb_cell_fit(
                                cell_w, thumb_h, tw as f32, th as f32,
                            );
                            let x_img = x + off;
                            let dst = egui::Rect::from_min_size(
                                egui::pos2(x_img, clip_rect.top() + 1.0),
                                egui::vec2(fit_w.min(cell_w - off).max(0.0), thumb_h),
                            );
                            let uv = egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2((dst.width() / fit_w).min(1.0), 1.0),
                            );
                            painter.image(t.id(), dst, uv, egui::Color32::WHITE);
                            painter.rect(
                                dst,
                                0.0,
                                egui::Color32::TRANSPARENT,
                                egui::Stroke::new(1.0, crate::theme::THUMB_FRAME),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                    x += thumb_w + 3.0;
                }
            }
            // video clip filmstrip: thumbnails from the index DB/service; drawing
            // is never blocked and never touches the player's decoders
            if c.kind == ClipKind::Video && !is_nest && !is_offline && clip_rect.width() > 30.0 {
                let thumb_h = clip_rect.height() - 2.0;
                let thumb_w = (thumb_h * 16.0 / 9.0).max(28.0);
                let step = crate::thumbs::thumb_source_step(
                    Tick(used_span),
                    (x1 - x0).max(1.0) * 1.0,
                    thumb_w,
                );
                let mut x = clip_rect.left();
                while x < clip_rect.right() {
                    // the moment of the MIDDLE of the column (v1: a thumb is centred on
                    // its position, not floored into the past — item 3)
                    let tl = x_to_tick(x + thumb_w * 0.5) - start;
                    if tl >= 0 {
                        // the bucket is from the START OF THE PIECE (source_in), not from zero of
                        // the file: for a clip with source_in=3c the first column
                        // showed a frame 3 seconds BEFORE the start of the piece.
                        // Quantisation FROM FILE ZERO — a shared cache with the bin
                        // (item 11 round 6, test 62)
                        let src_t = key_at(tl, step);
                        // the SOURCE address of the event, not its media (R.59,
                        // stage 2): the generator values live on the event, and
                        // the filmstrip must show THEM, not the kind defaults
                        // with which the media is labelled
                        let key = crate::thumbs::ThumbKey {
                            media: kt_engine::clip_source_addr(c),
                            t: src_t,
                        };
                        let tex = match self.thumb_textures.get(&key) {
                            Some(t) => Some(t.clone()),
                            None => match self.thumbs.get(&key) {
                                Some(img) => {
                                    let t = ui.ctx().load_texture(
                                        format!("thumb-{}-{}", key.media.0, key.t),
                                        (*img).clone(),
                                        egui::TextureOptions::LINEAR,
                                    );
                                    self.thumb_textures.insert(key, t.clone());
                                    self.thumb_lru.retain(|k| *k != key);
                                    self.thumb_lru.push_back(key);
                                    // LRU instead of a full clear() (item 2: resetting
                                    // all textures gave a storm of reloads
                                    // on zoom)
                                    while self.thumb_textures.len() > crate::thumbs::MAX_READY
                                        && let Some(old) = self.thumb_lru.pop_front()
                                    {
                                        self.thumb_textures.remove(&old);
                                    }
                                    Some(t)
                                }
                                None => {
                                    self.thumbs.request(key);
                                    None
                                }
                            },
                        };
                        if let Some(t) = tex {
                            // thumbnail in the proportions of the SOURCE (R.62, t719):
                            // the vertical one is centred in the cell, margins on the
                            // sides — the event body; nothing is stretched
                            let [tw, th] = t.size();
                            let cell_w = thumb_w.min(clip_rect.right() - x);
                            let (off, fit_w) = crate::thumbs::thumb_cell_fit(
                                cell_w, thumb_h, tw as f32, th as f32,
                            );
                            let x_img = x + off;
                            let dst = egui::Rect::from_min_size(
                                egui::pos2(x_img, clip_rect.top() + 1.0),
                                egui::vec2(fit_w.min(cell_w - off).max(0.0), thumb_h),
                            );
                            let uv = egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2((dst.width() / fit_w).min(1.0), 1.0),
                            );
                            painter.image(t.id(), dst, uv, egui::Color32::WHITE);
                            // filmstrip frames: a thin frame + gap
                            // 1px frame + gap between previews — the perforation
                            // removed (item 21 round 6: ~42 extra shapes per frame)
                            painter.rect(
                                dst,
                                0.0,
                                egui::Color32::TRANSPARENT,
                                egui::Stroke::new(1.0, crate::theme::THUMB_FRAME),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                    x += thumb_w + 3.0;
                }
            }

            // stereo waveform. For a REEL the envelope is taken from the preview
            // (R.56, item 452): the index service peaks are keyed by the FILE's `MediaId`,
            // and a reel has no file — before R.56 the audio half of the attachment
            // drew nothing at all.
            let reel_peaks = reel_key.and_then(|k| self.reel_previews.peaks(&k));
            if c.kind == ClipKind::Audio
                && !is_offline
                && let Some(levels) = peaks.get(&c.media).or(reel_peaks)
                && !levels.is_empty()
            {
                let frames_per_px = tpp * 48_000.0 / 90_000.0;
                let mut level = 0usize;
                let mut block = kt_media::PEAK_BLOCK_FRAMES as f64;
                while block * 2.0 <= frames_per_px && level + 1 < levels.len() {
                    level += 1;
                    block *= 2.0;
                }
                let (left_ch, right_ch) = &levels[level];
                let mid = clip_rect.center().y;
                let half = (clip_rect.height() - 6.0) * 0.5;
                let eff_src_in = c.source_in.0 + src_shift;
                let src_in_frames = Tick(eff_src_in.max(0)).to_audio_frames() as f64;
                let col_l = egui::Color32::from_rgba_unmultiplied(170, 210, 255, 230);
                let col_r = egui::Color32::from_rgba_unmultiplied(230, 190, 255, 230);
                // one mesh per clip instead of 2 line_segment per PIXEL
                // (item 1 v1.4: thousands of shapes per frame on wide clips)
                let mut mesh = egui::Mesh::default();
                let mut x = clip_rect.left();
                while x < clip_rect.right() {
                    let tl0 = material_off(x_to_tick(x) - start);
                    let f0 = src_in_frames + tl0 as f64 * 48_000.0 / 90_000.0;
                    if f0 >= 0.0 {
                        let bi = (f0 / block) as usize;
                        if let Some(&(mn, mx)) = left_ch.get(bi) {
                            let amp = (mx.abs().max(mn.abs())) as f32 / 32767.0;
                            mesh.add_colored_rect(
                                egui::Rect::from_min_max(
                                    egui::pos2(x, mid - (amp * half).max(0.5)),
                                    egui::pos2(x + 1.0, mid),
                                ),
                                col_l,
                            );
                        }
                        if let Some(&(mn, mx)) = right_ch.get(bi) {
                            let amp = (mx.abs().max(mn.abs())) as f32 / 32767.0;
                            mesh.add_colored_rect(
                                egui::Rect::from_min_max(
                                    egui::pos2(x, mid),
                                    egui::pos2(x + 1.0, mid + (amp * half).max(0.5)),
                                ),
                                col_r,
                            );
                        }
                    }
                    x += 1.0;
                }
                if !mesh.is_empty() {
                    painter.add(egui::Shape::mesh(mesh));
                }
            }

            // level line (item 12): for video opacity, for audio
            // volume — ONE element for both kinds, as in Vegas (requirement
            // “fades for audio and video look the same”: the vertical
            // v1 rail stood exactly in the fade zone and broke the picture)
            // the threshold is the same as for the HIT TEST (R.40 item 12): before, the line
            // was drawn at width > 24 px, while the level zone demanded the
            // same — and below that width the opacity was unreachable
            // altogether. Now there is one limit: an event that has
            // something to show.
            if clip_rect.width() > 6.0 {
                let lv = if c.kind == ClipKind::Audio {
                    match self.live_gain {
                        Some((id, g)) if id == c.id => gain_to_level(g),
                        _ => gain_to_level(c.gain_db),
                    }
                } else {
                    match self.live_opacity {
                        Some((id, o)) if id == c.id => o,
                        _ => c.opacity,
                    }
                };
                let op = lv;
                let ly = level_y(lv, top, lane_h);
                // “not default” is brighter: for video it is op<1, for audio 0 dB
                // sits in the middle (the v1 rail range ±6 dB)
                let dim =
                    if c.kind == ClipKind::Audio { (op - 0.5).abs() > 1e-3 } else { op < 1.0 };
                painter.line_segment(
                    [egui::pos2(clip_rect.left(), ly), egui::pos2(clip_rect.right(), ly)],
                    egui::Stroke::new(
                        if dim { 2.0 } else { 1.0 },
                        egui::Color32::from_rgba_unmultiplied(
                            255,
                            220,
                            120,
                            if dim { 230 } else { 90 },
                        ),
                    ),
                );
            }

            // clip name on the plate
            if clip_rect.width() > 40.0 && !suppress_name.contains(&c.id) {
                let mut label = media_names.get(&c.id).cloned().unwrap_or_default();
                // ×N BADGE — “how many places will change together with this”
                // (R.50). Only when N > 1: a promise that means nothing
                // is worse than its absence. This is our only differentiator
                // from Premiere, FCP and VEGAS, and it is free —
                // the place is already taken by the label.
                if is_nest {
                    let n = self.model.document().sequence_uses(c.media);
                    if n > 1 {
                        label.push_str(&format!("  ×{n}"));
                    }
                    // PIN — “this instance differs from the reel”
                    // (R.59, stage 3). The place is already taken by the label, so no
                    // separate strip is made for the sign (rule 13); how many
                    // handles are overridden exactly — is in the hint.
                    if !c.overrides.is_empty() {
                        label.push(' ');
                        label.push_str(crate::icons::PIN);
                    }
                }
                if !c.fx.is_empty() {
                    label.push(' ');
                    label.push_str(crate::icons::FX);
                }
                if c.reversed {
                    label.push_str(" ◀");
                }
                let galley = painter.layout_no_wrap(
                    label,
                    egui::FontId::proportional(10.5),
                    egui::Color32::WHITE,
                );
                let plate = egui::Rect::from_min_size(
                    egui::pos2(clip_rect.left().max(rect.left()), clip_rect.top())
                        + egui::vec2(4.0, 2.0),
                    galley.size() + egui::vec2(8.0, 3.0),
                );
                painter.rect_filled(
                    plate.intersect(clip_rect),
                    3.0,
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 140),
                );
                painter.galley(plate.min + egui::vec2(4.0, 1.5), galley, egui::Color32::WHITE);
            }

            // trim handles on the selection
            if selected && clip_rect.width() > 20.0 {
                for x in [clip_rect.left() + 2.0, clip_rect.right() - 4.0] {
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(x, clip_rect.center().y - 7.0),
                            egui::vec2(2.5, 14.0),
                        ),
                        1.0,
                        egui::Color32::from_gray(235),
                    );
                }
            }

            // coverage strip from the cache (invariant 4)
            let cov_kind = match c.kind {
                ClipKind::Audio => BurstKind::Audio,
                ClipKind::Video => BurstKind::video(Some(self.player.preview_quality())),
            };
            let (b0, b1) = match c.kind {
                ClipKind::Audio => {
                    let f0 = c.source_in.to_audio_frames();
                    let f1 = (c.source_in + c.duration).to_audio_frames();
                    (
                        f0.div_euclid(AUDIO_BURST_FRAMES as i64),
                        (f1 - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1,
                    )
                }
                ClipKind::Video => (c.source_in.0, c.source_in.0 + c.duration.0),
            };
            let _ = (cov_kind, b0, b1);
            for run in coverage_snapshot.get(&c.id).map(|v| v.as_slice()).unwrap_or(&[]) {
                let (t0, t1) = match c.kind {
                    ClipKind::Audio => (
                        c.start.0
                            + Tick::from_audio_frames(
                                run.start * AUDIO_BURST_FRAMES as i64
                                    - c.source_in.to_audio_frames(),
                            )
                            .0,
                        c.start.0
                            + Tick::from_audio_frames(
                                run.end * AUDIO_BURST_FRAMES as i64 - c.source_in.to_audio_frames(),
                            )
                            .0,
                    ),
                    ClipKind::Video => {
                        (c.start.0 + run.start - c.source_in.0, c.start.0 + run.end - c.source_in.0)
                    }
                };
                painter.line_segment(
                    [
                        egui::pos2(tick_to_x(t0).max(clip_rect.left()), clip_rect.bottom() - 1.5),
                        egui::pos2(tick_to_x(t1).min(clip_rect.right()), clip_rect.bottom() - 1.5),
                    ],
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(110, 235, 110)),
                );
            }
        }
        drop(peaks);

        // ---- named areas (R.35 item 2) --------------------------------
        // We do NOT make a separate strip (rule 13): the area is drawn as an outline
        // right over its own rows, and the name — IN A GAP of the top line (the same
        // technique as the bin group name).
        for a in &self.model.project().areas {
            let Some(r) = geom.area_rect(a) else { continue };
            let col =
                a.color.as_deref().and_then(crate::theme::parse_hex).unwrap_or(crate::theme::AREA);
            // a gap in the top line for the name
            let font = egui::FontId::proportional(11.0);
            let galley = painter.layout_no_wrap(a.name.clone(), font.clone(), col);
            let gap = (galley.size().x + 8.0).min((r.width() - 8.0).max(0.0));
            let stroke = egui::Stroke::new(1.0, col);
            painter.line_segment([r.left_bottom(), r.right_bottom()], stroke);
            painter.line_segment([r.left_top(), r.left_bottom()], stroke);
            painter.line_segment([r.right_top(), r.right_bottom()], stroke);
            painter.line_segment(
                [r.left_top(), egui::pos2((r.left() + 6.0).min(r.right()), r.top())],
                stroke,
            );
            painter.line_segment(
                [egui::pos2((r.left() + 6.0 + gap).min(r.right()), r.top()), r.right_top()],
                stroke,
            );
            if gap > 12.0 {
                painter.galley(
                    egui::pos2(r.left() + 10.0, r.top() - galley.size().y * 0.5),
                    galley,
                    col,
                );
            }
        }

        // ---- ZONE BEFORE ZERO inside a reel (R.58, item 462) ------------------------
        //
        // The owner’s decision: show it with a veil; the cursor and playback
        // do not go there. The same veil and the same technique as the tails of the strip
        // below, and it is drawn AFTER the events — an event dragged past zero
        // must look “outside the window”, not like an ordinary one. Only inside
        // the full entry: at the root zero is a wall, and there is nothing to its left.
        if self.model.editing().is_some() {
            let x0 = tick_to_x(0);
            if x0 > rect.left() && !geom.rows.is_empty() {
                let last = geom.rows.len() - 1;
                let (y0, y1) = (geom.row_top(0), geom.row_top(last) + geom.row_h(last));
                let mut veil = egui::Mesh::default();
                veil.add_colored_rect(
                    egui::Rect::from_min_max(
                        egui::pos2(rect.left(), y0),
                        egui::pos2(x0.min(rect.right()), y1),
                    ),
                    crate::theme::BAND_TAIL_VEIL,
                );
                painter.add(egui::Shape::mesh(veil));
                painter.line_segment(
                    [egui::pos2(x0, y0), egui::pos2(x0, y1)],
                    egui::Stroke::new(2.0, crate::theme::BAND_EDGE),
                );
            }
        }

        // ---- EXPANDED STRIP: tails and window boundary (R.55, C5) ------------
        //
        // Drawn AFTER the events, over them: the veil must also cover
        // the clips and their filmstrips with waveforms, otherwise “outside the window” and
        // “inside the window” would differ only by the frame.
        //
        // The reel material outside the instance window does NOT SOUND and is not shown in the
        // preview, but it exists, it is visible, and it can be edited. The owner
        // chose exactly this: “dimmed + editable + drag the edge”.
        for b in self.model.bands() {
            let Some(mut win) = b.window(self.model.project()) else { continue };
            // LIVE PREVIEW of the edge: without it the window would stand still
            // until release, and the gesture would look dead
            if let Some(Drag::NestWindow { inst, left, cur }) = &self.drag
                && *inst == b.inst
            {
                if *left {
                    win.start = Tick((*cur).min(win.end.0 - 1));
                } else {
                    win.end = Tick((*cur).max(win.start.0 + 1));
                }
            }
            let rows: Vec<usize> = geom
                .rows
                .iter()
                .enumerate()
                .filter(|(_, r)| b.owns(r.0, r.1))
                .map(|(i, _)| i)
                .collect();
            let (Some(&first), Some(&last)) = (rows.first(), rows.last()) else { continue };
            let y0 = geom.row_top(first);
            let y1 = geom.row_top(last) + geom.row_h(last);
            let (xa, xb) = (tick_to_x(win.start.0), tick_to_x(win.end.0));
            // VEIL to the left and right of the window — one mesh for both sides
            let mut veil = egui::Mesh::default();
            for (a, z) in [(rect.left(), xa), (xb, rect.right())] {
                if z > a {
                    veil.add_colored_rect(
                        egui::Rect::from_min_max(egui::pos2(a, y0), egui::pos2(z, y1)),
                        crate::theme::BAND_TAIL_VEIL,
                    );
                }
            }
            painter.add(egui::Shape::mesh(veil));
            // WINDOW BOUNDARIES — they are also the instance edge grab zone
            for x in [xa, xb] {
                if x >= rect.left() && x <= rect.right() {
                    painter.line_segment(
                        [egui::pos2(x, y0), egui::pos2(x, y1)],
                        egui::Stroke::new(2.0, crate::theme::BAND_EDGE),
                    );
                }
            }
            // BLOCK FRAME: the strip reads as a block, not as “more tracks”
            painter.rect_stroke(
                egui::Rect::from_min_max(egui::pos2(rect.left(), y0), egui::pos2(rect.right(), y1)),
                0.0,
                egui::Stroke::new(1.0, crate::theme::BAND_EDGE.gamma_multiply(0.5)),
                egui::StrokeKind::Inside,
            );
        }

        // ---- selection frame (R.33 item 7): same look as in the media bin ------
        if let Some(Drag::Rubber { anchor, .. }) = &self.drag
            && let Some(cur) = pointer.or(hover)
        {
            let band = egui::Rect::from_two_pos(*anchor, cur);
            painter.rect_filled(band, 0.0, egui::Color32::from_rgba_unmultiplied(90, 140, 220, 40));
            painter.rect_stroke(
                band,
                0.0,
                egui::Stroke::new(1.0, crate::theme::ACCENT),
                egui::StrokeKind::Middle,
            );
        }

        // ---- morph blocks (H1, v1 port: purple rounded block) ---------
        for m in &self.model.project().morphers {
            // preview row: during a move the block must FOLLOW THE CURSOR across
            // rows, otherwise the track change is not visible until release (R.40 item 2)
            let mut lane = m.lane;
            let (start, end) = match &self.drag {
                Some(Drag::Morph { id, cur_start, cur_lane, .. }) if *id == m.id => {
                    lane = *cur_lane;
                    (*cur_start, *cur_start + m.duration.0)
                }
                Some(Drag::MorphEdge { id, left, cur }) if *id == m.id => {
                    let min = kt_model::MIN_CLIP_DURATION.0;
                    let (s0, e0) = (m.start.0, (m.start + m.duration).0);
                    if *left {
                        ((*cur).min(e0 - min).max(self.time_floor()), e0)
                    } else {
                        (s0, (*cur).max(s0 + min))
                    }
                }
                _ => (m.start.0, (m.start + m.duration).0),
            };
            let x0 = tick_to_x(start);
            let x1 = tick_to_x(end).max(x0 + 8.0);
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let top = morph_lane_top(lane);
            // a service row three times lower (R.35 item 1): the margins shrink, but
            // the GRAB is 2 px wider outward than the drawing — checklist rule 1
            let rh = thin_h_at(LaneKind::Morph, lane);
            let pad = (rh * 0.12).clamp(1.0, 3.0);
            let r = egui::Rect::from_min_max(
                egui::pos2(x0.max(rect.left()), top + pad),
                egui::pos2(x1.min(rect.right()), top + rh - pad - 2.0),
            );
            let selected = self.selected_morph.contains(&m.id);
            painter.rect(
                r,
                4.0,
                egui::Color32::from_rgba_unmultiplied(140, 90, 200, 200),
                egui::Stroke::new(
                    if selected { 2.0 } else { 1.0 },
                    if selected {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_rgb(40, 30, 55)
                    },
                ),
                egui::StrokeKind::Inside,
            );
            // the STRENGTH of the layer and its fades are visible on the block itself (R.40 item 12):
            // until this round they lived ONLY in the morpher window, and from the timeline
            // one could not tell that the layer was weakened. We do not make
            // a separate strip for this — we draw inside the block
            // (rule 13); the line darkens at the edges exactly where
            // the fades work.
            if (m.strength as f32) < 1.0 || m.fade_in.0 > 0 || m.fade_out.0 > 0 {
                let h = (r.height() - 4.0).max(1.0);
                let ly = r.top() + 2.0 + (1.0 - m.strength.clamp(0.0, 1.0) as f32) * h;
                let dur = m.duration.0.max(1) as f32;
                let fi = r.left() + r.width() * (m.fade_in.0 as f32 / dur).clamp(0.0, 1.0);
                let fo = r.right() - r.width() * (m.fade_out.0 as f32 / dur).clamp(0.0, 1.0);
                let col = egui::Color32::from_rgba_unmultiplied(255, 220, 120, 220);
                let st = egui::Stroke::new(1.6, col);
                painter
                    .line_segment([egui::pos2(r.left(), r.bottom() - 1.0), egui::pos2(fi, ly)], st);
                painter.line_segment([egui::pos2(fi, ly), egui::pos2(fo.max(fi), ly)], st);
                painter.line_segment(
                    [egui::pos2(fo.max(fi), ly), egui::pos2(r.right(), r.bottom() - 1.0)],
                    st,
                );
            }
            // caption by the TARGET MODE (R.35): the morpher no longer has a kind,
            // and “whom it acts on” is the only thing not visible from
            // the block itself. If it does not fit — it goes into the hint (rule 7).
            let label = match &m.targets {
                MorphTargets::Clips { ids } => crate::tf!(k::tl::morph::CLIPS, ids.len()),
                _ => tr(k::tl::morph::LAYER).to_string(),
            };
            let font = egui::FontId::proportional((rh * 0.5).clamp(8.0, 11.0));
            let galley = painter.layout_no_wrap(label.clone(), font.clone(), egui::Color32::WHITE);
            if galley.size().x + 10.0 <= r.width() && galley.size().y <= r.height() {
                painter.text(
                    egui::pos2(r.left() + 6.0, r.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    font,
                    egui::Color32::WHITE,
                );
            } else if ui.ctx().pointer_hover_pos().is_some_and(|p| r.contains(p)) {
                self.hover_hint = Some(label);
            }
        }
        // ---- camera events (R.29): turquoise block on the camera track ------
        for c in &self.model.project().cameras {
            // live gesture preview: until R.38 the camera stood still during a
            // drag until release — the gesture looked dead
            let mut lane = c.lane;
            let (start, end) = match &self.drag {
                Some(Drag::Camera { id, cur_start, cur_lane, .. }) if *id == c.id => {
                    lane = *cur_lane;
                    (*cur_start, *cur_start + c.duration.0)
                }
                Some(Drag::CameraEdge { id, left, cur }) if *id == c.id => {
                    let min = kt_model::MIN_CLIP_DURATION.0;
                    let (s0, e0) = (c.start.0, (c.start + c.duration).0);
                    if *left {
                        ((*cur).min(e0 - min).max(self.time_floor()), e0)
                    } else {
                        (s0, (*cur).max(s0 + min))
                    }
                }
                _ => (c.start.0, (c.start + c.duration).0),
            };
            let x0 = tick_to_x(start);
            let x1 = tick_to_x(end).max(x0 + 8.0);
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let top = geom.row_top(geom.row_of((LaneKind::Camera, lane)).unwrap_or(0));
            // with a CAMERA divider, not a morph one (R.40): before, the camera block
            // was drawn the height of the morph row (base/1.5) on a row
            // of height base/3 and stuck out of it twice over
            let cam_h = thin_h_at(LaneKind::Camera, lane);
            let pad = (cam_h * 0.12).clamp(1.0, 3.0);
            let r = egui::Rect::from_min_max(
                egui::pos2(x0.max(rect.left()), top + pad),
                egui::pos2(x1.min(rect.right()), top + cam_h - pad - 2.0),
            );
            let selected = self.selected_camera.contains(&c.id);
            painter.rect(
                r,
                4.0,
                egui::Color32::from_rgba_unmultiplied(60, 150, 160, 200),
                if selected {
                    egui::Stroke::new(2.0, crate::theme::ACCENT)
                } else {
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(24, 52, 58))
                },
                egui::StrokeKind::Inside,
            );
            if r.width() > 46.0 {
                let label = if c.name.is_empty() { format!("C{}", c.id.0) } else { c.name.clone() };
                painter.text(
                    egui::pos2(r.left() + 6.0, r.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(11.0),
                    egui::Color32::WHITE,
                );
            }
        }

        // ---- file-drop silhouette (item 2 v1.4): track highlight + ghost ----
        // The same silhouette serves dragging a card FROM THE BIN
        // (item 5 round 8): the payload from the egui drag gives the position itself.
        let bin_drag_pos = egui::DragAndDrop::payload::<crate::bin_panel::BinDrag>(ui.ctx())
            .map(|p| *p)
            .and_then(|p| ui.input(|i| i.pointer.hover_pos()).map(|pos| (p.0, pos)));
        if let Some((media, pos)) = bin_drag_pos
            && let Some(g) = self.tl_geom.clone()
            && g.in_lanes(pos)
            && let Some((kind, lane)) = g.lane_at_y(pos.y)
        {
            self.dnd_hover = Some(pos);
            if ui.input(|i| i.pointer.any_released()) {
                let _ = egui::DragAndDrop::take_payload::<crate::bin_panel::BinDrag>(ui.ctx());
                let at = self.snap(x_to_tick(pos.x).max(self.time_floor()), None);
                self.place_media_at(media, Tick(at), Some((kind, lane)));
                self.dnd_hover = None;
            }
        }
        if let Some(pos) = self.dnd_hover
            && let Some(g) = self.tl_geom.clone()
            && g.in_lanes(pos)
            && let Some((kind, lane)) = g.lane_at_y(pos.y)
        {
            let top = g.lane_top(kind, lane);
            let lane_h = g.lane_h_of(kind, lane);
            let lane_rect = egui::Rect::from_min_max(
                egui::pos2(rect.left(), top),
                egui::pos2(rect.right(), top + lane_h - 2.0),
            );
            painter.rect(
                lane_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(96, 200, 140, 36),
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(140, 235, 170, 160)),
                egui::StrokeKind::Inside,
            );
            let at = self.snap(g.x_to_tick(pos.x).max(self.time_floor()), None);
            let x0 = g.tick_to_x(at);
            let ghost_w = (((90_000.0 * 3.0) / self.ticks_per_px) as f32).max(3.0); // ~3 s, min 3 px
            let ghost = egui::Rect::from_min_max(
                egui::pos2(x0, top + 1.0),
                egui::pos2((x0 + ghost_w).min(rect.right()), top + lane_h - 3.0),
            );
            painter.rect(
                ghost,
                4.0,
                egui::Color32::from_rgba_unmultiplied(140, 235, 170, 44),
                egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(160, 240, 180, 220)),
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [egui::pos2(x0, rect.top() + RULER_H), egui::pos2(x0, rect.bottom())],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(160, 240, 180, 180)),
            );
        }

        // ---- vertical scrollbar (item 12 round 6) ----
        if vl.scroll_max > 0.0 {
            let track = egui::Rect::from_min_max(
                egui::pos2(rect.right() - 7.0, rect.top() + RULER_H + 2.0),
                egui::pos2(rect.right() - 1.0, rect.bottom() - 2.0),
            );
            let usable = track.height();
            let frac_vis = (usable / vl.content_h).clamp(0.05, 1.0);
            let knob_h = (usable * frac_vis).max(24.0);
            let frac_pos = (self.tl_scroll / vl.scroll_max).clamp(0.0, 1.0);
            let knob_y = track.top() + frac_pos * (usable - knob_h);
            let knob = egui::Rect::from_min_size(
                egui::pos2(track.left(), knob_y),
                egui::vec2(track.width(), knob_h),
            );
            let resp =
                ui.interact(track, ui.id().with("tl-vscroll"), egui::Sense::click_and_drag());
            if resp.dragged()
                && let Some(p) = resp.interact_pointer_pos()
            {
                let f = ((p.y - track.top() - knob_h / 2.0) / (usable - knob_h).max(1.0))
                    .clamp(0.0, 1.0);
                self.tl_scroll = f * vl.scroll_max;
            }
            painter.rect_filled(track, 3.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 90));
            painter.rect_filled(
                knob,
                3.0,
                if resp.hovered() || resp.dragged() {
                    egui::Color32::from_gray(120)
                } else {
                    egui::Color32::from_gray(80)
                },
            );
        }

        // ---- bare overlaps (R.51, item 399): the zone is marked with hatching ----
        // The target of the RMB gesture “Transition from…” must be visible (rule 16);
        // before this, an accidental overlap was indistinguishable from the start of a clip,
        // and the neighbour's tail under it SOUNDED (a minus from F-TL-010).
        for &(kind, lane, w0, w1) in overlap_zones.iter() {
            let (x0, x1) = (tick_to_x(w0), tick_to_x(w1));
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let top = lane_top(kind, lane);
            let lane_h = lane_h_at(kind, lane);
            let r = transition_rect(x0, x1, top, lane_h);
            let clipped = painter.with_clip_rect(r.intersect(rect));
            let st =
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 34));
            let mut x = r.left() - r.height();
            while x < r.right() {
                clipped.line_segment(
                    [egui::pos2(x, r.bottom()), egui::pos2(x + r.height(), r.top())],
                    st,
                );
                x += 7.0;
            }
        }

        // ---- TAIL WITHOUT MATERIAL (R.63 item 2): hatching ----
        //
        // An event can be longer than the file (the edge was dragged past the end,
        // the speed was changed) — then beyond this line there is silence and a freeze frame. While the tail
        // did not differ from the material in any way, it looked like “the loop breaks
        // the sound”: the person saw an event, but there was no sound in it. We show
        // it with the same technique as the overlap zone, and do NOT touch the model (rule
        // 20 — nothing is switched off by itself).
        {
            let p = self.model.project();
            for c in &p.clips {
                let Some(end) = kt_model::material_end_on_timeline(c, p.media_duration(c.media))
                else {
                    continue;
                };
                let body_end = (c.start + c.duration).0;
                if end >= body_end {
                    continue;
                }
                let (x0, x1) = (tick_to_x(end), tick_to_x(body_end));
                if x1 < rect.left() || x0 > rect.right() {
                    continue;
                }
                let (kind, lane) = kt_model::lane_key(c);
                let kind = match kind {
                    LaneKind::Audio => ClipKind::Audio,
                    _ => ClipKind::Video,
                };
                let top = lane_top(kind, lane);
                let r = egui::Rect::from_min_max(
                    egui::pos2(x0, top),
                    egui::pos2(x1, top + lane_h_at(kind, lane) - 2.0),
                );
                let clipped = painter.with_clip_rect(r.intersect(rect));
                let st = egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 190, 120, 40),
                );
                let mut x = r.left() - r.height();
                while x < r.right() {
                    clipped.line_segment(
                        [egui::pos2(x, r.bottom()), egui::pos2(x + r.height(), r.top())],
                        st,
                    );
                    x += 7.0;
                }
                crate::widgets::field_probe::note(&format!("clip.{}.no_material", c.id.0), r);
            }
        }

        // ---- transitions (R.24, item 140): backing + X + seam tick ----
        {
            let p = self.model.project();
            for t in &p.transitions {
                // fade anchor (R.25) — the only live end, the seam is at it
                let Some(a) = t.a.or(t.b).and_then(|id| p.clips.iter().find(|c| c.id == id)) else {
                    continue;
                };
                let Some(base_joint) = t.joint(p).map(|j| j.0) else { continue };
                let joint = match &self.drag {
                    Some(Drag::TransitionBody { id, cur_at, .. }) if *id == t.id => *cur_at,
                    _ => base_joint,
                };
                // the fade handle is dragged — the window is drawn by the LIVE length (item 12)
                let (in_off, out_off) = match &self.drag {
                    Some(Drag::Fade { id, left, len, .. })
                        if t.is_fade()
                            && ((*left && t.b == Some(*id) && t.a.is_none())
                                || (!*left && t.a == Some(*id) && t.b.is_none())) =>
                    {
                        if *left {
                            (0, *len)
                        } else {
                            (*len, 0)
                        }
                    }
                    _ => (t.in_offset.0, t.out_offset.0),
                };
                // AN OVERLAPPING transition (R.51, item 399): the window is the overlap zone itself,
                // there are no “handles” (the material of both sides is real)
                let staggered = t.overlap_pair(p).and_then(|_| t.window(p));
                let (x0, x1) = match &staggered {
                    Some(w) => (tick_to_x(w.start.0), tick_to_x(w.end.0)),
                    None => (tick_to_x(joint - in_off), tick_to_x(joint + out_off)),
                };
                if x1 < rect.left() || x0 > rect.right() {
                    continue;
                }
                let top = lane_top(a.kind, a.lane);
                let lane_h = lane_h_at(a.kind, a.lane);
                let r = transition_rect(x0, x1, top, lane_h);
                let _ = (in_off, out_off);
                if staggered.is_none() {
                    // handles (R.26): the neighbours' material continues past the seam and
                    // has not gone anywhere — pull the clips apart and it comes back
                    let xj = tick_to_x(joint);
                    let (left, right) = transition_handle_rects(x0, xj, x1, top, lane_h);
                    let handle_fill = |id: Option<ClipId>| {
                        let c = id.and_then(|i| p.clips.iter().find(|c| c.id == i))?;
                        let base = media_paths
                            .get(&c.media)
                            .map(|path| clip_color(path, c.kind))
                            .unwrap_or(egui::Color32::from_gray(80));
                        Some(egui::Color32::from_rgba_unmultiplied(
                            base.r(),
                            base.g(),
                            base.b(),
                            70,
                        ))
                    };
                    // to the left of the seam the INCOMING shows through, to the right — the outgoing one
                    if let Some(fill) = handle_fill(t.b) {
                        painter.rect_filled(left, 2.0, fill);
                    }
                    if let Some(fill) = handle_fill(t.a) {
                        painter.rect_filled(right, 2.0, fill);
                    }
                }
                painter.rect_filled(
                    r,
                    2.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22),
                );
                let st = egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 130),
                );
                // crossfade — X (two opposing diagonals), fade — ONE
                // diagonal in the direction of the rise (R.25)
                match (t.a.is_some(), t.b.is_some()) {
                    (true, true) => {
                        painter.line_segment([r.left_bottom(), r.right_top()], st);
                        painter.line_segment([r.left_top(), r.right_bottom()], st);
                    }
                    // fade-in: empty on the left, the picture appears to the right
                    (false, true) => {
                        painter.line_segment([r.left_bottom(), r.right_top()], st);
                    }
                    // fade-out: fades to the right
                    _ => {
                        painter.line_segment([r.left_top(), r.right_bottom()], st);
                    }
                }
                // the transition kind is visible ON THE STRIP (R.33 C4): without this the 16
                // kinds would differ only by the properties window. The name
                // is written into the transition window itself — we do not make a separate strip
                // for it (rule “do not waste space”)
                if t.kind != TransitionKind::Crossfade {
                    let name = crate::transition_editor::kind_name(&t.kind);
                    let font = egui::FontId::proportional((lane_h * 0.32).clamp(8.0, 11.0));
                    let g = painter.layout_no_wrap(
                        name.to_string(),
                        font,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210),
                    );
                    if g.size().x + 6.0 <= r.width() {
                        // a backing under the text: otherwise the transition diagonals
                        // show through the letters
                        let at = egui::pos2(
                            r.center().x - g.size().x / 2.0,
                            r.center().y - g.size().y / 2.0,
                        );
                        painter.rect_filled(
                            egui::Rect::from_min_size(at, g.size()).expand(2.0),
                            2.0,
                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 150),
                        );
                        painter.galley(at, g, egui::Color32::WHITE);
                    } else {
                        // if it does not fit — a marker dot: “not a dissolve here”
                        painter.circle_filled(
                            r.center(),
                            2.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210),
                        );
                    }
                }
                // the seam tick — only on a butt-joined window: an overlapping
                // (R.51) transition has no seam, the window is the overlap itself
                if staggered.is_none() {
                    let xj = tick_to_x(joint);
                    painter.line_segment(
                        [egui::pos2(xj, r.top()), egui::pos2(xj, r.bottom())],
                        egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 80),
                        ),
                    );
                }
            }
        }

        // preview of a fade NOT YET created and the handles themselves (item 12)
        {
            let p = self.model.project();
            if let Some(Drag::Fade { id, left, len, moved: true }) = &self.drag
                && *len > 0
                && corner_fade_len(p, *id, *left).is_none()
                && let Some(c) = p.clips.iter().find(|c| c.id == *id)
            {
                let top = lane_top(c.kind, c.lane);
                let lane_h = lane_h_at(c.kind, c.lane);
                let (s0, e0) = (c.start.0, c.start.0 + c.duration.0);
                let (a, b) = if *left { (s0, s0 + *len) } else { (e0 - *len, e0) };
                let r = transition_rect(tick_to_x(a), tick_to_x(b), top, lane_h);
                painter.rect_filled(
                    r,
                    2.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22),
                );
                let st = egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 130),
                );
                if *left {
                    painter.line_segment([r.left_bottom(), r.right_top()], st);
                } else {
                    painter.line_segment([r.left_top(), r.right_bottom()], st);
                }
            }
            // The handles are visible on the SELECTED event and on the one UNDER
            // the pointer (R.40 item 12): the fade zone works on any event
            // always, but the handles were drawn only on the selected one — a person
            // saw the level line, did not see the handles, aimed at the line and hit
            // an invisible zone. The width threshold is removed: the handle shrinks along with
            // the grab (`fade_grab_for_width`), rather than disappearing.
            let handle_on: Vec<ClipId> =
                self.selected_one().into_iter().chain(hover_clip).collect();
            for c in p.clips.iter().filter(|c| handle_on.contains(&c.id)) {
                let top = lane_top(c.kind, c.lane);
                let (s0, e0) = (c.start.0, c.start.0 + c.duration.0);
                {
                    for left in [true, false] {
                        let len = corner_fade_len(p, c.id, left).unwrap_or(0);
                        let x = tick_to_x(fade_handle_tick(s0, e0, len, left));
                        let y = top + 2.0;
                        let d = 5.0f32;
                        let dx = if left { d } else { -d };
                        painter.add(egui::Shape::convex_polygon(
                            vec![egui::pos2(x, y), egui::pos2(x + dx, y), egui::pos2(x, y + d)],
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 190),
                            egui::Stroke::NONE,
                        ));
                    }
                }
            }
        }

        // ---- markers ----------------------------------------------------------------
        for m in &self.model.project().markers {
            let mt = match &self.drag {
                Some(Drag::Marker { orig, cur }) if *orig == m.t.0 => *cur,
                Some(Drag::MarkerEdge { start, left: true, cur }) if *start == m.t.0 => {
                    (*cur).clamp(0, m.end().0)
                }
                _ => m.t.0,
            };
            let mdur = match &self.drag {
                // the edge of the range is dragged — live preview
                Some(Drag::MarkerEdge { start, left: false, cur }) if *start == m.t.0 => {
                    (*cur - m.t.0).max(0)
                }
                Some(Drag::MarkerEdge { start, left: true, cur }) if *start == m.t.0 => {
                    m.end().0 - (*cur).clamp(0, m.end().0)
                }
                _ => m.duration.0,
            };
            let x = tick_to_x(mt);
            let xe = tick_to_x(mt + mdur);
            if xe < rect.left() || x > rect.right() {
                continue;
            }
            let amber = egui::Color32::from_rgb(255, 195, 0);
            let top = rect.top() + TIME_TOP;
            // range marker (R.24, marked_range): a ribbon on the row of
            // time labels (does not take a new place, checklist item 13) +
            // a semi-transparent end boundary
            if mdur > 0 {
                full_painter.rect_filled(
                    marker_band_rect(x, xe, top),
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 195, 0, 70),
                );
                full_painter.line_segment(
                    [egui::pos2(xe, top), egui::pos2(xe, rect.bottom())],
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 195, 0, 120)),
                );
            }
            full_painter.line_segment(
                [egui::pos2(x, top), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.0, amber),
            );
            full_painter.add(egui::Shape::convex_polygon(
                vec![egui::pos2(x - 5.0, top), egui::pos2(x + 5.0, top), egui::pos2(x, top + 8.0)],
                amber,
                egui::Stroke::NONE,
            ));
            // the marker name — ON THE SAME ROW as the time labels, to the right of
            // the flag (item 3 round 12: a separate strip for markers stood empty).
            // It is drawn after the ruler, so the amber name covers
            // the grey time digit, and not the other way round.
            if !m.name.is_empty() {
                let font = egui::FontId::proportional(10.5);
                let g = full_painter.layout_no_wrap(m.name.clone(), font, amber);
                let at = egui::pos2(x + 7.0, top + 1.0);
                // backing: the name lies over the time digit, and without it they
                // merge into a mess (checked live, item 3 round 12)
                full_painter.rect_filled(
                    egui::Rect::from_min_size(at, g.size()).expand2(egui::vec2(2.0, 0.0)),
                    2.0,
                    egui::Color32::from_rgba_unmultiplied(12, 13, 16, 235),
                );
                full_painter.galley(at, g, amber);
            }
        }

        // ---- inline marker name entry (item 1 round 10) --------------------------
        if let Some(ed) = self.marker_edit.clone() {
            let x = tick_to_x(ed.at);
            let mut buf = ed.buf.clone();
            let mut done: Option<bool> = None; // Some(true) — save
            egui::Area::new(egui::Id::new("marker-name-edit"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(x + 6.0, rect.top() + TIME_TOP))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut buf)
                                .hint_text(if ed.is_new {
                                    tr(k::tl::field::MARKER_NAME_NEW)
                                } else {
                                    tr(k::tl::field::MARKER_NAME)
                                })
                                .desired_width(140.0),
                        );
                        if !r.has_focus() && self.marker_edit_focus {
                            r.request_focus();
                            self.marker_edit_focus = false;
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            done = Some(true);
                        } else if ui.input(|i| i.key_pressed(egui::Key::Escape))
                            || (r.lost_focus() && !r.has_focus())
                        {
                            // click outside / Esc: the marker simply stays without a name
                            done = Some(false);
                        }
                    });
                });
            match done {
                Some(true) => {
                    let name = buf.trim().to_string();
                    self.marker_edit = None;
                    if !name.is_empty() {
                        self.apply(Command::SetMarkerName { at: Tick(ed.at), name });
                    }
                }
                Some(false) => {
                    // a new marker stays without a name; when renaming,
                    // Esc / click outside is just a cancel, the old name stays in place
                    self.marker_edit = None;
                }
                None => self.marker_edit = Some(crate::app::MarkerEdit { buf, ..ed }),
            }
        }

        // ---- inline track name entry (item 2 round 10) ------------------------
        if let Some(ed) = self.lane_edit.clone() {
            let y = geom
                .rows
                .iter()
                .position(|&(k, l)| k == ed.kind && l == ed.lane)
                .map(|row| geom.row_top(row))
                .unwrap_or(rect.top());
            let mut buf = ed.buf.clone();
            let mut done: Option<bool> = None;
            egui::Area::new(egui::Id::new("lane-name-edit"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(full.left() + 2.0, y + 2.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut buf)
                                .hint_text(tr(k::tl::field::LANE_NAME))
                                .desired_width(120.0),
                        );
                        if !r.has_focus() && self.lane_edit_focus {
                            r.request_focus();
                            self.lane_edit_focus = false;
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            done = Some(true);
                        } else if ui.input(|i| i.key_pressed(egui::Key::Escape))
                            || (r.lost_focus() && !r.has_focus())
                        {
                            done = Some(false);
                        }
                    });
                });
            match done {
                Some(true) => {
                    // the name is the CONTENT of the track (R.62): one history step
                    let name = buf.trim().to_string();
                    if let Some(t) = self.model.project().track_at(ed.kind, ed.lane) {
                        let (id, mut props) = (t.id, t.props());
                        props.name = name;
                        if props != t.props() {
                            self.apply(Command::SetTrackProps { id, props });
                        }
                    }
                    self.lane_edit = None;
                }
                Some(false) => self.lane_edit = None,
                None => self.lane_edit = Some(crate::app::LaneEdit { buf, ..ed }),
            }
        }

        // ---- renaming a region (R.38 item 1) -----------------------------
        // The same widget and the same behaviour as the track name: the field sits on
        // the entity itself, Enter applies, Esc cancels, focus is RETURNED
        // on closing (rule 14 — otherwise the hotkeys die).
        if let Some(ed) = self.area_edit.clone() {
            let anchor = self
                .model
                .project()
                .areas
                .iter()
                .find(|a| a.name == ed.name)
                .and_then(|a| geom.area_rect(a))
                .map(|r| r.left_top())
                .unwrap_or(rect.left_top());
            let mut buf = ed.buf.clone();
            let mut done: Option<bool> = None;
            egui::Area::new(egui::Id::new("area-name-edit"))
                .order(egui::Order::Foreground)
                .fixed_pos(anchor + egui::vec2(2.0, 2.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut buf)
                                .hint_text(tr(k::tl::field::AREA_NAME))
                                .desired_width(140.0),
                        );
                        if !r.has_focus() && self.area_edit_focus {
                            r.request_focus();
                            self.area_edit_focus = false;
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            done = Some(true);
                        } else if ui.input(|i| i.key_pressed(egui::Key::Escape))
                            || (r.lost_focus() && !r.has_focus())
                        {
                            done = Some(false);
                        }
                    });
                });
            match done {
                Some(true) => {
                    let to = buf.trim().to_string();
                    if !to.is_empty() && to != ed.name {
                        self.apply(Command::RenameArea { from: ed.name.clone(), to });
                    }
                    ui.ctx().memory_mut(|m| m.surrender_focus(egui::Id::new("area-name-edit")));
                    self.area_edit = None;
                }
                Some(false) => self.area_edit = None,
                None => self.area_edit = Some(crate::app::AreaEdit { buf, ..ed }),
            }
        }

        // ---- jump/edge-scroll heat zones: gradient on hover ------------------
        if let Some(pos) = hover {
            let boost = if self.drag.is_some() { 1.0f32 } else { 0.6 };
            let margin = edge_margin;
            let warm = |heat: f32, zone: egui::Rect, from_left: bool| {
                if heat > 0.02 {
                    let a = ((36.0 + heat * 174.0) * boost) as u8;
                    let (c0, c1) = if from_left {
                        (
                            egui::Color32::from_rgba_unmultiplied(255, 178, 100, a),
                            egui::Color32::TRANSPARENT,
                        )
                    } else {
                        (
                            egui::Color32::TRANSPARENT,
                            egui::Color32::from_rgba_unmultiplied(255, 178, 100, a),
                        )
                    };
                    let mut mesh = egui::Mesh::default();
                    for (p, c) in [
                        (zone.left_top(), c0),
                        (zone.right_top(), c1),
                        (zone.right_bottom(), c1),
                        (zone.left_bottom(), c0),
                    ] {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos: p,
                            uv: egui::epaint::WHITE_UV,
                            color: c,
                        });
                    }
                    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                    full_painter.add(egui::Shape::mesh(mesh));
                }
            };
            let lh = ((rect.left() + margin - pos.x) / margin).clamp(0.0, 1.0);
            let rh = ((pos.x - (rect.right() - margin)) / margin).clamp(0.0, 1.0);
            // the left zone starts from the EDGE of the panel (through the gutter): the hot
            // end of the gradient runs into the physical edge of the window, and does not
            // stop at the gutter seam (item 20 round 6)
            warm(
                lh,
                egui::Rect::from_min_max(
                    egui::pos2(full.left(), rect.top() + RULER_H),
                    egui::pos2(rect.left() + margin * 0.6, rect.bottom()),
                ),
                true,
            );
            warm(
                rh,
                egui::Rect::from_min_max(
                    egui::pos2(rect.right() - margin * 0.6, rect.top() + RULER_H),
                    egui::pos2(rect.right(), rect.bottom()),
                ),
                false,
            );
        }

        // ---- track name strip: semi-transparent gradient OVER everything --
        // (items 4-5 round 7.1) Drawn AFTER the clips and heat zones: the timeline and
        // cache strips show through, the heat gradient flows under it from the left
        // without a hard boundary; the names themselves are solid and readable.
        {
            let g = crate::theme::GUTTER;
            let solid = egui::Color32::from_rgba_unmultiplied(g.r(), g.g(), g.b(), 165);
            let mid = egui::Color32::from_rgba_unmultiplied(g.r(), g.g(), g.b(), 85);
            let split = gutter.left() + gutter.width() * 0.45;
            crate::theme::hgrad(
                &full_painter,
                egui::Rect::from_min_max(gutter.min, egui::pos2(split, gutter.bottom())),
                solid,
                mid,
            );
            crate::theme::hgrad(
                &full_painter,
                egui::Rect::from_min_max(egui::pos2(split, gutter.top()), gutter.max),
                mid,
                egui::Color32::TRANSPARENT,
            );
            // item 10 round 11: a renamed track does not fit into the 34-pixel groove —
            // before, the text was centred in it and spilled out on BOTH
            // sides: cut off by the window edge on the left, ran onto the clips on the right.
            // Now: if it fits — as before; if not — the label from the left edge
            // over the START of the track, on its own backing, truncated.
            let col = egui::Color32::from_gray(175);
            let font = egui::FontId::proportional(12.0);
            for (label, cy, row_top, row_h) in &gutter_labels {
                let font = if *row_h < 20.0 {
                    // a thin service row: the font is sized to its height, otherwise
                    // the caption does not fit and runs into the neighbour
                    egui::FontId::proportional((row_h * 0.62).clamp(7.0, 12.0))
                } else {
                    font.clone()
                };
                let fit = full_painter.layout_no_wrap(label.clone(), font.clone(), col);
                if fit.size().x <= gutter.width() - 4.0 {
                    full_painter.text(
                        egui::pos2(gutter.left() + gutter.width() * 0.42, *cy),
                        egui::Align2::CENTER_CENTER,
                        label,
                        font.clone(),
                        col,
                    );
                    continue;
                }
                let mut job = egui::text::LayoutJob::single_section(
                    label.clone(),
                    egui::TextFormat { font_id: font.clone(), color: col, ..Default::default() },
                );
                job.wrap = egui::text::TextWrapping {
                    max_width: LANE_LABEL_MAX_W,
                    max_rows: 1,
                    break_anywhere: true,
                    overflow_character: Some('…'),
                };
                let galley = full_painter.layout_job(job);
                let x = gutter.left() + 3.0;
                let _ = &galley;
                // by a row BELOW the clip name; on a low track we press against
                // its bottom, but still under the clip name (item 2 round 13)
                let y = lane_label_y(*row_top, *row_h, galley.size().y);
                let plate = egui::Rect::from_min_size(
                    egui::pos2(x - 3.0, y - 1.0),
                    egui::vec2(galley.size().x + 12.0, galley.size().y + 2.0),
                );
                crate::theme::hgrad(&full_painter, plate, solid, egui::Color32::TRANSPARENT);
                full_painter.galley(egui::pos2(x, y), galley, col);
            }
        }

        // ---- TRACK BADGES (R.62): M / S / lock / FX under the name ---------------
        //
        // The row sits in an already occupied spot (rule 13), and its width
        // fixes the header strip: when the state changes nothing
        // jumps (rule 3). The grab zone is `BADGE_W`, the drawing smaller
        // (rule 1). A probe per badge: the live scenario and the harness
        // press exactly where the person presses (R.40).
        //
        // THE WINDOW EDGE DOES NOT STEAL CLICKS (rule 15). Track headers
        // are pressed against the left edge of the window, and the first 8 px there are the resize zone:
        // without this record the left half of the M badge is dead when the media bin
        // is hidden. We collect into a local list — `track` holds a borrow of the project.
        let mut no_resize: Vec<egui::Rect> = Vec::new();
        for (row, (kind, lane)) in geom.rows.iter().copied().enumerate() {
            let (row_top, row_h) = (geom.row_top(row), geom.row_h(row));
            if row_top + row_h < rect.top() || row_top > rect.bottom() {
                continue;
            }
            // the row of an EXPANDED strip has foreign properties — no badges
            if self.model.band_of_row(kind, lane).is_some() {
                continue;
            }
            let Some(track) = self.model.project().track_at(kind, lane) else { continue };
            // a GESTURE OBSTACLE — the track lock (R.62): we highlight the
            // row that was run into, exactly like an event obstacle
            if self
                .blocked_track
                .is_some_and(|(id, at)| {
                    id == track.id
                        && (at.elapsed().as_millis() as u64) < crate::app::STATUS_TTL_MS
                })
            {
                full_painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(gutter.left(), row_top),
                        egui::pos2(rect.right(), row_top + row_h - 2.0),
                    ),
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(200, 60, 60, 46),
                );
            }
            for (badge, r) in TrackBadge::row_rects(kind, gutter.left(), row_top, row_h) {
                let on = badge.on(track);
                let (bg, fg) = if on {
                    (crate::theme::BADGE_ON, egui::Color32::from_gray(20))
                } else {
                    (egui::Color32::TRANSPARENT, egui::Color32::from_gray(120))
                };
                let inner = r.shrink(1.5);
                if on {
                    full_painter.rect_filled(inner, 2.0, bg);
                }
                full_painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    badge.glyph(),
                    egui::FontId::proportional((r.height() * 0.62).clamp(7.0, 11.0)),
                    fg,
                );
                // The probe name carries the KIND of the row: V1 and A1 are different tracks,
                // and without the kind letter the scenario would press the first one it found.
                no_resize.push(r);
                crate::widgets::field_probe::note(
                    &format!(
                        "track.{}.{:?}",
                        kt_model::default_lane_label(kind, lane),
                        badge
                    )
                    .to_lowercase(),
                    r,
                );
            }
        }

        self.no_resize_rects.extend(no_resize);

        // ---- gesture obstacle (item 5) ----------------------------------------------
        // “I have not scrolled there myself — did not realise there was still an
        // event at the end that gets in the way”: what was run into
        // is highlighted in RED, and if it lies outside the view — an arrow at the edge
        // of the row points in its direction.
        if let Some(b) = self.blocked {
            let top = lane_top(b.kind, b.lane);
            let y0 = top + 1.0;
            let y1 = top + lane_h_at(b.kind, b.lane) - 3.0;
            let err = egui::Color32::from_rgb(255, 105, 97);
            match block_view(b.span, (x_to_tick(rect.left()), x_to_tick(rect.right()))) {
                BlockView::Visible => {
                    let bx0 = tick_to_x(b.span.0).max(rect.left());
                    let bx1 = tick_to_x(b.span.1).min(rect.right());
                    let r = egui::Rect::from_min_max(egui::pos2(bx0, y0), egui::pos2(bx1, y1));
                    full_painter.rect_filled(
                        r,
                        4.0,
                        egui::Color32::from_rgba_unmultiplied(255, 105, 97, 48),
                    );
                    full_painter.rect(
                        r,
                        4.0,
                        egui::Color32::TRANSPARENT,
                        egui::Stroke::new(2.0, err),
                        egui::StrokeKind::Inside,
                    );
                }
                dir => {
                    // arrow at the edge of the row: where to look
                    let left = matches!(dir, BlockView::Left(_));
                    let cy = (y0 + y1) * 0.5;
                    let tip = if left { rect.left() + 3.0 } else { rect.right() - 3.0 };
                    let back = if left { tip + 14.0 } else { tip - 14.0 };
                    full_painter.add(egui::Shape::convex_polygon(
                        vec![
                            egui::pos2(tip, cy),
                            egui::pos2(back, cy - 7.0),
                            egui::pos2(back, cy + 7.0),
                        ],
                        err,
                        egui::Stroke::NONE,
                    ));
                }
            }
        }

        // ---- snap line / playhead -----------------------------------------------
        if let Some(t) = self.snap_line {
            let x = tick_to_x(t);
            full_painter.line_segment(
                [egui::pos2(x, rect.top() + RULER_H), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 220, 120)),
            );
        }
        // the cursor is the consumer of the position for DISPLAY: the display clock, like the preview and
        // the toolbar timecode (invariant 12; the raw transport.playhead() during play
        // jumps in callback quanta and diverges from what is heard by the latency)
        let ph = self.player.video_playhead();
        let x = tick_to_x(ph.0);
        if x >= rect.left() && x <= rect.right() {
            // Cursor glow (item 4 round 5, redone item 5 round 13): does NOT extend onto
            // the cache strips (it starts from the time strip) and shines ONLY from
            // the side of motion — a trail behind the cursor: forward on the left,
            // in reverse on the right. On pause the side is taken from the last sign of
            // the speed, so that it does not flicker at a stop.
            let glow_half = 12.0f32;
            let mut mesh = egui::Mesh::default();
            let c0 = egui::Color32::from_rgba_unmultiplied(255, 60, 60, 0);
            let c1 = egui::Color32::from_rgba_unmultiplied(255, 60, 60, 90);
            let top_y = rect.top() + TIME_TOP;
            let trail = if self.play_dir_back {
                [(x, x + glow_half, c1, c0)]
            } else {
                [(x - glow_half, x, c0, c1)]
            };
            for (xa, xb, ca, cb) in trail {
                let i = mesh.vertices.len() as u32;
                for (px, col) in [
                    (egui::pos2(xa, top_y), ca),
                    (egui::pos2(xb, top_y), cb),
                    (egui::pos2(xb, rect.bottom()), cb),
                    (egui::pos2(xa, rect.bottom()), ca),
                ] {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: px,
                        uv: egui::epaint::WHITE_UV,
                        color: col,
                    });
                }
                mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
            }
            full_painter.add(egui::Shape::mesh(mesh));
            full_painter.line_segment(
                [egui::pos2(x, rect.top() + TIME_TOP), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 100, 100)),
            );
            full_painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(x - 6.0, rect.top() + TIME_TOP),
                    egui::pos2(x + 6.0, rect.top() + TIME_TOP),
                    egui::pos2(x, rect.top() + TIME_TOP + 9.0),
                ],
                egui::Color32::from_rgb(255, 80, 80),
                egui::Stroke::NONE,
            ));
        }

        // ---- context menu ----------------------------------------------------------
        let target = self.ctx_target;
        response.context_menu(|ui| match target {
            Some(CtxTarget::Clip(id))
            | Some(CtxTarget::Nest(id))
            | Some(CtxTarget::Generator(id)) => {
                let (is_audio, has_group) = self
                    .model
                    .clip(id)
                    .map(|c| (c.kind == ClipKind::Audio, c.link_group.is_some()))
                    .unwrap_or((false, false));
                // BY NUMBER — the second path to the level (R.40 item 12, verbatim: “on a
                // small event I try to catch opacity there but it does not
                // work”). Until this round the five-pixel line was the
                // ONLY way to set opacity in the whole programme.
                // The field shows and edits EXACTLY what the line does:
                // for video — opacity, for audio — gain in dB.
                if let Some(c) = self.model.clip(id).cloned() {
                    ui.horizontal(|ui| {
                        if is_audio {
                            let mut db = c.gain_db as f64;
                            ui.label(tr(k::tl::menu::LEVEL));
                            // the limit is the v1 volume rail (RAIL_DB_SPAN), declared once
                            // and only used here
                            if crate::widgets::plain_row(
                                ui,
                                &crate::widgets::NumRow {
                                    label: "",
                                    probe: "clip.gain",
                                    unit: kt_model::ParamUnit::Db,
                                    min: -(RAIL_DB_SPAN as f64) / 2.0,
                                    max: RAIL_DB_SPAN as f64 / 2.0,
                                    step: 0.1,
                                    slider: None,
                                },
                                &mut db,
                            )
                            .changed()
                            {
                                self.apply_coalesced(
                                    Command::SetGain { id, gain_db: db as f32 },
                                    self.gesture_counter,
                                );
                            }
                        } else {
                            let mut op = c.opacity as f64;
                            ui.label(tr(k::tl::menu::LEVEL));
                            if crate::widgets::plain_row(
                                ui,
                                &crate::widgets::NumRow {
                                    label: "",
                                    probe: "clip.opacity",
                                    unit: kt_model::ParamUnit::Norm,
                                    min: 0.0,
                                    max: 1.0,
                                    step: 0.01,
                                    slider: None,
                                },
                                &mut op,
                            )
                            .changed()
                            {
                                self.apply_coalesced(
                                    Command::SetOpacity { id, opacity: op as f32 },
                                    self.gesture_counter,
                                );
                            }
                        }
                    });
                    ui.separator();
                }
                // COMPOSITION PARAMETERS — the first item for an instance
                // (R.59, stage 3): this is what the consumer reaches for the attachment
                // most often. Next to it — the author’s list: the pair
                // “fill” / “declare” lives in one place.
                if self.is_nest(id) {
                    let seq = self.model.clip(id).map(|c| c.media);
                    let r = ui.button(tr(k::pubp::INST_OPEN));
                    crate::widgets::field_probe::note("pub.inst_open", r.rect);
                    if r.clicked() {
                        // FX WINDOW ON THE HEAD rather than its own window (R.63 item 12):
                        // “composition parameters open in a separate window when they can be placed in the same
                        // fx-chain”,
                        // the owner’s ban on extra windows
                        self.open_fx_editor_head(id);
                        ui.close();
                    }
                    let r = ui.button(tr(k::pubp::LIST_OPEN));
                    crate::widgets::field_probe::note("pub.list_open", r.rect);
                    if r.clicked() {
                        if let Some(seq) = seq {
                            self.open_published_win(seq);
                        }
                        ui.close();
                    }
                    ui.separator();
                }
                // after the item's own selection — the same `copy_selection` as Ctrl+C.
                // Paste below shows NO chord: it pastes at the click, Ctrl+V at the playhead
                if crate::widgets::menu_item(ui, tr(k::tl::menu::COPY), Some("Ctrl+C")) {
                    if !self.is_selected(id) {
                        self.select_only(id);
                    }
                    self.copy_selection();
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::PASTE)).clicked() {
                    self.paste_at_click();
                    ui.close();
                }
                // a chord is promised ONLY when it really does this
                // (R.49, item 362): when a region is drawn, Del deletes the
                // contents of the region, not the selected event
                let del_accel = self.model.project().region.is_none().then_some("Del");
                if crate::widgets::menu_item(ui, tr(k::tl::menu::DELETE), del_accel) {
                    if !self.is_selected(id) {
                        self.select_only(id);
                    }
                    // exactly THIS clip (+ group), not the contents of the region (item 10)
                    self.delete_selected();
                    ui.close();
                }
                // sync by sound (R.49, item 375): visible where
                // the person works — on the events themselves
                if self.selected.len() == 2 && ui.button(tr(k::tl::menu::SYNC_AUDIO)).clicked() {
                    self.sync_selected_by_audio();
                    ui.close();
                }
                // nested reels (R.50). Collapse can be applied to anything —
                // the item sits on the event; enter, expand, unlink and
                // rename — only on the attachment ITSELF, and “unlink”
                // is shown only when there really is more than one place
                // item rectangles — outward for live scenarios
                // (R.49): the egui menu is not searched in the accessibility tree,
                // and the scenario must not guess offsets
                // generator (R.52 → R.59, stage 2): the values live on the
                // EVENT, and they are edited by the first crumb of its own effects
                // window — a separate generator window no longer exists. The same
                // item is in the bin card menu (rule 17).
                if self.model.clip(id).is_some_and(|c| c.r#gen.is_some()) {
                    let r = ui.button(tr(k::tl::menu::GEN_PARAMS));
                    crate::widgets::field_probe::note("gen.params", r.rect);
                    if r.clicked() {
                        self.open_fx_editor_head(id);
                        ui.close();
                    }
                }
                if self.is_nest(id) {
                    // EXPAND HERE is listed higher: this is the default mode, and
                    // the order of the items must match it
                    let open = self.model.bands().iter().any(|b| b.inst == id);
                    let label = if open {
                        k::tl::menu::NEST_COLLAPSE_HERE
                    } else {
                        k::tl::menu::NEST_EXPAND_HERE
                    };
                    // A chord is promised ONLY where it really does this
                    // (R.49, item 362): Enter on a collapsed one expands it,
                    // Enter on an expanded one enters it, and Esc collapses.
                    let accel = if open {
                        tr(k::tl::menu::NEST_COLLAPSE_HERE_ACCEL)
                    } else {
                        tr(k::tl::menu::NEST_EXPAND_HERE_ACCEL)
                    };
                    let (hit, rect) = crate::widgets::menu_item_probed(ui, tr(label), Some(accel));
                    crate::widgets::field_probe::note("nest.expand_here", rect);
                    if hit {
                        if open {
                            self.collapse_in_place(id);
                        } else {
                            self.expand_in_place(id);
                        }
                        ui.close();
                    }
                    let enter_accel = open.then(|| tr(k::tl::menu::NEST_ENTER_ACCEL));
                    let (hit, rect) = crate::widgets::menu_item_probed(
                        ui,
                        tr(k::tl::menu::NEST_ENTER),
                        enter_accel,
                    );
                    crate::widgets::field_probe::note("nest.enter", rect);
                    if hit {
                        self.enter_nest(id);
                        ui.close();
                    }
                }
                let r = ui.button(tr(k::tl::menu::COLLAPSE));
                crate::widgets::field_probe::note("nest.collapse", r.rect);
                if r.clicked() {
                    if !self.is_selected(id) {
                        self.select_only(id);
                    }
                    self.collapse_selection();
                    ui.close();
                }
                if let Some(seq) =
                    self.model.clip(id).map(|c| c.media).filter(|m| self.nest_uses(*m) > 0)
                    && self.model.project().sequence(seq).is_some()
                {
                    let r = ui.button(tr(k::tl::menu::NEST_EXPAND));
                    crate::widgets::field_probe::note("nest.expand", r.rect);
                    if r.clicked() {
                        self.expand_nest(id);
                        ui.close();
                    }
                    let uses = self.nest_uses(seq);
                    // “×N” without a continuation would be a message, not a
                    // verb: this is where to go from it
                    if uses > 1 {
                        let r = crate::widgets::menu_item(
                            ui,
                            tr(k::tl::menu::NEST_NEXT_PLACE),
                            Some(&format!("×{uses}")),
                        );
                        if r {
                            self.goto_next_place(id);
                            ui.close();
                        }
                    }
                    if uses > 1
                        && crate::widgets::menu_item(
                            ui,
                            tr(k::tl::menu::NEST_INDEPENDENT),
                            Some(&format!("×{uses}")),
                        )
                    {
                        self.nest_make_independent(id);
                        ui.close();
                    }
                    let r = ui.button(tr(k::tl::menu::NEST_RENAME));
                    crate::widgets::field_probe::note("nest.rename", r.rect);
                    if r.clicked() {
                        let name = self
                            .model
                            .project()
                            .sequence(seq)
                            .map(|s| s.name.clone())
                            .unwrap_or_default();
                        self.seq_name = Some(crate::app::SeqNameDialog { seq, name, focus: true });
                        ui.close();
                    }
                }
                if crate::widgets::menu_item(ui, tr(k::tl::menu::SPLIT), Some("S")) {
                    if !self.is_selected(id) {
                        self.select_only(id);
                    }
                    self.cut_action();
                    ui.close();
                }
                {
                    let r = ui.button(tr(k::tl::menu::FX));
                    crate::widgets::field_probe::note("tl.fx", r.rect);
                    if r.clicked() {
                        self.open_fx_editor(id);
                        ui.close();
                    }
                }
                if crate::widgets::menu_item(
                    ui,
                    tr(k::tl::menu::REGION_FROM_EVENT),
                    Some(tr(k::tl::menu::REGION_FROM_EVENT_ACCEL)),
                ) {
                    self.region_from_event(id);
                    ui.close();
                }
                let is_rev = self.model.clip(id).map(|c| c.reversed).unwrap_or(false);
                let rev_label = if is_rev {
                    format!("{} {}", tr(k::tl::menu::REVERSE), crate::icons::CHECK)
                } else {
                    tr(k::tl::menu::REVERSE).to_string()
                };
                let rev_r = ui.button(rev_label);
                crate::widgets::field_probe::note("tl.reverse", rev_r.rect);
                if rev_r.clicked() {
                    self.apply(Command::SetReversed { id, reversed: !is_rev });
                    ui.close();
                }
                ui.separator();
                if has_group {
                    if ui.button(tr(k::tl::menu::UNLINK)).clicked() {
                        if !self.is_selected(id) {
                            self.select_only(id);
                        }
                        self.unlink_selected();
                        ui.close();
                    }
                } else if ui.button(tr(k::tl::menu::LINK)).clicked() {
                    if !self.is_selected(id) {
                        self.select_only(id);
                    }
                    self.magnetize_selected();
                    ui.close();
                }
                // auto-ducking (R.48): counts the pauses on another track and
                // puts the volume curve onto the selected events
                // DUCKING lives in the TRACK window (R.63 item 8): it no longer has
                // its own window, and the menu item is a short path to the same place
                // (rule 17: a door has one body). The owner: “this could simply be done
                // more conveniently through fx-audio on the whole track”.
                if is_audio && ui.button(tr(k::tl::menu::DUCK)).clicked() {
                    let my_lane = self.model.clip(id).map(|c| c.lane).unwrap_or(0);
                    if let Some(t) = self.model.project().track_at(LaneKind::Audio, my_lane) {
                        let tid = t.id;
                        self.open_track_window(tid);
                    }
                    ui.close();
                }
                if is_audio && ui.button(tr(k::tl::menu::RESET_GAIN)).clicked() {
                    self.apply(Command::SetGain { id, gain_db: 0.0 });
                    ui.close();
                }
                // transitions (R.24): crossfade with the butted neighbour / remove
                {
                    let p = self.model.project();
                    let me = p.clips.iter().find(|c| c.id == id).cloned();
                    if let Some(me) = me {
                        // neighbour — butted OR “staircase” (R.51, item 399): one
                        // verb for both mechanisms (rule 17)
                        let me_end = me.start + me.duration;
                        let neighbor = |incoming: bool| {
                            p.clips
                                .iter()
                                .find(|c| {
                                    let c_end = c.start + c.duration;
                                    c.kind == me.kind
                                        && c.lane == me.lane
                                        && if incoming {
                                            c.start == me_end
                                                || (me.start < c.start
                                                    && c.start < me_end
                                                    && me_end < c_end)
                                        } else {
                                            c_end == me.start
                                                || (c.start < me.start
                                                    && me.start < c_end
                                                    && c_end < me_end)
                                        }
                                })
                                .map(|c| c.id)
                        };
                        let mut items: Vec<(bool, ClipId)> = Vec::new();
                        if let Some(n) = neighbor(true)
                            && !p.transitions.iter().any(|t| (t.a, t.b) == (Some(id), Some(n)))
                        {
                            items.push((true, n));
                        }
                        if let Some(n) = neighbor(false)
                            && !p.transitions.iter().any(|t| (t.a, t.b) == (Some(n), Some(id)))
                        {
                            items.push((false, n));
                        }
                        // (id, whether it is a fade, whether I am the outgoing side) — the exact
                        // name of the item is built from this triple (item 4)
                        let existing: Vec<(TransitionId, bool, bool)> = p
                            .transitions
                            .iter()
                            .filter(|t| t.touches(id))
                            .map(|t| (t.id, t.is_fade(), t.a == Some(id)))
                            .collect();
                        // fade at a free edge of the clip (R.25): there is no neighbour —
                        // the picture/sound appears out of nothing or fades out into it
                        let fades: Vec<bool> = [true, false]
                            .into_iter()
                            .filter(|&incoming| {
                                let occupied = p.transitions.iter().any(|t| {
                                    if incoming { t.a == Some(id) } else { t.b == Some(id) }
                                });
                                neighbor(incoming).is_none() && !occupied
                            })
                            .collect();
                        for (next, n) in items {
                            let label = if next {
                                tr(k::tl::menu::TRANSITION_NEXT)
                            } else {
                                tr(k::tl::menu::TRANSITION_PREV)
                            };
                            // the KIND is chosen at once, not “first fade,
                            // then redo it” — the claim of item 1 R.33.
                            // A submenu: dissolve comes first (the former
                            // one-click path stays short).
                            ui.menu_button(label, |ui| {
                                let mut pick: Option<TransitionKind> = None;
                                let mut cat: Option<crate::i18n::Key> = None;
                                for kind in TransitionKind::ALL {
                                    let c = crate::transition_editor::kind_category(kind);
                                    if cat != Some(c) {
                                        if cat.is_some() {
                                            ui.separator();
                                        }
                                        cat = Some(c);
                                    }
                                    if ui
                                        .button(crate::transition_editor::kind_name(kind))
                                        .clicked()
                                    {
                                        pick = Some(kind.clone());
                                    }
                                }
                                let Some(kind) = pick else { return };
                                let (a, b) = if next { (id, n) } else { (n, id) };
                                let clip = |i: ClipId| {
                                    self.model.project().clips.iter().find(|c| c.id == i).cloned()
                                };
                                if let (Some(ca), Some(cb)) = (clip(a), clip(b)) {
                                    // butted: the default window up to 0.5 s to
                                    // each side, clamped by the bodies; overlap
                                    // (R.51): the window is the overlap zone itself,
                                    // the offsets are not used
                                    let butt = ca.start + ca.duration == cb.start;
                                    let half = if butt {
                                        Tick(45_000)
                                            .min(Tick(ca.duration.0 / 2))
                                            .min(Tick(cb.duration.0 / 2))
                                            .max(MIN_TRANSITION_DURATION)
                                    } else {
                                        Tick::ZERO
                                    };
                                    self.apply(Command::AddTransition(TransitionEvent {
                                        id: TransitionId(self.next_transition_id()),
                                        a: Some(a),
                                        b: Some(b),
                                        in_offset: half,
                                        out_offset: half,
                                        kind,
                                        audio: Default::default(),
                                        auto: vec![],
                                        invert: false,
                                    }));
                                }
                                ui.close();
                            });
                        }
                        for incoming in fades {
                            let label = if incoming {
                                tr(k::tl::menu::FADE_OUT)
                            } else {
                                tr(k::tl::menu::FADE_IN)
                            };
                            if ui.button(label).clicked() {
                                let half = Tick(45_000)
                                    .min(Tick(me.duration.0 / 2))
                                    .max(MIN_TRANSITION_DURATION);
                                let (a, b) =
                                    if incoming { (Some(id), None) } else { (None, Some(id)) };
                                self.apply(Command::AddTransition(TransitionEvent {
                                    id: TransitionId(self.next_transition_id()),
                                    a,
                                    b,
                                    in_offset: half,
                                    out_offset: half,
                                    kind: TransitionKind::Crossfade,
                                    audio: Default::default(),
                                    auto: vec![],
                                    invert: false,
                                }));
                                ui.close();
                            }
                        }
                        for (tid, is_fade, i_am_outgoing) in existing {
                            // the name says WHAT and WHERE (the user’s item 4:
                            // “Remove crossfade” with one label removed both the
                            // fade-out at the end and the crossfade with the neighbour —
                            // “it turns out it removes the fadeout from the end”)
                            let label = match (is_fade, i_am_outgoing) {
                                (true, true) => tr(k::tl::menu::REMOVE_FADE_END),
                                (true, false) => tr(k::tl::menu::REMOVE_FADE_START),
                                (false, true) => tr(k::tl::menu::REMOVE_TRANSITION_NEXT),
                                (false, false) => tr(k::tl::menu::REMOVE_TRANSITION_PREV),
                            };
                            // properties BEFORE deletion: the transition kind is
                            // chosen here and by a double-click on its
                            // window (paired methods — item 17 round 28)
                            if ui.button(tr(k::tl::menu::TRANSITION_PROPS)).clicked() {
                                self.open_transition_editor(tid);
                                ui.close();
                            }
                            if ui.button(label).clicked() {
                                self.apply(Command::RemoveTransition { id: tid });
                                ui.close();
                            }
                        }
                    }
                }
                ui.separator();
                // morpher layer (R.35): one item instead of four, it does not
                // ask for targets — it acts on everything BELOW
                if ui.button(tr(k::tl::menu::ADD_MORPHER)).clicked() {
                    self.add_morph_layer();
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::SAVE_AREA)).clicked() {
                    self.save_marked_area();
                    ui.close();
                }
            }
            // Transition (R.49, item 361). Before this it had no menu of its own
            // at all: everything lay in the menu of the neighbouring CLIP, so a person
            // aiming at the transition got the menu of the wrong object — and, if
            // there were two neighbours, had to guess which one.
            Some(CtxTarget::Transition(tid)) => {
                let Some(t) =
                    self.model.project().transitions.iter().find(|t| t.id == tid).cloned()
                else {
                    ui.close();
                    return;
                };
                // WHAT this is: the transition kind right in the menu header. A submenu
                // of sixteen kinds is not made here — the kind is changed in the
                // properties window, where the curve and the sides are visible next to it.
                ui.label(
                    egui::RichText::new(crate::transition_editor::kind_name(&t.kind)).strong(),
                );
                if ui.button(tr(k::tl::menu::TRANSITION_PROPS)).clicked() {
                    self.open_transition_editor(tid);
                    ui.close();
                }
                // the removal label is EXACT (rule 18): “Remove crossfade” with one label
                // removed both the fade at the end and the crossfade
                let label = match (t.is_fade(), t.a.is_some()) {
                    (true, true) => tr(k::tl::menu::REMOVE_FADE_END),
                    (true, false) => tr(k::tl::menu::REMOVE_FADE_START),
                    // for the transition itself the side “previous/next”
                    // is indistinguishable — it lies BETWEEN the neighbours, not at one of them
                    (false, _) => tr(k::tl::menu::REMOVE_TRANSITION),
                };
                if ui.button(label).clicked() {
                    self.apply(Command::RemoveTransition { id: tid });
                    ui.close();
                }
            }
            Some(CtxTarget::Camera(cid)) => {
                // the camera is finally editable (R.34 K3): properties and removal
                if ui.button(tr(k::cam::PROPS)).clicked() {
                    self.open_camera_editor(cid);
                    ui.close();
                }
                if crate::widgets::menu_item(ui, tr(k::tl::menu::DELETE), Some("Del")) {
                    self.apply(Command::RemoveCamera { id: cid });
                    ui.close();
                }
            }
            // Area (R.38 item 1). Until this round it had NO menu
            // AT ALL: the commands RemoveArea/RenameArea/SetAreaRange/
            // SetAreaLanes lay in the model, written, covered by
            // unit tests, and never called once.
            Some(CtxTarget::Area(i)) => {
                let Some(a) = self.model.project().areas.get(i).cloned() else {
                    ui.close();
                    return;
                };
                ui.label(egui::RichText::new(crate::tf!(k::tl::menu::AREA_TITLE, a.name)).strong());
                if ui.button(tr(k::tl::menu::RENAME)).clicked() {
                    self.area_edit =
                        Some(crate::app::AreaEdit { name: a.name.clone(), buf: a.name.clone() });
                    self.area_edit_focus = true;
                    ui.close();
                }
                let has_region = self.model.project().region.is_some();
                if ui
                    .add_enabled(has_region, egui::Button::new(tr(k::tl::menu::AREA_FROM_REGION)))
                    .clicked()
                {
                    if let Some(r) = self.model.project().region {
                        self.apply(Command::SetAreaRange {
                            name: a.name.clone(),
                            start: r.start,
                            duration: r.end - r.start,
                        });
                    }
                    ui.close();
                }
                // the area’s tracks — by the current row selection region:
                // “if the user changed their mind, they can change/delete/add”
                if !self.last_band.as_ref().map(|b| b.2.is_empty()).unwrap_or(true)
                    && ui.button(tr(k::tl::menu::AREA_LANES_FROM_BAND)).clicked()
                {
                    let lanes = self.last_band.clone().map(|b| b.2).unwrap_or_default();
                    self.apply(Command::SetAreaLanes { name: a.name.clone(), lanes });
                    ui.close();
                }
                ui.menu_button(tr(k::tl::menu::AREA_COLOR), |ui| {
                    for (hex, col) in crate::theme::AREA_PALETTE {
                        let (rect, resp) =
                            ui.allocate_exact_size(egui::vec2(96.0, 16.0), egui::Sense::click());
                        ui.painter().rect_filled(rect.shrink(2.0), 3.0, col);
                        if resp.clicked() {
                            let mut na = a.clone();
                            na.color = Some(hex.to_string());
                            self.apply_batch(vec![
                                Command::RemoveArea { name: a.name.clone() },
                                Command::AddArea(na),
                            ]);
                            ui.close();
                        }
                    }
                });
                // without a chord: Del deletes the SELECTED EVENTS, not the area,
                // and the caption “Delete (Del)” here promised an untruth (item 362)
                if crate::widgets::menu_item(ui, tr(k::tl::menu::DELETE), None) {
                    self.apply(Command::RemoveArea { name: a.name.clone() });
                    ui.close();
                }
            }
            Some(CtxTarget::Morpher(mid)) => {
                // the layer strength as a NUMBER (R.40 item 12, the same second path as for an
                // event). Dragging the level line inside a morph block is not allowed
                // on purpose: the service row is three times lower than a track, and there
                // two edges and a move already converge — a third zone of 5 px would make
                // a miss the rule (checklist item 1). We show the strength on the block
                // itself, and edit it by number or in the window.
                if let Some(m) = self.model.project().morphers.iter().find(|m| m.id == mid).cloned()
                {
                    ui.horizontal(|ui| {
                        let mut v = m.strength;
                        ui.label(tr(k::tl::menu::LEVEL));
                        if crate::widgets::plain_row(
                            ui,
                            &crate::widgets::NumRow {
                                label: "",
                                probe: "morph.strength",
                                unit: kt_model::ParamUnit::Norm,
                                min: 0.0,
                                max: 1.0,
                                step: 0.01,
                                slider: None,
                            },
                            &mut v,
                        )
                        .changed()
                        {
                            let mut ev = m.clone();
                            ev.strength = v;
                            self.apply_coalesced(Command::SetMorpher(ev), self.gesture_counter);
                        }
                    });
                    ui.separator();
                }
                let is_batch = self
                    .model
                    .project()
                    .morphers
                    .iter()
                    .find(|m| m.id == mid)
                    .map(|m| !m.patch.video_fx.is_empty() || !m.patch.audio_fx.is_empty())
                    .unwrap_or(false);
                let bake_label = if is_batch {
                    tr(k::tl::menu::BAKE_BATCH)
                } else {
                    tr(k::tl::menu::BAKE_MORPH)
                };
                if ui.button(bake_label).clicked() {
                    if self.apply(Command::BakeMorpher { id: mid }) {
                        self.selected_morph.clear();
                    }
                    ui.close();
                }
                let del =
                    morpher_del_accel(&self.selected_morph, &self.selected_camera, mid);
                if crate::widgets::menu_item(ui, tr(k::tl::menu::REMOVE_MORPHER), del) {
                    if self.apply(Command::RemoveMorpher { id: mid }) {
                        self.selected_morph.clear();
                    }
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::MORPH_EDITOR)).clicked() {
                    self.open_morph_editor(mid);
                    ui.close();
                }
            }
            Some(CtxTarget::RegionEdge(t)) => {
                // PARITY with gesture M (the user’s item 11): with a selected region
                // both ways give a marker-RANGE, not a point.
                // The item name says what exactly will result (item 4).
                let on_region = self
                    .model
                    .project()
                    .region
                    .map(|r| r.start.0 <= t && t <= r.end.0)
                    .unwrap_or(false);
                let label = if on_region {
                    tr(k::tl::menu::MARKER_REGION)
                } else {
                    tr(k::tl::menu::MARKER_HERE)
                };
                if ui.button(label).clicked() {
                    self.marker_here(Tick(t));
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::REGION_CLEAR)).clicked() {
                    self.clear_region();
                    ui.close();
                }
            }
            Some(CtxTarget::Marker(m)) => {
                if ui.button(tr(k::tl::menu::RENAME)).clicked() {
                    let name = self
                        .model
                        .project()
                        .markers
                        .iter()
                        .find(|k| k.t.0 == m)
                        .map(|k| k.name.clone())
                        .unwrap_or_default();
                    self.marker_edit =
                        Some(crate::app::MarkerEdit { at: m, buf: name, is_new: false });
                    self.marker_edit_focus = true;
                    ui.close();
                }
                let is_range =
                    self.model.project().markers.iter().any(|k| k.t.0 == m && k.is_range());
                if is_range && ui.button(tr(k::tl::menu::CLEAR_RANGE)).clicked() {
                    self.apply(Command::SetMarkerDuration { at: Tick(m), duration: Tick::ZERO });
                    ui.close();
                }
                if ui.button(tr(k::tl::menu::REMOVE_MARKER)).clicked() {
                    self.apply(Command::RemoveMarker { at: Tick(m) });
                    ui.close();
                }
            }
            Some(CtxTarget::Lane { .. }) => {
                self.clip_lane_menu(ui, video_lanes, audio_lanes);
                ui.separator();
                self.empty_place_menu(ui);
            }
            Some(CtxTarget::MorphLane(_)) | Some(CtxTarget::CameraLane(_)) => {
                // R.40 item 2: the same menu as the row header. RMB on the track itself
                // previously fell into the empty-space branch, and a person did not find
                // “add morpher here”.
                self.service_lane_menu(ui);
                ui.separator();
                self.empty_place_menu(ui);
            }
            _ => self.empty_place_menu(ui),
        });
    }
}

#[cfg(test)]
mod geom_tests {
    use super::*;

    /// Test 61 (item 12 round 6): vertical layout — compressing the panel
    /// squeezes the rows down to 30, growing the number of tracks gives a scroll, v_zoom
    /// respects the clamps.
    #[test]
    fn t61_vertical_layout() {
        // 3 clip tracks in a tall panel: height comes from the panel, no scroll
        let v = vertical_layout(400.0, 3, 0, 0, None);
        assert_eq!(v.base_h, 84.0, "clamp from above");
        assert_eq!(v.scroll_max, 0.0);
        // compressing the panel squeezes (as before)
        let v = vertical_layout(160.0, 3, 0, 0, None);
        assert!(v.base_h < 84.0 && v.base_h >= 30.0);
        // many tracks: rows NOT thinner than 30 — a scroll appears
        let v = vertical_layout(300.0, 12, 0, 0, None);
        assert_eq!(v.base_h, 30.0);
        assert!(v.scroll_max > 0.0);
        let v13 = vertical_layout(300.0, 13, 0, 0, None);
        assert!(v13.scroll_max > v.scroll_max, "one more track — more scroll");
        // v_zoom sets the height explicitly, with clamps
        assert_eq!(vertical_layout(300.0, 12, 0, 0, Some(50.0)).base_h, 50.0);
        assert_eq!(vertical_layout(300.0, 12, 0, 0, Some(4.0)).base_h, 16.0);
        assert_eq!(vertical_layout(300.0, 12, 0, 0, Some(500.0)).base_h, 120.0);
        // with a v_zoom that fits entirely, there is no scroll
        assert_eq!(vertical_layout(800.0, 3, 0, 0, Some(40.0)).scroll_max, 0.0);
        // CAMERA rows weigh a third as much (R.35 item 1): three camera
        // tracks take from the video as much as one clip track
        let a = vertical_layout(300.0, 4, 0, 3, None);
        let b = vertical_layout(300.0, 5, 0, 0, None);
        assert!((a.base_h - b.base_h).abs() < 0.01, "camera row weight = 1/3");
        assert!((a.content_h - b.content_h).abs() < 0.01, "three camera rows == one clip track");
        // a morph row is TWICE as tall as a camera row (R.38 item 10): one and a half
        // morph tracks weigh the same as one clip track
        // the comparison must be base_h, not content_h: without the clamp the content
        // by construction fills the available height in BOTH cases
        let m = vertical_layout(300.0, 4, 3, 0, None);
        assert!(m.base_h < a.base_h, "morph rows take more from clips than camera rows");
        assert!(
            (row_height(LaneKind::Morph, 60.0) / row_height(LaneKind::Camera, 60.0) - 2.0).abs()
                < 0.01,
            "a morph row is exactly twice as tall as a camera row"
        );
        // and they do not eat the height of the video: with three camera rows the clip
        // row is exactly the same as it would be with one extra clip track
        let plain = vertical_layout(300.0, 4, 0, 0, None);
        assert!(a.base_h < plain.base_h, "service rows still take up space");
        // height floor: at a small zoom a thin row is not thinner than 10 px
        assert_eq!(row_height(LaneKind::Morph, 12.0), THIN_ROW_MIN);
        assert_eq!(row_height(LaneKind::Camera, 16.0), THIN_ROW_MIN);
        assert_eq!(row_height(LaneKind::Video, 16.0), 16.0);
        assert!((row_height(LaneKind::Camera, 84.0) - 28.0).abs() < 0.01);
    }

    /// Test 60 (item 17 round 6): deep zoom-out — the ruler ladder, the time
    /// format, the proportional snap threshold, the zones of a narrow clip
    #[test]
    fn t60_zoom_out_support() {
        // ladder: across the whole range minor >= 7px, label >= 70px
        for tpp in [30.0f64, 900.0, 9_000.0, 90_000.0, 500_000.0, 2_000_000.0] {
            let (minor, label) = ruler_steps(tpp);
            assert!(minor as f64 / tpp >= 7.0 || minor == 324_000_000, "minor@{tpp}");
            assert!(label as f64 / tpp >= 70.0 || label == 324_000_000, "label@{tpp}");
            assert!(label % minor == 0, "label is a multiple of minor@{tpp}");
        }
        // R.40 item 14: at the working zoom (100 px/s = tpp 900) the minor step is
        // a TENTH of a second, the label is a second. Until this round the ladder
        // started from a second, and there stood `assert_eq!(…0, 90_000)` —
        // that is, the test ASSERTED the defect: minor == label means that
        // not a single minor tick was drawn. The user’s claim, verbatim:
        // “there are no ticks on the main timeline either”.
        assert_eq!(ruler_steps(900.0), (9_000, 90_000));
        assert!(
            ruler_steps(900.0).0 < ruler_steps(900.0).1,
            "a minor tick must be more frequent than labels, otherwise it is not visible"
        );
        // format: mm:ss up to an hour, h:mm:ss beyond
        assert_eq!(format_ruler_time_step(0, 90_000), "0:00");
        assert_eq!(format_ruler_time_step(59 * 90_000, 90_000), "0:59");
        assert_eq!(format_ruler_time_step(3599 * 90_000, 90_000), "59:59");
        assert_eq!(format_ruler_time_step(3600 * 90_000, 90_000), "1:00:00");
        // the sub-second label shows the fraction BY STEP, not by value:
        // labels of one scale must be one shape (rule 3)
        assert_eq!(format_ruler_time_step(0, 9_000), "0:00.0");
        assert_eq!(format_ruler_time_step(45_000, 9_000), "0:00.5");
        assert_eq!(format_ruler_time_step(90_000 + 9_000, 9_000), "0:01.1");
        assert_eq!(format_ruler_time_step(90_000 + 900, 900), "0:01.01");
        // the second step and above — the former shape, byte for byte
        assert_eq!(format_ruler_time_step(90_000, 90_000), "0:01");
        // TIME ON THE RULER IS ALWAYS PRESENT (R.40 item 14): at any zoom and any
        // scroll the nearest label to the left is defined and not to the right of the edge
        for tpp in [30.0f64, 900.0, 9_000.0, 500_000.0] {
            for left in [0i64, 1, 12_345, 90_000, 5_400_001, 1_000_000_000] {
                let (_, label) = ruler_steps(tpp);
                let pinned = ruler_pinned_label(left, label);
                assert!(pinned <= left, "pinned label is right of the edge: {pinned} > {left}");
                assert!(
                    left - pinned < label,
                    "pinned label is farther than a step: {left} − {pinned} >= {label}"
                );
                assert_eq!(pinned % label, 0, "pinned label must sit on the grid");
            }
        }
        // the snap threshold grows with zoom, with no dead ceiling
        assert!(snap_threshold(30.0) >= 1_080);
        assert!(snap_threshold(2_000_000.0) > snap_threshold(90_000.0));
        assert_eq!(snap_threshold(90_000.0), 900_000, "10px at 90k ticks/px");
        // narrow clip: the middle is always Body
        assert_eq!(edge_grab_for_width(100.0), EDGE_GRAB_PX);
        let g = edge_grab_for_width(10.0);
        assert!((2.0..=2.5).contains(&g), "narrow clip: grab ~w/4, got {g}");
        // a 30-min clip in a narrow window gets a coarse thumbnail step
        let step = crate::thumbs::thumb_source_step(kt_time::Tick(30 * 60 * 90_000), 600.0, 150.0);
        assert_eq!(step, 10_800_000, "2-minute step");
    }

    /// Test 72 (item 30 round 7): auto-restore of follow — only when
    /// playing, the region is ENTIRELY visible, the cursor is inside it and the user has stopped
    /// scrolling (grace).
    #[test]
    fn t72_follow_rearm_rules() {
        use super::follow_rearm;
        let reg = Some((1000.0, 2000.0));
        // everything matched → true
        assert!(follow_rearm(true, 2.0, 1.5, 1500.0, reg, 500.0, 2000.0));
        // not playing / too soon after a scroll / no region
        assert!(!follow_rearm(false, 2.0, 1.5, 1500.0, reg, 500.0, 2000.0));
        assert!(!follow_rearm(true, 0.4, 1.5, 1500.0, reg, 500.0, 2000.0));
        assert!(!follow_rearm(true, 2.0, 1.5, 1500.0, None, 500.0, 2000.0));
        // region not fully visible (cut off on the left/right)
        assert!(!follow_rearm(true, 2.0, 1.5, 1500.0, reg, 1200.0, 2000.0));
        assert!(!follow_rearm(true, 2.0, 1.5, 1500.0, reg, 500.0, 1400.0));
        // cursor outside the region
        assert!(!follow_rearm(true, 2.0, 1.5, 800.0, reg, 500.0, 2000.0));
        assert!(!follow_rearm(true, 2.0, 1.5, 2000.0, reg, 500.0, 2000.0), "end excluded");
        // empty region is ignored
        assert!(!follow_rearm(true, 2.0, 1.5, 1000.0, Some((1000.0, 1000.0)), 0.0, 5000.0));
    }

    /// Test 59 (item 14 round 6): follow rules for returning the window to the playhead.
    #[test]
    fn t59_follow_view_rules() {
        // without follow — never
        assert_eq!(follow_view(false, true, false, 0.0, 1000.0, 100.0), None);
        // pause/drag — never
        assert_eq!(follow_view(true, false, false, 0.0, 1000.0, 100.0), None);
        assert_eq!(follow_view(true, true, true, 0.0, 1000.0, 100.0), None);
        // inside the zone [2%, 85%] — we do not touch
        assert_eq!(follow_view(true, true, false, 1050.0, 1000.0, 100.0), None);
        // went right past 85% → window to ph - 15%
        assert_eq!(follow_view(true, true, false, 1090.0, 1000.0, 100.0), Some(1090.0 - 15.0));
        // went left past 2% → likewise
        assert_eq!(follow_view(true, true, false, 1001.0, 1000.0, 100.0), Some(1001.0 - 15.0));
    }

    /// Test 47 (item 2 v1.4): timeline geometry for drop — lane_at_y over the
    /// display rows, x_to_tick round-trip, ruler/gutter/morph → None.
    #[test]
    fn t47_tl_geom_mapping() {
        use kt_model::LaneKind;
        let g = TlGeom::new(
            egui::Rect::from_min_max(egui::pos2(52.0, 100.0), egui::pos2(1052.0, 500.0)),
            50.0,
            vec![
                (LaneKind::Video, 0),
                (LaneKind::Video, 1),
                (LaneKind::Audio, 0),
                (LaneKind::Audio, 1),
                (LaneKind::Morph, 0),
            ],
            &[],
            0.0,
            90_000.0,
            900.0,
        );
        for t in [90_000i64, 180_000, 900_000] {
            let x = g.tick_to_x(t);
            assert!((g.x_to_tick(x) - t).abs() <= (g.tpp as i64), "tick round-trip {t}");
        }
        let lanes_top = g.rect.top() + RULER_H + 3.0;
        assert_eq!(g.lane_at_y(lanes_top + 10.0), Some((ClipKind::Video, 0)));
        assert_eq!(g.lane_at_y(lanes_top + 60.0), Some((ClipKind::Video, 1)));
        assert_eq!(g.lane_at_y(lanes_top + 110.0), Some((ClipKind::Audio, 0)));
        assert_eq!(g.lane_at_y(lanes_top + 160.0), Some((ClipKind::Audio, 1)));
        assert_eq!(g.lane_at_y(lanes_top + 210.0), None, "morph row is not a drop target");
        assert_eq!(g.morph_lane_at_y(lanes_top + 210.0), Some(0));
        assert_eq!(g.lane_at_y(g.rect.top() + 5.0), None);
        assert!(!g.in_lanes(egui::pos2(500.0, g.rect.top() + 5.0)));
        assert!(g.in_lanes(egui::pos2(500.0, lanes_top + 10.0)));
        assert!(!g.in_lanes(egui::pos2(10.0, lanes_top + 10.0)), "gutter left of rect");
        assert_eq!(g.lane_at_y(g.lane_top(ClipKind::Audio, 1) + 1.0), Some((ClipKind::Audio, 1)));
    }

    /// Test 50 (item 6 v1.4): arbitrary row order — bijection
    /// row_of/key_at_y, a clip’s drag snapping to the nearest row
    /// of its own kind.
    #[test]
    fn t50_mixed_rows_bijection_and_nearest() {
        use kt_model::LaneKind;
        let rows = vec![
            (LaneKind::Video, 0),
            (LaneKind::Audio, 0),
            (LaneKind::Video, 1),
            (LaneKind::Morph, 0),
            (LaneKind::Audio, 1),
        ];
        let g = TlGeom::new(
            egui::Rect::from_min_max(egui::pos2(52.0, 100.0), egui::pos2(1052.0, 500.0)),
            40.0,
            rows.clone(),
            &[],
            0.0,
            0.0,
            900.0,
        );
        // bijection row_of ↔ key_at_y at the centre of each row
        for (row, &key) in rows.iter().enumerate() {
            assert_eq!(g.row_of(key), Some(row));
            let y = g.row_top(row) + 5.0;
            assert_eq!(g.key_at_y(y), Some(key), "row {row}");
        }
        // lane_top reflects the shuffled order
        assert_eq!(g.lane_top(ClipKind::Audio, 0), g.row_top(1));
        assert_eq!(g.lane_top(ClipKind::Video, 1), g.row_top(2));
        // snapping: a video clip above the morph row (row 3) moves to V1 (row 2)
        assert_eq!(g.nearest_lane(ClipKind::Video, 3), 1);
        // an audio clip above the video row 0 sticks to A0 (row 1)
        assert_eq!(g.nearest_lane(ClipKind::Audio, 0), 0);
        // audio at the bottom (row 4) — A1
        assert_eq!(g.nearest_lane(ClipKind::Audio, 4), 1);
    }

    /// Test 95 (item 2 round 11): choosing the magnet candidate when moving a clip.
    #[test]
    fn t95_snap_move_candidates() {
        let dur = 100;
        let th = 10;
        // a target for the START only — before, “do not move” with
        // offset 0 won, and the magnet silently did not work
        assert_eq!(snap_move(104, dur, &[100], th), (100, Some(100)));
        // a target for the END only: the clip end is pulled to 500
        assert_eq!(snap_move(404, dur, &[500], th), (400, Some(500)));
        // both targets: the smaller offset wins (the end is closer)
        assert_eq!(snap_move(105, dur, &[100, 208], th), (108, Some(208)));
        assert_eq!(snap_move(103, dur, &[100, 208], th), (100, Some(100)));
        // not a single target within the threshold — nothing moves and there is no line
        assert_eq!(snap_move(300, dur, &[100, 500], th), (300, None));
        // exactly at the threshold the target still counts, beyond it — no longer
        assert_eq!(snap_move(110, dur, &[100], th), (100, Some(100)));
        assert_eq!(snap_move(111, dur, &[100], th), (111, None));
        // single point: the nearest of several
        assert_eq!(snap_point(104, &[100, 107, 300], th), Some(107));
        assert_eq!(snap_point(104, &[300], th), None);
    }
}
