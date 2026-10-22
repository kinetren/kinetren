---
id: F-HLP-028
title: Tours, coach marks and interactive lessons
status: rejected
priority: C
cost: L
category: help/onboarding
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136, jetbrains-2026.2]
we_have: no
crates: [kt-app]
invariants: [12]
ui_rules: [9, 10, 13, 14, 20]
round: null
point: null
tests: []
depends_on: [F-HLP-016, F-HLP-020, F-HLP-024, F-HLP-033, F-UI-002]
checked: 2026-09-07
---

# F-HLP-028 — Tours, coach marks and interactive lessons

## Essence

The program takes the floor by itself on first launch and leads the person through itself: either with a modal tour ("step 1 of 7" with a "Next" button), or with callouts over the interface that point at a specific button, or with a lesson that forces the person to DO the action and checks that it was done. What all three have in common is not the text but two things that exist nowhere else: a set ORDER of steps and MEMORY of which of them have already been passed.

## Who has it and how

The question for the row is one: is there, IN THE PROGRAM ITSELF, a sequence of training steps that leads through the interface and remembers where you stopped.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 b66 | Welcome screen (the "Getting Started" tab); earlier — Interactive Tutorials | A welcome screen with the tabs "New Project / Open Project / Getting Started"; the last one is a set of BUTTONS on the outside: "Use the buttons across the bottom to access tutorials, online help, various VEGAS communities". It is turned off by the checkbox "Show Welcome Screen on Startup" and returned in `Options → Preferences → General` ("Always show Welcome Screen when program starts") — rule 19 is satisfied by the vendor. The real tour ("Interactive Tutorials" in the Help menu and by a button on the main panel, "start an interactive guide that will show you each part of the VEGAS Pro interface and teach you how to create projects") is documented in the Vegas Pro 13 and 17 manuals; it is not in the 2026 what's-new document, its presence in 2026 is **[NOT VERIFIED]**. The official training of 2026 is a video series of 20+ lessons with free project files, "so you can follow each lesson step-by-step" |
| Adobe Premiere | 26 | Learn panel (historical); today — `adobe.com/learn/premiere-pro` | The public learning point is a web catalog "Tutorial article · Beginner · 3 min" with a separate block "Try these tutorials with Premiere", that is, the article opens in the browser, and the program is next to it. The state of the built-in Learn panel in 26 could not be verified: `helpx.adobe.com` does not open from this environment, neither directly nor through a text proxy **[NOT VERIFIED]** |
| DaVinci Resolve | 21 | none | Onboarding is moved out of the program into the INSTALLER: "the most important of these settings is selected via the installer's onboarding questions". Further, the manual advises the newcomer simply to check the settings at first launch; there is no tour, no callouts, and no checklist in the interface |
| Final Cut Pro | 12.3 | no tour; demo project | The "Get started" section of the guide consists of CHAPTERS ("What is Final Cut Pro?", "Intro to the Magnetic Timeline", "Final Cut Pro workflow", "Final Cut Pro interface"), and the only interactive move next to them is "Download a demo project" (via Apple Creator Studio). They teach on a real project, not on top of the interface |
| After Effects | 26 | **[NOT VERIFIED]** | The historical Learn panel with step-by-step lessons existed; the composition in 26 could not be verified for the same reason as Premiere's. The row is left as "no data" deliberately: a guess here would cost more than the gap |

**Outside the core.** Five NLEs show only the outcome of the dispute, but not the mechanism: a checklist that is checked OFF BY ITSELF on the fact of the performed action is built into VS Code, and the only coach-mark implementation whose source can be read and assessed by cost is JetBrains'. Without these two rows the conversation about a tour remains a dispute about tastes.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Visual Studio Code | 1.136 | Walkthroughs (the "Get Started" page) | The contribution point `contributes.walkthroughs` in the extension manifest: `id`, `title`, `description`, `when` and an array `steps`; a step has `title`, a markdown description, `media` (an SVG or markdown file) and `completionEvents`. The check mark is set by an EVENT: "Check off step when a command has been run" (`onCommand:`), `onSettingChanged:`, `onContext:`, `onView:`, `extensionInstalled:`, `onLink:`. It opens by itself when the extension is installed; the progress is reset by the command `workbench.action.resetGettingStartedProgress` |
| IntelliJ IDEA | 2026.2.2 | Learn tool window (IDE Features Trainer) + a tour of the new UI | The lessons run in a REAL editor: "Run IntelliJ IDEA with this plugin, and then click the *Learn* tool window button". In the same plugin `intellij-community` three mechanisms lie side by side — `src/` (lessons), `onboarding/` (the tour), `tips/` (tip of the day). A tour step (`LearnToolWindowStep.kt`) looks for the button in the live component tree by the localized text of the action, highlights it with a frame (the options have `usePulsation`), waits for the window to appear (`yield()`, the comment "wait for the tool window to appear") and places a balloon at a manually computed point; if it does not find the widget, it silently draws the step without highlighting. The onboarding of new users itself is closed by an experiment flag (`NewUsersOnboardingExperimentAccessorImpl`) |

