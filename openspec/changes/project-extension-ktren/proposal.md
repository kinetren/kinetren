# Proposal: project-extension-ktren

> DESIGN ONLY (spec session, 2026-11-25). Found by the registry audit: the owner's answer 8 of 2026-11-07 no change
> covered (`docs/user-feedback.md`, "Round of answers"; found-defects 17). Tests reserved: t4259–t4262.

## Why

The owner, 2026-11-07: "в.8. .ktren breaking change". What it decides (the recorded reading): a `.kt` no longer opens by
plain Open and goes through the import door (R.67). The cause: `*.kt` is also Kotlin's glob, and a name-only guess opens
a project in a code editor (`packaging/com.kinetren.Kinetren.xml` has a magic rule because of it; it helps only those
programs that sniff content).

## What Changes

- The project file's extension is `.ktren`; a save writes it, the dialogs offer it, an unnamed project is `untitled.ktren`.
- A `.kt` (and `.ktproj`) is refused by Open, by a drop and by the command line of the editor, each naming File › Import
  older Kinetren project…; that item takes `.kt` with the fixes of the version it was written in (the reader is the
  same, by content) and opens it as an unnamed document with the save name `<name>.ktren` (R.70's import).
- Recent projects, the last project and the recovery list show and take `.kt` rows: a click runs the import door itself.
- The file association: `*.ktren` is ours alone; `.kt` keeps no association of ours.

## Capabilities

### Modified Capabilities

- `project-format`: the extension, the door, the association.

## Impact

- `kt-model` `project_file::EXTENSION`; `kt-app` dialogs, drops, recents, recovery; `kt-cli` (reads by content, both);
  `packaging/com.kinetren.Kinetren.xml`, the desktop file and the Flatpak manifest; the fixtures' and scripts' names.
- No format version change, no OTIO. What's new says it, as a breaking change with the one step it asks of a person.
