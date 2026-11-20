## ADDED Requirements

### Requirement: A downloaded plugin is a library item with its declared manifest
A plugin that is not built in SHALL reach the program only as a library item of the kind `plugin`: a
package holding its manifest (id, version, API version, families, the parameter passport, the declared
capabilities with no I/O), its module and its licence, pinned by sha256 in the catalogue. It SHALL be
registered only through the external host's door. A project SHALL name the plugin's id and version, and a
project that names a plugin not installed SHALL open in the "missing plugin" state with a line that opens
the plugin's row in the library. A downloaded plugin SHALL never take part in the goldens.
Why: kkt-26.11.20 line 16 ("их же тоже можно будет качать. вот и сделат библиотеку плагинов"); F-EXT-003
fixes what foreign code declares; download-library D6.

#### Scenario: A plugin package is installed, registered and missed
- **WHEN** a test `.ktplugin` is installed from the library, a project using it is saved, the package is removed and the project opened again
- **THEN** the plugin is registered through the external door with its manifest, the project names its id and version, and after the removal the event shows the missing-plugin state with a line to the plugin's row
- **TEST** t3868 (`src/libs/kt-plugin-api/tests/library_door.rs`)
