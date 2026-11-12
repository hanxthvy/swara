#!/bin/sh
# Pasang Swara ke sistem: biner, ikon, dan entri menu aplikasi.
set -e

PREFIX=${PREFIX:-/usr/local}
APPS=${APPS:-/usr/share/applications}
ICONS=${ICONS:-/usr/share/icons/hicolor}
here=$(cd "$(dirname "$0")" && pwd)

install -Dm755 "$here/../target/release/swara" "$PREFIX/bin/swara"
for s in 32 48 64 128 256 512; do
    install -Dm644 "$here/swara-$s.png" "$ICONS/${s}x${s}/apps/swara.png"
done
install -Dm644 "$here/swara.desktop" "$APPS/swara.desktop"

command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -qtf "$ICONS" || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$APPS" || true
echo "swara terpasang di $PREFIX/bin/swara"
