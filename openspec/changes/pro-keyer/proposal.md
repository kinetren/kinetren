# Proposal: pro-keyer

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word after his answers to the
> questions at the end. Tests reserved: t3585–t3599.

## Why

The owner, `kkt-26.11.01b` B4 (line 5; registry `kkt-26.11.01b-index.md`): "нужен новый greenscreen fx,
построенный наверное на AI или каком-то другом нормальном алгоритме, учитывающий тени, учитывающий волосы,
учитывающий неравномерность фона, учитывающий засветы фантомным светом что дает зелень на нужных объектах -
нужен вообщем проффесиональный greenscreen." Taken (user-feedback, "the second list", 2026-11-01): "The
professional keyer (B4) is an algorithm, not a neural network, as the base: a screen-colour model with screen
balance, a clean plate for an uneven screen, a core and an edge matte for hair, spill and bounce-light removal
that restores the subject's colour, shadows kept as a ratio to the clean plate. Reasons: invariant 11 … and no
ML runtime or model weights in the program. A neural matte may come later as an extra assist. Its note in
`docs/features/` comes first." Note F-MSK-009.

## What Changes

- **A new effect "Pro Keyer"** (a barrier plugin in the colour family), working in linear light:
  1. **Clean plate**: generated from the frame (the screen pixels kept, the subject erased and filled by a
     push-pull pyramid — Nuke's IBKColour), or a chosen frame of the same clip (a moment with no subject),
     held for the whole event (Q1);
  2. **Screen-ness per pixel against the plate at that pixel** (the colour-difference model of
     `chroma-key-upgrade` with the plate's colour in place of one key colour) — the uneven screen divides out;
  3. **Core matte** (hard clip, choke, holes filled) and **edge matte** (soft, in a band around the core's
     boundary) combined — hair and motion blur keep their detail;
  4. **Shadows**: over the screen, the ratio luma(pixel)/luma(plate) becomes a darkening layer with its own
     amount and softness;
  5. **Spill and bounce light**: the plate colour's excess removed from the subject, the subject's luminance
     restored, and the removed light replaced by neutral, a chosen colour, or the subject's own hue measured
     in a non-spill neighbourhood;
  6. **Outputs** for the Viewer: Composite, Matte, Status, Core, Edge, Plate, Shadow, Spill map.
- **A held second frame for a barrier**: a barrier plugin may ask the host for a frame of its layer's own
  source at a fixed material tick (the clean plate); the plan asks and reads it by one function (request ==
  read), and it is warmed like any frame.
- **Analyze** (as the upgraded key): sets the screen, balance and clips from five frames.
- Garbage/holdout mattes are the slot's masks.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: the Pro Keyer kind, its stages, outputs and analysis.
- `plugins`: a barrier may declare a held frame of its own source.
- `ui-reach`: the keyer's device-like panel (stages in a row, each viewable), the plate frame picker.

## Impact

- kt-plugin-api: `BarrierKernel::held(p) -> Option<HeldFrame { material_tick }>`; the pass graph's second
  input `Held`.
- kt-engine: `clip_frame_keys` returns the held frame key with the layer's keys (the order and the read, one
  function; the frame-blending precedent), the content serial in the layer key.
- kt-gpu: the pass graph (~12 passes: linearize, plate pyramid down/up, screen-ness, core clip + choke H/V +
  fill, edge band, combine, shadow, spill/restore, delinearize), word passes where precision matters.
- `src/plugins/fx-video-color`: the `pro_keyer` plugin, rows, presets ("Green, uneven", "Blue, hair").
- kt-app: the keyer's panel (a pipeline like the ISP's), the plate frame chosen by "Use this frame".
- Format: a new kind; the file version by content (an older build reads a `Missing` slot — the plugin rule
  keeps it — so no version rise is needed; rows round-trip).
- Idle: per frame while playing; nothing at rest.

## Questions to the owner

**Q1. The clean plate's default?** Your words: "учитывающий неравномерность фона".
- (a) **Generated from each frame (the subject erased, the screen filled from around it); "Use this frame"
  switches to a held frame of the clip when a moment without the subject exists** (*recommended*: works on
  any shot, follows a moving camera; a held frame is cleaner on a locked camera).
- (b) A held frame required (the effect asks for it first).

**Q2. Light wrap** (the background's light bleeding into the subject's edge) needs the layer BELOW — an input
from another layer, which the effect contract does not have (fx-graph-dag keeps it single-input, its Q2).
- (a) **Later, with an input from another layer** (*recommended*).
- (b) Now, as a special track-level stage that sees the layer below.
