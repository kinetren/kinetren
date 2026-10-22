---
id: F-HLP-033
title: The cost of a help line in five languages
status: candidate
priority: B
cost: S
category: help/authoring
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2]
we_have: partial
crates: [kt-app]
invariants: [12]
ui_rules: [7, 11]
round: null
point: null
tests: []
depends_on: [F-HLP-032]
trackers: [kde-bugzilla, kde-invent, openshot]
checked: 2026-09-08
---

# F-HLP-033 — The cost of a help line in five languages

## Essence

A counter, not an overview: what ANY new line that the user will see costs. The other notes of this wave refer here instead of recalculating the price on their own and proposing the impossible.

**The cost of one new label or hint: 5 values of ≤ 160 characters (4 of them are translation) + 1 FULL path `k::…` in the code + 0 new tests with 3 old ones green (t100 glyphs, t125 catalogs, t128 budgets); a partial delivery does not exist — the absence of any of the five breaks the BUILD of `kt-app`. If a new hieroglyph appears in the translation — plus a rebuild of `NotoSansSC-kt.otf` and a commit of the font binary.**

**The cost of one manual page is different: there is no length limit (23 of 220 values of the landing page are already longer than 160), but 8 screenshots × 5 languages and versioning are added — and, unlike the program, a partial delivery DOES exist: a language with coverage < 90 % is built, but gets `noindex`.**

## Who has it and how

The column "How it is organized" answers one question: how many languages the INTERFACE has and how many the HELP has. Those are almost always different numbers.

| Editor | Version | What it is called | How it is organized |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Online Help | the language is in the path (`/Documentation/vegas/2026/en/`), that is, addressing like ours; beyond `en` the reconnaissance found nothing. The interface has historically been wider than the help [NOT VERIFIED: the vendor site returned 301 without a body from this environment] |
| Adobe Premiere | 26.x | Premiere User Guide | one live version on helpx, the language is in the path (`/de/`, `/jp/`); some locales return machine translation with a banner. The interface is translated more widely than the guide [NOT VERIFIED: `helpx.adobe.com` is unavailable both directly and through a text proxy] |
| DaVinci Resolve | 21 | Reference Manual | on the support page for version 21 there is exactly ONE entry "DaVinci Resolve 21 Manual", with no language variants next to it (checked 2026-09-07), whereas the interface language is chosen in the program's settings. F-HLP-032 calls the same PDF multilingual — the discrepancy is not resolved, [NOT VERIFIED] |
| Final Cut Pro | 12.3 | Final Cut Pro User Guide | **10 interface languages against 7 for the guide** (checked). Interface: en, fr, de, it, ja, ko, pt, zh-Hans, es, zh-Hant. The guide is translated into de, fr, ja, zh-CN, ko, es; for it, pt-BR, zh-TW, nl, ru, ar THE SAME address returns an ENGLISH page inside a localized site wrapper |
| Adobe After Effects | 26.x | After Effects User Guide | the same structure as Premiere: helpx, language in the path, one live version [NOT VERIFIED] |

Three interface languages of Final Cut Pro (Italian, Portuguese, traditional Chinese) have no help at all — and the user finds this out only by opening the page: the site title is in his language, the text of the article is English.

**Outside the core.** Without Blender this feature cannot be described: it is the only one of the programs reviewed where the interface and the manual are translated by DIFFERENT people at different times, openly, the rules of joining are written down, and the result is visible on one page.

| Program | Version | What it is called | How it is organized |
|---|---|---|---|
| Blender | 5.2 LTS | Blender Manual | a build per language (`docs.blender.org/manual/<lang>/latest/`), translation by the community on Weblate, separate from the interface translation. An untranslated string is output IN ENGLISH inside the localized page: in the German build the table of contents has "Blender installieren" and "Installing from Steam" next to each other, "Menüleiste" and "Status Bar" (checked). The page is not hidden and is not replaced with machine translation |

Checked by a query on 2026-09-07 on the same page of the Video Sequencer: the heading is translated for `ru`, `es`, `ja`, `zh-hans`, `it`, `vi`; English — for `de`, `ar`, `fi`. A coverage threshold below which a language is not published was not found for Blender: the build is released at any percentage.

