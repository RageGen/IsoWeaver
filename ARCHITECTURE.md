# Architecture

IsoWeaver is a GTK 4 desktop application written in Rust. The GTK window and user actions live in `src/main.rs`; ISO listing, extraction, modification, and validation are implemented in `src/backend.rs`.

## Data flow

1. The UI asks the backend to list the selected ISO using `7z`.
2. The selected folder and its entries are shown in the directory and file panes.
3. Editing actions are recorded as staged changes in the UI session.
4. The backend extracts the source into a temporary work area, applies staged operations, and asks `xorriso` to create a temporary output image.
5. The output is reopened and checked for detected boot data before it is copied into the destination.

External programs are launched directly with argument vectors. ISO contents are treated as untrusted data; extraction validates paths and refuses unsafe or conflicting destinations.

## Main components

- `src/main.rs`: GTK application setup, menus, toolbar, dialogs, browser panes, status, and staged edit state.
- `src/backend.rs`: subprocess handling, image listing, safe extraction, temporary paths, ISO rebuild, and post-build checks.
- `data/applications`: Linux desktop entry.
- `data/icons`: scalable and 256-pixel application icon used by the desktop entry and About dialog.
- `packaging/linux`: local Linux installation helper.

## Constraints

Operations currently run synchronously from GTK actions, so large images can temporarily block the interface. Boot verification checks the presence of detected structures rather than every boot parameter and metadata field. See [Known issues](KNOWN_ISSUES.md).
