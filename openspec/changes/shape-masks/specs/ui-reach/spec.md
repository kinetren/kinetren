## ADDED Requirements

### Requirement: Masks live where a person looks for them — one list widget in three existing places
The masks SHALL be reachable without a new window (rule 22): the event's picture masks in a "Masks"
group of the Geometry node's "Transform" form and from the preview's right-click menu (Mask →
Rectangle / Ellipse / Pen, for the selected event); an effect slot's masks in a "Masks" group under the
slot's parameters in the effects window; a track's masks in a "Masks" section of the track window. All
three SHALL be drawn by ONE widget: the tool buttons (Rectangle, Ellipse, Pen), and per mask a row with
its name, mode, invert, opacity, feather with falloff, expansion, the path's stopwatch, an edit button
that selects it on the preview, reorder arrows and a delete button; the selected mask's curves below,
in the one automation widget (on a track — the refusal hint of track curves). Every number is a passport
field (limits from the passport, keys only by gesture). The texts SHALL exist in all five catalogs.
Why: kkt-26.11.01 line 55 — "do not throw features anywhere"; the places are those of AE (the layer's
masks), Premiere (each effect's masks) and Resolve (a window on the node); rule 24 — one widget.

#### Scenario: An effect is limited to a mask from the effects window
- **WHEN** the effects window of an event with an effect is opened, the slot selected, its Masks group → Ellipse, a drag on the preview; invert is clicked; the feather field is dragged
- **THEN** the slot has one ellipse mask (the event's picture masks are untouched), inside it the effect, outside the plain event; invert swaps them; the feather reaches the model; the window's Cancel rolls all of it back
- **PATH** double-click the event → the effects window → click the effect's slot → under its parameters "Masks" → Ellipse → drag on the picture → the effect shows only inside the ellipse → tick "Invert" → it shows only outside → drag "Feather" → the edge softens
- **TEST** t1279 (`src/apps/kt-app/src/mask_tests.rs`)

#### Scenario: The picture masks in the Geometry node and the track's masks
- **WHEN** the Geometry node's form is opened and its Masks group used to add a rectangle and an ellipse, the second set to subtract and moved up; then the track window's Masks section adds a rectangle and a key is tried on its feather
- **THEN** the event has two masks in the new order with the modes; the track has one mask; the key is refused with the track-curve reason in the status line and the model is untouched
- **PATH** effects window → the Geometry slot → "Masks" → Rectangle, drag → Ellipse, drag → the ellipse's mode → Subtract → ↑ → the hole is cut first → track menu → Properties… → "Masks" → Rectangle, drag → the track is cut → the feather's curve button → the status line says track curves come later
- **TEST** t1281 (`src/apps/kt-app/src/mask_tests.rs`), t1280 (`src/apps/kt-app/src/mask_tests.rs`)

#### Scenario: Mask texts in every catalog; the live path
- **WHEN** the catalogs are checked for the mask keys; the live scenario runs on our fixture
- **THEN** every key exists in all five catalogs and every glyph is in the fonts; the live scenario draws a Bézier mask, feathers it, keys it at two ticks, inverts it and limits an effect to a mask, with a screenshot per step
- **PATH** right click on the picture → Mask → Pen → four points, close → Feather 40 → stopwatch, move the playhead, drag a point → Invert → effects window → the effect's Masks → Ellipse → drag → the effect only inside
- **TEST** t1283 (`src/apps/kt-app/src/mask_tests.rs`), t1284 live scenario `src/scripts/live/masks.py`
