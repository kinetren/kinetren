## ADDED Requirements

### Requirement: Everything the program downloads is an item of one library
Everything the program downloads SHALL be an item of one library. An item is:
- a codec library, a speech runtime, a speech model, a font pack, or a plugin;
- described by a catalogue entry: id, kind, version, licence, what needs it, its dependencies, and per
  platform its mirrors, sha256, size and unpack rule;
- installed under `<local>/library/<kind>/<id>/<version>/` with a record of the entry it came from and the
  verified hash.

**The list.** The release SHALL carry the list as of its build inside the binary. A fetched list SHALL be used only
when its signature verifies and its sequence number is higher than the list held, and an installed item
SHALL keep the entry it came from.

**Downloading.** Downloads SHALL be made only by one engine:
- the mirrors in order, with the declared length checked before the body;
- a size cap, and the hash checked while receiving;
- Cancel per chunk;
- a partial folder renamed into place, with stale partial folders removed.

A download SHALL start only on the person's click. "Install from file…" and `kt-cli library install --from`
SHALL accept a file the person brings, checked against the same pin.

**Resolving.** One function SHALL turn an item's id into its folder. The FFmpeg search plan, the speech
loader, the font registration and the plugin door SHALL use it, and nothing else SHALL build an item's
path. The FFmpeg search plan SHALL read the library's FFmpeg folder after the person's own folder and
before the former download root, which it SHALL keep reading.

**Loading.** A loader SHALL check the installed version against the versions its entry depends on, and
the item's own check (a library's version symbol, a model's header, a face's checksum). A mismatch SHALL
be named on the item's row, in a notice and in the trace, never a silent fallback.

Why: kkt-26.11.20b line 5 ("Всё с сервера"); kkt-26.11.20 line 16 ("все что у нас качается - надо переносить и делать архитектурно в виде некоторой
библиотеки"). Two designed downloads were about to copy the FFmpeg window. Kdenlive's tracker shows the
cost of separate paths: a model downloaded to one folder and searched for in another, and a runtime update
that silently invalidated its models (F-EXT-006; download-library D1–D4).

#### Scenario: The catalogue's pins and the one resolver
- **WHEN** every entry of the list the release packs is read, and the source tree is searched for code that builds a path under the library root
- **THEN** every entry has a sha256, a size, at least one mirror per platform it lists and a licence, and only `library::resolve` and the installer build such a path
- **TEST** t3850, t3851 (`src/libs/kt-library/tests/library.rs`)

#### Scenario: A download is verified and atomic
- **WHEN** a test item is downloaded from a loopback server that serves it whole, then a server that announces another length, one that serves other bytes under the same name, and one cut off midway, and a download is cancelled
- **THEN** only the first is installed, by a rename with its record; each other leaves no folder and names its reason; the cancelled one leaves no partial folder after the stale cleanup
- **TEST** t3852, t3853 (`src/libs/kt-library/tests/library.rs`)

#### Scenario: FFmpeg is found in the library and in its old place
- **WHEN** the search plan runs with FFmpeg in the library, then with it only in the former download root, then with a folder the person chose
- **THEN** the person's folder wins, then the library's, then the old root, each named in the plan's report
- **TEST** t3854 (`src/libs/kt-ffmpeg/src/search.rs`)

#### Scenario: An item installed from a file, and headless
- **WHEN** a test item's archive is installed with "Install from file…" and with `kt-cli library install --from`, and a file with another hash is offered
- **THEN** the first two are installed as a download would be, `kt-cli library list` shows them, and the wrong file is refused naming the pin
- **TEST** t3855 (`src/apps/kt-app/src/library/tests.rs`), t3856 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: A model that does not fit its runtime is named
- **WHEN** a speech model whose entry depends on runtime ≥ 1.8 is loaded with runtime 1.7 installed, and a model file with a broken header is loaded
- **THEN** neither loads; the rows read "needs whisper-runtime ≥ 1.8" and "broken: header", a notice names each once, and the trace records both
- **TEST** t3860, t3861 (`src/libs/kt-library/tests/library.rs`)

#### Scenario: A server list that does not verify is not used
- **WHEN** a signed list with a higher sequence number, the same list with one byte changed, an older signed list, and a newer list that drops an installed item are read
- **THEN** the first replaces the packed list, the second and third are refused with a trace record naming why, and the fourth is used while the installed item keeps its entry and reads "no longer offered"
- **TEST** t3866, t3867 (`src/libs/kt-library/tests/library.rs`)
