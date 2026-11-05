## ADDED Requirements

### Requirement: An offline job can hold cache entries it still needs
The burst cache SHALL offer a per-key hold: taking one on a present entry or at its insertion, in one
step under the cache's lock; releasing it; and the bytes of the entries held at least once. A held
entry SHALL NOT be evicted, whatever the budget and whatever the recency of the others; the budget
may be exceeded for held entries, as for a pin. A key may be held several times and stays held until
each hold is released. Holds do not change coverage, and overwriting an entry keeps them.
Why: a job that reads ahead of its use cannot rely on LRU. A read refreshes entries that will never be
needed again, and they outlive the ones decoded and not yet read: the pipelined export lost frames
that way (capability `export`). A pin is one bit, and one frame is needed by several chunks.

#### Scenario: A held entry outlives the pressure and goes when released
- **WHEN** a key is held twice, the cache is filled over its budget while other entries are read, and
  the holds are released one at a time, and a later insert passes the budget again; and a key that is
  absent is asked for a hold
- **THEN** the key stays through the pressure and after the first release, the held bytes count it once
  and fall to zero after the second release, then the next insert evicts it; the absent key answers
  "not present" and holds nothing; an overwrite keeps the holds
- **TEST** t1600 (`src/libs/kt-cache/src/lib.rs`)
