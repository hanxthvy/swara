#!/bin/sh
# Bersihkan cache build yang tidak dipakai lagi.
#
#   scripts/clean-cache.sh [batas_mb]
#
# Yang selalu dibuang: direktori `incremental` (riwayat kompilasi; dibuat ulang
# otomatis dan cepat, tapi bisa sampai beberapa GB).
# Kalau `target/debug` melewati batas (default 6000 MB), berkas di `deps` yang
# paling tua dihapus sampai di bawah batas. Berkas yang masih dipakai cargo
# akan dibuat ulang saat build berikutnya; ini menukar waktu dengan ruang.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BATAS=${1:-6000}
TARGET="$ROOT/target"

[ -d "$TARGET" ] || exit 0

mb() { du -sm "$1" 2>/dev/null | cut -f1; }

sebelum=$(mb "$TARGET")

# 1. incremental selalu aman dibuang
for d in "$TARGET"/*/incremental; do
    [ -d "$d" ] && rm -rf "$d"
done

# 2. deps: buang yang paling tua kalau folder debug kelewat besar
DEBUG="$TARGET/debug"
if [ -d "$DEBUG" ]; then
    while [ "$(mb "$DEBUG")" -gt "$BATAS" ]; do
        # 200 berkas tertua per putaran; berhenti kalau tidak ada lagi
        hapus=$(find "$DEBUG/deps" -maxdepth 1 -type f -printf '%T@ %p\n' 2>/dev/null \
            | sort -n | head -200 | cut -d' ' -f2-)
        [ -n "$hapus" ] || break
        echo "$hapus" | while IFS= read -r f; do rm -f "$f"; done
    done
fi

sesudah=$(mb "$TARGET")
echo "cache: ${sebelum} MB -> ${sesudah} MB (batas debug ${BATAS} MB)"
