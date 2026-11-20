# Build IsoWeaver

## Requirements

- Stable Rust and Cargo
- GTK 4 development package, GLib development tools (`glib-compile-resources`), and `pkg-config`
- `7z` executable
- `xorriso` executable

Install these dependencies using your distribution’s package manager. Check that `glib-compile-resources`, `pkg-config`, `7z`, and `xorriso` are available on `PATH`. The Cargo lockfile is checked in for reproducible dependency resolution.

## Commands

```sh
cargo check --locked
cargo build --release --locked
cargo run --release --locked
```

The release binary is `target/release/isoweaver`. For a local desktop installation, run `packaging/linux/install.sh` after building. To install to a system prefix, run the script with a prefix such as `/usr/local`.

## Source-run dependencies

When running directly from Cargo, IsoWeaver calls `7z` for listing and streaming extraction and `xorriso` for ISO inspection, modification, verification, and supported boot-data replay. Both programs must be installed on the build/run machine. The portable AppImage packages them for end users.

## Portable AppImage

The x86_64 Linux AppImage is built as a single executable file that bundles the application, GTK shared libraries, 7-Zip, xorriso, and their detected shared-library dependencies. The build machine needs GTK development files, 7-Zip, xorriso, and `linuxdeploy`; these are build-time tools and are not required on a user's machine.

```sh
chmod +x packaging/linux/linuxdeploy-x86_64.AppImage
LINUXDEPLOY="$PWD/packaging/linux/linuxdeploy-x86_64.AppImage" packaging/linux/build-appimage.sh
```

The result is `dist/IsoWeaver-x86_64.AppImage`. The script also accepts another linuxdeploy executable through `LINUXDEPLOY`, or uses `linuxdeploy` from `PATH` by default. The packager may need network access to download the AppImage runtime. Both the scalable installed icon and a root-level PNG launcher icon are included. No GitHub Actions workflow is included in this checkout. Build and validate the AppImage locally before distributing it. The package still uses the host Linux kernel, graphics driver, and display server.

## Run the AppImage

```sh
chmod +x dist/IsoWeaver-x86_64.AppImage
./dist/IsoWeaver-x86_64.AppImage
```

On a system without working FUSE support, use the extraction fallback:

```sh
APPIMAGE_EXTRACT_AND_RUN=1 ./dist/IsoWeaver-x86_64.AppImage
```

The current AppImage was built and passed a short launch check using GTK Broadway. This does not establish portability across clean distributions or graphics environments; see [Testing](TESTING.md).
