# IsoWeaver

IsoWeaver is a native GTK 4 desktop application for browsing, extracting, and editing ISO images on Linux. Its workspace pairs an image directory pane with a file list, a grouped toolbar, and concise Edit, View, Tools, and Help menus.

> **Release status:** 0.1.0 is the first release candidate. Read [Known issues](KNOWN_ISSUES.md) before relying on bootable-image rebuilds.

## Features

- Browse ISO contents in a directory tree and searchable file list.
- Extract selected files/folders or the whole image.
- Stage file/folder additions, replacements, deletions, renames, and new directories.
- Save changes or create a copy with Save As.
- Inspect volume, El Torito, and system-area information.
- Preserve the source image until a rebuilt temporary image passes xorriso reopen checks.
- Use native GTK dialogs and an application icon.

## Run the portable build

The GitHub Actions build produces a single Linux AppImage containing IsoWeaver, GTK, 7-Zip, and xorriso. Download the `IsoWeaver-AppImage` artifact from a successful workflow, run `chmod +x IsoWeaver*.AppImage`, and launch it. No GTK, 7-Zip, or xorriso installation is needed on the user's machine.

The AppImage still relies on the host Linux kernel, graphics stack, and display server. Building from source requires the dependencies below.

Ubuntu 24.04 or later:

```sh
sudo apt install build-essential pkg-config libgtk-4-dev 7zip xorriso
```

Fedora:

```sh
sudo dnf install gcc pkgconf-pkg-config gtk4-devel p7zip-plugins xorriso
```

`7z` lists ISO entries and streams file extraction; `xorriso` inspects boot data and applies ISO modifications. They must be on `PATH` only when building or running directly from source; the AppImage bundles both tools.

## Build and run

```sh
cargo build --release
cargo run --release
```

To install the release binary, desktop launcher, and icons under `~/.local`:

```sh
packaging/linux/install.sh
```

The installer accepts an optional prefix, for example `packaging/linux/install.sh /usr/local`.

The first run may require logging out and back in for the desktop environment to refresh its application cache.

## Quick guide

1. Choose **Open** and select an ISO.
2. Browse folders in the left pane. Double-click a directory to open it; search filters the current folder.
3. Click an entry to select it; hold **Ctrl** while clicking to add or remove individual entries. **Extract Selected** extracts only highlighted files. Selecting a folder extracts that folder and its contents. Use **Extract All** for the entire image.
4. Choose **Save** or **Save As** to apply staged changes. Replacing an existing image requires confirmation.
5. Use **Tools → Image Information** for volume and boot metadata.

## Integrity and security

- Changes are held in memory until a save operation.
- xorriso writes to a temporary sibling file. IsoWeaver reopens the result and checks detected boot-data presence before replacing the destination.
- Failed rebuilds leave the source image untouched.
- Extraction rejects absolute and traversal paths, symlinks, unsafe destination components, and existing files.
- IsoWeaver never executes files from an image.

## Current limits

IsoWeaver is an ISO-focused first release. It does not inspect raw disk partitions, edit boot configuration, or guarantee preservation of every boot parameter or filesystem extension. Long operations currently run on the GTK thread; progress and cancellation are not implemented. See [Known issues](KNOWN_ISSUES.md), [Roadmap](ROADMAP.md), and [Testing](TESTING.md).

## Project files

- [Architecture](ARCHITECTURE.md)
- [Build instructions](BUILD.md)
- [User guide](USER_GUIDE.md)
- [Changelog](CHANGELOG.md)
- [License](LICENSE) - GPL-3.0-or-later

## Contributing

Bug reports and changes are welcome. Include your Linux distribution, GTK version, the relevant command output, and a small ISO fixture or a way to reproduce the issue. Do not attach proprietary or sensitive disk images. Changes are licensed under GPL-3.0-or-later.
