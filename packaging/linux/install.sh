#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
install_prefix=${1:-"${HOME}/.local"}

if [ ! -x "$project_dir/target/release/isoweaver" ]; then
    printf '%s\n' "Missing release binary. Build it first with: cargo build --release" >&2
    exit 1
fi

install -Dm755 "$project_dir/target/release/isoweaver" "$install_prefix/bin/isoweaver"
install -Dm644 "$project_dir/data/applications/org.isoweaver.IsoWeaver.desktop" \
    "$install_prefix/share/applications/org.isoweaver.IsoWeaver.desktop"
install -Dm644 "$project_dir/data/icons/scalable/apps/org.isoweaver.IsoWeaver.svg" \
    "$install_prefix/share/icons/hicolor/scalable/apps/org.isoweaver.IsoWeaver.svg"
install -Dm644 "$project_dir/data/icons/256x256/apps/org.isoweaver.IsoWeaver.png" \
    "$install_prefix/share/icons/hicolor/256x256/apps/org.isoweaver.IsoWeaver.png"

printf 'Installed IsoWeaver under %s\n' "$install_prefix"
