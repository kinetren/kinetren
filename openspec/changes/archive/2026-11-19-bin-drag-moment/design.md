# Design

## D1. The moment is the press point

The scrub strip already maps the pointer's x to the file's time (F-MED-008). The press that starts a
drag on that strip is the moment the person was looking at, so the card records it at
`drag_started` (`KtApp::bin_press_moment`) and the carry-out payload `BinDrag(media, moment, length)`
takes it. A press off the strip (the name, an audio or generator card) carries moment 0: the start
lands under the mouse, as before.

## D2. Which point lands under the mouse

`timeline::BinLanding` from the modifiers of THIS frame: none — the moment; Alt — the start; Alt+Shift
— the end (his "еще какую-то комбинацию": Shift is added to Alt, the end being the start's mirror; the
right button he also named is the context menu and stays so). The tick under the mouse is snapped
first (the magnet catches what is under the mouse), then the start is that tick minus the offset,
held at the time floor; a start held at the floor shows in the ghost, which is drawn from the same
number as the drop. Alt pressed with the button is the window manager's move on many Linux desktops;
pressed after the drag began it reaches the program — the live scenario presses it so.

## D3. The ghost is the event

The silhouette was a fixed ~3 s mark at the mouse for every drop. For a bin card the length is known
(the thumb service's duration; a reel's content end), so the ghost is that length from the start the
drop will use, with a white tick at the mouse. A file dropped from the system is opened only on the
drop: its ghost stays the mark.

## D4. The card's hover text

egui keeps a dragged widget "hovered"; the card's file-and-loudness text, drawn after the timeline,
replaced the drop's line in the status bar for the whole carry. A dragged card writes no hover text.

## D5. The carried card holds its moment

The card's strip follows the pointer's x (F-MED-008); while carried, the pointer is far right of the
card, so the strip showed the file's last frame and its timecode — not what lands under the mouse.
A carried card scrubs at its press point: the picture and the timecode are the moment that lands.
Found in the first live run (screenshot `01-step-moment`).

## Class walk

The mechanism: a drop places by the start alone, whatever the person aimed at. Checked:
- bin card → timeline track: done here (files, generators, reels by their length);
- file from the system (X11 DnD): no moment exists before the open, the length is not known until it;
  the start at the mouse and the ~3 s mark stay;
- bin card → morpher source (`morpher_editor.rs`): a source substitution, not a placement; the moment
  is carried but not used — a source-in from it is not asked for;
- "Place at the cursor" in the card's menu: places at the playhead, no pointer and no moment; unchanged;
- dragging an event on the timeline: the grab point already stays under the mouse (`DragState.grab`).
