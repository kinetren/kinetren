# Design

## Context

See proposal.md — Why. Observed (2026-10-28):

- **Packaging files.** `src/packaging/kinetren.desktop` has `GenericName[ru]`, `Comment[ru]` and
  no `Keywords`; `src/packaging/kinetren-mime.xml` has `<comment xml:lang="ru">` for its two types,
  and the project type globs `*.kt` AND `*.ktproj`. Commit `caf1561` (2026-10-23) kept the `ru`
  lines on purpose ("Name[ru]/comment xml:lang=ru in the desktop and MIME files") — so `ru` was a
  leftover of the Russian-first era, not a decision against the other languages. The files are
  copied as they are by `release-build.sh:222-223` and `install.sh:30-37`; the distribution
  guard t834/t835 (`src/apps/kt-cli/tests/dist_guard.rs`, `src/scripts/dist-check.py`) reads the
  desktop file only for `Exec`/`StartupWMClass`.
- **Catalog languages**: `build.rs` lists `en, ru, es, zh-Hans, ar` (`src/apps/kt-app/build.rs:24-28`);
  `Lang::ALL` is the runtime list; t125 (`src/apps/kt-app/src/i18n_tests.rs:27`) demands every key
  be used by its Rust path except the prefixes in `NOT_REFERENCED_BY_PATH = ["lang."]`
  (`i18n_tests.rs:8`). The build REJECTS an ASCII comma, semicolon or question mark in an Arabic
  value (`i18n/README.md`, "What breaks the build").
- **`.kt` vs Kotlin.** `/usr/share/mime/packages/freedesktop.org.xml` declares
  `text/x-kotlin` with `<glob pattern="*.kt"/>` (line 44471 on this machine). A second type with
  the same glob makes shared-mime-info fall back to content sniffing; without a magic rule of ours
  the result for a Kotlin file is ambiguous. The project file starts with `{` and its first key is
  the marker `"kinetren"` (`src/libs/kt-model/src/project_file.rs:50`, `Envelope` field order
  `:127-135`, `to_string_pretty`), so `"kinetren"` lies within the first ~16 bytes.
- **`.ktproj`**: R.70/project-format — import only, "two formats under one suffix is a trap"
  (`openspec/specs/project-format/design.md:85`); "Open" (menu, launch with a file, a drop)
  rejects it naming the import door (ui-reach spec, "Project file doors"; `app.rs:7322`).
- **«Лента».** `grep` over `src/` and `src/fixtures/` finds «Лента» only in `i18n/ru.toml:856`
  (`msg.nest_default_name`). Production code always took the default reel name from the catalog
  (`app.rs:3118` collapse, `app.rs:3256`), never from a literal; t131
  (`i18n_tests.rs:206`) holds that. The head of a reel instance's strip prints the reel's NAME
  (ui-timeline "Head of the chain strip of a reel instance", t793). The collapse gesture does
  NOT ask for a name (the `seq_name` window opens only from "Rename the reel…",
  `timeline.rs:5903`, `bin_panel.rs:1147`), so «Лента» was written into the owner's file by the
  program in the Russian interface and carried into English. **Assumption, not verified:** the
  owner's project with that reel is not available here (runs use our fixtures, R.67); the first
  task reproduces it on our fixture.
- A related leftover found on the way: `unique_name(.., "copy")` passes the English stem `"copy"`
  for an empty wanted name (`app.rs:3118` and others) — never reached for reels (the dialog rejects
  an empty name and the collapse passes the catalog default), so not a defect; noted, not changed.

## Goals / Non-Goals

**Goals:**
- Every catalog language in the desktop and MIME files, from one source, guarded.
- The MIME file names our files and only ours.
- The owner's «Лента» shown as "Reel" in English, including in his existing file, with no format change.
- A guard that a Cyrillic word cannot hide in another catalog or a committed fixture.

