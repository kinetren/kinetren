# The cache folder of a project (`kinetren-cache/`)

Everything a project can compute again lies in one folder beside it, shared by the projects of that
folder: the media index (peaks, packet indexes, thumbs, posters, loudness, sync offsets), the reel
previews and the preview's frames. It is a **cache**: deleting it costs time, never work. The rules are
`openspec/specs/media-index/spec.md`; the code is `kt-media/src/{cache_folder,index,old_sidecar}.rs` and
`kt-engine/src/render_cache.rs`.

```
kinetren-cache/
  CACHEDIR.TAG  README.txt  .gitignore   what it is, to backup tools, people and git
  lock                                   tried, never waited for: the cycle's drops, the map's compaction
  usage.tsv                              per entry: bytes, last use, what a read saves, reads
  media/paths.tsv                        path, size, mtime -> content id (relative inside the project's folder)
  media/<content id>/                    peaks, packets-audio, packets-video, thumbs, thumb-size, poster,
                                         loudness, series, complete
  pairs/<id a>-<id b>                    sync offsets
  reels/<seq>-<revision>-<files>/        strip, peaks;  reels/refs/<project>-<seq> names a project's version
  results/v<FORMAT>/<device class>/frames/   the preview's final frames: video segments and their manifests
```

- **An entry file** has a 32-byte header: `KTC1`, its kind, the kind's version, the payload's length and
  xxh3-64. It is written under a temporary name and renamed. What does not verify is a miss and is removed.
  `thumbs` is a file of records, each with its own check, appended.
- **A media is its content**: `<content id>` is the xxh3-128 of the whole file. The map spares the hash while
  the path, the size and the modification time stand.
- **The room** is the folder's cap (Preferences, 20 GB by default) or the disk's free space less a reserve,
  whichever is smaller. A write that does not fit drops what is worth least first: frames, the mix and the
  copies of sources by value per byte, then the reels, then the media, and never what this process used in
  the last minute. The first drop of a session posts a notice.
- **A folder that takes no files**, and an untitled project, cache under `<data>/project-caches/`.
- **Every folder** is listed in `<data>/cache-folders.tsv`; Preferences show them with their sizes.
- **A former `<project>.ktindex.db`** (SQLite) is read into the folder once, in the background, and deleted.
