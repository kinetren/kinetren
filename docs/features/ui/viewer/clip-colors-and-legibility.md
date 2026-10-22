---
id: F-UI-005
title: Clip colour as data and as decoration
status: candidate
priority: B
cost: M
category: ui/viewer
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app, kt-model, kt-otio]
invariants: [12]
ui_rules: [13]
round: null
point: null
tests: []
depends_on: []
trackers: [kde-invent, kde-bugzilla, flowblade, pitivi, olive, openshot, losslesscut, otio]
checked: 2026-09-08
---

# F-UI-005 — Clip colour as data and as decoration

## Essence

Two different things live in one rectangle on the timeline. The first is a **mark that the editor sets**: "this take is rejected", "this is an interview", "come back here". This is project data, it survives saving and travels to another program. The second is **appearance**: what lets the editor tell video from audio, selected from unselected, a missing source from a whole one. This is a theme, and it belongs to the program, not to the project.

They are confused because both are expressed by filling the same pixel. And on this very same area the clip's name, filmstrip and waveform are drawn — the very things for which people look at the clip.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Track Display Color | The event has no mark of its own: "The color of the event corresponds to the color of the track header"; the color is chosen for the TRACK, and the color set is edited in Preferences → Display |
| Adobe Premiere | 26.x | Label | 16 colors, each with an editable NAME (Preferences → Labels: Label Colors + Label Defaults by media type); a label exists both on a bin item and on an event on the timeline, and the "Show Source Clip Name and Label" switch decides WHOSE label to show |
| DaVinci Resolve | 21 | Clip Color · Flag · Marker | Three separate mechanisms, separated by SCOPE OF ACTION: a flag lives on the source and colors all its instances in the timeline and is filterable; a marker is a frame inside an event; Clip Color is declared a visual separation of neighbours and does not take part in filters |
| Final Cut Pro | 12.3 | Roles | There is no separate label at all: a clip is colored with the color of its ROLE ("Roles are metadata text and color labels"), five roles by default (Video, Titles, Dialogue, Music, Effects), and the color of a role is changed in the role editor — an anonymous color is impossible in principle |
| After Effects | 26.x | Label | A Label column with a swatch + the layer bar in the same color; 16 labels with editable names; `Edit > Label > Select Label Group` turns the color into a QUERY for selection; label names are visible only if the column is widened |

**Outside the core.** Without the reference library OTIO one cannot say where the color data
LIVES: our project format is its schema, and in 2025 it closed this question.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| OpenTimelineIO | 0.18 | `Item.color` | A field on `Item` (the parent of Clip and Track), optional; the value is `Color`: four doubles RGBA **plus an optional `name`**, `from_hex`/`to_hex`, a set of ready-made constants. It is serialized as a primitive, not as an object — so that the adapter itself decides whether to put there a named label of a foreign NLE or raw RGB |
| Kdenlive | 26.08 | clip type colors | hard-coded into the code and derived from the application's palette (`timelinecontroller.cpp`); configurability is an open request kde-invent #2146 (2026-03-01) |

## The trick behind it

**1. In a mature mechanism a color is never anonymous.** A label lives in a pair with a name not for beauty: a color remembers its meaning only as long as the author remembers it. In Premiere the name of a label is edited directly in the settings; in Final Cut a color separate from the role does not exist — it is the role's projection; and when OTIO, two years into a dispute, closed issue #1614, the schema got not an RGB but a `Color` with a `name` field INSIDE the value. Three independent decisions about one thing: a color is a caption that has paint, not paint that sometimes has a caption.

**2. Resolve separated colors not by appearance but by SCOPE OF ACTION — and honestly denied meaning to one of the three.** The flag belongs to the source and therefore colors all its instances at once (it answers the question "where else is this take"), the marker belongs to the frame, and Clip Color belongs to the event — and it is stated outright that it serves the visual separation of neighbours and does not go into filters. That is, one of the three colored mechanisms is declared DECORATION, and this is written in the help, not guessed. Exactly the boundary that requests of the form "give us a palette" erase.

**3. A color has an owner, and there are two owners.** One and the same material is a bin item and an event on the timeline; the label may be on both and may not match. Premiere moved this into a view switch ("Show Source Clip Name and Label"), that is, it acknowledged that the question "whose color are you seeing right now" is a permanent one, not a one-time one. A fill derived from the SOURCE and a label placed ON THE CLIP are different channels, and they cannot be drawn in one paint.

## Pros

- A label with a name turns into a selection operator: `Select Label Group` in AE selects the whole group in one
  action — by colour you can not only look but also search. A colour without a name cannot do this: it cannot be
  expressed in a filter.
