# Build IsoWeaver

## Requirements

- Stable Rust and Cargo
- GTK 4 development package and `pkg-config`
- `7z` executable
- `xorriso` executable

Install distribution packages listed in [README.md](README.md). The Cargo lockfile is checked in for reproducible dependency resolution.

## Commands

```sh
cargo check --locked
cargo build --release --locked
cargo run --release
```

The release binary is `target/release/isoweaver`. For a local desktop installation, run `packaging/linux/install.sh` after building. To install to a system prefix, run the script with a prefix such as `/usr/local`.

## Source-run dependencies

When running directly from Cargo, IsoWeaver calls `7z` for listing and streaming extraction and `xorriso` for ISO inspection, modification, verification, and supported boot-data replay. Both programs must be installed on the build/run machine. The portable AppImage packages them for end users.

## Portable AppImage

An x86_64 Linux AppImage is built as a single downloadable file that bundles the application, GTK shared libraries, 7-Zip, xorriso, and their detected shared-library dependencies. The build machine needs GTK development files, 7-Zip, xorriso, and `linuxdeploy`; these are build-time tools and are not required on a user's machine.

```sh
LINUXDEPLOY=/path/to/linuxdeploy packaging/linux/build-appimage.sh
```

The result is written to `dist/`. The GitHub Actions workflow builds the AppImage and publishes it as a workflow artifact. The package still uses the host Linux kernel, graphics driver, and display server.
