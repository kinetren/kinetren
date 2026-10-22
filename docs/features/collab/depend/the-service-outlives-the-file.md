---
id: F-COL-044
title: What survives the service
status: researched
priority: B
cost: S
category: collab/depend
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, otio-lib-0.18]
we_have: partial
crates: [kt-otio, kt-model, kt-app]
invariants: [11]
ui_rules: [16]
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [frame-io, team-projects, bm-cloud, resolve-project-server]
checked: 2026-09-08
---

# F-COL-044 — What survives the service

## Essence

"Will I open this in ten years?" is a question with two different answers depending on where the work lives. For a file-based document the lifespan is set by the FORMAT: while there is a specification and something to read it with, the document is alive, and nobody outside affects this. For a service-based document the lifespan is set by a CONTRACT: the owner of the service may change the generation, change the owner, switch off the integration or hide part of the data — and all of this happens to work that the user considered his own.

A modern editor has the edit and the review on different sides of this boundary: the edit is in a file or in a database, the remarks are in the service. Therefore one and the same project has two different lifespans, and they do not coincide.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | `.veg` | a binary proprietary container, readable only by VEGAS itself; there is no service half at all, so the lifespan is exactly one — and it is determined not by the format (there is no specification) but by whether the program is still sold |
| Adobe Premiere | 26.x | `.prproj` + Frame.io / Team Projects | the document has TWO lifespans: the edit in the binary `.prproj`, the remarks in the service. Team Projects keeps the edit entirely on Adobe's side. Frame.io is a precedent of a generation change: 2026, the transfer of retail customers from Legacy to V4, together with which the list of integrations stopped working |
| DaVinci Resolve | 21 | a project is a database row | the project has no native file: it is a row in a disk database, in PostgreSQL or in the cloud library. `.drp` is an export, not the place of life. The lifespan is set by the database in which the project lies; for the cloud one — by a contract, for Project Server — by its own host |
| Final Cut Pro | 12.3 | `.fcpbundle` | the library is a package with a database inside; there is no service half, the lifespan lies entirely in the hands of Apple and the version of the application. The exchange format (`fcpxml`) exists, but it is not what the work lives in |
| After Effects | 26.x | `.aep` + Frame.io | the purest case of separation: AE does not store the comment in the document at all — the column holds a link that pulls the text from the service. One can open `.aep` in ten years; to read the remarks in it — only while the service is alive |

