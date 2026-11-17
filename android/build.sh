#!/bin/sh
# Bangun APK Swara: 32-bit, 64-bit, dan universal (32+64).
#
# Tanpa Gradle. Yang dipakai cuma alat dari Android SDK: cargo-ndk untuk
# menyusun libswara.so per ABI, javac + d8 untuk kelas Java, aapt2 untuk
# manifest dan ikon, zipalign + apksigner untuk berkas akhir yang ditandatangani.
#
# Pakai:
#   android/build.sh               # tiga APK rilis
#   android/build.sh uji           # satu APK x86_64 untuk emulator
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
SDK=${ANDROID_SDK_ROOT:-/root/android-sdk}
BT="$SDK/build-tools/34.0.0"
PLAT="$SDK/platforms/android-34/android.jar"
OUT="$ROOT/android/out"
KS=${SWARA_KEYSTORE:-/root/.swara-android/swara-release.jks}
KSPASS=${SWARA_KEYSTORE_PASS:-}
ALIAS=swara
VERSI=$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)
VC=$(echo "$VERSI" | awk -F. '{printf "%d%02d%02d", $1, $2, $3}')

MODE=${1:-rilis}

export PATH="/root/.cargo/bin:$PATH"
export ANDROID_SDK_ROOT="$SDK" ANDROID_HOME="$SDK"
# LTO tipis membuat libswara.so mati saat dibuka di Android (SIGSEGV di
# ANativeActivity_onCreate). Di build Android LTO dimatikan.
export CARGO_PROFILE_RELEASE_LTO="${CARGO_PROFILE_RELEASE_LTO:-false}"
# javac dan d8 butuh JDK 17; pakai yang terpasang di mesin ini kalau belum diatur.
export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/openjdk17}"
export PATH="$JAVA_HOME/bin:$PATH"

mkdir -p "$OUT"
rm -rf "$OUT/classes" "$OUT/dex" "$OUT/apk" "$OUT/res"
mkdir -p "$OUT/classes" "$OUT/dex" "$OUT/apk/lib"

echo "== Rust (cdylib) untuk Android"
cd "$ROOT"
if [ "$MODE" = "uji" ]; then
    cargo ndk -t x86_64 -o android/lib build --release --lib
else
    cargo ndk -t armeabi-v7a -t arm64-v8a -o android/lib build --release --lib
fi

echo "== Java"
javac --release 11 -classpath "$PLAT" -d "$OUT/classes" \
    $(find android/java -name '*.java')
"$BT/d8" --release --min-api 24 --lib "$PLAT" --output "$OUT/dex" \
    $(find "$OUT/classes" -name '*.class')

echo "== manifest + ikon"
"$BT/aapt2" compile --dir android/res -o "$OUT/res.zip"
"$BT/aapt2" link -o "$OUT/base.apk" -I "$PLAT" \
    --manifest android/AndroidManifest.xml \
    --min-sdk-version 24 --target-sdk-version 34 \
    --version-code "$VC" --version-name "$VERSI" \
    "$OUT/res.zip"

# Satu APK per kumpulan ABI. `lib/<abi>/libswara.so` disusun di direktori
# sementara supaya strukturnya persis seperti yang dicari Android.
susun() {
    nama=$1
    shift
    rm -rf "$OUT/apk/lib"
    for abi in "$@"; do
        mkdir -p "$OUT/apk/lib/$abi"
        cp "android/lib/$abi/libswara.so" "$OUT/apk/lib/$abi/libswara.so"
        # oboe (backend audio Android) ditulis dalam C++ dan butuh runtime ini.
        case "$abi" in
            arm64-v8a) triple=aarch64-linux-android ;;
            armeabi-v7a) triple=arm-linux-androideabi ;;
            x86_64) triple=x86_64-linux-android ;;
            x86) triple=i686-linux-android ;;
            *) triple="" ;;
        esac
        libcpp="$SDK/ndk/27.1.12297006/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/$triple/libc++_shared.so"
        if [ -f "$libcpp" ]; then
            cp "$libcpp" "$OUT/apk/lib/$abi/libc++_shared.so"
        fi
    done
    cp "$OUT/base.apk" "$OUT/$nama.unsigned.apk"
    (cd "$OUT/dex" && zip -q "$OUT/$nama.unsigned.apk" classes.dex)
    (cd "$OUT/apk" && zip -q -r "$OUT/$nama.unsigned.apk" lib)
    "$BT/zipalign" -f -p 4 "$OUT/$nama.unsigned.apk" "$OUT/$nama.aligned.apk"
    "$BT/apksigner" sign \
        --ks "$KS" --ks-key-alias "$ALIAS" \
        --ks-pass "pass:$KSPASS" --key-pass "pass:$KSPASS" \
        --v1-signing-enabled true --v2-signing-enabled true --v3-signing-enabled true --v4-signing-enabled true \
        --out "$OUT/$nama.apk" "$OUT/$nama.aligned.apk"
    rm -f "$OUT/$nama.unsigned.apk" "$OUT/$nama.aligned.apk"
    "$BT/apksigner" verify --print-certs "$OUT/$nama.apk" | head -3
}

echo "== paket"
if [ "$MODE" = "uji" ]; then
    susun "swara-$VERSI-x86_64-uji" x86_64
else
    susun "swara-$VERSI-armeabi-v7a" armeabi-v7a
    susun "swara-$VERSI-arm64-v8a" arm64-v8a
    susun "swara-$VERSI-universal" armeabi-v7a arm64-v8a
fi

ls -lh "$OUT"/*.apk
# base.apk cuma perantara, jangan ikut terunggah sebagai artefak rilis.
rm -f "$OUT/base.apk"
