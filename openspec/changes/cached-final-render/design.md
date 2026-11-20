# Design: cached-final-render

## Context

- `render_cache::durable_hash` (render-cache-disk D1): `xxh3_64` over a documented byte layout — the plan's
  fields in `plan_hash` order without content serials, then `Leaves::digest` (each leaf's source address and
  `src_rev`, each file's `(media id, xxh3(path, size, mtime ns))`, the project's files for a stream stage) —
  plus the program build salt (with the FFmpeg version). The device is the store's folder, not the key.
  The plan's `out_w/out_h` and the quality are in it, so the same moment at the export's size has its own key.
- render-cache-codec D3 wrote that key per frame into each segment's manifest "so the cached final render
  (F-EXP-003) can match it later"; the preview's segments are lossy, at the preview's size: they are not
  the final render's index.
- video-plan: an export frame always runs the settled form of a point-wise run (t2516); the preview may
  show own passes during a trial (t2514) — within 1 LSB a boundary on a card, equal on a software adapter.
- export: the pipelined writer takes frames in order (`export_pipeline.rs` writer), the encoder cascade
  per codec (`kt_ffmpeg::encoders`), MKV and MP4 containers (render-settings), `t2805` same settings → same
  bytes (fixed encoder threads), invariant 11.
- F-EXP-003 (`docs/features/export/pipeline/smart-render-passthrough.md`): "smart render" is two
  mechanisms — copying the source's packets, and reusing already computed render files (Premiere's "Use
  Previews", FCP's render files). The note rejected a hybrid file because one MP4 header carries one
  SPS/PPS and our encoder is chosen from a cascade. X1/U5 is the second mechanism between OUR renders.

## Goals / Non-Goals

Goals: a second render of a project after a small fix costs only the GOPs the fix touches; the render
window shows what is cached; nothing reused can be stale; a render without the option is unchanged.

Non-goals: copying the SOURCE's packets (F-EXP-003's first mechanism, its own verdict); reuse across
program builds or devices; reuse of the preview's tier (Q1).

## D1. The unit of reuse is a closed GOP of an earlier export

A packet cannot be copied alone: inter frames reference their GOP. A closed GOP (it starts with an IDR
and no frame references outside it) can. The cached render works in GOPs:

- an earlier file's GOP `g` is reusable for the new timeline's frames `[a, a+len)` when every frame's
  durable key equals the key the index lists for the same position in `g`, in order, and the file's
  fingerprint (D3) equals the new render's;
- a GOP of the new timeline that is not wholly matched is composed and encoded as today.

Frames, not NAL units: X1 says "кадры/nalu/etc"; the frame key is the unit of identity, the GOP the unit
of copy. A partial match inside a GOP is rendered (encoding the matching frames again costs what
encoding always costs; decoding the old GOP to splice would put a decode generation into the final file).

## D2. Keyframes where reuse can happen

A cached-render export places an IDR (closed GOP) at every frame whose durable key run differs from the
previous frame's "edit identity": a cut, the start or end of an effect's key range, a transition's edges —
in practice where `Leaves::digest` or the plan's layer set changes — and otherwise at the GOP rule's
spacing counted from that point. This is a function of the timeline (invariant 11), so two renders of
nearly the same timeline put GOPs at the same places, and a fix inside one shot invalidates only that
shot's GOPs. With the option off the encoder's own GOP rule stays as today (no file changes).

Cost: a few more IDRs per minute (one per cut) — visible as a slightly larger file at the same quality;
the window says so.

## D3. The fingerprint: what makes a copied GOP fit the stream

