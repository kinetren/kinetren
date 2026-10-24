---
id: F-TXT-006
title: Text animation: typewriter, per-character, scrolling ticker
status: done
priority: B
cost: M
category: titles/text
editors: [after-effects-26, resolve-21, fcp-12, vegas-pro-2026, premiere-26]
we_have: yes
crates: [kt-media, kt-model, kt-app, kt-engine]
invariants: [7, 11, 12]
ui_rules: [11, 16, 17, 20]
round: R.53
point: 423
tests: [t534, t535, t536, t537, t538, t539, t540]
depends_on: [F-TXT-001]
checked: 2026-09-17
trackers: [kde-bugzilla, kde-invent, openshot, olive, mlt, flowblade, shotcut]
---

# F-TXT-006 — Text animation: typewriter, per-character, scrolling ticker

## Essence

The caption does not appear all at once. Letters are typed as on a typewriter, words fly in one by one,
end credits crawl from bottom to top. The editor does not set forty keyframes for forty letters: they choose
a ready preset and drag one handle — where to start, how long to take to get there, and the unit
(character / word / line). Editing the text itself after this must not break the animation.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x | Text Animator + Range Selector; ready-made Animation Presets in the Text section | THE REFERENCE. The animator's value is not a value but an OFFSET, applied to a subset of characters; the subset is chosen by the Range Selector (Start / End / Offset; the basis is characters, characters excluding spaces, words, lines). What is animated is NOT a property but the Offset of the selector: one keyframe sweeps the range along the line. The layout is computed over the FULL line, while the "typewriter" is an animation of character opacity |
| DaVinci Resolve | 21 | Follower — a Fusion modifier on the Text+ node | The Follower is attached by right-clicking the **Styled Text** field, that is, to the STRING itself, not to a numeric parameter; further, "Character Level Anim", Delay Type "Between Each Character", delay and overlap in the Timing tab. The "Text" generator on the Edit page cannot do this at all — per-character work lives only on the Fusion page [NOT VERIFIED against the primary source: the official Reference Manual was not downloaded; the set of controls is taken from publicly available write-ups] |
| Final Cut Pro | 12.3 | Text Behaviors — Motion behaviors published into a title | The animation is set up not with keyframes but with a BEHAVIOR: the official Motion guide lists Sequence Text, Type On ("generate a typewriter-style text effect"), Scroll Text, Text Tracking, and in a separate section "Animate individual text glyphs". FCP has no editor of its own for behaviors: it is in Motion, and the editor gets the published handles (F-TXT-002). The unit (character / word / line), direction and overlap are the controls of Sequence Text [NOT VERIFIED: the page of controls is served truncated from this environment] |
| VEGAS Pro | 2026 | ProType Titler (legacy, hidden) — versus Titles & Text | The only one of the five that BURIED its per-character animation. Help: "The Pro-Type Titler is deprecated and hidden by default" — and this is exactly the tool that can do "static text, animated text effects with splined paths, per-character animation, and advanced curves", and also crawling and running titles. The standard Titles & Text animates the caption AS A WHOLE (intro/outro) |
| Adobe Premiere | 26.x | there is no native per-character animator | Text in Essential Graphics is animated with LAYER keyframes (Position / Scale / Opacity) — they move the whole caption at once. Per-character work comes in from outside: a `.mogrt` from AE (handles without keyframes, F-TXT-002) or third-party packages that openly sell the missing mechanism ("brings per Character Text Animation to Adobe Premiere Pro") |

## The trick behind it

**1. The stability requirement is placed on the RASTERIZER, not on the animator.**
Keyframes on a layer move the whole object; "typewriter" and "by words" MUST address PARTS
of the caption. So the mechanism has two inputs: the animator says
"which piece", and the layout answers "where and in what form". And all the weight falls on the second
input: the same character MUST land on the same
pixel in frame N and N+1, otherwise the letters jitter. Implementations split exactly
along this point. Kdenlive/MLT puts a filter that on every frame redraws the title producer from a TRUNCATED
string — the layout is computed anew, each time from different text. AE does the opposite: it lays out the
full line once and modulates the opacity of characters. With left alignment and Latin script the result is the same; beyond that — it is not.

