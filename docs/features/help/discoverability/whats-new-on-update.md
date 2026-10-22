---
id: F-HLP-021
title: "What's new" on update
status: candidate
priority: B
cost: M
category: help/discoverability
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136, jetbrains-2026.2, blender-5.2, photoshop-27]
we_have: no
crates: [kt-app, kt-version]
invariants: [12]
ui_rules: [9, 13, 17, 18, 19, 20]
round: null
point: null
tests: []
depends_on: [F-HLP-013, F-HLP-023, F-HLP-026, F-HLP-032, F-HLP-033]
checked: 2026-09-07
---

# F-HLP-021 — "What's new" on update

## Essence

A list of what has changed since the build the person used yesterday: the new separately, the fixed separately, and each item has a version and a date. It answers two different questions: "is it worth installing this update" (the list of fixes) and "what can be done now" (the list of novelties). The only channel that reaches the user, who has ALREADY learned to work, is this one: they no longer read hints, because they have found everything they need — and therefore they do not learn that something new has appeared.

## Who has it and how

| Editor | Version | How it is named | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 (build 143+) | Release Announcements on the forum + `readme\Vegas_readme.htm` in the install directory + the marketing "What's New" | In the program itself — NOTHING: the update arrives through a separate application, Boris FX Hub; after Vegas starts there is no notification, and users directly ask for one (the thread of build 143). Release notes are scattered across four places, therefore the history of releases was assembled BY HAND by a moderator in a separate thread; in it the structure is visible: each build has two sections — "New features" and "Bug fixes and changes" |
| Adobe Premiere | 26.x | "What's new" by version on helpx + a separate page of fixes | [NOT VERIFIED]: `helpx.adobe.com` does not open from this environment either directly (timeout) or through the text proxy (HTTP/2 framing error), including the `/in/` mirror. Adobe's scheme is confirmed on a neighboring product of the same platform — Photoshop, second table: the announcement of a version in the community, where "What's New" and "Bug fixes" are DIFFERENT sections of one post |
| DaVinci Resolve | 21 (the 20.x line verified) | "New Features Guide" — a separate PDF for each point release + a video "What's New in …" | Not a page but a DOCUMENT per release: "This New Features Guide provides a focused and comprehensive look at **only the new features** in DaVinci Resolve 20.2" — that is, the novelties are deliberately taken out of the general manual and live in a separate file with a date. It is downloaded from the support site; the behavior of the program itself at first launch after the update — [NOT VERIFIED] |
| Final Cut Pro | 12.3 | "What's new" — a CHAPTER of the user guide | A versioned address (`…/whats-new-ver19637ebab/12.3/mac/15.6`), seven items, and each has a tail "See <link to the chapter of the guide where this is described>". The changelog is built like a table of contents of the manual, not as an independent text. There are no fixes at all: Apple does not publish a list of what was repaired for FCP |
| Adobe After Effects | 26.x | as for Premiere — one help platform per line | [NOT VERIFIED] for the same reason: helpx is unavailable |

The five rows of the core describe what the vendor PUBLISHES, and almost nothing about what the program does. About the behavior at first launch after an update, the five core editors yielded exactly one checkable detail, and that one is negative (Vegas: does nothing; complaints are in "What users say"). Therefore the second table here is not an addition but half of the subject.

**Outside the core.** The mechanism "the program itself showed what has changed" does not come from video editing: for the five NLEs it is a page on the vendor's website, while the device — a display gate, a switch, a paired menu item — exists only in code editors and in Photoshop. Both details of "The trick behind it" are taken from here.

