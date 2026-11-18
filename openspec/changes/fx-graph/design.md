# Design: fx-graph

## Context

- Claim: kkt-26.11.18b item 3 (registry row U3), quoted in `proposal.md`.
- Feature note: `docs/features/ui/editing/fx-chain-graph.md` (F-UI-008) — the reconnaissance
  (Resolve Color/Fusion, Nuke, Ableton Racks, Bitwig containers and modulators, REAPER routing,
  Vegas, Premiere, FCP, AE Flowchart) and the verdict: a ROW graph laid out by order, relations drawn
  on demand, folding groups; no free canvas.
- His standing decisions this design keeps (`docs/user-feedback.md`):
  - R.33 S2 / C2 — ONE chain widget for events, morphers and tracks; rule 17 "one widget".
  - R.35 item 3 — the morpher's source swap is the purple pinned HEAD of its chain, effects of both
    kinds in one view. (Whether "one view" stays "one lane" is decision O2.)
  - R.37 item 2 — the warp's off switch on the picture is the slot checkbox; no second switch of one
    meaning (kt-app rule 9).
  - R.38 item 12 — the layer strength acts on picture AND sound at once.
  - R.63 — no new window without an obvious purpose; parameters go where the person already is.
  - R.66 — the window form contract: the body scrolls, buttons outside it, numbers only through the
    common row, collapsible groups. The forms below the chain are not re-laid out by this change.
  - 2026-11-03 (sim-audio-pairs) — a device recording picture and sound simulates both, one device
    state; the sound half is "visible and switchable off".
  - 2026-11-07 — the program at rest costs nothing, measured on a heavy project.

## D1. The defect: the sound half of a device in a morpher is a ghost

**Mechanism.** `MorphPatch::chain_for(kind)` appends `derived_pairs(kind)`: for each ENABLED paired
slot on the other side, the counterpart is appended "unless this side already holds an n-th one of
that kind" — and the count of what this side holds filters out bypassed slots
(`count(own, own_off, here)`). So:

1. the half is never in `patch.audio_fx` → `merge_chains` never shows it → no checkbox;
2. a sound half added by hand and switched off counts as "not held" → the derivation puts it back;
3. symmetric: a patch carrying only `vhs_tape_sound` derives `vhs_tape` on the picture, also
   un-switchable.

**The class: a slot that the render has but the edited model does not.** Walked:

| Place | What the render gets beyond the edited slots | Found |
|---|---|---|
| Morpher patch, paired device | the counterpart half (`derived_pairs`) | **the defect** — fixed here (D2) |
| Morpher patch, time warp | `chain_for(Video)` adds the warp from `audio_fx` | the same ghost: the picture's slowing by a layer cannot be switched off alone, while on an event it can (R.37 slot checkbox). Decision O4 |
| Morpher patch, layer strength | `chain_for(Audio)` adds `LayerMix` from `video_fx` | by his decision R.38 item 12 ("at once"); shown as one node with a tie and ONE dot — not a defect |
| Event, paired device | — real slots on both events (`sync_pairs_to_link_group`) | clean |
| Event, warp | — real slots on both events (`sync_warp_to_link_group`) | clean |
| Track chain, paired device | nothing: a track has no partner track, VHS Tape on a video track has no sound | a gap of the same family (no pair at all); not fixed here, decision O5 |
| Composition instance chain | an instance is an event: the event rule | clean |
| Preset / pasted chain into a patch | goes through `SetMorpher` → the new rule (D2) | covered |
| Bake of a morpher | `apply_to` reads `chain_for` → real slots on the pieces, then the event rule | clean after D2 |
| kt-cli `probe` | prints `chain_for` | prints the real halves after D2 |

## D2. The fix: both halves are real slots of the patch

A rule in the model, beside `sync_pairs_to_link_group`, applied inside `SetMorpher` (and therefore in
one undo step, including under `apply_coalesced`): `sync_pairs_in_patch(patch, edited_side)`.

- The n-th paired slot on the edited side ↔ the n-th counterpart slot on the other side: values and
  curves copied from the edited side, a missing one appended at the end of the other chain, an extra
  one removed. **Presence is shared, bypass is each half's own.** The edited side is known to the
  window (the rail the gesture was on); a model-only `SetMorpher` (paste, preset, kt-cli) without a
  side takes the picture side as the leader, the sound side when the picture holds no paired slot.
- `derived_pairs` is removed; `chain_for`/`auto_for` return the patch's own enabled slots (plus the
  warp and the strength, unchanged).
- `Project::normalize` materializes the halves once for patches read from older files (the only
  migration place, precedent `migrate_gen_to_events`). It derives exactly what `derived_pairs`
  derived, so an old project renders byte for byte as before (t2991).
