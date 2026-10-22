# Design

## Context

Motivation — `proposal.md`. Source — `docs/ktprompts/v26.10.txt` (verbatim). Here —
a cross-check of each item against the code and specs (12.10.2026), the registry rules, questions for
the owner and the order of the next rounds. The cross-check is against the sources (`grep` over kt-app/kt-model/kt-fx/kt-engine,
the feature-note registry `docs/features`, the specs), not from memory: where the code cross-check gives no
answer, there is a "check live" — a task for the round that takes the item.

What already exists and what the registry relies on: `features-index.py --check` (the note registry),
`live-coverage.py` with the `MIGRATED` registry (the test address must exist in the sources),
`specs-user-path.py --check` (the PATH line of UI scenarios). The guard of the claims registry is
the fourth of the same kind.

## Goals / Non-Goals

**Goals:** every item of the file has a state and an address; "done" is proven by a test; questions are
asked once and recorded; the order of the next rounds is named and confirmed by the owner.

**Non-Goals:** implementing the items (each — its own change); reconnaissance of new features (a note is opened
by the round that takes the feature); editing the application code.

## Decisions

### 1. Registry states — a closed set, and each has its own mandatory address

`done R.NN, tNNN` · `partial` (what exists / what is missing) · `no: defect → capability` ·
`no: feature → F-… or "no note"` · `question: text` · `future`. The guard checks the form and the existence of the test.
The alternative "free text" was rejected: that is exactly the checklist that does not work.

### 2. The registry is a Markdown table, not YAML

It is read by a person (the owner) — a table in `docs/` next to the source; the guard's parser is simple:
a row `| G1 | … |`. The item numbering is `<section><number>`, because in the file the numbers start over in each section and even repeat (`4.` twice in the timeline block).

### 3. Cross-check of the items (state as of 12.10.2026)

Abbreviations: LC — check live in the round that takes the item.

**GENERATORS (G)**

| # | Claim (brief) | State | Address / stage |
|---|---|---|---|
| G1 | create a generator via RMB on a track, without drop-down lists of kinds; for transitions RMB is the same; change the type in the window | no: reachability defect | today the doors are `File` and the bin's RMB (`gen_ui::generator_menu`), both list the kinds; the transition window already changes the kind (`transition_editor`), the RMB menu — LC. `ui-reach` "an action has one door". R.72 |
| G2 | generators have no automation | **done R.59** (`gen_auto`, `draw_gen_auto`; t6xx) | the claim is outdated; LC — does a person find it. R.72 |
| G3 | few parameters; the timecode is text, but it is not controlled as text | no: feature | the timecode is a separate kind with a small passport; `Text` has a full passport. Candidate decision: the timecode as a SOURCE of the text of a `Text` generator (question 5). `generators`. R.72 |
| G4 | the generator and its effects in one window, with a "breadcrumb" | **done R.59** (gen section inside the FX window, `fx_editor.rs:736`) | outdated; LC — is it the first node. R.72 |
| G5 | very few generators | partial | 8 kinds (R.52). Which are needed — question 6; linked to plugins P3. R.72 |
| G6 | no preview when choosing a generator; the same for transitions | no: defect | thumbnails exist only for FX (`fx_thumbs`). The same service for the pickers of generators and transitions. `ui-reach`. R.72 |
| G7 | the 1 kHz tone of SMPTE does not sound; the generator does not put an audio event | no: defect | `tone/tone_db` exists in the model (`genmedia.rs:216,1333`), but `create_generator` puts only a video event — there is nothing to sound. Decision: a generator with sound gives an A/V link pair. `generators`. R.72 |

**USABILITY (U)**

