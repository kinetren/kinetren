# Proposal: motion-driven-params

> DESIGN ONLY (spec session, 2026-11-20). Registry `v26.10-index.md` TD2 (partial, later). Note F-AUT-014.
> Tests reserved: t3947–t3952.

## Why

The owner, `v26.10` lines 160–164: a fisheye on falling letters that comes on while the motion accelerates or brakes
and switches off smoothly while it moves evenly, without "руками вылавливать и подстраиваться под уже существующие
кривые тормажения-скольжения которые находятся у другого события"; the acceleration may sit on a whole composition or
a morpher; and "в момент обычного проигрывания некоторого участка t0-t1 - у нас fisheye не должен активироваться а вот
в момент перемотки тот же участок … уже будет с fisheye … Нужно очень хорошо над этим подумать и как это связано
например с физикой." Today a link (R.61) can read another event's speed curve, but not its acceleration, not a
composition's or a morpher's speed, and nothing of the playhead's own motion.

## What Changes

- **Speed channels as link sources**, from any speed in the project: an event's warp speed, a composition
  instance's speed (its own warp), a morpher layer's warp. Each gives three channels:
  - **Speed** — the signed rate (exists for events today);
  - **Acceleration** — the rate's change per second, the slope of the speed curve;
  - **Speed change** — 0 while the speed is steady, rising towards 1 while it accelerates or brakes and falling back
    smoothly: a peak-hold of |acceleration| scaled by a "full at" value, with Attack and Release in seconds.
  All three are pure functions of the curve at a tick (no state): the same value in play, scrub, seek and export.
- **The playhead as a source — preview only**: "Transport: speed" (the speed the person plays at: the play rate,
  the space boost, a scrub drag's velocity, smoothed) and "Transport: rewinding" (0…1, how far from normal 1×
  playback the playhead moves). An export and every offline render read them as normal playback (1 and 0) — so
  "fisheye only while rewinding" is seen while rewinding and never rendered (Q1). The link picker writes "preview
  only" beside them; a frame that reads them is not kept as the edit's frame.
- **One gesture for the owner's case**: on a parameter's link menu, "Follow speed change of…" picks the source
  (an event, a composition, a morpher layer or the transport) and writes the link with a default mapping (0 → the
  parameter's value now, 1 → its maximum), editable as any link.
- The falling-letters physics (F-FX-005) reads the same channels later.

## Capabilities

### Modified Capabilities

- `automation`: speed, acceleration and speed-change channels as link sources; the transport channels and their
  preview-only rule.
- `ui-reach`: the source picker entries, the one-gesture "Follow speed change of…", the preview-only mark.
- `project-format`: the new link source kinds by content.

## Impact

- kt-model: link source addresses `…/warp/{rate,accel,change}` with `change`'s attack/release/full-at in the link;
  `ParamOwner::Morpher { id }` and the instance's own warp as speed owners; the closed-form envelope (D2).
- kt-engine: `Transport::preview_speed()` smoothed on the UI clock and handed to the plan as a frame input; offline
  plans pass the normal values; the kept-composite key includes the quantized transport value for a frame whose
  plan reads it (or such a frame is not kept).
- kt-app: the link picker entries and the gesture; i18n ×5.
- Format: the next free `.kt` version by content. OTIO: the link is carried in `metadata["kt"]` with the other links;
  a foreign editor sees the parameter's static value.
- Idle: no work at rest; the transport channel is constant (1×, 0) while paused, so a paused frame is the edit's.

## Questions to the owner

**Q1. Rewinding and the render.** Your words: "в момент обычного проигрывания … fisheye не должен активироваться а
вот в момент перемотки тот же участок … уже будет с fisheye". A render plays the edit once, at normal speed; it does
not know how you moved the playhead.
- (a) **Transport-driven effects exist in the preview only; a render sees normal playback (no fisheye there)**
  (*recommended*: the render is the edit; to render the fisheye, drive it from a speed curve of the edit instead).
- (b) A render option "record the transport": the playhead's motion of a chosen take is recorded as a curve and
  rendered (a performance recorded like automation).