## The trick behind it

The mechanism carries a **foreign name** — the name by which the program is called not by the program, but by its help, translated by another person at another time. A single status-bar string cannot hold this cargo by construction: it takes the name from its own catalog and knows nothing about the existence of a second catalog. The agreement between the two names lives not in the string, but BETWEEN the two catalogs, and therefore nowhere.

For us this cargo has already fallen, and it is visible with grep:

* `site/content/en.toml`, section `[fxdesc]`, key `cammotion` describes the effect **CamMotion**, which has not existed in the program since R.34: it was migrated to `Pan` with curves, and this is fixed by test t230 (`src/libs/kt-engine/tests/fx_auto_render.rs:146`). The landing page in five languages advertises an effect that the user will not find in the picker;
* the reverse side of the same hole: `Pan 3D` — the flagship of R.29, the sole member of the category `three_d` — is absent from the landing page. The counter "10 video effects" (`site/content/structure.toml:77`) matched reality by chance: the program also has exactly 10 templates, but the SET is different;
* `[fxcat]` carries 6 categories, `[fxreg.cat]` in the program has 10. The Russian values of the six common ones coincide word for word ("Motion", "Geometry", "Blur", "Key", "Stylization", "Glow"), but they coincide by hand: `site/build.py` does not read `src/apps/kt-app/i18n/` in a single line, and vice versa.

Blender knows the same hole and patches it with prose, not with a machine: its translation guide requires "UI elements should use the same translation as in Blender's user interface" and, in case of a discrepancy, allows keeping the English term with the translation in parentheses. We stand exactly in the same place — with the difference that Blender has this written down and we do not.

## Pros

- A missing key, an empty value, an extra key, a mismatch of placeholders, a PUA character and an ASCII comma in Arabic **break the BUILD of `kt-app`**, not a test and not the display: `src/apps/kt-app/build.rs`, `fail()` prints `cargo:warning=` and panics. The translator physically cannot ship an unfinished catalog.
- The reverse direction is closed by t125: a key that no FULL path `k::section::CONSTANT` in the sources calls fails the test — the catalog does not rot and the translator does not work in vain.
- The runtime cost is zero: the tables are static, `tr()` returns `&'static str`, Arabic is reordered already in `build.rs` (`bidi::visual`). The catalog does not touch invariant 12 with a single allocation.
- Subsets are cut BY THE FACT of use: 527 CJK codepoints → 99 KB instead of 19 MB (`src/apps/kt-app/assets/SUBSET-MANIFEST.txt`). Arabic is taken as a whole block (U+0600…U+06FF, 63 KB), so a new Arabic letter does NOT require a rebuild — only a new hieroglyph does.
- Coverage is already measured for free: `build.rs` prints the keys whose value coincides with the English one, BY NAME. Today ru has 9 matches out of 517, es 31, ar 8, zh-Hans 3 — that is, 94…99 % of the interface translation.

## Cons

- **There is no rollback.** "Let's ship en+ru first, the rest later" is impossible: no key — no build. Five languages are bought together or not bought at all.
- **The limit of 160 characters is not about length, but about Arabic.** The comment in `build.rs` says it directly: a reordered paragraph cannot be re-wrapped, each visual line needs its own order. So a paragraph of prose in the catalog is impossible structurally, not "not recommended". The built-in MANUAL in `i18n/*.toml` will never fit.
- **The real display ceiling is below 160, and nobody measures it.** t128 measures three menu headings (160 px), eight lines of the performance panel (190 px) and one checkbox (380 px) — and not one of the 85 hints. The longest today is `mrph.src_slot_hint` with 114 characters (en) and `tl.hint.empty` with 124 (es); at the minimum window size 640×640 (`app.rs:5245`) such a line no longer fits in the status strip.
- **The direction "code → catalog" is not closed by anything.** t131 catches only CYRILLIC in literals; an English literal shown to the user outside the catalog is caught by no one. This is exactly how English strings from `thiserror` (`ExportError`, `Error::KtMetaBroken`) get into the status bar.
- A new hieroglyph = rebuild of the subset + commit of the binary font into the repository + the system Noto on the agent's machine. `scripts/subset-fonts.py --check` does not guard this automatically: t100 turns red, and one has to guess the cause from it.
- The subset is computed from the catalogs of the PROGRAM. A hieroglyph that appears only in `site/content/zh-Hans.toml` will not get into the program's font — the site has its own set of fonts.
- **The fallback "no translation → English" costs no less than its absence.** The tracker showed the price directly: Kdenlive #510090 "Kdenlive incorrectly uses English as fallback language for Chinese interface elements" is closed as WORKSFORME, that is, a mixed language is read by the user as a defect, and by the maintainer as the norm, and the dispute is not resolved. Our "no key — no build" buys precisely the absence of this class; introducing a fallback means introducing this too.
- **The coverage threshold punishes the translator, not the author.** Kdenlive #463084: "Slovenian help for KDEnlive is not built, even if translation exists" — the help translation existed, but there was no build for that language. Our `translated_min = 0.9` works the same way: a language with 89 % gets `noindex`, and the translator learns about it not from the tool, but from the absence of his page in `sitemap.xml`.

