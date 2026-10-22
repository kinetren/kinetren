# Spec Delta

## MODIFIED Requirements

### Requirement: Public parameters — the template value is not stored, overrides come before morpher layers
A reel SHALL declare handles (id, author's name, target address, type, range narrowing); an instance
SHALL override ONLY what is published, by a value or a curve. The template value MUST
NOT be stored — it is read live from the target. Resolving the reel for an instance SHALL go through one
door; the overrides are laid down BEFORE the morpher layers; an empty set is the former path without clones.
A curve lives in the material of the instance (the reel's time); the carriers of a curve are the slot, the generator,
the attachment, and also the LEAD GEOMETRY of the event (`ParamOwner::Geometry` — the fields of `EventGeometry`:
rule, crop, position, zoom, rotation, anchor) — through the same passport, the same `set_value`, the same
curves (`geometry_auto`). Publication commands (`PublishParam`, removal, editing
of the declaration) SHALL carry the reel address EXPLICITLY (`seq`) and MUST NOT depend on which
reel the person is standing in: publication from an expanded strip addresses the reel of the strip without entering
it. An override of an effect slot by a number SHALL act on the frame of the instance the same way
as an override of a generator: two instances with different values of one handle on an
effect slot give different frames and different memo keys (checked on `RadialBlur.amount`, not only on `zoom`). The address is attached to the target's id: it survives trim, move, the left half of a cut;
if the target has disappeared or changed type, the publication and all overrides go away in one
step, per batch, without a list of triggers; validation rejects orphans and foreign types. The expanded strip shows the TEMPLATE; unfolding bakes the values in and names the count.
The range narrowing of a numeric handle SHALL by default equal the passport of the TARGET (`min`/`max` of its `ParamSpec`, obtained by the address): the author's window shows the target's limits, not zero;
only the narrowing is written to the file — a value equal to the target's limit is stored as absence
(`None`); resetting to the target's limits is one action. The value at the consumer SHALL be clamped by the target's limits and, on top of them, by the author's narrowing.
Why: a stored copy would diverge from the reel at the first edit inside; the morpher appends slots to the end — after it the slot address would point to the wrong place; "min/max always at 0" — an empty `Option` was drawn as zero, while the target's passport already knows its limits (R.60). R.63: the lead geometry is the only carrier of properties without a publication address (the owner: "or do we have duplicated code?" — not duplicates, a hole in addressing); "why do we automatically go fully into the composition" — the command of the rule goes to "the reel you are standing in"; "Radial blur… I changed it in the composition — and it doesn't change" — the path of the effect slot had not been proven by a test on its own parameter.

#### Scenario: Commands and two questions
- **WHEN** a handle is published, overridden, removed
- **THEN** six commands with undo; "what is in the template" is read from the target, "what plays here" — with the override
- **TEST** t671 (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: The target's fate
- **WHEN** the target is trimmed, cut, moved, deleted or changes type
- **THEN** the address survives trim and the left half of a cut; the target's death takes away the publication and the overrides in one step; orphans and foreign types are rejected by validation
- **TEST** t672 (`src/libs/kt-model/tests/pubparam.rs`), t672b (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: An empty set — the former path
- **WHEN** an instance without overrides is resolved
- **THEN** transitions and cameras are borrowed (`Cow::Borrowed`), not a single clone of the reel
- **TEST** t673 (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: The band shows the template, expansion bakes it in
- **WHEN** an instance with overrides is expanded in the strip; it is unfolded
- **THEN** the strip shows the template's values; after unfolding the values are split into pieces, and the number of baked values is stated
- **TEST** t690 (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: Handle limits by default — the target's passport
- **WHEN** a numeric handle has just been published; the author narrows `max`; the author returns `max` to the target's limit; the author presses "reset to the target's limits"
- **THEN** the author's window shows the target's `min`/`max` (not 0); only the narrowing lies in `PubRange` (`min: None`); a value equal to the target's limit is again `None`; after the reset `PubRange::is_empty()`, and the file without narrowings is byte-for-byte as before
- **TEST** t692 (`src/libs/kt-model/tests/pubparam.rs`), t692b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The consumer's field is clamped to the target's limits
- **WHEN** the consumer drags a handle's value without the author's narrowing past the limit of the target's passport; with a narrowing — past the narrowing
- **THEN** the value does not go past the target's passport; with a narrowing — does not go past the narrowing; the model and the field hold the same number
- **TEST** t692b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Leading geometry — a carrier of a handle
- **WHEN** inside a reel the zoom of an event's lead geometry is published; an instance overrides it with a number and a curve; the event is trimmed and cut; the event is deleted
- **THEN** the handle with address `Geometry` is in the file and in the model (a v-predicate over the content); the instance's frame differs from the template by exactly the zoom; the curve lives in `geometry_auto` in the material of the instance; the address survives trim and the left half of a cut; the death of the event takes the handle away in one step; validation rejects the geometry address on an audio event
- **TEST** t752 (`src/libs/kt-model/tests/pubparam.rs`), t752a (`src/libs/kt-engine/tests/nest_param.rs`)

#### Scenario: Publishing addresses the reel explicitly, without entering it
- **WHEN** the publication command is submitted with the `seq` of a reel from the root (the reel is expanded in the strip), from another reel, and from inside the reel itself; `seq` does not exist
- **THEN** the handle lands in the named reel regardless of context, `editing()` does not change; a nonexistent reel — the command is rejected with the address; the transition and the camera from the strip are addressed by the same `seq` (the R.59 "Deferred" item is lifted)
- **TEST** t753 (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: Overriding an effect slot changes the frame
- **WHEN** in the reel the event has `RadialBlur`, and the handle `amount` is published; two instances override it with 0.0 and 0.9; only the second is edited
- **THEN** the frames of the instances differ (the first equals the frame without blur); the memo keys and `instance_content` differ; the edit of the second does not touch the first; the backend has no effect on this — the handle values are resolved when the PLAN is BUILT, and the executor receives ready numbers (invariant 7)
- **TEST** t755 (`src/libs/kt-engine/tests/nest_param.rs`)

### Requirement: Publishing interface — one door, the author's name verbatim, two badges
Publication SHALL be reachable from one door — a right click on a field's caption inside
a composition (the full entry or the strip); outside a reel there is no item, and a hint explains it — only
when compositions exist in the document. The owner reel SHALL be determined by the working view (an event of the strip belongs to the strip's reel), and the publication confirmation MUST NOT enter
the reel: the command carries its address, the person stays where they were. The consumer SHALL edit
handles in the FX window of the instance — in the pinned head of the chain's strip (each type with its own
widget; reset sends the removal of the override; the window session is one history step); there is no
separate parameter window of the instance. The handle's name SHALL be drawn verbatim, bypassing the translation catalogue. Badges: a pin at the tail of the event's caption, a number in the header of the reel's card; zero is not written.
The curves of overrides SHALL be VISIBLE in the same head with the same automation widget as the effect slot
(graph ↔ keyframe table, choice of the active curve, keyframe at the cursor, one axis lock per program); a curve is created by the labelled action "Automation: add curve…" (a menu over the number and colour handles), not by a nameless glyph. A hidden keyframe record MUST NOT exist: editing a field places a keyframe at the cursor only for a handle whose curve has been created and drawn; for a handle without a curve, editing the field is a constant. The automation controls SHALL have a hint in the status line (rule 10 — no pop-ups).
Why: no editor has "show everything outward" — the interface is assembled by hand; a publication into the "wrong" reel would die in the reconciliation of the same batch; the test over the accessibility tree was green on code that drew the catalogue string; the "+" glyph turned the window into a hidden automation recording mode with not a single curve on screen ("how did such a mistake even get through", R.60) — a violation of "one animation mechanism for everything" (R.34). R.63: entering the reel on publication from the strip is an imposed change of context; a separate parameter window of the instance is "fucking insane" — the owner's ban on windows without an obvious purpose.

#### Scenario: One door
- **WHEN** a right click on the caption from the full entry, from the strip, outside a reel; the instance menu
- **THEN** present, present, absent altogether (the hint explains when reels exist); the instance menu leads to the FX window with the parameters head
- **TEST** t682 (`src/apps/kt-app/src/reach_tests.rs`), t518 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Consumer window, author name, badges
- **WHEN** the FX window of an instance is open with the parameters head; the program language is changed; the instance is overridden
- **THEN** each type with its own widget; reset sends `ClearOverride`; the name is drawn verbatim and does not move with the language; a pin on the event, a number on the card, nothing at zero
- **TEST** t683 (`src/apps/kt-app/src/layout_tests.rs`), t684 (`src/apps/kt-app/src/layout_tests.rs`), t685 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The override curve is visible and created by a named action
- **WHEN** in the parameters head the number handle has no curve, and the field is edited at two different cursor positions; then the curve is created via "Automation: add curve…"; the field is edited at two cursor positions
- **THEN** without a curve the override holds one value and zero keyframes; after creation the window has an automation widget with this curve (the graph/table tabs are found), the keyframes are placed in the material of the instance at the cursor and are visible in the table; there is no "+" glyph in the window; the automation buttons have a status-line hint
- **TEST** t691 (`src/apps/kt-app/src/layout_tests.rs`), t693 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Live publication scenario
- **WHEN** in the real kt-app a handle is published and overridden on the instance; then a curve is created for the handle and a keyframe is moved
- **THEN** the consequence is visible in the frame and in the file; the curve is visible in the window, and the keyframe in the file sits in the material of the instance
- **TEST** t689 (`src/scripts/live/nestparam.py`)

#### Scenario: Publication from the band stays in the band; the effect handle acts on the instance
- **WHEN** the instance is expanded in the strip at the root; a right click on the caption `amount` of the radial blur of the strip's event, with the name confirmed; then the second instance is placed from the bin and its handle is edited in the head of the chain's strip
- **THEN** after confirmation the person is still at the root with the strip expanded (`editing()` has not changed; on the old code — a full entry); the handle is in the reel; editing the second instance changes its frame in the preview (a pixel), the first does not change; in the file the override is on the second
- **TEST** t753b (`src/apps/kt-app/src/layout_tests.rs`), t755b (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/nestparam.py` (steps added)
