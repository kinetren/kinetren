## ADDED Requirements

### Requirement: The trace names the memory ledger's budget and every grant
The structured trace SHALL record each sizing of the memory ledger (`mem_budget`: the growth budget,
what is granted, the memory available then, whether an export claims it) and each grant that changed
(`mem_grant`: the tenant, its want, its new cap, what it holds). An unchanged sizing SHALL write
nothing. `kt-cli trace memory` SHALL print them in order, and each tenant's last cap and bytes.
Why: the owner's run must say why a loop was or was not kept whole — which cache held what and how much
the machine allowed (memory-ledger).

#### Scenario: A sizing is a record and the query reads it
- **WHEN** a trace holds two ledger sizings, the second taking back a stream grant, and a sizing that changed nothing
- **THEN** it holds two `mem_budget` records and the changed `mem_grant` records only; `kt-cli trace memory` prints both sizings and the stream tenant's last cap
- **TEST** t2300 (`src/apps/kt-cli/src/trace_query.rs`)
