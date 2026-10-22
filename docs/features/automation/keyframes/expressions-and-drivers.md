---
id: F-AUT-007
title: Expressions: a property driven by a formula
status: done
priority: B
cost: XL
category: automation/keyframes
editors: [after-effects-26, premiere-26, resolve-21, fcp-12, vegas-pro-2026, nuke-17, blender-5.2]
we_have: no
crates: [kt-model, kt-engine, kt-fx, kt-app, kt-otio]
invariants: [11]
ui_rules: [20]
round: R.61
point: 462
tests: [t701, t707, t708, t709, t710, t698d]
depends_on: [F-EXT-002]
checked: 2026-09-22
---

# F-AUT-007 — Expressions: a property driven by a formula

## Essence

Instead of keys placed by hand, a parameter has a formula string, and the value is computed by it on every frame. The formula reads the time, the length of the event, the speed, another parameter of the same effect, a parameter of a neighboring effect — and outputs a number. The editor needs this not for programming, but for three things: to link two parameters that must move together ("shadow offset = minus picture offset"), to get motion that cannot be laid out with keys (jitter, oscillation, bounce), and to set a curve with NUMBERS instead of clicking it out with the mouse.

## Who has it and how

The question for each one is the same: can a PARAMETER be given a formula instead of a value, and what does that formula see?

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x | Expressions | the string lives IN THE PROPERTY ITSELF (`Alt`+click on the stopwatch) and is computed on every rendered frame. The language is JavaScript: up to 16.0 the engine is ES3 (Legacy ExtendScript), from 16.0 — a modern one, switched in the PROJECT settings. Addressing goes through the composition's object model (`thisComp.layer("Null 1").transform.position[0]`); the string is not typed by hand but built with the pick-whip — dragging the "whip" onto the needed property. From 15.0 an expression can read JSON footage (`sourceData`), from 15.1 it can connect function libraries |
| Adobe Premiere | 26.x | none | there are no expressions in any form: a parameter has a value, keys and an interpolation type, and that's all. Automation of a property is done either with keys, or by moving a piece to After Effects via Dynamic Link |
| DaVinci Resolve | 21 | none on the Edit page; Expression on node inputs on the Fusion page | on the editing page the Inspector parameter has no formula. It exists one floor down, in Fusion: right-click on a node's input → `Expression`, and in the field one writes an expression in its own Lua-like syntax with addressing via `Node.Input`. That is, the formula exists in the same application, but in another editor, with another language and another time scale |
| Final Cut Pro | 12.3 | none | the inspector parameter has no formula. The closest is a rig made in Motion (F-AUT-008): it too is "one value drives others", but it is set not by text but by state snapshots |
| VEGAS Pro | 2026.0 | none | envelopes and OFX keys have no formulas. The only way to compute a parameter with a program is a .NET script that places the keys IN ADVANCE (F-EXT-002), that is, not a live expression but a key generator |

