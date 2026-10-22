# Design

## Context

See proposal.md — Why. Constraints that set the shape:

- OpenSpec (1.13, schema `spec-driven`) has exactly two long-term memory places:
  `openspec/specs/<capability>/spec.md` (Purpose + Requirements with scenarios; `opsx:propose`
  must review them via `openspec show`) and `openspec/config.yaml` (`context` and `rules` are
  injected into every artifact generation). `design.md` inside a change **goes to the archive
  together with it** and is not read by a new change.
- The main-spec parser reads only `## Purpose` and `## Requirements`; foreign files next to
  `spec.md` it does not touch either in validation or in archiving, but it also does not show
  them through `openspec show`.
- The unit of work in the project is a round R.N with stages S1…S5; the regression tests tNNN
  are numbered across the whole project and are the actual contracts.

## Goals / Non-Goals

**Goals:**
- Every decision that still constrains the code lies in exactly one place, which OpenSpec reads
  on every new change.
- The "why" of a decision (the owner's pain, the rejected options, the non-obvious detail)
  survives the migration and is reachable from the spec in one step.
- `docs/features/*` returns to its role: the base of other editors' features.

**Non-Goals:**
- Do not rewrite history: `user-feedback.md` is not touched, `porting-notes.md` is frozen, not
  cleaned. Feature notes with status "done" remain frozen (rule 7 of the README) — only the
  "How it fits" section is cut out of them.
- Do not change code, tests, invariants. Do not rename tests.
- Do not build our own OpenSpec schema (`openspec/schemas/`): the standard one is enough.

## Decisions

### D1. The long-term "why" — `design.md` next to the capability's `spec.md`, not inside the requirements

`openspec/specs/<capability>/design.md` — a file outside the OpenSpec parser. The link is held by
`config.yaml`: `rules.proposal` and `rules.design` oblige every affected capability's `design.md`
to be read, and a contradiction of it is formalized as an explicit item "revision: <decision> —
<reason>". The rule is injected into every generation — this is more reliable than the agent's
memory.

*Alternative — the "why" inside the text of the requirements*: OpenSpec would read it itself, but
the spec instructions forbid function names and `src/…` paths, and 23,000 lines of "our" material
from features do not fit there. Rejected. Compromise: in the requirement — one line of "why", the
full argument — in `design.md`.

### D2. Capabilities — by behaviour, 17 of them

Not by crates (kt-engine = "everything") and not by feature categories (`help/`, `collab/` are
empty for us). The slicing coincides with the test table of CLAUDE.md and with how the rounds were
named.

| capability | contracts (spec.md) | "why" (design.md) — sources |
|---|---|---|
| `time-model` | ticks 90 kHz, burst 1024/1920, fps grids, tick↔sample 15/8 | CLAUDE.md "Time model" |
| `transport` | clock = cpal callback, starvation does not stop time, `video_playhead()`, speed ±, freeze frame, scale | inv. 6/12; R.43 (M3TEOR), R.57; kt-engine/CLAUDE.md |
| `audio-render` | f32 48k/2ch always, FX state only on the render thread, chain diff by clip id, PDC, ducking, LUFS, reverse/warp, cost O(1) from project size | inv. 1–3; the defect catalogue slowed&reverb; R.48; features audio/processing (8 done), loudness, audio-waveform-sync |
| `video-plan` | one composition code (`frame_plan`), decode scale step in the order, CPU = byte reference, GPU by tolerances via `assert_frames_close`, memoization of post-FX, export does not mix backends | inv. 4/7/11; R.44–46; features transform-concatenation; kt-gpu/CLAUDE.md |
| `video-fx` | registry of 27 kinds, `plan_chain` the only place of decisions, K× barriers, golden cycle t328/t329, `geom_fused.wgsl` ↔ `geom.rs` as a pair, picker thumbnails | R.47, R.54; features fx/* (4 done), color/basic-color-fx, preset-gallery |
| `audio-fx` | registry of 16 nodes, frame of t322, slot order, spectral probes | R.47–48; features audio/processing |
| `transitions` | 33 kinds, `transitions.rs` ↔ `composite.wgsl` as a pair, MODE_* only at the end, inversion by the plan, overlay, closed-form t459 | R.33 S4, R.51; features transition-vocabulary, same-track-transitions |
| `automation` | domain of keys = material, compiled video curves, passport at runtime, parameter types, clamp in the macro, expressions/plugins (interfaces) | R.33–34, §466–467; features keyframe-domain, presets-carrying-automation, expressions-and-drivers, plugin-abi-and-sandbox |
| `compositions` | reel = link, laws E and G, three keys, reel scale step ≠ instance input, expansion strip, zero is not a wall, public parameters and `gen_salt`, deferred §464 | R.50, R.55, R.56, R.58, §459–464, §469–470; features nested-sequences, inline-nest-expansion, composition-public-parameters |
| `morpher` | layers, placement, bake == render, "by time" cut suppresses by force, source swap carries curves by one delta | H1, R.35–37, item 300; user-feedback about Adjustment Events |
| `generators` | 8 kinds, pseudo-paths only through `factory`, titles: vendored font + pins + FONT-MANIFEST, native 4096, parameters on the event | R.52–53, §468; features titles/* (5 done), solid-gradient-shapes |
| `media-index` | decoder ownership (the two halves of inv. 5), MediaWorker queue with priorities, scroll/zoom does not decode, sidecar DB, dlopen FFmpeg 6/7/8, `ldd` without libav | inv. 5/8/10; docs/media-index.md; R.49 wave B (BgJob/IndexService) |
| `export` | in-process, encoder cascade with a real opening, preview == export, golden audio/video/titles | inv. 9; features hardware-encoder-selection, preview-vs-export-mismatch |
| `project-format` | .otio, `metadata.kt`, versions v1–v9 by content, passthrough of foreign data, broken kt section = error, ticks only | docs/file-format.md; kt-otio/CLAUDE.md; §464 |
| `durability` | autosave into recovery, flock, Drop-boundary, journal, exit watchdog, second copy | R.49; features resilience/* (6 done); user-feedback "you did not check" |
| `ui-timeline` | repaint = motion, preview key by the fps grid, no allocations in draw, usability checklist, limits "set on a complaint", i18n and glyphs, `finish.sh` + version | kt-app/CLAUDE.md; R.4–5, R.8–13; the "repeated" table of user-feedback |
| `ui-reach` | "there are no dead ends": t252–t255, verbs for each `TlEntity`, a right click does not stay silent, live scenarios per item | R.38, R.40, wave V of R.49; features help/* (already-have ×4, done ×1) |

Disputed boundaries, accepted consciously: `ui-reach` separate from `ui-timeline` (its own test
mechanism, seven rounds of repeats); `morpher` separate from `compositions` (older than reels, its
own domain of curves; otherwise design.md of compositions swells).

### D3. The culling criterion — one for all three sources

A line stays if it **still constrains the code**: there is a test tNNN, an invariant or a rule that
an agent would break without knowing it. Together with it its "why" stays. Dropped: chronology
("in R.43 first…"), "rewritten from C++", comparison with v1, rejected options without a reason,
item numbers as such (a reference to the test replaces the reference to the item).

Form of a scenario in `spec.md`: `#### Scenario: <name>` with WHEN/THEN and a last line
`- **TEST** tNNN (`crate/tests/file.rs`)`. So the table "crate → tests" from CLAUDE.md moves into
the specs without loss.

### D4. Order of work — one capability at a time, each: spec → design → truncation of sources

For each capability: (1) collect its lines from the three sources by the table in D2 (grep by the
tests tNNN and by the paths `src/…` — more precise than by headings); (2) write `spec.md`;
(3) write `design.md`; (4) truncate "How it fits" in the affected feature notes to a reference;
(5) `openspec validate --strict` and `features-index.py --check`. Only after all 17 — `config.yaml`,
freezing of porting-notes, cleaning of CLAUDE.md. So until the last step the old chronicles remain
intact, and the work is reversible.

### D5. `porting-notes.md` is frozen, not cleaned

The defect catalogue of the old engine and the chapters of the rounds are the history of the port.
Cleaning would cost more than the transfer and would destroy the context that the commits refer to.
A line is added to the header: "frozen at §470; current rules — `openspec/specs/`".

## Risks / Trade-offs

- [Loss of a decision during culling — the criterion "constrains the code" will miss a rule without
  a test] → before truncating a feature note, compare the list of tNNN and `src/…` paths in its
  "How it fits" with the `spec.md`/`design.md` of the capability; uncovered ones are to be carried
  over, not thrown away.
- [`openspec show` does not show `design.md`, the agent will not open it] → a rule in `config.yaml`
  for `proposal` and `design`; a live check: the first real change after the migration must reference
  the capability's `design.md`.
- [Duplication between the invariants in CLAUDE.md and the specs] → in CLAUDE.md the invariants stay
  one line each with a reference to the capability; the full text — only in the spec.
- [`features-index.py --check` depends on rule 5 (the paths in "How it fits")] → the rule changes
  together with the script in one task.
- [Scope: 17 capabilities × 3 sources] → tasks one capability at a time, each self-sufficient and
  verifiable by `openspec validate --strict`.

## Open Questions

- The fate of `docs/architecture.md`: a table of contents of `design.md` or deletion — to be decided
  when it is clear what is left after the transfer.
- Whether `time-model` needs to be a separate capability or its place is in `transport`: decided by
  the volume after culling (if the spec has < 3 requirements — merge).
