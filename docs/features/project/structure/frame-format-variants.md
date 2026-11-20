---
id: F-PRJ-006
title: One edit in several frame formats — 16:9, 9:16 and square from one timeline
status: candidate
priority: B
cost: L
category: project/structure
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, capcut-2026]
we_have: partial
crates: [kt-model, kt-engine, kt-app, kt-otio]
invariants: [4, 7, 11]
ui_rules: [13, 17, 20, 22]
round: null
point: null
tests: []
depends_on: [F-MED-004]
checked: 2026-11-20
---

# F-PRJ-006 — One edit in several frame formats — 16:9, 9:16 and square from one timeline

## Essence

The editor cut a 16:9 video and now needs the same piece as a 9:16 vertical for Shorts/Reels/TikTok and
a 1:1 square. The cut, the sound and the effects are the same; only the FRAMING differs: in 9:16 the
speaker is centred and the lower third moves up out of the platform's buttons. The feature is to keep
ONE edit and several frames of it, each with its own placement of each event, and render them all.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | none | the project has one frame size; a vertical version is a second project or a nested project with Pan/Crop on the nest [NOT VERIFIED that 2026 added nothing] |
| Adobe Premiere | 26 | Auto Reframe Sequence | "Auto Reframe Sequence" makes a NEW sequence of the chosen aspect: clips are nested or kept, and an analysis puts Motion keyframes that follow the action; from then on the two sequences are separate edits [NOT VERIFIED live: helpx.adobe.com closed from this environment] |
| DaVinci Resolve | 21 | Smart Reframe (Studio), per-timeline resolution | each timeline may have its own output resolution (timeline settings); Smart Reframe re-positions a clip by an object-detection analysis per clip; a vertical version is a duplicated timeline with another resolution [NOT VERIFIED live] |
| Final Cut Pro | 12 | Smart Conform | "Duplicate Project As" with another aspect (square, vertical) runs Smart Conform: the clips are re-positioned (Transform / Crop keys) to keep the subject; the duplicate is a separate project from then on [NOT VERIFIED live] |
| After Effects | 26 | compositions | one composition per format; a shared precomp of the content, a top comp per aspect positions it; changes in the precomp reach all formats, framing is per top comp |
| CapCut desktop | 2026 | Ratio + Auto reframe | the canvas ratio of the project is switched; auto reframe re-positions clips; switching back does not restore the old framing [NOT VERIFIED live] |

## The trick behind it

**1. Almost everyone answers "a copy", and from the copy on the edits diverge.** Premiere's Auto Reframe
and FCP's Smart Conform both make a NEW sequence/project: the reframing is real work stored in the copy,
and the next cut in the 16:9 master is not in the vertical. A late change of one shot means re-cutting
every format by hand. Only the After Effects pattern — content in a shared precomp, framing in a top comp
per format — keeps one edit, and it pays with a nesting level per format.

**2. The framing is per EVENT, not per format.** A format is not a crop of the whole master: a 9:16 crop of
a 16:9 frame cuts the interview's second person, the title, the lower third. What changes is each event's
position/zoom (and some events' presence: a wide establishing shot is replaced, a title moves). So a
format is "the master's geometry with per-event overrides", and an event without an override follows a
format-wide default (fill, fit, the master's centre).

**3. Safe areas are platform UI, not broadcast title-safe.** For vertical delivery the danger zones are the
platform's own buttons and captions (right column, bottom 20–25 %), and they differ per platform; the
grids people use are platform-shaped overlays, not the 90 %/80 % rectangles of TV.

## Pros

- One cut, N frames: a trim, a new shot or a sound fix lands in every format with no re-cut.
- Overrides are sparse: only the events whose framing actually needs fixing carry data for a format.
- A batch render of all formats is one job description, not N projects to open.

## Cons

- Every place that asks "the frame size" must ask "of which format", and a forgotten place silently uses
  the master's size.
- A format-only event (a vertical-only title) breaks "one edit": it must be expressed as presence per
  format, or the vertical becomes a fork again.
- Caches keyed by the project size must key by the format, or switching formats shows a stale frame.

## What users say

NO QUOTES: this session had no web access budget for the reconnaissance (2026-11-20, a spec session that
only reads the repository); the forums (r/premiere, r/finalcutpro, the BMD forum) are where the "the
reframed sequence does not follow my edits" complaints would be searched. The editor rows above carry
[NOT VERIFIED] for the same reason; the next wave verifies them live.

## How it fits Kinotrain

### What already exists

- **The frame-conform rule and the leading Geometry node** (R.62 A, F-MED-004): `ProjectSettings.conform`
  (`ConformRule` fit/fill/stretch/as-is) and each event's `EventGeometry { rule, crop, pos, zoom, … }`
  with `geometry_auto` curves (`src/libs/kt-model/src/project.rs`). A format's override is exactly a
  second `EventGeometry` of the event.
- **A reel's own raster** (`Sequence.raster`, R.62 revision of R.50): a composition instance is conformed
  into the parent's frame by the same rule — the After Effects pattern exists already, but as a manual
  nesting.
- **The one plan** (`frame_plan`, invariant 7) takes the raster from the project settings; the render
  window chooses a size and fps (`src/libs/kt-engine/src/export_settings.rs`).

### What is missing

- No notion of a second frame of the same edit: `ProjectSettings.width/height` is the one raster.
- No per-format override storage, no format switch in the preview, no batch render of formats.
- No platform safe-area overlays.

### What hinders

- Many places read `project.settings.width/height` directly (the plan, the masks' reference frame, the
  title's layout, the camera's world scale, the disk render cache's key, the reel strip). Each must take
  the format's raster or it renders the master's framing into the vertical.
- Masks and on-screen gizmos are normalized to the owner's reference frame: a mask on an event is in the
  event's own space (it travels with the event's geometry), a track mask is in the canvas space — in a
  vertical, the canvas mask needs its own placement too.

### Which invariants it touches

- **7** — the format is an input of the one plan, never a second composition.
- **4 / 11** — caches and the export key by the format's raster.
- UI rules **13** (the switch on an occupied place), **17** (one geometry control, per format), **20**
  (an override is never made by itself), **22** (no new window).
- CAPABILITY: openspec/specs/video-plan/

## Estimate

| What | Answer |
|---|---|
| Scope | L — the model (formats, overrides), the plan's raster input, the UI switch, batch render |
| Where the main work is | kt-model, kt-engine (raster from the format), kt-app (switch, gizmos per format), the render window |
| Project format | a new version by content (formats present) |
| Tests | a format with no overrides equals a fill/fit of the master; an override moves only its event in its format; export of all formats equals each format's preview; caches keyed by format |
| Depends on | F-MED-004 (the conform rule — done in R.62) |
| Breaks compatibility | no |

## Verdict

Take it, in the After Effects sense but without the manual nesting: one edit, formats as rasters with
per-event geometry overrides and per-event presence, a batch render. The copy model of Premiere/FCP is
rejected: it is exactly the divergence the owner's "варианты кадра одного таймлайна" asks to avoid.
Automatic subject-following reframe is a later stage (it needs the point tracker, F-MSK-005).

## Sources

- The owner's candidate ★15, `docs/ktprompts/kkt-26.11.01-candidates.md` (taken 2026-11-01).
- Note F-MED-004 (`docs/features/media/sources/import-scaling-policy.md`) and capability `video-plan`.
- Editor rows from prior knowledge of the products; marked [NOT VERIFIED] (no live access this session).
