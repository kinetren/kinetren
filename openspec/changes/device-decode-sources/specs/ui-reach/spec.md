## ADDED Requirements

### Requirement: Decoding on the card is on by default, with a checkbox in Preferences
Preferences, in its FFmpeg section, SHALL offer "Decode video on the card", on by default, saved in the user
store with the other start choices. The environment's `KT_DECODE_DEVICE=off|on` SHALL win over it. Turning
it off or on SHALL reopen the player's sources, so the next decoded picture follows it; an export SHALL read
it at its start. Diagnostics SHALL show one line: how many files decode on the card and how many on the
processor, with the reason for each processor file, or "off in Settings".
Why: kkt-26.11.20 A12 ("Вкл. + галочка"): on by default, and a switch for a driver that decodes correctly
but slowly; the choice cannot change a frame (device-decode-sources D1, D11).

#### Scenario: The checkbox turns card decoding off and on
- **PATH** Edit → Preferences → FFmpeg: "Decode video on the card" is ticked → untick it: Diagnostics reads "Decoding on the card: off in Settings" → tick it again: Diagnostics counts the files on the card
- **WHEN** a project over a fake-driver file plays with the box on, then off, then on, and the store is reloaded
- **THEN** the trace's decode records name the device, then the processor, then the device; the store keeps the last choice; the box's setter is in t1294's list of the store's writers
- **TEST** t4100 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

#### Scenario: The environment wins over the box
- **WHEN** `KT_DECODE_DEVICE=off` is set and the box is on, then `KT_DECODE_DEVICE=on` with the box off
- **THEN** the first decodes on the processor and Diagnostics says the environment turned it off; the second decodes on the device
- **TEST** t4101 (`src/libs/kt-media/tests/segment.rs`)
