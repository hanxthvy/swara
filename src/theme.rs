//! Token warna dan ukuran bergaya Material 3 Expressive.
//!
//! Skema dibangun dari satu warna benih (rata-rata sampul lagu): hue benih
//! dipertahankan, lalu peran tonal diturunkan seperti `material-color-utilities`
//! menyusun `SchemeExpressive` — primer terang di tema gelap, kontainer tonal
//! untuk permukaan, dan outline tipis untuk pemisah.

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
    /// Kolom pencarian, tombol tonal, state layer.
    pub surface_highest: Color32,
    pub on_surface: Color32,
    pub on_surface_variant: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
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

pub fn scheme(dark: bool, seed: Option<[u8; 3]>) -> Scheme {
    let (h, s) = seed_hs(seed);
    if dark {
        Scheme {
            dark,
            primary: hsl(h, (s * 0.85).max(0.42), 0.78),
            on_primary: hsl(h, (s * 0.75).max(0.40), 0.16),
            primary_container: hsl(h, (s * 0.60).max(0.30), 0.34),
            on_primary_container: hsl(h, (s * 0.65).max(0.32), 0.93),
            secondary_container: hsl(h, s * 0.30, 0.26),
            on_secondary_container: hsl(h, s * 0.30, 0.92),
            surface: hsl(h, s * 0.12, 0.055),
            surface_low: hsl(h, s * 0.14, 0.085),
            surface_high: hsl(h, s * 0.16, 0.125),
            surface_highest: hsl(h, s * 0.18, 0.175),
            on_surface: hsl(h, 0.07, 0.94),
            on_surface_variant: hsl(h, 0.08, 0.72),
            outline: hsl(h, 0.07, 0.44),
            outline_variant: hsl(h, 0.07, 0.24),
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
            surface: hsl(h, s * 0.16, 0.975),
            surface_low: hsl(h, s * 0.14, 0.955),
            surface_high: hsl(h, s * 0.18, 0.925),
            surface_highest: hsl(h, s * 0.20, 0.885),
            on_surface: hsl(h, 0.22, 0.13),
            on_surface_variant: hsl(h, 0.14, 0.38),
            outline: hsl(h, 0.10, 0.52),
            outline_variant: hsl(h, 0.10, 0.78),
        }
    }
}

/// Skala tipe: display, headline, title, body, label.
pub mod type_scale {
    use egui::Color32;
    use egui::RichText;

    pub fn display(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(36.0).color(c).strong()
    }
    pub fn headline(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(27.0).color(c).strong()
    }
    pub fn title(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(21.0).color(c).strong()
    }
    pub fn title_m(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(15.5).color(c).strong()
    }
    pub fn body(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(14.0).color(c)
    }
    pub fn body_s(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(12.5).color(c)
    }
    pub fn label(text: impl Into<String>, c: Color32) -> RichText {
        RichText::new(text).size(12.0).color(c).strong()
    }
}

/// Sudut membulat khas Expressive: kartu besar, tombol penuh, sheet lebar.
pub mod shape {
    pub const CARD: f32 = 24.0;
    pub const SHEET: f32 = 30.0;
    pub const CHIP: f32 = 16.0;
    pub const FIELD: f32 = 26.0;
}