One hash, written in the index, of: container, codec, encoder name and its library version
(`avcodec_version()`, x264/x265 build where the encoder reports it), profile, level, pixel format,
size, frame rate, rate control mode, bitrate or quality value, GOP rule, B-frame count, colour tags,
encoder threads, the device identity (adapter + driver; frames composed on another card differ within
tolerance) and the program build salt (a kernel changed in a new build would otherwise come back from
an old file). Equal fingerprints → the copied GOP decodes with the stream's parameters; different → that
file is not a candidate at all, and the window says why ("other bitrate", "other encoder", "other
build").

Rejected: a lenient match ("same codec and size") — rate control and level mismatches produce files
that some decoders refuse; the owner's X1 itself says "управление битрейтом/параметрами и разрешение
видео должно быть одинаковым на всех участках".

## D4. Parameter sets in-band, timestamps re-stamped

- H.264/HEVC: a cached render writes the parameter sets before every IDR (x264 `repeat-headers`, NVENC/
  VAAPI's in-band headers; the `dump_extra` bitstream filter where an encoder only gives extradata). In MP4
  the sample entry is `avc3`/`hev1` (parameter sets in the stream may change); MKV allows it always.
  X1: "не мешает pps/sps/vps/etc перед сегментами класть".
- Timestamps: a copied GOP's packets keep their decode order and relative pts/dts; they are shifted to
  the new position (`av_packet_rescale_ts` + offset), so the B-frame reorder inside the GOP is preserved
  ("соблюсти pts nalu"). The muxer gains a packet input beside `write_video_frame`.
- The encoder's own state around a copied GOP: the encoder is flushed at the GOP before the copy and
  reopened (or forced IDR with a fresh rate-control segment) after it; the per-GOP restart is what makes
  a GOP closed on both sides. The cost is measured (x264's lookahead restart is ~the lookahead length).
- Audio is always rendered and encoded (cheap; its encoder priming and frame grid make packet copies
  fragile); invariant 1 unchanged.

## D5. The index and where renders are found

Per Q3. Recommended form: `<output>.kt-render` beside the file — text, as the prerender's manifests:
the fingerprint, then one line per GOP (`gop <first frame> <frames> <byte offset> <packets>`), then one
key per frame (16 hex digits, the documented layout). The project keeps nothing; the data folder keeps a
list of the project's earlier renders (path, size, mtime, xxh3 of the index), checked before use: a
moved or edited output is not a candidate until found again (the render window offers "Look in folder…").
A file whose index or fingerprint does not verify is skipped with its reason.

SEI (Q3 b): one user-data-unregistered SEI per GOP with a UUID of ours, carrying the GOP's keys; the
render window's tool strips it by re-muxing with the `filter_units` bitstream filter.

## D6. The render window

A "Cached render" step (X1: "отдельный шаг настройки рендера"): the switch, the candidate files with
their verdicts, and a strip over the timeline range showing which GOPs are found (the bars' colour
language of the timeline, invariant 4's honesty: a GOP shown cached IS copied). The estimate counts
copied GOPs at the copy's speed. The finished render's summary says "copied N of M seconds from F
files". UI rules (`src/apps/kt-app/CLAUDE.md`) and a `- **PATH**` line apply to its scenarios when built.

## D7. Found-defects 773: the preview's tier stays out of the export

The tier holds the preview's size and quality, lossy, possibly in a run form the export does not use. A
final render made of those frames would be worse than its own and not byte-stable (the preview's kept
set changes run to run). The cached render reuses only exports' own GOPs — composed in the settled form
at the export's size by the export's path — so reuse never puts a preview-only frame into a render.
Q1 b (a draft render) is the only way the tier would reach a file; recommended against.

## D8. Invariant 11 for a cached render

A copied GOP is the earlier render's bytes; a fresh render at the same place would encode differently
(rate control state). So a cached render's bytes are a function of (project, settings, the set of
verified indexed files). Deterministic given those inputs: candidate files are ordered by their index
hash, the first match wins, never by time or disk order. A render with the option off is unchanged and
keeps t2805/t11. Q2 is the owner's amendment of invariant 11 for this mode.

## D9. The class walk: everything that keys or writes a rendered frame

| Place | Relation |
|---|---|
| `render_cache::durable_hash`, `Leaves` | the shared key; its layout becomes a contract (video-plan delta); a new plan field hashed by `plan_hash` must reach it (render-cache-disk's rule) |
| Prerender manifests `<id>.idx` | same key form; not a reuse source for exports (D7) |
| Kept composites / render-ahead / present memo | in-process, serial-keyed; not involved |
| Export pipelined and serial | the writer gains the copy path; compose/convert skip copied frames |
| Encoder cascade | the fingerprint names the encoder actually opened; a different machine's cascade yields another fingerprint |
| `export-host-copy` (YUV on device) | orthogonal; the fingerprint includes the build, so a conversion change re-keys |
| `device-decode-sources` | orthogonal (sources' frames are byte-equal whichever decodes) |
| Stream stages (Datamosh history) | their key carries every project file (render-cache-disk D1); a GOP inside a stream stage is reusable only with the same history inputs |
| Audio | always rendered |
| OTIO door | nothing in the model; no row in "What OTIO export loses" |
| `.kt` format | untouched |
| Windows | same design; MF encoders' in-band headers to be checked per encoder (the cascade's trial) |

## D10. Stages (when the owner says go)

1. The index: fingerprint, per-frame keys at the export's quality, GOP table, written beside a normal
   export (no reuse yet). t3322–t3324.
2. Keyframes at edit points and in-band parameter sets under the option. t3325, t3326.
3. The copy path in the writer, candidate lookup, re-stamping, encoder restart around copies. t3327–t3331.
4. The render window step and the summary; kt-cli `export --cached`. t3332–t3334.

## Risks

- Encoders that cannot restart cheaply or cannot emit in-band headers: the fingerprint then marks the
  file "not reusable" (the cascade's trial learns it once per encoder).
- Players and NLEs that dislike `avc3`/`hev1` in MP4: MKV is the default container of cached renders;
  MP4 only when the person asks, with the window's note.
- A key collision (64 bits per frame): a GOP needs all its frames to match in order; the probability
  is negligible against a disk error, and the build salt + fingerprint narrow it further.
