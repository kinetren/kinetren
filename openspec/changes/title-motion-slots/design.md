# Design

## Context

`text_anim_curves(anim, secs, span)` is the one place of the formulas; a preset sets everything in one step and shows
itself active by an inverse comparison (R.53). Title rows that a curve may drive (the R.59 passport): size, position,
offset x/y, reveal, reveal_soft, plate alpha, opacity of text through `gen_auto`. Curves live in the material-tick domain
with the anchor at `source_in`.

## Decisions

### D1. Motions are curve writers over existing rows
`title_motion_curves(motion, slot, len, span, event_len) -> Vec<ParamCurve>`: In covers `[0, len]`, Out `[event_len − len,
event_len]`, Loop repeats as a zigzag segment over the span between them (the R.61 shape, so no key pile). The ten: fade
(alpha), slide ×4 (offset by 0.15 of the frame), rise (offset_y + fade), scale in (size ×0.8→1), type on (reveal), soft
type (reveal with `reveal_soft` 1), blur-in is left out of the writer's row set unless the title has a blur row — it is
NOT a title row, so blur-in is listed as "needs the effect" and adds a `blur` slot through the chain preset of the
event (its curve by the same writer). Loop: pulse (size), sway (offset_x), float (offset_y).

Two slots that touch the same row compose: the curve is the In segment, then a hold, then the Out segment; In and Loop of
one row cannot both exist (the Loop row says so, greyed with the reason).

### D2. The slot's state is read back by the inverse
The window asks which motion the present curves are, by generating each candidate with the length found and comparing
(R.53's technique): a hand edit shows "custom". A click on another motion removes the curves of the rows the slot's
old motion wrote in the slot's window and writes the new: one undo step, in `SetGen`/`SetGenAuto` as today.

### D3. Trim
A trim that stretches animation (the standing `TrimStretchesAuto` flag) and a split keep R.37's domain: after a cut,
In stays on the left piece and Out on the right; the window shows what is left. Not a new rule.

### D4. Class walk
Generators with `gen_auto`: the plate's `reveal` uses the same In writer (fade, slide, wipe); the counters use text's.
The Title presets set the In slot with them (`TitlePreset.anim` becomes In for typewriter and fade; Crawl and Roll stay
presets: they span the event). Reels and published parameters expose the rows, so nothing else is added.