- **The device state stays one when both are on**: sim-audio-pairs D3 — each half reads the shared
  rows and the timeline time (`AudioState::place`), not the other half's runtime state. Switching the
  sound off removes one node from the audio chain; the picture's bytes do not move (t2992), and with
  both on, a dropout is still seen and heard at the same tick (t1467 stays).
- One text for both rules: the matching of the n-th occurrence moves into one helper used by
  `sync_pairs_to_link_group` and `sync_pairs_in_patch` (kt-model CLAUDE "one rule").

## D3. The graph: two rails laid out by order

One widget, `fx_graph`, replaces `widgets::chain_strip` everywhere it is called (FX window, morpher
window, track window). It edits the SAME vectors through the SAME commands; it adds no model.

- **Rails.** A picture rail and a sound rail. The FX window shows the anchor event and its link
  partners as ONE graph: the video event's chain on the picture rail, the linked audio event's chain
  on the sound rail (today: two strips one under another). An event without a partner shows one rail.
  The morpher window shows `patch.video_fx` on the picture rail, `patch.audio_fx` on the sound rail,
  the source swap as the purple pinned head before both (it acts on both kinds). The track window
  shows its one rail.
- **Nodes.** A chip: on-dot, short name elided at `SLOT_NAME_CHARS`, a kind glyph; height 22 px,
  the grab pad of rule 1. The leading Geometry node and the generator source head keep their pinned
  places (`ChainSel::Lead`, `HeadSlot`).
- **Ties.** A paired device's halves are joined by a vertical tie (a diagonal when they stand at
  different positions); the tie is a thin line always, emphasized while either half is selected or
  hovered (the "relation on demand" trick). A switched-off half dims, its tie turns dotted.
