# Design

## Context

See proposal.md — Why. What exists in the tree today:

- The shape is one for the whole curve: `ParamCurve::ease: bool`, default `true`. It is computed by
  `curve_value(keys, ease, at)` (`src/libs/kt-model/src/project.rs`) — linear interpolation, with `ease` —
  `smoothstep01`. The field is read in seven places, including `autoplan.rs` (the compiled plan of R.59),
  `warp.rs` and `kt-otio/convert.rs`.
- `CurveSource { Keys, Formula { expr }, Link { path } }` was introduced in R.59 and is rejected loudly by
  validation (t649). The field is not serialized at its default.
- The envelope buffer: copying writes to the system buffer (`ui.ctx().copy_text`) AND to the mirror
  `KtApp::env_clipboard`; the availability of the button is computed from the mirror and by a match of
  `fx_tag` (`src/apps/kt-app/src/fx_editor.rs`). `arboard` is already in the tree through eframe.
- The precedents we rely on: `AudioCurve` — a closed set of crossfade shapes with one function for all
  halves; `Project::addr_spec`/`addr_type` (R.60) — the passport and the type by address; the search for a
  cycle of reels by a traversal from all vertices (t423); `clip_material_at` / `clip_timeline_at` — the only
  translation of time.
- The survey of other editors lies in `docs/features/automation/keyframes/`:
  F-AUT-007 (expressions, accepted), F-AUT-008 (parameter linking), F-AUT-009 (segment shapes).

## Goals / Non-Goals

**Goals:**
- A shape belongs to the segment and has its own parameters.
- The value of a property may be driven by another property or by the text of a formula, and this is visible.
- The envelope buffer is a real system one; pasting into another effect's view is possible.
- A project that uses none of this pays neither a byte nor a tick.

