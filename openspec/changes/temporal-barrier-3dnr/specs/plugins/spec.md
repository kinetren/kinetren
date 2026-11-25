## ADDED Requirements

### Requirement: A stream kernel may be active by its values and may cut its slot in three
A plugin MAY declare a stream kernel beside another kernel with a rule of its values that says when the slot keeps a history; when it does not, the slot
SHALL be what the other kernel makes it, byte for byte. Such a kernel MAY cut its slot into a front, run on every frame of the history with the slots
before it, and a back, run on the shown frame in the slot's place; both SHALL be internal plugins that reach no file and no picker. The host
SHALL tell a kernel whether its event continues the take of the event before it. A stream's picture MAY be handed over as RGBA kept on the device.
Why: the camera's recursion is a history between two halves of one effect; one machinery hosts every history (invariant 7).

#### Scenario: A slot becomes a stream and back
- **WHEN** a test kernel with `active` by a row and a `split` is registered; a chain holds it with the row off, on, masked, muted and in an arm; a kernel splits into a plugin that is not registered
- **THEN** off: the plan and bytes of the other kernel; on: the front runs per history frame and the back in the slot's place, with the mask and the mute as for any slot; the last registration is refused naming it
- **TEST** t3545 (`src/libs/kt-plugin-api/tests/registry.rs`)