| Program | Version | How it is named | How it works |
|---|---|---|---|
| Visual Studio Code | 1.136 | Help → Show Release Notes; separately — the post-update widget | TWO different surfaces. (1) The notes open as an EDITOR TAB — an ordinary document, which is scrolled, searched and closed; the same item is in the Help menu and in the command palette (`ShowReleaseNotesAction`, `f1: true`). The switch is declared as the setting `update.showReleaseNotes` ("Show Release Notes after an update", default `true`). (2) `PostUpdateWidgetContribution` — a sticky panel of 420 px in the upper right corner with a badge, the heading "New in {version}", buttons and a cross; the content is DOWNLOADED from the address of the release, rather than lying in the build. The gate: the storage key `postUpdateWidget/lastKnownVersion` (version + commit + time), plus a refusal to show it on a metered connection and if the window was not in focus |
| IntelliJ IDEA | 2026.2 | Help → What's New in IntelliJ IDEA | "When IntelliJ IDEA updates to a **new major release**, it opens the What's New in IntelliJ IDEA tab in the editor" — also a document tab, also with a manual duplicate in the Help menu, and with a named switch in the update settings: "Show What's New in the editor after an IDE update". Three decisions of VS Code are repeated independently: a document instead of a popup, only for a major release, a paired menu item |
| Blender | 5.2 LTS | Splash Screen → What's New | A special path: there is no permanent "after an update" at all — the splash shows at EVERY launch (it is closed by clicking elsewhere or by Esc), in its lower half next to "Recent Files" stands the item "What's New — Open the latest release notes", and the version is printed in the upper right corner of the picture. The update changes not the presence of the surface but its content: at the first launch of a new version a Quick Set Up appears in the same area |
| Adobe Photoshop | 27.9.1 (July 2026) | An announcement of the version in the official community | A post in three parts: "What's New" (each item linked to the help), "**Bug fixes – Thanks to you!**" — and this device is nowhere described as a feature: each fix is made by a LINK TO THE THREAD where the bug was reported. The list of what was repaired is addressed by name to those who complained. 28 replies, 2217 views — and in the replies people ask about their own bugs |

## The trick behind it

The load that one status-bar line cannot hold is here a **LIST THAT HAS A DATE**: the difference between two builds. It has no and cannot have an on-screen representative: there is nothing to hover over, because "new" is exactly what the user has not yet found on the screen, and "fixed" yesterday looked exactly as it does today. There is nothing to point at and nowhere to come to — the only load in this wave that must catch up with the person by itself. For us it will lie on two surfaces, both already existing: one line in the notifications window (`app.rs:1103`, a bell with a counter of unread) and the full list — as a page on the site, where growing text is already assembled in five languages.

**1. The display gate is not "the version changed" but "the MAJOR changed", and this was learned at a high cost.** VS Code stores in `postUpdateWidget/lastKnownVersion` the version AND the commit, but compares via `isMajorMinorVersionChange`. The reason is bug 110511 (14 reactions): emergency reissues were counted as a new version, and "for the August release, this meant that occasional users got to see the release notes 4 times". JetBrains came to the same thing independently: the tab opens only on a major release. For us this is not a subtlety but a question of feasibility: our version is `ktYY.MM.DD-<hash8>` by the date of the COMMIT (`src/libs/kt-version/build.rs`), that is, every build is a new version. In the last 30 days the repository has 207 commits. A banner "show once per version" would pop up almost every launch.

**2. A list of fixes where each line is a link to the author's complaint.** At Adobe the section is called "Bug fixes – Thanks to you!", and four items — four links to the threads where the bugs were reported. This turns the genre over: the changelog stops being a vendor's announcement and becomes a RECEIPT, addressed by name. The reverse side also works: people read exactly this section looking for THEIR item — "is initial direction brush problem fixed in this update?". We have one user, so the share of "Thanks to you" is 100 %, and the addressee of each receipt is known in advance.

**3. "What's new" as a table of contents of the manual, not as a document.** At Apple the chapter lives INSIDE the manual at a versioned address (`/12.3/`), and each of the seven items ends with a link to the chapter where this is described. The consequence is not obvious: such a changelog cannot be written before the manual — it consists of links into it. And a second consequence, visible only when switching the version of the manual: the old "What's new" moves away together with the old manual, and Apple has no accumulated history at all. Resolve solves the same task in a mirror way — a separate PDF for each release ("only the new features"), which goes nowhere, but also links to nothing.

## Pros