| # | Claim | State | Address / stage |
|---|---|---|---|
| U1 | render to an existing file — ask | no: defect | `render_dialog.rs` does not check the output. `export`. R.74 |
| U2 | Ctrl+space — reverse while held | no: feature (no note; minor) | there is acceleration by holding (`space_boost`); the sign — from the modifier. `transport`. R.73 |
| U3 | smooth speed build-up after pause, reset faster than build-up | no | the `space_boost` acceleration curve — LC. `transport`. R.73 |
| U4 | pressed space from pause and held → released → stop | no | now it always continues. The threshold "short press = toggle" — question 7. `transport`. R.73 |
| U5.1 | the lower corner to the label — only length drag | no: defect | the handle zones are fixed in px (`FADE_HANDLE_PX = 12`), they do not depend on zoom. `ui-timeline`. R.73 |
| U5.2 | side edges at a small zoom — only fade and stretch | no: defect | one decision for U5.1–5.3: zones from the size of the event ON SCREEN (px thresholds for height and width). R.73 |
| U5.3 | at a small zoom do not offer gain/opacity | no: defect | the same. R.73 |
| U6 | gain before mute; a gain/opacity badge on the event | partial | the track has −60 dB (`TRACK_GAIN_MIN_DB`); for the event the rail `RAIL_DB_SPAN` — the limit is LC; badges exist on the TRACK (`row_badges`), not on the event. `ui-timeline`. R.73 |
| U7 | move the second region marker; "marker over the region" — a separate item | question 9 | the region edges are dragged (`region_edge`), I/O set them; no item with that name was found — clarify what is meant. R.73 |
| U8 | notes: multi-line text with remarks | no: feature, no note | the related F-COL-021 "two notes on one frame". The "notes" round (next) |
| U9 | cut (scissors) of a region, from the keyboard ("C" is free — yes, only Ctrl+C is taken) | no: feature, no note | the related F-TL-001 ripple, close gaps. The "editing commands" round (next) |
| U10 | toolbar in a narrow window — scroll with the wheel | no: layout defect | there is a burger menu at a narrow column. `ui-timeline`. R.75 |
| U11 | drag the preview by its body; the title bar as in the main window; the main window — and behind the preview | partial | the detached preview window exists (item 7 of v1.4, `show_viewport_immediate`); title-bar style and dragging by the body — no. `ui-timeline`. R.75 |

**TIMELINE / WINDOW / RENDER (T)** — the second list of the file (numbering 1–18, "4" twice)

| # | Claim | State | Address / stage |
|---|---|---|---|
| T1 | vertical zoom flies down (the anchor is not under the mouse) | no: defect | `ui-timeline`. R.73 |
| T2 | the scrollbar cannot be found: window resize or the jump zone intercepts | no: defect, **repeat** of the theme "resize steals clicks" (R.12/R.13, t101) | `ui-timeline`. R.73 |
| T3 | track buttons broke the jump zone on the left | no: defect (regression of R.62 B) | `ui-timeline`. R.73 |
| T4a | an event in the jump zone opens on click | no: defect | `ui-timeline`. R.73 |
| T4b | "Add morpher here" / camera — obvious labels; forbid for agents | no: defect, **repeat** of the checklist of item 17 of R.7 | `tl.hint.add_morpher_here`, `morph_lane`, `camera_lane` in i18n. Remove + a guard against "obvious labels" (the question of form — in R.73). `ui-timeline`. R.73 |
| T5 | a maximized window is dragged at the same size; after a repeated maximize it shrinks a bit | no: defect | `pending_unmax_drag` exists, the first case does not trigger. `ui-timeline`. R.75 |
| T6 | the order of tracks — by mouse on the side | no: feature (minor) | `SetLaneOrder` exists, only from the menu. `ui-timeline`. R.73 |
| T7 | copy buttons for presets/curves/FX — shorter, icons | no: UI | `ui-timeline`. R.75 |
| T8 | the side strip on the left of expanded audio/video in the FX window — remove | no: UI | `ui-timeline`. R.75 |
| T9 | the FX window at the top — the main window's title bar pokes through | no: layout defect | R.75 |
| T10 | X/Y of the center on different rows; the center — by clicking the preview (Radial Blur and all similar) | partial | R.66 — pairs on one row in six windows (t780), R.64 — the frame only for geometry. Needed: an X/Y pair everywhere by the passport + "point on preview" for coordinate parameters. `ui-timeline`/`video-fx`. R.75 |
| T11 | Shift+back/forward — one frame | question 8 | Shift on these buttons is taken by the region edge (`hint.back/forward`). R.73 |
| T12 | the render window counts statistics after finish | partial | ETA is suppressed at `done` (`render_dialog.rs:555`); speed — LC. R.74 |
| T13 | video 517/516, audio 412672/413364 | no: count defect | looks like a tail frame +1 and an incomplete last burst. `export`. R.74 |
| T14 | loudness without emphasis and norms; design | no | there is LUFS and a delta to −23 (`render_dialog.rs:628`); there are no norms by parameter, highlighting, by channels. `export`. R.74 |
| T15 | an evaluation of video, like loudness for sound | no: feature, no note | overexposed/black/frozen frames, letterbox — reconnaissance in R.74 |
| T16 | h264_vaapi — crash | no: defect, already in the queue ("encoders against what opens", `export`/`media-index` Deferred) | R.74 |
| T17 | on a crash nothing is shown | partial | there is a crash report on the next start (R.49 item 355, panic hook), but a crash in libav — abort/SIGSEGV, the hook does not catch it. Candidate decision: export in a separate process or a sigsegv handler with a journal record. `durability`. R.74 |
| T18 | h265/vp9/formats, not only MP4 | no: feature → F-EXP-001 (candidate) | R.74 |