**Non-Goals:**
- Do not make plugins: the contract remains an interface.
- Do not make physics and simulations.
- Do not make linking across the reel boundary (see D3) — public parameters exist for that.
- Do not keep backward compatibility of the format (the owner's direct decision), but do not move
  `GOLDEN_*` either.

## Decisions

### D1. The shape — on the key, with parameters of its kind; `ease` dies

`AutoKey` gets a field of the shape describing the segment AFTER the key. The shape is an enumeration with
data, not a number: the cubic and ease have tension, the zigzag has periods, amplitude and profile.
`curve_value` becomes a dispatcher by the shape of the left key; each shape is a closed function
`(s ∈ [0,1], a, b, parameters)`, like `AudioCurve::rise`.

*Alternative — the shape as a number and a common parameter "stiffness".* Rejected by the consequence of D2:
one number is not enough for a zigzag.

**The default must reproduce today's `smoothstep01` bit for bit.** Otherwise `GOLDEN_AUDIO`/`GOLDEN_VIDEO`
would move, and that is a regression of determinism, not a "new feature". Therefore the cubic with the default
tension is computed exactly as `s*s*(3-2s)`, and this is fixed by a test against a frozen oracle (t695b).

### D1b. The curve format: `ease` remains the WIRE for the uniform shape

Found during implementation. Today `ParamCurve::ease` is ALWAYS written to the file (the field has a `default`
for reading, but no `skip_serializing_if`). If the field simply disappeared, the bytes would change for every
file with curves — and that breaks the requirement "a file without novelties remains with the same bytes" and
the version-cascade test over all committed fixtures (t433b), plus the golden `demo-3d.otio` (t189b).

Decision: **in the MODEL the shape lives on the key, in the FILE it is written by content.** All keys of a curve
carry one and the same shape, and that is the cubic with the default tension — the former `ease: true` is
written; all Linear — `ease: false`; in all other cases per-key shapes are written, and the file gets v10.
Reading: `ease` expands into the uniform shape, a per-key shape overrides it.

This is not the backward compatibility that the owner refused, but the same rule "the version by CONTENT",
applied one level lower: the project pays neither in version nor in bytes for a feature it does not use.
The wording of the previous revision of the spec ("the straight line as default is not serialized") was
imprecise and has been replaced.

### D2. Revision: the zigzag is a SHAPE OF A SEGMENT, not a source

**A revision of the verdict of F-AUT-009, item 3 — the owner's decision (R.61).** The note claimed:
"a zigzag set by a number of periods is not expressible by keys at all, and trying to squeeze it as a sixth
kind into the closed list is a direct path to Smooth 2", and referred the zigzag to the source of the value.
The owner decided otherwise: the zigzag is a shape on the segment between two neighbouring keys.

The reason for the decision: a zigzag between TWO KEYS is a local ornament of a stretch, and it has a beginning
and an end set by the keys; a source, however, describes the curve WHOLLY and has no keys at all. Mixing them
would mean requiring a formula where a person wants "to tremble on this piece".

A consequence that the note predicted correctly and that is accepted together with the decision: the shape must
have ITS OWN set of parameters, not one number of stiffness. Therefore the shape is an enumeration with data
(D1), and adding a sixth kind breaks neither the file nor the screen.

A separate consequence, found while analysing: **the amplitude of the zigzag is set in units of the parameter,
not as a share of the difference between the keys.** The most frequent zigzag is a tremor around a CONSTANT value,
where the difference is zero: a share would collapse the feature exactly where it is needed. The amplitude is
clamped by the passport of the target (`addr_spec`, R.60).

### D3. Link addresses: any event of the project, but NOT across the reel boundary

**The owner's decision — "any event of the project"** (chosen against the recommendation "only one's own
event"). Accepted. Exactly one restriction follows from the invariants, and it does not narrow the decision;
it explains where the "project" has its edge.

A link MUST NOT cross the boundary of a reel. Outward — because a reel that reads an event from outside stops
being self-sufficient: two of its instances would give different pictures, and the law E (inline is identical
to the expanded content, t434–t436) would stop holding. Inward — because "setting a value from outside inside"
is already done and is called the public parameters of the composition (R.59). Thus "any event of the project"
= any event within the same address space, and our space is already closed by the reel (t425).

**Time between different events is computed by the timeline.** The domain of a key is the material of its own
event, and two events in different places of the edit have no common material time. The translation goes through
`clip_timeline_at` of its own event and `clip_material_at` of the source — the same pair that the whole program
uses; we do not introduce a second translation rule. Outside the body of the source its edge value is taken.

*Alternative — compute the link in fractions of the length.* Rejected for the same reason that killed the
fractional domain of keys: rubber.

### D4. The evaluator: one for the link and the formula, topological order, zero cost

The mini-formula — one parser and one evaluator, shared by the recalculation expression of a link and by the
formula source. The set of operations is closed; there are no clocks, files or randomness from the environment
(the determinism invariant). The resolution of derived sources goes ONCE at the build of the renderer, in the
topological order of the dependency graph, next to the compilation of curves (`autoplan.rs`) and the audio
profiles — no appearance in the audio callback.

The dependency graph is built ONLY over events that have derived sources. A project without them builds no graph
at all: this is "zero cost", and it is checked by a test, not by a promise (t705, t710).

A cycle is searched by a traversal from all vertices and prints the path — by the same technique as the cycle of
reels (t423). The reason for "from all vertices": a pair of properties that lead each other and are read by nobody
is still a cycle, and the file with it would be saved but would fail to open.

**The text of the formula and the address of the link enter the hash of the chain and the key of post-FX.**
Otherwise an edit of the formula would leave the old frames on the screen — the same class as "the reel's scale
step in the memo key".

### D5. Non-numeric values — thresholds and snapshots; the axis of the rig is the VALUE, not the time

**The owner's decision — to lead non-numeric properties too** (chosen against the recommendation "only number and
colour"). A flag is led by a threshold, the rest — by snapshots at the marks of the VALUE axis of the leading
property: between the marks the previous snapshot is held. This is not a curve over time, and the two must not be
confused: for a snapshot the axis is the value of the leader, for a key it is the time.

A hysteresis at the threshold is not introduced: the threshold is compared once per frame against the already
resolved number; there is no jitter of the source here by construction. Equality with the threshold is treated in
one direction by an explicit rule and is fixed by a test (t704).

### D6. The envelope buffer: no mirror, parsing on press

The mirror `env_clipboard` is removed entirely. The paste button is always active; on press the system buffer is
read (`arboard`, already in the tree), the content is parsed, and a failure is announced as an error with a reason —
the rule "the impossibility of an action is an error, not silence" (t172). Remapping: parameters that match BOTH by name
AND by type land silently; for the remainder a matching window opens; refusing the matching names the lost curves.

*Alternative — keep the mirror and fix only the remapping.* Rejected: the state would remain, and what was copied in
another window would still not be pasted.

## Risks / Trade-offs

- [Removing `ease` moves the bytes of all files with curves] → D7: the field remains the wire of the uniform shape;
  the guard — t433b (the cascade over fixtures) and t189b (golden `demo-3d.otio`).
- [Changing `curve_value` moves `GOLDEN_*`] → the default shape is bit-for-bit equal to the former `smoothstep01`; the
  guard — the test against the frozen oracle (t695b) plus the export goldens.
- [Addressing across the whole project makes an edit of an event O(edit)] → the graph is built only over the participants;
  a test asserts that the neighbour plan is byte-for-byte the same and the cost does not grow with the number of events
  (t705, t710). If the test shows otherwise — narrow the addressing to the reel and return to the owner, rather than
  "optimize" silently.
- [A zigzag at warp speed will break the closed map] → `rate` accepts only a straight line, the refusal is loud (t696);
  this rule already exists for `ease` and merely becomes wider.
- [The mini-formula turns into a language] → the set of operations is closed by a list in the spec; extending the set is a
  separate decision, not an in-place edit.
- [A link and public parameters do the same thing at the reel boundary] → the boundary is described in D3 and checked by a
  test from both sides (t702b).

## Open Questions

- The profile of the zigzag (triangle/rectangle/sine) — three kinds or one with a duty-cycle parameter. Decided
  when writing t695: if three closed functions are shorter than one parameterized one, three are taken.
- Whether an action "turn a derived curve into keys" with a choice of sampling step is needed, or the frame grid
  step is enough. Decided in S3 on the spot; the spec fixes only the existence of the action "unlink".
