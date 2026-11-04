## Why

The owner (2026-11-04, C2 of kkt-26.11.03): the math of the whole program should run on Vulkan rather
than on hand-written AVX; the byte references may go ("и видео и аудио можно удалить/обновить
эталон"), ARM is needed, no new kernel language, and the CPU copies are deleted ("да, удаляем").
F-EXT-004 showed the Datamosh encoder as a plain WGSL compute shader on software Vulkan: 34 ms against
67 ms per 1080p P-frame, 46.01 dB both.

## What Changes

- A compute door in the plugin contract: a kernel names WGSL compute shaders and buffers, and the host
  runs them on its own Vulkan/DX12/Metal device. A plugin still sees nothing of wgpu.
- **BREAKING**: the Datamosh encoder (motion search and residual) runs only as compute shaders. Its
  CPU text is deleted; vectors and bytes change, and with no compute device the effect says so.
- The later stages (the remaining kernels, the CPU composite) are not in this change: deleting the CPU
  composite costs export time on a machine without a card, which the owner must see first.

## Capabilities

### New Capabilities

### Modified Capabilities

- `plugins`: the compute door and the refusal of a compute shader that does not compile.

## Impact

`kt-plugin-api` (new `compute`), `kt-gpu` (the compute host), `fx-video-codec`, `kt-app` and `kt-cli`
(installing the host), the Datamosh tests.
