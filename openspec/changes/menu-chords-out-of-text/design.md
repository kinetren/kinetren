# Design

## Context

- The five items and their bodies (`src/apps/kt-app/src/timeline.rs`):
  - empty-space menu (`empty_place_menu`, ~1723): Paste → `paste_at_click` (the click's time
    and row); with a region, Delete clips in the region → `delete_action`;
  - event menu (~5748): Copy → select the clicked event if needed, then `copy_selection`;
    Paste → `paste_at_click`; Region over this event (~5906) → `region_from_event`;
  - morpher menu (~6301): Delete morpher → `Command::RemoveMorpher { id: mid }`. A right click
    on a morpher keeps a multi-selection that contains it (`select_morph_only` only if it is
    not selected).
- The keys: `Ctrl+C` → `copy_selection`; `Ctrl+V` → `paste_at_cursor` (the playhead, row under
  the mouse); `Del` → `delete_action` (selected morphers and cameras first, then the region,
  then the selected events); Shift+double-click on an event → `region_from_event`.
- The precedent: the event menu's Delete shows `Del` only when there is no region
  (`del_accel`, ~5762). The reel items take a translated chord text (`nest_expand_here_accel`
  "Enter, 2×click").

## Goals / Non-Goals

**Goals:** no menu label carries a key in any language; every chord shown is true in its
context; a guard keeps it so.

**Non-Goals:** changing what any item or key does; touching `[tb.*]` or other hints (rule 3
lets a hint carry its key); the Spanish key name in the chord column (chords are not
translated: the event menu's Delete already shows `Del` in Spanish).

## Decisions

### D1. The chord follows the body
| Item | Chord | Why |
|---|---|---|
| Copy (event menu) | `Ctrl+C` | same `copy_selection` after the item's own selection |
| Paste (both menus) | none | the item pastes at the click; `Ctrl+V` pastes at the playhead |
| Delete clips in the region | `Del` | same `delete_action`; the item exists only with a region |
| Delete morpher | `Del` iff the selection is exactly that morpher and no camera | otherwise `Del` deletes more |
| Region over this event | `tr(tl.menu.region_from_event_accel)` | the same body as Shift+double-click |

One residual case is accepted. With a region drawn AND only this morpher selected, `Del` deletes
the morpher (selected service blocks go first in `delete_action`), so the chord stays true.

### D2. `morpher_del_accel` is pure
`fn morpher_del_accel(sel_morph: &BTreeSet<MorphId>, sel_camera: &BTreeSet<CameraId>, mid)
-> Option<&'static str>` in `timeline.rs`, next to the menu. It is pure because a morpher scene
in the harness costs more than the decision it would test. The decision is the whole point, and
`reach_tests` already proves the item is reachable by a right click.

### D3. t871
1. For every `Lang::ALL` and every key named `tl.menu.*` or `menu.*` that does not end in `_accel`
   (via `tables::KEY_NAMES`), the value has no `Ctrl+`, `Shift+`, `(Del)`/`（Del）`, `(Supr)`,
   or a parenthesised key form.
2. The harness (`harness_three_clips_dt`, Russian UI, so labels come through `tr()`): right-click
   event 2 → an item `"{COPY}\tCtrl+C"`, an item exactly `PASTE` and none starting `"{PASTE}\t"`,
   and an item `"{REGION_FROM_EVENT}\t{REGION_FROM_EVENT_ACCEL}"`. Esc. A region over empty
   space → right-click empty track space → `"{DELETE_IN_REGION}\tDel"`.
3. `morpher_del_accel`: alone → `Some("Del")`; with a second morpher or a camera selected →
   `None`.
It is written first and fails on today's catalog (step 1: "Копировать (Ctrl+C)").

## Risks / Trade-offs

- **The empty-space right click with a region.** The click must land on a track below the
  ruler, away from the region edges (an edge has its own menu target). The geometry comes
  from `tl_geom`.
- **A label length check (t128)** only gets shorter, so it cannot fail.
