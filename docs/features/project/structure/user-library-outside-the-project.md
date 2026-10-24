---
id: F-PRJ-004
title: User storage shared between projects
status: candidate
priority: B
cost: L
category: project/structure
editors: [resolve-21, after-effects-26, premiere-26, fcp-12, vegas-pro-2026]
we_have: partial
crates: [kt-app, kt-model, kt-otio]
invariants: [11]
ui_rules: [16, 19, 20]
round: null
point: null
tests: []
depends_on: [F-PRJ-003, F-AUT-004]
checked: 2026-09-21
---

# F-PRJ-004 — User storage shared between projects

## Essence

There are things that in meaning belong not to the project but to the person: their set of FX presets, their title templates, their export presets, their keyboard layout, their new-project templates, their template compositions. They must survive the closing of a project and appear in the next one. The question is not "where to put the file" but "what it is paid for with": everything that went into the user's storage has left the project, and on someone else's machine the project will not find it.

Boundaries. What exactly is lost when a project is moved is F-PRJ-003 (`project/structure/project-portability.md`); collecting media into one folder is F-MED-005; the FX preset library itself as a feature is F-AUT-004. This note is about the PLACE shared by six features, and about its mandatory second half.

## Who has it and how

The question for each: where does "mine", which survives the project, live, and what happens to it when the project is moved to another machine and when the program is updated.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| DaVinci Resolve | 21 | **PowerBins, Power Grades, Templates — inside the DATABASE** | the strongest entry. In Resolve a project is not a file but a record in a database, and "mine across projects" lives THERE TOO: a PowerBin is visible from any project of this database, a PowerGrade is the same for grades. The consequence is structural: the library travels not with the project but with the DATABASE, and is moved by its mechanisms (database export/archive), not by copying the project file. The reverse side: another database means another library, and "giving a project to a colleague" gives nothing from the PowerBins |
| After Effects | 26.x | `Presets` folder next to the installation + local Essential Graphics templates | animation presets `.ffx` are files in the user/installation directory and are picked up in the Effects & Presets panel by subfolders as categories; Motion Graphics templates have a "local templates folder" assigned in the settings. A preset is a separate file, so it gets backed up and sold (an industry has grown around `.ffx`), but it travels with the project by nothing |
| Adobe Premiere | 26.x | version profile: `Effect Presets and Custom Items.prfpset`, Libraries | the entire user effects library is ONE file in the profile of a SPECIFIC version (`Documents/Adobe/Premiere Pro/<version>/`). Two chronic classes follow from this: switching to a new version leaves you with no presets at all, and imported presets disappear on restart. Libraries (cloud) are a second, independent mechanism with its own life cycle |
| Final Cut Pro | 12.3 | `~/Movies` (libraries) + `~/Library/.../Motion Templates`, Effects Presets | the only one that made portability an EXPLICIT SETTING: custom titles, effects and generators from Motion are by default in the user directory and do NOT travel with the library, but the library properties have a `Motion Content: In Library` switch and a `Consolidate` button. By default it is NOT in the portable position, and Apple explicitly writes that it has no effect on a third-party FxPlug |
| VEGAS Pro | 2026.0 | registry + `Documents`, whoever how | the worst end of the scale: different kinds of presets lie in different places (part in `Documents`, part in the Windows registry), and there is no single "my catalog". The practical consequence: a clean reinstallation or removal of an old version takes part of the library with it, and which part the user learns only afterwards |

## The trick behind it

**1. Every feature that put its own assets into the user storage SUBTRACTED from portability — and this is not a side effect, but a structural trade-off that exactly two have acknowledged.** Resolve answered "the library is part of the database": it travels with the database, not with the project, and therefore in principle cannot "get lost when the project is moved" — it is simply not there. Adobe answered "synchronization through the account": Libraries live in the cloud, not next to the file. Apple answered with a switch: `Motion Content: In Library` plus a `Consolidate` button, and it must be pressed BEFORE copying. These are three different structural answers to the same question, and none of them is "convenience": each has its own price — someone else's database, someone else's subscription, a forgotten button. Where there is no answer at all (VEGAS), the user library is precisely the dependency that silently does not travel.