## What users say

SIGNAL: translation and help as a separate item of expense — 131 tasks in kde-bugzilla, `translat|locale|language|i18n|l10n|\brtl\b|documentation|\bdocs\b|user guide|\bmanual\b`; share 2.1 % against 1.2 % in kde-invent (26 tasks out of 2217)

SIGNAL: the price is not in the fixing, but in the volume — 89 tasks in kde-bugzilla, `\b(translation|documentation)\b`; share 1.4 % against 0.4 % in kde-invent, and among the closed ones the share of real fixes is 57 % against 47 % across the whole tracker.

The second count is the one for which the note looks into the tracker at all. In Kdenlive translation and documentation are not a topic, but **two separate components** of Bugzilla: `Translation` (38 tasks) and `Documentation` (45), together 83 out of 6346. Their structure is the opposite of the rest of the tracker: open there hang **3 tasks out of 83 = 3.6 % against 14.0 %** (891 out of 6346) across the tracker, and among the closed ones the share of real fixes is 56 % against 47 %. That is, translation is fixed willingly and not accumulated — the price lies not in the fixing, but in the volume, and in the fact that the volume returns with every new line. At the same time, shares of fixes can be compared only within Bugzilla: in GitHub/GitLab dumps, closing without a label is indistinguishable from a fix, and the baseline there is 1.00 — the "fixes" count measures nothing there.

Verbatim — from the translators' guide of Blender itself, that is, from those who do this work:

> We expect our readers to use the English version of Blender, not a
> translated one.
> — Blender 5.2 LTS Manual, Style Guide for translators, 2026-09-07,
> https://docs.blender.org/manual/en/latest/contribute/translate_manual/style_guide.html

There, on how names diverge: "UI elements should use the same translation as in Blender's user interface. If the UI element has not yet been translated, keep the English term and add the translation in parentheses."

A live complaint about exactly the gap that this note counts was found in the OpenShot tracker: the interface is translated, the help is not, and the lists of supported languages of the program and of the site are different. The reproduction step there is written in one line: "Click on 'support>User guide' … User guide remains in English".

> "There seems to be inconsistency between which languages are supported. …
> Expect the manual to be in selected language (Dutch/French/?)"
> — OpenShot, issue #3406 "Language of Program vs website vs listed as supported", 24.04.2020,
> https://github.com/OpenShot/openshot-qt/issues/3406 (accessed 2026-09-08)