- The only channel of the wave that works WITHOUT the user's participation. All the other surfaces of the reconnaissance (the palette F-HLP-015, the context menu F-HLP-016, the cheat sheet F-HLP-024, the description of a parameter F-HLP-006) are pull: they are opened by the one who has already understood that they do not know something. An experienced user does not understand this by definition: they solve their task and have no reason to search.
- The division into "new / fixed" is not design but two different readers. The list of fixes is read by the one who decides "whether to install", and they look for one line of their own; the list of novelties is read by the one who has already installed. A text merged into one answers neither of them: in it one cannot quickly find one's own bug, nor see what has appeared.
- A changelog item is the cheapest unit of documentation of all: it is written once, at the moment of the edit, by the one who made the edit, and it does not become obsolete any more, because it is tied to a version. The manual (F-HLP-023) becomes obsolete with every edit; the changelog — never.
- It gives the first "release" unit in the project. Today there is none: the git tags are working ones (`tested-1`, `nontested-32`, `cpp-reference-final`, `4K`), there is not a single release one, and "which builds were ever released" is not recorded in the repository.
- Incidentally it closes the question "is it the program I launched": the line "in this build X has appeared" is a checkable statement, visible to the RUNNING program, not only to the build script (the same pain as in F-HLP-026, from the other side).

## Cons

- It grows monotonically and forever. This is the only text of the product with such a property: the help is rewritten, the changelog is only appended to. It is multiplied by five languages (F-HLP-033) — and unlike a hint, which was translated once, here translation is required EVERY release, otherwise the share of the translated falls by itself with complete inaction.
- A text written from commits is not readable. This is not a hypothesis: we have literally such a text, 268 items of `docs/porting-notes.md`, and it is written for agents — the average item is 903 characters (median 692, maximum 4957), 159 items mention the number of a regression test. None of them can be shown to the user as is, and the matter is not the language but the genre: there it is explained WHY it was done this way.
- It requires discipline at the moment of the edit, not at the moment of release. A changelog written after the fact is work over the whole catalog at once, and it is done once and never again (precisely why the history of Vegas releases was assembled not by the vendor but by a forum moderator).
- A thing shown by itself irritates instantly, and the complaint is formulated precisely: "Since we open release notes (by default) on each release, there should be a visible way to opt-out". Everything that is shown without a request must carry a visible switch (rule 19), otherwise it pays in trust.
- An honest list of fixes also publishes what was broken. For a project with one user this is cheaper than it seems (he himself wrote about these defects), but the rule is universal: the section "fixed" cannot be kept selectively — as soon as the inconvenient falls out of it, people stop reading it too.

## What users say

Demand for the channel, verbatim — the thread for the new Vegas Pro build, on release day:

> What would be nice is if there were a notice after I launched Vegas Pro.
> — arpierre, Boris FX forum, thread "New Vegas Pro build v20260.0.0.143",
> 19.06.2026, https://forum.borisfx.com/t/24196

In the same thread — a check of that hope and what was left in its place:

> After updating the Boris FX Hub, I launched Vegas to see if some notice has
> popped up, but nothing.
> — bitman, same thread, 19.06.2026 (accessed 2026-09-07)

The reverse side — the price of showing it without a gate:

> For the August release, this meant that occasional users got to see the
> release notes 4 times.
> — digitarald, VS Code, issue 110511 "Only auto-open release notes for major
> releases", 12.11.2020, https://github.com/microsoft/vscode/issues/110511

And the demand for an off switch — from the developer of the same product:

> Since we open release notes (by default) on each release, there should be a
> visible way to opt-out.
> — eamodio, VS Code, issue 110507, 12.11.2020,
> https://github.com/microsoft/vscode/issues/110507

What people look for in a list of fixes:

> is **initial direction brush** problem fixed in this update?
> — sbn85, official Adobe community, Photoshop 27.9.1 announcement, 18.08.2026,
> https://community.adobe.com/announcements-710/photoshop-v27-9-1-is-live-explore-ideas-faster-with-firefly-boards-smarter-ai-prompts-and-streamlined-editing-1634244

## How it fits Kinotrain

The main point here is not "we need a changelog" (that is known without any reconnaissance), but that
**the source for it is already written, and not in the right genre.** A verbatim example, item 259 of `docs/porting-notes.md`:

> "**The morph block length is editable (R.38 item 9).** "I don't understand how to change the length of a morpher event": `morpher_at` checked only the body, there were no edges, `Drag::Morph` could do only one move, and the morpher window had no length at all. Now the block has edges (`Drag::MorphEdge`): the right edge changes the duration, the left edge moves the start while the end STAYS — as with a range marker (rule 20: nothing grows on its own). Test 259."

