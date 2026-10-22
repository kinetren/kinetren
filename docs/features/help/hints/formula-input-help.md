---
id: F-HLP-034
title: Help and hints in the formula field — without popups
status: done
priority: B
cost: S
category: help/hints
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2, vscode-1.136]
we_have: yes
crates: [kt-model, kt-app]
invariants: [12]
ui_rules: [9, 10, 16]
round: R.67
point: "change r67-curves-stills-import (chronicle frozen at §470)"
tests: [t789, t789b]
depends_on: [F-AUT-007, F-HLP-006, F-AUT-010]
trackers: []
services: []
checked: 2026-09-26
---

# F-HLP-034 — Help and hints in the formula field — without popups

## Essence

The editor opens the formula field and sees what can be written: next to it a collapsible "Help" lists the variables (`t`, `k`, `src`, `s`, `a`, `b`) with their purpose and all functions with the number of arguments; the status line next to the field repeats the variables; a wrong name is reported with the closest similar ones; Tab completes the name from the typed prefix — right in the field, without a list over the interface.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026.0 | — | no formulas |
| Adobe Premiere | 26 | — | no formulas |
| DaVinci Resolve | 21 | Fusion → Expression field | no hints; the list of functions — only in the help; an error — a red frame without text |
| Final Cut Pro | 12.3 | — | no formulas |
| After Effects | 26 | Expression Language Menu (▶ next to the field) | a POPUP tree menu inserts a call template into the text; an error — a yellow icon and text in the panel with the line number |

**Outside the core.** Blender and VS Code show two limits: a panel of Python hints and full autocomplete with a popup.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 | Driver editor + Python tooltips | an expression with an error is underlined, the error text is in the driver panel; hints are popups |
| Visual Studio Code | 1.136 | IntelliSense | a popup with candidates, Tab accepts; sorting by prefix |

## The trick behind it

1. **In After Effects the language reference is an INSERT MENU, not a document:** a menu item inserts `wiggle(freq, amp)` with named arguments into the text. The form of the hint "insert a sample" is worth more than "read the list" — but in AE it is a popup, while our popups are forbidden (rule 10, `ui-reach`). Our equivalent: a click on a function in the "Help" group inserts a sample call into the field at the cursor position.
2. **An error with a position is rare:** Resolve paints the frame and stays silent, AE names the line. Our evaluator already returns the symbol position (F-AUT-007); the help is to show it to the person and add "the closest names" (`floor` for `flor`), which none of the five does.
3. **Tab completion without a popup has existed in shells for thirty years** (bash: a single candidate — complete it, several — up to the common prefix, a list — on a separate line). This is exactly the form that coexists with the ban on floating layers: the list of candidates goes to the status line.

## Pros

- Not a single layer above `Order::Middle` (t83 stays green); the help is part of the block, not a window and not a popup.
- The content of the group is built from the evaluator's registry — not a second copy of the language.
- An error names the name and the closest ones — rule 16 (impossibility is an error with a cause).

## Cons

- The "Help" group takes up space in the block — collapsed by default, egui memory.
- Tab in the formula field is intercepted from field navigation; in other fields Tab stays as before.

## What users say

NO QUOTES: the reconnaissance was carried out on the help of After Effects (Expression Language Menu), Resolve Fusion (Expression), Blender (Drivers) and VS Code (IntelliSense); a search across sites was not carried out — the requirement is verbatim from the owner (`docs/user-feedback.md`, R.67: "it is completely unclear what can be specified in the curve formulas, there is no help at all, no hints at the moment of input, etc.").

## How it fits Kinotrain
CAPABILITY: openspec/specs/ui-timeline/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
The help is a collapsible group in the curve block built from the evaluator's registry, a status-line hint next to the field, an error with a position and the closest names, Tab completes in place; there are no popups (t83).

## Estimate

| What | Answer |
|---|---|
| Size | S — the function registry exported from the evaluator, a group in the block, Tab |
| Where the main work is | kt-model `formula.rs` (`Fun::ALL`, closest names), kt-app `automation.rs` |
| Project format | not touched |
| Tests to be written | the group equals the registry; Tab completes without a layer; an error names the closest; the status-line hint |
| Depends on | F-AUT-007, F-AUT-010 |
| Breaks compatibility | no |

## Verdict

**Accepted (R.67).** The AE "insert menu" form is moved into a group of the block (a click inserts a sample), the error — with a position and the closest names, completion — by the shell pattern.
TEST: t789 (`src/apps/kt-app/src/layout_tests.rs`) — the group from the registry, Tab without a popup, an error with the closest names; t789b (`src/apps/kt-app/src/reach_tests.rs`) — the help and the field are reachable from the FX window, the generator and the instance of the reel.

## Sources

- Adobe After Effects Help: «Expression language menu», «Expression errors».
- Blackmagic DaVinci Resolve 21 Reference Manual, Fusion: «Expressions».
- Blender 5.2 Manual: «Drivers → Expression».
- VS Code Docs: «IntelliSense».
- GNU Bash Reference Manual: «Readline Completion».
