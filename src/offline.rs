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
    let s = sampul_berkas(id);
    if s.is_file() {
        let _ = std::fs::remove_file(s);
    }
    lupakan(id);
    Ok(())
}

// ------------------------------------------------------------ progres unduh

use std::collections::HashMap;
use std::sync::Mutex;

/// Kemajuan unduhan per id lagu, 0..1. Ditulis dari thread pekerja dan dibaca
/// tiap bingkai oleh antarmuka, jadi cukup satu peta bersama — tidak perlu
/// pesan tambahan di saluran `Out`.
static PROGRES: Mutex<Option<HashMap<String, f32>>> = Mutex::new(None);

fn kunci() -> std::sync::MutexGuard<'static, Option<HashMap<String, f32>>> {
    PROGRES.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn set_progres(id: &str, f: f32) {
    let mut g = kunci();
    g.get_or_insert_with(HashMap::new)
        .insert(id.to_string(), f.clamp(0.0, 1.0));
}

pub fn progres(id: &str) -> Option<f32> {
    kunci().as_ref().and_then(|m| m.get(id).copied())
}

pub fn hapus_progres(id: &str) {
    if let Some(m) = kunci().as_mut() {
        m.remove(id);
    }
}

// ------------------------------------------------------------ sampul

pub fn sampul_berkas(id: &str) -> PathBuf {
    dir().join(format!("{}.jpg", aman(id)))
}

/// Sampul ikut disimpan: tanpa ini daftar lagu offline tampil sebagai kotak
/// kosong, dan saat diputar tanpa jaringan tidak ada gambar sama sekali.
pub fn simpan_sampul(id: &str, bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    if std::fs::create_dir_all(dir()).is_err() {
        return false;
    }
    std::fs::write(sampul_berkas(id), bytes).is_ok()
}

pub fn baca_sampul(id: &str) -> Option<Vec<u8>> {
    std::fs::read(sampul_berkas(id)).ok()
}

/// Awalan `thumb` untuk sampul lokal: `lokal:<id>`. Antarmuka memakainya untuk
/// membedakan berkas di data aplikasi dari URL jaringan.
pub const AWALAN_LOKAL: &str = "lokal:";

/// Unduh aliran ke berkas sementara sambil melaporkan kemajuan, lalu ganti nama
/// dan catat metadatanya. `total` 0 berarti server tidak memberi panjang isi;
/// progresnya lalu hanya bisa ditaksir dari jumlah byte.
pub fn simpan_aliran(
    t: &crate::api::Track,
    detik: f64,
    total: u64,
    rd: &mut dyn std::io::Read,
    sampul: Option<&[u8]>,
    mut lapor: impl FnMut(f32),
) -> anyhow::Result<u64> {
    use std::io::Write;
    let d = dir();
    std::fs::create_dir_all(&d)?;
    let tmp = d.join(format!("{}.part", aman(&t.id)));
    let mut berkas_tulis = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
    let mut buf = vec![0u8; 64 * 1024];
    let mut sudah: u64 = 0;
    loop {
        let n = match rd.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(e.into());
            }
        };
        if let Err(e) = berkas_tulis.write_all(&buf[..n]) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        sudah += n as u64;
        if total > 0 {
            lapor((sudah as f32 / total as f32).min(0.999));
        }
    }
    berkas_tulis.flush()?;
    drop(berkas_tulis);
    if sudah == 0 {
        let _ = std::fs::remove_file(&tmp);
        anyhow::bail!("aliran kosong");
    }
    std::fs::rename(&tmp, berkas(&t.id))?;
    let mut url = t.thumb.clone();
    if let Some(b) = sampul {
        if simpan_sampul(&t.id, b) {
            url = format!("{AWALAN_LOKAL}{}", t.id);
        }
    }
    catat(Simpanan {
        id: t.id.clone(),
        judul: t.title.clone(),
        artis: t.artist.clone(),
        detik: if detik > 0.0 { detik } else { t.secs },
        sampul: url,
    });
    Ok(sudah)
}

/// Simpan audio + keterangannya sekaligus. `detik` diambil dari info stream
/// (bukan dari teks durasi di kartu) karena itulah durasi yang dipakai pemutar.
pub fn simpan_lagu(
    t: &crate::api::Track,
    detik: f64,
    bytes: &[u8],
    sampul: Option<&[u8]>,
) -> anyhow::Result<()> {
    simpan(&t.id, bytes)?;
    let mut url = t.thumb.clone();
    if let Some(b) = sampul {
        if simpan_sampul(&t.id, b) {
            url = format!("{AWALAN_LOKAL}{}", t.id);
        }
    }
    catat(Simpanan {
        id: t.id.clone(),
        judul: t.title.clone(),
        artis: t.artist.clone(),
        detik: if detik > 0.0 { detik } else { t.secs },
        sampul: url,
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
