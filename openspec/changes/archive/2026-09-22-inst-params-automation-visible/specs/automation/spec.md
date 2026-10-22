# Spec Delta

## ADDED Requirements

### Requirement: One widget shows the curves of any carrier, the target passport is read by address
Every carrier of curves (an effect slot, a generator on an event, a NESTING with overrides, a
transition, a camera) SHALL draw its own curves with one automation widget (`automation::draw`):
graph ↔ table, the active curve, a key at the cursor, one axis lock for the whole program. A window
that allows editing a value over time without this widget MUST NOT exist. The passport of a property
by address (`Project::addr_spec`, a mirror of `addr_type`) SHALL be the only source of limits, step and
default for publication and for the consumer window; the `match` over the owner is exhaustive — a new
owner will not compile without a branch.
Why: "one animation mechanism for everything" (the decision of R.34) — the instance window bypassed it and
got hidden recording of keys without a graph (R.60); a second source of limits (zero instead of `None`)
diverged from the passport of the target on the very first day.

#### Scenario: Carriers draw curves with one code
- **WHEN** the windows of kt-app where a value can be edited over time are enumerated
- **THEN** each goes through `automation::draw`; the probe of the widget is found in the window of the slot,
  the generator and the reel instance; there are no direct calls of `bound_apply` bypassing `CurveBind`
  outside the widget
- **TEST** t691 (`src/apps/kt-app/src/layout_tests.rs`), t693 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The passport by address for each owner
- **WHEN** the passport is requested by the address of a slot, a generator, a clip attribute, a transition,
  a camera and a nested publication
- **THEN** a `ParamSpec` is returned with the limits and default of the target (for a nested publication —
  the narrowing of the outer handle on top of the passport of its target); for a vanished target — `None`;
  the type matches `addr_type`
- **TEST** t694 (`src/libs/kt-model/tests/pubparam.rs`)
