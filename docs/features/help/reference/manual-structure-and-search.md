---
id: F-HLP-023
title: User manual: how it is built and how one finds things in it
status: candidate
priority: B
cost: L
category: help/reference
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2]
we_have: no
crates: [kt-app, kt-cli]
invariants: []
ui_rules: [18, 20]
round: null
point: null
tests: []
depends_on: [F-HLP-032, F-HLP-033, F-HLP-019]
checked: 2026-09-07
---

# F-HLP-023 — User manual: how it is built and how one finds things in it

## Essence

The manual is not a text but a set of addressable topics, which are read ONE AT A TIME. The editor comes there with one of two different questions — "what does this checkbox do" and "how to edit a video" — and gets into one of two different books: a reference on elements and a tutorial on tasks. Inside the book he searches: by the table of contents, by the index or by search. Together these are four decisions (what kind of book, how it is cut, what the address of a topic is, what it is searched by), and products accept them differently. We have neither a book nor addresses: there is a landing page, where pieces of both lie mixed on one page.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Online Help | 492 topics in 14 numbered sections (`0-gettingstarted` … `13-appendix`), MadCap Flare; search is a stemmed word index in `Data_Search.js`, there is no alphabetical index; "see also" by hand ("For further information"); **652 context addresses in `Alias.xml`** lead to 266 of the 492 topics |
| Adobe Premiere | 26.x | Premiere User Guide | one live web version on helpx, the product version is not indicated on the page (quote — F-HLP-032); there is no offline edition |
| DaVinci Resolve | 21 | Reference Manual | **one PDF: 4444 pages, 187 MB, 203 chapters, 15 groups**; search is delegated to the reader (their own words); the index is not alphabetical but a MENU: pages 4429–4443, "item → line → page number" |
| Final Cut Pro | 12.3 | Final Cut Pro User Guide | 500 topic pages, web; **71% of the headings are verb phrases** ("Add titles", "Adjust volume") + 58 pages "Intro to …"; the address carries both the stable id of the topic and the version: `/guide/final-cut-pro/<slug>-ver<9 hex>/12.3/mac` |
| Adobe After Effects | 26.x | After Effects User Guide | the structure is the same as Premiere: one live version, an address without a version |

None of the five has a search that understands the question. Those who cope do NOT cope by search: Apple names topics as tasks, so the query "how do I add a title" lands in a heading; Vegas and Blender keep a hand-written layer ("For further information", `genindex`); Blackmagic honestly sends the user to the Acrobat search.

**Outside the core.** The mechanism "the program itself computes the address of the page of its manual and knows where the manual has a hole" is not described by any of the five in anything that can be read; in Blender it lies in an open file, therefore without this row the trick cannot be described.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | Blender Manual | docs-as-code, ~2350 pages in the navigation, divided into "Getting Started" (4 topics) and "Sections" (17 sections) + Glossary + a hand-written "Manual Index" (`genindex`); `F1` over any button opens its page by the table `scripts/modules/_rna_manual_reference.py` — **4668 pairs "property → URL#anchor", generated automatically**; the URL prefix is assembled at runtime from the LANGUAGE and the VERSION of the program itself |

## The trick behind it

The payload which one status-bar line cannot lift physically is **ORDER and ADDRESS**. A manual topic knows what is read before it and what after, and a link to it can be given, passed to another person and opened tomorrow. A status-bar line under the cursor has no neighbors and no name: it cannot be opened, forwarded, bookmarked or quoted in a letter. For us this payload will not go into the binary (there is no surface for it there and, by invariant 12, there should not be), but into `site/` — into the address of a topic; and the whole note is about the fact that the address has to be created BEFORE the text.

**1. The index of the largest manual of the industry is not alphabetical but a MENU.**
The Reference Manual of Resolve 21 ends with fifteen pages (4429–4443), where all menu items of the program are listed: the item, one line of what it does, the page number. There is no alphabetical index in the manual at all — checked by the table of contents and the last pages of the file. The bet is made on the fact that the user's question has the form not of a word but of a PATH IN THE MENU — exactly what in our UI checklist is written as rule 18 ("the menu item says WHAT and WHERE"). The book has no search of its own: page 10 is titled "Why Is This Manual So Big?" and refers the reader to the "search functionality of your preferred PDF browser". The largest documentation of the industry searches exactly as our landing page does today — by means of the reader.

