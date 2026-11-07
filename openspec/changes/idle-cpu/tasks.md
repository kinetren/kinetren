# Tasks: idle-cpu

## 1. Stage 1: nothing runs at rest

- [x] 1.1 Work counters in `kt_time::work`, read by the geometry dump
- [x] 1.2 The heavy project: `kt-cli showcase --heavy`, and `kt-cli pause-scan`
- [x] 1.3 Composition previews settle (t1830), one sidecar row per release (t1831)
- [x] 1.4 An incomplete frame waits for news (t1832, t1836)
- [x] 1.5 At rest no frame is asked for: `wake`, `rest_wake` (t20, t1833)
- [x] 1.6 A hover frame does not lay out the events (t1834)
- [x] 1.7 The card that drives the monitor (t1837, t1837b)
- [x] 1.8 The load watch, the Diagnostics section (t1839)
- [x] 1.9 Live scenario `idle`, red on the old logic
- [x] 1.10 Compared with the build of 2026-11-03

## 2. Stage 2: the interface never waits for a picture

- [ ] 2.1 Find every wait of the UI thread on a composite, a device or a lock
- [ ] 2.2 The composite leaves the UI thread: one in flight, the latest wins, a stale one abandoned
- [ ] 2.3 A slow chain does not hold the UI's device queue
- [ ] 2.4 Test with an injected kernel that takes seconds; live scenario with Datamosh
