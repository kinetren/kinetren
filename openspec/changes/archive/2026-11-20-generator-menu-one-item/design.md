# Design

## D1. The selected track

The program has no track selection. "The selected track" is the track of the earliest selected event,
ordered by start and then by id. The event lands at the cursor on that track, like the track menu's
item, which lands at the click. With nothing selected it is video track 1, as the item gave before.
Several selected events on different tracks give the earliest one's track: one rule, no question.

## D2. A kind change keeps the sound pair (found by t3702)

A picture kind with sound (Sync flash, and SMPTE bars with their tone on) sounds through a linked audio
event on the same media. Creation from the menu made the pair (`clip_cmds_for`: one event per stream
the media has). A kind change was a bare `SetMediaPath`, from the window's gallery and from the bin
card alike, so:

- a Sync flash picked in the window had no sound event, and its beep was silent;
- a Sync flash switched to a solid colour kept a sound event that held silence.

With one item on every menu the gallery is the person's only way to Sync flash, so this had to be
fixed. `KtApp::gen_kind_switch_cmds(media, kind)` is the one body for both doors. For a sounding kind,
every picture event of the media without a pair gets an audio event on A1: same start, duration and
source, linked. For a soundless kind, the pairs are removed before the path changes. It is one batch,
so one Ctrl+Z gives both halves back (t3713).

## D3. The class walk

The mechanism: one entry builds or changes a generator differently from another. Checked:

- Edit menu, bin background, track menu: one item, one body (`generator_item`; t560 reads the sources).
- The window's Kind gallery and the bin card's "Change kind": one body for the kind change (D2).
- The card's "Change kind" keeps its list of kinds. It is a different verb on a media that has no
  window, and his answer named the create items.
- The kind gallery holds the kinds of the event's stream (t3128, generator-from-track D4). From Edit a
  sound kind is reached by selecting an audio event, and the gallery of a sound event changes among the
  sound kinds.
- Not fixed here, the same class: switching the SMPTE tone on in the window of an event created
  without it gives no sound event either. The value lives on the event, and no pair is made when a
  flag changes. Recorded as found-defects 1000.

## D4. Tests changed by this decision

t1164 picked Tone from a submenu with nothing selected. It now selects an event on audio track 2 and
reads a tone at the cursor there. t560 reads `generator_item` in Edit and the bin. t3702, t3705, t3708
and t3710 reach their kinds through the window's gallery (`create_from_edit`). The tour test of lesson
1 presses the item and then Text in the outlined Kind field.

## D5. Not taken

A remembered "last created kind" for the item. His answer chose the track, and a memory would give two
answers to one click.
