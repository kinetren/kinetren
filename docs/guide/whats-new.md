# What's new

The program shows the section of its own version in the welcome window. One section per
month, newest first.

## kt26.11

- **A Datamosh works out its history in batches on the graphics device, and a jump back costs a few frames, not the whole history.** The effect used to compute its history one frame at a time, waiting for the graphics device twice per frame — on a graphics card most of that time was the wait, not the work, and the preview crept along with little processor load. Now up to 32 frames go to the device at once and only the pictures you will see come back; a frame of the history you jumped away from is found again from a nearby checkpoint (at most 16 frames to recompute instead of hundreds). While playback shows an older picture of the effect because its history is still being computed, the preview says "Catching up…" in its corner, and the V-fps counter no longer counts those repeated pictures as new frames. The picture is exactly the same as before.
- **A loop is kept whole as far as the computer's memory allows.** The finished pictures of a loop you have played were kept in a fixed 512 MB — about ten seconds at preview size and one second at full HD — so a longer loop, like a heavy Datamosh region, went on being worked out lap after lap however much memory was free, and a loop just a little too long reused nothing at all. Now the program measures the memory it may take while it runs (a limit set for it included) and keeps every frame of the loop within half of it; after a few laps the loop plays with nothing left to compute. A loop too long for the memory keeps part of itself instead of none, a render takes that memory back while it runs and holds no pictures it has already used, and the Diagnostics window shows where the memory went.
- **A heavy effect working in the background no longer holds up the picture you are watching.** Rendering, effect previews, posters and the look-ahead of a Datamosh used to send the graphics card long batches of work, and the frame you were looking at waited behind them — on a card that runs the window and the effects together, up to most of a second. Now the card is given the frame on screen first and the rest in short pieces between frames: with a render and a Datamosh working beside the preview, its frames wait at most a few tens of milliseconds (a computer without a graphics card: the worst wait 764 ms → 73 ms, three times as many frames shown). A Datamosh you have moved away from stops working out its history at once, and a paused render no longer wakes up to check whether you resumed.
- **A Datamosh costs the processor a fraction of what it did, and a loop over it stops recomputing it.** Every frame of a Datamosh used to work its whole schedule out again — and the program asked that dozens of times a second while looking ahead — and converted every picture's colours on the processor; now the schedule is worked out once per change and the conversion runs on the graphics device, so the effect's own share of a frame fell to about a third (on a 720-line Datamosh: 10–17 → 4.4 ms of processor a frame) and it moves a third of the data it did. A loop over a long Datamosh no longer forgets its history on every lap: the program keeps as much of it as the loop needs and the computer's memory allows, so from the second lap the effect itself costs nothing. The picture and the sound are exactly the same as before.
- **Saving and bringing in files no longer freeze the window, and background work steps aside while you play.** **Ctrl+S** used to stand on the disk for seconds on a busy computer, and dropping a file onto the timeline, placing one from the bin or undoing its arrival opened files with the window waiting; now they happen in the background and the window answers at once. While you play, filmstrip thumbnails, effect previews, composition strips, project posters, loudness analysis and waveform building wait, so playback has the computer to itself, and they finish as soon as you stop. A wide filmstrip or effects gallery no longer redraws its own pictures over and over, and a program standing still no longer wakes up a few times a second to check on things.
- **Colour effects in a row cost about as much as one.** Color Grade, Posterize, Duotone and Vignette placed one after another on an event or a track used to pass the whole picture through the graphics device once each; now they run together in one pass, so a chain of four or six of them takes about a quarter or a sixth of the device's time it did (six at full HD on a computer without a graphics card: 9.3 ms → 2.2 ms a frame). The picture is exactly the same as before.
- **Quick keys and double clicks are no longer lost on a busy computer.** When the program drew slowly (a build running beside it, a computer without a graphics card), several keys pressed quickly could count as one: → → → ← moved the cursor nowhere, two **Ctrl+Z** undid one edit, two quick taps of **Space** paused instead of stopping at the start, a short tap could do nothing at all, and a double click could open nothing. Now every key runs as pressed, a tap stays a tap, and the double press and the double click allow for how long the program takes to draw a frame.
- **The program rests again right after an edit.** After a change, while it checked which played frames it can still show without work, the window redrew itself as fast as it could — on a busy computer for seconds, taking several processor cores while nothing moved. It now redraws only when the check has something to show.
- **Playback no longer stutters at its very start on a busy computer.** The first moment of sound after Play or a jump could break up while another program loaded the processor; the start now waits until its sound is ready, as it already did for the first frame.

