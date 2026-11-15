## ADDED Requirements

### Requirement: A value edit costs its own address on the calling thread
`Player::update_gen` SHALL retune the edited event's own address and hand its other addresses (an
instance with overrides, a morpher's pieces) to the builder thread; it SHALL resolve no reel and
build no renderer on the calling thread, and SHALL NOT copy the stored project when it already holds
the values.

#### Scenario: A title keystroke
- **WHEN** a generator inside a reel placed eight times takes a new value through `update_gen`, and then the same value again
- **THEN** the call counts no reel resolve and no renderer build, the stored project carries the value, and the second call keeps the stored project as it is
- **TEST** t2669 (`src/libs/kt-engine/tests/gen_event.rs`)
