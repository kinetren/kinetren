# Tasks: timecode-is-text

> DESIGN ONLY until the orchestrator's go. No open question. Tests t3993–t3997. After `fx-graph-dag` (format 8), any order
> with `presets-everywhere`.

## 1. The repair first (the kind still there)

- [ ] 1.1 `RETIRED_GENERATORS` (empty), the JSON walk, `ProjectFileError::Retired`; `import_interchange` on a `.kt`
- [ ] 1.2 `retired_timecode`: media, event, curves, links, published, overrides, morpher patches; the look (D3); t3993

## 2. The kind goes

- [ ] 2.1 the plugin, its id, the `ORDER` row, `TC_NOTE`, its catalog key; the t1048 row goes with it, t493 reads the title (the owner's answer 5)
- [ ] 2.2 the menu row as a title, `kind_name`, the probe; t3995, live `text_counter` extended by the menu row
- [ ] 2.3 `timecode` enters the table; t3994; recovery, t3996

## 3. The door

- [ ] 3.1 the dialog's filter, the `-imported` hint; t3997; i18n ×5

## 4. Close

- [ ] 4.1 What's new; F-TXT-007 and F-TXT-001 "Verdict" lines; registry G3 → done; `docs/guide` pages naming the kind
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