The titles of the `Translation` component at Kdenlive read like a list of our own risks, and all three — verbatim: `Slovenian help for KDEnlive is not built, even if translation exists` (#463084, FIXED — the help translation for the language existed, but a build for that language did not come out), `Kdenlive incorrectly uses English as fallback language for Chinese interface elements` (#510090, closed WORKSFORME), `Some effects/composite descriptions, option names and option tooltips show always in English` (#512355, FIXED).

What could not be found: a live complaint about the discrepancy between the help and the interface at Blender and at the paid five. The `WebSearch` budget in the session is exhausted (200 of 200); `helpx.adobe.com` does not open either directly or through the text proxy `r.jina.ai` (HTTP/2 framing error), `docs.blender.org` and `translate.blender.org` return a Cloudflare stub on a direct `curl`; the Discourse API of `blenderartists.org` was queried with six requests ("manual not translated language", "documentation in my language manual", "switched back to english interface", "changed blender to english tutorials", "interface language english easier find", "english manual difficult language documentation") — no relevant threads in the results.

## How it fits Kinotrain

ADDRESS: `src/apps/kt-app/src/i18n_tests.rs` (t131) — the class "the interface is translated, but part of the text is still English" is POSSIBLE with us, and nothing catches it: t131 fails the test only on CYRILLIC in a literal of a `.rs` file, so an English string shown to the user outside the catalog passes through in all five languages — exactly this is how `#[error("project uses 3D effects on {0} clip(s), but no GPU is available")]` (`src/libs/kt-engine/src/export.rs:292`) and `Error::KtMetaBroken` (`src/libs/kt-otio/src/lib.rs:59`) get through. At Kdenlive this is not a hypothesis but three separate tasks of the `Translation` component (#502594, #512355, #473086 — effect descriptions, option names and tooltips "show always in English"), and all three are closed as FIXED: the class is real, it gets fixed, and it returns.

### What already exists

The numbers are counted from the code on 2026-09-07, not from memory.

* **522 keys** in each of the five catalogs `src/apps/kt-app/i18n/*.toml` (en/ru/es/zh-Hans/ar); `en.toml` is the reference for the set AND the order, and the index in it is the `Key`.
* **85 out of 522 are hints.** The counting recipe: sections `[tb]` 32, `[tl.hint]` 21, `[hint]` 11 plus keys of the form `*_hint`/`hint_*` in other sections — `[fx]` 13, `[mrph]` 4, `[bin]` 3, `[cam]` 1. If only the three dedicated sections are counted, one gets 64; the discrepancy of the neighboring notes of the wave is about this boundary, not about different data.
* **The value limit is `MAX_VALUE_CHARS = 160`**, `src/apps/kt-app/build.rs:42`. Exceeding it breaks the BUILD (not the test): `check_value` calls `fail()`, which prints `cargo:warning=` and panics. Today nobody hits the ceiling: the maximum is en 139, ru 128, es 145 (`mrph.below_note`, `es.toml:242` — the only value longer than 140), ar 136, zh-Hans 60 characters. If it grew to 173, the build would say `i18n/es.toml:242: "mrph.below_note" is 173 chars, limit is 160 (split it into two keys)`.
* **What else breaks the build** (all in `build.rs`): a missing key (`missing key "…"`), a value that is not in `en.toml` (`key "…" is not in en.toml`), an empty value (`is empty`), a set of placeholders different from the English one, a PUA codepoint U+E000…U+F8FF (`icons are composed in Rust, not in catalogs`), an ASCII `,`/`;`/`?` in an Arabic value (`use Arabic punctuation (U+060C, U+061B, U+061F)`), differing self-names of `lang.*`, and more than 65535 keys.
* **t100** (`src/apps/kt-app/src/font_tests.rs`) requires a glyph for EVERY non-ASCII character of the catalogs and sources. The fonts are taken exactly those that `install_fonts` loads: the default egui bundle (Ubuntu-Light, NotoEmoji, emoji-icon-font) plus `icons::VENDORED` — Phosphor, `NotoSansSC-kt.otf`, `NotoSansArabic-kt.ttf`. On failure it prints `символы без глифа в шрифтах программы (пользователь увидит квадрат): U+XXXX 'ч' — ru.toml:317` (characters without a glyph in the program's fonts (the user will see a square)). The test has a self-check: `я`, `ñ`, `日`, `ب` must be covered, while the Thai `ก` U+0E01 must not be.
* **t125** (`src/apps/kt-app/src/i18n_tests.rs`) — integrity of the GENERATED tables plus a reverse check: a key that no full path `k::…` in the sources calls fails the test (`catalog keys nobody uses (delete them or wire them up)`). A shortened `use` hides a key — therefore the path is written in full.
* **t128** (`src/apps/kt-app/src/layout_tests.rs:1899`) — budgets in PIXELS in all five languages: three menu headings ≤ 160 px, eight assembled lines of the performance panel ≤ 190 px, the checkbox of the economical UI ≤ 380 px. It is fixed by translation, not by widening the panel: the limits 420 and 90 px stand by the owner's direct requests.
* **t131** — the source language of the literals is English: Cyrillic in a string literal of any `.rs` file (except `*_tests.rs` and `tests/`) fails the test.
* **t126/t127** — Arabic is reordered WORD BY WORD (letters inside a word are logical, harfrust will shape them), the sections `fxreg.kw.`/`lang.` are not reordered at all; t127 runs the layout in all five languages, t129 — the preservation of window positions when the language changes.
* **Subsets** — `src/scripts/subset-fonts.py`. A rebuild is needed ONLY for a new symbol from the CJK ranges (U+2E80…9FFF, 3000…303F, 3400…4DBF, FF00…FFEF): Arabic is taken as the whole block U+0600…U+06FF. Everything else (the Turkish `ğ`, a typographic quote) must be found in Ubuntu-Light, otherwise the script exits with `characters covered by no font`. There is a `--check` mode, which says `subsets are stale, rerun without --check`.
* **The site pipeline** — see F-HLP-032; one detail here is important: `translated_min = 0.9` (`site/content/structure.toml:23`) and the automatic `noindex` in `site/build.py:805`. The mechanism is ALIVE and works against us right now: `[fxdesc]` and the whole landing page are translated into ru (98 %) and NOT translated into es/zh-Hans/ar — there lie character-by-character copies of the English, 0 %. Today `site/dist/sitemap.xml` has three entries — the root, `en` and `ru`; the other three languages are not in it.

### What is missing

1. A machine check of the landing page against the program: the names of effects, the categories, and whether the description refers to an existing effect at all.
2. Transfer of `[fxdesc]` into the program. Ten descriptions fit into 160 characters with room to spare (the maximum ru is 93 characters), but "the translation is already paid for" is wrong: en and ru are paid for, three languages would have to be translated, 30 values.
3. A place for prose. The program's catalog is structurally unfit for a manual — 160 characters and line-by-line Arabic.

### What gets in the way

Arabic. Our `bidi::visual` reorders words ONCE at build time and hands over a ready string; this is cheap and works exactly as long as the string is single-line. A multi-line paragraph would require real bidi at the layout stage (the order depends on where the line breaks fall), that is, a different text-display path, not a different text file. The `translated_min` threshold transfers to the program only halfway: `build.rs` already knows how to measure coverage, but "do not show the under-translated" makes no sense in the GUI — a button cannot be hidden. The meaningful analogue — "show in English" — does not exist today at all: the table per language is dense, there is no fallback to English by key, because a missing key does not survive to runtime.

### Which invariants it touches

**Invariant 12** — for its sake the tables are static and `tr()` does not allocate; any future fallback "no translation → English" must remain a lookup in two static tables, not a search in a dictionary in the draw path. UI rules **7** (the text fits) and **11** (glyphs are verified) — these are exactly t128 and t100. Rule **20** (nothing grows by itself) explains why the subsets are committed rather than assembled on the fly.

## Estimate

| What | Answer |
|---|---|
| Volume | S — one stage of a round |
| Where the main work is | `src/apps/kt-app/src/i18n_tests.rs` (a new test) and `site/content/*.toml` (fixing what the test shows) |
| Project format | does not touch it |
| Tests that must be written | one: a check of the names/categories of the landing page's effects against the program's registry. The budget for hint width does not go here (F-HLP-002) |
| Depends on | F-HLP-032 (the site pipeline), F-HLP-002 (the status strip) |
| Breaks compatibility | no |

Separately: the transfer of `[fxdesc]` into the program is not assigned by this note — it has its own price (30 new values and 10 new keys) and its own place in the round.

## Verdict

The note by itself builds nothing: it is a counter, and its use is that the other 32 do not reinvent the price. But the reconnaissance found a hole in the counter, and the hole has an address.

TEST: i18n_tests::landing_and_app_name_the_same_effects — today it fails three times: `[fxdesc].cammotion` describes the effect CamMotion, migrated to `Pan` as early as R.34, `[fxcat]` knows 6 categories out of 10, and `Pan 3D` from the category `three_d` is absent from the landing page altogether; there is nothing to compare them with, because `site/build.py` does not read the program's catalogs, and `fx_registry.rs` knows nothing about the site.

The price that the other notes quote is the two highlighted lines in "Essence". What matters in them is not the number 160 (today no one hits it), but the absence of partial delivery: five languages are bought together.

## Sources

- `src/apps/kt-app/build.rs`, `src/apps/kt-app/i18n/{en,ru,es,zh-Hans,ar}.toml`, `src/apps/kt-app/i18n/README.md`, `src/apps/kt-app/src/font_tests.rs`, `src/apps/kt-app/src/i18n_tests.rs`, `src/apps/kt-app/src/layout_tests.rs`, `src/apps/kt-app/src/i18n/mod.rs`, `src/apps/kt-app/src/fx_registry.rs`, `src/scripts/subset-fonts.py`, `src/apps/kt-app/assets/SUBSET-MANIFEST.txt`, `site/build.py`, `site/content/*.toml` — read 2026-09-07, all numbers counted by a script over these files
- Blender Manual, Style Guide for translators: https://docs.blender.org/manual/en/latest/contribute/translate_manual/style_guide.html (accessed 2026-09-07). Live verification was not planned for software outside the core; the pages were read through the text proxy `r.jina.ai` — `docs.blender.org` on a direct `curl` returns a Cloudflare 403. This refines the F-HLP-032 entry "`docs.blender.org` returned 403": there is a way around it, and the version-and-language addressing `manual/<language>/latest/` is now verified by a query, not taken from the documentation
- Blender Manual, fallback measurement: the page `editors/video_sequencer/index.html` in the ru/es/ja/zh-hans/it/vi builds has a translated heading, in de/ar/fi — English (2026-09-07)
- Final Cut Pro User Guide, language measurement: https://support.apple.com/de-de/guide/final-cut-pro/welcome/mac and the same addresses for fr-fr, ja-jp, zh-cn, ko-kr, es-es (translated) versus it-it, pt-br, nl-nl, ar-ae, ru-ru, zh-tw (English text in the localized wrapper); checked on two different pages of the guide, 2026-09-07
- Final Cut Pro interface languages: https://apps.apple.com/us/app/final-cut-pro/id424389933 — "English, French, German, Italian, Japanese, Korean, Portuguese, Simplified Chinese, Spanish, Traditional Chinese" (accessed 2026-09-07)
- DaVinci Resolve, composition of the help editions: https://www.blackmagicdesign.com/support/family/davinci-resolve-and-fusion (accessed 2026-09-07)
- Trackers (third wave, R.41): dumps from `~/.cache/kinotrain/trackers/{kde-bugzilla,kde-invent,openshot}.jsonl`; the count is reproducible with `python3 src/scripts/tracker-rank.py --query …` using the filters from the `SIGNAL:` lines; the components `Translation` (38) and `Documentation` (45) were read from the `component` field of the kde-bugzilla dump (2026-09-08)
- https://github.com/OpenShot/openshot-qt/issues/3406 — OpenShot, "Language of Program vs website vs listed as supported": the interface is translated, `User guide remains in English` (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=463084 — Kdenlive, "Slovenian help for KDEnlive is not built, even if translation exists" (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=510090 — Kdenlive, "Kdenlive incorrectly uses English as fallback language for Chinese interface elements", closed WORKSFORME (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=512355 — Kdenlive, "Some effects/composite descriptions, option names and option tooltips show always in English" (accessed 2026-09-08). The bodies of the Bugzilla tasks in the dump are empty — the REST `/bug` endpoint does not return them, therefore the titles are quoted verbatim
- Premiere, After Effects and VEGAS Pro: **from documentation, not checked live** — `helpx.adobe.com` is unavailable from this environment either directly or through `r.jina.ai`, `vegascreativesoftware.com` returns 301 without a body. All statements about them are marked `[NOT VERIFIED]`
