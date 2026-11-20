# Proposal: chroma-key-upgrade

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word after his answers to the
> questions at the end. Tests reserved: t3558–t3569.

## Why

The owner, `kkt-26.11.01b` B3 (line 4; registry `kkt-26.11.01b-index.md`): "greenscreen очень скудный, и
откровенно плохого качества - его надо доработать, а еще проще было бы кнопку автоанализа добавлять, чтобы
не подбирать всё руками". Taken (user-feedback, "the second list"): "The existing chroma key (B2, B3) gets
the eyedropper … and an auto-analysis that sets its values from the frame; its quality is fixed in the same
change." The eyedropper is done (F-MSK-007). The professional keyer (B4) is a separate effect (change
`pro-keyer`).

Today `greenscreen` (`src/plugins/fx-video-color/src/greenscreen.wgsl`) takes the Euclidean RGB distance to
the key colour with a hard threshold and a soft ramp, and its despill pulls the key channel down to the
larger of the other two. Note F-MSK-004 found why this is poor: (1) a distance in RGB means a different thing
on green and on blue screens — a key works on the colour DIFFERENCE with a screen balance; (2) without a
matte view nobody can tune a key; (3) despill by subtraction darkens light hair to grey — a despill must
return the luminance.

## What Changes

- **A colour-difference model** (`model = difference`): the matte is the pixel's key-channel excess over the
  balance-weighted mix of the other two, relative to the screen's own excess (Keylight's Screen Balance;
  green ≈ 0.5, blue ≈ 0.95), with clip black and clip white. **Every greenscreen uses it** (the owner, A22:
  "Все ключи на новую модель"): the distance model is removed; an existing slot's rows (key colour, tolerance,
  softness, spill) are read by the new model through a fixed mapping, so old projects open and key better, and
  look different from before.
- **Matte operations**: clip black / clip white, shrink/grow (choke, px), edge softness (alpha blur, px).
- **Despill with luminance restore**: the excess is removed, then the pixel's luma is restored to the
  original's (Keylight's Soft Colour) or to a chosen replacement colour's; a despill bias picks how strong.
- **Views of the key** through the node editor's Viewer (fx-graph v1): the Viewer on the key node offers
  Composite, Matte, Status (near-0 and near-1 alphas tinted so the eye sees them) and Screen; a preview-only
  view, never rendered.
- **Analyze** (one button): from the event's picture at the key's input on five frames across the event, it
  finds the screen colour (the dominant saturated cluster in Cb/Cr), sets the key colour, the balance by the
  screen's hue, clip black from the screen pixels' matte (their 99th percentile) and clip white from the
  subject's (1st percentile), in one undo step, and says what it set.
- Garbage and holdout mattes are the existing slot masks (shape-masks); the key's form links to them.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: the greenscreen's models, matte operations, despill, its views, the analysis.
- `ui-reach`: the Analyze button, the key's views in the Viewer.

## Impact

- `src/plugins/fx-video-color`: new rows (`model`, `balance`, `clip_black`, `clip_white`, `choke`, `soften`,
  `despill_mode`, `despill_bias`, replacement colour) omitted at defaults; the difference kernel as a barrier
  (choke and soften need neighbours) with its pass graph; the distance path untouched.
- kt-app: the Analyze button in the slot's head; the Viewer's view menu for a key node; i18n ×5.
- kt-engine: the Viewer's view mode reaches the preview plan only (never the export plan).
- Format: the new rows are omitted at their defaults; a file with any is written with the next free version
  by content. OTIO: effect rows in metadata as before.
- Idle: the analysis is a click; views compose once per change.
- Feature note: F-MSK-004 refreshed.

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20` A22, his Q38–Q39): "Все ключи на новую модель" — Q1 (b), every greenscreen
switches to the difference model and old projects look different after opening; Q2 (a), Analyze looks at five
frames spread over the event. The questions as asked:

**Q1. New keys use the new model; old projects keep the old?** Your words: "откровенно плохого качества -
его надо доработать". The rule of effects (kt-fx): the default of a new parameter equals the previous
behaviour, byte for byte, so old projects do not change by themselves (rule 20).
- (a) **Old slots keep the distance model; a key added from now on starts with the difference model; a
  slot's form offers "Use the new key" which switches it** (*recommended*).
- (b) Every greenscreen switches to the new model; old projects look different after opening.

**Q2. What does Analyze look at?**
- (a) **Five frames spread over the event, at the key's input** (*recommended*: one frame can miss the
  screen's unevenness and a moving subject).
- (b) The frame under the playhead only.
