#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
linuxdeploy=${LINUXDEPLOY:-linuxdeploy}

if ! command -v "$linuxdeploy" >/dev/null 2>&1; then
    printf '%s\n' "linuxdeploy is required to build the AppImage (build-time dependency)." >&2
    exit 1
fi
for tool in 7z xorriso; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'Missing %s. Install it on the build machine first.\n' "$tool" >&2
        exit 1
    fi
done

cargo build --release --locked --manifest-path "$project_dir/Cargo.toml"
appdir=$(mktemp -d "${TMPDIR:-/tmp}/isoweaver-appdir.XXXXXX")
chmod 755 "$appdir"
trap 'rm -rf "$appdir"' EXIT HUP INT TERM
export APPIMAGE_EXTRACT_AND_RUN=1
mkdir -p "$project_dir/dist"
cd "$project_dir/dist"
"$linuxdeploy" \
    --appdir "$appdir" \
    --executable "$project_dir/target/release/isoweaver" \
    --executable "$(command -v 7z)" \
    --executable "$(command -v xorriso)" \
    --desktop-file "$project_dir/data/applications/org.isoweaver.IsoWeaver.desktop" \
    --icon-file "$project_dir/data/icons/256x256/apps/org.isoweaver.IsoWeaver.png" \
    --output appimage