**Outside the core.** Two programs are mandatory: both are the ORIGIN of the mechanism,
and without them only the Adobe variant is visible, and it is not the only one. Nuke — there an expression is not an "animation feature" but a property of any input field; Blender —
the only place where such a mechanism has a declared security model, and it turned out to be part of the feature itself.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Foundry Nuke | 17.0 | Expression on any knob | ANY input field has an expression mode (`=` in the field or right-click → `Edit expression`); the field is highlighted, and from then on the value is computed by the formula. Addressing goes by the node tree with relative prefixes (`parent.`, `input0.`), that is, the expression is not tied to a "layer": it references a neighboring node in the graph |
| Blender | 5.2 LTS | Drivers | a property is driven by ANOTHER property: a driver has a list of variables (each a path to someone else's property), a link type and, in `Scripted Expression` mode, a Python string. The key detail: a driver is not evaluated until `Auto Run Python Scripts` is enabled in the settings: the file opens, the console writes "skipping driver, automatic scripts are disabled", and the scene looks broken. The reason is named in the manual in plain text: Python does not restrict what a script can do |

## The trick behind it

**1. An expression is evaluated on EVERY rendered frame, and therefore the picture
becomes a function of the interpreter version and the evaluation order, not only of the
input numbers.** This is not theory: the After Effects transition to a new JS
engine in 16.0 broke projects that had rendered for years, and it broke them not in
the language but in the SEMANTICS — "the new engine evaluates whole blocks/structures, not
line by line as the old one did" (Mylenium, 16.10.2018). Adobe had to keep the engine
switch in the PROJECT settings and ship the script `Upgrade Legacy Expressions.jsx`,
that is, admit that the project file carries code that depends on the runtime. For us this is exactly the subject
that invariant 11 fights over: `GOLDEN_AUDIO`/`GOLDEN_VIDEO` are a promise
of "the same bytes", and an expression moves part of the answer outside the file, into the
build. The second half of the same price is speed: the expression is called per frame per property, and users feel it before they read the documentation ("I think it's because it re-reads the expression on every single frame" — Fred Lobo, 27.10.2008).

**2. A document carrying executable code is a document that executes code when opened, and in the world of video editing this problem has not yet arrived only because
expressions go into the project file only from Adobe.** Blender went to the end of the chain and moved the conclusion into the interface: drivers are a legitimate part of rigging, but `Auto Run Python Scripts` is OFF by default, and a stranger's file opens with non-working drivers and a line in the console. The honest price is visible at once: users complain for years that "scripts are disabled" and the scene looks broken, and this is cured either by trusting the file's source or by a global checkbox — that is, the choice is handed to a person who cannot verify someone else's code. None of the five core editors has such a switch, because none except AE has expressions in the file; for AE the question is closed by the fact that the expression JS engine is a sandbox with no file system access — an implicit, nowhere advertised, but key design decision. Anyone who introduces a formula into the project format is obliged to state out loud which of the two variants they belong to.

**3. Every editor with expressions sooner or later gets a command "convert the expression to keys", and it explains what an expression really is.** In AE this is `Animation → Keyframe Assistant → Convert Expression
to Keyframes`, and they advise it for exactly two reasons — to speed up rendering and to hand the project to a place where the expression would not survive. The conclusion is unpleasant and useful: people use a live formula while editing, and want static values on output. That is, an expression is first of all a KEY GENERATOR that was allowed to live longer than it should; it has to be "live" only where it depends on what changes by itself (event length, speed, another parameter). This separation is worth making consciously, not inheriting: a formula that merely describes the shape of a curve can be baked once at editing time and cost nothing to render.

## Pros

- A formula is the ONLY way to express the link "this parameter is a function of that one" without duplicating keys: an edit to the source automatically changes the driven one, and desynchronization is impossible by construction.
- Motion that cannot be laid out with keys (oscillation, jitter, bounce, dependence on event speed) is described by one line instead of a hundred keys — and stays editable: you edit the formula, not a hundred points.
- A curve given by numbers is portable and comparable: it can be seen in a diff of the project file, printed, sent as text, repeated verbatim on another event. A curve clicked out with the mouse can be neither.
- A formula removes the question of foreign length that trips up presets (F-AUT-004): an expression written relative to the fraction of the event fits a clip of any duration without three mutually exclusive transfer modes.
- In Nuke an expression is a property of ANY field, not a separate subsystem: this shows that the cost of the feature is determined not by the language but by whether the parameters have a single passport. Where there is one passport, the formula attaches to it in one place.

## Cons

- Debugging: the error lives in one property of one event, and shows up as "the render goes wrong". There is neither a breakpoint nor intermediate values — only the final number in the frame.
- A per-frame price. The expression is called for every property on every frame, and in AE this is a regular reason to convert expressions to keys by hand. A project without formulas must not pay at all — which means the presence of formulas must be visible BEFORE the hot path, not checked on every frame.
- A public contract. As soon as a formula can address other objects, their names and structure become part of the format: rename an effect — break others' projects. In AE this is softened by the pick-whip (the program writes the string), but not removed.
- Cycles. `A = f(B)`, `B = g(A)` is a legitimate notation, and it has to be resolved before rendering, not during it.
- A general-purpose language drags in a runtime, a version, compatibility, and the whole security question (Blender). For a program that has exactly one `libloading` for the whole tree (F-EXT-002), this is not a detail.
- A formula competes with keys for the same property, and in AE, with a live expression, the keys do not disappear but stop acting — a source of constant confusion "why am I moving the key and nothing changes".

## What users say

The most valuable here is not the raves but two negative testimonies which together describe the PRICE of the mechanism.

First: a change of expression engine changes the result of finished projects.

> In the new version CC2019, an expression error began to appear, which was
> not in the version of cc2018.
> — Stosheska, Adobe Community (After Effects), 15.10.2018,
> https://community.adobe.com/t5/after-effects/after-effect-cc2019-expression-issue/td-p/10069990

> The new engine evaluates whole blocks/ structures, not line by line like
> the old one.
> — Mylenium, same thread, 16.10.2018

Second: the per-frame price, named by a user before the documentation did.

> I figured it is because it re-reads the expression in every single frame.
> — Fred Lobo, Creative COW (After Effects Expressions), 27.10.2008,
> https://creativecow.net/forums/thread/expression-slow-render-timeae/

On security: Blender drew the conclusion ON the user's behalf and wrote it into the manual — and the reverse side is written there too, because by default drivers are not evaluated at all.

> Python does not restrict what a script can do, so you should only run
> scripts from sources you know and trust.
> — Blender 5.2 LTS Manual, section "Scripting & Security",
> https://docs.blender.org/manual/en/latest/advanced/scripting/security.html
> (accessed 2026-09-21)

## How it fits Kinotrain
CAPABILITY: openspec/specs/automation/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Our own mini-formula with a closed set of operations, with no environment; it is resolved into numbers before the audio stream, and costs nothing when not used.

## Estimate

| What | Answer |
|---|---|
| Size | XL — a round plus a version of the format envelope: a parameter's value gets a second source, and this touches the model, the file, and the cache key |
| Where the main work is | kt-model (the value source of a curve, parsing and evaluating the formula, cycle checking), kt-engine (resolution in `frame_plan`, the formula text in `fx_chain_hash`), kt-app (`automation.rs` — display of a non-editable graph, the input field), kt-otio (the envelope) |
| Project format | affected: the `metadata["kt"]` envelope grows by a version (`KT_META_VERSION`, `src/libs/kt-otio/src/meta.rs`). A file WITHOUT formulas must stay byte-identical, a file WITH a formula must be explicitly rejected by an old build, not read "almost the same" |
| Tests that will have to be written | one and the same formula gives the same bytes on two runs and against the reference; GOLDEN_AUDIO/GOLDEN_VIDEO did not budge on files without formulas; editing the formula text invalidates the post-FX frame (next to t66); a cycle is rejected by a COMMAND (next to t423); a project without formulas pays nothing (the counter of evaluator calls is zero) |
| Depends on | F-EXT-002 (there the price of the fourth model is analyzed — a script inside a parameter — and why an external interpreter is closed to us); indirectly F-TL-012 (a public parameter of a composition — the first meaningful RECIPIENT of a formula) |
| Breaks compatibility | not forward, yes backward (a file with a formula will not open in an old build) |

## Verdict

**We take it — as our own mini-formula, evaluated live on every frame, without a single external interpreter.** This is a direct decision of the owner, recorded in
`docs/user-feedback.md` (R.59): "Expressions — our own mini-formula, evaluated live on every frame; there are no external interpreters." The reconnaissance confirms this decision from three sides: the change of engine in AE changed the result of finished projects (quotes above) — so the runtime cannot be someone else's; Blender
shows what a document with executable code pays — so we do not put a general-purpose language into the project file; Nuke shows that the cost of the feature is
determined not by the language but by the presence of a single property passport — and we now have one
(`HasParams::param_specs`, R.59 item 466).

Three conditions without which the feature must not be started, and all three are consequences of invariants, not of taste. First: the formula text is part of the chain key
(`fx_chain_hash`), otherwise editing the formula would leave old frames on the screen.
Second: the formula is resolved into numbers BEFORE the audio stream, as today
`AutoProfiles::from_curves` — there are and will be no computations in the callback.
Third: a project without formulas pays nothing, and this is checked by a test, not by a promise (R.59, item 4).

**R.59 does only the slot and the interface** — the value of a parameter gets a place for a second source, the formula itself arrives in the next round. The order was chosen by the owner consciously: first the property model and the public parameters of a composition, then what they control. A formula without a recipient ("another property") would degenerate into a function of time, that is, an expensive way to draw a curve that
`src/apps/kt-app/src/automation.rs` already draws.

### Decisions of round R.61

The formula stops being only a slot. The evaluator is one per formula and one per the recalculation expression of a link; the set of operations is closed, there is no clock, files, or randomness from the environment.
The three conditions of the note are accepted verbatim and became requirements of the spec: the formula text enters the post-FX key; the formula is resolved into numbers BEFORE the audio stream; a project without formulas pays nothing, and this is checked by a test.

Added beyond the note: a parse and evaluation error (an unknown name, division by zero, an unclosed bracket, an attempt to take the machine's time) must be NAMED with a position in the line — a silent zero is forbidden.
## Sources

- https://community.adobe.com/t5/after-effects/after-effect-cc2019-expression-issue/td-p/10069990
  — breakage of expressions on the transition to the new JS engine, quotes from Stosheska and
  Mylenium (accessed 2026-09-21)
- https://helpx.adobe.com/ca/after-effects/using/legacy-and-extend-script-engine.html
  — syntax differences of Legacy ExtendScript (ES3) and the JavaScript engine
  (ES2018), the switch in project settings, `Upgrade Legacy
  Expressions.jsx` (accessed 2026-09-21)
- https://ae-expressions.docsforadobe.dev/introduction/changelog/ — the history of the expression engine: JSON in 15.0, function libraries in 15.1, the new engine in
  16.0 (community documentation, not a primary source; 2026-09-21)
- https://creativecow.net/forums/thread/expression-slow-render-timeae/ — "re-reads the expression on every frame" (2026-09-21)
- https://creativecow.net/forums/thread/convert-expressions-to-keyframes/ — `Convert Expression to Keyframes` as the standard way to remove the per-frame price
  and hand the project onward (2026-09-21)
- https://docs.blender.org/manual/en/latest/advanced/scripting/security.html
  — `Auto Run Python Scripts`, the wording about Python having no restrictions (2026-09-21)
- https://projects.blender.org/blender/blender/issues/36131 and
  https://projects.blender.org/blender/blender/issues/48111 — "skipping
  driver, automatic scripts are disabled": drivers are not evaluated while
  auto-run is off. Both pages returned 403 on direct access, the content is taken from search snippets — **[NOT VERIFIED]** verbatim (2026-09-21)
- Nuke: expressions on knobs and relative addressing (`parent.`) — per the `nuke-17` line in `docs/features/_editors.md` and per the reconnaissance of R.44
  (F-CMP-001); no separate check was made in this pass
- Resolve: the `Expression` field on node inputs exists on the Fusion page and is absent from the Inspector of the editing page — per the analysis of F-EXT-002
  (the standard `README.txt` of the Scripting API) and the Fusion help;
  **[NOT VERIFIED]** live
- Ours: `src/libs/kt-model/src/project.rs` (`HasParams`, `ParamSpec`,
  `ParamType::curvable`, `ParamCurve`, `AutoKey`, `curve_value`,
  `apply_fx_auto`), `src/libs/kt-engine/src/video.rs` (`layer_src`,
  `fx_chain_hash:184`), `src/libs/kt-fx/src/lib.rs`
  (`AutoProfiles::from_curves`), `src/apps/kt-app/src/automation.rs`,
  `src/libs/kt-model/src/warp.rs`, `docs/user-feedback.md` section "Round
  R.59", `docs/features/plugins/scripting/scripting-api.md` (F-EXT-002)

No foreign editor was run live. The AE lines rely on Adobe's help and forum threads, the Blender line on the 5.2 manual and the tracker, and the
Premiere/FCP/VEGAS lines on the absence of the mechanism in their help (checked by search across the sections on keyframes and effect parameters).
