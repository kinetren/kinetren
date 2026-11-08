## ADDED Requirements

### Requirement: Linked curves resolve leaders first, each leader found once
`resolve_derived` SHALL resolve a reel's derived curves in an order with every link's leader
before its follower, find each leader by an index of the reel's events, once per curve and not per
sample, and read a link to the event's own other curve from the event as it stood before its step.
The resolved keys are the former ones: a leader's keys do not depend on the pass that resolved
them.
Why: each pass resolved one more link of a chain and copied the whole reel, and each sample of each
linked curve scanned the reel for its leader — reel events × frames of the curve.

#### Scenario: A chain of links
- **WHEN** 12, 48 and 192 events each link their gain to the previous event's
- **THEN** the resolution inside the builds grows at most 8× and 32×
- **TEST** t2067 (`src/libs/kt-engine/tests/build_cost.rs`)
