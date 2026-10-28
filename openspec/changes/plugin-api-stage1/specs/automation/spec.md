# Spec Delta

## MODIFIED Requirements

### Requirement: Sources of a curve value — keys, link, formula; a plugin remains an interface
A curve SHALL have a value-source field: `Keys` (default, not serialized — the bytes of files with keys do not move), `Link` and `Formula`. All three SHALL be WORKING
sources; the behaviour of the link and the formula is described by separate requirements. A source whose
address does not resolve, or whose text does not parse, SHALL be rejected LOUDLY on load
and on command — a silent render "by keys" would show something other than what is written in the file.
A plugin SHALL NOT be a fourth source of a curve value: it is an OWNER of a passport, exactly like a
built-in kind, and its curves are resolved by the same compiled automation (capability `plugins`).
Its kernels SHALL be pure functions without a clock or I/O. A VIDEO plugin that declares state between
frames, a tail or a look-ahead SHALL be refused at registration; an AUDIO plugin MAY declare state, a tail
and a look-ahead, and its state object SHALL be owned by the host on the audio render thread, created and
reset by the host and kept across model edits by clip id. No plugin outside the closed built-in set SHALL
enter the golden-cycle registry.
Why: the place for sources was prepared in R.59 in advance precisely so that the evaluator would arrive in the next round without breaking the format. Revision (plugin-api-stage1, 2026-10-27):
"refused on load for a declared state or tail" was written for frames — post-FX memoization is built on
frames without history; audio has no post-FX cache (invariant 2) and every built-in reverb, delay and
limiter already has exactly this host-owned state (invariant 3), so the blanket rule would have refused
the very effects that the first stage moves into plugins.

#### Scenario: An unresolvable source is rejected
- **WHEN** a curve's source has an address pointing to a vanished property, or has text that cannot be parsed
- **THEN** a project with ordinary curves is valid; with an unresolvable source — an explicit error with the address and the reason
- **TEST** t649 (`src/libs/kt-model/tests/props.rs`)

#### Scenario: The plugin remains an interface
- **WHEN** the plugin contract is checked
- **THEN** a declared state or tail is refused on load for a video plugin and admitted for an audio
  plugin; the plugin does not enter the golden registry
- **TEST** t649 (`src/libs/kt-model/tests/props.rs`), t972 (`src/libs/kt-plugin-api/tests/registry.rs`)

#### Scenario: A plugin's curve is compiled like a built-in one
- **WHEN** the pilot plugin (Posterize) carries a curve on `levels` and a clip without curves carries it
  too; both are rendered, the curve's key is edited, and the chain is rendered again
- **THEN** the curve is resolved by the compiled plan at the frame-aligned material tick, the clip without
  curves borrows its chain (no clone per frame), the post-FX key changes after the edit and the old frame
  is not shown, and the values are clamped by the plugin's passport
- **TEST** t974 (`src/libs/kt-gpu/tests/parity.rs`), t643 (`src/libs/kt-model/tests/props.rs`)