**2. A monolith in the VERSION profile is the most expensive of the cheap schemes, and you pay for it exactly on the day you update.** In Premiere the entire user effects library is one `.prfpset` file in the directory of a specific version; in VEGAS some presets are in the registry. The outcome is the same for both: "if you leave Vegas Pro 9 and just install Vegas Pro 10 next to it, all your presets will be kept. If you uninstall Vegas Pro 9, that will delete your presets" (John Rofrano, 23.06.2011). So the preservation of the library turns out to be a property of the INSTALLATION PROCEDURE, not of the program, and the user learns about it from a forum. A separate subclass of the same thing is presets that do not survive even a restart: the Adobe thread about this has run from 2016 to 2023, and the working workaround turned out to be a folder that the user creates by hand.

**3. A second tenant soon appears in a storage between projects, and it arrives FROM THE PROJECT, not from outside.** This is visible in our own code. The file `workspace.json` (`src/apps/kt-app/src/user_store.rs`) stores the width of the meters panel and the recent views per project — but earlier `meters_w` lived in the project file (`UiState`), and F-PRJ-003 named this the ONLY known case of "a setting traveled with the project": open a foreign project and the panel takes the foreign width. So the exchange goes both ways: the storage not only takes away from portability, but also returns to it what has no place in the project. Hence a practical criterion, which should be written down before the first line of code: what goes into the project is what the project cannot be reproduced without; what goes into the storage is what describes the PERSON, not the work. A preset, a template and a layout describe the person; the width of a panel does too; the text of a title lying in a template no longer does, and therefore a template must be inserted as a COPY, not by reference.

## Pros

- One place instead of six: the catalog, the atomic write and the policy for a broken file are written once, and the six features each place their own files next to it. Without it each would invent its own, and its own refusal policy too.
- A library outside the project gets backed up, versioned and transferred separately from the work: an entire industry of ready-made packages has grown around AE's `.ffx`, which would not have happened around a record inside the project.
- "Mine" appears in a new project by itself, without import: this is exactly the purpose of the feature, and none of the five could do without such a place.
- A library as part of the DATABASE (Resolve) removes the question "where did the presets go when the project was moved" entirely: they were never there, so there is no silent loss either.
- A "put inside the project" switch (FCP) is the only known form in which the storage does not reduce portability; it is also mandatory under the verdict of F-PRJ-003.

## Cons

- Everything that went into the storage is subtracted from portability: a project that refers to the author's service directory breaks on the recipient's side, and, as Flowblade #1168 shows (F-PRJ-003), it breaks on SAVING, that is, after an hour of work.
- A monolith punishes updates and clean reinstallations; different kinds of presets in different places (VEGAS) punish in the same way, only in parts.
- The storage acquires its own schema version, and therefore its own migration, and its own case of "a file newer than the program". This is a second format in the product, with the same obligations as the first.
- Name conflicts and the absence of history: the library knows no undo, and an overwritten preset does not come back.
- The storage is shared by all copies of the program on the machine: two running copies write to one file. The same class as the second copy and the recovery file (F-COL-013, R.42).
- Subscription synchronization (Adobe Libraries) moves the problem into the account: the library becomes a service with its own availability.

## What users say

The preservation of a library is a property of the installation procedure, not of the program:

> If you keep Vegas Pro 9 installed and just add Vegas Pro 10, all of your
> presets will be saved. If you uninstall Vegas Pro 9 that deletes your
> presets and then Vegas Pro 10 won't have them.
> — John Rofrano, Creative COW (VEGAS Pro), 23.06.2011,
> https://creativecow.net/forums/thread/when-upgrading-do-presets-saveae/

A smaller and nastier subclass: a library that does not survive even a restart; the thread runs from 2016:

> i import presets - fine...i can use them normaly - great...i save my
> project as usual. i start the program the next time and all the imported
> presets are gone.
> — kidgrillo, Adobe Community (Premiere Pro), 13.11.2016,
> https://community.adobe.com/t5/premiere-pro-discussions/effect-presets-disappear-after-restart-cc2015/m-p/8676875

