---
id: F-CLR-006
title: Node graph versus linear effect chain
status: rejected
priority: C
cost: XL
category: color/workflow
editors: [resolve-20, premiere-26, fcp-12, vegas-pro-2026, after-effects-26]
we_have: no
crates: [kt-model, kt-fx, kt-engine, kt-gpu, kt-cache, kt-otio, kt-app]
invariants: [4, 7, 11]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: [F-MPH-005, F-MPH-006]
checked: 2026-09-07
---

# F-CLR-006 — Node graph versus linear effect chain

## Essence

The fork is in WHAT describes the processing of a clip. In a chain (Premiere, VEGAS,
FCP, us) this is an ordered list: each effect takes the result of the previous one,
the order is changed by dragging, and there is nothing to branch. In a graph (Resolve, Fusion,
Nuke) it is a scheme of nodes and wires: a node has a picture input, a picture output and a
SEPARATE key input/output, nodes can be placed in parallel, their results can be mixed
with blend modes, and a matte can be pulled from one place of the scheme to another.
For the editor the difference is visible not in the list of capabilities but in which
tasks are formulated in one gesture and which are not formulated at all.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| DaVinci Resolve | 20.3 | Node Editor (Color page); separately the Fusion page | a directed graph of Corrector nodes: RGB input/output **plus a separate key input and key output**; ready-made structures Serial / Parallel Mixer / Layer Mixer / Outside / Key Mixer / Splitter-Combiner / Shared; on top of the graph — a FIXED stack: pre-clip, L1…L4, post-clip, Timeline |
| Adobe Premiere | 26.x | Effect Controls | a linear stack of effects on a clip, the order changed by dragging; several Lumetri instances on one clip; the mask lives INSIDE the effect, "one mask per effect"; a matte from another element — only the Track Matte Key from another track |
| Final Cut Pro | 12.3 | Video inspector + Color inspector | there is no graph, but there are TWO lists for one clip: corrections are selected in the Color inspector and at the same time stand as rows in the Effects list of the Video inspector; the order is changed by dragging, Balance/Match are pinned to the top, built-in effects cannot be reordered |
| VEGAS Pro | 2026.0 | Plug-In Chain | a linear chain (up to 32 plug-ins per event) at four levels of application — F-MPH-005; there is no branching at any level |
| After Effects | 26.x | effect stack on a layer | there is no graph, but a parameter of the "layer" type (Set Matte, Mask Reference in Compositing Options — see F-MPH-002), track mattes and precomps give references between layers: there are links, there is no editor of links |

## The trick behind it

