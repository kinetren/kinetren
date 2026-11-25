# Spec Delta

## ADDED Requirements

### Requirement: A title has In, Out and Loop motions
A title's window SHALL offer In, Out and Loop, each a motion and a length; choosing one SHALL write curves of the title's
own rows in the material domain, replace the slot's former motion in one undo step and leave the rows of the other slots;
a curve edited by hand SHALL be shown as "custom" and left as it is.
Why: the owner's K7 ("the text generator is poor"); curves and not a mode, so the render has one way to read a title.

#### Scenario: Slots compose and are undone
- **WHEN** a 4 s title gets In "slide left" of 0.5 s, Out "fade" of 0.5 s and then In "type on"
- **THEN** the offset row has the hold between; the type replaces the slide; each choice is one undo step; a 0.6 s event halves the lengths instead of overlapping them
- **TEST** t4281 (`src/libs/kt-model/tests/title_motion.rs`)

#### Scenario: The active motion is read back
- **WHEN** the present curves are those of one motion, then a key is moved by hand
- **THEN** the first is recognised with its length; the second is "custom" and no motion is highlighted
- **TEST** t4282 (`src/libs/kt-model/tests/title_motion.rs`)

#### Scenario: The person sets an arrival
- **WHEN** a title's window is open and "Slide from left" is chosen for In
- **THEN** the title slides in on the preview and one Ctrl+Z restores it
- **PATH** double-click a title → its window shows In, Out, Loop → click In → the list shows the motions with little previews; pick "Slide from left", length 0.5 s → the preview slides it in at the start of the event; Ctrl+Z → the title stands still
- **TEST** t4283 (`src/apps/kt-app/src/reach_tests.rs`)
