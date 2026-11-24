# Proposal: durable-moment-key

> DESIGN (spec session, 2026-11-24, on orch `887c2972`). IMPLEMENTATION-READY; no owner question. Size M.
> Tests t4108–t4116. Found-defects 1061–1063 (closed here), 965 (its remaining half).

## Why

- `kkt-26.11.22` D2: "у тебя что, после каждой цепочки есть отдельный кэш?!?!? а не дохера ли мы данных сейчас в таком
  случае храним??" The audit's largest row is the decoded frames: 7.9 GB of 10.7 GB resident on the heavy showcase.
- `kkt-26.11.22` D7: "у тебя есть кэш рендера - ты его декодируешь как видео и отображаешь - дальше что не так?" What
  is held final needs no source. On disk that is true. In memory it is not: a kept composite is found by a key that
  contains the decoded frames, so the program must keep both.
- Found-defects 965 (mosh-sharp): leaving the kept moments' frames out of the decoded-frame cache's want turned a
  loop of 21.1 V-fps into 1.9.

## What Changes

- **One key for a moment**, in kt-engine, used by the memory and by the disk. It names each frame by what determines
  it (which source, its identity, the index, the decode size), never by whether it is decoded now.
- **A source carries an identity**: a file's name, size and modification time taken when it was opened; a generator's
  values; otherwise a number of this process. The decoded-frame cache holds it beside the source's entries, so a
  frame and the identity it was decoded from cannot be taken apart.
- **The decoded-frame cache stops counting moments held final in memory**, as cut 1 already does for the disk.
- Three defects found by reading, closed by the same means: a Datamosh over a relinked file keeps the old file's
  history (1061); the disk prerender can name an old decoder's picture by the new file (1062) and a generator's old
  picture by its new values (1063).

## Capabilities

### Modified Capabilities

- `video-plan`: the key of a kept moment.
- `media-index`: the identity of a source in the cache; held moments cost no decoded frame, in memory too.

## Impact

- kt-cache `lib.rs`; kt-media `lib.rs` (`MediaSource::identity`), `ffmpeg.rs`, `generator.rs`, `synthetic.rs`.
- kt-engine `worker.rs`, `video.rs` (`layer_src`), `video_stream.rs` (`ident`, the reads), `video_plan.rs`,
  `render_cache.rs` (the key; `Leaves`, `FileIds`, `restored_key` deleted), `player.rs`, `prefetch.rs`,
  `compose_queue.rs`.
- kt-gpu `engine.rs`: `plan_hash` deleted, `plan_key` calls the engine's key.
- No pixel changes, no project format change, no interface. The disk store's key moves once (one miss per entry).
- `project-cache-folder` section 3 and `persistent-cache` build on this key; see the build queue for the order.
