## ADDED Requirements

### Requirement: Every press of a slow frame counts outside the key table too
A gesture that reads keys outside `handle_keys` SHALL count every press a frame read (the mask pen's
Backspace/Delete, the search window's arrows), and the preview's deferred single click SHALL wait
egui's widened double-click window measured from frame starts.
Why: class-walk-revision — test-hygiene made `handle_keys` and egui's windows count the busy frame;
these three gestures kept one press a frame and a fixed 300 ms from when the frame ran.

#### Scenario: Two Backspaces in one frame take two pen points
- **WHEN** the pen has three points and one frame reads two Backspaces
- **THEN** one point is left
- **PATH** the picture's menu → Mask → Pen → three clicks on the picture → Backspace twice quickly on a busy machine → one point is left
- **TEST** t2456 (`src/apps/kt-app/src/mask_tests.rs`)

#### Scenario: The preview click and the search arrows count the busy frame
- **WHEN** egui's double-click window is widened by a 400 ms busy frame, or three ↓ are read in one frame of the search window
- **THEN** the single click has not played 350 ms after it, and the selection moves three rows
- **PATH** a busy machine → a double click on the preview → the preview detaches, playback does not toggle; Ctrl+F → ↓↓↓ quickly → the third find is selected
- **TEST** t2457 (`src/apps/kt-app/src/layout_tests.rs`)
