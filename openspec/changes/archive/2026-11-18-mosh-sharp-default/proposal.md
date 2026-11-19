## Why

The owner's answer (kkt-26.11.18b U7): «Резко» по умолчанию для новых Datamosh? — "Вкл. для новых". A
Datamosh added from now on is sharp; projects already holding one keep their saved look. And t2044 went
red under a parallel run of the merged stage (a red under load is a cause, test-stability).

## What Changes

- `kt_plugin_api::Decl::fresh`: what a NEW instance takes over its passport's defaults; `PluginSlot::new`
  applies it, the file reader does not (a missing row reads the passport default). Datamosh and its sound
  half declare `sharp` = 1.
- t2044's red under load: since mosh-sharp a soft Datamosh with `lines` above the layer read the layer's
  size and resampled every position up to the codec's, a quarter more of a software device's walk with
  nothing in front to save. The input is the layer's only when slots run in front (row 762).
- A sharp carrier re-asked the previous source frame at every step: the walk keeps it (`mosh::src_key`),
  and `needs` does not name it again (row 761).

## Capabilities

### New Capabilities

### Modified Capabilities
- `video-fx`: `sharp` is on for new Datamosh instances.

## Impact

`kt-plugin-api` (`Decl`), `kt-model` (`PluginSlot::new`), `fx-video-codec` (`FRESH`). No file-format change:
a saved slot reads and writes as before.
