# Keyboard and mouse

Every key and mouse gesture kinetren understands. The keys work during playback and from the
detached preview window too. While you type into a text field, the keys go to the field.

## Transport

| Keys | Action |
|---|---|
| <kbd>Space</kbd> | play / pause |
| <kbd>Space</kbd> twice quickly | stop at the region start, or at the project start when there is no region |
| hold <kbd>Space</kbd> | play faster, ramping up to 4× over five seconds; release to return to the normal speed |
| <kbd>A</kbd> / <kbd>Home</kbd> | back to the previous marker, region edge or event edge |
| <kbd>D</kbd> / <kbd>End</kbd> | forward to the next marker, region edge or event edge; after the last one, the end of the project |
| <kbd>Shift</kbd>+<kbd>Home</kbd> / <kbd>Shift</kbd>+<kbd>End</kbd> | the very start / end of the project |
| <kbd>Ctrl</kbd>+<kbd>L</kbd> | type a timecode to jump to |

When the region has been played to its end, <kbd>Space</kbd> plays it again from its start.
With the loop on, it starts from the region start when the playhead is outside the region.

## Editing

| Keys | Action |
|---|---|
| <kbd>S</kbd> | split at the playhead: the selected events, or every event under the playhead when nothing is selected |
| <kbd>Delete</kbd> / <kbd>Backspace</kbd> | delete the selected morphers and cameras; otherwise every event crossing the region; otherwise the selected events |
| <kbd>Ctrl</kbd>+<kbd>C</kbd> | copy what lies in the region, or the selected events when there is no region |
| <kbd>Ctrl</kbd>+<kbd>V</kbd> | paste at the playhead, on the track under the mouse; the playhead moves to the end of the pasted block, so pasting again chains copies |
| <kbd>Ctrl</kbd>+<kbd>Z</kbd> | undo |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Z</kbd> / <kbd>Ctrl</kbd>+<kbd>Y</kbd> | redo |
| <kbd>Ctrl</kbd>+<kbd>S</kbd> | save |
| <kbd>Alt</kbd>+<kbd>M</kbd> / <kbd>Alt</kbd>+<kbd>S</kbd> | mute / solo the track of the selected event, or the track under the mouse |
| <kbd>Ctrl</kbd>+<kbd>F</kbd> | search events, markers, tracks, media and bin groups by name: <kbd>↑</kbd> / <kbd>↓</kbd> choose, <kbd>Enter</kbd> jumps, <kbd>Esc</kbd> closes |
| <kbd>Esc</kbd> | step back, one thing per press: close the speed field, search or the top window, cancel a drag, clear the region and the selection (the loop stays on), then collapse an expanded reel, then leave the reel |

Pasting events that have names first asks for a name prefix: <kbd>Enter</kbd> pastes,
<kbd>Esc</kbd> cancels. Cutting the region's interior out of the events is **Cut out the region interior**
in the menu of a track's empty space.

## Region and markers

| Keys | Action |
|---|---|
| <kbd>I</kbd> / <kbd>O</kbd> | region start / end at the playhead; with no region, <kbd>I</kbd> starts a one-second region |
| <kbd>Shift</kbd>+<kbd>A</kbd> / <kbd>Shift</kbd>+<kbd>D</kbd> | jump back / forward and drag the left / right region edge along; with no region, select the stretch just jumped over |
| <kbd>L</kbd> | loop the region on / off |
| <kbd>M</kbd> | add a marker at the playhead, or remove the one next to it; with the playhead inside the region, add or remove a range marker over the region |
| drag a marker | move it |
| drag a region edge | move it |
| double-click a range marker on the ruler | region over it, with its events selected |
| <kbd>Shift</kbd>+double-click an event | region over the event and its linked partners |

## Reels

| Keys | Action |
|---|---|
| <kbd>Enter</kbd> on a selected reel | expand it in place; on an expanded one, enter the reel |
| double-click a reel | expand it in place / collapse it |
| <kbd>Esc</kbd> | collapse the expanded reel; again — leave the reel you entered |
| drag the edge of an expanded reel's window | resize the instance |

## View and zoom

| Keys | Action |
|---|---|
| wheel | scroll along time |
| <kbd>Ctrl</kbd>+wheel / pinch | zoom around the mouse |
| <kbd>Shift</kbd>+wheel | scroll tracks |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+wheel | change all track heights, keeping the track under the mouse in place |
| click near the left / right edge of the tracks | place the playhead there and jump the view that way |
| drag past the edge of the tracks | the view scrolls along |

## Mouse on the timeline