**2. The address of a page is computed by the program itself, therefore an old build reads ITS OWN manual.** In Blender `url_manual_prefix` is assembled from the language of the interface and `bpy.app.version[:2]`, and the table of 4668 pairs "property → page#anchor" is generated by a script, not written by hand. Two consequences follow, and both are useful to us: an update of the online help does not spoil life for those who sit on the previous version (their `F1` goes to `manual/<language>/<version>/`), and coverage becomes a measurable quantity — the manual explicitly warns that 100% of it does not exist, and an uncovered button reports this in the interface, rather than silently opening the general page. The hole is named, not hidden; exactly as our `--check` names the lag of `dist/`.

**3. Half of the Vegas help has no door from the program, and one can see where the doors are placed.** `Alias.xml` (652 records `IDH_*`) covers 266 topics of 492 — 54%. The distribution: 215 addresses lead to `7-edit`, 177 to `8-design`, 93 to `2-window`, and only 11 to `10-export`. The doors are cut where there is a DIALOG with a help button, and not where the person gets stuck: export, where people err most often, is almost not covered. This is an argument against "we'll make context help and close the topic with that": context help arrives where there is a window, not where there is incomprehension.

## Pros

- **A task in the heading works better than any search.** 71% of Apple's topics start with a verb, therefore the word index hits the question "how to do X" without any understanding of the question. This is a structural decision, not a search one, and it costs only discipline of naming.
- **The version in the address saves old builds**: `/12.3/mac` at Apple, `/2026/en/` at Vegas, `manual/<language>/<version>/` at Blender. The help is updated — the previous release goes on reading its own.
- **A PDF gives a full offline copy in one file.** Of the five this exists only at Resolve, and it is the only one of the five helps that works in the field without a network.
- **Docs-as-code turns the reader into a proofreader.** Each Blender page has at the bottom "View Source", "View Translation" and "Report issue on this page" with the file path, version and language already substituted.
- **A manual cut into topics is split up and re-published by third parties** (`ltbits.github.io/davinci-resolve-manuals` — the same PDFs laid out in parts). The demand for structure is visible from the fact that people build it by hand.

## Cons

- **A reference is written from the code, a tutorial from experience, therefore there is always more of the first.** At Resolve 203 chapters are named by a tool and a page ("Using the Cut Page"), not by a task; at Apple, on the contrary, 500 tasks and almost no parametric reference. A complete set exists at nobody — and this is not laziness but a different cost of production.
- **Search indexes the words of the PAGE, not the words of the user.** The Flare index of Vegas stores the stemmed roots of the text; the query "remove green background" will not find "Chroma Keyer" until the author adds a synonym by hand. The hand-written layer (index, keywords, "see also") is the only known remedy, and it does not scale.
- **An online help without a version in the address misleads users of old builds.** It is exactly this that people complain about in the Adobe community (quote — F-HLP-032).
- **An offline manual is a file on disk that the menu opens by path.** It can be NOT THAT file: in 2021 the item Help → Reference Manual in Resolve 17 opened a 200-page "What's New", while placing the full manual in the same folder under the same name was suggested.
- **187 MB in one file is the price of completeness.** It opens slowly, searches slowly on a weak machine, and cannot be downloaded in parts.
- **Splitting breaks Ctrl+F.** While the help is one page, the browser's search is ideal and free; on N pages it stops working, and one has to make an index of one's own. No one writes about this price of splitting.

## What users say

> What I get from the HELP menu from within the app is a 200 page long pdf
> document.
> — member of the Blackmagic forum, 20.02.2021,
> https://forum.blackmagicdesign.com/viewtopic.php?p=729719

> You could put a full 16.2 manual in the folder, re-name it and it would open
> via the menu.
> — John Holt, ibid., 20.02.2021

