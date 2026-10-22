# Spec Delta

## MODIFIED Requirements

### Requirement: Version of the `kt` envelope — one per file, set BY CONTENT
`Timeline.metadata.kt.v` SHALL be the only file version; nested version envelopes do not
carry one. The writer SHALL set the lowest version at which an older build would read the file
without silent corruption: v2 — cameras; v3 — curves in material ticks, switches, transition kind and
audio curve; v4 — morpher layer and regions; v5 — curves and layer strength; v6 — nested reels;
v7 — inverted and overlapping transitions; v8 — an event left of zero in the reel; v9 — a generator on an
event, public parameters and overrides; **v10 — the segment shape on a key, working sources of a curve (a link with an address and an expression, a formula with text) and the project's default form** (one version per round, a single predicate).
A file newer than the supported version SHALL be rejected with `KtMetaTooNew`; unknown fields of our
envelope SHALL pass through as passthrough. A file without new features MUST remain byte-stable and at its
earlier version — the cascade is checked over all committed fixtures.
**BREAKING (model):** the form moves from the curve to the KEY; the `ParamCurve::ease` field in the model
is removed. In the FILE the form is written BY CONTENT, by the same rule as the version: a curve whose
keys all carry one form SHALL be written with the previous `ease` field (cubic —
`true`, linear — `false`) and the version is not bumped; a curve with different forms, or with a form not
expressible by this field, SHALL write the form on the keys and raise the file to v10. Reading:
`ease` expands into a uniform form; a form on a key overrides it. A link to a vanished property and an unparseable formula SHALL be rejected on read with the address and the
reason.
Why: the owner does not require backward compatibility, but "ours so that nobody breaks": a
refusal is more honest than silent corruption, and a condition built into the cascade in the wrong place raises the version for every
file in a row; a silent change of form during migration would shift the frames of finished projects.

#### Scenario: A file newer than the build is rejected, the version is read
- **WHEN** the envelope has `v: 999`; in an ordinary file `v: 1`
- **THEN** the version check gives `KtMetaTooNew(999)`; the version is read as 1
- **TEST** t146 (`src/libs/kt-otio/tests/format.rs`)

#### Scenario: Cameras — v2, automation — v3, without them — v1; future fields pass through
- **WHEN** a project without cameras, a project with cameras without curves, and a project with cameras and curves are written; an unknown field is put into the envelope
- **THEN** the versions are 1/2/3 respectively; without cameras there are no camera keys; the round trip is idempotent; the unknown field comes back; the output is byte-for-byte equal to the golden `demo-3d.otio`
- **TEST** t189, t189b (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Effect automation raises to v3, a broken curve is a refusal
- **WHEN** a flat project with a blur-radius curve is saved; then in the file a key with a negative position; then `auto` is replaced with garbage
- **THEN** v3, the round trip field-for-field and byte-for-byte; both corrupted files fail the load explicitly
- **TEST** t192 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The morpher layer and areas — v4, the natural order of rows is not stored
- **WHEN** a morpher becomes a "by time" layer for everything below; a named region is added to the project; the order of rows is shuffled
- **THEN** the file is v4, the layer defaults are not serialized; the region survives the round trip; the order is written in its own field `lane_order` outside `ui`; a project without morphers/regions/reorderings carries none of these keys and does not bump the version
- **TEST** t240, t241, t248 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The version cascade does not move old files
- **WHEN** every committed fixture `src/fixtures/*.otio` is read and re-saved
- **THEN** the envelope version is the same as it was, the output is byte-for-byte equal to the source, and there are no attachments in them
- **TEST** t433b (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Public parameters and overrides — v9
- **WHEN** a reel declares a handle "title text", a second instance carries its own value; an ordinary reel without publications
- **THEN** the first file is v9, the set and the overrides survive the round trip idempotently; the second contains neither `published` nor `overrides` and remains v6
- **TEST** t681 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: A shape, a link and a formula raise to v10
- **WHEN** a project without new features is saved; then a project with a segment shape; then with a link; then with a formula
- **THEN** the first remains its previous version and byte-for-byte the same, the others get v10; there is one version in the file
- **TEST** t698b (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The v10 round trip and the uniform shape in the former field
- **WHEN** a v10 file with different forms on keys is read and written; then a file with the old `ease` field in both values is read; then a curve whose keys all carry one form is written
- **THEN** the round trip is identical; `ease = true` became the cubic with the previous smoothing, `ease = false` — linear; the curve values after reading are bit-for-bit equal to the values before it; the uniform curve is written with the previous `ease` field, the bytes and the file version did not move
- **TEST** t698c (`src/libs/kt-otio/tests/convert.rs`), t433b (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: An unresolvable source in the file — an error with an address
- **WHEN** a file is read where a link points to a vanished event and a formula does not parse
- **THEN** the read is rejected with the address and the reason, rather than silently drawing the curve from the keys
- **TEST** t698d (`src/libs/kt-otio/tests/convert.rs`)
