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

The suite and release build have passed locally. These checks do not establish complete functional or desktop visual coverage.

## Before release

Manually check light/dark themes, keyboard navigation, real pointer input, narrow windows, long filenames, dialogs, and desktop icon updates. Test corrupt images, Unicode names, traversal and symlink rejection, extraction failures, large images, and unwritable destinations. Verify BIOS, UEFI, and hybrid boot fixtures independently; current tests do not prove boot preservation. Test the AppImage on a clean target distribution.