Both replies are about one property of the offline help: the menu item does not "open the manual", it opens a FILE BY PATH, and nothing checks that it is that file and that version. Separately, on the question of completeness versus structure: a third party downloaded the official PDFs and split them into parts (https://ltbits.github.io/davinci-resolve-manuals/, accessed 2026-09-07), because reading 187 MB whole is impossible.

## How it fits Kinotrain

### What is already there

- **The documentation pipeline as a whole** — F-HLP-032, not reopened: `site/build.py` (930 lines), five catalogs of 220 values each, 17 sections, `--check` with a sha256 comparison, `site/screenshots.py`, which drives the real `src/target/debug/kt-app` through `src/scripts/ui-probe.py`.
- **Pieces of the manual, already written and already typeset** (`site/content/en.toml`): `[keys]` — 30 chords in 4 groups plus an introductory paragraph that explains the model of hints itself; `[care]` — five topics about autosave, project readability, cache and warnings in human language; `[fxdesc]` — 10 descriptions of effects; plus `[morpher]`, `[bin]`, `[export]`, where the prose is already explanatory rather than promotional.
- **A ready teaching edit**: `kt-cli demo` (`src/apps/kt-cli/src/main.rs`) — six named tracks, markers, a region with loop, an FX with automation, a morpher, groups of the bin. Any example of the manual can already be shot on it.
- **The synonym mechanism already works for us** — `[fxreg.kw]` in `src/apps/kt-app/i18n/en.toml`: 12 keys of invisible search words (`greenscreen = "chroma key green transparency"`). That is the hand-written layer which the search of Vegas lacks; in the effects picker it exists, in the help it does not, because there is no help. Search across the program — F-HLP-019 and F-HLP-015; here it is only noted that a mechanism does not have to be made from scratch.
- Search by the CONTENT of the project — `search_hits` in `src/apps/kt-app/src/app.rs:4543` (markers, media, tracks, clips, groups); it has no relation to the help (F-MED-001).

### What is missing

1. **Addresses.** In the assembled page there are 11 sections with an `id` (`#keys`, `#care`, `#export`…) and 31 `h3` headings WITHOUT an `id` — checked in `site/dist/en/index.html`. The most precise address that can be given to a person today is "a section of the landing page", not "a topic". Everything else (a link from the program, "see also", the index, search, an offline copy) hangs on an address and is not set up without one.
2. **Languages are not five but two.** Checked by grep across `site/content/*.toml`: `[fxdesc]`, `[care]` and `[keys]` in `es`, `zh-Hans` and `ar` — English text in a placeholder; only `en` and `ru` are translated (the same 0.0% coverage that the build counts, and `noindex`). The formulation "the effect descriptions are translated into five languages" is not confirmed by the reconnaissance: two languages out of five are paid for. The price of the additional translation is F-HLP-033, not recounted here; what matters is that for the pages of the site there is NO limit of 160 characters, that is, the text of the manual on the site does not pay the catalog tax of the application at all.
3. **Register.** The landing page is written in a selling tone ("It looks after your work"); a tutorial requires a different voice, and mixing the two in one catalog is not allowed — otherwise the first edit of the manual will spoil the sales page.
4. **There is no door from the program at all.** By grep across `src/apps/kt-app/src`: no `xdg-open`, no opening of a URL, not one line `http://` — the program today physically cannot send a person to a page. The first link outward will be the FIRST in the project, and that is a separate decision, not this one.
5. **Offline copies.** `site/dist` weighs 2.0 MB in total (screenshots 1.2 MB, font subsets 72 KB, five `index.html` of 32–40 KB each), and the tarball (`src/scripts/release-build.sh`) carries only the two binaries, `.desktop`, mime, icons and `install.sh`. A full help in five languages costs 2 MB in the delivery — against 187 MB for Resolve.

### What gets in the way

- **One page type.** `build_page()` builds exactly one page per language from the 12 declared `[[section]]` in `site/content/structure.toml`. A second page type (a manual topic with its own slug) is work inside `build.py`, not a line of config: routing, a table of contents, "previous/next" and its own set of checks are needed.
- **Splitting breaks the only search that works today.** While the page is one, the browser's Ctrl+F is a full search of the whole help for free. As soon as there are forty topics, a client-side index will have to be made: JS weight, five languages, Arabic morphology and stemming, which we do not have. This is the main structural point of the note, and it is not technical but a matter of size: **up to some volume the manual is CHEAPER to keep as one page**, and our ~65 named commands (20 items of the File/Edit/View menu, 38 keys of the timeline context menu, 9 in the bin) — this is the order of "tens of topics", not "hundreds".
- The discipline of synchronizing the text with screenshots and the names of menu items — already described and already has machinery (F-HLP-032), not recounted here.

### Which invariants it touches

As long as we are talking about `site/` — **none of 1–12**: the assembly lives outside the application process and does not get into the binary. This is the main argument for "cheap". Invariant 12 (repaint only on motion, the UI does not heat the system) would forbid an animated tour inside the program, but it does not hinder the manual on the site and, on the contrary, explains why the manual must be OUTSIDE. From the UI checklist two are touched: **18** (the menu item says WHAT and WHERE) — this is the form of our future index, the cheapest and most honest, because the list of commands already exists in the catalogs `[menu.*]`, `[tl.menu]`, `[bin.menu]`; and **20** (nothing grows on its own) — the table of contents and the index must be assembled by the build from the declared sources or written by hand, but not "added on occasion". Rule 19 (a dialog says where to return) and the price of the application catalogs (F-HLP-033) will be switched on only on the day when a door from the program appears — and that is no longer this note.

## Estimate

| What | Answer |
|---|---|
| Volume | L overall; of it M is machinery (a second page type, slugs, the table of contents, keywords), the rest is writing the topics and translation |
| Where the main work is | `site/build.py` (the second page type and its checks), a new root `site/content/manual/`, `src/scripts/release-build.sh` (an offline copy into the tarball) |
| Project format | does not touch it |
| Tests that will have to be written | no Rust tests are required while there is no door from the program; clauses of `site/build.py --check`: the slug of a topic is stable and does not vanish, a topic has a set of all five catalogs, a topic has a non-empty list of search words, the table of contents is assembled from the declared topics and only from them |
| Depends on | F-HLP-032 (the pipeline and synchronization), F-HLP-033 (the price of a row — does not apply to the site, there is no limit of 160 characters there), F-HLP-019 (search words as a mechanism) |
| Breaks compatibility | no |

A frame, not a specification: the set of topics, the order and the voice are the work of the round, not of the note.

## Verdict

Grow it from the site, but NOT inside the landing page. A separate tool (Sphinx, mkdocs) would have to be taught again everything that `site/build.py` already does: key parity across five catalogs, the translation metric with automatic `noindex`, the whitelist of markup, font subsets, Arabic punctuation, a reproducible build and the check of `dist/` by sha256. This is more expensive than adding a second page type to the existing build. But topics must not be added to `index.html`: the landing page sells, while the manual teaches, and they have a different voice and a different lifespan.

First what is set up is not the text but the ADDRESS: while a topic has no stable slug, the manual cannot be linked by "see also", opened from the program, placed offline, or told apart from advertising. And up to a few dozen topics the search remains the browser's Ctrl+F — deliberately, because structure matters more than completeness: 4444 pages are read one at a time, not in sequence.

CHECK: site/content/manual/ — a clause of the build "a topic has a stable slug and the address /<language>/manual/<slug>/, a set of all five catalogs and a non-empty list of search words; a vanished slug is a build error, not a 404, because it is already referenced by an installed build".

## Sources

- `site/build.py`, `site/README.md`, `site/content/{structure,en,ru,es,zh-Hans,ar}.toml`,
  `site/dist/*/index.html`, `src/scripts/release-build.sh`,
  `src/apps/kt-app/i18n/en.toml`, `src/apps/kt-app/src/app.rs`,
  `src/apps/kt-cli/src/main.rs` — read 2026-09-07; sizes and anchors
  computed from the working tree
- DaVinci Resolve 21 Reference Manual, PDF:
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf
  — the file was downloaded and parsed 2026-09-07 (`pdfinfo`: 4444 pages, 187 277 990
  bytes, A4, InDesign 21.2, 09.07.2026; chapters 1–203; "Menu Descriptions"
  4429–4443; page 10 — "Why Is This Manual So Big?"). The press names
  4443 pages (F-HLP-032 cites ProVideo Coalition) — a discrepancy of one sheet of the colophon, not an error
- VEGAS Pro 2026 Online Help:
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/default.htm
  — the exported MadCap Flare files were parsed (`Alias.xml` — 652 records and 266
  unique topics, `Data_Search.js` with stemming, `Vegas_TOC*.js`, 492 topics, the page "Keyboard shortcuts" — 15 tables and ~330 rows), accessed
  2026-09-07
- Final Cut Pro User Guide 12.3: https://support.apple.com/guide/final-cut-pro/toc
  — the table of contents was exported 2026-09-07, 500 unique topics were counted and the share of verb headings (71%, a script over the slugs)
- Blender 5.2 Manual: https://docs.blender.org/manual/en/latest/getting_started/help.html
  (through the text proxy `r.jina.ai`, accessed 2026-09-07) and
  `scripts/modules/_rna_manual_reference.py` from the sources of Blender 5.2 —
  4668 pairs "property → page". **There was no live check** (the rule of
  `_editors.md` for software outside the core): the pages were read as text, the program was not run
- The Blackmagic forum, a thread about access to the manual from the Help menu, 20.02.2021:
  https://forum.blackmagicdesign.com/viewtopic.php?p=729719 (through `r.jina.ai`)
- A third-party breakdown of the manuals into parts:
  https://ltbits.github.io/davinci-resolve-manuals/ (accessed 2026-09-07)
- Premiere and After Effects: the rows are taken from F-HLP-032; `helpx.adobe.com` from
  this environment still does not open directly; they were not checked again