- **Groups (read-only context in the FX window).** After the event's own chain, each morpher layer
  that reaches the event (bottom-up, the engine's order) is a FOLDED purple group: `◆ <layer name>
  (n)`. Click unfolds it in place, showing its nodes greyed (they are edited in the morpher window);
  double click opens that layer's morpher window. Then the track stage: `▤ <track> (n)` folded, with
  an incoming key wire `← <key track>` on a compressor with a sidechain. Nothing about layers or
  tracks is edited from an event's graph — one place to edit each thing.
- **Order is the layout.** x = position in the processing order; nothing is stored. A long chain
  scrolls horizontally inside the graph's own band (the wheel scrolls it while over it); the window
  body below keeps its R.66 vertical scroll.
- **The form.** The selected node's existing form opens below the graph exactly as today.

### Mockups

One event's chain (FX window; a video event linked to its audio; nothing selected):

```
 Video 2 · "street.mp4"                                                     [⏻ chain]
 picture ●Geometry ─ ●Color Curves ─ ●Glow ─ +      ┊ ◆ Layer "grade" (3) ┊ ▤ Video 1 (1)
 sound   ●EQ ─ ●Compressor ─ +                      ┊ ◆ Layer "grade" (0) ┊ ▤ Audio 1 (2) ← key: Music
 ───────────────────────────────────────────────────────────────────────────────────────────
 (form of the selected node opens here — deck face / passport grid / automation, as today)
```

A paired device, its sound half switched off (VHS Tape selected):

```
 picture ●Geometry ─ ●VHS Tape ─ ●Glow ─ +
                      ┆ (dotted tie: one tape, sound half off)
 sound   ●EQ ─ ○VHS Tape · sound ─ +
 ───────────────────────────────────────────
 [VHS Tape — deck face: Tape · Heads · Tracking · Sound (Hi-Fi / Normal / Mix) ...]
```

A morpher batch (morpher window; Datamosh and Camera ISP, Datamosh's sound off):

```
 ◆ Source: (original)   [purple head, acts on both rails]
 picture ●Datamosh ─ ●Camera ISP ─ ●Layer strength ─ +
           │              │               ║ (strength: one dot for both, R.38 item 12)
 sound   ○Datamosh · sound ─ ●Camera ISP · mic ─ +
```

The same layer folded inside an event's graph (FX window):

```
 picture ●Geometry ─ ●Glow ─ +   ┊ ◆ "mosh" (3) ▸ ┊ ▤ Video 1
 sound   ●EQ ─ +                 ┊ ◆ "mosh" (2) ▸ ┊ ▤ Audio 1
                    click ▸ → ◆ "mosh" ▾ [ Datamosh · Camera ISP · strength ]  (greyed, read-only)
```

### Gestures (PATH lines)

- **PATH** double click a morpher block carrying VHS Tape → the morpher window opens with the graph:
  the purple head, VHS Tape on the picture rail, "VHS Tape · sound" on the sound rail with a tie →
  click the on-dot of "VHS Tape · sound" → the node dims, the tie turns dotted, the picture rail is
  unchanged → play → the picture keeps the tape, the sound under the layer is clean.
- **PATH** double click an event with linked audio → the FX window shows both rails → click `+` at
  the end of the picture rail → the picker gallery opens in place → click VHS Tape → a node appears
  on the picture rail and its sound half at the end of the sound rail, tied → the deck face opens
  below.
- **PATH** drag the "Glow" node right past "VHS Tape" on the picture rail → an insertion mark shows
  between nodes while dragging → release → the order changes, one undo step; dragging onto the sound
  rail shows the refusal mark and the hint "picture effects stay on the picture rail".
- **PATH** hover "VHS Tape" → its `×` shows → click `×` → both halves go (the tie told it is one
  device) → Ctrl+Z → both return with their values and curves.
- **PATH** in the FX window click the folded group `◆ "mosh" (3) ▸` → it unfolds in place, greyed →
  double click it → that morpher's window opens.
- **PATH** move the mouse over the graph and leave it still with the window open on a project
  carrying VHS Tape, Datamosh and Camera ISP → only the hover tint changes → no frame is asked at
  rest.

## D4. Cost at rest

- The graph lays out once per key (model `revision()`, window width, selection, fold set, hover
  node id) into cached galleys and one mesh for chips and wires; a hover frame re-tints the hovered
  chip only (the `EventPaint` precedent). No animation exists in it; a fold toggles instantly.
- Nothing in it polls: the folded layer/track groups are read from the per-revision index
  (`tl_facts()`), not a walk of `effective_clips` per frame.
- Checked by an idle test with the graph open (`kt_time::work` rows, t3003) and the live `idle`
  scenario extended with an open FX window and an open morpher window on the showcase heavy project
  (t3006).

## D5. What is migrated and what stays

- Migrated: every `chain_strip` call → `fx_graph`; `merge_chains`/`split_chains` → the graph's
  rail-to-vector mapping (the morpher keeps its two vectors); the FX window's member strips → rails;
  the strip's copy-paste, presets, master switch and right-click target (`ChainCtx`) move into the
  graph unchanged.
- Stays: every form under the chain (passport grid, deck face, ISP panel, EQ graph, masks,
  automation widget), the picker gallery (R.54), the preset library (R.48), the windows themselves
  and the registry `WINDOWS`. No window is added.
- Removed: `widgets::chain_strip` (no second copy of the widget, rule 17), `derived_pairs`.

## D6. Project format and OTIO

- No format version change. `MorphPatch` keeps its fields; after D2 a patch with a paired kind
  serializes the counterpart slot in its other vector. Files from before this change are
  canonicalized by `normalize` on read (render byte-identical, t2991); a file written after reads in
  an older build with the counterpart as an ordinary slot and the derivation adding nothing (the
  derivation counted only enabled ones → a bypassed half would be re-derived there: an older build
  shows the old defect, nothing breaks).
- Graph layout: not stored (computed from order); fold state: per window session, not stored.
- OTIO: morphers travel only in `metadata["kt"]`; the sim-audio-pairs row of "What OTIO export
  loses" (`docs/ktprompts/v26.10-index.md`) gains "and the halves held by a morpher patch". No new
  row.

## Open decisions (the owner's)

- **O1 — where the graph lives.**
  (a) it replaces the strip inside the three existing windows (recommended: no new window, R.63);
  (b) as (a), plus a docked band under the preview showing the selected event's graph (a panel takes
      screen height permanently — rule 13);
  (c) the strip stays, the graph is a toggle (two widgets of one meaning — against rule 17).
- **O2 — the morpher: one lane or two rails.** R.35 item 3 made it one lane with both kinds mixed.
  (a) two rails, picture and sound, the head before both (recommended: the tie of a device needs two
      rows; t244/t218 are rewritten to "two rails, one head" with his OK);
  (b) one lane as now, the sound half drawn right after its picture half with a small tie; no test
      rewritten, but long batches wrap again.
- **O3 — the context in an event's graph.**
  (a) layers and track stage shown folded, read-only, double click opens their window (recommended);
  (b) only the event's own chain (smaller, but the order of processing stays invisible);
  (c) editable in place (two places to edit one layer — rejected by "one place to edit each thing",
      offered only for completeness).
- **O4 — the warp in a morpher.** Today a layer's Slowed slows picture and sound with one slot.
  (a) make it a tied pair like a device (a picture warp node with its own dot, presence and `rate`
      shared — the event rule of R.37 extended to the patch);
  (b) leave it one slot for both (recommended for this change; the class walk records it).
- **O5 — a device on a track chain has no sound half** (a track has no partner track).
  (a) a separate change: a video track's device sounds on the audio track(s) the person names;
  (b) leave it: a device on a track is picture-only, said in its form.

## Risks

- t244/t218 pin the strip's look; rewriting them needs O2's answer (they are regression tests).
- The FX window with many partners (an instance with several audio events) — more than two rails:
  extra rails stack under the sound rail, each named; tested (t3001).
- The live scenarios that click strip crumbs by label (`vhs_sound`, `stream_sound`, `camera_mic`,
  the morpher ones) must follow the graph: their probes move with the widget.