**2. A typewriter on a proportional font changes the line width on every frame.** Monospace hides this, a proportional font does not. With left alignment the anchor coincides with the start of the line and nobody notices anything; with centering and right alignment the anchor is computed from WHAT HAS ALREADY BEEN PRINTED, and the whole phrase slides sideways for the whole span of printing.
Therefore "cutting the string" is suitable for exactly one of the three alignments, while "hiding characters" is free for all three: the boxes are computed once from the full line, and the anchor is fixed by construction. This difference is not visible in the list of features at all — both items are called "typewriter".

**3. "The next character" is not uniquely defined, and it breaks through three layers in a row.** Layer one — byte versus codepoint: MLT cut the string by
bytes, and a non-ASCII character at the boundary fell apart into U+FFFD (kde-bugzilla
#436113, 2021, closed UPSTREAM by a fix in MLT 7.2.0). Layer two —
codepoint versus GRAPHEME CLUSTER: a base with combining diacritics, an emoji ZWJ
sequence, Devanagari — none of the implementations found reached this level. Layer three, the nastiest, and it is not fixed
by a patch: in Arabic the form of a letter ALREADY SHOWN depends on the NEXT one, so the appearance of the next character rewrites the previous one. The reporter of
kde-bugzilla #484962 (open since 03.04.2024) describes this verbatim: letters are shown in isolated form until the next one appears, and only then change their shape — plus the printing goes left to right and pushes the text to the right, instead of going right to left. The conclusion that unites all three points: **shape once, expand by clusters.** An implementation that cuts the string does not get to the right result: what is wrong with it is not the arithmetic but the design.

## Pros

- A selector in fractions of the string length decouples the text and the animation: a word was added — the animation rebuilt itself, the keyframe is still one (on Offset). This is the same property that F-TXT-001 named the main one in AE and the Fusion Follower.
- "Hide, don't cut" gives frame-to-frame stability for free, a fixed anchor and unchanged letter shapes: the layout is a pure function of the full string, and does not depend on the animation frame at all.
- The selector unit (character / word / line) is one handle instead of three different implementations: FCP calls it Sequence Text, AE — the basis of the Range Selector.
- The animator knows nothing about the text: the pair "subset selector + constant offset" applies to any set of identical objects.
- A preset works here as documentation (F-HLP-031): the user sees not a list of parameters but "this is how it is set up", and from there digs inside.

## Cons

- The layer MUST output BOXES OF PARTS, not one picture. The rasterizer's "draw the string into a buffer" is in principle unsuitable for per-character animation, and this is a requirement on the lower layer that cannot be circumvented from above.
- The mechanism is two-storied and therefore opaque: in AE "why isn't it moving" runs into an invisible Range Selector, not into the animator. For templates this same property is carried to the end — the consumer does not see a single keyframe (F-TXT-002).
- The typing speed is set in FRAMES per character and does not scale with the event length: a hundred words on the fastest step are typed for more than twenty-five seconds, and there is nothing to shorten it with (kde-invent #1188, 2021).
- A title with a typewriter moves across its entire span: every frame is a new frame, and every optimization of "the picture did not change" is switched off on it.
- The mechanism does not carry over between subsystems of the same editor: in Kdenlive a typewriter exists for a title and does not exist for a subtitle, because the title is an XML producer and the subtitle is the `avfilter.subtitles` filter. The request has been open since 2021.
- The end of the path is visible in Vegas: per-character animation existed, and it was removed to "deprecated and hidden", leaving animation of the caption as a whole.

## What users say

Why this is needed at all — a request to Olive, where the wording hits the mechanism exactly (and the reporter himself refers to a third-party preset package, because there is nowhere to get them in the editor):

> the texts are handled like a picture, So we can't play dynamically with
> it, such as adding some animation effects to the characters of a word!
> — zakaria-chahboun, github.com/olive-editor/olive/issues/783, 11.04.2019,
> https://github.com/olive-editor/olive/issues/783

The third item of the "trick behind it", in the reporter's words; the report has been open for more than two years:

> the text is considered unjoined while typed so the letters appear in the
> singular form until the next character appears, and only then does it
> become the joined form while changing the shape completely.
> — bugs.kde.org #484962, 03.04.2024,
> https://bugs.kde.org/show_bug.cgi?id=484962

The same reporter on direction: "in RTL languages (such as Hebrew) the text is typed
left aligned and pushed over to the right (while it should be typed on the
right and pushed to the left)" (ibid.).

The same thing from the other side — where animated titles are handed to an external renderer:

> When I type Arabic text in Animated titles, the text is not displayed
> after rendering. All animated titles won't work with Arabic.
> — abogadeer, github.com/OpenShot/openshot-qt/issues/1165,
> 29.12.2017, https://github.com/OpenShot/openshot-qt/issues/1165

Why the mechanism does not carry over within one editor:

> Titles and subtitles work completely different under the hood. While
> titles are XML that is treated as producer, subtitles are applied via the
> avfilter.subtitles effect internally.
> — kde-invent #959, opened 27.02.2021, OPEN (last activity
> 24.03.2026), https://invent.kde.org/multimedia/kdenlive/-/work_items/959

On the speed domain:

> With 100 words it will take easily more than 25 seconds to display
> everything, which is too long.
> — kde-invent #1188, 14.09.2021,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1188

And the official VEGAS help on the fate of the only tool in the lineup with per-character animation:

> The Pro-Type Titler is deprecated and hidden by default.
> — help.magix-hub.com, VEGAS Pro help, accessed 08.09.2026,
> https://help.magix-hub.com/video/vegas/en/content/topics/titles.htm

SIGNAL: text animation — 75 issues in openshot, `typewriter|animated title|scrolling text|credit(s)? (roll|scroll)|rolling credit|text animation`; share 1.7% against 0.3% in kde-bugzilla

The gap of sixfold is not about the popularity of the feature but about its design: in OpenShot
animated titles are rendered by an EXTERNAL Blender binary, and the tracker pays
for every seam of this joint separately — "no frame found in blender output" (#412),
render time on a simple caption (#429), five available fonts instead of the
system ones (#954), Arabic (#1165). Kdenlive has the same mechanism built in, and its
share is five times smaller. This is exactly the case of `gap ≥ 3` from `_trackers.md`: "at
someone's, the design is three times worse, and the question why is material for the trick behind it".

SIGNAL: the typewriter breaks by itself, rather than "is lacking" — 11 issues in kde-bugzilla, `typewriter.*(crash|render|fail|unicode|rtl|reversed|slow)|(crash|render|fail).*typewriter`; share 0.2% against 0.1% in kde-invent

Eleven of the fifteen issues in total with the word `typewriter`. The tail is a decade long and begins with the execution of a WISH: the wish #351168 (10.08.2015,
severity `wishlist`, "I really liked the typewriter effect, and would like to
use it again") was closed FIXED in February 2021 — and then by year come
encoding (#436113, #440217 duplicate, #475282), RTL (#484962, open), "present in the monitor,
absent in the render" (#442495), a green flash on export (#441456, open since
2021), and render crashes in 2015, 2024 and 2025 (#346451, #492309, #503634 plus
duplicate #506776). "Closed" here does not, by any means, mean "fixed": of the thirteen
closed ones, six are marked FIXED, four WORKSFORME.

SIGNAL: complex scripts in text — 7 issues in kde-invent, `(rtl|right.to.left|arabic|hebrew|persian|urdu|bidi).{0,40}(text|title|subtitle|caption|font|typewriter)|(text|title|subtitle|caption|font|typewriter).{0,40}(rtl|right.to.left|arabic|hebrew|persian|urdu|bidi)`; share 0.3% against 0.1% in kde-bugzilla

The numbers are small for everyone and should not mislead: this is not a "rare problem" but a
small share of users for whom the feature does NOT work AT ALL. The topic is universal — it is present in four of the ten trackers that were exported.

## How it fits Kinotrain
CAPABILITY: openspec/specs/generators/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Text animation presets are curves of generator parameters on the event; the source itself resolves them by its own tick; RTL — via `unicode-bidi`.
ADDRESS: src/apps/kt-app/src/i18n/bidi.rs — the class is possible, and nothing catches it: t126 (`i18n_tests.rs:86`) asserts "letters inside a word must stay logical" only for the strings of the translation CATALOG, while a per-character typewriter would break the same condition for the USER'S text, where there is no guard and nothing to attach one to today — across the whole `src/` there is not a single grapheme segmenter (`src/apps/kt-app/Cargo.toml:24` has only `unicode-bidi`), and the "glyph atom" from `push_rtl_run` is coarser than a cluster by construction.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages INSIDE the title round, not a separate round: the expensive part (the rasterizer, the vendored font, the golden) is paid for by F-TXT-001 |
| Where the main work is | kt-media (cluster boxes of the title source, expansion by mask), kt-model (the selector as a title parameter + one `ParamCurve` on Offset), kt-app (the selector widget and presets), kt-engine (nothing beyond F-TXT-001) |
| Project format | does not touch beyond what F-TXT-001 will already require; does NOT create a NEW automation domain |
| Tests to write | "hide, don't cut": the cluster boxes on every frame of the animation coincide with the boxes of the full line; an Arabic string — the letter forms do not change during the reveal and the reveal goes right to left (a relative of t126); the unit is a cluster, not a codepoint (combining diacritics, ZWJ emoji); preview == export on a title with a typewriter (a relative of t11/t67); determinism between runs (a relative of t208); while paused the repaint does not grow (a relative of t38/t39) |
| Depends on | F-TXT-001 (hard: without the title there is no subject), F-AUT-001 (without Bézier an Offset curve is only linear/smoothstep) |
| Breaks compatibility | no |

## Verdict

We take it — but as a STAGE of the title round, not as a separate feature, and with one
condition, written down BEFORE the code: **the layout is computed once over the full
string, the animation modulates the visibility of clusters.** This is not an
implementation detail. All four classes of other products' defects — bytes instead of codepoints, mutation of the Arabic letter form, anchor drift on centered text, render crashes — are consequences of the same decision "cut the string"; the decade-long tail of kde-bugzilla shows that it is not brought to the right result but only reopened. Our `bidi.rs` has already forbidden this operation, and t126 catches it — but only in the translation catalog.

Priority B: without animation a title remains "a picture with letters" (the wording of the Olive reporter), and the editor works around it — chops the string into events, after which any edit of the text means re-laying everything. Cost M: the selector, one curve on Offset and a set of presets on top of the already paid-for rasterizer.

What NOT to do: do not create a fourth automation domain for the sake of "typed exactly by the end of the event". We dropped the share of length in R.33 with a reason (t212), three domains are already marked as debt in F-AUT-002, and the right place for this property is the parameters of the title itself, next to the protected time zones that F-TXT-002 asked to think about in advance.

**Done in R.53 (items 423–424).** The condition written down before the code was fulfilled verbatim: there is no time in kt-text, the animation arrives as two numbers from kt-media (`reveal`/`reveal_soft` and `offset`), the layout is computed once over the full string, and at full visibility the EXACTLY prior branch is taken — the golden of the title did not move. The clusters are given by harfrust for free, so there was no need to create our own segmenter. Motion — the shift of the whole block with a quantum of 1/64 px. A fourth automation domain was not created, as promised: the duration is a parameter of the title (`anim_secs`).

A known limitation, written down honestly: with a soft reveal the outline "clicks" at the midpoint (the EDT is computed over the binarized silhouette); the typewriter does not have this. Further life of the feature — items 423–424.

## Sources

Trackers (dumps — `~/.cache/kinotrain/trackers/`, export of 08.09.2026, not included in the repository; of the twelve registry keys, ten were exported, `blender-vse` and `ffmpeg-trac` were empty at the time of work, so they do not participate in the comparisons):

- https://bugs.kde.org/show_bug.cgi?id=351168 — wish of 10.08.2015, `severity=wishlist`, RESOLVED/FIXED 16.02.2021 (accessed 08.09.2026)
- https://bugs.kde.org/show_bug.cgi?id=436113 — encoding, 24.04.2021, RESOLVED/UPSTREAM 15.11.2021, the fix went into MLT 7.2.0 (08.09.2026)
- https://bugs.kde.org/show_bug.cgi?id=484962 — RTL and mutation of letter forms, 03.04.2024, OPEN (08.09.2026)
- https://bugs.kde.org/show_bug.cgi?id=442495 — "present in the monitor, absent in the render", 16.09.2021, RESOLVED/WORKSFORME (08.09.2026)
- https://bugs.kde.org/show_bug.cgi?id=441456 — green flash on export, 24.08.2021, OPEN (08.09.2026)
- https://bugs.kde.org/show_bug.cgi?id=503634,
  https://bugs.kde.org/show_bug.cgi?id=492309,
  https://bugs.kde.org/show_bug.cgi?id=346451 — render crashes 2025 / 2024 /
  2015 (08.09.2026)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/959 — typewriter on a subtitle, open since 27.02.2021 (08.09.2026)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1188 — speed in frames per character, 14.09.2021 (08.09.2026)
- https://github.com/olive-editor/olive/issues/783 — "texts are handled like
  a picture", 11.04.2019 (08.09.2026)
- https://github.com/OpenShot/openshot-qt/issues/1165 — Arabic in animated titles, 29.12.2017; there also #412, #429, #954 — the cost of the external Blender (08.09.2026)
- https://github.com/jliljebl/flowblade/issues/115 — a scrolling line as a request, 07.06.2015 (08.09.2026)

Vendor documentation:

- https://help.magix-hub.com/video/vegas/en/content/topics/titles.htm —
  ProType Titler "deprecated and hidden by default", the composition of its features
  and crawling/running titles (08.09.2026)
- https://support.apple.com/guide/motion/intro-to-text-behaviors-motn1e5df5e5/mac
  — Sequence Text, Type On ("typewriter-style text effect"), Scroll Text,
  Text Tracking, "Animate individual text glyphs" (08.09.2026). The page of
  Sequence Text controls is served truncated — the unit (character / word /
  line), direction and overlap are marked `[NOT VERIFIED]`
- Resolve Follower (right-click on the Styled Text field, Character Level Anim,
  Delay Type "Between Each Character", the Timing tab) — from publicly available
  write-ups, https://jayaretv.com/fusion/text-node/ and
  https://forum.blackmagicdesign.com/viewtopic.php?f=22&t=177057
  (08.09.2026); the official Reference Manual was not downloaded, hence
  `[NOT VERIFIED]`
- The AE lines (Animator + Range Selector, the selector basis, the ready-made Animation
  Presets of the Text section) were written from knowledge of the tool and agreed with
  F-TXT-001: `helpx.adobe.com` does not open from this environment (re-checked
  08.09.2026 — the connection is refused by the proxy)
- What Premiere per-character work brings in from outside — from descriptions of third-party
  packages that openly sell this mechanism
  (https://misterhorse.com/products/text-presets-for-premiere/3308 and
  posts about typeGEMs on community.adobe.com, 08.09.2026); Adobe publishes no
  official confirmation that "there is no native animator", so the table row relies on the absence of such a mechanism in the
  Essential Graphics help `[NOT VERIFIED]`
- The Vegas forum at the old address `vegascreativesoftware.info` returns a 301 to
  `forum.borisfx.com` and the archive is lost — the same observation as for F-TXT-001;
  the quote is taken from the OFFICIAL help, not from a thread

Internal sources: `src/apps/kt-app/src/i18n/bidi.rs`,
`src/apps/kt-app/src/i18n_tests.rs`, `src/apps/kt-app/src/layout_tests.rs`,
`src/apps/kt-app/src/font_tests.rs`, `src/apps/kt-app/Cargo.toml`,
`src/libs/kt-media/src/placeholder.rs`,
`src/libs/kt-model/src/project.rs`, `src/libs/kt-model/tests/fx_auto.rs`,
`src/libs/kt-engine/src/video_plan.rs`,
`src/libs/kt-engine/tests/golden_export.rs`,
`src/apps/kt-app/src/util.rs`, `src/apps/kt-app/src/render_dialog.rs`,
items 126, 221, `docs/features/_trackers.md`.
