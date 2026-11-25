# Proposal: chroma-key-upgrade

> DESIGN ONLY, revision 3 (2026-11-24, base `0c2a4065`): made implementation-ready against today's code; revision 2
> (2026-11-20) had the owner's A22. Revision 3 corrects three things the code contradicts (design D0). Tests:
> t3558–t3569. No open question.

## Why

The owner's items, whole:

- `kkt-26.11.01b.txt` line 4 (B3): "greenscreen очень скудный, и откровенно плохого качества - его надо доработать, а еще
  проще было бы кнопку автоанализа добавлять, чтобы не подбирать всё руками"
- `kkt-26.11.20.txt` line 30 (A22), the one answer to Q38, Q39, Q46 and Q47: "Все ключи на новую модель"

**The reading, checked.** A22 answers Q38 by its text: every greenscreen goes to the new model, old projects look
different after opening. It says nothing of Q39 (what Analyze looks at), Q46 and Q47; the registry took the
recommended answers there (five frames; the pro keyer's two). "Все ключи" also decides what happens to a key whose
colour is not a screen's (a neutral one): it may not keep the old formula, and it may not silently stop keying (D2).

**Today.** `greenscreen` (`plugins/fx-video-color/src/lib.rs:311–423`) takes the Euclidean RGB distance to the key with a
hard threshold and a ramp, and despills by pulling the key channel down to the larger of the other two. F-MSK-004 found
why it is poor: a distance in RGB means another thing on green than on blue; no matte is seen while tuning; a despill by
subtraction greys light hair. The eyedropper (F-MSK-007) is built.

## What Changes

- **The colour-difference model** for every greenscreen: the screen-ness of a pixel is its key channel's excess over the
  balance-weighted mix of the other two, relative to the screen's own excess. The distance kernel is deleted (tap and
  barrier). `tolerance`, `softness` and `spill` stay the rows of the model, now measured along the key axis: a pixel that
  differs from the screen only in the key channel keys exactly as before, so an old key opens near its former cut
  and its curves, links and presets go on working.
- **New rows**: `balance_auto` + `balance`, `choke`, `soften`, `despill` (restore / replace / subtract), the replacement
  colour. Omitted at their defaults.
- **Despill with luminance restore** by default.
- **Views in the Viewer**: while the Viewer (Ctrl+Shift+click on a node) is on, its badge over the preview carries
  Composite / Matte / Status, and Screen for a key. A preview of the cut chain only; never in the model, never rendered.
- **Analyze**: one button beside the key colour; five moments across the owner's span at the key's input; sets the
  colour, balance, tolerance, softness and despill in one undo step and says what it set, or says why it cannot.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: the greenscreen's model, its matte operations and despill, its views, the analysis.
- `project-format`: the new rows by content.
- `ui-reach`: the Analyze button, the views in the Viewer's badge.

## Impact

- `plugins/fx-video-color`: the rows, the tap and barrier kernels, three new passes, the internal view stage.
- kt-plugin-api: `VideoKernel::neighbours` (a point-wise plugin that needs neighbours at these values),
  `UiDecl.internal`.
- kt-fx: `geom::is_pointwise` asks it; `PluginTap` cooks from every row, not from the first eight.
- kt-app: the Viewer's badge, the Analyze button and job, i18n ×5.
- Stored frames of every case holding a greenscreen are stored again in the commit that deletes the distance kernel.
- Format: the next free version by content. OTIO: rows in metadata as before. Idle: nothing at rest.
- The persistent cache: a kernel's WGSL is part of a frame's key, so stored frames of old keys are misses.

## Questions to the owner

None. The questions Q38–Q39 as asked are in the git history of this file (ffe4eebf).
