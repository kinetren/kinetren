# Spec Delta

## ADDED Requirements

### Requirement: Paths of media on Windows and across operating systems

The writer SHALL encode a Windows absolute path as a valid file URL:
`C:\a b\x.mp4` → `file:///C:/a%20b/x.mp4`, and a UNC path `\\server\share\x.mp4` →
`file://server/share/x.mp4`. The reader SHALL decode `file:///C:/…`, `file://localhost/C:/…`
and `file://server/share/…` back to `C:\…` and `\\server\share\…` on Windows. Relativising
on Windows SHALL treat "only the drive (or UNC share) is shared" the way Linux treats "only
the root is shared": media on another drive or share is written as an absolute file URL,
never as a chain of `..`. Components SHALL compare without regard to case on Windows, and
the verbatim prefix `\\?\` SHALL be removed before comparison and before writing. A path
that is absolute on the OTHER operating system SHALL be recognised as absolute on reading:
`/home/u/a.mp4` read on Windows, and `D:\x.mp4` or `D:/x.mp4` read on Linux. Such a path
MUST NOT be joined to the project directory. It becomes an offline placeholder that shows
the path exactly as written, and writing the project back keeps it unchanged.
Why: the URL encoder percent-encoded the raw string (`file://C%3A%5Ca%5Cb.mp4`), which no
other tool reads. The decoder turned `file:///C:/x` into the drive-relative `/C:/x`. The
relativiser counted the drive and root as two shared components and wrote
`../../../Media/a.mp4` across the whole disk. `Path::is_absolute` is the reader's OS
opinion: a project saved on Linux and opened on Windows turned `/home/u/a.mp4` into
`C:\project\home\u\a.mp4`, a different (missing) file. This item was deferred in
`project-format/design.md` ("cross-platform paths in `target_url`"); the Windows build makes
it current.

#### Scenario: File URLs with drive letters and UNC both ways
- **WHEN** Windows paths `C:\a b\x.mp4`, `D:\Медиа\кадр 1%.mov` and `\\server\share\x.mp4` are encoded and decoded by the Windows rules (pure functions, checked on any OS), and `file://localhost/C:/x.mp4` and the Linux `file:///home/u/a.mp4` are decoded
- **THEN** the URLs are `file:///C:/a%20b/x.mp4`, `file:///D:/%D0%9C…/%D0%BA%D0%B0%D0%B4%D1%80%201%25.mov` and `file://server/share/x.mp4`; each decodes back to the original path; `localhost` gives `C:\x.mp4`; the Linux URL decodes to `/home/u/a.mp4` unchanged
- **TEST** t853 (`src/libs/kt-otio/tests/format.rs`)

#### Scenario: Relativising on Windows
- **WHEN** with project `C:\Users\x\proj\p.kt` the paths `C:\Users\x\proj\m\a.mp4`, `c:\users\X\PROJ\b.mp4`, `\\?\C:\Users\x\proj\c.mp4`, `C:\Media\d.mp4`, `D:\e.mp4` and `\\nas\v\f.mp4` are written by the Windows rules
- **THEN** they give `m/a.mp4`, `b.mp4`, `c.mp4`, `file:///C:/Media/d.mp4` (only the drive and its root are shared), `file:///D:/e.mp4` and `file://nas/v/f.mp4`
- **TEST** t854 (`src/libs/kt-otio/tests/format.rs`)

#### Scenario: A path from the other OS stays as written
- **WHEN** a project holding `/home/u/a.mp4` is read by the Windows rules, and a project holding `D:\x.mp4` and `D:/y.mp4` is read by the Linux rules; then each is written back
- **THEN** each path resolves to itself (not joined to the project directory), the media is an offline placeholder showing that path, and the written file contains the same strings as before
- **TEST** t855 (`src/libs/kt-otio/tests/format.rs`)