## The trick behind it

**The payload of a tour is not text, but ORDER and MEMORY OF WHAT WAS DONE.** A single status bar line holds neither of these by construction: it does not know what came before it, and does not remember what you can already do. Neither a context menu, nor a cheat sheet, nor an effect description carries this payload either — they answer the question "what can be done here", not "where to start and what comes next". For us the order goes into the chapters of the guide and into the demo project, while the memory of what was done goes nowhere: we do not carry it at all, and deliberately (see "Verdict").

Three details that are absent from any list of features.

**1. In the reference implementation "interactivity" is off by default.**
The VS Code documentation: "By default, if no `completionEvents` events are provided,
the step will be checked off when any of it's buttons are clicked, or if the
step has no buttons, when it is opened". That is, READ = DONE, until the lesson's author lists by hand the specific commands and settings. The difference between a "lesson" and a "slideshow with check marks" is one optional field of the manifest, and by default one gets the second. Any conversation "an interactive lesson is better than a tour" must begin with the question of whether this field is enabled.

**2. A walkthrough aims at COMMANDS, not at pixels — and therefore survives rearrangement.** The step does not know where the button is: it knows `onCommand:commandId`, `onSettingChanged:settingId`, `onContext:` and `onView:viewId`. The lesson remains correct after any rearrangement of panels, because it is described in the language of actions, not of locations. The reverse side is exactly the same: without a command registry as objects, such a lesson cannot be expressed in principle — there is no one to say "the step is closed, the command has been executed". This makes the VS-Code form dependent not on drawing but on F-UI-002.

**3. The second half of the price is visible in the JetBrains source, and it concerns layout.** One step of the tour is a search for a widget in the live component tree by the LOCALIZED text of the action, the highlighting subsystem (`LearningUiHighlightingManager`) with a separate option `usePulsation`, waiting for the window to appear through a coroutine, a balloon at a computed point (`component.width + 3`, `min(150, height/2)`) and the registration of cleanup in `Disposer`. If it does not find the widget, the step does not fail and does not report — it quietly moves on without highlighting. This is the price of ONE step at a vendor that keeps the IDE on four platforms, and the same explains why their onboarding stands behind an experiment flag rather than simply being "on".

## Pros

- The only channel capable of saying "first X, then Y": menus, hints, cheat sheets and references have no order — they are lists, not a route.
- A checklist by events (VS Code) knows whether you DID it, not only whether you read it. This is exactly what distinguishes a lesson from a tour, and the distinction is mechanical rather than rhetorical: `onCommand` either fired or did not.
- A push channel: it reaches the one who did NOT search. Our hint line is pure pull, and you cannot hover the mouse over a feature whose existence you do not know (the thesis of R.40).
- The first launch is the only moment when the person has no project of his own, and occupying the screen is not a pity. This is the same basis on which the teaching empty states stand (F-HLP-020), only for the whole window at once.
- Binding to commands and settings, not to coordinates, makes a lesson cheap to maintain — but only for the one who has commands existing as objects.

## Cons

- **It shows once and at the beginning, while the questions come in the middle of work.** This is the main minus of the form, and for us it is confirmed by our own history (see "How it fits", the table of the seven claims of R.38).
- **A coach mark by definition covers what it points to**: the callout stands over the interface and covers the neighbors of the target. Overlap is a settled question (item 14 of round 8), and the form runs into it not at the edge but at its essence.
- **Human memory is shorter than a tour.** NN/g: "our short-term memory cannot retain very much information, and that information fades in about 20 seconds" — seven steps in a row do not survive the road from the tour to the first real action.
- **Auto-opening does not scale**: as soon as there is more than one source of steps, you get a "storm of walkthroughs" (quote below).
- **One who skipped it must be able to return.** VS Code keeps a separate command for this, `workbench.action.resetGettingStartedProgress`; VEGAS — a checkbox in `Preferences → General`. That is, the mechanism drags along its own line "where to return the decision" (rule 19, F-HLP-009), otherwise it is one-off.
- **The aim breaks silently.** In a non-contract implementation (JetBrains) a widget not found is not an error but a skipped highlight: the lesson keeps saying "click here", without showing where.
- **The translation of a step is a paragraph, not a line.** Our limit of 160 characters per catalog value and the whole chain t100/t125/t128 are counted in F-HLP-033; the steps of a tour are the most expensive kind of text of all those the wave considered.
- **The industry is going the other way.** The vendor that had a tour (VEGAS 13/17) today teaches with a video series with project files; Apple — with a downloadable demo project; Blackmagic moved onboarding into installer questions. None of the five NLEs of the core is today confirmed as a carrier of a live tour of the interface.