- The flag's scope is the source, so "where else is this duplicate" is answered without searching the bin: mark
  one instance and all of them take the colour.
- With roles (FCP) the meaning cannot fall out of sync with the colour: the colour is computed from metadata, not
  painted by hand, so it does not go stale when the project is edited.
- With OTIO 0.18 colour data has a STANDARD place (`Item.color`) — it is carried between programs, rather than
  settling in private metadata.

## Cons

- Colour takes area away from the content. In Final Cut, to see role colours on video, clips have to be EXPANDED
  (otherwise the colour is not shown); in AE the label name is not visible by default — the column has to be
  widened. The price of a "bright palette" is the same pixels on which the waveform and the filmstrip live.
- An algorithmic palette plus a fixed foreground = unbounded spread of contrast. As soon as colours are generated
  and the indicators over them are drawn in one paint, some combinations become unreadable, and no palette edit
  cures this — only making the foreground depend on the background does.
- Colour as the ONLY channel of distinction excludes some users and some situations (dark/light theme, colour
  blindness, a cheap matrix). Therefore for states that must not be missed, colour has to be duplicated by a shape
  or a caption.
- A label that has no name means nothing a week later: the meaning is stored in the author's head, not in the project.

## What users say

SIGNAL: clip colour as a setting, not as a defect — 2 tasks in flowblade, `(timeline|clips?|events?|tracks?|nodes?)[^.]{0,22}colou?rs\b|colou?rs\b[^.]{0,18}(for|of|on|to)[^.]{0,14}(clips?|timeline|tracks?|nodes?)|colou?r.?(cod(ed|ing)|tag|palette|scheme)[^.]{0,25}(clip|timeline|track)|(clip|timeline|track)[^.]{0,25}colou?r.?(cod(ed|ing)|tag|palette)`; share 0.2% against 0.0% (none of 6346) in kde-bugzilla

The filter gives nine tasks across five trackers (flowblade 2, kde-invent 2, pitivi 3, olive 1, openshot 1) — and not one in kde-bugzilla, the largest tracker of COMPLAINTS. All nine are requests. This is the finding: "the clip colour is wrong" cannot be filed as a defect, because there is nothing to violate; the subject exists only where wishes are accepted. The share is twice as high at flowblade — the only one of the five where a user clip colour ALREADY exists, and both tasks there are not about "give us a palette" but about the shades in it not being consistent with one another.

The request with which everything begins is always triple — palette, label and readability in one text:

> Bonus: * Make the audio waveforms clearer. * Add ability to color tag clips.
> — kde-invent #445 "Improve timeline clips color palette", 20.11.2019 (70
> comments, closed 21.04.2024), https://invent.kde.org/multimedia/kdenlive/-/work_items/445

The price of a generated palette, verbatim:

> Observe that the the white timeline & playback indicators against these
> high-lightness colours are not easily visible due to the contrast ratio.
> — losslesscut #1199 "Segment colour scheme generate results in poor contrast
> ratio", 18.06.2022, open, https://github.com/mifi/lossless-cut/issues/1199

What a caption does to the content beneath it (closed as INVALID — that is, not even recognised as a subject):

> clip labels in tracks should be in clip centres because putting them right next
> to clip start obscures audio signal
> — kde-bugzilla #480108, 20.01.2024, RESOLVED INVALID,
> https://bugs.kde.org/show_bug.cgi?id=480108

The same pain two years later and already with a working scenario — finding the silence at the start of a clip in order to cut it:

> i can't see the audio waveform to correctly CUT a video piece (usually there is
> an empty without voice space at every clip start) i need to cut this little
> piece of "no voice".
> — kde-invent #2179, 01.05.2026, open,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/2179

And a colour without a name, through the eyes of someone who sees it for the first time (closed by its author after nine minutes):

> What do colors on Timeline mean?
> — olive #1208, 21.09.2020, https://github.com/olive-editor/olive/issues/1208

From the side of the core editors — Adobe's announcement that the question "whose colour to show" became a view switch:

> Now, this option has been removed from the Project Settings and can be found as
> a view option in the wrench menu in the Timeline panel called "Show Source Clip
> Name and Label".
> — Adobe Premiere Pro Beta Discussions, 31.03.2021,
> https://community.adobe.com/t5/premiere-pro-beta-discussions/discuss-new-way-to-control-how-label-colors-and-clip-names-are-displayed-in-the-timeline/td-p/11940375

## How it fits Kinotrain

### What already exists