> Everytime i restart premiere pro 2019 the imported presets are
> diappeared. I alway gotta reimport them...
> — Nikolai Reuther, same thread, 15.12.2018

And negative feedback of a special kind, from the first wave of the base, about the same monolithic storage of Premiere:

> all custom effects presets are gone and my custom bins are gone as well.
> — Anthony Paglia, Creative COW (Premiere Pro), 14.07.2014,
> https://creativecow.net/forums/thread/custom-effects-presets-disappeared/

## How it fits Kinotrain

### What already exists

**The place was set up in R.48 and already holds two tenants.**
`src/apps/kt-app/src/user_store.rs` is a shared module whose header names the reason directly: "Four reconnaissance features ran into the absence of a shared place (the FX preset library, title templates, export presets, keyboard layouts) — therefore the module is made SHARED: the catalog, the atomic write and the policy for a broken file are here, and the concrete libraries place their own files next to it".

What is already settled there, verbatim from the code:

- **Catalog** — `store_dir()`: `$XDG_DATA_HOME/kinotrain` (otherwise `$HOME/.local/share/kinotrain`), the same place where the autosave lies, "one place for the backup"; overridden by `KT_USER_STORE_DIR` for tests and live scenarios.
- **Two files side by side**: `fx-presets.json` (`FxPresetStore`, `PRESETS_VERSION = 1`) and `workspace.json` (`Workspace`: `meters_w` and the recent views per project).
- **The project is self-sufficient**: "the library holds insertion templates, not references. If the library file is lost, the projects are intact." This is exactly the answer to Flowblade #1168 from F-PRJ-003, adopted IN ADVANCE.
- **Records are stored raw** (`serde_json::Value`) next to the parsed copy: an unknown record from a future version is skipped when shown, but survives saving — "an old build does not eat the work of a new one". The same principle as passthrough in `src/libs/kt-otio/src/convert.rs`.
- **A file newer than the program is read-only** (`readonly`); **a broken file does not bring the program down**: it is moved aside to `fx-presets.json.broken` on the first write.
- **Factory presets live in the binary** (`include_str!` from `apps/kt-app/assets/fx-presets-factory.json`) and are parsed by THE SAME code as the user file, so the loader is checked on every launch.
- Tests: t344 (file round-trip), t344b (broken / foreign version), t344c (factory), t347 (compatibility by registry) — `src/apps/kt-app` unit `user_store`.

**And one debt has already been paid off by this place.** `meters_w` used to live in the project (`UiState`, `src/libs/kt-model/src/project.rs`) and overrode my width when a foreign file was opened — F-PRJ-003 named this the only known case of "a setting traveled with the project". Now the width lives in `workspace.json`, so the storage has already worked in the reverse direction: it returned to portability what has no place in the project.

**What is not in the project and must not be.** Across the whole of `src/libs/kt-model/src/project.rs` there is exactly one field with a path — `MediaRef::path`: no font, no LUT, no plugin, no cache directory, no encoder (analyzed in F-PRJ-003). This is that reserve of portability which every new library will spend.

### What is missing

1. **There is no "put inside the project" switch in any of the tenants.**
   The verdict of F-PRJ-003 requires it in plain text: "as soon as a user storage outside the project appears, a 'put inside the project' switch must appear on the same day". The storage appeared in R.48 — the switch did not appear.
2. **There is no report "what this project needs from outside".** An analogue of `Generate Report Only` does not exist; the closest thing is the list of unavailable sources at the RECIPIENT's side, and too late (`src/apps/kt-app/src/render_dialog.rs`).
3. **There is no transfer of the storage itself.** Export/import of a library as a single file (an analogue of `.ffx`, `.sfv`, the Resolve database archive) is absent: to take one's presets to a second machine, one must know about `$XDG_DATA_HOME/kinotrain` and copy by hand.
4. **There is no rule for who is next.** Of six candidates one is occupied (F-AUT-004). The others — title templates (F-TXT-002), export presets (F-EXP-001), keyboard layouts (F-UI-002), new-project templates (F-HLP-027) and template compositions (F-TL-012) — will each arrive with its own schema, its own version and its own question "does this travel with the project?".
5. **Two copies of the program write to one directory.** For the project there is a guard (`src/apps/kt-app/src/project_lock.rs`, t362); for the storage there is none.

