---
id: F-TXT-005
title: Outline, shadow, plate and text draw order
status: done
priority: B
cost: M
category: titles/text
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, libass-0.17]
we_have: yes
crates: [kt-fx, kt-media, kt-engine, kt-gpu, kt-model, kt-app]
invariants: [4, 7, 10, 11]
ui_rules: [6, 13, 17, 20]
round: R.52
point: 406
tests: [t486]
depends_on: [F-TXT-001]
trackers: [kde-invent, kde-bugzilla, pitivi, flowblade, olive, mlt, openshot]
checked: 2026-09-08
---

# F-TXT-005 — Outline, shadow, plate and text draw order

## Essence

A title is read over someone else's picture, and white letters on a white sky are not visible.
Readability comes not from the choice of font but from a styling layer: an outline along the glyph
contour, a drop shadow with offset and blur, a plate under the line. The editor turns them on with
checkboxes and adjusts three or four numbers. The inconspicuous part on which all of this works or
does not work is the ORDER: what is drawn under what. An outline drawn over the text eats half of the
stroke, a shadow under the outline disappears, a plate drawn over the line covers the line.

Ownership: the text itself as an object is F-TXT-001 (there: the string, the font, shaping, the
determinism of rasterization). Here: the styling of this text and the order between the styling
layers.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x | Character panel: Fill / Stroke; Layer Styles | Order is a FIRST-CLASS property: a character has "Fill Over Stroke" / "Stroke Over Fill", and a layer has two override switches, "All Fills Over All Strokes" / "All Strokes Over All Fills", which override the per-character choice. The stroke is centered on the glyph outline: an "outside" stroke is obtained not by width but by drawing a stroke of double thickness UNDER the fill. Shadow/glow is a different mechanism, Layer Styles (a legacy of Photoshop) with a built-in order. Documented limitation: with per-character 3D, neither layer-level switch has any effect |
| Adobe Premiere | 26.x | Essential Graphics → Appearance: Fill, Stroke, Background, Shadow | The only one that exposes "outside/inside" explicitly: Outer grows outward from the edge and is drawn BEHIND the fill, Inner grows inward and is drawn ON TOP of the fill, Center is drawn over the fill for shapes and UNDER the fill for text. Up to 10 strokes, the first is drawn over the second; a stroke can be applied to individual characters too; there can also be several shadows. Inner/Outer appeared in October 2022, while the earlier Center behavior was kept for the sake of the look of already made projects **[NOT VERIFIED against the primary source: `helpx.adobe.com` is not reachable from the environment (proxy refused); the wording is taken from a search-result snippet of the page `premiere-pro/using/work-with-fill-stroke-background-shadow.html` and confirmed by an announcement in the Adobe community]** |
| DaVinci Resolve | 21 | Text+ (Fusion), Shading tab | The order is made as a STACK, not as a pair: eight numbered shading elements, a "Sort By" menu = Priority (1 is top, 8 is bottom) or Z-depth, plus a "Priority Back/Front" slider. Element kinds: Text Fill / Text Outline / Border Fill / Border Outline. The outline has Thickness, Join Style (Sharp / Rounded / Beveled) and "Adapt Thickness to Perspective". Border has a separate Level menu: Text / Line / Word / Character, so the plate's area is chosen explicitly. The simple "Text" generator of the Edit page can do none of this |
| Final Cut Pro | 12.3 | Text inspector: Face / Outline / Glow / Drop Shadow | Four sections with independent switches; each has its own fill type, color, opacity and blur; Outline also has width, Glow has radius, Drop Shadow has distance and angle. The order between sections is fixed (face on top, shadow at the bottom) and cannot be rearranged. 3D titles have a separate set of parameters (Substance / Edges) |
| VEGAS Pro | 2026 | "Titles & Text" (OFX generator) | The poorest set: Outline Width and Outline Color; Shadow Enable, Shadow Color, Shadow Offset X, Shadow Offset Y, Shadow Blur; Background Color is the color of the frame BACKGROUND, not a plate under the line. There is exactly one outline, and no order at all. Tellingly, a separate third-party plugin exists for multiple outlines |

