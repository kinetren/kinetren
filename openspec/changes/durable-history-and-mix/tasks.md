# Tasks: durable-history-and-mix

> IMPLEMENTATION-READY (2026-11-22). The owner decided (kkt-26.11.20 A14): build now, the history with LZ4,
> both tiers in the project's cache folder. Starts after `project-cache-folder` sections 1 and 3 (the folder,
> the header, `ask_room`, the key without the build) and after `stream-picture-stays` (STREAM). Tests:
> t3336–t3355, t4105. Code references checked against orch `873934f7` (design D10). Read first: design D2,
> D3, D5, D9, D10; `src/libs/kt-engine/CLAUDE.md`; `openspec show video-fx --type spec` and `audio-render`.
> Build-queue areas: STREAM, CACHE, AUDIO (`mix.rs`), PLAYER. Commit per section; target-prune after each.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D10, class walk D7), deltas `video-fx`, `audio-render`, `telemetry`
- [x] 0.2 the owner's answers (Q1 b, Q2 b); design D9 answers his question; rows 770, 771 point here

## 1. The restart line (telemetry)

- [ ] 1.1 a restart recorder over the first minute after open: history positions walked (`walked()`
      `stream.rs:124`, `positions_walked()` `:141`) with device time, covered bursts rendered again;
      `kt-cli trace summary` line (`trace_query.rs:466`); t3336
- [ ] 1.2 record kinds for stream entries and mix runs in `kt_time::trace::render_cache` (`trace.rs:631`),
      with raw and stored bytes; `kt-cli trace cache` (`trace_query.rs:130`) prints them; t3337

## 2. The history tier

- [ ] 2.1 kt-plugin-api: `EntryCodec { version, encode(&dyn Any) -> Option<Vec<u8>>, decode(&[u8]) -> Option<Arc<dyn Any + Send + Sync>> }`
      and `StreamKernel::entries()` (default `None`); the memo's backing hook: `put_entry` (`stream.rs:440`)
      marks pinned and broken entries unwritten; `holds` (`:596`) and `get` (`:403`) ask the backing on a
      memory miss, through the thread-local scope of 2.3; a kernel without a codec is never stored; t3342
- [ ] 2.2 Datamosh's codec (`fx-video-codec`): `Planes` exactly (w, h, y, cb, cr); `Clean` is not stored;
      t3339 in `walk.rs`
- [ ] 2.3 the stage's scope (`video_stream.rs` `run` `:884`, `key_only` `:863`, like `with_leaves`): the
      store's salt, the plugin's id, version and WGSL hash, the codec version, `FileIds.all`
      (`RenderCache::files`, `render_cache.rs:907`); t3340
- [ ] 2.4 kt-engine's history writer: LZ4 (`lz4_flex` into the workspace and kt-engine `Cargo.toml`), the
      `history/` tenant through `project-cache-folder`'s header and `ask_room`, writes only at rest (the
      prerender's quiet rule, `render_cache.rs` `quiet_left` `:925`, gates `:1096`), the ratio recorded per
      write; t3338, t3341, t3343, t3344
- [ ] 2.5 a device-held checkpoint (`HeldPic`, `walk.rs:882` with `hold_ahead`) is read back into `Planes` by
      a compute-host job at class `background`, only at rest, then written; t4105
- [ ] 2.6 `cargo test -p kt-plugin-api -p fx-video-codec -p kt-engine`; clippy; the idle check on the heavy
      showcase (after the writes, nothing at rest); commit "durable-history-and-mix 2: the Datamosh history
      survives a restart"

## 3. The mix tier

- [ ] 3.1 `Unit.data` (`mix.rs:56`) becomes `Arc<[f32]>`; `MixCoverage` (`:499`) carries a snapshot of the
      unwritten units; the player's thread writes runs (raw f32, a manifest per run) to `mix/v<FORMAT>/` at
      rest, salted by the store's salt and the audio file identities of the widened range (`audio.rs:1012`)
- [ ] 3.2 the loader on `prefetch_loop` (`player.rs:3400`, beside `mix.budget.ask` `:3522`) sends restored
      runs over a second `rtrb` ring; `audio_render_loop` (`:2442`) drains it at the top of a pass into
      `MixStore::write` (`mix.rs:303`) as first recordings; the render thread never opens a file;
      t3346–t3350
- [ ] 3.3 the bars' scan at rest adds restorable bursts; the export never reads the tier; t3351, t3352
- [ ] 3.4 `cargo test -p kt-engine`; clippy; the idle check; kt-engine `CLAUDE.md` ("the render thread never
      reads a file; a new kept result names its durable salt"); What's new line ("a restart no longer
      recomputes the Datamosh history and the covered sound"); commit; the report's "Найдено, не исправлено"
