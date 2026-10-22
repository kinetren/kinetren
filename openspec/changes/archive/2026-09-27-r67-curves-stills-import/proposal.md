# Proposal

## Why

The owner sent thirteen complaints about three of his own projects (`sources/26.09.26/`), and the reconnaissance (`docs/user-feedback.md`, R.67) reduced them to four classes: an old file does not open; an automation block neither shows nor lets one edit what it itself stores (the segment shapes are not drawn, "the shape of all segments" edits the invisibly selected curve, the formula preview counts time from the wrong start, a link cannot be assigned, a formula for composition parameters is rejected silently, and a field with a curve writes keys by itself during play); convenience that is "not for the user" (the frame in the preview is off and not remembered, the scheme is clipped, the head of a strip is long and blue, the fields grow beyond the window); and two data defects — the reel previews do not survive to disk, and stills are "Loading…" forever. The seventh item — "HOW MANY TIMES MUST I REPEAT SO THAT ALL AGENTS THINK ABOUT THE USER" — requires not one more rule in prose, but a mechanism that holds the rule by machine.

## What Changes

- **Import of projects of old versions — a separate door.** `File → Import older Kinotrain
  project…` reads a file of an old version's envelope, applying the fixes of the old forms (the first one —
  `targets: null` of the morpher in v5 ⇒ "everything below"), and opens it as a document of the current
  form. The ordinary "Open" rejects such a file with its version and with the name of the import item.
  **Rule for agents**: if you broke the reading of an old file, the fix goes into the import, not into the
  reader, and it retires together with the versions (the owner's decision).
- **The curve is keys + a shape/formula on each segment + a layer-formula + a link, and all of it is visible.** The formula also becomes the SHAPE OF A SEGMENT (its own on a stretch, like the zigzag); a LAYER-formula that sees `k` — the value of the keys with their shapes — is placed over the whole curve; the link (the view curve) gets the door "Driven…" with the choice of event and property. No step erases the previous one: they are removed separately. The file is v13 by content.
- **The graph draws the shape as it sounds**: the line is built by the same evaluator of the value, not by segments between keys; a segment has a handle of the shape's parameter (tension/amplitude/periods) under the mouse; the formula preview counts `t` from the start of the material, as the render does.
- **The block "curve as a parameter" is one for all carriers** (slot, generator, instance of a reel): the shape of all segments and its parameters, the formula layer, the link, delete — are bound to the curve that the person SEES as selected, and for an instance of a reel this block is the same; the overrides of an instance accept sources (formula, link, shapes) and are resolved in the domain of the reel.
- **A field with a live curve writes a key only from a person's gesture**: the widget's clamp and an external change of value do not count as an edit; during play the field shows and is edited as usual, there is no automation recording mode (the owner's decision).
- **Help on the formula — in the block and in the status line**, without popups: variables, functions with their arity, operators; Tab completes a name by prefix; an error — with the position and with the name that does not exist.
- **The width of fields does not depend on the window width** — the numeric fields, the formula field and the choice of shape keep a fixed width by the gallery of the longest value.
- **The frame of the geometry in the preview is enabled by default for the selected event; switching it off is remembered** (the user storage); the scheme in the "Transform" form fits the frame into its rectangle at any zoom.
- **The head of the instance strip of a reel** — a short label (pin + the name of the reel, truncated like the slot's name), the colour — not the accent; the selected one is not stretched.
- **The reel preview survives to disk**: the frames are written to the sidecar right after the frame stage, the peaks — in their own transaction afterwards; a cancelled task does not become "failed" but is queued again at the first pause; a second opening of the owner's project does not re-render.
- **Stills**: a single picture (JPEG/PNG) — a full-fledged source of any duration with a fixture and tests; a missing picture — a stub with the name, rather than an endless "Loading…"; the path of added media is made absolute at the moment of adding and relative to the project file only on writing.
- **rendereg3** — a comparison of the frames of the region between the tag `tested-48` and the current build, with a named cause and a regression test, if a difference exists.
- **The mechanism "thinking about the user"** (the owner's decision: "both, but not on my projects"): (1) each new UI scenario (test ≥ t783 in the UI capability) requires a line `PATH:` — the mouse steps and what the person sees; without it the check is red; (2) the report of the work carries the screenshots of the live scenarios on OUR fixture projects, and `finish.sh` prints their list.
- NOT included: an automation recording mode during play (touch/latch), a stack of modifiers with time scopes (the owner chose the combination "segment + layer"), Bezier handles for speed (F-AUT-001 "candidate"), media relinking, fixing the reader for v5 (the decision: import only).

## Capabilities

### New Capabilities

(none — all the work falls on existing capabilities)

### Modified Capabilities

- `automation`: a formula as the shape of a segment (`SegShape::Formula`), a layer-formula of the curve with the variable `k`, the domain `t` of the formula — from the start of the material; the sources and shapes of instance overrides are resolved in the domain of the reel; a link is assigned by the address of the property of the selected event; a key is set only by a gesture.
- `ui-timeline`: the graph draws the value by the evaluator; the handle of a formula parameter; the block "curve as a parameter" is one for slot/generator/instance; the link door; the formula help in the block and in the line; the widths of fields are fixed; the geometry frame is enabled by default and remembered; the scheme fits; the head of an instance strip is short; a field with a curve does not write by itself.
- `ui-reach`: the item for importing an old version is reachable and named; "Open" rejects with the address; the link door and the formula help are reachable; the contract "PATH:" for new UI scenarios and the screenshots of live scenarios in the report.
- `compositions`: instance overrides accept sources and shapes; the frames of the reel preview are written to disk before the peaks; cancellation ≠ failure; a second opening does not render.
- `project-format`: v13 by content (segment formula, layer); the import of old versions as a separate door with fixes that retire together with the versions; the relative path of media is computed from the project file on writing, the absolute one — on adding.
- `media-index`: a single picture — a source with a frame at any duration; a missing picture — a stub with the name without "Loading…".

## Impact

- `src/libs/kt-model`: `SegShape::Formula`, `ParamCurve.layer`, resolution of the layer and the segment formulas into keys in the same pass as the links and source formulas; validation of overrides accepts sources; `project.rs`/`links.rs`/`formula.rs`.
- `src/libs/kt-otio`: v13 by content; the module of import of old versions with fixes (`legacy_import.rs`), `meta.rs`/`convert.rs`; relative paths on writing.
- `src/libs/kt-media`, `src/libs/kt-ffmpeg`: a single picture as a source (one frame position at any tick), fixtures `fixtures/gen.sh` (JPEG, PNG with alpha).
- `src/apps/kt-app`: `automation.rs` (drawing by the evaluator, the shape handle, help, the formula and layer field, the link door), `widgets.rs` (the common block "curve as a parameter", protection of the field from self-writing, fixed widths, the head of the strip), `pubparam.rs`/`fx_editor.rs`/`gen_ui.rs` (the common block), `geomctl.rs` (the default and the memory of the frame, fitting the scheme), `reel_preview.rs` (writing frames before peaks, cancellation ≠ failure), `app.rs`/menu (import), `user_store.rs`; i18n catalogs ×5; tests t783–t799; live scenarios `curves.py`, `stills.py`, `import_old.py`.
- `src/scripts`: `specs-user-path.py --check` (the line `PATH:` in new UI scenarios), `finish.sh` prints the list of screenshots of the live scenarios; `live-check.sh` — new scenarios.
- `docs/features`: new notes F-AUT-010 (formula as segment and layer, combination of sources), F-AUT-011 (the parameters of the shape under the mouse on the graph), F-HLP-034 (help on the formula in the input field); the verdicts F-AUT-008 (the link door) and F-MED-003 (stills) are updated after archiving.
- `CLAUDE.md` (root and kt-app): the rule "fixes for old files — in the import", the rule "`PATH:` in a UI scenario" and "screenshots in the report".
