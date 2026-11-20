# IsoWeaver

A native Rust and GTK 4 application for browsing, extracting, and editing ISO images on Linux. Licensed under [GPL-3.0-or-later](LICENSE).

Version 0.1.0 is an initial release candidate. Read [Known issues](KNOWN_ISSUES.md) before rebuilding bootable images.

## Features

- Browse folders and filter the current folder by name.
- Select one entry with a click; use Ctrl-click to add or remove individual selections.
- Extract selected files, folders, or the entire image without overwriting existing files.
- Stage additions, replacements, deletions, renames, and new folders.
- See added files and nested folders immediately, marked **Added (unsaved)**.
- Save a validated, dated copy beside the source, preserving the source image.
- Inspect structured volume, date, and boot information, with the full xorriso report available.
- Run image operations in background workers with pulsing progress indicators.

## Build and run

Install Rust, Cargo, GTK 4 development files, GLib resource tools, `pkg-config`, `7z`, and `xorriso`. See [Build instructions](BUILD.md) for details.

```sh
cargo build --release --locked
cargo run --release --locked
```

Install the built application, launcher, and icons under `~/.local`:

```sh
packaging/linux/install.sh
```

The installer refreshes available desktop and icon caches. Restart the application after an update.

## Save edited images

Choose **Save** to create `name_edited_YYYY-MM-DD_HH-MM-SS.iso` beside the open image. The timestamp uses local time. A numeric suffix avoids existing filenames. The destination folder must be writable.

Changes remain staged until saved. The source stays untouched. Boot checks verify the presence of detected structures; they do not guarantee preservation of every boot parameter or filesystem extension.

## Portable packaging

The local [AppImage build script](packaging/linux/build-appimage.sh) bundles the application, GTK libraries, 7-Zip, and xorriso into `dist/IsoWeaver-x86_64.AppImage`. A local build and launch check succeeded; clean-system portability remains unverified. See [Build instructions](BUILD.md) for building and running it. No GitHub Actions workflow is included in this checkout.

## Documentation

- [User guide](USER_GUIDE.md)
- [Build instructions](BUILD.md)
- [Testing](TESTING.md)
- [Known issues](KNOWN_ISSUES.md)
- [Architecture](ARCHITECTURE.md)
- [Roadmap](ROADMAP.md)
- [Changelog](CHANGELOG.md)

## Contributing

Include your distribution, GTK version, relevant tool output, and reproduction steps when reporting a bug. Use small, redistributable ISO fixtures; do not attach proprietary or sensitive images. Contributions use GPL-3.0-or-later.
