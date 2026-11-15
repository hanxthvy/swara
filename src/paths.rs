//! Lokasi berkas milik Swara, mengikuti kebiasaan tiap sistem operasi.
//!
//! Linux memakai XDG, macOS memakai `~/Library`, Windows memakai `%LOCALAPPDATA%`.
//! Semua jalur dihitung saat dipanggil, bukan disimpan, supaya tes bisa
//! mengarahkan `HOME`/`XDG_CACHE_HOME` ke direktori sementara.

use std::path::PathBuf;

/// Di Android tidak ada `HOME`: aplikasi hanya boleh menulis di direktori
/// privatnya sendiri, dan jalurnya baru diketahui setelah Activity hidup.
#[cfg(target_os = "android")]
static ANDROID: std::sync::OnceLock<(PathBuf, PathBuf)> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
pub fn set_android_dirs(data: PathBuf, cache: PathBuf) {
    let _ = ANDROID.set((data, cache));
}

fn home() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

/// Tempat kredensial InnerTube dan sesi akun. Aman dihapus kapan saja.
pub fn cache_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    if let Some((_, cache)) = ANDROID.get() {
        return cache.clone();
    }
    let base = if cfg!(windows) {
        std::env::var("LOCALAPPDATA").ok().map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library").join("Caches"))
    } else {
        std::env::var("XDG_CACHE_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".cache")))
    };
    base.unwrap_or_else(std::env::temp_dir).join("swara")
}

/// Tempat berkas yang seharusnya bertahan: pengaturan dan riwayat.
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    if let Some((data, _)) = ANDROID.get() {
        return data.clone();
    }
    let base = if cfg!(windows) {
        std::env::var("APPDATA").ok().map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library").join("Application Support"))
    } else {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".local").join("share")))
    };
    base.unwrap_or_else(std::env::temp_dir).join("swara")
}
