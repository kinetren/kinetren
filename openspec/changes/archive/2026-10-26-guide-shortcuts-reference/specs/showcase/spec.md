## ADDED Requirements

### Requirement: The guide stays within the site's Markdown, and the shortcuts page is the one key reference
Every file under `docs/guide/` SHALL use only the Markdown the site renders: `#` headings,
paragraphs, `-` and `1.` lists with at most one level of nesting, `|` tables (a pipe inside a
cell written `\|`), fenced code blocks, `**bold**`, `*italic*`, `` `code` ``, `[text](path)`
links, an image on a line of its own, and `<kbd>…</kbd>`. Outside code it MUST NOT contain
any other HTML tag, an unpaired `<kbd>`, a `>` quote, a setext (`===`/`---` underline)
heading, a reference link `[x][y]` or a footnote `[^n]`, `_italic_`, or a backslash other
than `\|`. `docs/guide/shortcuts.md` SHALL be the complete reference of the keys and
mouse gestures the program handles. Each row describes a key only where it really does that
action, and the behaviour stated is the program's, checked against the code. Its tables SHALL
have the header `| Keys | Action |`. Every key and modifier SHALL be written as a
`<kbd>` cap, chords joined with `+`, alternatives with ` / `, and holds, double presses,
wheel and clicks in words next to the caps. It MUST NOT write a key chord as bold text. The guard
`src/scripts/readme-check.py --check` SHALL report each violation as a `markup` defect with
file and line. It runs in `cargo test` and in `finish.sh` like the other classes.
Why: the site (kinetren.github.io) shows `docs/guide/*.md` as `/docs/`, and its renderer
fails the whole build on anything outside this subset. The landing page's "Keyboard" section
was removed (owner, 2026-10-26: "a reference does not belong there"), so `shortcuts.md` is now
the only one, and it was poorer than the landing: bold-text keys, no region or view groups,
handled gestures missing.

#### Scenario: The committed guide passes the markup check
- **WHEN** `readme-check.py --check` runs on the repository
- **THEN** exit 0; `shortcuts.md` has only `| Keys | Action |` tables and no bold key chord
- **TEST** t826 (`src/apps/kt-cli/tests/readme_guard.rs`)

#### Scenario: The guard catches each markup defect
- **WHEN** it runs on a temporary tree whose guide has, one at a time: a `<br>` tag, an unpaired `<kbd>`, a `>` quote, a setext heading, a reference link, a footnote, `_italic_`, a stray backslash, a list nested two levels deep, a `shortcuts.md` table headed `| Key | Action |`, a `**Ctrl+S**` in `shortcuts.md`; and the same constructs inside a fenced block and a code span
- **THEN** each defect exits non-zero and names the file, the line and the class `markup`; `<kbd>`, `\|` in a cell and the constructs inside code are accepted; the clean tree exits 0
- **TEST** t869 (`src/apps/kt-cli/tests/readme_guard.rs`)
