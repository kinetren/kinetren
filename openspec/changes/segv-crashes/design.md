# Design: segv-crashes

## Context

The owner's crash folder `fails/26.11.08/` (read only):

- `new/kt-app-crash.txt` (build kt26.11.08-b614203c, 06:21:59): the version, the time, "the editor
  was ended by SIGSEGV (segmentation fault), 11", and a "log tail" that is the run's header line alone.
  `kt-app.log` holds warnings only since telemetry-journal, so the tail the supervisor read was empty
  of content, while the run's telemetry journal (what happened second by second) was not read at all.
- `new/load-report.txt` (06:21:42): the run's last records — the Effects window opened at 06:21:38,
  `SetFxBypass` at 06:21:40, four engine builds, the UI thread blocked 330 ms per frame, then a hot
  spell of three seconds over 100 % (06:21:40.5, 41.5, 42.5), which is exactly what makes the load
  watch rewrite the report (and, before this change, create a new wgpu instance for its header). The
  process died 17 s later; no record says by what.
- `new2/load-report.txt` (kt26.11.08-360b50e6, 06:53:33): another hot spell, a prefetch thrash, the
  UI thread blocked 3.6 s and 4.5 s; no crash file (taken at the next start).
- `old/kt-app.log` (kt26.11.07-61414dba): three missing sources, nothing about a crash.

None of them can say which thread died or where. That is H5, and it is why H4 has no diagnosis.

## D1. The mechanism and the class

The mechanism: the process dies by a native signal inside code we cannot unwind or catch (a driver,
the Vulkan loader, FFmpeg, ALSA, a C++ destructor at exit), and the program keeps no record of the
moment — the panic hook never runs for a signal, the supervisor sees only the number, and the journal
it read was the wrong one.

The walk — every native path that can fault, what was checked, what was found:

| Path | Checked | Verdict |
|---|---|---|
| wgpu instance creation (Vulkan loader, all ICDs) | gdb on `kt-cli export` without DISPLAY; every `Instance::new` site (`context.rs` ×3, `compute.rs`, `adapter_facts`) | **CAUSE (row 131)**: two to four `Instance::new` at once on `export_engines`' threads → SIGSEGV in `vkEnumerateInstanceExtensionProperties` beside `vkCreateInstance` (libvulkan 1.3.275). Closed: one instance per backend set, one opening at a time (D3) |
| The load watch's report header | `load_watch::header` → `adapter_facts()` | **CAUSE (same class)**: a whole instance created and dropped on the load-watch thread every 5 s of a hot spell, beside the preview and the offline devices — every ICD dlopened, every driver's config parsed (libexpat on the faulting neighbour in the gdb dump), then dlclosed. Closed: the list is taken once per process, before eframe |
| The offline engine and the compute host, first use | `offline_engine()` (fx thumbs, reel strips, posters), `compute::host()` (prefetch, `compute-open`) | **CANDIDATE, closed by D3**: each opened its own instance lazily on its own thread; the first Effects window and the first compute kernel could open at the same moment. Now they share the instance under the lock |
| eframe's own instance | `app::run` order | eframe creates it on the main thread at start, before the app's threads; the adapter list is now taken before it, so no instance of ours is created beside it at start |
| Process exit with a frame on llvmpipe (libLLVM destructors) | t985/t547/t1644 history, `park_offline_engine` | D4 |
| FFmpeg decoders/encoders (dlopen) | invariant 5 (one thread per decoder), `hw_trial` | no fault reproduced; a decoder fault now leaves a stack naming `libav*` and the calling worker. The libva first-touch guard (invariant 9) already moves the one known faulting driver into a throwaway process |
| cpal/ALSA callback | `audio_out.rs` | no fault reproduced; the callback thread (`cpal_alsa_out`) is named in the report's thread list |
| Threads outliving objects at exit (compose, prefetch, posters, thumbs) | ui-never-waits notes, `on_exit`, `Drop for KtApp` | D4 |
| Drop order of engines | `Player` drop joins its workers; export engines drop on the render thread after the last frame | no fault found |
| Stack overflow | — | ends without the report (D2, no alternate stack); the supervisor still names SIGSEGV |

## D2. The report is written at the signal and read in a healthy process

- **What the handler may do.** Only async-signal-safe work: system calls (`open`, `write`, `read`,
  `getdents64` for `/proc/self/task`, `process_vm_readv` for the stack scan, `prctl(PR_GET_NAME)`,
  `clone`/`execve`/`wait4`, `tgkill`), fixed static buffers (1 MiB for the mappings, never touched
  until a crash), and libgcc's `_Unwind_Backtrace`, primed once at `install` so its object
  registration does not happen inside the handler. No `std::fs`, no `format!`, no lock of ours: the
  dying thread may hold the allocator's or any other lock.