**WINDOWS AND GLOW (W)**

| # | Claim | State | Address / stage |
|---|---|---|---|
| W1 | the glow of the meters shows through an opaque window; the "body" — no: several objects | no: defect | glow in `meters.rs`/`theme.rs`/`timeline.rs`; one object in a layer under the windows. `ui-timeline`. R.75 |
| W2 | FX windows and others — outside the main window, without losing speed, in the same style | partial | the viewports mechanism exists (preview); the `WINDOWS` registry windows are `egui::Window` inside. Any registry window is detachable. Risk — the speed of immediate viewports. `ui-reach`. R.75 |
| W3 | the event window's title — to the point, the event number unobtrusive | no: UI | R.75 |

**AUTOMATION AND PRESETS (A)**

| # | Claim | State | Address / stage |
|---|---|---|---|
| A1 | selecting and moving keyframes | no: feature | `automation.rs` — keyframe one by one (R.67 "keyframe only by gesture"). `automation`. R.76 |
| A2 | own presets of curves/chains/effects; take from another event by reference | partial | effect presets — `FxPresetStore` (R.48); linking parameters between events — R.61 (`links.rs`); curves and chains as a preset — no. R.76 |
| A3 | for all entities — copy, default and own presets | partial | the FX buffer exists (`FxSlotBlock/FxChainBlock/EnvelopeBlock`), there are no transition/curve presets. A single contract — a `ui-reach` requirement. R.76 |

**LAYOUTS (L)**: L1 "Layout & Controls style" on first entry, a "like Vegas" tile,
trademark captions; L2 customize + "save code" — no: feature → F-UI-002, F-UI-003
(candidates). Later.

**NLE (N)**

| # | Claim | State | Address / stage |
|---|---|---|---|
| N1 | multicam | no: feature → F-TL-005 (reconnoitred, C, XL) | later |
| N2 | other basic features, "check the feature DB" | registry: F-TL-001 ripple (A, no), close gaps, … — the list is in the registry | later |
| N3 | auto-mute/boost of the background by sections (title/content) | partial | sidechain ducking R.65 — by signal; "by sections" — automation of track volume from `MarkedArea` regions with a role; Premiere has auto-ducking by clip type. Question 10; no note. Later |
| N4 | whisper: text search, pauses; analysis of pauses by sound | no: feature, no note | FFmpeg 8 carries the `whisper` filter. Future |
| N5 | dynamic analysis (tempo/beat, expressiveness, motion, MD-SAD) | future | no note |

**CORE / FORMAT (C)** — the file was written before/during R.70; part of it is closed by it

| # | Claim | State | Address / stage |
|---|---|---|---|
| C0 | we are moving away from OTIO, crutches — into export, export/import last; name `ktproj`; remove import of old `ktproj` | **done R.70 with discrepancies** | extension `.kt`, not `ktproj` (two formats under one suffix — a trap; the legacy `.ktproj` lives on in import) — question 1; import left as one door by its own decision R.67 — question 2; export done at once (t814). Rule "list of what breaks OTIO" — no → stage 3 of this change |
| C2a | the parser indexes 10 million elements, we look at the middle — it does not hang | no | `from_text` reads the whole file. 15,220 of the owner's events in `.kt` ≈ 3 MB, read in milliseconds; 10 million ≈ 2 GB of text. Is the threshold real? — question 3. `project-format`. After the answer |
| C2b | indexable in git, several people on a project | **done R.70** (t822) in the git part; "several people" — F-COL-* (reconnoitred) | later |
| C2c | less copy-paste: references for attachments; reference chains of effects and copied events | partial | reels are references (R.70, t810); chains/copies — no. Is it about size (1000×100 effects ≈ a few MB) or about "edit in one place" (that is composition/public parameters R.59)? — question 4 |
| P1–P6 | plugin system: super-flexible, generators/effects as plugins, different languages (rust, wasm), scene exchange formats | no: feature → F-EXT-003 (accepted, XL), F-EXT-002 (partial) | an architecture round after the nearest ones; "rewrite half the code" — the cost is named in the note |

**FX (F)**: F1 lens — the corners of the lens, dirt/drops/flares — no, no note (related
F-FX-002 is done); F2 letters as physical objects with inertia from rate — no → F-FX-005
(reconnoitred, XL). Later.

