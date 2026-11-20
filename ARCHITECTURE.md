# Architecture

IsoWeaver is a Rust and GTK 4 desktop application. External tools are launched directly with argument vectors.

## Components

- `src/main.rs`: GTK layout, menus, dialogs, selection, session state, ISO listing, extraction, staged-add previews, and background operation dispatch.
- `src/backend.rs`: staged change types, xorriso inspection, metadata formatting, ISO rebuilds, temporary output paths, and validation.
- `src/runtime.rs`: resolves helpers bundled under `APPDIR/usr/bin`, falling back to `PATH` for source runs.
- `build.rs` and `data/resources.xml`: compile application icons into a GLib resource embedded in the executable.
- `data/applications` and `data/icons`: desktop launcher and installed icons.
- `packaging/linux`: local installation and AppImage build scripts.

## Data flow

1. A worker lists the selected ISO with `7z`; GTK displays the entries.
2. Editing commands accumulate in the session as `Change` values. Host additions also update the visible entry model and retain their source paths.
3. Save chooses an unused dated filename beside the source. xorriso loads the source and applies the staged operations directly to a temporary sibling output; the application does not first extract the whole ISO.
4. The result is reopened and checked for detected boot structures, then committed without replacing an existing dated destination.
5. The editor loads the saved copy.

Opening, extraction, inspection, and rebuilding run on worker threads. GTK polls their completion and displays modal pulsing progress bars. Recursive host-folder enumeration still runs on the GTK thread.

## Boundaries

Extraction validates image paths and destination components and refuses overwrites. Staged additions are extracted from host files; other entries are streamed from `7z`. Boot validation checks structure presence rather than complete metadata equivalence. See [Known issues](KNOWN_ISSUES.md).