- **Unwinding through the signal frame.** libgcc recognises the kernel's `__restore_rt` frame and
  continues into the faulting code; the frames before the faulting pc are the handler's and are cut.
  A driver without unwind tables (a stripped `.so`, llvmpipe's JIT) stops the unwinder at once: then
  the words above the stack pointer that point into executable mappings are listed (at most 64, from
  128 KiB) — the callers, some stale, as breakpad's stack scan does.
- **Re-raise.** The handler restores the default action and sends the signal to its own thread
  (blocked while the handler runs, so it lands as the handler returns): the death, the exit status
  (139), the supervisor's verdict and a core dump stay what they were. A fault inside the handler
  dies at once; a second thread faulting meanwhile waits for the first to end the process.
- **Bounded.** `alarm(20)`: a hung unwinder (it may block on the loader's lock if the crash happened
  inside `dlopen`) or a hung symbolizer child ends the process by the ORIGINAL signal. The signal and
  thread lines are written before anything that can hang.
- **No `SA_ONSTACK`.** Rust gives its threads an alternate stack of a few KiB (SIGSTKSZ); the unwinder
  needs more, and a C library's thread has none. Running on the thread's own stack serves every fault
  but a stack overflow, which then ends without the report — the supervisor still names SIGSEGV. The
  rejected alternative (an alternate stack per thread) would need a hook in every thread start,
  including the drivers' threads, which we do not own.
- **Symbolizing.** A raw report is the exact record (addresses + mappings); names come later, in a
  process that is not dying: the supervisor right after the child's death (it is the same binary
  file), the next start when no supervisor took it, or — kt-cli has neither — a child started from the
  handler by a raw `clone` with fork semantics and `execve(kt-cli crash-report <raw>)` (libc's `fork`
  runs atfork handlers and takes locks). Names come from the line tables (`addr2line` over `gimli`;
  the dev profile keeps `debug = 1`, t1072), else the symbol tables, else a distribution's separate
  debug file by build id; a frame with none keeps `module+offset`.
- **Rejected:** a minidump with a ptrace-ing supervisor (crashpad-style) — exact, but the processor
  stack is a dependency family we do not have offline, and the supervisor would need `PR_SET_PTRACER`
  and a remote unwinder; the in-process unwinder plus a stack scan already names the frames the
  owner's complaint needs. Symbolizing inside the handler — `addr2line` allocates.
- **The journal beside it.** The editor's report carries the crashed run's last 60 telemetry-journal
  lines, found by the child's pid on its RUN line (a second instance may write the same file), and
  the window shows them instead of `kt-app.log`'s tail (which, since telemetry-journal, holds warnings
  only). The journal's "the previous run crashed" record carries the summary line.

## D3. One wgpu instance per process, devices opened one at a time (row 131)

Reproduced: `env -u DISPLAY kt-cli export demo.kt out.mp4 libx264` → exit 139, 3 of 3. gdb: thread A
in `vkEnumerateInstanceExtensionProperties` called from `wgpu_hal::vulkan::Instance::desired_extensions`
← `kt_gpu::export_engine` on a scoped thread of `export_engines`, jumped to address 0; thread B in
`vkCreateInstance` → `libVkLayer_MESA_device_select` → `libvulkan_nouveau` → libexpat, from the same
function on the next thread. On a software adapter `export_engines` opens 2–4 devices at once, each
with its own `wgpu::Instance`; the loader's instance-level entry points raced. DISPLAY only moved the
timing (EGL's X11 platform init inside the same `Instance::new` staggered the threads).

Fix: `context::instance(&opening, backends)` keeps one leaked `wgpu::Instance` per backend set;
`context::opening()` is a process-wide lock held across instance creation, adapter request and device
request (rare: a handful per process). The headless contexts, the D3D12 probe, the compute host and
`adapter_facts` use them; `adapter_facts` is computed once. The instances are never dropped: an
instance owns the loader's ICD handles, and destroying it at exit is one more native call racing the
threads still on its devices. The devices themselves are opened one at a time now; on llvmpipe that
adds the opening of 1–3 devices (≈0.1 s each) to an export's start, against a crash every run.

t2086 (kt-cli, no DISPLAY, `KT_VIDEO_ENGINES=4`, three runs): red on the old kt-gpu (exit 139 at the
first run), green now. t2087: four threads × three rounds open devices and the list is asked twice —
one instance.

## D4. t985 under nextest (row 132)

(see the measurement below)