- **The clip fill is a hash of the PATH**: `clip_color()` in `src/apps/kt-app/src/timeline.rs` (a port of
  `colorFromPath` from v1) gives `80 + h%120` per channel. It is not a theme and not a label, but a third thing:
  the channel of the SOURCE'S IDENTITY. It already works in two places — the event fill and the translucent
  transition handles (the closure `handle_fill` there), where it shows whose material continues across the join.
- **Audio is dimmed ×0.55 for the sake of the waveform** (`fill.gamma_multiply(0.55)`) — that is, the conflict
  "colour against content" is already recognised with us and was once resolved in favour of the content.
- **The stereo waveform in two colours** L/R (`col_l`/`col_r` in the "stereo waveform" block) — exactly what is asked
  for in kde-invent #2095 ("the same color for the waveform make the L R just
  confusing"); here we are ahead of the source.
- **The clip name** — white on a black plate at alpha 140 in the top-left corner (the "clip name on a plate" block),
  dimmed for the audio half of an A/V pair (`names()` in `src/apps/kt-app/src/app.rs`). The plate is our answer to
  openshot #1425 ("glow behind track name makes it unreadable"), dimming the half is the answer to "a mess of letters".
- **A missing source** is painted with two channels at once — the fill and the outline (the `if is_offline` branch),
  not with one colour.
- **The only real user label we have is on the AREA**: `MarkedArea.color` (`src/libs/kt-model/src/project.rs`) and
  the palette of five colours `AREA_PALETTE` in `src/apps/kt-app/src/theme.rs`. It was introduced by R.38 item 1
  precisely because the field existed in the format, but the UI for it did not.
- **`Marker.color`** (`src/libs/kt-model/src/project.rs`) — an OTIO enum ("GREEN" by default, `Marker::default_color()`
  in `src/libs/kt-otio/src/schema.rs`): round-trip of foreign files exists, but all markers are drawn in amber (`amber`
  in the "markers" block of the same file). The same defect that R.38 item 1 closed for areas remains open for markers.
