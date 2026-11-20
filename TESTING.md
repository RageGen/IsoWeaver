# Testing

## Run automated checks

```sh
cargo check --locked
cargo test --locked
cargo build --release --locked
```

The tests require `xorriso` and a working GTK display. For a headless machine with GTK Broadway installed, start `gtk4-broadwayd :5` in another terminal, then run:

```sh
GDK_BACKEND=broadway BROADWAY_DISPLAY=:5 cargo test --locked
```

Use `--offline` when the locked Cargo dependencies are already cached.

## Current coverage

Four test functions cover metadata grouping and missing boot data, real ISO copy saving and source/existing-target preservation, and GTK behavior. The GTK test includes click/Ctrl-click selection, dated filename collisions, embedded icon lookup, menu actions and disabled states, short-menu allocation, staged nested-folder previews and extraction, worker completion, and status-text bounds at 100%, 125%, and 150% text size.

The suite and release build have passed locally. Both the top image-status label and the footer entry count have allocation checks at the three text sizes. These checks do not establish complete functional or desktop visual coverage.

## AppImage verification

A local x86_64 AppImage build succeeded. The archive was extracted to check the bundled application, `7z`, `xorriso`, root PNG icon, and `.DirIcon` link. Bundled xorriso reported its version successfully. The AppImage stayed running during a three-second launch check using GTK Broadway and extraction mode; it was then stopped by the test timeout. Broadway emitted a frame-timing warning.

This was a launch check, not an end-to-end ISO editing test or a clean-system portability test. Packaging still emits an AppStream metadata warning.

## Before release

Manually check light/dark themes, keyboard navigation, real pointer input, narrow windows, long filenames, dialogs, and desktop icon updates. Test corrupt images, Unicode names, traversal and symlink rejection, extraction failures, large images, and unwritable destinations. Verify BIOS, UEFI, and hybrid boot fixtures independently; current tests do not prove boot preservation. Test the AppImage on a clean target distribution.
