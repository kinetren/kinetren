---
id: F-HLP-032
title: Documentation as code and automatic screenshot re-shooting
status: already-have
priority: B
cost: S
category: help/authoring
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2, vscode-1.136]
we_have: yes
crates: [kt-app, kt-cli]
invariants: []
ui_rules: []
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-HLP-032 — Documentation as code and automatic screenshot re-shooting

## Essence

The help is assembled from versioned sources by the same machinery as the program: the prose lies in text files next to the code, the pictures are taken by a script that drives the REAL window of the program, and the build fails if anything has diverged. The reader gets for this a page in their own language, where the snapshot of the interface shows the program that is running for them, and not the one that was two releases ago.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Online Help | HTML on the servers of Boris FX; the version AND the language are in the path (`/Documentation/vegas/2026/en/`), that is, addressing like ours; the sources are closed, there is no PDF edition for the 2026 line |
| Adobe Premiere | 26.x | Premiere User Guide | one live online version on helpx; the page does not say which version it refers to; there is no offline edition; the source is closed |
| DaVinci Resolve | 21 | Reference Manual | one PDF of 4443 pages, multilingual, versioned with the release, downloaded from the site and opened from the Help menu — the only one of the five that gives full help offline |
| Final Cut Pro | 12.3 | Final Cut Pro User Guide | a live web guide on support.apple.com; the edition in Apple Books is marked as no longer updated, the web version is canonical; the source is closed |
| Adobe After Effects | 26.x | After Effects User Guide | the same structure as Premiere: helpx, one live version, the source is closed |

Docs-as-code (help as code in a repository) — **none of the five has it.**
In no case can the reader send a correction, build the help themselves, or learn which build the picture on the page belongs to.

**Outside the core.** The mechanism was born not in video editing: none of the five NLEs builds its help from sources, therefore to explain the device in five lines of the core is impossible — both lines below are needed as the ORIGIN of the mechanism.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | Blender Manual | reStructuredText + Sphinx in an open git (`projects.blender.org/blender/blender-manual`); translations — `.po` files from the community through Sphinx's standard i18n; published at an address with the language AND the version (`docs.blender.org/manual/<language>/<version>/`); from the program the context menu "Online Manual" opens the page of EXACTLY the element under the cursor |
| Visual Studio Code | 1.136 | vscode-docs | Markdown on public GitHub, edits come as external PRs; release notes are a separate file per version (`release-notes/v1_104.md`), that is, the history of changes is versioned, not the guide itself |

## The trick behind it

The cargo that one status-bar line physically cannot hold is here —
**the PICTURE: a snapshot of one's own interface, taken in the reader's language**. A line can say "región guardada"; it cannot show, at any number of characters, where this area lies on the screen and what its handle looks like. With us this cargo goes not into the program but onto the page `site/dist/<language>/`, and this is a deliberate fork: the snapshot is available before installation and costs not a byte in the binary, but it is unavailable to the one who is already inside the program and got lost.

Three details that are absent from the description of any pipeline.

**1. An automatic snapshot cannot lie about the interface — but only while the layout is stable, and someone must pay for this stability.**
`site/screenshots.py` drives the real window at FIXED coordinates (`CLIP_VIDEO = (660, 772)`, `FX_ADD_BUTTON = (65, 82)`), with one set for all five languages. The legitimacy of one set is held by the layout test t127: the limits of the left column (420 px) and of the meter panel are the same in all five catalogues, because they were set by the direct requests of the owner and do not move. This bond is rare: usually a layout test exists for the sake of the layout, but here it LEGALISES the shooting pipeline — remove t127, and the coordinates become guesswork, and silently so.

**2. The lag of a translation is a measurable quantity with an automatic consequence, not a promise of "we will translate someday".** The build counts the share of values that differ from the English ones; a language below `translated_min` (0.9) is built, but receives `noindex` and drops out of `sitemap.xml` and `hreflang`. Today: `en` 100 %, `ru` 98.1 %, `es`/`zh-Hans`/`ar` — 0.0 % and `noindex`. The reverse side of the device: the metric measures NON-coincidence, therefore correctly untranslated terms (`Kinotrain`, `FFmpeg`, effect names) count as untranslated — those very 1.9 % that Russian lacks up to a hundred.

**3. A check that breaks the build for the lag of the artefact behind the sources.** `site/build.py --check` builds the site into a temporary directory and compares the sha256 of all files with the committed `dist/`. For this the build had to be made byte-reproducible: `SOURCE_DATE_EPOCH` is fixed, otherwise fontTools writes the build time into the font, the hash in the file name changes, and behind it the CSS and all the pages. This is exactly the discipline that the engine calls invariant 11, but applied to documentation.

## Pros

- **The translator has nothing to break in the layout.** In values a whitelist is allowed: `<em> <strong> <kbd> <code> <br>`; everything else is escaped; an unpaired tag and a foreign tag are a build error, not a crooked page.
- **A key is missing or extra — the build fails**, exactly as for the application in `src/apps/kt-app/build.rs`. The key set is defined by `en.toml`; 17 sections, 220 keys, five catalogues, and all five must match in composition.
- **The web analogue of t100 is built in:** a character that is absent from every assembled font subset is an error; Arabic ASCII comma, semicolon and question mark are also errors, by the same rule as in the application.
- **Shooting per language, not in English.** `/es/` shows the Spanish window, because `KT_LANG` is read before the application's Storage. This was a direct request of the owner (R.22: "and why are the pictures everywhere in Russian?").
- **Every frame — from its own launch of the application.** A whole class of errors "the morpher window did not close and covered the File menu" is eliminated by construction, not by attentiveness: the state does not carry over between frames at all.
- **The cost of truth is low:** re-shooting all five languages is one command and about fifteen minutes, not a human-week of manual screenshots ×5.

