## ADDED Requirements

### Requirement: One memory ledger over every cache that holds pictures, mixes or device buffers
Every cache that holds decoded frames, kept composites, stream history, a kept final mix or device
buffers (the burst cache, the present memo, the stream memo, the kept mix, the upload, readback and
transient pools, the compute pool) SHALL report the bytes it holds to one ledger of the process, under
its tenant kind. Each tenant SHALL keep its base — the working set it was proven with — whatever the
ledger says. What a tenant wants above its base SHALL be granted from ONE growth budget: half of what
the process could still take beyond its bases, measured at run time when a want changes (the memory
available now, a control group's limit included, plus the growth the tenants already hold). Grants
SHALL go in a fixed order of value — kept composites, then stream history, then decoded frames — each
up to its want; taking back SHALL go the reverse way, and a tenant whose grant shrinks SHALL drop what
it no longer may hold at once. With the memory unknown nothing above the bases SHALL be granted. A hold
of an offline job SHALL stay stronger than any cap. No grant, cap or eviction SHALL change a byte of a
frame or a mix (invariant 11), and the ledger SHALL do nothing at rest: it acts only when a want, a
claim or a tenant's size changes.
Why: found-defects 310, M9 of `docs/engine-audit.md` — ten caches each picked their own constant, and
nobody saw the sum or took back a byte where it was worth least; the owner (kkt-26.11.08 H13): no
limit tuned for one machine.

#### Scenario: Grants follow the order of value within the budget
- **WHEN** the kept composites, the stream history and the decoded frames each want more than their bases, with plenty of memory, with memory for part of it, and with the memory unknown
- **THEN** with plenty each gets its want; with part, the kept composites are granted first, the stream history gets what is left and the decoded frames nothing; unknown — every cap is its base; the bases are never cut, and the sum of grants never passes half of the memory available plus the growth held
- **TEST** t2290 (`src/libs/kt-time/src/mem.rs`)

#### Scenario: A higher want takes back the lowest grant first, and only a change acts
- **WHEN** the stream history holds its grant, then the kept composites' want rises past what is left, then the same wants are asked again
- **THEN** the stream history's cap drops and its owner is told at once, the kept composites' does not move down; asking the same wants again changes no cap, tells no owner and writes no record
- **TEST** t2291 (`src/libs/kt-time/src/mem.rs`)
