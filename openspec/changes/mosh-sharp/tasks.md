## 1. Note

- [x] 1.1 Feature note F-FX-015 `docs/features/fx/simulate/datamosh-sharp-carrier.md`

## 2. Codec

- [ ] 2.1 `resample` convert (input size → codec size), CPU reference and shader, same bytes
- [ ] 2.2 The `sharp` row; the carrier step (`coded` flags, `carry`), CPU reference and shaders
- [ ] 2.3 The reference walk and the batched walk carry at the input size; keys, needs, output size

## 3. Host

- [ ] 3.1 `StreamKernel::carried`, `stream::coded_size`; `stream_quality` follows the layer's size

## 4. Checks

- [ ] 4.1 Tests t2970–t2976; codec/engine tests; clippy
- [ ] 4.2 Live scenario (sharp against soft), load at rest with the effect window open
- [ ] 4.3 found-defects 734 and rows 760+, registry U1/U2, What's new; target-prune
