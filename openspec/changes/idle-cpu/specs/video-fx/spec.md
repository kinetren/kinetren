## ADDED Requirements

### Requirement: The UI and everything but an export run on the card that drives the monitor
The preview and the offline engine (effect cells, reel strips, posters) SHALL ask for
`display_power_preference`: a discrete card that alone drives the monitors is asked for by
HighPerformance; anything else stays LowPower. An export stays HighPerformance. WGPU_POWER_PREF
overrides both.
Why: LowPower is there for hybrid graphics, where the discrete card copies every frame over PRIME and
the whole system stutters (item 39). On a desktop with the monitors on a discrete card it picks the
processor's integrated GPU instead: the owner's panel read "AMD Radeon Graphics (RADV
RAPHAEL_MENDOCINO)", and since the CPU composite was deleted every effect runs there.

#### Scenario: The card with the monitor gets the work
- **WHEN** the monitors hang off the discrete card, off the integrated one, off both, or nothing is known
- **THEN** only the first asks for HighPerformance
- **TEST** t1837, t1837b (`src/libs/kt-gpu/src/context.rs`)
