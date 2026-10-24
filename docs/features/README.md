# Features of other editors and our future ones

`openspec/specs/<cap>/` answers the question "which defect was closed and by which test", `docs/user-feedback.md` — "what the user wanted and why the decision is exactly this", `docs/architecture.md` — "how the engine is built", and the `CLAUDE.md` of the crates — "how to write code now". This directory answers the question **"what another editor can do, thanks to what NON-OBVIOUS detail it works, and what we take from it"**.

Opened in R.36 (`docs/user-feedback.md`); the decision and its justification — item 245. The reason: the morphers were invented from the inside, and then it turned out that Vegas has Adjustment Events, where a layer HAS an overlay opacity slider, and we do not. Such discrepancies must be found before reddit talks about them.

**The base is organized BY FEATURE, not by editor.** One feature — one note; the comparison of five editors lives inside it as a table "Who has it and how". With an axis by editor, the same feature would be described five times, and the verdict "take / do not take" would have no place to live.

## Maintenance rules (mandatory for agents)

1. **Read BEFORE working on a new FEATURE** (not a defect). If a note exists — read "Verdict" and "How it fits Kinotrain"; if not — first open a note, then write code.
2. **One note = one "trick of the feature".** The section is mandatory and must contain a detail that is NOT in the feature list on the product's website. If there is nothing to write, do not open a note — instead a line in the coverage table "checked, we have nothing of our own". The feature list is on Wikipedia; this base is not needed for it.
3. **Version and date are mandatory.** The single reference — `docs/features/_editors.md`, it is updated FIRST when a new wave of notes starts.
4. **A quote is verbatim, ≤ 2 sentences, with a link and a date.** "Users complain" is not feedback. Negative feedback is no less valuable than praise. If a live quote could NOT be found (the site is unavailable, the search budget is exhausted) — write in the section the line `NO QUOTES:` and then what was searched and why it was not found. An honest gap is better than an invented quote; `--check` accepts such a mark on a par with a quote and does not accept an empty section.
5. **"How it fits Kinotrain" is a reference to a capability, not to the project.** The section must carry the line `CAPABILITY: openspec/specs/<cap>/` (an existing directory; the contract is in its `spec.md`, the "why" and the paths in code are in `design.md`). For the statuses `done`/`accepted`/`already-have`/`rejected` the section is FROZEN and no longer than five non-empty lines: `CAPABILITY:`, one line of the essence of the decision, plus the mandatory wave lines (`ADDRESS:`, `NO NETWORK:`, `NETWORK COST:`). For `candidate`/`researched` the section still names code paths (`src/…`) and invariant numbers (1–12 of the root `CLAUDE.md`) or UI rules (1–20 of `src/apps/kt-app/CLAUDE.md`) — without this the note does not differ from an overview; a reference to a capability is desirable for them. Kinotrain's project decisions live in `openspec/specs/<cap>/design.md`, not here (change `bootstrap-specs-from-chronicles`, 2026-09-21: 4,000 lines of our decisions were squeezed out of 46 notes into capabilities).
6. **Status `rejected` — only with a reason** in the section "Verdict". A reason outlives its author; "not needed" is not a reason, and in a year someone will open the note again.
7. **Status `done` freezes the note.** The body is no longer edited — it becomes a historical document "why we decided so"; only the header and "Verdict" live on. The further life of the feature is in `openspec/specs/<cap>/`. Without this rule the base would turn into a second product documentation and diverge from the code.
8. **A note does not design the implementation.** Reconnaissance, evaluation, verdict — yes; the data schema and the order of stages — in an OpenSpec change (`openspec/changes/<name>/design.md`), and the long-term "why" after archiving — in `openspec/specs/<cap>/design.md`. Otherwise we get fifty stale half-specs (exactly this happened by R.59: a third of the base's volume was our project documents).
9. **There are no placeholder files.** A file appears when its note is written; what is not covered is visible in the table below. Otherwise `grep` would stop distinguishing what is done from what is planned.
11. **In the category `help/` the verdict has a FORM.** The note must carry a line of the form `TEST:` · `CHECK:` · `REJECTED:` · `ALREADY-HAVE:` · `DEFERRED:`, and the tail must name an ADDRESS (a test name, a path `src/…`/`site/…`/`docs/…` or a clause of `--check`). The first address is not repeated for two notes of the category. Reason: the conclusion "we need help" is known before the reconnaissance, and without a form it would have been written 33 times. The dispute about hover tooltips is resolved (R.40 unfroze item 14 of rule 8), but a note touching the prohibition must analyze t83 in "How it fits" and carry `ui_rules: [10]`.

12. **A note relying on a tracker must carry a COUNT and an ADDRESS.** If it declared a non-empty `trackers:` in the header, it must contain in "What users say" a line `SIGNAL: <topic> — N tasks in <key>, `<exact filter>`; share X% against Y% in <other key>`, and in "How it fits Kinotrain" a line `ADDRESS: src/… — …`. The filter of the first `SIGNAL:` line is not repeated for two notes. Reason: "N bugs about proxies in Kdenlive" is a statistic, not a finding; a finding begins where the number of one tracker is compared with the number of another and runs into our code. The keys come from `docs/features/_trackers.md`, which is updated FIRST (the same order as `_editors.md` in rule 3). The numbers themselves are not checked by `--check` — they are reproduced by `tracker-rank.py --query`; `--check` must remain green offline, and there are no dumps in the repository.

13. **In the category `collab/` the note must separate the feature into offline and network.** In the section "How it fits Kinotrain" there are two lines:

    `NO NETWORK: <REMAINS|DEGENERATES|DOES NOT REMAIN> — `<anchor src/…>` — <what works on ONE machine, or where it is visible that nothing works>`

    `NETWORK COST: <what it requires from the other side> — <invariant N | mechanism from the dictionary> — <why the feature is not expressed without it>`

    The anchor of the `NO NETWORK:` line must point to a file that **exists in the tree today** — `--check` verifies existence, not the form of the path — and must not be repeated for two notes of the category. The anchor is mandatory in ALL three branches: in `DOES NOT REMAIN` it points to the place where the absence is visible, otherwise this branch would not fall under the uniqueness check and a defect would pass in disguise. The tail after the marker, WITHOUT the anchor, is not shorter than 80 characters.

    The closed dictionary of mechanisms: `user account` · `shared clock` · `edit log` · `access rights` · `project-as-database` · `cross-project storage` · `external executor` · `media address`. **The word "network" is deliberately absent from the dictionary:** its absence was known BEFORE the reconnaissance (as of 2026-09-08: 0 matches for `std::net|TcpStream|reqwest|hyper|tokio` in all of `src/`, not a single HTTP/TLS crate among the 505 packages of `Cargo.lock`; since 2026-10-22 the only network path is anonymous telemetry `src/apps/kt-app/src/telemetry.rs` via `ureq`, F-COL-046; it carries no collaborative work), and "we have no network, therefore we do not take it" is not a finding but one and the same fact, rewritten forty-five times. The value of the wave lies in what remains of a cloud feature when there is no network.

    The invariant named in `NETWORK COST:` must also stand in the `invariants:` field of the header — the first check in the base of "prose against the header". The `services:` field in `collab/` must be non-empty; the keys come from `docs/features/_services.md`, which is updated FIRST.

    Requirement of an existing anchor is not an exception to rule 8, but its execution: the note describes TODAY's code. `src/libs/kt-collab/src/session.rs` — this is already an implementation schema, and it will not build.

10. **After each batch — `python3 src/scripts/features-index.py --check`**, and an update of the coverage table. The summary table is inserted BY HAND from the output of `--table`: there is not a single generated markdown in the repository (rule 20 of the checklist — nothing grows by itself).

## Note header

The YAML frontmatter is the only place in `docs/` with a machine-readable format; it was set up by direct decision of R.36 ("create a DB"). The dictionaries are closed, `--check` verifies:

| Field | Values |
|---|---|
| `id` | `F-<CODE>-<NNN>`, a sequential number within the category, never reused |
| `status` | `researched` · `candidate` · `accepted` · `in-progress` · `done` · `rejected` · `already-have` |
| `priority` | `A` — without this the product does not count as a video editor · `B` — a noticeable loss, the editor works around it · `C` — niche |
| `cost` | `S` — one stage of a round · `M` — 2–3 · `L` — a whole round · `XL` — a round plus a change of format or invariant |
| `we_have` | `yes` · `no` · `partly` |
| `editors` | keys from `_editors.md` |
| `invariants` / `ui_rules` | numbers from the root `CLAUDE.md` / `src/apps/kt-app/CLAUDE.md` |
| `round` / `point` / `tests` | filled in when the feature is accepted and done |
| `checked` | the date of the check; `--check` complains about an age > 18 months for statuses `candidate`/`accepted` |
| `trackers` | keys from `_trackers.md`; the field is optional, but a non-empty one activates rule 12 |

**The ID is the address, the path is a label.** A note can be renamed and moved to another subcategory; the ID does not change: it gets into commit messages (`RGB and Luma curves (F-CLR-002, item 253, R.37 S1)`), where the path `docs/features/color/primary/curves-rgb-and-luma.md` is unreadable.

## Categories

| Directory | Code | About | Written / planned |
|---|---|---|---|
| `morpher/` | MPH | adjustment layers: ours against someone else's | 8 / 8 |
| `color/` | CLR | color correction, scopes, LUT, color management | 8 / 8 |
| `masks/` | MSK | masks, keying, tracking, stabilization | 6 / 6 |
| `audio/` | AUD | audio processing and mixing | 12 / 9 |
| `timeline/` | TL | operations on events, edit structure | 12 / 5 |
| `titles/` | TXT | titles, generators, subtitles | 6 / 4 |
| `time/` | SPD | speed, retiming, synchronization, frame rate conform | 5 / 5 |
| `automation/` | AUT | keyframes, curves, presets | 11 / 8 |
| `media/` | MED | media organization, proxies, sources | 6 / 3 |
| `export/` | EXP | presets, queue, smart render | 4 / 3 |
| `project/` | PRJ | sequences, history, versions | 5 / 2 |
| `ui/` | UI | viewer, input, layouts | 5 / 3 |
| `plugins/` | EXT | OFX/VST hosts, scripting, the contract of foreign code | 3 / 3 |
| `help/` | HLP | hints, help, learning, documentation | 34 / 34 |
| `resilience/` | RES | keeping work safe, recovery, behavior under load and on foreign hardware | 12 / 12 |
| `collab/` | COL | not working alone: claim, merge, say something about a frame, hand off, distribute material, pass on the bill, depend on the other side | 46 / 46 |
| `compositing/` | CMP | how a frame becomes a frame: transform concatenation, resampling quality, resolution of intermediate stages, transition vocabulary, blend modes | 3 / 3 |
| `fx/` | FX | video effects of a clip: stylization, distortions, blurs, 3D transforms, simulations | 6 / 6 |

A subcategory is a stable taxonomy declared here; it is not created or deleted on a whim. The directories are not numbered (`01-timeline/`): adding a category would reshuffle all the paths.

`morpher/` is the only category with a subcategory `gaps/` (what is missing in OUR product). This is the only place where we are ahead of the market, and the difference from Vegas/AE is the reason for the whole work.

`collab/` was set up in the fourth wave (R.42). The axis is **by ACTION, not by place**: not "where it lies", but what a person tries to do together with someone else. "Cloudiness" is a dimension INSIDE a note, not an axis: a bin lock works the same way on a shared disk and in the cloud, and the category `cloud/` would repeat the mistake that item 245 already rejected for an axis by editor. Seven verbs: `claim/` — take (to NOT LET them diverge) · `merge/` — reconcile (what to do when they HAVE already diverged) · `review/` — say something about a frame (speaks without touching) · `handoff/` — hand off and receive (the EDIT has left) · `assets/` — distribute material (the BYTES have left) · `offload/` — pass on the bill (the computation is done by a process that is NOT ours) · `depend/` — depend (the other side as a condition of work).

Boundaries with the outside: **`resilience/` — "the MACHINE will survive", `collab/` — "a SECOND PERSON will survive"** (one line `app.rs:1063` gives output in both: there "there is no second state on disk", here "there is no check that the state on disk is not someone else's"); `help/` — the program tells the person, `collab/` — a person tells a person THROUGH the program; `project/` — the structure of MY document, `collab/` — the fate of the document with the second party; `export/` — the queue and output parameters, `offload/` — the form of a job that can be carried off the machine.

**The doctrine of three registries.** `_editors.md` — what we compare with (verdict), field `editors:`. `_trackers.md` — where the number comes from (count), field `trackers:`. `_services.md` — what stands on the other side of the network (cost), field `services:`. The separation is not aesthetic: `read_editor_keys()` takes any line of its file with a slug in the first cell, and a third table there would silently make `frame-io` a legal value of `editors:`.

`resilience/` was set up in the third wave (R.41). The boundary with `help/` is drawn like this: **`help/` — "how the program will SAY this", `resilience/` — "what the program will DO in this case"**. The report of a crash lives in `help/errors/`, and what survives the crash — here. The category appeared not because a new source appeared, but because the vendor documentation does not describe what it costs: the first two waves read the selling side, and the domain was not visible.

## Summary priority table

Generated by `python3 src/scripts/features-index.py --table` and inserted here by hand.

<!-- table:begin -->
| ID | Feature | Status | Prio. | Cost | We have | Note |
|---|---|---|---|---|---|---|
| F-AUD-001 | Parametric equalizer | done | A | S | yes | `audio/processing/eq-parametric.md` |
| F-RES-007 | "In the preview there was one thing, and in the export another" | already-have | A | S | yes | `resilience/environment/preview-vs-export-mismatch.md` |
| F-AUD-009 | Delay / Echo on an event | done | A | M | yes | `audio/processing/delay-echo.md` |
| F-CLR-008 | Basic color correction by an event — ColorGrade, Duotone, Posterize | done | A | M | yes | `color/grading/basic-color-fx.md` |
| F-FX-001 | Stylization — Vignette, Pixelate, Halftone, Sharpen | done | A | M | yes | `fx/stylize/vignette-pixelate-halftone-sharpen.md` |
| F-FX-004 | 3D transforms — Corner Pin and Cylinder | done | A | M | yes | `fx/three-d/corner-pin-and-cylinder.md` |
| F-RES-008 | Installation and packaging: an application that did not start | candidate | A | M | partly | `resilience/environment/packaging-and-install.md` |
| F-RES-009 | Codec and format: "it does not open my file" | candidate | A | M | partly | `resilience/environment/codec-and-format-availability.md` |
| F-CLR-001 | Lift / Gamma / Gain wheels (and Offset) | candidate | A | L | no | `color/primary/lift-gamma-gain-wheels.md` |
| F-CLR-002 | RGB/Luma curves and "by attribute" curves | candidate | A | L | no | `color/primary/curves-rgb-and-luma.md` |
| F-FX-002 | Distortions — Lens, Twirl, Ripple, Kaleidoscope, Chromatic Aberration | done | A | L | yes | `fx/distort/warp-family-and-chromatic-aberration.md` |
| F-MED-003 | Images, frame sequences and material with alpha | candidate | A | L | partly | `media/sources/image-sequences-and-alpha.md` |
| F-MED-004 | Frame conform rule — fit, fill, stretch or leave | in-progress | A | L | partly | `media/sources/import-scaling-policy.md` |
| F-TL-001 | Ripple — an edit that shifts everything to the right | candidate | A | L | no | `timeline/trimming/ripple-delete.md` |
| F-TXT-001 | Text in frame: titles, captions, lower thirds | done | A | L | yes | `titles/text/text-generator.md` |
| F-CMP-001 | Transform concatenation | done | A | XL | yes | `compositing/transforms/transform-concatenation.md` |
| F-TL-004 | Nested sequences — an edit that behaves like a clip | done | A | XL | yes | `timeline/structure/nested-sequences.md` |
| F-TL-011 | Inline expansion of a nest IN PLACE | done | A | XL | yes | `timeline/structure/inline-nest-expansion.md` |
| F-TL-012 | Public parameters of a composition | accepted | A | XL | yes | `timeline/structure/composition-public-parameters.md` |
| F-AUD-011 | Distortion / saturation and Bitcrusher | done | B | S | yes | `audio/processing/distortion-and-bitcrush.md` |
| F-AUD-012 | Stereo Width (mid/side width) | done | B | S | yes | `audio/processing/stereo-width.md` |
| F-COL-002 | Service material next to the project — cache, index and lock on a shared disk | candidate | B | S | partly | `collab/assets/sidecar-on-a-share.md` |
| F-COL-006 | Who has the right to delete material | candidate | B | S | partly | `collab/assets/who-may-delete.md` |
| F-COL-008 | What a foreign file loses when it passes through us | candidate | B | S | partly | `collab/handoff/foreign-file-round-trip.md` |
| F-COL-013 | Two processes over one project file | candidate | B | S | partly | `collab/claim/single-writer-beside-the-file.md` |
| F-COL-014 | A lock that outlives its owner | candidate | B | S | no | `collab/claim/stale-lock-and-takeover.md` |
| F-COL-017 | Where the lock lives — in the file, beside the file, or in a database | researched | B | S | no | `collab/claim/where-the-lock-lives.md` |
| F-COL-021 | Two notes on one frame | researched | B | S | no | `collab/review/two-notes-on-one-frame.md` |
| F-COL-030 | The conflict-resolution dialog, the empty fourth year | researched | B | S | partly | `collab/merge/empty-conflict-dialog.md` |
| F-COL-032 | Whoever saved last wins | candidate | B | S | partly | `collab/merge/last-writer-wins.md` |
| F-COL-038 | What a job consists of | candidate | B | S | partly | `collab/offload/what-a-job-is.md` |
| F-COL-039 | A worker is just us, without the interface | candidate | B | S | partly | `collab/offload/a-worker-is-just-us-without-ui.md` |
| F-COL-044 | What outlives the service | researched | B | S | partly | `collab/depend/the-service-outlives-the-file.md` |
| F-EXP-004 | Choice of a hardware encoder on export | already-have | B | S | yes | `export/pipeline/hardware-encoder-selection.md` |
| F-HLP-001 | Our model — one hint line at the bottom right of the status bar | already-have | B | S | yes | `help/hints/our-one-line-status-hint.md` |
| F-HLP-002 | Info View in Ableton Live: a permanent place for a paragraph | researched | B | S | partly | `help/hints/ableton-info-view.md` |
| F-HLP-003 | Instrument strip against a reference strip — what others keep in the status bar | researched | B | S | partly | `help/hints/hover-help-in-nle-status-bars.md` |
| F-HLP-005 | Live modifier hint — what changes if you hold Shift/Ctrl/Alt | candidate | B | S | partly | `help/hints/modifier-hints-live.md` |
| F-HLP-009 | "Where to change it back" — a badge naming the place of return | candidate | B | S | no | `help/errors/where-to-change-it-back.md` |
| F-HLP-010 | Impossibility of an action: how an editor explains why a gesture failed | already-have | B | S | yes | `help/errors/blocked-action-feedback.md` |
| F-HLP-013 | Notification center and log: where what flashed by lives | candidate | B | S | partly | `help/errors/notification-center-and-log.md` |
| F-HLP-015 | Command palette — search through the program itself | candidate | B | S | partly | `help/discoverability/command-palette.md` |
| F-HLP-016 | Context menu as a reference: right click shows what can be done | candidate | B | S | partly | `help/discoverability/right-click-is-the-manual.md` |
| F-HLP-017 | Visibility of a hotkey — an accelerator next to the command | done | B | S | yes | `help/discoverability/accelerator-visibility.md` |
| F-HLP-018 | A label that explains itself — "what" and "where" right in the name | candidate | B | S | partly | `help/discoverability/labels-that-say-what-and-where.md` |
| F-HLP-024 | Keyboard shortcut cheat sheet inside the program | candidate | B | S | partly | `help/reference/keyboard-shortcut-reference.md` |
| F-HLP-025 | Description of an effect where it is chosen | candidate | B | S | partly | `help/reference/effect-catalogue-descriptions.md` |
| F-HLP-026 | "About" — the build version and its composition on the user's machine | candidate | B | S | partly | `help/reference/about-and-build-provenance.md` |
| F-HLP-027 | Start screen: recent projects, templates, entry to learning | candidate | B | S | no | `help/onboarding/welcome-screen-and-recent-projects.md` |
| F-HLP-031 | Preset as a form of documentation — "this is how it is set up" | researched | B | S | partly | `help/onboarding/presets-as-documentation.md` |
| F-HLP-032 | Documentation as code and automatic re-shooting of screenshots | already-have | B | S | yes | `help/authoring/docs-as-code-and-screenshot-pipeline.md` |
| F-HLP-033 | The cost of a help line in five languages | candidate | B | S | partly | `help/authoring/help-text-localization-cost.md` |
| F-HLP-034 | Help and completion in the formula field — without pop-ups | accepted | B | S | no | `help/hints/formula-input-help.md` |
| F-RES-002 | Atomic project write and fsync | done | B | S | yes | `resilience/durability/atomic-write-and-fsync.md` |
| F-RES-003 | A project created by another program version | candidate | B | S | partly | `resilience/durability/project-format-migration.md` |
| F-RES-005 | Unsaved work on exit — an asterisk and a question on close | done | B | S | yes | `resilience/recovery/unsaved-work-guards.md` |
| F-RES-006 | Dependence on system libraries and drivers | candidate | B | S | partly | `resilience/environment/system-libraries-and-drivers.md` |
| F-TL-002 | Slip and slide — trims that do not change the length | candidate | B | S | partly | `timeline/trimming/slip-and-slide.md` |
| F-UI-004 | The system file dialog against our own | candidate | B | S | partly | `ui/input/native-file-dialogs.md` |
| F-AUD-002 | Dynamic processing — compressor, limiter, gate, expander | done | B | M | yes | `audio/processing/compressor-and-limiter.md` |
| F-AUD-005 | Loudness normalization by standards — LUFS, EBU R128, true peak | done | B | M | yes | `audio/mixing/loudness-r128-and-lufs.md` |
| F-AUD-006 | Routing and mapping of audio channels | candidate | B | M | no | `audio/mixing/channel-routing-and-mapping.md` |
| F-AUD-008 | Automatic cutting by silence | candidate | B | M | no | `audio/processing/silence-detection-and-auto-cut.md` |
| F-AUD-010 | Modulations — chorus, flanger, phaser, tremolo, autopan, vibrato | done | B | M | yes | `audio/processing/modulation-family.md` |
| F-AUT-002 | The keyframe domain — source material or timeline time | already-have | B | M | yes | `automation/keyframes/keyframe-domain-source-vs-timeline.md` |
| F-AUT-004 | Effect presets that carry animation | done | B | M | yes | `automation/presets/presets-carrying-automation.md` |
| F-AUT-005 | Pitfalls of editing keyframes | candidate | B | M | partly | `automation/keyframes/inserting-and-editing-pitfalls.md` |
| F-AUT-006 | Preview gallery in the picker — a thumbnail of the effect on its own frame | done | B | M | yes | `automation/presets/preset-gallery-with-previews.md` |
| F-AUT-009 | The shape of a curve segment: not only linear and smoothed | candidate | B | M | partly | `automation/keyframes/segment-shapes.md` |
| F-AUT-010 | A formula on a segment and a formula layer over keys | accepted | A | M | partly | `automation/keyframes/formula-as-segment-and-layer.md` |
| F-AUT-011 | The segment shape is visible on the graph, its parameters — under the mouse | accepted | B | S | partly | `automation/keyframes/shape-parameters-by-mouse.md` |
| F-CLR-004 | LUT 1D and 3D (.cube) — technical against creative | candidate | B | M | no | `color/management/lut-1d-and-3d.md` |
| F-CLR-005 | Instruments — waveform, parade, vectorscope, histogram, false color | candidate | B | M | no | `color/scopes/waveform-parade-vectorscope.md` |
| F-COL-001 | A shared disk instead of the cloud — the network as a file system | researched | B | M | partly | `collab/assets/shared-storage-and-the-protocol.md` |
| F-COL-003 | "Not arrived yet" is not "missing" | researched | B | M | no | `collab/assets/not-here-yet-is-not-missing.md` |
| F-COL-011 | The source timecode as a shared address | candidate | B | M | partly | `collab/handoff/timecode-is-the-shared-address.md` |
| F-COL-012 | The owner of a track | candidate | B | M | partly | `collab/handoff/who-owns-which-lane.md` |
| F-COL-015 | "Read-only" as a standard mode | candidate | B | M | no | `collab/claim/read-only-is-a-mode.md` |
| F-COL-020 | A comment tied to a frame | researched | B | M | partly | `collab/review/comment-anchored-to-a-frame.md` |
| F-COL-022 | The author and time of a comment | researched | B | M | no | `collab/review/who-said-it-and-when.md` |
| F-COL-023 | Markers ON THE CLIP, not on the edit | researched | B | M | partly | `collab/review/markers-on-the-clip.md` |
| F-COL-024 | A version of a cut as the unit of review | candidate | B | M | no | `collab/review/version-stack-of-a-cut.md` |
| F-COL-025 | How a comment is addressed in someone else's player | candidate | B | M | no | `collab/review/burn-in-makes-it-addressable.md` |
| F-COL-026 | Returning comments into the edit | candidate | B | M | partly | `collab/review/bringing-comments-back.md` |
| F-COL-027 | Approval status — a state, not text | candidate | B | M | no | `collab/review/approval-as-a-state-machine.md` |
| F-COL-031 | Comparison of two edits | candidate | B | M | partly | `collab/merge/timeline-diff.md` |
| F-COL-034 | A document revision number that the file does not have | researched | B | M | partly | `collab/merge/document-revision-in-the-file.md` |
| F-COL-042 | What must be handed off is not the render but the analysis | candidate | B | M | partly | `collab/offload/offload-the-analysis-not-the-render.md` |
| F-EXP-001 | Library of export presets | candidate | B | M | no | `export/presets/preset-library.md` |
| F-EXP-002 | Render queue and background export | candidate | B | M | partly | `export/pipeline/render-queue-and-batch.md` |
| F-FX-003 | Radial/Zoom Blur and Drop Shadow | done | B | M | yes | `fx/blur/radial-blur-and-drop-shadow.md` |
| F-HLP-004 | Anatomy of a pop-up hint: delay, content, rich tooltips | researched | B | M | no | `help/hints/tooltip-anatomy-and-delay.md` |
| F-HLP-006 | Explanation of a parameter where it is turned | candidate | B | M | no | `help/hints/parameter-help-in-effect-ui.md` |
| F-HLP-007 | Feedback during a gesture — trim delta, offset, new timecode | candidate | B | M | partly | `help/hints/hints-during-a-gesture.md` |
| F-HLP-008 | Anatomy of an error message: cause, consequence, action | candidate | B | M | partly | `help/errors/error-message-anatomy.md` |
| F-HLP-011 | Project check before the render starts | candidate | B | M | partly | `help/errors/preflight-before-render.md` |
| F-HLP-012 | A missing source: how the editor speaks about it | researched | B | M | partly | `help/errors/offline-media-messaging.md` |
| F-HLP-014 | Recovery after a crash and the bug report | candidate | B | M | partly | `help/errors/crash-and-bug-report-flow.md` |
| F-HLP-019 | Search for settings and effects: find by what it does | candidate | B | M | partly | `help/discoverability/searchable-settings-and-effects.md` |
| F-HLP-020 | Empty states that teach, and grey widgets that stay silent | candidate | B | M | partly | `help/discoverability/empty-states-and-disabled-controls.md` |
| F-HLP-021 | "What's new" on update | candidate | B | M | no | `help/discoverability/whats-new-on-update.md` |
| F-HLP-029 | Demo project and material for it | candidate | B | M | partly | `help/onboarding/sample-project-and-media.md` |
| F-HLP-030 | First launch on someone else's hardware: what the program says about itself | candidate | B | M | partly | `help/onboarding/first-run-hardware-and-defaults.md` |
| F-MED-006 | Several audio streams in one file | candidate | B | M | no | `media/sources/multiple-audio-streams.md` |
| F-MPH-006 | The blend strength slider on a morpher layer | candidate | B | M | no | `morpher/gaps/opacity-and-wet-dry.md` |
| F-MSK-004 | Chroma key: showing the matte, despill and edge processing | candidate | B | M | partly | `masks/keying/chroma-key-spill-suppression.md` |
| F-PRJ-002 | Edit history and project versions | candidate | B | M | partly | `project/history/versions-and-snapshots.md` |
| F-PRJ-003 | A project opened on another machine | candidate | B | M | partly | `project/structure/project-portability.md` |
| F-RES-001 | Autosave and project versions — over the file or beside it | done | B | M | yes | `resilience/durability/autosave-and-project-versions.md` |
| F-RES-004 | What survives a crash: the recovery mechanism | done | B | M | yes | `resilience/recovery/crash-recovery-flow.md` |
| F-RES-010 | Responsiveness under load: the program is alive but does not respond | candidate | B | M | partly | `resilience/performance/responsiveness-under-load.md` |
| F-RES-011 | Memory and large projects: who evicts whom and by which rule | candidate | B | M | partly | `resilience/performance/memory-and-large-projects.md` |
| F-RES-012 | Cancelling a long operation and progress | done | B | M | yes | `resilience/performance/cancelling-long-operations.md` |
| F-SPD-001 | Event speed — constant and variable | candidate | B | M | partly | `time/retiming/video-speed-ramping.md` |
| F-SPD-004 | Synchronizing a camera and a recorder by waveform and timecode | done | B | M | yes | `time/sync/audio-waveform-sync.md` |
| F-TL-003 | JKL navigation and dynamic trim | candidate | B | M | partly | `timeline/trimming/dynamic-trim-jkl.md` |
| F-TL-007 | Guides and the spacer tool | candidate | B | M | partly | `timeline/navigation/guides-and-spacer.md` |
| F-TL-008 | Closing gaps on a track — one, the whole track, the whole project | candidate | B | M | no | `timeline/assembly/close-gaps-and-blank-space.md` |
| F-TL-009 | Zoom bar and timeline ruler | candidate | B | M | partly | `timeline/navigation/zoom-bar-and-ruler.md` |
| F-TL-010 | A transition between clips on one track | already-have | B | M | yes | `timeline/structure/same-track-transitions.md` |
| F-TXT-003 | Generators — color, gradient, noise, shapes, timecode, test charts | done | B | M | yes | `titles/generators/solid-gradient-shapes.md` |
| F-TXT-005 | Outline, shadow, plate and the drawing order of text | done | B | M | yes | `titles/text/stroke-shadow-and-draw-order.md` |
| F-TXT-006 | Text animation: typewriter, by letter, scrolling ticker | done | B | M | yes | `titles/text/text-animation-presets.md` |
| F-UI-001 | On-screen controls in the viewer: transform frame, handles, guides | candidate | B | M | no | `ui/viewer/on-screen-controls.md` |
| F-UI-003 | Window layouts — presets by task and where the layout lives | candidate | B | M | partly | `ui/input/workspaces-and-layouts.md` |
| F-UI-005 | Clip color as data and as decoration | candidate | B | M | partly | `ui/viewer/clip-colors-and-legibility.md` |
| F-AUD-003 | Noise reduction and speech restoration | candidate | B | L | no | `audio/processing/noise-reduction.md` |
| F-AUD-013 | Pitch shift — pitch without changing the duration | done | B | L | yes | `audio/processing/pitch-shift.md` |
| F-AUT-008 | Linked parameters: one control moves a bundle | candidate | B | L | no | `automation/keyframes/parameter-linking.md` |
| F-CMP-002 | Transition vocabulary: inversion, patterns, volume | done | B | L | yes | `compositing/transitions/transition-vocabulary.md` |
| F-CMP-003 | Blend mode — on the track or on the clip | in-progress | B | M | partly | `compositing/blending/blend-mode-track-vs-clip.md` |
| F-COL-010 | Handles: how much extra travels with the edit | candidate | B | L | no | `collab/handoff/handles.md` |
| F-COL-016 | The unit of claiming — a library, a bin or a sequence | researched | B | L | no | `collab/claim/lock-granularity.md` |
| F-COL-033 | How an edit is addressed: id, tick or lineage | researched | B | L | partly | `collab/merge/identity-of-an-edit.md` |
| F-COL-037 | Pulling someone else's edit into your project | researched | B | L | no | `collab/merge/import-a-foreign-cut.md` |
| F-HLP-023 | User manual: how it is structured and how one finds things in it | candidate | B | L | no | `help/reference/manual-structure-and-search.md` |
| F-MED-001 | Smart folders, metadata and material markup | candidate | B | L | partly | `media/organization/smart-bins-and-search.md` |
| F-MED-002 | Proxies — editing on light copies, rendering from originals | candidate | B | L | no | `media/proxies/proxy-toggle-workflow.md` |
| F-MED-005 | Relinking sources and consolidating a project into one folder | candidate | B | L | no | `media/organization/relink-and-consolidate.md` |
| F-MPH-001 | Adjustment Events in VEGAS Pro | candidate | B | L | partly | `morpher/analogues/vegas-adjustment-events.md` |
| F-MPH-002 | Adjustment Layer in After Effects | candidate | B | L | partly | `morpher/analogues/ae-adjustment-layer.md` |
| F-MPH-003 | Adjustment Clip: an effect over the composite | candidate | B | L | partly | `morpher/analogues/resolve-adjustment-clip.md` |
| F-MSK-002 | Where a mask lives — on a layer, on an effect, or as a separate object | candidate | B | L | no | `masks/shapes/mask-scope-per-effect.md` |
| F-MSK-003 | Track matte — one clip sets the transparency of another | candidate | B | L | no | `masks/keying/track-matte.md` |
| F-PRJ-004 | A user storage shared between projects | candidate | B | L | partly | `project/structure/user-library-outside-the-project.md` |
| F-PRJ-005 | PROJECT format against EXCHANGE format | accepted | A | L | no | `project/structure/project-file-vs-interchange.md` |
| F-SPD-002 | Retime curve — time as an animatable parameter | candidate | B | L | partly | `time/retiming/time-remap-curve.md` |
| F-TL-006 | Track switches — mute, solo, lock and "hide" | in-progress | B | L | partly | `timeline/structure/track-solo-mute-lock.md` |
| F-TXT-004 | Subtitle track, SRT/VTT and text editing | candidate | B | L | no | `titles/subtitles/subtitle-track-and-srt.md` |
| F-UI-002 | Customizable keys and other editors' layouts | candidate | B | L | no | `ui/input/keymap-presets-and-editor.md` |
| F-AUD-004 | Buses, track FX, sends and solo/mute | in-progress | B | XL | partly | `audio/mixing/buses-and-track-fx.md` |
| F-AUT-001 | Bezier handles on keyframes | candidate | B | XL | partly | `automation/keyframes/bezier-handles.md` |
| F-AUT-003 | An envelope on a track | researched | B | XL | no | `automation/keyframes/envelope-lane-on-track.md` |
| F-AUT-007 | Expressions: a property driven by a formula | accepted | B | XL | no | `automation/keyframes/expressions-and-drivers.md` |
| F-CLR-003 | Secondary correction: HSL qualifier, windows and tracking | candidate | B | XL | no | `color/primary/hsl-qualifier-secondaries.md` |
| F-COL-018 | What Ctrl+Z undoes once the edit has already been handed out | researched | B | XL | partly | `collab/claim/undo-under-shared-ownership.md` |
| F-COL-036 | Operation against snapshot | researched | B | XL | partly | `collab/merge/operation-log-vs-snapshot.md` |
| F-EXT-001 | Hosting third-party plugins — OFX for video and VST for sound | candidate | B | XL | no | `plugins/hosts/ofx-host.md` |
| F-EXT-003 | The plugin contract: what foreign code must declare | accepted | B | XL | no | `plugins/hosts/plugin-abi-and-sandbox.md` |
| F-FX-005 | Physics: motion that is not in the keys | researched | B | XL | no | `fx/simulate/physics-and-dynamics.md` |
| F-MPH-004 | Adjustment Layer in Adobe Premiere | researched | B | XL | partly | `morpher/analogues/premiere-adjustment-layer.md` |
| F-MPH-005 | Levels of applying video FX in VEGAS and the Video Bus Track | in-progress | B | XL | partly | `morpher/analogues/vegas-video-bus-track.md` |
| F-MPH-007 | What a layer sees at its input — the composite below or each clip | researched | B | XL | no | `morpher/gaps/composite-below-vs-per-clip.md` |
| F-MPH-008 | Automation (curves) of the morpher layer itself | candidate | B | XL | no | `morpher/gaps/strength-automation.md` |
| F-MSK-001 | Shape masks: bezier contours and primitives | candidate | B | XL | no | `masks/shapes/bezier-shape-masks.md` |
| F-MSK-005 | Tracking: point, planar and "AI object tracking" | researched | B | XL | no | `masks/tracking/planar-tracking.md` |
| F-MSK-006 | Stabilization: Warp Stabilizer and its kin | candidate | B | XL | no | `masks/stabilization/warp-stabilizer.md` |
| F-PRJ-001 | Several sequences (timelines) in one project | candidate | B | XL | no | `project/structure/multiple-sequences.md` |
| F-SPD-003 | Frame interpolation when slowing down: three quality steps | candidate | B | XL | partly | `time/retiming/optical-flow-interpolation.md` |
| F-SPD-005 | Conforming the source frame rate to the project grid | researched | B | L | partly | `time/retiming/frame-rate-conform.md` |
| F-COL-009 | Handed off for color — taken back | candidate | C | S | no | `collab/handoff/out-and-back.md` |
| F-COL-028 | Chat inside the editor | rejected | C | S | no | `collab/review/chat-next-to-the-document.md` |
| F-COL-035 | Conflict as a separate file | researched | C | S | no | `collab/merge/conflict-as-a-file.md` |
| F-COL-043 | The price of joint work and silence about non-payment | researched | C | S | no | `collab/depend/what-happens-when-you-stop-paying.md` |
| F-COL-045 | An account as a condition of launch | researched | C | S | no | `collab/depend/identity-and-who-you-are.md` |
| F-COL-004 | Material that is synchronized during shooting | researched | C | M | partly | `collab/assets/sync-while-shooting.md` |
| F-COL-007 | What exactly the edit is handed over as: AAF, OMF, EDL, XML | researched | C | M | partly | `collab/handoff/interchange-formats.md` |
| F-COL-019 | Who is in the project now | researched | C | M | partly | `collab/claim/presence-and-whose-cursor.md` |
| F-COL-046 | Anonymous usage telemetry — launches, time in the program, clean exit | done | C | M | yes | `collab/depend/anonymous-usage-telemetry.md` |
| F-EXP-003 | Smart render — copying untouched pieces without re-encoding | candidate | C | M | no | `export/pipeline/smart-render-passthrough.md` |
| F-EXT-002 | Scripting and automation — an API for your own operations on the project | candidate | C | M | partly | `plugins/scripting/scripting-api.md` |
| F-HLP-022 | Context help on F1: a page about what you are looking at | researched | C | M | no | `help/reference/f1-context-help.md` |
| F-COL-005 | Proxies made not for oneself | researched | C | L | no | `collab/assets/proxy-as-the-unit-of-sharing.md` |
| F-COL-040 | Gluing pieces after a parallel render | rejected | C | L | no | `collab/offload/stitching-segments.md` |
| F-COL-041 | Nobody has native cloud render | rejected | C | L | no | `collab/offload/no-native-cloud-render.md` |
| F-HLP-028 | Tours, coach marks and interactive lessons | rejected | C | L | no | `help/onboarding/guided-tour-and-coachmarks.md` |
| F-CLR-006 | Node graph against a linear chain of effects | rejected | C | XL | no | `color/workflow/node-vs-layer-grading.md` |
| F-CLR-008 | Color management — space tags, working space, output | researched | B | XL | no | `color/management/color-management-and-tags.md` |
| F-COL-029 | Why nobody has merging | rejected | C | XL | no | `collab/merge/nobody-merges.md` |
| F-FX-006 | Particles and emitters | researched | C | XL | no | `fx/simulate/particles-and-emitters.md` |
| F-TL-005 | Multicam editing | researched | C | XL | no | `timeline/structure/multicam.md` |
| F-TXT-002 | Title templates with public parameters | researched | C | XL | no | `titles/text/title-styles-and-templates.md` |
<!-- table:end -->

## What the first wave showed (54 notes, 2026-09-07)

Three conclusions that were not visible in any single note — they appeared only when there were half a hundred notes. This is the main benefit of the base.

1. **Five different features run into ONE missing subsystem — "offline analysis of material"**: tracking (F-MSK-005), stabilization (F-MSK-006), synchronization by waveform (F-SPD-004), noise profile (F-AUD-003), integrated loudness (F-AUD-005). All of them need a background pass over the material with the result saved — and we ALREADY have a place for it: the SQLite sidecar `<project>.ktindex.db` (`src/libs/kt-media/src/index.rs`), where the packet index, the peak pyramid and the thumbnails are likewise computed in the background. It is cheaper to build the subsystem once than five times in place. The cheapest pilot of the five is the noise profile.

2. **Four features run into the absence of a "user storage outside the project"**: a library of FX presets (F-AUT-004), title templates (F-TXT-002), export presets (F-EXP-001), key layouts (F-UI-002). A project for us is one `.otio` file, while everything listed lives, in meaning, BETWEEN projects. Until such a place exists, each of the four features will invent it anew.

3. **Nesting (F-TL-004) is the bottleneck of three other features**: title templates, multicam (F-TL-005) and "an effect on a composited picture" (F-MPH-007) cannot be expressed without it. This is the most structural position of the base: the later, the more expensive.

Separately, about the reason for all this work: the divergence of our morpher from the Adjustment Event turned out NOT to be in the transparency slider, but one floor lower — in WHAT the layer processes (F-MPH-007). The slider (F-MPH-006) is a consequence. The reverse side of the same choice: substituting `media`, shifting `source_in`, the `Areas`/`Marks` modes and an honest bake of a layer working on the composited frame cannot be done in principle. We are not behind — we are at a different point, and this must be decided consciously, not caught up with.

The first wave was written with the web-search budget exhausted and with `helpx.adobe.com`, `reddit.com` and `discussions.apple.com` closed; where a quote could not be obtained, this is said in the note itself with the `NO QUOTES:` mark. The working workarounds found by the reconnaissance and useful to the next wave are recorded in `_editors.md`.

## What the second wave showed (33 notes help/, 2026-09-07)

1. **We have solved pull and have not solved push — these are two different tasks, not one half-done.** Our status line answers the question "what is this?" perfectly and does not answer the question "what can be done here at all?" in any way: **you cannot hover the mouse over what you have not found**. All seven complaints of R.38 of the kind "I do not understand how" are failures of the push channel, and none of them is cured by a hint on hover. From this follows the conclusion for which the wave was undertaken: one must work not on the capacity of the line, but on a **second surface** that covers and delays nothing. The cheapest one already exists and is half empty — the right click (F-HLP-016 found seven mute objects), followed by the label (F-HLP-018) and the teaching empty states (F-HLP-020: of 16 places where "there is nothing here", the verb names 5, the key exactly one).

2. **The text about the program lives in several places and has already diverged — this is a system, not an accident.** The chord is recorded in FOUR sources (the handler, the `menu_btn` literal, the translated catalog phrase, the landing page cheat sheet) and has diverged in Spanish (F-HLP-017). The default of an effect is in THREE places (serde, `ParamSpec.default`, the picker template), two discrepancies out of twelve (F-HLP-031). The effect descriptions advertise `CamMotion`, which was migrated into Pan back in R.34 (F-HLP-025, F-HLP-033). The cheat sheet promises a reset by double click, which does not exist in the handler (F-HLP-024). None of the discrepancies is caught by anything — because all the existing checks look INWARD into the catalog, while the sources diverge BETWEEN one another.

3. **The domain is abnormally cheap: 18 notes out of 33 have cost `S`, not one has priority `A`.** This is not "build the help", but "bring what is already written to where people look". The translation of the effect descriptions has been paid for and lies on the landing page; the documentation pipeline is built and stopped (`site/build.py --check` is red, dist is two commits behind); the demo project exists and is unreachable; the cheat sheet is written and there is no link to it from the program; the functions `backend_info`, `simd_name`, `EncoderChoice` are declared "for the perf panel and diagnostics" and are called from nowhere. The only `rejected` in the whole wave is the teaching layer over the interface (tours and coach marks), and it was rejected not for cost, but by invariant 12 and rule 10.

Separately: the wave applied for the first time the **form of a verdict with an address** (rule 11) — the conclusion "we need help" physically cannot be written, because the script requires naming a concrete non-existent artifact that nobody named before you. The protection was verified on probes: a conclusion without the form, a form without an address, and a repeated address each break `--check`.

## What the third wave showed (42 notes, 12 bug trackers, 2026-09-08)

For the first time the wave read not the vendors' documentation, but a **corpus of other people's pain**: 28,537 tasks from twelve trackers, of which 25,442 are in the common ranking (ffmpeg-trac is outside it — its denominator is incomparable). The registry, the lexicon of topics and the pins — `_trackers.md`.

1. **Our invariants cover the top of the ranking, but our MESSAGES do not. The category `resilience/` was born exactly at this junction.** "Render/export" is the third topic of the corpus (3,921 tasks) and at the same time the topic with the WORST share of real fixes (0.75): its large classes — "in the preview there was one thing, and in the export another" and "the encoder did not open" — are closed by construction by invariants 7 and 9 and verified by t11/t67/t184 (F-RES-007 is the only note of the wave with status `already-have` and priority `A`). But three of the four `A`-priority notes fell into ONE subcategory `environment/` (F-RES-007/008/009), and all three are about the moment when the program meets a foreign machine and must SAY so. Here we lose: a missing FFmpeg arrives as "N missing sources", `GpuRequired` is shown as a raw English string, `panic::set_hook` is installed nowhere while `strip = "symbols"` is set in the dist profile — that is, a crash gives neither a message nor a usable backtrace (F-HLP-014 found 54 reports in other trackers that were formatted by a crash handler, and four tasks of the kind "a backtrace cannot be obtained in principle" where there is none). The boundary `help/` ↔ `resilience/` — "will SAY" against "will DO" — turned out to be not bureaucracy, but a description of our own skew.

2. **The main haul of the wave was not features but our own defects: 47 lines `ADDRESS:` across 23 different files in `src/`.** A foreign tracker works as a list of places worth looking at in your own code, and three grep checks converged at once: autosave targets the user's file, not a separate version (`apps/kt-app/src/app.rs:1078`, `unwrap_or_else(autosave_path)` — that is, background saving OVERWRITES what the person has not yet decided to save, whereas Premiere writes versions into a separate folder); `write_project_atomic` (`app.rs:5051`) does tmp+rename **without `sync_all`** — on a power loss the rename may outlive the content; `set_hook` does not occur in either kt-app or kt-cli. None of the three is caught by a test. Here too — the silent loss of `link_group` and `gain` during conversion (reproduced: `kt-cli roundtrip` returned 0, having lost the fields) and `kt-cli demo`, which writes an edit with eight non-existent sources and therefore does not render itself. The wave does not fix them — these are separate tasks — but nobody had seen them before.

3. **"Closed" ≠ "fixed", and the numbers move together with the corpus — this is a methodological conclusion paid for by an error.** For Kdenlive, `WORKSFORME` is 1,549 out of 6,346 (24.4 %), therefore `fixed_ratio` is computed, not "closed". The first ranking over THREE trackers produced the sensation "hardware/driver is fixed worst of all, 0.46"; on the full eleven it is 0.76, and the whole spread of topics shrank from 0.26–0.61 to 0.75–0.89 — that is, the sensation was an artifact of a small corpus. Exactly those notes survived that STATED the scope of the count ("0.46 across three trackers"), and this is a direct argument for the form of rule 12: `SIGNAL:` must name the filter and the trackers, otherwise the number cannot be rechecked. The check worked in practice: after the last three dumps were downloaded, six notes had to be recalculated, and both claims of ABSENCE held on the full corpus — neither "notification history" (zero tasks out of 25,442) nor a growth of the class "the caption is incomprehensible" (8 tasks, 0.03 %) was brought by the new dumps. What would not have held would have been visible.

Separately: the wave applied the **third generation of protection of a conclusion** (rule 12) — `SIGNAL:` with two numbers, the `%` sign and a unique filter in backticks, plus `ADDRESS:` with a path `src/…`. The uniqueness of the filter means that two notes cannot rely on the same count. The protection was verified by probes: `trackers:` without `SIGNAL:`, a number without `%`, a repeated filter, and `resilience/` without `trackers:` each break `--check`. The numbers themselves are deliberately not checked by `--check`: there are no dumps in the repository and there never will be (≈150 MB, not a single distribution license); the form and uniqueness are checked offline, and the number is reproduced by `tracker-rank.py --query`.

## What the fourth wave showed (45 notes `collab/`, 2026-09-08)

The wave read the cloud capabilities of commercial editors, and measured them by a second corpus of trackers: pipelines and farms (Syncthing, OpenCue, Kitsu, Ayon, Flamenco) — where the five core editors have no public tracker at all.

1. **The pain of joint work exists in the world — but not in video editors.** Eight topical lines of the lexicon cover **0.33 % of the corpus of editing programs** (85 tasks out of 25,442) and **6.52 % of the corpus of pipeline tools** (160 out of 2,453): a twentyfold difference. Across the whole corpus of twelve open-source editors there is ONE real request for joint editing (`kde-invent #1243`, open since 06.11.2021), and that one asks not for merging but for a prohibition of overwriting: "You can't overwrite the opened project". This explains the second half of the picture: **NOBODY merges**, the whole industry has bought prevention instead of resolution, and only the granularity of the lock differs — it is always equal to the unit of a RECORD, not the unit of an edit (Final Cut writes the library as a whole and occupies it as a whole; Resolve writes bins and timelines as separate database records and occupies them apart). Nobody locks a clip.

2. **Zero notes of priority `A` out of all forty-five.** The domain is entirely B (33) and C (12), 24 of 45 with status `partly`, 17 with cost `S`. For comparison: the third wave gave four `A`-priority notes out of forty-two. The conclusion for which the wave was undertaken: the cloud is not something without which a product is not a video editor. But **the main haul again was our own defects**, and this time almost all of them do not require a second participant at all: `Retained` is keyed by `ClipId`, is not cleaned up on deletion, and is substituted by number on the record, so a new clip with a reused id silently carries the name, markers and effects of the dead one into the file; `Split` gives the right half `ClipExtra::default()`, losing markers, name, foreign effects and the timecode of the material; `kt-cli` writes the same `.otio` with a bare `fs::write` bypassing the guard, and three calls of `write_project_guarded` discard its answer "did not write" — Ctrl+S can report "Saved" about a write that did not happen; two copies of the program on one machine are indistinguishable, and the second silently overwrites the recovery file of the first. **The second participant is you yourself in a second window, yesterday's copy, and a scheduled script.**

3. **Pipeline tools fix what they are told about, and editing tools do not.** The share of real fixes in the `collab` corpus is 0.94–1.00 across almost all topics (for "farm and jobs" 0.96); in the `nle` corpus it is 0.74–0.89. A caveat of honesty: the corpora differ in size and age, and the explanation may be as much the culture of maintenance as the fact that for a tool for engineers a task is filed by someone able to reproduce it. The "duplicates" column in the `collab` corpus cannot be read at all — GitHub does not return `dupe_of`.

Separately: the wave applied the **fourth generation of protection of a conclusion** (rule 13). For the first time the check looks not at the form of the address but at the TREE: the anchor of the `NO NETWORK:` line must point to a file that exists today — that is, a path to a crate that has not been written will not build, and this is the mechanical execution of rule 8. Result: 45 notes, 45 unique anchors, the markers 19 `DEGENERATES` / 14 `DOES NOT REMAIN` / 12 `REMAINS` — the axis did not degenerate either into an invented local core or into forty-five refusals. The four refusals of the wave are constructive, and not one of them is motivated by the absence of a network.

The wave began with a check of its own basis, and the basis turned out to be defective: in the cache of the third wave `blender-vse` lay doubled in its entirety, and `ffmpeg-trac` was three thousand empty records. The real corpus is 25,442, not 27,537. No counting claim of the notes was affected (the shares are computed within their own tracker), but the totals were recalculated, and the previous ones are kept alongside — by the same rule by which the third wave kept its "Caveat on the mobility of the numbers".

## Where to look

The order of trust and the specific sites — in `docs/features/_editors.md`, section "How to verify". The occasion for a note is often a foreign phrase "switched to the new version because of X" — it is precisely this that means the feature costs money, not a line in a list of capabilities.