## Cons

- **Fixed coordinates break silently.** t127 holds the LIMITS of the columns, but not the position of a clip on the timeline and not the offset of a context-menu item (`FX_MENU_ITEM_DY = 97`). If an item shifts, it is not a test that fails, but you get a beautiful snapshot of the wrong window.
- **A heavy environment.** It needs X11, a built `src/target/debug/kt-app`, `python3-xlib`, ImageMagick and `ffmpeg` for the synthetic sources. This does not run "in passing" in any CI.
- **The translation metric measures the coincidence of strings, not the meaning** — see 1.9 % for Russian.
- **`dist/` is committed**, so it diverges from the sources exactly until the next launch of the build; the check catches this, but only if it is called.
- **The checks cover the text and do not cover the pictures** — nobody checks the size, the presence of the set, or the content of the frame.

## What users say

> The online User Guide for V23.2 contains incomplete, incorrect, misleading
> entries as well as basic writing errors. There is still no way for users know
> which version of Premiere the pages of the User Guide apply to.
> — Remote Index, 17.02.2023, https://community.adobe.com/feature-requests-730/address-user-guide-and-known-issues-documentation-errors-1329644

The opposite pole is Resolve, where the help is versioned with the release and lies in one file:

> I'm talking abou the mammoth, 4443 page DaVinci Resolve 21 Manual.
> — Scott Simmons, ProVideo Coalition, 22.07.2026, https://www.provideocoalition.com/a-noteworthy-davinci-resolve-21-event/

Both quotes are about the same property seen from two sides: the reader needs to know to which version what they are reading refers. A closed online help does not give this answer; a PDF per release does.

## How it fits Kinotrain
CAPABILITY: openspec/specs/ui-reach/ — the contract in `spec.md`, the "why" and the paths in the code in `design.md`.
Already there: help = code (the i18n catalogues, `features-index.py --check`), screenshots — the live scenario `ui-probe.py`/`live-check.sh`.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage of the round |
| Where the main work is | `site/build.py` (two clauses), `src/scripts/finish.sh` (the call), `src/apps/kt-cli/src/main.rs` (the line of the command list) |
| Project format | not touched |
| Tests to write | no Rust test is needed: the clauses inside `site/build.py --check` — the declared size against the real file and the presence of the screenshot set for a language; t127 already exists and must not be touched |
| Depends on | nothing; F-HLP-033 (the price of a translation line) does not apply to the site — the landing values have no limit of 160 characters |
| Breaks compatibility | no |

## Verdict

There is nothing to build: the pipeline is built and by its design stronger than that of any of the five core editors — the sources are open, the translation lag is measured and has an automatic consequence, the screenshots are taken by the program itself. Two clauses of the check and one call at the end of the work must be finished.

ALREADY-HAVE: site/screenshots.py — the pipeline is built but stopped, and the reconnaissance changed the wording thus: "we have no documentation" is inexact; we have the PRODUCTION of documentation without the documentation itself.

The full clarification: the production is assembled (the build, the checks, five languages, the automatic shooting of the real window); there is no content for the user in it except the landing page, and the production itself is now stopped — `dist/` lags behind the sources by two commits; `--check` catches this, but it is called neither by `src/scripts/finish.sh` nor by `src/scripts/live-check.sh`, therefore the page in all five languages promises the `.ktproj` extension, cancelled in R.24.

## Sources

- `site/build.py`, `site/screenshots.py`, `site/README.md`,
  `site/content/*.toml`, `site/assets/kt.css` — read 2026-09-07
- `python3 site/build.py --check` run 2026-09-07: red, all five `index.html` diverge; the coverage of the translation was counted by the same code
- VEGAS Pro 2026 online help: https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/default.htm (access date 2026-09-07)
- Premiere User Guide, the appeal about versioning: https://community.adobe.com/feature-requests-730/address-user-guide-and-known-issues-documentation-errors-1329644 (access date 2026-09-07)
- DaVinci Resolve 21 Reference Manual: https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf and https://www.provideocoalition.com/a-noteworthy-davinci-resolve-21-event/ (access date 2026-09-07)
- Final Cut Pro User Guide: https://support.apple.com/guide/final-cut-pro/welcome/mac and https://support.apple.com/en-us/102917 (access date 2026-09-07)
- Blender Manual, sources and translations: https://projects.blender.org/blender/blender-manual (access date 2026-09-07). **No live check was made** (the rule of `_editors.md` for software outside the core); `docs.blender.org` returned HTTP 403 from this environment, therefore the versioned address of the form `manual/<language>/<version>/` was taken from Blender's documentation on translations, and was not verified by a request. [NOT VERIFIED] the offer of an offline edition of the Blender manual as a separate archive
- vscode-docs: https://github.com/microsoft/vscode-docs/tree/main/release-notes (access date 2026-09-07). No live check was made
