# Proposal: motion-driven-params

> IMPLEMENTATION-READY (spec-engine, revision 3, 2026-11-25, orch `fc60a5d9`) after the owner's answer (`kkt-26.11.20b` B7). Registry
> `v26.10-index.md` TD2. Note F-AUT-014. Tests: t3947–t3951, t3967, t4146, t4147.

## Why

The owner, `docs/ktprompts/v26.10.txt` lines 160–164, whole:

> TODO: туда же в капилку падающих букв при ускорении-замедлении, можно было бы прикрутить эффект fisheye и в момент
> ускорения-замедления его начать активировать, а когда обычно все двигается - отключать его плавно. это тоже в копилку,
> не хочется руками вылавливать и подстраиваться под уже существующие кривые тормажения-скольжения которые находятся у
> другого события.
> Ну и важны моменты:
> 1. ускорение то можно наложить на всю композицию или весь морфер - и надо как-то удобно продумать это связывание с тем
> же fisheye/
> 2. в момент обычного проигрывания некоторого участка t0-t1 - у нас fisheye не должен активироваться а вот в момент
> перемотки тот же участок события/композиции t0-t1 уже будет с fisheye, т.е. возникает двоякость-композиционность. Это
> как с плавностью движения букв при замедлении.
> Нужно очень хорого над этим подумать и как это связано например с физикой.

Asked with this whole item (2026-11-22, `kkt-26.11.20b.txt` line 19), he chose "Скорость монтажа": "перемотка" is the
EDIT's speed — a speed curve put on a composition, a morpher or an event — not the playhead's motion. Revision 1 read it
as the playhead and is withdrawn.

So: the same stretch t0–t1 of a composition plays at normal speed in one instance (no fisheye) and sped up or slowed in
another instance (fisheye while it accelerates or brakes). An effect INSIDE must read the speed of the time map OUTSIDE
it — the speed at which its container plays it — not a curve copied from another event.

## What Changes

- **The outer speed as an input of any parameter**: an effect inside a composition, under a morpher layer, or on an
  event reads the rate of the time map that plays it — the instance's speed for everything inside a reel, the morpher
  layer's warp for what the layer covers, the event's own warp for its own effects — composed when maps nest (a reel
  inside a sped-up reel inside a slowed instance: the product).
- **Three channels of it** for a link: **outer speed**, **outer acceleration** (its slope per second), **speed change**
  (0 while steady, up to 1 while accelerating or braking, falling back smoothly: a peak-hold with attack and release).
  Pure functions of the edit at the moment: preview, render, seek and loop agree.
- **Per instance**: two instances of one reel compute the channels from their own speeds, so the same t0–t1 is plain in
  the normal instance and fisheyed in the sped-up one — the owner's "двоякость-композиционность".
- **One gesture**: a parameter's link menu → "Follow the outer speed change" writes the link with a default mapping
  (0 → the value now, 1 → the parameter's maximum), editable as any link; no source to pick — the container is known.
- The playhead's motion is not an input (revision 1 withdrawn).
- Physics later (F-FX-005): falling letters read the outer acceleration as a force.

## Capabilities

### Modified Capabilities

- `automation`: the outer-speed channels as a link source, resolved per instance and composed through nested maps.
- `compositions`: an instance's speed is visible to the reel's content as its outer speed; its picture and its sub-mix are
  keyed by it.
- `ui-reach`: "Follow the outer speed change".
- `project-format`: the new link source by content.

## Impact

- kt-model: `CurveSource::Outer`, `outer.rs` (the path of time maps and its three channels), `AutoPlan` evaluated with it. kt-engine: the picture reads the channels at the OUTER moment while the plan is
  built (a composition that stands still has one inner moment and a fisheye that must die away: keys cannot say that), one sub-renderer per reel as today; the sound samples them into keys per instance.
- kt-app: the "Driven by…" menu; i18n ×5.
- Format: the next free `.kt` version by content; OTIO: a row in "What OTIO export loses".
- Idle: nothing at rest.
- Refused, with a reason: the speed's own row, generator and title rows (`simulated-elements`), track chains.

## Questions to the owner

Decided (2026-11-22, B7, his line 19): "Скорость монтажа" — the edit's speed, preview and render agree. No open question.