| Keys | Action |
|---|---|
| click the ruler | move the playhead (it snaps to markers) |
| drag the playhead on the ruler | scrub |
| drag on the ruler or on empty space | region |
| click empty space | move the playhead |
| <kbd>Ctrl</kbd>+drag on empty space | rubber-band selection |
| click an event | select it and move the playhead there |
| <kbd>Ctrl</kbd>+click an event | add it to the selection or take it out; the playhead stays |
| drag an event | move it, the selection moves together; drop it over a neighbour for a crossfade |
| <kbd>Ctrl</kbd> while dragging | switch the magnet off |
| drag an event's edge | trim |
| drag a top corner of an event | fade at that edge |
| drag the level line | opacity (video) / gain (audio) |
| double-click the level line | full opacity / 0 dB |
| double-click an event | effects window |
| drag a transition's edge / its cut | resize / roll the edit |
| double-click a transition | transition properties |
| drag a morpher or a camera, or its edges | move / resize |
| double-click a morpher / a camera | morpher editor / camera properties |
| drag an area's edge | resize the area |
| click a track badge M / S / lock / FX | mute / solo / lock / track window |
| double-click a track header | rename the track |
| drag a track header / its bottom edge | reorder / change its height |
| right-click a track header → **Reset height** | back to the default height |
| right-click anything | its menu |

With the **Selection tool** button on, a plain drag on empty space selects events, and
<kbd>Ctrl</kbd>+drag adds to the selection.

## Windows and panels

| Keys | Action |
|---|---|
| click the preview | play / pause |
| double-click the preview | detach it into its own window; in that window — put it back |
| <kbd>Esc</kbd> in the detached preview | put the preview back |
| right-click the preview | frame controls |
| double-click the timecode | type a position |
| digits, <kbd>Backspace</kbd>, <kbd>Enter</kbd>, <kbd>Esc</kbd> in the timecode | digits fill from the right, a pasted timecode is laid in whole; <kbd>Enter</kbd> jumps, <kbd>Esc</kbd> cancels |
| <kbd>Shift</kbd>+click the Back / Forward buttons | jump and drag the region edge along |
| drag the top / bottom half of the speed strip | constant speed / shuttle (release to return) |
| <kbd>Shift</kbd> while dragging the speed strip | fine adjustment |
| double-click the speed strip | back to 1× |
| right-click or <kbd>Ctrl</kbd>+click the speed strip | type a speed from 0.01 to 8, a minus plays backwards; <kbd>Enter</kbd> applies, <kbd>Esc</kbd> cancels |
| drag the fader or turn the wheel over it | preview volume, 1 dB per wheel step |
| <kbd>Shift</kbd>+wheel over the fader | 0.2 dB per step |
| double-click the fader | 0 dB |
| click the M button at the bottom of the meters | mute / unmute the preview |
| click a card in the media bin | place it at the playhead |
| <kbd>Ctrl</kbd>+click / <kbd>Shift</kbd>+click a card | add it to the bin selection or take it out |
| drag a card | onto the timeline, or to another spot in the bin |
| drag across the bin background | select cards with a frame; <kbd>Ctrl</kbd> or <kbd>Shift</kbd> adds to the selection |
| double-click a generator card | its parameters |
| click / drag / cross on a slot of the effect chain | select / reorder / remove |
| double-click a parameter label | reset the parameter to its default |
| right-click a parameter label inside a reel | publish the parameter of the composition |
| click empty space in the curve graph | add a key to the active curve |
| drag / double-click / right-click a key | move it / delete it / its menu |
| wheel / <kbd>Ctrl</kbd>+wheel over the curve graph | scroll / zoom the graph |
| drag the graph's ruler | move the main playhead |
| drag a segment's handle; <kbd>Shift</kbd>+drag sideways | the curve's tension or amplitude; the number of periods |
| <kbd>Ctrl</kbd>+click the key-before / key-after button | lock every curve of the slot, not just one |
| <kbd>Tab</kbd> / <kbd>Enter</kbd> in a formula field | complete a name / apply |
| drag a point of the equalizer graph | its frequency and gain |
| wheel over the middle point of a band | its Q |
| drag the picture / a corner / the rotate handle / the anchor dot / a crop side in the transform frame | move / scale / rotate / move the anchor / crop |
| <kbd>Shift</kbd> while rotating | 15° steps |
| <kbd>Alt</kbd> while grabbing in the transform frame | a wider grab zone for the anchor dot |
| <kbd>Enter</kbd> / <kbd>Esc</kbd> while renaming a track, marker, area, card or published parameter | save / cancel the name |
| drag / double-click the title bar | move the window / maximize or restore it |

The status line always shows what the thing under the mouse does.
