# Proposal

## Why

The owner, kkt-26.11.01 item 14 (registry K14), verbatim: "Perfomance надо переместить в окно
diagnostics, чтобы там три графика было" — Performance must move into the Diagnostics window, so
that there are three graphs there. Today the "Performance" box takes the bottom of the left column
under the media bin (text rows plus two graphs: Quality and FFmpeg activity), and the Diagnostics
window holds the third graph (cache churn), the cache occupancy bar and the problem log. A person
diagnosing a slow spot has to look in two places, and the box squeezes the bin all the time, even
when nobody is diagnosing anything.

## What Changes

- The left column's "Performance" box goes away; the media bin keeps the whole column.
- The Diagnostics window gets a "Performance" section with every line the box had (cache MB,
  underruns render/out, preview size or decode step, clips/media, latency/callback, V-shown,
  V-skip/back, GPU adapter, FX backend, compose cost, FFmpeg totals, "Save the playback journal"
  with the stall count) and all THREE graphs: Quality (FPS / V-fps / A-hit / V-hit), FFmpeg
  activity (Total / Open / Read / V-dec / A-dec) and Cache churn (evict / miss / no-read), next to
  the cache occupancy bar and the problem log that are already there.
- The three graphs share the window's height: enlarging the window enlarges the graphs (the
  owner's standing complaint of item 12, "looking through a needle's eye").
- A graph is redrawn from a cache built once per sample: drawing it on a frame without a new
  sample allocates nothing (invariant 12); the auto-scale moves per sample, not per UI frame.
- The box's "Economy UI" checkbox is NOT carried over: the same switch already lives in View
  (the owner's item 13 forbids the same checkbox in two places); its hint moves onto the View item.
- Entry points stay: the gauge button of the toolbar and View → Diagnostics… open the window.
- The always-visible layer stays the status line (vfps, cache, underruns in ms).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `ui-timeline`: the requirement "The performance panel tells the person the truth" now says
  where the panel lives (a section of the Diagnostics window, not the left column) and that the
  window carries three graphs that grow with it and cost nothing while idle. `telemetry` is not
  touched: nothing new leaves the machine, the window is local.

## Impact

- `src/apps/kt-app/src/app.rs` — `draw_perf_box` removed from the column, its body becomes the
  section of `draw_diagnostics`; the window's default/min size grows.
- `src/apps/kt-app/src/graphs.rs` — a draw cache (lines as one mesh, labels as galleys), the
  scale step in `push`.
- `src/apps/kt-app/src/toolbar.rs` — the View "Economy UI" item takes the hint.
- `i18n/*.toml` — `perf.economy` removed (its text is `menu.view.economy`).
- Tests: t947 (opens Diagnostics now), t128 (measures the View item), new t1145–t1150 in a new
  `src/apps/kt-app/src/diag_tests.rs`; live scenario `src/scripts/live/diagnostics.py`.
- README's "Diagnostics" screenshot (`readme_shots.py` already opens the window by `tb.diag`).