For the user, one phrase is addressed out of these 470 characters: "The morpher block now stretches past its edges: the right edge changes the length, the left edge moves the start." The function name, the enum variant, the rule number and the test number all disappear — that is, everything the item was written for. The same goes for item 267 (2.5 KB about `FxInstance::LayerMix { amount, from }`, the segment-by-segment chain computation and the unaffected golden references): the user-facing line is "Morpher layers gained blend strength and fades: 60% is a mix of the processed picture with the unprocessed one." And items 252 and 264 do not make it into the list AT ALL: the "there are no dead ends" mechanism and the live scenario `deadend.py` are changes to our work, not to what its program does. The filter is as mandatory here as the translation: of 268 items, noticeably fewer than half are user-facing.

### What is already there

- **The version carries the commit date**: `kt_version::FULL` = `ktYY.MM.DD-<hash8>`
  (`src/libs/kt-version/src/lib.rs`, computed in `build.rs`); today it is
  `kt26.09.07-7cdcc88d`. The question "is this build newer?" is settled by comparing
  strings, and a changelog is not needed for it — which removes half of the usual
  motivation and leaves the other half: "so what changed in it?"
- **The user-facing genre ALREADY exists and is already being translated** — the
  `[roadmap]` section of the landing page: `shipped_title = "Shipped"` and eight lines like
  "Morphers: batch source swaps and effect batches, with bake"
  (`site/content/en.toml`). These are exactly the phrases a changelog is made of — but
  without versions, without dates and without fixes, and the section is honestly labeled
  "Direction, not dates.": an inventory of capabilities, not a history of events.
