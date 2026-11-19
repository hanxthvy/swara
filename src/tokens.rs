//! Token dari kanvas `swara-ui.figcraft`. Jangan diedit tangan —
//! jalankan `gen_tokens.py` lagi kalau kanvas berubah.
use egui::Color32;

pub mod warna {
    use egui::Color32;
    pub const LATAR: Color32 = Color32::from_rgb(0x12, 0x12, 0x12);
    pub const PERMUKAAN: Color32 = Color32::from_rgb(0x18, 0x18, 0x18);
    pub const ISIAN: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x1f);
    pub const KARTU: Color32 = Color32::from_rgb(0x25, 0x25, 0x25);
    pub const TEKS: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
    pub const REDUP: Color32 = Color32::from_rgb(0xb3, 0xb3, 0xb3);
    pub const GARIS: Color32 = Color32::from_rgb(0x4d, 0x4d, 0x4d);
    pub const GARIS_ISIAN: Color32 = Color32::from_rgb(0x7c, 0x7c, 0x7c);
}

pub mod ukuran {
    pub const SIDEBAR: f32 = 240.0;
    pub const SIDEBAR_SEMPIT: f32 = 88.0;
    pub const BILAH_ATAS: f32 = 64.0;
    pub const BILAH_PUTAR: f32 = 90.0;
    pub const PANEL_KANAN: f32 = 384.0;
    pub const KARTU_LUBANG: f32 = 76.0;
    pub const BARIS_LAGU: f32 = 56.0;
    pub const SAMPUL_BARIS: f32 = 40.0;
    pub const SAMPUL_MINI: f32 = 48.0;
    pub const SAMPUL_PEMUTAR: f32 = 390.0;
    pub const RADIUS_KARTU: f32 = 8.0;
    pub const RADIUS_SAMPUL: f32 = 6.0;
    pub const JARAK_SEKSI: f32 = 48.0;
    pub const JARAK_BARIS: f32 = 16.0;
    pub const KOLOM_GRID: f32 = 6.0;
}

pub mod tipe {
    pub const JUDUL_HALAMAN: f32 = 24.0;
    pub const JUDUL_SEKSI: f32 = 18.0;
    pub const JUDUL_ALBUM: f32 = 40.0;
    pub const JUDUL_LAGU: f32 = 14.0;
    pub const ISI: f32 = 14.0;
    pub const SUB: f32 = 12.0;
    pub const LABEL: f32 = 11.0;
    pub const KECIL: f32 = 10.5;
    pub const WAKTU: f32 = 11.0;
}

/// Palet dasar untuk tema gelap.
pub fn terang() -> bool { false }

/// Warna dari nama token; berguna untuk uji cepat di luar UI.
pub fn cari(nama: &str) -> Option<Color32> {
    if nama == "latar" { return Some(warna::LATAR); }
    if nama == "permukaan" { return Some(warna::PERMUKAAN); }
    if nama == "isian" { return Some(warna::ISIAN); }
    if nama == "kartu" { return Some(warna::KARTU); }
    if nama == "teks" { return Some(warna::TEKS); }
    if nama == "redup" { return Some(warna::REDUP); }
    if nama == "garis" { return Some(warna::GARIS); }
    if nama == "garis-isian" { return Some(warna::GARIS_ISIAN); }
    None
}