- **A loop you have played no longer shows "Loading" again.** A short loop under a heavy chain with Datamosh (Camera ISP → Datamosh → VHS Tape) could show "Loading" lap after lap, and pausing on one of its frames waited seconds for the effect's history again, however many times you had played it. Now a frame you have seen is shown straight from what was kept, without decoding or working the effect out again: the loop settles within a few laps, pausing inside it shows the picture at once, and the same holds at a slow rate and when you scrub back over it.
- **The program now keeps a trace a developer can ask questions of.** Beside the journal, the data folder holds `trace/`: a compact record of what every part of the program did and why — which frame waited for what, what the cache kept or dropped, what the effects computed, what the machine and its graphics card are. Send the folder with a report and the question "why was it Loading here" is answered from your own run, with no test build and no repeating it on another machine. It costs nothing while the program stands still and is never sent anywhere.
- **Dragging a slider on a big project no longer stalls the program.** Every change, and every step of a slider drag in an effects window, rebuilt the whole sound and video engine four or five times over, partly while the window waited: on a project of two thousand events each step held the interface for about a tenth of a second, and a drag kept two and a half processor cores busy. Now each change is prepared once, in the background, and only the event you touched is worked out again; the window answers at once, the sound plays on, and changing the loop region costs nothing at all.
- **The first frame of an effect no longer stalls.** Opening a project, adding an effect or seeking to a Datamosh or a generator used to freeze the picture for a moment — up to several seconds for Camera ISP or VHS Tape on a computer without a graphics card — while the effect was prepared for the graphics device. The program now prepares the effects a project uses in the background as soon as it opens or an effect is added, the window no longer stands still while the video engine starts, and on a graphics card that can keep them the prepared effects are remembered for the next start.
- **Modulation effects no longer get heavier with a detailed frequency curve.** Tremolo, Auto-Pan, Vibrato, Chorus, Flanger and Phaser re-read their whole frequency curve for every sample, so a formula on the frequency (which becomes a point per frame) made a minute-long event cost ten times the processor; now the cost does not depend on the curve. Formulas on curves are also worked out much faster everywhere, and a change on a project with shaped speed curves recomputes only the curves it touched. Everything sounds and looks exactly as before.
- **An edit on a project with shaped speed curves is fast again.** A speed curve with a smooth or formula segment made every change rebuild the sound engine in about half a second on a big project (the work grew with the square of the curve's length); now that rebuild takes a few milliseconds. Many transitions, a composition placed many times, a morpher layer over many events and chains of linked curves no longer make edits or opening slower out of proportion either.
- **A crash now says where it happened.** When the program is ended by a fault in a graphics driver, a decoder or its own code, the crash window names the part of the program that faulted and what it was doing, lists what you and the program did in the last moments, and the report in the log folder holds the whole trail with file and line, ready to send. A command-line render that crashes prints the same report.
- **Rendering from the command line works with no screen attached.** `kt-cli export` on a machine with no display (a server, a terminal session) crashed at once; it renders now. The editor also no longer reloads every graphics driver each time the load report is written.
- **A heavy effect no longer freezes the program.** When an effect takes seconds to draw a frame (a Datamosh with an extreme setting, say), the picture comes late, but the windows, the timeline, playback and **Ctrl+Z** answer at once: you can always undo the effect that made it slow. If a moment of the interface still stands, the journal now says which part of the window it stood in.
- **A Datamosh no longer stalls the picture.** After you added or changed a Datamosh, the program could ask for the same frames over and over, throw each one away before using it, and leave a paused frame or a short loop standing still for as long as you watched, with the interface freezing for seconds at a time. Now its frames are fetched in the order the effect uses them, the effect is worked out in the background, and the interface never waits for it: a loop over a Datamosh plays, and on a computer too slow for the effect in real time you see it run late rather than not at all.
- **Camera ISP and Datamosh ask less of the graphics card.** The camera simulation no longer recomputes the same sensor row and column for every pixel around it, and Datamosh's encoder spreads its work over the whole card instead of a few of its lanes and sends a quarter of the data. They look exactly as before.
- **The program now writes down what the graphics card spent its time on.** Every busy second, the telemetry journal (and `load-report.txt`) gets a line with the card's name, the frame size, how many milliseconds a frame took on the card and which effect pass took them — VHS Tape's head read, Camera ISP's demosaic, the composite. Diagnostics shows the same line while it plays. Nothing to switch on and nothing to measure by hand: a normal run is enough.
- **A range you have played plays again for free.** The picture after the effects and the sound after the effects are kept as you first saw and heard them, like a prerender: a loop over heavy effects (VHS, Camera ISP, a slowed composition) runs at the project's frame rate from its third lap, with no decoding and no effect running. Changing a parameter recomputes only the part it touches. The strips at the top of the timeline and under each event are now bright where a range plays without work and keep their darker colour where only its sources are ready.
- **Editing during playback no longer cuts the sound.** On a large project every change used to stop the sound for a moment.
- **A composition placed many times no longer eats the memory.** Its inner effects are prepared only for the places that are playing.
- **One journal of what happened and what you did, with the time.** The program now keeps `telemetry-journal.log` in its data folder: every busy second, each playback with its frame rate and dropouts, the warnings, and your own actions (an edit by name, a window opened, play and pause, a save) — each written the moment it happens, with the clock time, and kept after a crash. Press **Ctrl+Alt+M** or **Mark the moment** in Diagnostics (with a note if you like) when something goes wrong, and the line says “here”. Diagnostics shows the journal at the bottom whatever the window's height, and `load-report.txt` is now its last quarter of an hour with real times instead of “seconds ago”. Nothing in it is ever sent anywhere.
- **Sound effects take less of the processor.** VHS Tape's sound costs less than half of what it did, and Gate, Limiter, Distortion and Bitcrusher a tenth: they no longer redo the same calculation on every sample while their knobs stand still, and on a processor with AVX2 every sound effect runs on its wider instructions. They sound exactly as before, to the last bit.
- **An open effects window no longer works while you only move the mouse.** A number sitting exactly at the end of its range (VHS Tape's dark clip at its floor, Slowed+Reverb or Camera ISP at a ceiling) was taken as an edit on every move, and each one rebuilt the whole sound and picture engine: on a big project two processor cores ran flat out. Now only what you change is an edit, and the load report says when a busy thread did work it does not count instead of calling it rest.
- **The mouse over a big project costs a fraction of what it did.** Moving it over the preview, along the timeline or over an open effects window no longer makes the program redo work for every event of the project: a frame now costs what is on screen, at any scroll and zoom, and playback leaves the picture more of the processor. Diagnostics › Performance shows what one interface frame costs and where ("UI frame … ms").
- **A pause is a pause.** A program standing still does no work now: with several releases of a composition on the timeline it kept rendering their previews in a circle, and every move of the mouse started another one. Moving the mouse no longer costs anything that depends on what the timeline shows, and each release keeps its preview between openings.
- **Effects run on the card your monitor is plugged into.** On a desktop they could end up on the processor's built-in graphics.
- **The load report.** If the program holds the processor for more than a few seconds it writes `load-report.txt` in its data folder by itself: which threads, which work, which graphics devices. Diagnostics shows the same live, with a button that opens the folder. The file stays on your computer.
- **Work the way your hands already know.** At the first start, or in File › Preferences…, choose a Layout & Controls style: ours, like Vegas, or like Premiere Pro, DaVinci Resolve, Final Cut Pro or After Effects. The keys, the wheel and the panels change at once, and each tile says what that program has and we do not. **Customize** changes any key, and the **settings code** under the tiles brings it all back after a reinstall.
- **Renders with no graphics card are faster**, most of all under the VHS simulation and on events
  with a camera, and the preview gains the same in those places.
- **A render no longer stalls on a file cut short.** A file whose header promises more than it holds
  is now read to its real end once, and the render goes on with the last picture.
- **Every picture is now made by one engine, on the graphics card.** The separate processor path is
  gone, so the preview and the render can no longer disagree, and effects no longer exist twice. With
  no graphics card the program runs the same engine through Vulkan on the processor, and a render
  there uses several devices at once, as many as the memory allows. A computer with no Vulkan, DX12 or Metal driver has no picture
  and no render, and the program says so: on Linux install `mesa-vulkan-drivers`, on Windows a
  graphics driver.
- **Shapes, gradients and the countdown generate up to fifty times faster.** Rectangles, ellipses, lines,
  gradients, corners, noise and the leader now draw as shaders.
- **Slowed footage in blending mode no longer shows a frame of the wrong weight.**
- **A render takes much less memory.** A dense 1080p edit went from 17 GB to under 9 and got faster, and
  on a machine short of memory the render gives up speed instead of being killed: the same edit finished
  under a limit of 3 GB.
- **A render comes out the same every time.** Rendering a busy edit could quietly drop a layer from a
  few frames, different ones on each run, leaving wrong pictures in the file. Now every frame keeps
  all its layers, and the speed of the render is unchanged. If a source really cannot give a picture,
  the render's log says which file and at what time, and opens by itself so you see it.
- **A picture in the project no longer holds up the waveforms.** One still kept a processor core busy for
  hours and grew the memory, and the waveforms of the files after it never came.
- **Ctrl+S during a freeze no longer cuts the event.** If the interface stalled for a moment and Ctrl was
  already released when it woke, the program saw a plain S and split the event under the playhead.
- **A failed render explains itself better.** The message no longer starts with a technical "io:", and
  the window says no encoder opened instead of still claiming to choose one.
- **The timeline stops fighting the hand.** A bottom corner of an event always changes its length.
  On a low row or a short event a press moves it instead of touching the volume, and the level is in
  the right-click menu. The sound of an event goes down to mute, and the event now says its level
  right after the name: `−12 dB`, `40 %`, `mute`.
- **The space bar does more.** Held from a pause it plays only while held and stops where the picture
  got to. Ctrl+Space plays backward. The speed builds up smoothly and comes back faster. A short press and
  two quick presses work as before.
- **Step one frame** with ← and →, or Ctrl+click on Back and Forward.
- **Freeze frame.** Right-click an event and choose "Insert frame hold (freeze frame)": the picture stands
  for 2 seconds at the playhead and goes on from the same frame. The event keeps its length.
- **Tracks can be dragged again by their header**, which had stopped working long ago. The header also
  shows a hand now, and a click on its free part is the strongest jump to the left.
- **Jumps and scrolling.** A click at the timeline's edge jumps even over an event, and no longer opens it.
  The side scrollbar can be caught without resizing the window. Zooming the rows keeps the track under the
  mouse. The end of a range marker can be moved. Empty morpher and camera rows carry no captions.
  When playback freezes, the bell says where.
- **Datamosh encodes on the graphics card.** The effect's codec now runs as a shader, and even with no
  card, through Vulkan on the processor, a frame takes about a third less time. Existing moshes may
  look slightly different, because the search picks some other vectors.
- **Hardware encoding on Linux no longer risks the render.** Choosing `h264_vaapi` used to end the
  program. Now the render tries a hardware encoder on the side first: if the graphics driver fails
  or crashes there, a render left on Auto goes on with the next encoder, and one you chose by hand
  stops with a message saying what went wrong, while the program and your project stay open. On
  Intel and AMD graphics the render can now use the card, though we have not yet been able to test
  that on real hardware.
- **Camera ISP records the sound too.** On an event with linked audio, the sound goes through the
  device's microphone: the phone's port and noise suppressor, a webcam's 16 kHz, a security
  camera's G.711 and wind, with an automatic gain control that lifts the room between words.
  The looks set it, and the Microphone block in the panel lets you change every step.
  A project that uses a look with a device microphone, or changes a device's sound settings, opens
  only in this version and newer.
- **Datamosh breaks the sound too.** On an event with linked audio, the sound now goes through a
  real audio codec in the same stream: where the picture loses a keyframe the sound loses a packet
  at the same moment and smears until the next clean keyframe — a Reset key heals both.
- **VHS Tape now plays the sound too.** Put it on an event with linked audio and the audio gets
  the same deck: Hi-Fi tracks that crackle exactly where a clogged head or a dropout breaks the
  picture, or the linear track — dull at LP, with tape hiss and the capstan's wobble. Its Sound
  section is in the deck panel; the checkbox in the audio's chain switches it off.
- **The preview keeps up at full quality.** A shot over its own blurred backdrop used to make the
  program decode the same video about ten times over; now each picture is decoded once, the sound
  no longer drops out there, and playback at the project's full size stays smooth where it used to
  stutter. The interface stays responsive during playback, even on a computer without a graphics
  card, where frames are now prepared ahead on several processor cores, and a cut into a large 4K
  file no longer arrives late. The first play after opening a project starts with its sound
  instead of a moment of silence.
- **A busy disk no longer makes the program forget its previews.** On a spinning disk under heavy
  writing, opening a project could rebuild the previews of its compositions, and the waveforms and
  loudness of its media, as if seen for the first time. The program now keeps using what it stored.
- **The final render is several times faster, and it no longer crashes on some footage.** It
  used one core for everything in turn; now it reads, draws, converts and encodes at the same
  time on all of them, and the sound renders alongside the picture instead of after it. A
  project that jumps around in long 4K files no longer reads minutes of video it does not show,
  and a picture-in-picture of the same clip no longer makes it re-read the clip for every frame.
  The render of a project with a square 1080×1080 video used to crash within seconds. On a
  machine without a graphics card the render now draws on the processor's cores, which is the
  faster way there.

- **Camera ISP no longer smears the edges of the frame.** "Phone at night" and the looks with a
  distorting lens filled a band along the edges — about 30 px on a 1080p frame — with streaks of
  the edge pixels. The simulated lens is now framed on what your footage recorded, and the
  camera's processor crops its distortion correction to what its sensor saw, as a phone does:
  the edges are picture, and a corrected lens frames slightly tighter.
- **Pitch: choose what happens to the echo before it.** When a reverb or an echo stands before a
  pitch (Slowed+Reverb under a morpher's pitch, for example), the pitch effect now has the line
  "Echo of the effects before the pitch": **cut at the event's end** (as before — the joins stay
  seamless) or **rings on after the event** (the echo goes on past the event's end, pitched, as
  without the pitch). Projects that use "rings on" open only in this version and newer.
- **A loop over an echo before a pitch no longer dips at every lap.** When the loop region started
  at an event with an echo or reverb before its pitch, each pass began with a short hole (with
  "rings on") or a sudden drop with a bit of foreign sound (with "cut"). Every pass now starts as
  playing from that point does, and the echo of the pass before still rings on over it.
- **Playing layered 4K footage keeps its frames ready.** Where several layers play at once —
  a shot over its blurred backdrop, a picture-in-picture, a composition — the preview used to
  ask for more frames ahead than its memory holds and threw away the ones it needed next, so
  the picture stalled and Diagnostics filled with "dropped before the first read". The
  look-ahead now fits the memory, and frames the playhead has already passed are no longer
  decoded.

- **Ctrl+F shows what it finds again.** The search window kept the height of its empty field, so
  the media, events and markers it found were hidden under its bottom edge until you dragged the
  window larger.

- **Opening a project with a reel no longer asks to save changes you did not make.** Once the
  reel's preview had drawn on the timeline, opening another project or quitting asked "save your
  changes?" about a project nobody had touched, and quitting left a recovery file behind.

- **Rendering a region no longer closes the program at 100 %.** A render of a region that does not
  start exactly on a frame — any region drawn by hand, even 0.2 s — used to reach 100 % and take the
  whole program down, sometimes with the file half-written. It now finishes, shows its summary and
  closes like any other.

- **A camera you can film your frame with: Camera ISP.** The new effect undoes the camera that
  made your footage back into a scene, then films it again with a simulated camera: a lens
  (defocus, distortion, chromatic aberration, vignetting, colour shading), a colour mosaic (RGGB
  and its phases, Quad Bayer, X-Trans, RGBW, RGB-IR with infrared pixels), a sensor that counts
  electrons with their real noise, blooming and banding, and the camera's processor with a block
  for every one of those defects — black level, row/column correction, defect pixels, green
  balance, raw denoise, lens shading, white balance, demosaic, distortion and colour-fringe
  correction, colour matrix, tone curve, denoise, sharpening and YUV with 4:2:0. Every block can be
  switched off or set wrong, "View" shows the picture after any stage, and the looks follow from
  it: "Phone at night", "Cheap webcam", "Security cam, IR", "Cinema raw", "Wrong CFA phase", "No
  IR-cut filter". The noise is the same on every play and render.
- **A real start window.** The welcome window has three columns — Start (the tour, templates
  drawn in their frame's shape, the five lessons with your progress), Recent (your projects with
  their pictures, as many per row as fit) and What's new, now readable in its own column. Each
  column scrolls on its own, and the window can be made as large as the screen.
- **File → Preferences… holds the program's settings.** Language, Economy UI, whether the welcome
  window opens at start, the usage statistics, the daily update check and FFmpeg are in one
  window, each switch in one place; the View menu keeps only what changes the view.
- **The language you chose stays chosen.** Opening another project used to forget it, and the
  next start came up in the system language; Economy UI was forgotten the same way.

- **A real VHS: the tape is written and read back.** The new **VHS Tape** effect does not paint
  stripes on the picture — it records it as a VHS deck does (the brightness on an FM carrier, the
  colour on a low "colour-under" carrier, two heads on a spinning drum writing slanted tracks) and
  plays the tape back. Snow, colour smear, the noise bar, the tear at the bottom and wobbly lines
  come out of the causes: click a head on the drawn drum and make it dirty, clogged or chipped,
  turn the tracking, switch SP/LP, a TBC, four heads for a clean pause, or record on one deck and
  play on another. The old **VHS** look is still there.

- **Datamosh — a real one.** The new Datamosh effect runs its own little video codec over the
  clip — motion search, a prediction loop, quantised detail, keyframes — and lets you break it on
  purpose: drop the keyframes so the previous shot gets dragged by the new shot's motion, repeat one
  frame's motion until it blooms, freeze the reference, scale, turn or swap the motion vectors,
  borrow the motion of the clip on the track above or below, lose detail or whole blocks. The
  breakages take curves: switch the mosh on and off, or put a Reset key on the beat to start clean
  from that frame. Presets: Cut mosh, Bloom, Transplant, Melt. Any frame looks the same when you
  play up to it, jump to it or render it.

- **Masks.** Right-click the picture → Mask → Rectangle, Ellipse or Pen, and draw right over the
  preview: the pen makes a Bézier path (click — a corner, click and pull — a smooth point). Each mask
  has feather (smooth or linear), expansion, opacity, invert and a mode — add, subtract, intersect,
  difference — and several masks combine from the top down. A mask on the event's picture follows
  the event when you move, scale or rotate it; an effect can be limited to its own masks (under its
  parameters in the effects window) — a blur on a face, a colour only in a corner; a track can be
  masked from its window. The stopwatch on a mask animates its whole shape: drag a point at another
  moment and the shape is keyed there.
- **Composition previews no longer get rebuilt by chance when a project opens.** Opening a
  project sometimes found its own index file "busy" and drew the frames and waveform of every
  composition all over again; now the saved previews come back every time.
- **Diagnostics holds all of Performance, with three graphs.** The Performance box no longer
  sits under the media bin — the bin gets the whole column. The gauge button (or View →
  Diagnostics…) opens one window with every number the box had and three graphs side by side in
  time: playback, FFmpeg activity and the cache. Make the window larger and the graphs grow.
- **Sound generators: tone, noise, sweep, beeps and silence.** Edit → Create a generator → Audio
  puts a line-up tone, white/pink/brown noise, a sine sweep, a train of sync beeps or a stretch of
  silence on an audio track. Levels are in dBFS (−20 dBFS of tone and of noise read the same on
  the meters), a tone or a beep can go to one channel, and the sound is the same on every play,
  seek and render.
- **"Create a generator" left the File menu.** It is in **Edit** now, and in a track's right-click
  menu, which puts the generator exactly where you clicked — a video track offers the picture
  kinds, an audio track the sound kinds. The media bin keeps its item.
- **Every window can be made bigger, and what is inside grows with it.** Drag a window's corner:
  the effects chain, the curve graph, the lists, the Guide and the render log take the new room
  instead of leaving an empty frame around a small box. Each window remembers the size you gave
  it — an effects window opens as large as the last one you enlarged.
- **The Guide reads like a manual.** Its tables are real tables, with a shaded header, lines
  between the cells and a key column as wide as its keys; every "I want to…" entry is its own
  card, the place to act written under it.
- **Adding an effect shows one big gallery.** The "+ effect" picker is as wide as its window and
  as tall as the screen allows — presets first, then the effects — with more thumbnails per row
  in a wider window.
- **No button is cut off at the end of the toolbar.** In a narrow window the buttons that do not
  fit (Render, Search, Diagnostics, Save…) move into a **⋯** menu at the end of the row.
- **Zigzag and the other segment shapes on one piece of a curve, or on a hundred.** Right-click
  a segment, or <kbd>Ctrl</kbd>+click several, or drag across empty space in the curve graph:
  under "Shape of all segments" a row appears with the same fields for just those segments.
  Values that differ show "—"; a typed number sets all of them, dragging the field shifts
  each by the same amount.
- **Windows: the welcome window names your project folders plainly** — `C:\Users\…`, not
  `\\?\C:\Users\…`.
- **A project sounds and looks the same on Windows and Linux.** Slowed+Reverb, the phaser and
  pitch shift gave slightly different bytes on Windows; every effect now uses the program's own
  math, so a render is identical on both — and on any processor.
- **Forcing an encoder this computer does not have** (VAAPI on Windows) now says so by name
  instead of an empty error.
- **If Kinetren crashes, a window says so at once** — the error, the version and the last lines
  of the log, with **Restore the project** and **Open the log folder**. Before, you learnt it only
  at the next start, if at all.
- **Restoring after a crash brings back your own footage.** The restored project no longer plays
  a lesson's test patterns in place of your sources, and keeps its own name.
- **Rendering a region that does not start on a frame** no longer closes the program.
- **The log keeps one file per run** (the four previous runs next to it), each starting with the
  version and the start time in your time zone.
- **The tour points where to act.** Every lesson step outlines its place — the menu item, the
  event, the marker, the button in a window — and **Show** scrolls to it or opens its menu
  instead of doing the step for you. Keys and buttons in the lesson text are drawn as keycaps.
- **The tour no longer lags or misleads.** "Play a part in a loop" counts the moment the loop is
  on, even with a window open; the transitions lesson teaches **Transition with previous** (an
  overlap stays a cut); after an effect or a transition the lesson says what changed and moves
  the playhead to where you can see it. "Next lesson" no longer asks to save the lesson's own
  copy, the lessons panel no longer hides the right end of the toolbar, and the render lesson
  tells silly jokes while it waits.
- **Click a marker on the ruler** to put the playhead on it.
- **Pitch no longer tears between short events.** Cut an event with pitch into many short
  pieces and they sound exactly like the uncut event; any pitched event starts and ends
  without a dip — also when the pitch comes after Slowed+Reverb or an echo, or from a morpher.
- **Changing an audio effect takes effect at once.** Put a filter where an equaliser stood
  and you hear the filter right away — before, the old effect kept playing with the new
  settings until you jumped in time.
- **The strength slider in the effects gallery** no longer mixes the settings of two
  different effects.
- **Every effect, transition and generator is now a plugin inside** — blur, glow, VHS, noise,
  the chroma key and the rest moved too. Your projects open and look exactly as before.

## kt26.10

- **Windows.** A portable Windows build; FFmpeg is found on the computer or downloaded with
  one click.
- **FFmpeg 9** works, as do 6, 7 and 8.
- **Under wine** the interface is dark everywhere and our own title row is back.
- **A quiet reminder about updates.** Once a day the program checks whether a newer release is
  out and says so in the bell — no pop-ups.

- **A welcome window at start.** New projects from templates, the recent projects with a
  frame of each, the tour and this page. One tile is highlighted: **Enter** takes it. Switch
  the window off with **Show at start**; **Help → Welcome window…** brings it back.
- **The tour.** Short lessons on a demo project made of generators. A step closes by itself
  when the project shows it done — see [Getting started](getting-started.md).
- **Help → Guide…** finds where a thing is done: type what you want, or open a page of this
  guide.
- **One window shell.** Every window closes by its cross and by **Esc**, the top one first,
  and no window grows by itself.
- **The render window** names the encoder before the first frame, counts the bytes and asks
  before replacing a file.
- **The FFmpeg window** tells what it downloads, where it goes, how fast, and why.
- **Effects are plugins inside.** The built-in effects moved into their own crates behind one
  interface; projects keep their bytes.
- **Faster event grids.** A grid computes one cell and copies it; a morpher layer at strength
  zero costs nothing.
