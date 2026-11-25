# Design: memory-breakdown

> DESIGN ONLY, revision 3 (2026-11-24), base `0c2a4065`.

## D0. What revision 2 said and the code contradicts

1. **"accounts + not counted = resident" over every tenant.** Six of the ledger's eight kinds (`Kept`, `Upload`, `Readback`,
   `Transient`, `Compute`, part of `History` through `Compute`) are device memory: on a card they are not in the resident
   memory at all. The bar would have shown "not counted 0". D2 splits by where the bytes are.
2. **The sidecar `ktindex.db`.** Gone; the cache folder's tenants (`kt_media::cache_folder::Tenant`: frames, mix, copies,
   reels, media) are the project's disk, counted already by `Folder::known_sizes` without touching it. No walk every 5 s.
3. **Nine report-only kinds with a `report` call.** An account with base 0 of a kind outside `GROWING` is exactly that
   (`Upload`, `Compute` live so). And most of the nine are holders only the interface reads: they are asked when shown.
4. **The model's estimate per revision.** A walk of 2 000 events at every step of a drag, for a window that is closed.
5. **VRAM = "kt-gpu's pools and live textures".** The pools are the part we count; the interface's textures, pipelines,
   swapchains are not in them. D3.
6. **The popup on hover.** t83.

## D1. The accounts

`Tenant::Decoders = 8`, `AudioState = 9` (device-decode-sources appends `Surfaces`: whichever merges second takes the
next number; the numbers are in traces). Not in `GROWING`; `growth_budget`, `room` and every grant are unchanged (t3962).
- **Decoders**: `kt-ffmpeg` after open knows the coded size, the pixel format and the thread count: `(threads + DPB) ×
  surface` (the software decoder's own pool is not readable; this is FFmpeg's sizing). Account set at open, dropped with
  the decoder. It is an estimate and the list says "about".
- **AudioState**: `fxstore.rs`: what every live processor's buffers hold (reverb lines, rings, the kept mix excluded: that is
  `Mix`), summed when the store changes, and the output ring.
`mem_used` records follow by themselves (`Tenant::ALL`); `MemSample`'s sum grows by two kinds.

## D2. The sections

`kt_app::mem_view::snapshot()` makes one `MemView { resident, sections: Vec<(Section, u64)>, not_counted, video: Vec<…>,
video_total, shared: bool }` from:
- the ledger: `Frames`, `Mix`, `History`, `Decoders`, `AudioState` — the process's; `Kept`, `Upload`, `Readback`,
  `Transient`, `Compute` — the device's (`Tenant::on_device`). **On a software rasterizer** (every device's type is `Cpu`)
  the device's kinds are in the process's memory: they join the first bar and the second is not drawn. On an integrated
  card the second bar says "shared with the memory": the driver decides what of it is resident, so it is not subtracted;
- asked now: the model (`Project::approx_bytes` of the open document, memoized by revision), undo (`TimelineModel::memory`:
  each `Snapshot` keeps its own estimate in a cell, at most eight new ones per reading, "…" until all are known: 80
  steps of a large project are ten seconds of one-a-second readings, never a stall), waveform peaks (`KtApp.peaks`), reel
  previews (`reel_preview` ready frames), picker cells (`fx_thumbs`), the cache of title layouts;
- egui's textures (`ctx.tex_manager().read().allocated()`, `TextureMeta::bytes_used`): video memory, a section of the
  second bar.
`not_counted = resident − Σ process sections`, never below zero. Sections under 1 MB are summed into "other". `resident` is
the once-a-second reading of `PerfState`, and the sections are read in the same frame.

## D3. Video memory

wgpu's feature `counters` (a counter per allocation, no cost worth naming; in the workspace's `wgpu` line, so kt-app's egui
device counts too): `device.get_internal_counters().hal.{buffer_memory, texture_memory}` — kept by the Vulkan and DX12
backends (wgpu-hal 30, `vulkan/device.rs:1063, 1210`, `dx12/suballocation.rs:196, 220`). `kt_gpu::memory::video()` sums them
over the devices of the process, each once: the live `Compiled`s hold their devices and register themselves weakly;
`compute::host_device()` and the window's device are named. On GL the counters are zero: the sum is the ledger's device
kinds plus egui's textures, shown as "≥ 1.1 GB". Unknown (no device yet): the counter shows "3.2 GB" alone.

## D4. The status bar (B12)

`app.rs:8573–8590`: the two items become one: `"{0} GB · VRAM {1} GB"`, one decimal, read at most once a second in frames
drawn anyway (as `PerfState::resident` today; video the same way). The underruns and the rest of the bar stay. Probe
`status.memory`. Its hint (the one hint line, priority hover): "Memory 3.2 GB: decoded frames 1.1 · undo 0.4 · kept mix 0.2
· not counted 0.9. Video 1.1 GB: kept frames 0.5 · uploads 0.3. Project on disk 4.3 GB" — the three largest of each, built when
hovered and once a second while it is. A click opens Diagnostics with the section in view.

## D5. The section in Diagnostics

In place of the "Caches: …" line (`app.rs:11316–11346`), above the graphs, inside the scrolling part; its height is fixed by
the number of sections possible (a figure gaining a digit moves nothing: t918). Each bar is one row of segments in the order
of size, "not counted" last in grey; a segment's hover puts its name, size and what it is into the hint; under each bar
the list in two columns. The room the caches may still take (the old line's last number) is the last row of the first
list. The first performance row ("Program … MB · rest …", t2398) stays, its "rest" becomes D2's `not_counted` so the two
numbers of one window agree (the owner's line 143 is exactly two numbers that do not).

**The project's disk**: the cache folder of the open project, by tenant from `Folder::known_sizes`; not counted yet: one
recount on the `media` lane when the section opens, "counting…" until it answers. The folder belongs to the project's
FOLDER (his B2): other projects in it are named ("also: B.kt"). Plus the project file and its recovery file (two
`metadata` calls in the same job). A segment's hint says what clearing it costs; the way there is Preferences ›
Persistent cache (a link, `CachesUi.show`). Untitled project: "no folder yet".

## D6. Idle and load

The section closed and the counter not hovered: two readings a second in frames that are drawn anyway — none at rest. The
section open on a paused program: the window is redrawn only as today (its graphs decide), a reading is ≈ 0.1 ms without
the undo estimates. `approx_bytes` runs at most once a second and only when the revision moved.

## D7. Class walk — "a number that does not say what it counts, or two that disagree"

| Number | Today | Here |
|---|---|---|
| status: frames V/cap | decoded frames | leaves the bar (B12); Diagnostics' occupancy line has it |
| status: memory | resident | GB, with VRAM and the hint |
| Diagnostics: Program · rest | resident − all tenants, device ones too | rest = not counted (D2): the same number as the bar's |
| Diagnostics: caches line | eight kinds, device summed | the bars |
| occupancy | frames against their bound | unchanged |
| trace `mem_sample` sum | all kinds | unchanged meaning, two kinds more; `trace machine` names them |
| Preferences › Persistent cache | per folder | unchanged; the third bar is the open project's part |
| the render window's file size | file | — |
| kt-cli's printed memory | resident | — |

## Rejected

- `/proc/self/smaps` — "не про тупое чтение procinfo". - `generate_allocator_report` — it lists every allocation under the
  allocator's lock, a second's reading of thousands. - `VK_EXT_memory_budget` — the whole machine's heaps. - A walk of the
  folder every 5 s. - A pop-up.