**OTHER**

| # | Claim | State | Address / stage |
|---|---|---|---|
| R1–R4 | reconnaissance of 3D scenes among professionals, bug-tracker dumps, expand the base | no: reconnaissance | `docs/features/_trackers.md` exists; a reconnaissance task. Later |
| O1–O4 | OpenFX: hook it up, where the plugins are, what else is in OFX, where we are incompatible | no → F-EXT-001 (candidate, XL) | reconnaissance questions. Later |
| X1 | FastCachedRender: cache hashes in SEI/sidecar, reuse of NALUs, the "already cached" zone | no → F-EXP-003 smart render (candidate, M) — extend | later |
| M4 | AVX/SIMD for audio FX | no | `kt-fx/simd.rs` — video only; by the measurement rule 1.3×. `audio-fx`. Later |
| M5 | "playback froze: 5" — what is this; the important thing — into the bell | question/LC | the line is not in the sources (renamed or gone); rule 16 already requires the bell. R.73 |
| Q1–Q3 | two RAM counters do not agree; RSS 4–10 GB against 500 MB; breakdown by sections and `ktindex` | no → F-RES-011 (candidate) | later |
| K1, K2 | a correlator; auto-blurring | future | no note |
| S1 | a convenient freeze frame | partial | `rate 0` = freeze frame (R.57), there is no "freeze frame here" command (split + rate 0). `transitions`/`ui-timeline`. R.73 |
| E1 | real 3D (irreversible mappings, meshes), custom parallax shapes | future / reconnaissance R | — |
| MW1 | media widget: timecode under the cursor while scrubbing, move to a point | no: minor | R.75 |
| TD1, TD2 | an effect on one word of a crawl; fisheye on acceleration, tied to rate; the ambiguity of play/scrub | partial | the parameter link R.61 solves "tie fisheye to rate"; the rest — F-FX-005. Future |
| FUT | MIDI keyboard, fruc_vulkan, objectdetection, mcp server, HDR10, links to videos | future | notes — as they come |

### 4. Questions for the owner (he asked "don't limit yourself")

1. **Format name.** The file says `ktproj`; R.70 chose `.kt`, because `.ktproj` is a live legacy under the same suffix (a trap for people and scripts). Keep `.kt`?
2. **Import of old files.** The file says "delete it"; R.67 — your decision "import is its own door". Your 115 MB projects open ONLY through it. Keep one door?
3. **The threshold "10 million elements".** Your project 115 MB = 15,220 events = ≈3 MB `.kt`, read in milliseconds. What project size is real, so that it counts as "does not hang"?
4. **Reference chains/copies of events** — is it about file size (1000 copies × 100 effects — a few MB) or about "I edit in one place — all change"? The second is composition with public parameters (R.59), which already exists.
5. **Timecode as a mode of the text generator** (one kind with a full text passport), not a separate kind?
6. **Which generators are needed first?** (there are 8: solid, gradient, noise, bars/SMPTE, shapes, text, timecode, test pattern).
7. **Space from pause.** A short press — still a toggle, and "hold" starts from the threshold (say, 0.3 s)? Or always "release — stop"?
8. **Frame step.** Shift on back/forward is taken by the region edge; which modifier (Alt?) or other buttons?
9. **"marker over the region"** — which menu item/gesture is this? Not found by name.
10. **Auto-mute by sections** — are sections set by regions (`MarkedArea` with the role "title/content") or by track/clip type, like Auto-ducking in Premiere?
11. **Order of rounds**: R.72 generators → R.73 timeline and transport defects → R.74 render → R.75 windows/glow → R.76 automation/presets. Is that right?

### 5. Order of the nearest rounds — why this one

Generators first — this is the first section of the file with the most detailed elaboration, and the only one
where it directly says "split into stages, discuss and decide". The timeline/transport second —
there are twenty defects there at cost S each, two of them repeats of earlier themes. Render third —
the crash on vaapi (T16/T17) and the encoders are already standing in the queue of specs. Windows and automation —
after: this is UI work at cost M that does not block editing.

## Risks / Trade-offs

- **The registry gets outdated** → the guard keeps ONLY the form and the existence of tests, not the truth of "done"; the truth is LC in the round. This is deliberate: a guard of truth would be a test of everything.
- **Unanswered questions block stages** → items in the state `question` are not taken into a round;
  everything else is taken.
- **The file was written before R.70** and part of it already contradicts its decisions → the discrepancies are named as
  questions 1–2, not silently resolved.