### What gets in the way

Not the volume of code, but three pressure points, and all three concern decisions, not lines.

**First: "put inside" has no unit.** A preset is an `FxChainBlock` (`src/libs/kt-model/src/ops.rs`), and there is technically nowhere to put it in the project except a new key in the envelope `metadata["kt"]`; a template composition is a whole reel with content; a keyboard layout is meaningless in a project altogether. So the switch demanded by F-PRJ-003 is not one flag on the storage, but a DIFFERENT answer for each tenant, and this must be decided as a list, not as a common policy.

**Second: insertion by copy is an already accepted decision, and it must not be weakened silently.** `user_store.rs` declares "the library holds insertion templates, not references", and on this rests the claim "if the library file is lost, the projects are intact". The first tenant that wants references (a template composition, updated centrally) breaks this property for ALL, not just for itself: the user cannot tell which of their presets is a copy and which is a reference.

**Third: the storage has its own schema version, and its migration has two sides.** `PRESETS_VERSION` grows independently of `KT_META_VERSION` (`src/libs/kt-otio/src/meta.rs`), and the rule "a file newer than the program is read-only" means that an old build sees the library as incomplete. For one file of presets this is acceptable; for six it is six independent reasons to show the user an incomplete list, and they will never learn why.

### Which invariants it touches

Root invariants — practically none, and this is a strong argument for "cheap".
**Invariant 11 (determinism)** is touched only indirectly and only with reference insertion: a project whose frame depends on a file in the user's directory stops being reproducible on another machine — therefore insertion by copy is precisely the form of fulfilling the invariant, not a convenience. Invariants 1–10 are not involved: the storage touches neither the audio format, nor the cache, nor decoder ownership, nor clocks, nor export, nor dlopen.

Checklist rules of `src/apps/kt-app/CLAUDE.md`: **16** (a newer or broken file is an ERROR, named aloud, not a silently incomplete list), **19** (the dialog must say where to change the decision — this concerns the future "put inside the project" switch), **20** (the library is not replenished with "recently used" by itself — only by an explicit save; this is exactly how it is written in the verdict of F-AUT-004).

## Estimate

| What | Answer |
|---|---|
| Scope | L — a whole round, but not in code: (1) a rule "what travels with the project" for each tenant — S; (2) the "put inside" switch where it makes sense, plus a dependency report — M; (3) transfer of the storage as a single file and a guard for a second copy — M |
| Where the main work is | kt-app (`user_store.rs` — the shared catalog already exists; tenants and policy are added; the switch dialog), kt-model (the "put inside" unit for those tenants that have one), kt-otio (the envelope key, if the content goes into the project) |
| Project format | does not touch it by itself; touches it exactly in the part where a tenant moves INSIDE the project — then `KT_META_VERSION` grows |
| Tests to write | "the library file is lost — the projects are intact" as an explicit test, not as a comment; "the storage file is newer — the list is incomplete AND this is stated" (rule 16, next to t344b); two copies of the program do not overwrite each other's library (on the model of t362); transfer of the storage as a round-trip; for each tenant — a test answering "travels with the project or not" |
| Depends on | F-PRJ-003 (the verdict requires the "inside the project" switch), F-AUT-004 (the only occupied tenant, R.48). Waiting for a place: F-TXT-002, F-EXP-001, F-UI-002, F-HLP-027, F-TL-012 |
| Breaks compatibility | no; moving a tenant inside the project goes forward only, old files are read as they are |

## Verdict

**Candidate. The note exists not for the sake of a new feature, but so that the next feature that needs a place between projects does not invent it again.** This is conclusion 2 of the first wave, recorded in the catalog README: four features ran into the absence of a storage (F-AUT-004, F-TXT-002, F-EXP-001, F-UI-002); later the start screen with new-project templates (F-HLP-027) was added to them, and in R.59 template compositions (F-TL-012). Six candidates, one occupied.