- **One theme, dark**: `Visuals::dark()` in `theme::apply()`, there is no light one at all — half of someone else's pain
  ("changed the theme — lost the waveform", kde-invent #40) is absent with us by decision, not by chance.
- **A foreign `Item.color` is not lost**: `Clip` in `src/libs/kt-otio/src/schema.rs` has `#[serde(flatten)] extra`, so a
  file from OTIO 0.18 passes through (t145) — we simply do not show it.

### What is missing

- `ClipEvent` (`src/libs/kt-model/src/project.rs`) has neither a colour field nor its own name:
  there is physically nowhere to put a label, and the name is always the file name.
- The bin and the timeline are not linked by colour: `bin_panel.rs` does not call `clip_color` at
  all, so the question "whose colour to show" does not even arise with us — there is nothing to show.
- Not a single contrast check. `t100` counts glyphs, `t128` measures the width of a caption; nobody
  looks at colour.

### What gets in the way

Not the volume of code, but the **occupancy of channels**. On the clip colour already speaks: the fill
is the identity of the source, the white outline is selection, the translucent white is hover, red is
offline, the yellow line is level, amber is a marker, turquoise is an area, purple is the morpher. There
is no free colour for a user label, and rule 13 of the UI checklist and the direct requirement of
R.13 item 2 (`docs/user-feedback.md`) forbid opening a new strip for it:

> "I did not say that one may not drive onto the clips — I said not to make a mess out of letters…
> you could at least have guessed to write track names EXACTLY under the names of the clips themselves."

So the label must sit on an ALREADY occupied place (the clip's edge, a break in the outline, the name
plate), and must not take pixels away from the waveform and the filmstrip.

### Which invariants it touches

- **12** (repaint only on movement, no allocations, no clones in draw paths): the label colour must land in
  the same cache keyed by `model_rev` as `names()`, and not be computed per frame; the palette must not add
  shapes — the label fill is the same single rectangle.
- **Rule 13 of the UI checklist** — a new strip under the label is impossible.
- Format: `Item.color` is a STANDARD OTIO field, and the old build preserves it through `extra`. Under the
  criterion "version by content" (`kt-otio/CLAUDE.md`, item 189: one Pan3d does not bump the version, because
  an old reader has foreign passthrough) our `kt.v` does not need bumping.

ADDRESS: src/apps/kt-app/src/timeline.rs, `clip_color()` — possible, and nothing catches it: the event fill is taken from the hash of the path (80..199 per channel), and everything drawn on top is painted with fixed colours; the contrast of white against the fill wanders from 8.06:1 down to 1.69:1, and below 3:1 for 40% of the cube of `clip_color` values, so a 2-pixel white selection outline on a light hash disappears — this is the class of losslesscut #1199, and not one of our tests measures colour.

## Estimate

| What | Answer |
|---|---|
| Volume | M — 2–3 stages of a round |
| Where the main work is | kt-model (the label field: colour + name on `ClipEvent`), kt-app (the picker, drawing on an occupied place, the contrast guard), kt-otio (`Item.color` outward and inward) |
| Project format | our `kt.v` is not touched: `Item.color` is a standard OTIO 0.18 field, the old build keeps it through `extra` |
| Tests that will have to be written | contrast of foreground against fill across the whole `clip_color` cube (a threshold for both the white outline and the waveform); round-trip of `Item.color` with a name and without; a layout test "the label took not a single pixel from the waveform"; t100 on label names |
| Depends on | nothing; adjacent — F-MED-001 (smart folders and tagging: the label must get into search) and F-UI-001 |
| Breaks compatibility | no |

## Verdict

Take it, but as TWO things, not one. **The first — a label-data on the event: colour plus name**, because
all three mature mechanisms (Premiere, FCP, OTIO) converged on the point that an anonymous colour is rubbish
after a week, and the place in the format is already standard. **The second — not a feature but a guard**:
the contrast of the foreground against the fill must be measured by a test, because the fill with us is chosen
by a hash, not by a person, and on 40% of the values the white selection is not readable on it. The first without
the second will do exactly what losslesscut #1199 complains about.

What we do NOT take: a customisable palette of clip types (kde-invent #2146) — we have one dark theme by decision,
and the difference between video/audio/morpher/camera is already carried by the tracks; and colour as the only
channel of state — offline already speaks with us through two channels, and this rule stays.

## Sources

- kde-invent #445 "Improve timeline clips color palette", https://invent.kde.org/multimedia/kdenlive/-/work_items/445 (accessed 2026-09-08)
- kde-invent #2146, #2179, #2095, #40 — https://invent.kde.org/multimedia/kdenlive/-/work_items/2146 · /2179 · /2095 · /40 (2026-09-08)
- kde-bugzilla #480108, https://bugs.kde.org/show_bug.cgi?id=480108 (2026-09-08)
- losslesscut #1199, https://github.com/mifi/lossless-cut/issues/1199 (2026-09-08)
- olive #1208 and #1384, https://github.com/olive-editor/olive/issues/1208 · /1384 (2026-09-08)
- openshot #1425 "Remove timeline clutter", https://github.com/OpenShot/openshot-qt/issues/1425 (2026-09-08)
- flowblade #1045 and #1141, https://github.com/jliljebl/flowblade/issues/1045 · /1141 (2026-09-08)
- pitivi #269, #1328, #2270, https://gitlab.gnome.org/GNOME/pitivi/-/work_items/269 · /1328 · /2270 (2026-09-08)
- otio #1614 and PR #1887/#1915 (`Item.color`, merged 11.08.2025, milestone Public Beta 18), https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/1614 (2026-09-08)
- The numbers are reproducible: `python3 src/scripts/tracker-rank.py --query '<SIGNAL filter>'`; dumps in `~/.cache/kinotrain/trackers/` (not committed to the repository)
- VEGAS Pro, help: https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/events_intro.htm (2026-09-08). The portal serves the 22 line's pages; 2026.0 pages were not found publicly, the mechanism has not changed in the documentation since then
- Adobe Premiere, Labels preferences: https://helpx.adobe.com/premiere/desktop/get-started/preferences-and-settings/labels-preferences.html and the beta discussion (link above), 2026-09-08
- Apple, Final Cut Pro — roles and their colours: https://support.apple.com/guide/final-cut-pro/intro-to-roles-verb71cbcbe/mac and https://support.apple.com/guide/final-cut-pro/change-role-colors-or-names-verbd20e0664/mac (2026-09-08)
- Adobe After Effects — layer labels: https://helpx.adobe.com/after-effects/using/layers.html (2026-09-08)
- Resolve: the mechanism (a flag on the source, a marker on the frame, Clip Color without filtering) is taken from the DaVinci Resolve manual and secondary analyses; the manual's mirror (`steakunderwater.com`) returned 404, and `forum.blackmagicdesign.com` — 403, as in the first wave. Therefore there is no verbatim quote for Resolve
- The old VEGAS forum (`vegascreativesoftware.info`, thread "Color coding individual clips") now redirects to `forum.borisfx.com` without preserving threads — a change of publisher recorded in `_editors.md`; a quote from there could not be obtained
