## 1. Diagnosis

- [x] 1.1 Read the owner's trace `fails/26.11.18/new`: the producer's unkept moments, the history's cost, the ring's waits (design Context)

## 2. Producer

- [x] 2.1 `render_ahead::Waits`: an incomplete moment waits for the walked positions / decoded frames it lacked; a new lap clears the waits; the display composes a waiting moment on demand (D1, t2931)
- [x] 2.2 `VideoRenderer::stream_compose_steps`; job record `b` 4 for a short history

## 3. Arbiter

- [x] 3.1 `Ring::piece_limit` = half the bound; t2930; t2356's unmeasured claim (D2)

## 4. Trace

- [x] 4.1 `trace loop`: the producer's outcomes and the ring's waits per lap (D3, t2932)

## 5. Checks

- [x] 5.1 Measurements on our fixtures (design Measurements)
- [x] 5.2 Tests of kt-gpu, kt-engine, kt-cli; clippy; live scenario `render_ahead`; target-prune
- [x] 5.3 found-defects rows, kkt-26.11.18 T2, What's new
