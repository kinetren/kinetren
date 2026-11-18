# Design

## D1. Our layout only

`OWN` is the owner's table; `C` is added there at his request. In the Premiere layout C is the Razor
tool, in Resolve and Final Cut B is the blade and C is free but means nothing there by habit; binding
C in a borrowed layout would give their letter a meaning those editors' users do not expect. The
command stays unbound there and is reachable from the Edit menu and Customize. The settings code stores
only diffs from the style, so a new command number changes no existing code (t1702 green).

## D2. One body

The key, the Edit item and the empty-space item call `punch_out_action` (gap-keeping, through
`apply_batch_restoring`, one undo step). The Edit item and the empty-space item print the chord from
the table (`controls.label`), never a literal (t1715).

## D3. No region is an error, not silence

The empty-space item exists only with a region; the key and the Edit item exist always, so without a
region they say why nothing happened (rule 16) through the notice line. Shift+C falls off to C as
Shift+S does to S (`lookup`).

## Class walk

The mechanism: an action reachable only from one right-click menu, no key and no main-menu door.
Walked over the region's actions: Delete clips in the region — Del and Edit › Delete (has both);
Clear the region — Esc (the cascade), a key in the borrowed layouts and the ruler menu; Loop — L and the toolbar; Marker over the
region — M; Save area from the selection — menu only (an occasional naming action, not a keyboard
edit: left); Cut out the region interior — fixed here. Nothing else on the region was menu-only.
