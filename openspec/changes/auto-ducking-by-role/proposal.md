# Proposal: auto-ducking-by-role

> DESIGN ONLY (spec session), revision 2 (2026-11-22) from the owner's whole item (`kkt-26.11.20b` B5). Registry
> `v26.10-index.md` N3, round R.75. Note F-AUD-007. Tests reserved: t3911–t3918, t3966–t3969.

## Why

The owner, `docs/ktprompts/v26.10.txt` lines 84–88, whole:

> Также хотелось бы в функции что-то типо: Авто-мут трека(-ов) аудио во время основного контента, т.е.
> вступление-заголовок1-контент-заголовок2-контент2-заголовок3 и т.д.,
> во время контента звук контента и задний звук, а во время заголовка может быть либо автоматом усиление заднего звука
> или мут заднего звука и размут заголовочного звука. можно это всё руками проставлять (т.к. звуки разные и т.д.) а можно
> какую-то штуку придумать, чтобы оно автоматом могло туда-сюда переключаться.
> Эта функция может быть по другому в редакторах устроена и называется - лучше как у них сделать, я могу чего-то не
> знать и сузить смысл.).
> мне казалось у нас даже какой-то код дакинга есть

His answers: 2026-10-30, question 10 — "п.1. по типу клипа. еще должен сказать - что у нас дакинг сделан как-то вообще не
интуитивно понятно... очень не очевидно что и куда крутить чтобы было более-менее нормально, и ощущение что у нас на
грани возможностей он работает... очень втупую сделан дакинг у нас." 2026-11-22 (`kkt-26.11.20b.txt` line 16): "наверное
имеется ввиду несколько дорожек а не две. ну и да ручные правки тоже полезны. а вообще мне кажется там в моем тексте
понятно было что написано а ты подрезал суть".

What the item asks, read whole: a video runs in SECTIONS (intro, title 1, content, title 2, content 2…). During a
content section one hears the content's sound AND the background. During a title section either the background is
raised automatically, or the background is muted and the title's own sound is unmuted. This can be set by hand,
since the sounds differ, or switched automatically. Name and build it as the editors do.

Today: R.65's compressor with a key input, keyed by ONE track, set by threshold, ratio, attack and release; nothing
knows where the titles are.

## What Changes

- **Auto-ducking** (the industry's name: Premiere's Essential Sound "Ducking", Resolve's "Ducker") on a background
  track, as one section of its track window with two rules in the person's terms:
  - **During content**: the background goes down by N dB under the content's sound (default −12 dB) and comes back in
    the pauses (Fade, ms). The content's sound is the key: **several tracks** ticked in a list, and/or every event
    of a role (Dialogue, Music, Effects, Ambience, Title sound) wherever it lies.
  - **During titles**: one choice — **Raise the background** by N dB (default +6 dB), or **Mute the background** (the
    title's own sound plays; a title sound muted outside titles is unmuted there), or **Leave it**. A title section is
    where a title event is on screen (a Text generator event, or any event tagged "Title"), with fades of its own.
- **Sections come from the edit, not from loudness**: titles are known from the timeline (title events), content is
  heard from the keyed tracks; moving a title moves its section; nothing is "generated" and left stale.
- **Hand edits stay useful**: the automatic gain is drawn as a band on the background events; the person's own level
  keys on those events add to it (dB on top), so a hand fix at one spot survives every re-edit; "Convert to volume
  curve" bakes the band when the person wants only hand keys.
- **Roles** on tracks and events (an event without its own takes its track's) — the clip type the owner chose; they
  also feed the key list.
- **The R.65 compressor stays the device** (one widget, rule 24): the content rule writes its rows; its own rows are
  visible, dimmed, under "Details".

## Capabilities

### Modified Capabilities

- `audio-fx`: a key made of several tracks and/or roles; the title-section gain from title events; the hand level
  adding to the automatic gain.
- `ui-reach`: roles; the Auto-ducking section with its two rules; the band; Convert to volume curve.
- `project-format`: roles, multi-track and role keys, the title rule by content.

## Impact

- kt-model: `AudioRole` on tracks and events; `SidechainKey::Set { tracks: Vec<TrackId>, roles: RoleSet }`;
  `Ducking { content: {reduce_db, fade_ms, key}, titles: TitleRule::{Raise(db), Mute, Leave}, title_fade_ms }` on an
  audio track; title sections derived from title events (`Project::title_spans`, indexed).
- kt-engine: the key mix of several tracks/roles per window; the title gain as a deterministic envelope from the
  title spans (no state), multiplied into the background's track stage; the hand level of the events is summed in dB
  after it.
- kt-app: the track window section, role menus, the band (analysis lane, cached by content), i18n ×5.
- Format: the next free `.kt` version by content; OTIO: a planned row (a foreign editor plays the background flat).
- Idle: nothing runs at rest; the band is computed once per content hash on a background lane.

## Questions to the owner

Decided (2026-10-30, question 10): by the clip type. Decided (2026-11-22, B5): several tracks, hand edits useful.

**Q1. What counts as a "title" section?** Your item: "вступление-заголовок1-контент-заголовок2-контент2-заголовок3 и т.д.,
… во время заголовка может быть либо автоматом усиление заднего звука или мут заднего звука и размут заголовочного
звука". Today the program knows Text generator events (titles) on video tracks, and nothing else marks a section.
- (a) **A section is where a title event is on screen: every Text generator event, plus any event (picture or sound)
  you tag "Title"** (*recommended*: your titles are found by themselves; a title made otherwise is tagged once).
- (b) Sections are named areas on the timeline that you mark "Title" by hand.