**Outside the core.** Without the ASS format this feature cannot be described: it is the only place where
the styling stack is not a set of checkboxes but a declared model, and it is the one that explains a whole
class of other products' defects with subtitle plates.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| libass + ASS/SSA format | 0.17.1 (package `libass9 1:0.17.1-2build1`, linked into the system `ffmpeg 6.1.1`) | `BorderStyle`, `Outline`, `Shadow`, `OutlineColour`, `BackColour` | Outline, shadow and plate are not three switches but ONE enum: `BorderStyle` 1 = "outline + shadow", 3 = "opaque plate", 4 (libass only) = a per-line plate where the padding is set by the `Shadow` parameter. Outline and plate are mutually exclusive; when `BorderStyle` ≠ 0, the `Outline` value MUST be zero, otherwise it overrides the mode. The plate color is taken from the outline color / `BackColour`, so the alpha of the "outline color" controls the transparency of the plate |

## The trick behind it

**1. "Outside outline" is not a matter of width but of ORDER; and the vendor hard-coded an exception specifically for text.** The rasterizer lays a stroke along the glyph contour centered: half inside, half outside. The inner half eats the letter's stroke, and on a thin font it eats it entirely — that is why a "thick outline" in an editor with a centered stroke exists only on paper. The cheap way to get an outside outline is known, and it is also the only one: draw the stroke at DOUBLE thickness and place it UNDER the fill, then the inner half is simply hidden. That is exactly why After Effects' corresponding control is not a "stroke position" but "fill and stroke order". And that is exactly why Adobe's one and the same `Center` item behaves differently depending on the object type: over the fill for shapes and under the fill for TEXT. The vendor hard-coded the exception exactly where the eaten half of the stroke destroys readability — and that is the proof that "outline under text" is not a taste but a requirement.

A real outside outline (not an imitation by order) requires computing an offset contour, and this is visible from the fact that Resolve alone can do it: Join Style (Sharp / Rounded / Beveled) and "Adapt Thickness to Perspective". Neither can be obtained by drawing the glyph twice: the double rendering has no notion of joins at all. The cost of the cheap variant is concrete and is visible on thin fonts: the glyph edge is composited TWICE — first the anti-aliased edge of the stroke, then the anti-aliased edge of the fill on top — and on straight (non-premultiplied) alpha the coverage adds up too high, so the contour comes out systematically a little thicker and darker than intended.