**1. A matte in the graph is a FULL SIGNAL, not a parameter inside an effect.**
Each Resolve Corrector node has its key input and key output separate from the picture:
"Each node's key input and key output makes it possible to route key channel
data from one node to another" (Reference Manual 20, ch. 146, "Introduction to
Manipulating and Combining Keys"). Hence the trick that is not formulated at all in a chain:
put a node whose RGB input is taken from the VERY START of the tree, it passes the picture
nowhere, and its key output is plugged into a node at the end — and the key is pulled from the
clean frame, "regardless of what's happening to the picture in between" (ch. 146, "Feeding Keys From One
Node to Another"). The manual explains directly why this works: "Keep in mind
that a key is just a grayscale image" (ch. 146, "Connecting Key Outputs to RGB
Inputs, and Vice Versa") — so a key can be fed into the RGB input of another node,
processed with curves, blur or denoise, and returned to the key input of a third. The Key Mixer
adds, subtracts and intersects keys from different nodes. In a chain the mask is a field
inside the effect: in Premiere 26 "You can only apply one mask to one effect", and one
mask cannot be shared between two effects by any means other than a copy,
which then lives its own life.

**2. The Outside node is not a "second mask" but ONE CHECKBOX on another key.** The manual
strips it of all halo: "Outside nodes are really just Corrector nodes with
the Key Palette's Key Input Invert control enabled" (ch. 146, "Outside
Nodes"). Two things follow from this that do not exist in a chain. First: the inverse area
does not require a second mask and cannot drift away from it — you adjust the window in
the source node, and both corrections move. Second: if the inversion checkbox is UNCHECKED,
you get a second correction of THE SAME area — "If, instead of using the Outside
node to invert the incoming key, you want to copy the existing key in order to
perform another operation to the same isolated region, you can disable the key
input's Invert control" (ibid.). That is, "inside/outside" is a state of one
signal, not two objects that the editor has to keep in sync by hand.

**3. The strength slider of a branch lives in the KEY palette, not in the effect.** Key Output
Gain: "lowering this value reduces the strength of that node's contribution to
the Layer Mixer" (ch. 145, "Adjusting Layer Node Strength Using Key Output
Gain"), default 1.00, and the same parameter works in the Parallel Mixer.
The practical consequence is visible only to those who have worked with it: a node
is turned into a Shared one (the edit spreads across all clips where it sits), and next to it
a EMPTY layer node is hung, and its output gain becomes a personal
opacity of the shared look on each frame separately. A shared object and
an individual blend strength — this is exactly what our morpher lacks
(F-MPH-006), and in a graph it comes not as a separate feature but as a side effect of
the weight of a branch being a property of its key channel.

## Pros

- The Parallel Mixer feeds ALL branches ONE AND THE SAME picture state: "This
  makes it easy to apply multiple secondary operations without worrying about
  whether or not a change to one will affect the keys of the others"
  (ch. 145, "Parallel Node Structures"). In a chain the second secondary keys off a
  frame already spoiled by the first, and an edit of the first breaks the selection of the second.
- The Layer Mixer gives blend modes BETWEEN BRANCHES OF ONE CLIP: the same list of composite
  modes as for clips on the timeline (ch. 145, "Using Composite
  Modes With the Layer Mixer"). A glow is made by a pair "sharp curve + blur"
  through Add inside the clip, without a second copy of the clip on a neighbouring track and without
  a second decode of the source.
- The priority in the Layer Mixer is set by the ORDER OF INPUTS, not by the order of operations:
  the lower input overlaps the upper ones (ch. 145, "Layer Mixer Prioritization"),
  so "skin on top of stylization" means re-routing a wire, not rearranging half of the chain.
- A key pulled once feeds any number of corrections; an edit of the qualifier
  spreads to all consumers at once.
- Shared Node: "Changes made to a Shared node are automatically rippled to all
  other instances of that node in the grades of other clips" (ch. 142, "What
  Are Shared Nodes Good For?"), and shared and personal nodes can be mixed in
  any order. Copy-paste of a chain cannot do this in principle: the copies diverge
  with the very first edit.
- A node without a picture output is a legitimate object. A key generator is not obliged to be
  someone's effect, so it can be placed anywhere and fed by any state of the frame.

## Cons

- The order of operations stops being readable. In a chain "the last in the list is the last
  in count"; in a graph with two mixers and a key branch the answer to the
  question "why is the picture like this" requires reading the whole scheme, and from a screenshot of two
  similar trees the result cannot be predicted: in the Layer Mixer everything is decided by
  WHICH input the wire is plugged into.
- The number of nodes has become a meme (see the quotes): the freedom to branch turns into
  trees of 22–37 nodes, most of which are "default & not touched".
- The graph does not organize the work itself. Resolve had to put a LINEAR stack on top of the graph —
  Node Stack Layers, "applied sequentially (in order from
  lowest to highest)" (ch. 143, "Using Node Stack Layers"), with the number of
  layers set in the PROJECT settings. The price is honestly recorded there too: "If
  for some reason you return to the settings and lower the amount of node
  stack layers later, all the grades in those layers are removed across your
  entire project". That is, the flagship of nodes
  arranges the real work into fixed levels — exactly like VEGAS
  (F-MPH-005), only there are six levels and they are configurable.
- The entry threshold is paid not in knowing the buttons but in the need to unsee layers:
  a newcomer lays the nodes out in a row, gets "almost the same thing" and for years
  does not learn why a second input exists at all.
- Memoization gets more expensive: a chain has one result per clip, a graph has a result per
  node, and a node with two consumers (Shared, a key branch) is read twice
  and stored once.
- Even Resolve has TWO graphs, with different sets of nodes and different rules
  (Color and Fusion), and a scheme cannot be transferred between them.

## What users say

> 22 nodes for what amounts to "decrease exposure, crank blue". This could be
> done in a single node, or three if you want to be fancy about it.
> — u/ore_wa_kuma, r/colorists, 22.06.2026, https://www.reddit.com/r/colorists/comments/1u9w0j4/dayfornight_grade_practice_looking_for_feedback/ot36nyq/

> Also idk who made this power grade, but 28 nodes is disgusting honestly.
> — u/Namisaur, r/davinciresolve, 03.09.2026, https://www.reddit.com/r/davinciresolve/comments/1w5o1rj/can_anyone_explain_what_im_looking_at/p7hy62d/

> I'm also completely new to Resolve and tutorials using 4–5 nodes are quite
> confusing.
> — u/Kevin_Blackberry93, r/davinciresolve, 26.08.2026, https://www.reddit.com/r/davinciresolve/comments/1vyixyx/struggling_with_dlog_m_color_gradin_is_there_an/p5xrhrh/

> There's nothing wrong with thinking of nodes as layers, at least to start
> out. If you just use them one after the other, that's basically how they're
> functioning.
> — u/Legomoron, r/colorists, 12.07.2026, https://www.reddit.com/r/colorists/comments/1utrv88/need_help_with_colour_matching/owzr44i/

> What I do is having a node that is only connected to input with no output. I
> can make a qualification inside of it and then use the alpha out connected to
> a node in my node tree.
> — u/Adridulte, r/colorists, 21.06.2026, https://www.reddit.com/r/colorists/comments/1ubvd22/secondaries_before_primaries/osznna1/

> If I need to alter the image before it goes to my qualification node, I can
> add a node before and do an adjustment that will only affect the
> qualification and not the rest of the grade.
> — u/Adridulte, there, 21.06.2026, https://www.reddit.com/r/colorists/comments/1ubvd22/secondaries_before_primaries/osznna1/

> For each shared node in that tree I also add a layer node which is left
> blank. Altering the output gain of this blank layer node becomes an opacity
> control, which can be altered on a shot by shot basis.
> — u/useless_farmoid, r/colorists, 12.05.2026, https://www.reddit.com/r/colorists/comments/1tafmf9/look_dev_questions/olbnuwf/

This should be read as follows. Negative feedback about the graph almost never sounds like
"nodes are worse than layers" — it sounds like "you have too many nodes". People complain not about the mechanism,
but about the freedom that the mechanism gives; across five hundred-odd comments in r/colorists and r/davinciresolve
not a single "went back to layers" came up. But the reverse movement does occur: a person comes from Photoshop,
lays the nodes out as layers, and is told that this is fine too — "If you just
use them one after the other, that's basically how they're functioning". This is
the honest measure of the graph's usefulness: 90% of a real tree is a linear chain,
and the whole gain sits in a few places where a second input is needed.

## How it fits Kinotrain
CAPABILITY: openspec/specs/video-fx/ — the contract is in `spec.md`, the "why" and code paths are in `design.md`.
Rejected: we do not take node grading — the chain of effects on an event and the morpher layers cover the cases, and a node graph breaks the invariant "one composition code".

## Estimate

| What | Answer |
|---|---|
| Size | XL — and not one round: the way FX are executed is rewritten in BOTH backends, the post-FX cache key, the project format and the effects editor |
| Where the main work is | kt-fx + kt-engine (a graph interpreter instead of a loop), kt-gpu (a second such one), kt-cache (key per node, not per chain), kt-otio (a DAG outside the OTIO specification), kt-app (a canvas instead of a strip) |
| Project format | incompatible: `Item.effects` is a list, the DAG goes wholesale into `metadata.kt`, passthrough loses its meaning; version v5+ is mandatory |
| Tests that will have to be written | from scratch — the whole kt-fx/video.rs block (video_fx, t68, t228), postfx_cache t66/t66c, golden_export t11/t67, kt-gpu t181/t182/t184, kt-otio t144–t148; plus a new class "graph traversal order is the same on CPU and GPU", which does not exist now |
| Depends on | — (F-MPH-005 and F-MPH-006 are a neighbouring, cheaper fork: levels and strength instead of wires) |
| Breaks compatibility | yes — another tool stops reading our effects from `.otio` |

## Verdict

**Rejected.** The reason is not "expensive" but this one: our "clip chain is an
ordered list" simultaneously works as the post-FX cache key (`fx_chain_hash`), as the shape of
both byte-matching executors (invariants 7 and 11), as the `Item.effects` list in the project format, and as the only
`chain_strip` widget. A graph cancels this property in all four places at once, and the
price for it will not be "a new panel" but a new class of CPU↔GPU divergences, for which we have no
tests and no references. Meanwhile three of the five editors at the core live without a graph, and the
flagship of nodes itself put a linear stack of levels on top of the graph — so the graph is not a condition of
professional grading, it is a way of organizing, and this way reproduces our own
levels (F-MPH-005) by a more expensive mechanism.

**What we take from the graph — and take WITHOUT the graph.** The value that a chain really loses is
exactly one: the matte as a separate signal. It is obtained
not by wires but by a field: an effect gets a KEY SOURCE (a reference to another slot of the chain, to a named area or to the alpha of a neighbouring layer) plus an
inversion checkbox — and we get both a shared mask for two effects and Outside behaviour,
without introducing a DAG: the picture stays a list, and ONLY the key branches, and the key, in the
manual's own words of Resolve, "is just a grayscale image". The strength
of a separate branch and a shared instance are covered by their own notes
(F-MPH-006, F-MPH-005), not by a canvas.

**When the note can be reopened.** Only if we get a separate compositing page (an analogue of Fusion) — there a graph is appropriate and does not have to promise anything about grading on the timeline. For the grading page the question is closed: "nodeness" by itself gives not a single capability that cannot be expressed as a field
on an effect, and it costs the whole of our determinism system.

## Sources

- DaVinci Resolve 20 Reference Manual (PDF, read in full locally):
  ch. 142 "Grade Management", sections Using Shared Nodes / What Are Shared
  Nodes Good For?; ch. 143 "Node Editing Basics", section Using Node Stack
  Layers; ch. 145 "Serial, Parallel, and Layer Nodes", sections Parallel Node
  Structures, Layer Mixer Prioritization, Using Composite Modes With the Layer
  Mixer, Adjusting Layer Node Strength Using Key Output Gain; ch. 146
  "Combining Keys and Using Mattes", sections Introduction to Manipulating and
  Combining Keys, Outside Nodes, Feeding Keys From One Node to Another,
  Connecting Key Outputs to RGB Inputs and Vice Versa (approx. pp. 3297–3345). https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf (accessed 2026-09-07)
- https://www.reddit.com/r/colorists/comments/1ubvd22/secondaries_before_primaries/osznna1/ — a key-generator node with no picture output (2026-09-07)
- https://www.reddit.com/r/colorists/comments/1tafmf9/look_dev_questions/olbnuwf/ — a Shared node + an empty layer node as a strength slider (2026-09-07)
- https://www.reddit.com/r/colorists/comments/1u9w0j4/dayfornight_grade_practice_looking_for_feedback/ot36nyq/ — "22 nodes" (2026-09-07)
- https://www.reddit.com/r/davinciresolve/comments/1w5o1rj/can_anyone_explain_what_im_looking_at/p7hy62d/ — "28 nodes is disgusting" (2026-09-07)
- https://www.reddit.com/r/davinciresolve/comments/1vyixyx/struggling_with_dlog_m_color_gradin_is_there_an/p5xrhrh/ — a newcomer's entry threshold (2026-09-07)
- https://www.reddit.com/r/colorists/comments/1utrv88/need_help_with_colour_matching/owzr44i/ and …/owzpe9m/ — "nodes as layers" from someone who came from Photoshop (2026-09-07)
- https://www.reddit.com/r/davinciresolve/comments/1w8lcrb/affect_colour_adjustment_to_previous_node_only/p83zosx/ — what an Outside node does, in a user's words (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/add-color-correction-effects-ver9f5631358/mac — FCP 12.3: several corrections per clip, order changed by dragging in the Video inspector, Balance/Match pinned to the top, and separately: "Applying multiple color corrections to a clip has no effect on the color masks in the additional color corrections" — the mask of the second correction is built on the SOURCE colour, that is, the same "key from the start of the tree", but hard-wired (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/change-clip-effect-order-ver761c7810/mac — FCP: effect order changed by dragging, "You can't change the order of the built-in effects" (2026-09-07)
- https://larryjordan.com/articles/adobe-updates-and-modifies-masking-in-premiere-26/ — Premiere 26: "You can only apply one mask to one effect", a mask is created Unassigned and dragged onto an effect (2026-09-07)
- https://www.provideocoalition.com/tool-tip-tuesday-for-adobe-premiere-pro-2026-mask-tool-changes-to-know/ — the same about the Unassigned mask and its assignment to Opacity or to an effect (2026-09-07)
- https://creativecow.net/forums/thread/any-way-to-apply-event-fx-after-track-fx/ — VEGAS: order of levels instead of branching (2026-09-07)

Notes on trust. The After Effects row is taken from the neighbouring note
F-MPH-002 (Mask Reference / Compositing Options), not from the primary source: `helpx.adobe.com`
does not open at all from this environment (connection drop on any URL), and the search
of Adobe Community returns 403 from CloudFront. Reddit is also unavailable by direct links —
comments were read through the Arctic Shift archive, the quotes are given verbatim from
the archived records, and the links point to the original permalinks. Nothing was checked live
in any editor: Resolve — by the official 20th-version manual, FCP — by the official 12.3 help,
Premiere — by two reviews of the 26 update.