**Outside the core.** One row is needed because the question "what does the reader do with an unfamiliar construction" has been solved in the domain exactly once, and not by an editor.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| OpenTimelineIO | 0.18.x | schema versioning + `SerializableObject` | the only place in the domain where a schema DOWNGRADE is declared as an API (`target_schema_versions`, version maps by release), and an unfamiliar schema does not break reading but becomes an object that remembers it verbatim (issue #326, 2018) |

## The trick behind it

**1. Frame.io is a ready-made precedent, and what is valuable in it is not the breakage but the form of the documentation: the step forward is described by name, the step back is not described at all.** The article "What to expect when updating to V4" (updated 16.06.2026) lists by name what stops working after the transition: the Legacy panel in Premiere, the integration with DaVinci Resolve, creating a Final Cut project from the share destination, publishing to Vimeo and Dropbox, Capture One, Carousel Viewer, new links to Slack. The list is detailed, engineering, honest. And yet in the entire article there is not a single sentence about whether one can RETURN to Legacy. This is not forgetfulness of the help editor: a service-based document has no concept of a backward step — the version of the service is not the user's choice, and therefore there is nothing to describe. The comparison with the file world is literal: in OTIO a schema downgrade is a public API with version maps, because there the reader and the writer belong to different people; in the service they belong to one, and the question disappears.

**2. Within the same transition one sees exactly what the review pays with, and this is a quantity that a file never has at all: DEFAULT VISIBILITY.** Part of the data did not disappear and did not break — it stopped being shown: "those comments (now known as Internal Comments) will be hidden by default". Another part requires manual restoration of access: "all Members must be invited back to them to gain access". That is, the document survived the transition, but the right to see it did not. A file cannot be built this way: a file has no state "exists, but hidden by default", and there is no party that could switch this state. This is the most precise formulation of the difference between the two lifespans that could be found: not "the data is gone", but "the data has stopped being yours by default".

**3. The third detail is ours, and it spoils the pretty answer "we have a text JSON, so forever".** Our format guarantees that in ten years the file will be PARSEABLE; it does not guarantee that an effect inside will mean anything. The effect is written with a string tag (`#[serde(tag = "kind", rename_all = "snake_case")]`, `src/libs/kt-model/src/project.rs:2242`) — the file contains `"kind": "slowed_reverb"`, and what this string means lives in an enumeration inside the binary. A reader does NOT break on an unknown tag and does not throw it away: it puts the effect among foreign ones with a notice (`src/libs/kt-otio/src/convert.rs:1211`, `unreadable effect … kept as foreign`) — that is, it applies to its OWN lost schema exactly the mechanism that was set up for FOREIGN ones. The practical consequence: an edit ten years old will open and play, and a vanished effect will turn into a silent zero plus a line in the log. **Our "contractual" half exists, it is just that we ourselves are the party of the contract** — and it is held up by the enumeration in `kt-model` and by a registry line in `src/apps/kt-app/src/fx_registry.rs:47`. The asymmetry here is the reverse of what one would expect: an old project with a vanished effect still somehow lives, but to create such an effect anew is no longer possible — the file survives the possibility of creating it.

## Pros

- The project format is a text JSON under the open OpenTimelineIO specification. It can be read by eye, put into git, compared with a diff, repaired by hand and parsed by a foreign program. None of these properties exists for `.veg`, `.prproj`, the Final Cut library, or a row in PostgreSQL.
- An unfamiliar construction does not break reading: a foreign schema goes into passthrough and survives the round trip entirely, and an unreadable effect becomes a foreign object. The file always opens — the content degrades, not the access.
- We do not have a second half that could be switched off by the owner's decision: there is not a single project quantity whose meaning would live on the other side. Link columns, as in After Effects, cannot appear with us.
- Invariant 11 makes a stronger promise than "it will open": the committed references require that an old project also RENDERS byte-for-byte the same. Our durability is verifiable, not declarative.
- Paths to material are relative to the project file (`src/libs/kt-otio/src/url.rs`), therefore "in ten years" does not break even on a moved folder.

## Cons

- **This advantage is bought by refusing the service half entirely.** "Outliving the service" has nothing for us to outlive, because there is no service: remarks, versions of a cut and approval do not exist in any form, and the question of their lifespan has no subject. Formulating the advantage without naming the price would be a lie.
- The meaning of an effect lives not in the file but in the binary. Removing a variant of the enumeration is a routine refactoring operation which today has not a single guard: it silently zeroes the effect in all old projects.
- The trace of this zeroing is one line in the log (`notices`), that is, a surface that a person does not look at if nothing has broken. On the timeline a vanished effect is marked by nothing.
- The version of the DOCUMENT is not in the file — there are only versions of the FORMAT (F-RES-003). The file does not answer the question "is this the edit version that was approved in March", and this is exactly the quantity for whose sake services set up a stack of versions.
- Our durability depends at one point on someone else's library: the OTIO specification does not belong to us. It is open, and this is incomparably better than a contract, but "nobody's" and "ours" are different things.

## What users say

The most precise illustration that the lifespan is set not by the format but by the side: the service rewrites the content of the LOCAL document, returning it to its own version.

> Frame.IO V4 is overwriting & deleting comments markers on my sequences in
> Premiere Pro, and I need it to stop making my comments markers revert to the
> previously saved version on Frame.IO.
> — Andrew C. Brinsmead, 28.10.2025,
> https://community.adobe.com/bug-reports-728/frame-io-v4-keeps-overwriting-comments-markers-in-premiere-pro-pro-1332248

And a second one, about how the transfer of data across the service boundary looks when the integration itself works: the remarks arrive, but not where they were and not in the form they had.

> So I import comments much like premiere but it creates a text layer as a guide
> but I cannot see any of the comment text.
> — curt.gallery, 05.02.2024,
> https://community.adobe.com/t5/after-effects-discussions/imported-frame-io-comments-not-visible/td-p/14403508

The note has no counting part: `trackers:` is empty deliberately. The durability of a document in the trackers of open source is represented by tasks about migrating ONE'S OWN format (they are analyzed in F-RES-003), and about "outliving someone else's service" there are no tasks and there cannot be — free editors have no service that could be outlived.
Filling the field for the sake of the form would mean fabricating a signal (rule 12).

## How it fits Kinotrain

### What already exists

- `src/libs/kt-otio/` — the project format: a text JSON under the OpenTimelineIO specification, our quantities in `metadata["kt"]`, foreign schemas in passthrough. The round-trip tests t144–t147 (idempotence, passthrough, versions, paths) and t149 (conformance with the reference library) are exactly the mechanical check of that same "in ten years".
- `src/libs/kt-otio/src/convert.rs:1211` — the only place where the fate of an effect whose `kind` is unknown to the reader is decided: it goes into `effects_foreign` with the notice `unreadable effect … kept as foreign`. The mechanism of a foreign schema is applied to our own — so our format pays the debt for item 3 of "the trick behind it".
- `src/libs/kt-model/src/project.rs:2242` — `FxInstance` with a string tag; the comment says directly "typed FX (not opaque JSON — validation on load, test 12)", and exactly this typing makes the set of effects part of the contract, not part of the data.
- `src/apps/kt-app/src/fx_registry.rs:47` — the second half of the same contract: the table by which a string from the file becomes a name in the picker. The string has left the registry — the old effect still plays, but a new one of the same kind can no longer be created.
- Invariant 11: GOLDEN_AUDIO and GOLDEN_VIDEO are committed; audio is byte-for-byte always, video on the CPU path is checked against the reference in the repository. This is a promise stronger than "the file will open".

### What is missing

- An event "the effect became foreign" has no visible trace anywhere except the log. On the clip it is indistinguishable from a clip without effects, and the editor learns of the loss only from the finished render — that is, by the same scenario analyzed in F-RES-007.
- There is no test that would take a project with a deliberately unfamiliar `kind`, and check that the file opened, the edit is intact, the effect was preserved as a foreign one and came back to the writing unchanged. The mechanism exists; its guard is absent.
- There is no document version in the file: `model_rev` and `saved_rev` are runtime quantities and do not get into the file. "Is this the right edit" is checked only by eye.

### What gets in the way

Nothing constructive, and this should be said directly: the half of the question that costs dearly for others and is solved by contract, we have solved for free and forever by one decision of R.24 — not to set up our own container. What gets in the way is the reverse: the temptation to declare this a complete victory. It will not be complete as long as we have our own "contract" — the enumeration of effects — which has neither a version, nor a guard, nor a trace on the screen.

### In category `collab/` — two mandatory lines (rule 13)

OFFLINE: DEGENERATES — `src/apps/kt-app/src/fx_registry.rs:47` — durability remains entirely with the edit and degenerates where the document refers to something BY NAME: the file itself will be parsed in ten years, because it is a text JSON under an open specification, but the effect inside means exactly what the string in the registry table means and the enumeration variant next to it; if the string disappears, the old project will open and play, the effect will silently become a foreign object, and there will be nothing with which to create the same one anew.

NETWORK COST: a document living not as a file but as rows on someone else's side — project-as-database — only there can the lifespan be set by a contract: while a document is a file, it cannot be hidden by default or taken away together with the right of access, and the whole class of failures "the data is intact, but you will no longer be shown it" has no place to stand with us.

### Which invariants it touches

Invariant 11 is the only one, and here it works as an argument, not as an obstacle: determinism with committed references turns "the file will open" into "the file will open and give the same bytes", that is, into a verifiable promise of durability. Rule 16 of the UI checklist (`src/apps/kt-app/CLAUDE.md`) — "the impossibility of an action is shown as an ERROR, not as a faded hint" — at this point is not fulfilled at all today: the impossibility of reading an effect is not shown even as a faded hint, but goes as a line into the log, that is, more quietly than the mildest of the forbidden variants of the rule.

## Estimate

| What | Answer |
|---|---|
| Volume | S — one stage: a test for an unfamiliar `kind` and a visible trace "the effect was preserved as foreign" |
| Where the main work is | kt-otio (the round-trip test), kt-app (the trace on the clip instead of a log line) |
| Project format | does not touch it: the mechanism already exists, only a guard is added |
| Tests that will have to be written | a round trip of a project with an unfamiliar `kind`: it opened, the edit is intact, the effect came back to the writing unchanged; a check that the notice reaches the surface, not only the log |
| Depends on | F-RES-003 (format migration), F-COL-008 (what a foreign file loses) |
| Breaks compatibility | no |

## Verdict

We do not set up a service half — there is no participant, and the note has no right to declare his appearance. From the reconnaissance we take three conclusions, and all three apply alone.

**First, and it must be named EXPLICITLY, because it was bought dearly.** Our format is a text JSON under an open specification, and this is an advantage of exactly that class which for others costs a subscription: the document cannot be hidden by default, taken away, or moved to a new generation without asking. It was bought by refusing the service half entirely — there are no remarks, versions of a cut, or approval with us — and therefore the formulation "we are more durable" is legitimate only together with "we are poorer". Both halves belong to one decision, R.24, and in conversation they cannot be separated.

**Second, and this is the only WORK the note leaves.** We have our own contract, and we are a party to it: the set of variants of `FxInstance` and the registry strings. Today removing a variant is an ordinary refactoring without a single guard, and its price is the silent zeroing of the effect in all old projects. A round-trip test for an unfamiliar `kind` and a visible trace on the clip instead of a line in the log are needed. This is cheap work, and it turns our advantage from a declaration into a property.

**Third, as a decision.** No quantity of the project has the right to live on someone else's side — neither as text nor as a link. The After Effects path, where the document's column holds not a comment but the address of a comment, is closed for us: it makes the lifespan of the document equal to the lifespan of the contract, in the most unpleasant form — the file is intact, but it cannot be read.

Boundaries. **F-COL-043** — the price of shared work: there it is about the account and about what happens when one stops paying for it; here it is about the fate of the data itself, which does not depend on payment and is decided by a change of the service generation. **F-COL-045** — the user account: there it is about the condition of entry, here it is about what remains when there is nowhere to enter. **F-COL-007** — exchange formats: there it is about what is lost when the edit is TRANSFERRED to another, here it is about what is lost when it is STORED over time. **F-COL-008** — what a foreign file loses on passing through us: the neighboring half of the same passthrough mechanism, but from the point of view of a foreign schema, not of our lost one. **F-RES-003** — format migration: there it is about the versions of OUR format and the path forward; here it is about what the reader does when the path forward has already been traversed and the data has remained from the past. **F-PRJ-002** — versions and snapshots: there it is about the history of a document within one person.

## Sources

- https://help.frame.io/en/articles/11534393-what-to-expect-when-updating-to-v4-a-comprehensive-guide-for-updating-your-account-on-a-free-pro-or-team-plan
  — "What to expect when updating to V4", updated 16.06.2026: a named list of
  integrations that stopped working, "those comments (now known as Internal
  Comments) will be hidden by default", "all Members must be invited back to
  them to gain access"; no mention at all of a return to Legacy (accessed
  2026-09-08).
- https://community.adobe.com/bug-reports-728/frame-io-v4-keeps-overwriting-comments-markers-in-premiere-pro-pro-1332248
  — Andrew C. Brinsmead, 28.10.2025: the service overwrites markers in the local
  sequence (accessed 2026-09-08).
- https://community.adobe.com/t5/after-effects-discussions/imported-frame-io-comments-not-visible/td-p/14403508
  — curt.gallery, 05.02.2024, and the reply by David Arbor, 07.02.2024, that
  comments arrive as a text layer while the markers themselves stay empty
  (accessed 2026-09-08).
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  three places where a Resolve project lives, and the free Project Server
  (accessed 2026-09-08).
- The service registry and the rule "a change of service owner matters more
  than a change of editor version" — `docs/features/_services.md`; the row
  `otio-lib-0.18` and its justification — `docs/features/_editors.md`.
- Our code, checked by reading on 08.09.2026: `libs/kt-model/src/project.rs:2242`
  (the tag string of `FxInstance`), `libs/kt-otio/src/convert.rs:1211` (an
  unknown effect → `effects_foreign` with a notice), `apps/kt-app/src/fx_registry.rs:47`
  (the name table), `libs/kt-otio/src/url.rs` (relative paths).