**Non-Goals:**
- Translating names the person typed (reel, track, marker, area names; published parameter names —
  kt-model CLAUDE.md: the published name is "FREE TEXT of the author… it will never get into the
  translation catalogue"). Unchanged.
- Windows: there is no desktop entry; the Windows installer-free zip has no file associations.
- Flatpak's `.desktop`/metainfo (change `flatpak-flathub`) — it will consume the same files.

## Decisions

### D1. Packaging texts live in the catalogs, the files are kept equal by a guard

A `[desktop]` section in the five catalogs: `generic_name`, `comment`, `keywords` (SPACE-separated,
like `[fxreg.kw]`, because an Arabic value may not hold an ASCII `;`), `mime_project`,
`mime_otio`, `mime_legacy`. t125 exempts the `desktop.` prefix (as `lang.`); t930 is the user of the
section: it reads the build tables, maps each language to its freedesktop locale (`zh-Hans` →
`zh_CN`), joins keywords with `;` plus a trailing `;`, and compares with the two files. The files
are edited by hand — six lines per language, and a generator would be the second writer of
committed files that rule 20 of the checklist forbids growing by themselves. Alternative: a
script that rewrites the files (`--write`) — rejected for the same reason; the guard's message prints
the exact line to paste. Alternative: no catalog section, the translations only in the packaging
files — rejected, a translator works in the catalogs and would never see these strings.
The Arabic values stay in logical order (the desktop environment shapes them); only egui needs
the reordered table, and t930 reads the logical one (`AR_LOGICAL`).

### D2. `*.kt` with a magic rule; `.ktproj` its own type

```xml
<mime-type type="application/x-kinetren-project">
  <glob pattern="*.kt"/>
  <magic priority="80"><match type="string" value="&quot;kinetren&quot;" offset="0:64"/></magic>
```
plus `application/x-kinotrain-legacy-project` for `*.ktproj` (comment "Older Kinotrain project",
translated), listed in `MimeType=` so a double-click starts Kinetren, whose "Open" names the import
door — the same outcome as a dropped `.ktproj` today. Alternative: drop `*.ktproj` and leave the
files unowned — rejected, the file manager would show "unknown", and the owner's old projects are
exactly the files a person double-clicks. Alternative: route a launched `.ktproj` straight into the
import — rejected, it contradicts R.67 (the owner's decision: "Open" rejects old forms, the import
is its own door) and the ui-reach requirement that launch-with-a-file is "Open".

### D3. A program-chosen reel name is recognised by its text, shown in the current language

`reel_display_name(stored) -> Cow<str>`: if `stored` equals `nest_default_name` of ANY of the five
catalogs, optionally followed by `" " + digits`, return the current language's default plus the same
suffix; else return `stored` borrowed. Every place that shows a reel name calls it (proposal,
Impact); a lookup by name (search, `unique_name`'s taken list for a rename) checks both forms.
Alternatives:
- *A flag `Sequence.name_auto`* set at collapse, cleared by rename — rejected: a project-format
  field for a display concern, and it does not fix the owner's EXISTING file (the flag is absent
  there); teaching the reader to set it for old names would be the reader learning an old form
  (R.67 rule: fixes go into the import, not the reader).
- *Store no name (empty) and show the default* — rejected: names are unique addresses in the
  model (`unique_name`, the rename check) and an empty name breaks them; export to OTIO needs a name.
- *Leave it as data* (the industry norm — Premiere names a nest in the language of the moment) —
  rejected for this case: the person never typed the word; the program wrote it silently.
The cost, written down: a person who deliberately typed exactly "Reel" or "Reel 3" sees it in the
other language too (Open Question 1).

### D4. Cyrillic placement guard

t933 extends t131's reach beyond literals: catalogs other than `ru.toml` value by value (the
`[lang]` endonym excepted), and committed fixtures as text with a declared list of Cyrillic test
data and a reason per entry (the title goldens, the kt-text pangram, the README Cyrillic guard —
the exceptions `caf1561` named). The `.desktop`/MIME files are not in it: their `[ru]` lines are
checked against `ru.toml` by t930.

## Risks / Trade-offs

- [A future default name in a sixth language collides with a real word a person types] → the match
  is exact (whole name, optional number); the cost is display-only and documented.
- [`update-mime-database` absent on a developer machine makes t935 print instead of check] → the
  build container installs `shared-mime-info`, and the CI job that runs `cargo test` gets it too
  (task 2.4 checks which environment the CI workflow tests in), so the check runs on
  every push even if a laptop skips it.
- [A magic rule on a pretty-printed JSON depends on the writer's key order] → the order is part of
  the format already (`Envelope`: "the version first, so that an unknown one is rejected without
  reading the rest"); t935 writes the file through `to_text` itself, so a reorder turns it red.

## Migration Plan

Installed systems pick up the new MIME file on the next `make install`/`install.sh` run
(`update-mime-database` is already called there if present — task 2.3 checks it). No project
data changes. Rollback: revert; the stored names were never touched.

## Open Questions

1. Should a reel the person deliberately named exactly "Reel"/"Лента"/"Bobina" (the default word
   in any language) also follow the interface language? Recommended answer: yes — telling the two
   apart needs a stored flag (rejected in D3), and a person who wants a fixed name types any other
   word.
2. English keywords for the menu search: recommended `video;editor;editing;timeline;nle;montage;cut;`
   — the owner may prefer a shorter list.
