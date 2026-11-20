## ADDED Requirements

### Requirement: One head menu on every entity form
Every form of an entity with values SHALL begin with the same head menu — Copy, Paste, Presets (built-in, then
mine), Save as preset…, Take from event… — drawn by one widget; a guard SHALL fail when an entity form lacks it.
Why: UI rules 17 and 24 — one gesture, one widget; F-AUT-013 trick 2.

#### Scenario: Take from another event by mouse
- **WHEN** an event's effects window head → "Take from event…" is clicked, another event is clicked on the timeline, "Picture chain" and "Curves" are ticked, OK
- **THEN** the pick mode outlines events under the pointer and says how to cancel; the checklist lists the source's parts; after OK the window shows the taken chain; Ctrl+Z restores
- **PATH** double-click an event → its effects window → head menu → "Take from event…" → the status line says "click the event to take from" → click another event on the timeline → the checklist window → tick "Picture chain" and "Curves" → OK → the chain appears in the window → Ctrl+Z → the former chain
- **TEST** tPENDING-7 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `take_from`