## What users say

> Oh, I never use the Welcome screen.
> — arpierre, quoted in the thread "Video Scopes Settings?", forum.borisfx.com,
> 13.08.2026, https://forum.borisfx.com/t/video-scopes-settings/25018/29

> Honestly, a beginner without any learning simply starts VEGAS is
> catastrophically. And the welcome guidance won’t help much.
> — YelandKeil, ibid., 13.08.2026, https://forum.borisfx.com/t/video-scopes-settings/25018/29

> Top extension packages right now trigger a storm of walkthroughs opening and
> fighting for attention, which doesn't help do user onboard but causes confusion.
> — digitarald, microsoft/vscode#174304, 14.02.2023,
> https://github.com/microsoft/vscode/issues/174304

The reverse side — a living quote of enthusiasm could not be found, and this must be said directly: I searched the VS Code tracker (the queries `walkthrough in:title`, `walkthrough
disable OR annoying OR reopen`, sorted by reactions — the top of the results is occupied by bugs and iteration plans, not reviews) and the Boris FX forum (`interactive
tutorials`, `welcome screen startup`, `how to learn vegas beginner`). The most popular live request about a walkthrough is "Provide a command to reset the
completed steps of a walkthrough" (#212819, 25 reactions, open), that is, people ask not to show the tour, but to RESET it.

Separately, not a review but a recommendation of the vendor of one of the five core editors — Apple HIG, the section "Onboarding": "People tend to grasp and retain information
better when they can actually perform the task they’re learning about instead of
just viewing instructional material", and next to it — "Consider providing a collection
of context-specific tips instead of a single onboarding flow". The first sentence is the only argument FOR in this note; the second is exactly what we chose instead.

## How it fits Kinotrain
CAPABILITY: openspec/specs/ui-reach/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
Rejected: there is no teaching layer over the UI — the t83 rule (no pop-up hints) and "right-click is the help itself"; coach marks contradict both.

## Estimate

The price is given for a hypothetical implementation: so that the rejection is comparable with
the alternatives, and not unfounded.

| What | Answer |
|---|---|
| Size | L — a whole round: the layer, the aim, the progress storage, a menu item, five catalogs |
| Where the main work is | kt-app (the layer on top, the search for targets, the storage) + `apps/kt-app/i18n/*.toml` |
| Project format | does not touch it |
| Tests that will have to be written | on rejection — one guard `reach_tests::no_teaching_layer_over_the_ui` (a closed list of layers above `Order::Middle`); on implementation — plus a test for the survival of the aim on rearrangement and a t128-like budget for the paragraphs of steps |
| Depends on | F-UI-002 (the command registry — without it the lesson cannot be expressed), F-HLP-033 (the cost of text), F-HLP-009 (the line "where to return") |
| Breaks compatibility | no |

## Verdict

We do not take it. The reconnaissance found not a single confirmed live tour of the interface
among the five core editors: Resolve moved onboarding into the installer, Apple — into a
downloadable demo project, VEGAS has its tour documented in the 13 and 17 manuals, and the 2026 training is sold as a video series with project files. We are refusing
not the industry standard, but the form from which the industry is leaving.

The main point, however, is the addressee. The seven claims "I don't understand how" from R.38 are not removed by a tour in any certain way: they were written by the program's owner in the middle of work, while a tour speaks to a newcomer at first launch. The same task is covered more cheaply and without overlap by four surfaces already described — F-HLP-016, F-HLP-020, F-HLP-024 and the demo project (`docs/features/help/onboarding/sample-project-and-media.md`) — each of which answers at the moment of the question, not months before it.

REJECTED: invariant 12 and rule 10 — we will not have a teaching layer over the interface, and the prohibition must be not weakened but extended, therefore the rejection is addressed to the test `reach_tests::no_teaching_layer_over_the_ui`: today above `egui::Order::Middle` exactly eight named layers live (`status-hint`, `project-title`, `ambient-overlay`, `speed-drag-overlay`, `bin-group-name`, `marker-name-edit`, `lane-name-edit`, `area-name-edit`), of them four are in-place input and four are state display, and not one explains the interface; t83 catches only `egui::Order::Tooltip`, that is, a callout on `Order::Foreground` would bring back the overlap WITH A GREEN GUARD, and the pulsation would moreover become a sixth member of the closed list `animating` in `src/apps/kt-app/src/app.rs`, where at rest `repaint_delay(false)` ≥ 100 ms stands (t20).

