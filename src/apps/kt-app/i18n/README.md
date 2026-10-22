# Translation catalogues

The text of the Kinetren interface lives here, one file per language:
`en`, `ru`, `es`, `zh-Hans`, `ar`. **`en.toml` is the reference**: it sets the set and
order of keys, and the others must repeat it.

## Format

A strict subset of TOML: a comment `#`, a section header `[a.b]`, and a pair
`key = "value"` on ONE line. Escaping: `\n`, `\t`, `\"`, `\\`,
`\uXXXX`. The file remains valid TOML — any editor opens it and
`taplo` checks it.

## What breaks the build

`cargo build` fails (rather than silently showing an empty label) if:

* a key is missing in some catalogue, the value is empty, or the key is missing from `en.toml`;
* the set of placeholders `{0}`, `{1}` differs from the English one;
* the value is longer than 160 characters — a reordered Arabic paragraph cannot be re-wrapped, so a long text is split into two keys;
* the value contains a character from the private use area (U+E000…U+F8FF) — Phosphor icons are assembled in Rust, not in the catalogue;
* an Arabic value contains an ASCII comma, semicolon or question mark: the Arabic `،` `؛` `؟` are needed — they lie inside the Arabic block and do not break a word.

## Translation rules

1. **Placeholders are positional.** `{0}`, `{1}` — not `{}`. The order of arguments
   may be changed: `"clip {0} of {1}"` → `"{1} 个片段中的第 {0} 个"`.
2. **There is NO agreement with the number.** We write "noun: N"
   (`"Clips copied: {0}"`), not "N clips copied". This way the wording
   works in all five languages without plural tables.
3. **There are no hotkeys in the menu.** The shortcut (`Ctrl+S`) is glued together in Rust
   (`Toolbar::menu_btn`). Accelerators that ARE part of the hint text —
   `"Copy (Ctrl+C)"` — are carried over verbatim.
4. **Effect names are not translated**: `Pan`, `Blur`, `VHS`, `Slowed+Reverb`,
   `Morpher`. Effects are searched by them in any locale.
5. **The section `[fxreg.kw]`** holds the search keywords, in lower case, separated by spaces. Keep English words IN THEM next to the local ones: the search runs both on the
   current language and on English. This section is the only one that is not reordered for Arabic — it is compared with what the user types.
6. **The section `[lang]`** — the self-names of the languages; they are the same in ALL catalogues (in the list of languages each item is written in its own language). The build checks this.
7. **Brevity.** The length is checked by test 128 against the real layout budgets: the three menu headings — 160 px each, a line of the performance panel — 190 px. The panel limits (420 px and 90 px) were set by direct requests of the user and do not move — translations make things shorter.
8. **Do not create obvious labels** (rule item 17 of R.7): a value with a unit explains itself; the prefixes "zoom:", "FPS:" are not to be brought back.

## Untranslated text

The build prints a list of keys whose value matches the English one:

```
i18n ES: 15 value(s) identical to English: fx.horizontal fx.vertical render.audio …
```

This is a warning, not an error: a match is often legitimate — units
("Mbit/s"), effect names ("Morpher"), jargon ("underruns", "no-read drops")
and words that are spelled the same in the language ("Horizontal" and "Vertical" in
Spanish). The keys are named one by one on purpose: from a single counter one has to
guess what stands behind it.

## After an edit

```bash
cd src
python3 scripts/subset-fonts.py     # if new ideographs were added
cargo test -p kt-app                # 100 (glyphs), 125 (catalogues), 128 (lengths)
```

`zh_TW` and `zh_HK` point to this same simplified catalogue: there is no traditional one yet.

## A new key in the code

```rust
use crate::i18n::{k, tr};
ui.button(tr(k::menu::file::SAVE));            // without arguments
self.note(crate::tf!(k::msg::CLIPS_COPIED, n)); // with arguments
```

The constant's path MUST be FULL (`k::section::CONSTANT`): test 125 searches for the use
precisely by it; an abbreviated `use` would hide the key and it would land in
the list of unused ones.