The place itself has existed since R.48 and is done more correctly than in four of five editors: not a monolith in the version profile (Premiere) and not "partly in the registry" (VEGAS), but a catalog with an atomic write, a schema version, raw passthrough of unknown records, a policy for a broken file, and factory records parsed by the same code. The main decision, however, was made even earlier and is written in the module itself: **insertion by copy, not by reference** — "if the library file is lost, the projects are intact". This is our answer to the trade-off that F-PRJ-003 calls unavoidable: we pay with portability only for what is not in the project, and for nothing more.

The debt, however, is single and named directly: **there is no "put inside the project" switch**, although the verdict of F-PRJ-003 requires it "on the same day". While there is one tenant and insertion by copy, the debt does not fire. It will fire on the first tenant that wants references, and this must be decided BEFORE that tenant moves in, not after, because the user does not distinguish which of their presets is a copy and which is a reference.

## Sources

- https://creativecow.net/forums/thread/when-upgrading-do-presets-saveae/ —
  VEGAS presets are tied to the installation of the version; deleting the old one takes them away
  (John Rofrano, 23.06.2011; access date 2026-09-21)
- https://community.adobe.com/t5/premiere-pro-discussions/effect-presets-disappear-after-restart-cc2015/m-p/8676875
  — imported presets do not survive a restart, thread 2016–2023
  (2026-09-21)
- https://creativecow.net/forums/thread/custom-effects-presets-disappeared/
  — loss of the entire user library of Premiere together with bins
  (14.07.2014; taken from the analysis of F-AUT-004, the link re-checked
  2026-09-21)
- https://www.vegascreativesoftware.info/us/forum/what-do-i-need-to-do-to-not-lose-custom-settings-upon-fresh-reinstall--147179/
  — VEGAS presets lie in different places, part in the registry. The domain
  moved to `forum.borisfx.com` and returns a redirect; the content is taken from a search snippet — **[NOT VERIFIED]** verbatim (2026-09-21)
- Final Cut Pro User Guide 12.3, "Manage Motion content": `Motion Content:
  In Library`, `Consolidate` and the caveat about a third-party FxPlug — from the analysis of
  F-PRJ-003 (the PDF was read locally there), not reopened in this pass
- DaVinci Resolve 21 Reference Manual: PowerBin and PowerGrade as objects of the
  DATABASE, visible from any project of the database — from the manual (chapters on Media Pool and
  Gallery); **[NOT VERIFIED]** live, the line relies on item 1 of the trust order in `docs/features/_editors.md`
- After Effects: `.ffx` in the presets folder and the local folder of Essential Graphics
  templates — from the analysis of F-AUT-004 and Adobe's help
  (`helpx.adobe.com` does not open from this environment; the caveat of the first
  wave in `_editors.md` remains in force)
- Ours: `src/apps/kt-app/src/user_store.rs` (`store_dir`,
  `FxPresetStore`, `PRESETS_VERSION`, `fx-presets.json`, `workspace.json`,
  `.broken`, `readonly`, `KT_USER_STORE_DIR`, factory records via
  `include_str!`), `src/apps/kt-app/src/app.rs` (`autosave_path`,
  `recovery_path_for`), `src/apps/kt-app/src/project_lock.rs`,
  `src/libs/kt-model/src/ops.rs` (`FxChainBlock`),
  `src/libs/kt-model/src/project.rs` (`MediaRef::path`, `UiState`),
  `src/libs/kt-otio/src/meta.rs` (`KT_META_VERSION`),
  `docs/features/README.md` (conclusion 2 of the first wave),
  items 343–347

No foreign editor was run live. The forum quotes were read verbatim on `creativecow.net` and `community.adobe.com`; the Resolve, FCP and AE lines rely on the manuals and on the related notes of the base, and the VEGAS line relies on forum threads and search snippets of a domain that changed hands.
