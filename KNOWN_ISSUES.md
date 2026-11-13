# Known issues

- Host additions are mapped to the ISO root. Replacing an existing directory may merge new contents rather than remove obsolete files from the old tree.
- Save verification reopens the result and checks that previously detected El Torito and system-area data remain present. It does not compare every boot parameter, payload checksum, filesystem extension, or other ISO metadata.
- Boot configuration editing and raw disk partition inspection are not implemented.
- Image listing uses 7-Zip, so ISO-specific volume/filesystem metadata is limited to the separate xorriso boot information report.
- Progress, cancellation, sorting, tabs, preferences, and drag-and-drop are absent. Search currently filters only the open folder.
- The current folder tree is a navigable indented directory list; expandable/collapsible tree nodes are not implemented.
- Extraction rejects overwrite, absolute/traversal paths, symlinks, and unsafe destination components. UI operations currently run synchronously and may block for large images.
- `cargo check --offline` and release compilation succeeded. The functional test suite has not yet been run; bootable-image preservation still needs broader verification.
