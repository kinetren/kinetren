# Proposal

## Why

The project is moving to OpenSpec, but its project decisions lie not where OpenSpec reads them.
Today one decision of a round is smeared across five chronicles: `docs/features/*`
(63,500 lines, of which ~36 % is "How it fits Kinotrain", that is, our decisions, not the survey of
other editors), the porting log (7,191 lines; from §440 on it is a log of rounds in the
genre "what was / cause / rule", not a catalogue of defects), `docs/user-feedback.md` (2,637 lines of
the owner's decisions), `docs/architecture.md` (150 lines, empty) and the test table in the root
`CLAUDE.md` (lines of 5 KB each). A new change in OpenSpec will not see any of this automatically:
in the long term OpenSpec reads only `openspec/specs/<capability>/spec.md` and `openspec/config.yaml`.

## What Changes

- Create `openspec/specs/<capability>/` for 17 capabilities **by behaviour** (the list is in
  design.md). In each there are two files: `spec.md` — the contract (requirements + scenarios, each
  scenario names a regression test tNNN); `design.md` — the "why" (what was, who was hurt, what was
  rejected and for what reason, which detail is non-obvious, `src/…` paths).
- Distill the content from the three sources by ONE criterion: a line stays if it **still constrains
  the code** (there is a test, an invariant or a rule that an agent would break without knowing it) —
  plus its "why". The chronology of rounds, "rewritten from C++", the comparison with v1, things
  rejected without a reason, item numbers — rubbish.
  - `docs/features/*`: the section "How it fits Kinotrain" and the "our" parts of the "Verdict" move
    into the capability's `design.md`; 3–5 lines and a reference to the capability remain in the
    note. The survey of other editors, "the trick behind it", quotes — are not touched.
  - the porting log: rules and their tests → `spec.md`; "what was / cause" → `design.md`.
    The file is **frozen at §470** as a historical document of the port; new chapters are not written
    into it.
  - `docs/user-feedback.md`: "hurt by X → demanded Y → rule Z" → `design.md` of the corresponding
    capability. The file itself is not touched — it is the primary source of the quotes.
- Fill in `openspec/config.yaml`: `context` — the time model and the 12 invariants, one line each;
  `rules.proposal` and `rules.design` — the obligation to read `design.md` of every affected capability
  and to formalize a contradiction of it as an explicit "revision" item with a reason.
- Root `CLAUDE.md`: the table "crate → mandatory tests" moves into the scenarios of the specs; the file
  keeps a pointer to `openspec/specs/`. Invariants 1–12 remain (this is the text most readable by
  agents), but without the history of items.
- `docs/features/README.md`: rule 5 ("How it fits" with code paths) is replaced by the rule "How it
  fits" = a reference to the capability + ≤ 5 lines; `--check` is updated for the new rule.
- `docs/architecture.md`: either it becomes a table of contents of `openspec/specs/*/design.md`, or it
  is deleted — decided in tasks according to what is left.

Program behaviour **does not change**. The change does not create delta specs: it creates the main
specs directly, so `.openspec.yaml` carries `skip_specs: true`.

## Capabilities

### New Capabilities

None via a delta: the main specs are created directly in `openspec/specs/` by the tasks of this change
(bootstrap). The list of capabilities and the binding of sources — in design.md.

### Modified Capabilities

None.

## Impact

- Only documentation and OpenSpec configuration: `openspec/specs/**`, `openspec/config.yaml`,
  `docs/features/**` (truncation of sections), `docs/features/README.md` and
  `src/scripts/features-index.py` (the rule "How it fits"), the porting log
  (freezing, header), the root `CLAUDE.md` (test table), `docs/architecture.md`.
- Code and tests are not touched. `cargo test` and `features-index.py --check` must remain green.
- Environment: the `openspec` CLI requires Node ≥ 20 (`import … with` in cli-spinners); the system
  Node 18 does not fit, `~/.nvm/versions/node/v24.21.0` works.