- **A push surface in the program exists, and it does not overlap**: the notifications
  window (`src/apps/kt-app/src/app.rs:1103`, `win.notices_title`, rows "time +
  text", scrolling), the bell with `notices_unread`, and the menu item `menu.view.notices`
  (`src/apps/kt-app/src/toolbar.rs:344`). A ring of 200 entries, with deduplication of
  consecutive repeats. This is a ready address for one line "build updated".
- **A place for the "last seen version" mark exists**: `autosave_path()`
  (`src/apps/kt-app/src/app.rs:4908`) already writes to
  `$XDG_DATA_HOME/kinotrain/`. This is NOT the "user storage outside the project" from
  output no. 1 of the first wave (F-AUT-004 / F-TXT-002 / F-EXP-001 / F-UI-002) — that
  needs an entity library; here one line in an already existing directory is needed, and
  it can be created without opening that question.
- **The multilingual page pipeline is ready** (F-HLP-032): `site/build.py`,
  `translated_min = 0.9` in `site/content/structure.toml`; a language below the threshold
  is built, but it gets `noindex` and drops out of the sitemap.
- **The unit "release" exists in exactly one place, and outside the repository**:
  `src/scripts/release-build.sh:85` writes `target/dist-out/LATEST` — the version and the
  tarball name. That is everything the project remembers about releases.

### What is missing

- There is no CHANGELOG at all. A grep of the whole tree finds the word exactly once — in
  `docs/user-feedback.md:1190`, in the list of what we do not have.
- **There is no unit "release"**, and therefore no gate, which VS Code and JetBrains learned
  at a high price: 396 commits, 207 of them in 30 days, each one a new version line; there
  is not a single release tag.
- The installation leaves no trace: `src/packaging/install.sh` puts the binaries into
  `~/.local/bin` on top of the old ones. There is no version file and no "updated" record —
  the program can learn about an update only by comparing itself with its own mark.
- The interface has no menu item and no key: there are three menus (`[menu.file]`,
  `[menu.edit]`, `[menu.view]` in `src/apps/kt-app/i18n/en.toml`), and there are zero keys
  about the version or about updates (checked by grepping the catalogs).
- The version in the only user-facing place is typed by hand and is already out of date:
  `site/content/structure.toml:175` — `tar xf kinotrain-26.08-linux-x86_64.tar.gz`, while
  the current build is `kt26.09.07`. The site does not know that something newer has come out.
- Not one item of `porting-notes.md` has a user-facing wording: the genre is not provided
  for by the format, so it is not written.

### What gets in the way

- **Rule 20 (nothing grows on its own)** — a changelog is exactly what grows on its own.
  Hence the choice of place: a growing text has no place in the application's catalogs —
  522 keys × 5 languages, a limit of 160 characters per value (`src/apps/kt-app/build.rs`,
  `MAX_VALUE_CHARS`; a longer value BREAKS THE BUILD), t125 does not tolerate extra keys,
  t100 requires a glyph for every non-ASCII character. The average item in porting-notes
  is 5–6 times longer than our own per-line limit.
- **The reconnaissance corrected the shared place of the wave about translations.** The
  claim "effect descriptions are already translated into five languages" is wrong: in
  `site/content/` ONLY Russian has been translated (96% of values differ from the English
  ones), while `es`, `zh-Hans` and `ar` match `en` on all 220 values, that is, 0%. In the
  APPLICATION, on the contrary, everything is translated (`ru` 97%, `es` 93%, `zh-Hans` 98%,
  `ar` 98% of 522 keys). For this note the conclusion is direct: growing text has to live
  where an untranslated language is honestly marked `noindex` (the site), not where the
  completeness of the catalog is a build condition (the application).
- **Rule 19** (a dialog must say where the decision can be changed later) — the very thing
  that is not fulfilled by a single line of the catalog. Showing "what's new" without a
  visible off switch reproduces VS Code's complaint word for word; the off switch must be
  named in the entry itself.
- **Rule 17** (paired ways are identical): what is shown must also open on demand — through
  a menu item, with the same text. Otherwise the push and pull channels diverge, and someone
  who switched the display off loses access to the list for good.
- **Rules 9 and 13**: there must be no banner covering the workspace — and this is not just
  taste, it is the thesis of the wave (we do not propose a popup).
- **There is nowhere to link**: Apple has every item lead to a chapter of the guide, we have
  no guide (F-HLP-023), and there is not a single link from the program to the outside.
  The full form of a changelog (item → page) is unreachable in this note; in the accessible
  form an item ends on itself.

### Which invariants it touches

- **12** (repaint only on movement, no allocations or clones in draw paths): the comparison
  of the version with the build mark is done ONCE at startup, and the result is a single
  entry in `notices`; there is no animation, no timer, no surface that lives frame by frame.
  Any "banner with an appearance effect" would break the invariant.
- UI rules **9**, **13** (place), **17** (paired menu item), **18** (the menu item says WHAT
  and WHERE), **19** (where to switch it off), **20** (grows on its own) — see "What gets
  in the way".
- Decisions 1 and 2 of the R.40 unfreeze (overlap, delay) are not affected: both proposed
  surfaces are an existing window and an external page, not a popup. `theme.rs` and t83
  stay as they are.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages of the round: (1) the unit "release" + a user-facing line in the change; (2) a page on the site and one entry in the program |
| Where the main work is | `docs/porting-notes.md` (writing discipline, not code), `site/` (the release page via the existing `build.py`), kt-app: comparison of the version with the mark in `$XDG_DATA_HOME/kinotrain/`, one entry in `notices`, one menu item |
| Project format | does not touch it |
| Tests that will have to be written | a `--check` clause for a porting-notes item without a user-facing line; `site/build.py --check` for the release page (the version in the body = the version in `LATEST`); `i18n_tests` for the new menu keys; `reach_tests` for "what is shown opens on demand too" |
| Depends on | F-HLP-026 (the version must be visible in the program), F-HLP-032 (the page pipeline), F-HLP-033 (the cost of five languages), F-HLP-013 (the bell as an address), F-HLP-023 (where items can link) |
| Breaks compatibility | no |

A frame, not a spec: what counts as a release, where exactly the mark of the last seen
version lies, and what the page looks like — these are the subject of the round.

## Verdict

We take it, but one must start not with the display. The user-facing list of changes is not
something we "need to write" — it has to be TRANSLATED FROM ITS GENRE: the source exists,
268 items, and it was written for agents. Translating after the fact is expensive and is done
once; writing one phrase at the moment of the edit costs nothing. The second obstacle is
structural: there is nothing to hang "what's new" on while every build is a new version, and
there is no list of releases in the repository at all.

CHECK: docs/porting-notes.md — a clause "every new item carries a line `User:` — one phrase without function names, rule numbers and test numbers, or the word 'not visible' if the change is not shown to the user; an item without such a line is an error of `--check`, not an author's oversight".

DEFERRED: until F-HLP-023 the form "item → link to a guide chapter" (the Apple model) is unreachable, since there are no guide pages; until then an item ends on itself, and this has to be acknowledged rather than imitated with links to nowhere.

ONE LINE: overlap — untouched | delay — untouched | capacity — the status bar gets zero new
characters: a single entry about the update lands in the EXISTING notifications window
(`src/apps/kt-app/src/app.rs:1103`, the bell with the unread counter), and the growing list of
releases goes onto the site as a page through the ready `site/build.py`, where it belongs by
design.

## Sources

- Boris FX forum (VEGAS Pro), thread "New Vegas Pro build v20260.0.0.143",
  19.06.2026 — https://forum.borisfx.com/t/24196 (accessed 2026-09-07)
- Boris FX forum, thread "Vegas Pro Release History", NickHope, 22.06.2026 (where the vendor
  publishes the notes, `readme\Vegas_readme.htm` in the install directory, sections "New
  features" / "Bug fixes and changes" for each build) —
  https://forum.borisfx.com/t/24284 (accessed 2026-09-07)
- Blackmagic Design, DaVinci Resolve support page — the cards "DaVinci Resolve 20.2 New
  Features Guide" and "What's New in DaVinci Resolve 20.2" —
  https://www.blackmagicdesign.com/support/family/davinci-resolve-and-fusion
  (accessed 2026-09-07)
- Apple, Final Cut Pro guide, chapter "What's new in Final Cut Pro for Mac"
  (version 12.3) —
  https://support.apple.com/guide/final-cut-pro/whats-new-ver19637ebab/12.3/mac/15.6
  (accessed 2026-09-07)
- VS Code, source code of the current branch: `update.config.contribution.ts`
  (the setting `update.showReleaseNotes`, "Show Release Notes after an update"),
  `update.contribution.ts` (`ShowReleaseNotesAction`, the Help menu item and the palette),
  `postUpdateWidget.ts` (`postUpdateWidget/lastKnownVersion`,
  `isMajorMinorVersionChange`, the look and the buttons of the widget) —
  https://github.com/microsoft/vscode (accessed 2026-09-07)
- VS Code, issues 110511 and 110507 —
  https://github.com/microsoft/vscode/issues/110511 and
  https://github.com/microsoft/vscode/issues/110507 (accessed 2026-09-07)
- JetBrains, IntelliJ IDEA help, "Update IntelliJ IDEA" (the What's New tab after a major
  update, the Help menu item, the setting "Show What's New in the editor after an IDE
  update") — https://www.jetbrains.com/help/idea/update.html (accessed 2026-09-07)
- Blender Manual 5.2 LTS, "Splash Screen" (the item "What's New — Open the latest release
  notes", the version in the corner of the picture) —
  https://docs.blender.org/manual/en/latest/interface/window_system/splash.html
  (accessed 2026-09-07)
- Official Adobe community, the Photoshop 27.9.1 announcement (the sections "What's New" and
  "Bug fixes – Thanks to you!" with links to the threads of the bug-report authors) —
  https://community.adobe.com/announcements-710/photoshop-v27-9-1-is-live-explore-ideas-faster-with-firefly-boards-smarter-ai-prompts-and-streamlined-editing-1634244
  (accessed 2026-09-07)
- No program was checked live: there is no access from this environment — neither to the five
  core editors nor to software outside the core (`_editors.md` does not assume it either).
  The Premiere and After Effects rows are marked [NOT VERIFIED]: `helpx.adobe.com` did not open
  either directly (WebFetch timeout) or through the text proxy `r.jina.ai` (HTTP/2 framing error),
  including the `/in/` mirror, which had worked in the first wave; a clarification for the next notes.
- The WebSearch budget for the session was exhausted before work began (200 of 200), so
  everything was obtained by direct requests: the Discourse API of the Boris FX forum
  (`/t/<id>.json`, `/search.json`), the GitHub REST API (`/repos/microsoft/vscode/issues/<n>`),
  `raw.githubusercontent.com` for the VS Code sources, the text proxy `r.jina.ai` for
  docs.blender.org, support.apple.com, jetbrains.com, blackmagicdesign.com and
  community.adobe.com; search went through `search.yahoo.com` via the same proxy
  (`html.duckduckgo.com` through the proxy returned no results in this session).
- Data about our state (268 porting-notes items and their lengths, 396/207 commits, the
  coverage of the site and application translations, `LATEST`, the tags) was obtained by
  counting over the repository on 2026-09-07, not from documentation.