**2. The order is a STACK, not a pair, and "fixing" one pair breaks the next.** The live sequence in the Kdenlive tracker: October 2022 — a request to put the outline under the text (#1550); February 2023 — the same thing as a complaint in bugzilla (#465912, closed as FIXED); May 2023, three weeks after the order was changed, — #1689 "shadow is fully covered by the outline". The "text/outline" pair was fixed, and exactly that broke the "outline/shadow" pair. Hence the design of those who went through this path: Resolve does not give a switch, it gives eight numbered elements with sorting by priority and a "closer/farther" slider; Premiere gives up to ten strokes with the rule "the first is drawn over the second". Whoever gives the user a couple of booleans is guaranteed a second complaint.

The third consequence is more expensive than both: **the order is the look of ALREADY MADE projects.** Adobe, when it introduced Outer/Inner in October 2022, kept the old `Center` behavior precisely for compatibility. Kdenlive simply changed the order — and all existing titles began to look different, as can be seen from the same #1689. So the order MUST be a stored value, not a constant of the drawing routine, and this is decided BEFORE the first title, not after the first complaint.

**3. The plate under the line is a separate geometry that cannot be derived from glyph metrics.** In ASS it does not exist as an independent object at all: `BorderStyle` is one enum where outline and plate are mutually exclusive, the plate color is taken from the outline color, and in mode 4 the plate's padding is set by the SHADOW parameter. The practical consequence is caught in the tracker by users' hands: for a subtitle with an empty line it is enough to lower the alpha of the "outline color" below 255 — and the plate falls apart into per-line rectangles (kde-invent #1846), because it is those per-line rectangles of the outline color, which had been merging into one at alpha 255. Resolve is the only one that did this honestly: Border has a Level menu (Text / Line / Word / Character), that is, the area of the plate is CHOSEN. It is impossible in principle to derive it from metrics: the plate's width is the width of the LINE after wrapping plus padding, not the sum of the glyph advances, and it changes with every edit of the text.

## Pros

- The order as a stored property (AE, Premiere, Resolve) makes it possible to fix readability without breaking the look of old projects: a new order is a new value of the field, old titles stay as they were.
- A numbered stack (Resolve, 8 elements; Premiere, up to 10 strokes) does not require a new pair of switches for each next complaint: a third outline and a second shadow are expressed by the same mechanism as the first.
- An outline via order (a double-thickness stroke under the fill) does not require contour geometry — so any raster path can do it, and this is exactly what makes the feature cheap when there is no own shaper.
- A plate as an EXPLICIT area (Level = Text / Line / Word / Character) survives editing of the text: the line became longer — the plate is rebuilt, because it is computed from the line and not set by a number.
- Styling separate from the text means the style is portable: one set "outline+shadow+plate" applies to forty lower thirds.

## Cons

- An outside outline done by order gives double anti-aliasing on the glyph edge: the edge is composited twice, and on a thin font the contour comes out thicker and darker than intended. The honest variant — computing an offset contour — drags in the notion of a join (Sharp/Rounded/Beveled), which a raster path does not have.
- A stack with priorities (Resolve) is the most powerful model and the least understandable: eight numbers and a "Sort By" menu instead of an "outline" checkbox.
- A hard-wired order (FCP, Vegas) cannot be fixed by any setting at all: the sections Face / Outline / Glow / Drop Shadow cannot be rearranged, and the shadow under the outline cannot be removed.
- A single outline (Vegas) means that "white letter — black contour — white rim" cannot be expressed; for it one pays with a third-party plugin.
- A plate made by the outline mode (ASS) does not combine with outline and transparency: the alpha of the plate and the alpha of the outline are one number, and an attempt to make a semi-transparent plate breaks it into per-line rectangles.
- Styling parameters are the least reliably preserved part of a title. In Kdenlive the following were filed separately: outline thickness is not saved (#363340), the outline value is multiplied by ten when a new title is created (kde-invent #1342), a click on one text object changes the shadow setting of another (#476393), shadow parameters revert (#498351).
- Shadow blur grows the bounding box of the object — and where the canvas has stayed the same, the shadow is clipped: `git 16.04: text shadow in title clips gets clipped` (#361899) and `Title shadow blur of 18 or higher causes clipping` (#499550), nine years apart.
- Outline thickness changes the line metrics: in Kdenlive the text MOVED horizontally when the shadow and outline were edited (kde-invent #1340).
- The parameter range is a separate source of discrepancies: for the outline in MLT the slider is 0..3, while in Shotcut, on top of the same filter, it is 0..200 (mlt #976).

## What users say

The problem statement — why this is a bug and not a matter of taste (the task has been open since 2022, twenty comments, labels `Enhancement` + `Titler`):

> When adding an outline to text in title clips, the outline is rendered
> on top of the text rather than below. This severely impacts legibility
> especially at smaller font sizes, and limits the functionality of the
> text outline feature to only very thin outlines before the outlines
> start obscuring and very visibly disrupting the typography.
> — kde-invent #1550, 19.10.2022,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1550

What happened three weeks after the order was changed:

> with latest changes to outline (now not more "inside" the text) the
> shadow is completely "overlapped" by the outline.
> — kde-invent #1689, 07.05.2023,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1689

Three years after the first request, with a named workaround:

> From my experience using Kdenlive, I've noticed that the current
> "Outline" option in the title clip editor only applies inner outlines,
> and unfortunately, there is no setting to choose between inner and
> outer outlines.
> — kde-invent #2043, 28.08.2025,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/2043

A plate made from the outline color — a user's observation:

> But if I choose a "Custom Outline Color" and change the "Alpha channel"
> from 255 to anything lower, the empty line becomes visible i.e. the
> lower part of the subtitle background.
> — kde-invent #1846, 26.03.2024,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1846

The opposite problem — styling hard-wired and impossible to turn off:

> Currently the text in a title clip is shown with a border and shadow
> which can be easily seen by setting the foreground color to transparent
> and the background color to some bright color.
> — pitivi #1860, 04.07.2016,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1860

Double anti-aliasing of the edge as seen by a user (in Flowblade — via the MLT/Qt backdrop):

> I'm getting a green halo around fonts in the titler. I don't get the
> same halo effect in gimp so I'm not sure where the problem is.
> — flowblade #937, 28.11.2020,
> https://github.com/jliljebl/flowblade/issues/937

SIGNAL: text styling (outline/stroke/shadow/halo) — 9 issues in kde-invent, `(outline|stroke|shadow|halo)[^.]{0,30}(text|title|subtitle|caption|font|letter|glyph)|(text|title|subtitle|caption|font|letter|glyph)[^.]{0,30}(outline|stroke|shadow|halo)`; share 0.4% against 0.1% in pitivi

SIGNAL: drawing ORDER as an independent class — 2 issues in kde-invent, `(outline|shadow)[^.]{0,40}(cover|overlap|above|on top|obscur|hidden|wrong layer|below|under)|(cover|overlap|obscur|hidden|above|on top of|wrong layer)[^.]{0,40}(outline|shadow|the text)`; share 0.1% against 0.0% (1 issue of 6346) in kde-bugzilla

The most interesting number is not in the volume but in the fate. By the first filter in kde-bugzilla there are 13 issues, of which 11 are closed, and **10 are closed as FIXED — 91% against 47% of real fixes across the whole tracker** (2568 of 5422 closed). That is, the class gets fixed twice as eagerly as average — and it still does not get closed: the request from 2022 is open in 2026, and in 2025 it was filed anew in both trackers at once (kde-invent #2043, kde-bugzilla #511673, `severity: wishlist`). This is the signature of the stack: each fix moves the problem to the next pair. It is reproduced with the same dump:

```bash
python3 src/scripts/tracker-rank.py --query '(outline|stroke|shadow|halo)[^.]{0,30}(text|title|subtitle|caption|font|letter|glyph)|(text|title|subtitle|caption|font|letter|glyph)[^.]{0,30}(outline|stroke|shadow|halo)'
# shares by tracker; the fix rate is computed from the state/closed_reason fields
# of the same ~/.cache/kinotrain/trackers/kde-bugzilla.jsonl
```

## How it fits Kinotrain
CAPABILITY: openspec/specs/generators/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Outline/shadow/drawing order are parameters of a title on an event; rasterization is pinned by the pins and the vendored Noto (FONT-MANIFEST).
ADDRESS: src/libs/kt-fx/src/video.rs:401 and :658 (mirrors `src/libs/kt-gpu/src/shaders/edges.wgsl`, `glow_add.wgsl`) — the class is possible, and nothing catches it: both of our "decorative" operations work BYPASSING alpha (`edges` returns `Frame { …, has_alpha: false }` at `video.rs:464` and writes `255u` in the shader, `glow` sums only `for c in 0..3` at `video.rs:699` and returns `vec4<u32>(rgb, c.a)`), while `alpha_over` in `src/libs/kt-engine/src/video.rs:997` composites straight alpha and multiplies the source RGB by its alpha — so any outline or shadow assembled from these two kernels is INVISIBLE outside the glyph, and no test notices: `src/libs/kt-fx/tests/video_fx.rs:34` builds the reference frame as `has_alpha: false`, and `src/libs/kt-gpu/tests/parity.rs:285` runs ALL conformance cases of t181 on `frame(W, H, false, 0)`.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages, and ONLY inside the round F-TXT-001: (1) the styling model and the order as a stored list; (2) raster operations over alpha — silhouette dilation, offset blurred copy, per-line plate; (3) the editor in the UI |
| Where the main work is | kt-media (styling inside the title `MediaSource`, next to `placeholder.rs`), kt-model (order and parameters as a non-numeric field), kt-app (the editor); kt-fx and kt-gpu are NOT touched, unless styling becomes a layer of the plan |
| Project format | an order field is needed — a stored list, not a constant. It travels in the same envelope as the title itself (F-TXT-001) and does not require a separate version |
| Tests to write | determinism of styling (a relative of t208: the same parameters → the same bytes); order as a value — a permutation changes the frame predictably; an outline does not eat the stroke (a thickness check on a thin font); the plate is rebuilt when the text is edited; **and separately, independently of titles** — a run of video FX over a frame with alpha in `video_fx.rs` and in t181, absent today |
| Depends on | F-TXT-001 hard (without text there is nothing to style). Related: F-TXT-004 (a subtitle has the same stack, but its own model), F-TXT-003 (a plate-backdrop as a separate clip — a different way to solve the same readability), F-MED-003 (material with alpha — the same unverified path) |
| Breaks compatibility | no; but the order MUST appear in the format AT THE SAME TIME as the title, otherwise it will break later |

## Verdict

We take it, but **not as a separate feature: as one stage inside F-TXT-001**. A title
without this layer is unreadable exactly where titles are needed — over a real
frame; the workaround at neighbors' is known and named aloud ("make a PNG in
GIMP and import it", kde-invent #2043), and that is exactly the price for which the catalog was started.

From the reconnaissance follows exactly one decision that cannot be made on the fly: **the drawing order is a stored value, not a constant of the drawing routine, and a list, not a pair of booleans.** The justification is not aesthetic:
Kdenlive changed the order with a constant and three weeks later got a complaint about the next pair, and the look of all previously made titles changed; Adobe made the same step a field and kept the old behavior for old projects. It costs one field in the format if done at once, and incompatibility if done later.

The second decision, also before the code: **a sixth library in
`src/libs/kt-ffmpeg/src/loader.rs` does not appear.** libass sits in the
process within arm's reach and can do everything this note is about —
and that is precisely why the prohibition has to be written in words: it does not follow from
any existing invariant, and its consequence (different bytes on different
machines) will show up not at compilation but half a year later on someone else's render.

From our kernels there is nothing to reuse: `Edges` computes brightness and kills alpha, `Glow` sums RGB and does not extend alpha, and outline/shadow are operations over ALPHA, of which we have not a single one. Reusable are the discipline (an integer kernel + AVX2 + a WGSL mirror + the t181 tolerance) and downscale blur. Separately and independently of titles: running video FX over a frame with alpha is not covered by a single test — this has to be fixed in any case.

**Done in R.52 S3 (item 406):** the per-line plate, a box-blur shadow of alpha,
an outline by exact EDT with the outside done by ORDER, `order` — a stored field of
the format from day one. The layer-order editor in the UI is deferred (the default
plate→shadow→stroke→fill covers the norm; the field is already in the file, so the UI
will be added without migration).

## Sources

- https://invent.kde.org/multimedia/kdenlive/-/work_items/1550 (accessed 2026-09-08) — problem statement, open since 19.10.2022, 20 comments
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1689 (2026-09-08) — the next pair broke three weeks after the first was fixed
- https://invent.kde.org/multimedia/kdenlive/-/work_items/2043 (2026-09-08) — the request repeated in 2025, a workaround via GIMP named
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1846 (2026-09-08) — a subtitle plate made from the outline color
- https://invent.kde.org/multimedia/kdenlive/-/work_items/222 (2026-09-08) — "Shadows of titles are applied on the wrong "layer"", 2019, the same class three years before #1550
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1340 (2026-09-08) — outline thickness moves the text horizontally
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1342 (2026-09-08) — the outline value is multiplied by ten for a new title
- https://bugs.kde.org/show_bug.cgi?id=465912 (2026-09-08) — "Title outline effect covers up the text itself", RESOLVED/FIXED
- https://bugs.kde.org/show_bug.cgi?id=511673 (2026-09-08) — the same request anew, 05.11.2025, `severity: wishlist`
- https://bugs.kde.org/show_bug.cgi?id=361899 and https://bugs.kde.org/show_bug.cgi?id=499550 (2026-09-08) — shadow clipped by the bounding box, 2016 and 2025
- https://bugs.kde.org/show_bug.cgi?id=476393, https://bugs.kde.org/show_bug.cgi?id=498351, https://bugs.kde.org/show_bug.cgi?id=363340 (2026-09-08) — the cluster "styling parameters do not persist"
- https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1860 and https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2312 (2026-09-08) — styling hard-wired, and a request to open shadow control
- https://github.com/jliljebl/flowblade/issues/937 (2026-09-08) — "Green Halo around Fonts"
- https://github.com/jliljebl/flowblade/issues/706 (2026-09-08) — shadow blur in pixels
- https://github.com/olive-editor/olive/issues/963 (2026-09-08) — the outline was dropped during a rewrite, a request to restore it
- https://github.com/olive-editor/olive/issues/490 and https://github.com/olive-editor/olive/issues/607 (2026-09-08) — shadow direction; "Shadow Angle" active while the shadow is off
- https://github.com/mltframework/mlt/issues/976 (2026-09-08) — outline range 0..3 in MLT against 0..200 in Shotcut
- https://github.com/OpenShot/openshot-qt/issues/2843 (2026-09-08) — the outline is enabled where it was not requested
- https://jayaretv.com/fusion/text-node/ (2026-09-08) — the Shading tab of Text+: eight elements, "Sort By" Priority/Z-depth, Priority Back/Front, element kinds, Join Style, Adapt Thickness to Perspective, the Level menu (Text/Line/Word/Character)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/8-design/texttitles_plugin.htm (2026-09-08) — the "Titles & Text" parameters in Vegas Pro 2026
- https://support.apple.com/guide/final-cut-pro/adjust-titles-ver4e32ca5/mac (2026-09-08) — the four sections of the Text inspector: Face / Outline / Glow / Drop Shadow and their parameters
- https://community.adobe.com/t5/premiere-pro-beta-discussions/now-released-inner-outer-strokes-for-premiere-pro-titles/m-p/13223993 (2026-09-08) — Inner/Outer in the October 2022 release and the retention of the former `Center` behavior for the sake of old projects
- https://github.com/libass/libass/wiki/ASS-File-Format-Guide (2026-09-08) — `BorderStyle` 1/3/4, mutual exclusivity with `Outline`, `Shadow` as the plate padding in mode 4
- **[NOT VERIFIED against the primary source]** `helpx.adobe.com/premiere-pro/using/work-with-fill-stroke-background-shadow.html` (Outer/Inner/Center and "over the fill for shapes, but under the fill for text") and `helpx.adobe.com/after-effects/using/formatting-characters-character-panel.html` ("All Fills Over All Strokes", the limitation with per-character 3D): `helpx.adobe.com` from this environment returns "proxy refused the connection" (the same was recorded in the first wave in `_editors.md`), the wording is taken from search-result snippets of these same pages
- **State of the dumps as of 2026-09-08:** the export is closed, 12 dumps of 12 (28 537 records; 11 in the ranking — 25 442, ffmpeg-trac is outside it). The count below was re-checked on the full corpus: `blender-vse` and `ffmpeg-trac` gave no hits for this filter. This changes nothing for the subject of the note: both are outside the class of interest
- Internal sources: `src/libs/kt-fx/src/video.rs`, `src/libs/kt-gpu/src/shaders/edges.wgsl`, `src/libs/kt-gpu/src/shaders/glow_add.wgsl`, `src/libs/kt-engine/src/video.rs`, `src/libs/kt-engine/src/video_plan.rs`, `src/libs/kt-ffmpeg/src/loader.rs`, `src/libs/kt-media/src/placeholder.rs`, `src/libs/kt-model/src/project.rs`, `src/libs/kt-fx/tests/video_fx.rs`, `src/libs/kt-gpu/tests/parity.rs`, `src/libs/kt-engine/tests/golden_export.rs`; neighboring notes F-TXT-001, F-TXT-003, F-TXT-004, F-MED-003
