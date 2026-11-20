# Roadmap

1. Make delete, rename, and new-folder changes visible immediately; fix selection status and folder-tree ordering.
2. Fix boot-check gating, extraction pipe/error handling, and saving to filesystems without hard links; add regression tests.
3. Expand safety tests for path parsing, extraction failures, and rebuild failures.
4. Validate BIOS, UEFI, and hybrid boot preservation with redistributable fixtures.
5. Move host-folder enumeration off the GTK thread and add cancellation and measurable progress where possible.
6. Add sorting, undo, recent images, and collapsible folder navigation.
7. Separate session, GTK, and extraction code into focused modules as needed.
8. Validate portable packaging on clean Linux distributions before publishing releases.
