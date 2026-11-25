# Proposal: point-tracker

> DESIGN, revision 3 (2026-11-25), base `fc60a5d9`: implementation-ready; parts may start before its dependencies (design D11). Decided by A20. Tests:
> t3520–t3531, t4255.

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

- **Two tracker kinds behind one address** (the owner, 2026-11-20: "первые два"): a POINT tracker (AE's, below)
  and a PLANAR tracker (Mocha's): the person outlines a flat surface with four corners, and per material frame the
  tracker stores the plane's homography with a confidence, refined over the whole region, not from a few points;
  a mask of the event may be named as the area to ignore (a hand passing over a sign). Its channels are the four
  corners, the centre, angle, scale and the homography; Corner pin and "Attach event…" with full perspective are
  its natural gestures.
- **The defaults are the most convenient and exact** (the owner): one click places a point with boxes sized to the
  picture (feature 1/24 of the shorter side, 24–96 px; search 3× with motion prediction); one "Track" runs both
  directions from the frame under the playhead; matching always on the native-size frame, sub-pixel; the planar
  kind refines on the full region every frame; the threshold 60 % (point) and 70 % (plane).
- **A point tracker belongs to an event** and holds 1–4 track points. Each point has a feature box and a search
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

- kt-model: `ClipEvent.trackers: Vec<Tracker>` (`TrackerKind::Points | Plane`); `Mask.follow`; the Geometry placement's numbers (from kt-fx); link owners
  `trk<id>` and `geo`, link resolution reads
  samples (linear between material frames), validation, `SetTrackers`.
- kt-media / kt-engine: the tracking job (offline source, luma pyramid, ZNCC; for the plane, feature matches, a
  robust homography and a region refinement), progress, stop rules.
- kt-app: the tracker mode on the preview, head controls, the apply menu, i18n ×5.
- Format: the next free `.kt` version by content; samples one line per second of material.
- OTIO: a row in "What OTIO export loses".
- Idle: nothing runs until a run is asked; drawing points is a cached mesh.
- Feature note: F-MSK-005 refreshed (point and planar now).

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20.txt` line 31, A20, whole): "Q32–Q35 tracker and wand: "первые два, по умолчанию - самые удобные и точные. позже сеть - нейросеть"": the point AND the
planar tracker now, a neural one later, the defaults the most convenient and exact. Read again on 2026-11-25: "первые два" are the first two kinds of Q32 (point, planar); where the
controls stand (Q33) his words do not say, the recommended place was taken. His T1 (line 60, whole above) says a tracker must have somewhere to be applied ("применить то его не
куда") and calls the property system a story of its own: that system is the links, and it lacks the Geometry node on both ends (design D0.1), which is why that is a task here. No open question.
