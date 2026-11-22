//! Lagu yang disimpan untuk diputar tanpa jaringan.
//!
//! Sengaja di direktori data privat aplikasi, bukan folder Unduhan: di Android
//! itu `files/offline`, jadi begitu aplikasi di-uninstall berkasnya ikut hilang
//! (tidak ada sisa lagu yang tertinggal di penyimpanan bersama). Nama berkas
//! memakai id video supaya tidak perlu basis data.

use std::path::PathBuf;

/// `<data>/offline`. Dibuat saat menyimpan, bukan saat dibaca.
pub fn dir() -> PathBuf {
    crate::paths::data_dir().join("offline")
}

/// Keterangan satu lagu tersimpan. Disimpan di `index.json` supaya daftarnya
/// bisa ditampilkan tanpa jaringan sama sekali.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Simpanan {
    pub id: String,
    pub judul: String,
    pub artis: String,
    pub detik: f64,
    pub sampul: String,
}

impl Simpanan {
    pub fn track(&self) -> crate::api::Track {
        crate::api::Track {
            id: self.id.clone(),
            title: self.judul.clone(),
            artist: self.artis.clone(),
            album: String::new(),
            thumb: self.sampul.clone(),
            dur: String::new(),
            secs: self.detik,
        }
    }
}

fn indeks() -> PathBuf {
    dir().join("index.json")
}

pub fn meta() -> Vec<Simpanan> {
    let Ok(teks) = std::fs::read_to_string(indeks()) else {
        return Vec::new();
    };
    let mut v: Vec<Simpanan> = serde_json::from_str(&teks).unwrap_or_default();
    // Buang entri yang berkasnya sudah tidak ada (mis. dihapus dari luar).
    v.retain(|m| ada(&m.id));
    v
}

fn tulis_meta(v: &[Simpanan]) {
    let d = dir();
    if std::fs::create_dir_all(&d).is_err() {
        return;
    }
    let tmp = d.join("index.json.part");
    if let Ok(teks) = serde_json::to_string(v) {
        if std::fs::write(&tmp, teks).is_ok() {
            let _ = std::fs::rename(&tmp, indeks());
        }
    }
}

fn catat(m: Simpanan) {
    let mut v = meta();
    v.retain(|x| x.id != m.id);
    v.insert(0, m);
    tulis_meta(&v);
}

fn lupakan(id: &str) {
    let mut v = meta();
    v.retain(|x| x.id != id);
    tulis_meta(&v);
}

pub fn berkas(id: &str) -> PathBuf {
    dir().join(format!("{}.m4a", aman(id)))
}

/// Id video hanya berisi huruf/angka/-/_, tapi tetap disaring: nama berkas ini
/// datang dari respons jaringan.
fn aman(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

pub fn ada(id: &str) -> bool {
    berkas(id).is_file()
}

pub fn baca(id: &str) -> Option<Vec<u8>> {
    std::fs::read(berkas(id)).ok()
}

pub fn ukuran(id: &str) -> u64 {
    std::fs::metadata(berkas(id)).map(|m| m.len()).unwrap_or(0)
}

/// Tulis ke berkas sementara lalu ganti nama: lagu setengah jadi tidak pernah
/// terlihat sebagai "sudah tersimpan".
pub fn simpan(id: &str, bytes: &[u8]) -> anyhow::Result<()> {
    let d = dir();
    std::fs::create_dir_all(&d)?;
    let tmp = d.join(format!("{}.part", aman(id)));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, berkas(id))?;
    Ok(())
}

pub fn hapus(id: &str) -> anyhow::Result<()> {
    let p = berkas(id);
    if p.is_file() {
        std::fs::remove_file(p)?;
    }
    lupakan(id);
    Ok(())
}

/// Simpan audio + keterangannya sekaligus. `detik` diambil dari info stream
/// (bukan dari teks durasi di kartu) karena itulah durasi yang dipakai pemutar.
pub fn simpan_lagu(t: &crate::api::Track, detik: f64, bytes: &[u8]) -> anyhow::Result<()> {
    simpan(&t.id, bytes)?;
    catat(Simpanan {
        id: t.id.clone(),
        judul: t.title.clone(),
        artis: t.artist.clone(),
        detik: if detik > 0.0 { detik } else { t.secs },
        sampul: t.thumb.clone(),
    });
    Ok(())
}

/// Id lagu yang sudah tersimpan, dibaca sekali saat aplikasi mulai.
pub fn daftar() -> Vec<String> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir()) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("m4a") {
            continue;
        }
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            out.push(stem.to_string());
        }
    }
    out
}

/// Total ukuran semua lagu tersimpan, untuk ditampilkan di pengaturan.
pub fn total_ukuran() -> u64 {
    daftar().iter().map(|id| ukuran(id)).sum()
}
