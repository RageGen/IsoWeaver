# Known issues

- Delete, rename, and new-folder operations are staged but do not immediately update the browser. Added host content does appear immediately.
- Clearing selection can leave a stale selected-count status message.
- Folder-tree ordering is by depth, so children can appear separately from their parents. Nodes are not collapsible.
- Replacing a directory can merge new contents with old contents rather than remove obsolete files.
- Save checks reopen the image and verify detected boot-data presence, not every boot parameter, payload checksum, filesystem extension, or ISO metadata field. BIOS, UEFI, and hybrid preservation need broader validation.
- Progress bars indicate activity; percentage estimates and cancellation are unavailable. Recursive host-folder enumeration can block the interface.
- Search filters only the current folder. User-controlled sorting, tabs, preferences, drag-and-drop, recent images, and undo are unavailable.
- Boot configuration editing and raw disk partition inspection are not implemented.
- Images with El Torito data but no system area can be rejected by the post-save check, which currently requires system-area data whenever either boot structure was detected.
- Extraction pipes 7-Zip stderr but drains it only after stdout; sufficiently large diagnostic output can stall extraction. Some early failures can leave an incomplete destination file.
- Saving commits the temporary output using a hard link. Destinations on filesystems without hard-link support, such as FAT/exFAT, fail rather than save the copy.
- AppImage launch was checked locally through Broadway. Clean-system compatibility is unverified, and AppStream metadata is not yet supplied.
- Automated checks pass locally, but full desktop visual verification and clean-system AppImage testing remain outstanding. See [Testing](TESTING.md).
