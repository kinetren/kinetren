# Tasks: pipeline-prewarm

- [ ] 1.1 The instrument `kt-cli first-frame`; baseline on llvmpipe (empty and warm Mesa cache); the class walk in design D1
- [ ] 2.1 kt-gpu: warm draws (`warm.rs`), compiled pipelines per device (`Compiled`, `new_kept`), the cold-draw counter
- [ ] 2.2 kt-gpu: geometry shapes off the frame path (interpreter until warm; worker thread), `prewarm(ids)`, `settle`
- [ ] 3.1 Compute host: `prepare`, `compute::prewarm`, plugin-declared shaders (Datamosh, generator kinds), in-job compile counter
- [ ] 4.1 Disk tier: `PipelineCache` (feature asked where the adapter has it), key, bound, atomic save; determinism test
- [ ] 5.1 kt-engine: `VideoGpuExec::prewarm`, the player prewarms the project's reach at open and on an edit adding an effect
- [ ] 5.2 kt-app: the engine built off the UI thread on the kept device pipelines; kt-cli sets the cache folder
- [ ] 6.1 Tests t2120+: no cold draw after prewarm for every plugin and the core; shapes; compute; cache bytes; at rest nothing runs
- [ ] 6.2 Measure after (first-frame, pause-scan on the showcase, CPU at rest); design D-measure; found-defects 64/139; What's new
- [ ] 6.3 Archive: specs merged
