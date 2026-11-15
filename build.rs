//! Penyesuaian build khusus Android.
//!
//! `oboe-sys` memakai pustaka oboe yang sudah dibangun lebih dulu, jadi ia
//! tidak pernah memanggil `cc::Build::cpp_link_stdlib` dan runtime C++ tidak
//! ikut tertaut. Akibatnya `libswara.so` gagal dibuka di HP dengan pesan
//! `cannot locate symbol "__cxa_pure_virtual"`. Di sini runtime itu ditautkan
//! eksplisit; berkas `libc++_shared.so`-nya ikut dibungkus ke dalam APK.

fn main() {
    println!("cargo:rerun-if-env-changed=ANDROID_NDK_HOME");
    println!("cargo:rerun-if-env-changed=ANDROID_NDK_ROOT");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("android") {
        return;
    }
    // Jangan menambahkan `rustc-link-search` ke sysroot/usr/lib/<triple>: di
    // direktori itu hanya ada arsip `.a` (libc.a, libm.a, libdl.a), sehingga
    // `-lc` jadi menautkan libc statis dan proses punya dua alokator — gejalanya
    // SIGSEGV di scudo saat thread pertama dibuat. Cukup minta libc++ dinamis;
    // clang sudah tahu sendiri di mana mencarinya.
    println!("cargo:rustc-link-lib=dylib=c++_shared");
}