DEFERRED: until F-UI-002 — the only form that the reconnaissance did not reject (a lesson that CHECKS the performed action, rather than showing a picture) is expressible only through the command registry: there is no one to say "the step is closed because the command has been executed" while we have no "command" object; it is meaningful to return to this conversation after F-UI-002 and only in a form that aims at commands, not at coordinates.

**Addendum from the reconnaissance (a later return of the agent).** The deferred form is not a hypothesis: it has shipped. **Ableton Live 12.4.5** has a **Learn View**, verbatim from the manual — "structured learning modules that combine video content with written
explanations and guide you through Live's interface and key features step by
step"; the lessons are filtered by topics and **are marked as completed** with progress tracking, the view is open by default and is closed through the Help menu. What matters is WHO did this: Ableton is the only one in the sample that chose the same hint architecture
as we did (a permanent place instead of a pop-up, F-HLP-002). Facing the same limit, it set up not callouts over the interface but a SEPARATE VIEW — that is, exactly
the second surface that the whole wave is looking for: it does not overlap, does not delay, opens deliberately. This strengthens both lines of the verdict at once: a layer over the interface was rejected correctly, and the deferred form has a live industrial model. The postponement nevertheless remains: "the step is closed because the command has been executed" still has no one to say it, until there is a "command" object (F-UI-002).

ONE LINE: overlap — does not touch (and proposes to strengthen the prohibition with a test, because t83 catches only `Order::Tooltip`) | delay — does not touch | capacity — in the status bar line it changes not by a single character, and this note sets up NO new surface: the whole payload is distributed among the already accepted ones — the context menu (F-HLP-016), the bodies of empty containers (F-HLP-020), the demo project, the cheat sheet (F-HLP-024), the chapters of the guide (F-HLP-023).

## Sources

- VS Code, Contribution Points → `contributes.walkthroughs` (step fields,
  `completionEvents`, default behavior, auto-opening on installation) —
  https://code.visualstudio.com/api/references/contribution-points
  (accessed 07.09.2026)
- microsoft/vscode#174304 "Only auto-open walkthroughs for extensions a user
  choose to install" — https://github.com/microsoft/vscode/issues/174304
  (07.09.2026)
- microsoft/vscode#212819 "Provide a command to reset the completed steps of a
  walkthrough" — https://github.com/microsoft/vscode/issues/212819 (07.09.2026)
- JetBrains, `plugins/ide-features-trainer/README.md` and
  `onboarding/src/.../LearnToolWindowStep.kt`, `NewUsersOnboardingExperimentAccessorImpl.kt` —
  https://github.com/JetBrains/intellij-community/tree/master/plugins/ide-features-trainer
  (07.09.2026)
- Apple, Human Interface Guidelines → Onboarding —
  https://developer.apple.com/design/human-interface-guidelines/onboarding
  (07.09.2026)
- Apple, Final Cut Pro User Guide for Mac, the sections "Get started" and "Download a
  demo project" — https://support.apple.com/guide/final-cut-pro/welcome/mac
  (07.09.2026)
- Nielsen Norman Group, Aurora Harley, "Instructional Overlays and Coach Marks for
  Mobile Apps", 16.02.2014 (summary of the article: "users tend to dismiss them quickly and
  do not read thoroughly") —
  https://www.nngroup.com/articles/mobile-instructional-overlay/ (07.09.2026)
- VEGAS Pro 22 "Introduction to the new features" (the welcome screen, the "Getting Started" tab,
  the return via `Options → Preferences → General`) and the manuals of Vegas Pro 13 / VEGAS Pro 17 (the section "Interactive Tutorials",
  the Help menu and the main panel button) — the vendor's PDFs, parsed with `pdftotext -layout`
  (07.09.2026)
- The VEGAS Pro 2026 training page (a video series of 20+ lessons, "Download Projects")
  — https://www.vegascreativesoftware.com/vegas-pro/learn/ (07.09.2026)
- forum.borisfx.com, thread "Video Scopes Settings?", post 29 —
  https://forum.borisfx.com/t/video-scopes-settings/25018/29 (07.09.2026)
- DaVinci Resolve 21 Reference Manual, chapter 1 "Introduction to DaVinci Resolve"
  ("installer's onboarding questions") — the Blackmagic Design PDF (07.09.2026)
- Adobe Learn → Premiere (the catalog of web lessons, the block "Try these tutorials with
  Premiere") — https://www.adobe.com/learn/premiere-pro (07.09.2026)
- **There was no live check for any program**: the Premiere and After
  Effects lines rest on Adobe's public catalog, because `helpx.adobe.com` does not open from
  this environment, neither directly nor through a text proxy; the VS Code
  and JetBrains lines — on the documentation and the open source; for software outside the core a
  live check is not assumed (`docs/features/_editors.md`).
