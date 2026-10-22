# Spec Delta

## MODIFIED Requirements

### Requirement: Reel preview — offline rendering by the same composition code, with a disk tier
The frames and the envelope of a reel on the timeline SHALL be rendered offline by the same composition code by key (the reel, the revision of the INSTANCE); a request is placed before the draw cycle, by key, once (zoom and scrolling do not generate orders); the render MUST be paused during playback and export; the frames go out before the peaks, until then the envelope is empty; the moment of a frame is the same map "event tick → material" as in the file. The disk tier SHALL lie in the sidecar by key (the reel, the revision, the fingerprint of the media files of the tree) in the task's stream; a DB error is a cache error, not a preview error. The FRAMES SHALL be written to the sidecar right after the frame stage in their own transaction, the peaks — in their own transaction afterwards: a strip that has reached the screen survives to disk even if the audio stage is cancelled. A cancelled task (pause, edit, closing) MUST NOT fall into the negative cache as a failure: the request is placed again at the first pause; only a render error counts as a failure. A second opening of a project with an unchanged reel and unchanged media files SHALL return the frames and peaks from disk without a single render — both on a project with TWO instances of one reel one above the other with different sets of overrides, and on a reel with a missing source (the stub is also a frame, its fingerprint is "missing", the key is stable). The offline render in kt-app SHALL live in one place.
Why: a thumbnail by file is impossible (a reel has no file), but its content is rendered; `content_hash` sees the editing, not the substitution of a file under it; two releases with different titles would show the same frames. R.67: "the composition preview is AGAIN not saved, every time it recomputes the preview" — the write went only after the audio stage, and cancellation made the key a "failure" until the next edit; the owner's sidecar held a strip of forty identical frames that nothing asked for.

#### Scenario: Service rules
- **WHEN** the service receives requests, the revision changes, the reel does not fit in the LRU
- **THEN** the step size, the frame ceiling, deduplication, invalidation by revision; the frames and peaks reach the screen
- **TEST** t599 (`src/apps/kt-app/src/layout_tests.rs`), t600 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Scrolling orders nothing
- **WHEN** the timeline is scrolled and zoomed over a reel
- **THEN** there are no requests, the thumbnail of the reel is not ordered; the renderer in kt-app is set up only by the render dialog and by this service
- **TEST** t601 (`src/apps/kt-app/src/layout_tests.rs`), t602 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Disk tier
- **WHEN** the project is opened a second time; the mtime of the source is shifted
- **THEN** zero renders if frames and peaks are present; a miss and a re-render by fingerprint; a cycle over three tables, a foreign key — a miss, eviction, cleanup
- **TEST** t639 (`src/libs/kt-media/tests/index_db.rs`), t640 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Preview key is per instance
- **WHEN** two instances of one reel with different overrides
- **THEN** the keys are different; the set of live revisions does not discard each other's work; without overrides the key equals the revision of the reel
- **TEST** t680 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Frames survive to disk without peaks, cancellation is not a failure
- **WHEN** the preview task is cancelled right after the frame stage (playback has started); then pause; then the project is closed and opened again; separately — the project in the form of the owner's file (two reels one above the other with overrides, a missing still) is opened twice
- **THEN** after the cancellation the frames are already in the sidecar (the strip row is there, there are no peaks); on pause the request is placed again and the peaks are appended; the second opening — zero renders, the frames and peaks from disk; on the owner's project the second opening — zero renders (on the previous code — a render at every opening); the key is one for both openings
- **TEST** t795 (`src/apps/kt-app/src/layout_tests.rs`), t795b (`src/libs/kt-media/tests/index_db.rs`)

## ADDED Requirements

### Requirement: Instance override curves — the same shapes, sources and block as the slot
The override of a public parameter SHALL accept a curve with any segment shape (including a segment formula), a layer-formula and a source (a formula, a link inside the reel), and the instance window SHALL show for it THE SAME block "curve as a parameter" as the effect slot and the generator: the shape of all segments with parameters, the layer, the link, delete. The axis of the override's curve is the time of the instance; the resolution of sources goes in this domain at the moment of overlaying onto the content of the reel, before the translation into the target's time. A refusal (an address across the reel's boundary) SHALL be named in the status line, not erase the field.
Why: R.67, the owner: "in the composition parameters' automation curve you cannot set the shapes of curves for a whole segment (as in fx) — it turns out we have some duplicate code there instead of a generalization! And if you enter there, for example, t*2 — it just erases the field after enter and nothing happens at all" — validation rejected any source of an override (`CurveSourceUnsupported`), and the instance had no block of forms.

#### Scenario: The instance accepts a formula and shapes, one block
- **WHEN** in the head of the reel instance, `t*2` is typed and Enter is pressed into the override curve's field; a zigzag is set on all segments; a `k*0.5` layer is set; then everything is cleared
- **THEN** the field after Enter holds `t*2`, the curve is a formula, the instance frame changes along it (not erased, not silent); the zigzag and the layer apply to the override and are visible on the graph; the block is the same code and the same labels as the slot; clearing returns the keys
- **PATH** FX window of the instance → head "📌 Reel" → curve "Amount" → field "Formula": `t*2`, Enter → dashed line grows, frame blurs more toward the end → "Shape of all segments: Zigzag" → it jitters
- **TEST** t791 (`src/apps/kt-app/src/layout_tests.rs`), t792 (`src/libs/kt-model/tests/pubparam.rs`), live scenario `src/scripts/live/curves.py`
