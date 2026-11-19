## ADDED Requirements

### Requirement: A passport names at most three main rows per lane
A plugin's UI declaration SHALL be able to name its main rows: at most three, each an existing
numeric or choice row of its passport; a device's sound half names its own in its own passport. A
simulated device (a kind with a pair) SHALL name them, or the registry SHALL refuse it. A plugin that
names none SHALL get its first three numeric smooth rows. The node editor SHALL show exactly these
rows in a node.
Why: kkt-26.11.18b U8 — "2-3 ползунка, но и нужно учесть как уместить огромные эффекты типо isp
чтобы не нагромождали": the node shows what matters and never grows with an 80-row passport.

#### Scenario: Every built-in plugin has a valid main set
- **WHEN** every built-in plugin of the registry is asked its main rows for each lane
- **THEN** each set has at most three rows, each row exists in its passport and is numeric or a choice; every paired kind declares its set explicitly; a test plugin pair without one is refused at registration
- **TEST** t2998 (`src/libs/kt-plugins-builtin/tests/main_rows.rs`)
