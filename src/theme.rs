//! Token warna dan ukuran mengikuti kanvas `swara-ui.figcraft`.
//!
//! Bahasanya Spotify tanpa merek: permukaan hampir hitam, teks akromatik, dan
//! **putih satu-satunya aksen fungsional** — tombol putar berupa lingkaran putih
//! dengan glyph hitam. Warna lain hanya datang dari sampul album, jadi hue dari
//! benih tetap disimpan pada permukaan (sangat tipis) supaya panel tidak terasa
//! mati, sementara teks dan aksen tetap netral.

use crate::tokens::{tipe, ukuran, warna};
use egui::Color32;

#[derive(Clone, Copy)]
pub struct Scheme {
    pub dark: bool,
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,
    pub secondary_container: Color32,
    pub on_secondary_container: Color32,
    /// Latar paling belakang.
    pub surface: Color32,
    /// Kartu dan panel isi.
    pub surface_low: Color32,
    pub surface_high: Color32,
    /// Kolom pencarian, tombol pil, baris aktif.
    pub surface_highest: Color32,
    pub on_surface: Color32,
    pub on_surface_variant: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
    /// Garis kolom isian saat fokus (token `garis-isian`).
    pub outline_fokus: Color32,
}

/// Hue dan saturasi kasar dari warna benih.
pub fn seed_hs(seed: Option<[u8; 3]>) -> (f32, f32) {
    let (r, g, b) = match seed {
        Some(c) => (
            c[0] as f32 / 255.0,
            c[1] as f32 / 255.0,
            c[2] as f32 / 255.0,
        ),
        None => (0.60, 0.52, 0.74),
    };
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    let mut h = if d < 1e-4 {
        265.0
    } else if (max - r).abs() < 1e-6 {
        60.0 * (((g - b) / d) % 6.0)
    } else if (max - g).abs() < 1e-6 {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    if h < 0.0 {
        h += 360.0;
    }
    // Sampul nyaris hitam memberi hue acak; tarik ke netral.
    let s = if l < 0.14 || d < 0.06 { 0.40 } else { d };
    (h / 360.0, s.clamp(0.20, 0.80))
}

pub fn hsl(h: f32, s: f32, l: f32) -> Color32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h * 6.0).rem_euclid(6.0);
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(to(r1), to(g1), to(b1))
}

/// Campur `a` ke `b` sebanyak `t` (0..1).
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(
        f(a.r(), b.r()),
        f(a.g(), b.g()),
        f(a.b(), b.b()),
    )
}

pub fn scheme(dark: bool, seed: Option<[u8; 3]>) -> Scheme {
    let (h, s) = seed_hs(seed);
    if dark {
        // Netral dari kanvas, dengan sentuhan hue sampul 4% supaya sampul
        // berwarna tidak terasa menempel di atas abu-abu mati.
        let tint = |l: f32, sat: f32| hsl(h, s * sat, l);
        Scheme {
            dark,
            primary: warna::TEKS,
            on_primary: warna::LATAR,
            primary_container: warna::KARTU,
            on_primary_container: warna::TEKS,
            secondary_container: warna::ISIAN,
            on_secondary_container: warna::REDUP,
            surface: mix(warna::LATAR, tint(0.07, 0.05), 0.35),
            surface_low: mix(warna::PERMUKAAN, tint(0.09, 0.05), 0.35),
            surface_high: mix(warna::ISIAN, tint(0.11, 0.05), 0.30),
            surface_highest: mix(warna::KARTU, tint(0.14, 0.05), 0.30),
            on_surface: warna::TEKS,
            on_surface_variant: warna::REDUP,
            outline: warna::GARIS,
            outline_variant: warna::GARIS_ISIAN.gamma_multiply(0.55),
            outline_fokus: warna::GARIS_ISIAN,
        }
    } else {
        Scheme {
            dark,
            primary: hsl(h, (s * 0.90).max(0.42), 0.40),
            on_primary: hsl(h, 0.20, 0.99),
            primary_container: hsl(h, s * 0.42, 0.90),
            on_primary_container: hsl(h, s * 0.55, 0.16),
            secondary_container: hsl(h, s * 0.30, 0.90),
            on_secondary_container: hsl(h, s * 0.40, 0.18),
            surface: hsl(h, s * 0.10, 0.975),
            surface_low: hsl(h, s * 0.10, 0.955),
            surface_high: hsl(h, s * 0.12, 0.925),
            surface_highest: hsl(h, s * 0.14, 0.885),
            on_surface: hsl(h, 0.22, 0.13),
            on_surface_variant: hsl(h, 0.14, 0.38),
            outline: hsl(h, 0.10, 0.52),
            outline_variant: hsl(h, 0.10, 0.78),
            outline_fokus: hsl(h, 0.10, 0.44),
        }
    }
}

/// Skala tipe kanvas: judul halaman 24, seksi 18, hero 40, lagu 14,
/// isi 14, subteks 12–13.5, label kapital 10.5–11.5.
pub mod type_scale {
    use crate::tokens::tipe;
    use egui::Color32;
    use egui::RichText;

    /// Judul album besar di kepala koleksi.
    pub fn hero(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(tipe::JUDUL_ALBUM).color(c).strong()
    }
    /// Judul halaman.
    pub fn page(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(tipe::JUDUL_HALAMAN).color(c).strong()
    }
    /// Judul seksi rak.
    pub fn section(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(tipe::JUDUL_SEKSI).color(c).strong()
    }
    pub fn display(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(30.0).color(c).strong()
    }
    pub fn headline(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(tipe::JUDUL_HALAMAN).color(c).strong()
    }
    pub fn title(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(tipe::JUDUL_SEKSI).color(c).strong()
    }
    /// Judul lagu atau kartu.
    pub fn title_m(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(tipe::JUDUL_LAGU).color(c).strong()
    }
    pub fn body(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(tipe::ISI).color(c)
    }
    pub fn body_s(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(tipe::SUB).color(c)
    }
    /// Label kecil: kapital, tebal, dipakai untuk PUSTAKA dan label tombol.
    pub fn label(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text.into().to_uppercase())
            .size(tipe::LABEL)
            .color(c)
            .strong()
    }
    /// Angka waktu di ujung bilah durasi.
    pub fn time(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(tipe::WAKTU).color(c)
    }
}

/// Bahasa bentuk kanvas: pil untuk tombol, lingkaran untuk putar, 4–8 untuk
/// kartu dan sampul.
pub mod shape {
    use crate::tokens::ukuran;
    pub const CARD: f32 = ukuran::RADIUS_KARTU;
    /// Radius sampul kartu dan sampul baris.
    pub const RAD_CARD: f32 = ukuran::RADIUS_SAMPUL;
    pub const RAD_THUMB: f32 = ukuran::RADIUS_SAMPUL;
    pub const SHEET: f32 = ukuran::RADIUS_KARTU * 2.0;
    /// Pil: separuh tinggi tombolnya.
    pub const CHIP: f32 = 15.0;
    pub const FIELD: f32 = 22.0;
    /// Baris aktif di daftar lagu.
    pub const ROW: f32 = ukuran::RADIUS_SAMPUL;
}
