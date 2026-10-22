# Spec Delta

## MODIFIED Requirements

### Requirement: Public parameters — the template value is not stored, overrides come before morpher layers
The reel SHALL declare handles (id, the author's name, the target address, type, narrowing of limits); the
instance SHALL override ONLY what is published, by a value or by a curve. The template value MUST NOT be
stored — it is read live from the target. The resolution of the reel for an instance SHALL go through one
door; the overrides are laid BEFORE the morpher layers; an empty set — the former path without clones.
The curve lives in the material of the instance (the reel's time); the carriers of a curve are the slot,
the generator, the nesting. The address is glued to the id of the target: it survives a trim, a move, the
left half of a cut; if the target has vanished or changed its type — the publication and all the overrides
go away in one step, on each batch, without a list of triggers; validation rejects orphans and foreign
types. The expansion strip shows the TEMPLATE, expansion bakes and names the number.
The narrowing of the limits of a numeric handle SHALL by default equal the passport of the TARGET (`min`/`max`
of its `ParamSpec`, obtained by the address): the author window shows the limits of the target, not zero;
only the narrowing is written to the file — a value equal to the limit of the target is stored as absence
(`None`); reset to the limits of the target — one action. The value at the consumer SHALL be clamped to the
limits of the target and, on top of them, to the author's narrowing.
Why: a stored copy would have diverged from the reel at the first edit inside it; the morpher appends slots
to the end — after it the address of a slot would point to the wrong place; "min/max always 0" — an empty
`Option` was drawn as zero, while the passport of the target already knows its own limits (R.60).

#### Scenario: Commands and two questions
- **WHEN** a handle is published, overridden, removed
- **THEN** six commands with undo; "what is in the template" is read from the target, "what plays here" — with the override
- **TEST** t671 (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: The target's fate
- **WHEN** the target is trimmed, cut, moved, deleted or changes its type
- **THEN** the address survives a trim and the left half of a cut; the death of the target takes the publication and the overrides away in one step; orphans and foreign types are rejected by validation
- **TEST** t672 (`src/libs/kt-model/tests/pubparam.rs`), t672b (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: An empty set — the former path
- **WHEN** an instance without overrides is resolved
- **THEN** transitions and cameras are borrowed (`Cow::Borrowed`), not a single clone of the reel
- **TEST** t673 (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: The band shows the template, expansion bakes it in
- **WHEN** an instance with overrides is expanded by the strip; it is expanded
- **THEN** the strip shows the values of the template; after the expansion the values are in pieces, and the number of baked ones is named
- **TEST** t690 (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: Handle limits by default — the target's passport
- **WHEN** a numeric handle has just been published; the author narrows `max`; the author returns `max` to the limit of the target; the author presses "reset to the limits of the target"
- **THEN** the author window shows `min`/`max` of the target (not 0); `PubRange` holds only the narrowing (`min: None`); a value equal to the limit of the target is again `None`; after the reset `PubRange::is_empty()`, and a file without narrowings is byte-for-byte as before
- **TEST** t692 (`src/libs/kt-model/tests/pubparam.rs`), t692b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The consumer's field is clamped to the target's limits
- **WHEN** the consumer drags the value of a handle without the author's narrowing beyond the limit of the passport of the target; with the narrowing — beyond the narrowing
- **THEN** the value does not go beyond the passport of the target; with the narrowing — does not go beyond the narrowing; in the model and in the field it is the same number
- **TEST** t692b (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Publishing interface — one door, the author's name verbatim, two badges
The publication SHALL be reachable from one door — a right click on the label of a field inside the
composition (the full entry or the strip); outside a reel there is no item, and the hint explains it — only
when compositions exist in the document. The owner reel SHALL be counted by the working view (an event of
the strip belongs to the strip's reel). The consumer window SHALL draw each type with its own widget, a reset
sends the removal of the override; a session of the window is one step of history. The name of the handle
SHALL be drawn verbatim, bypassing the translation catalogue. Badges: a pin at the tail of the event's label,
a number at the head of the reel card; zero is not written.
The override curves SHALL be VISIBLE in the consumer window with the same automation widget as that of the
effect slot (graph ↔ key table, selection of the active curve, a key at the cursor, one axis lock for the
whole program); a curve is created by the labelled action "Automation: add a curve…" (a menu over the number
and colour handles), not by an unnamed glyph. Hidden recording of keys MUST NOT exist: editing a field puts a
key at the cursor only for a handle whose curve is created and drawn; for a handle without a curve, editing the
field is a constant. The automation controls SHALL have a hint in the status line (rule 10 — no pop-ups).
Why: no editor has "show everything outward" — the interface is assembled by hand; publication into the "wrong"
reel would have died in the cross-check by the same batch; a test over the accessibility tree was green on code
that drew the catalogue row; the glyph "+" turned the window into a hidden automation recording mode without a
single curve on the screen ("how did such a bug even get through", R.60) — a violation of "one animation
mechanism for everything" (R.34).

#### Scenario: One door
- **WHEN** a right click on the label from the full entry, from the strip, outside a reel; the instance menu
- **THEN** present, present, absent entirely (the hint explains when reels exist); the instance menu leads to both windows
- **TEST** t682 (`src/apps/kt-app/src/reach_tests.rs`), t518 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Consumer window, author name, badges
- **WHEN** the parameter window of the instance is open; the program language is changed; the instance is overridden
- **THEN** each type with its own widget, a reset sends `ClearOverride`; the name is drawn verbatim and does not move with the language; a pin on the event, a number on the card, nothing at zero
- **TEST** t683 (`src/apps/kt-app/src/layout_tests.rs`), t684 (`src/apps/kt-app/src/layout_tests.rs`), t685 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The override curve is visible and created by a named action
- **WHEN** in the instance window a numeric handle has no curve and the field is edited at two different cursor positions; then the curve is created through "Automation: add a curve…"; the field is edited at two cursor positions
- **THEN** without a curve the override has one value and zero keys; after creation the window has the automation widget with this curve (the probes of the graph/table are found), the keys landed in the material of the instance at the cursor and are visible in the table; there is no glyph "+" in the window; the automation buttons have a status-line hint
- **TEST** t691 (`src/apps/kt-app/src/layout_tests.rs`), t693 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Live publication scenario
- **WHEN** in the real kt-app a handle is published and overridden on the instance, then a curve is created on the handle and a key is moved
- **THEN** the consequence is visible in the frame and in the file; the curve is visible in the window, the key in the file is at the material of the instance
- **TEST** t689 (`src/scripts/live/nestparam.py`)
