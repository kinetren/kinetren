# Proposal: point-tracker

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word after his answers to the
> questions at the end. Tests reserved: t3520–t3534.

## Why

The owner, `kkt-26.11.01` T1 (line 60; registry `kkt-26.11.01-index.md`): "Крутой pointracker, или как
это называется в AE. А у нас даже применить то его не куда, даже система свойств до сих пор наверное не
сделана хорошая, чтобы привязывать одно к другому и т.д. - это отдельная история."

The second half is answered already: links (R.61, F-AUT-008, `src/libs/kt-model/src/links.rs`) let any
numeric property of any event follow another property by address (`"<event>/<owner>/<param>"`), across
events by the timeline tick. What is missing is something worth linking to: a point that follows a
feature of the picture. Note F-MSK-005 found the trick that decides the design: a track must be an
INDEPENDENT entity with an address that a mask, an effect parameter and another event can all read
(VEGAS), not a field inside one mask (Premiere/FCP, where "Magnetic Masks are not included in XML
exports"); and a broken track is the normal case, so tracking is a session with stops and corrections.

## What Changes

- **A tracker belongs to an event** and holds 1–4 track points. Each point has a feature box and a search
  box (AE's two rectangles), and per MATERIAL frame a sample (x, y, confidence). Samples are tied to the
  material, so trims, moves and splits never shift them (the keys' rule, R.33).
- **Tracking is a session**: from the frame under the playhead, forward or backward, one frame or
  continuous; it stops by itself when confidence falls below a threshold, on Esc, or at the event's edge.
  Dragging a point on the preview at any frame sets a manual sample and the next run continues from it.
  A run is an offline job (its own source, invariant 5), background class, paused while playing.
- **The algorithm is deterministic**: zero-mean normalized cross-correlation of the feature against the
  search box on a luma pyramid, coarse to fine, sub-pixel by a parabola fit of the peak; the reference
  template adapts only when confidence is high. The project's own math (`kt_plugin_api::math`), so two
  machines produce the same samples.
- **The result is an address**: every point exposes `x`, `y`, `conf`; two points add `angle`, `scale`;
  any numeric property can LINK to them (the existing link mechanism with a new owner `tracker`).
- **Apply in one gesture** (AE's "Apply", VEGAS's "Transfer Motion Data"): "Attach event…" (another
  event's Geometry position — and rotation/scale with two points — follows with the offset kept at the
  current frame), "Effect centre" (an effect's `_x`/`_y` pair), "Corner pin" (four points → a Corner Pin
  slot's corners), "Mask follows", "Stabilize" (the inverse onto the tracked event's own Geometry). Each
  gesture writes links, visible and removable as links.

## Capabilities

### New Capabilities

- `tracking`: the tracker entity, track points and samples, the session, the matching algorithm, its
  determinism and cost.

### Modified Capabilities

- `automation`: a link may read a tracker's channels; the apply gestures write links.
- `project-format`: trackers raise the file by content.
- `ui-reach`: tracking on the preview, its controls, the apply menu.

## Impact

- kt-model: `ClipEvent.trackers: Vec<Tracker>`, `ParamOwner::Tracker { id }`, link resolution reads
  samples (linear between material frames), validation, `SetTrackers`.
- kt-media / kt-engine: the tracking job (offline source, luma pyramid, ZNCC), progress, stop rules.
- kt-app: the tracker mode on the preview, head controls, the apply menu, i18n ×5.
- Format: the next free `.kt` version by content; samples one line per second of material.
- OTIO: a row in "What OTIO export loses".
- Idle: nothing runs until a run is asked; drawing points is a cached mesh.
- Feature note: F-MSK-005 refreshed (point first, planar later).

## Questions to the owner

**Q1. Point tracker first, planar later?** Your words: "Крутой pointracker, или как это называется в
AE". Note F-MSK-005 preferred a planar tracker (Mocha) as the general case.
- (a) **AE's point tracker now (1–4 points: position, rotation/scale, corner pin), planar later as another
  tracker kind behind the same address** (*recommended*: what you asked for; corner pin from four points
  covers screens and signs).
- (b) Planar first.

**Q2. Where are the tracker's controls?** UI rule 22; masks are edited on the preview (shape-masks).
- (a) **On the preview, as the masks are: right click → "Track point"; while a tracker is selected the
  preview head shows track backward/forward (continuous and one frame), the threshold and Apply; the
  event's effects window lists its trackers in a section** (*recommended*).
- (b) A Tracker window like AE's panel.
