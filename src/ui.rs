//! Antarmuka Swara — Material 3 Expressive di atas egui.
//!
//! Semua ukuran mengikuti lebar jendela: rail navigasi menyempit, grid kartu
//! menghitung sendiri jumlah kolomnya, dan pemutar berpindah dari panel kanan
//! ke mini player mengapung saat ruang tidak cukup.

use crate::api::{Card, Collection, Lyrics, Row, Stream, Track};
use crate::player::{Player, Progress};
use crate::mpris::Ctl as MprisCtl;
use crate::theme::{self, shape, type_scale as ts, Scheme};
use crate::worker::{Job, Out, Worker};
use egui::{
    pos2, vec2, Align, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Layout, Rect,
    Response, RichText, Rounding, ScrollArea, Sense, Shape, Stroke, TextureHandle, Ui,
};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type ThumbOut = (String, Option<(Vec<u8>, TextureHandle)>);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Cari,
    Koleksi,
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Home => "Beranda",
            Tab::Cari => "Cari",
            Tab::Koleksi => "Koleksi",
        }
    }
    fn icon(self) -> Icon {
        match self {
            Tab::Home => Icon::Home,
            Tab::Cari => Icon::Search,
            Tab::Koleksi => Icon::Library,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Feed,
    Collection,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Icon {
    Play,
    Pause,
    Next,
    Prev,
    Home,
    Search,
    Library,
    Lyrics,
    Volume,
    Theme,
    ChevronDown,
    ChevronUp,
    Queue,
    Like,
    Dislike,
    More,
    Shuffle,
    Repeat,
    Bell,
    Cast,
    Sample,
    Upgrade,
    Logo,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Btn {
    Filled,
    Tonal,
    Ghost,
}

// ------------------------------------------------------------------ ikon

fn draw_icon(p: &egui::Painter, icon: Icon, r: Rect, c: Color32) {
    let m = r.center();
    let s = r.width().min(r.height());
    match icon {
        Icon::Play => {
            let tri = vec![
                pos2(m.x - s * 0.26, m.y - s * 0.32),
                pos2(m.x - s * 0.26, m.y + s * 0.32),
                pos2(m.x + s * 0.34, m.y),
            ];
            p.add(Shape::convex_polygon(tri, c, Stroke::NONE));
        }
        Icon::Pause => {
            for dx in [-0.20, 0.06] {
                let bar = Rect::from_center_size(
                    pos2(m.x + s * dx, m.y),
                    vec2(s * 0.14, s * 0.62),
                );
                p.rect_filled(bar, Rounding::same(s * 0.06), c);
            }
        }
        Icon::Next | Icon::Prev => {
            // Cermin sempurna: segitiga dan palang sama-sama 0,60 s, jadi
            // tinggi kedua ikon selalu identik.
            let flip = if icon == Icon::Next { 1.0 } else { -1.0 };
            let tri = vec![
                pos2(m.x - s * 0.22 * flip, m.y - s * 0.30),
                pos2(m.x - s * 0.22 * flip, m.y + s * 0.30),
                pos2(m.x + s * 0.20 * flip, m.y),
            ];
            p.add(Shape::convex_polygon(tri, c, Stroke::NONE));
            let bar = Rect::from_center_size(
                pos2(m.x + s * 0.32 * flip, m.y),
                vec2(s * 0.10, s * 0.60),
            );
            p.rect_filled(bar, Rounding::same(s * 0.03), c);
        }
        Icon::Home => {
            let roof = vec![
                pos2(m.x, m.y - s * 0.34),
                pos2(m.x - s * 0.36, m.y - s * 0.02),
                pos2(m.x + s * 0.36, m.y - s * 0.02),
            ];
            p.add(Shape::convex_polygon(roof, c, Stroke::NONE));
            p.rect_filled(
                Rect::from_center_size(pos2(m.x, m.y + s * 0.16), vec2(s * 0.50, s * 0.34)),
                Rounding::same(s * 0.07),
                c,
            );
        }
        Icon::Search => {
            p.circle_stroke(
                pos2(m.x - s * 0.06, m.y - s * 0.06),
                s * 0.26,
                Stroke::new(s * 0.11, c),
            );
            p.line_segment(
                [
                    pos2(m.x + s * 0.16, m.y + s * 0.16),
                    pos2(m.x + s * 0.34, m.y + s * 0.34),
                ],
                Stroke::new(s * 0.11, c),
            );
        }
        Icon::Library => {
            for (dx, h) in [(-0.24, 0.56), (0.0, 0.44), (0.24, 0.62)] {
                let bar = Rect::from_center_size(
                    pos2(m.x + s * dx, m.y),
                    vec2(s * 0.12, s * h),
                );
                p.rect_filled(bar, Rounding::same(s * 0.06), c);
            }
        }
        Icon::Lyrics => {
            for (i, w) in [0.52f32, 0.40, 0.46].iter().enumerate() {
                let y = m.y + s * (i as f32 - 1.0) * 0.20;
                let bar = Rect::from_center_size(pos2(m.x, y), vec2(s * w, s * 0.08));
                p.rect_filled(bar, Rounding::same(s * 0.04), c);
            }
        }
        Icon::Volume => {
            let tri = vec![
                pos2(m.x - s * 0.30, m.y - s * 0.12),
                pos2(m.x - s * 0.30, m.y + s * 0.12),
                pos2(m.x - s * 0.08, m.y + s * 0.26),
                pos2(m.x - s * 0.08, m.y - s * 0.26),
            ];
            p.add(Shape::convex_polygon(tri, c, Stroke::NONE));
            p.circle_stroke(
                pos2(m.x - s * 0.02, m.y),
                s * 0.22,
                Stroke::new(s * 0.07, c),
            );
        }
        Icon::ChevronDown => {
            let p1 = pos2(m.x - s * 0.22, m.y - s * 0.08);
            let p2 = pos2(m.x, m.y + s * 0.16);
            let p3 = pos2(m.x + s * 0.22, m.y - s * 0.08);
            p.line_segment([p1, p2], Stroke::new(s * 0.11, c));
            p.line_segment([p2, p3], Stroke::new(s * 0.11, c));
        }
        Icon::ChevronUp => {
            let p1 = pos2(m.x - s * 0.22, m.y + s * 0.08);
            let p2 = pos2(m.x, m.y - s * 0.16);
            let p3 = pos2(m.x + s * 0.22, m.y + s * 0.08);
            p.line_segment([p1, p2], Stroke::new(s * 0.11, c));
            p.line_segment([p2, p3], Stroke::new(s * 0.11, c));
        }
        Icon::Queue => {
            for (i, w) in [0.54f32, 0.54, 0.36].iter().enumerate() {
                let y = m.y + s * (i as f32 - 1.0) * 0.22;
                let bar = Rect::from_center_size(pos2(m.x + s * 0.03, y), vec2(s * w, s * 0.09));
                p.rect_filled(bar, Rounding::same(s * 0.045), c);
            }
            for i in 0..3 {
                let y = m.y + s * (i as f32 - 1.0) * 0.22;
                p.circle_filled(pos2(m.x - s * 0.32, y), s * 0.055, c);
            }
        }
        Icon::Like => {
            let up = vec![
                pos2(m.x - s * 0.34, m.y + s * 0.34),
                pos2(m.x - s * 0.34, m.y - s * 0.02),
                pos2(m.x - s * 0.10, m.y - s * 0.02),
                pos2(m.x + s * 0.02, m.y - s * 0.36),
                pos2(m.x + s * 0.16, m.y - s * 0.30),
                pos2(m.x + s * 0.10, m.y - s * 0.02),
                pos2(m.x + s * 0.34, m.y - s * 0.02),
                pos2(m.x + s * 0.34, m.y + s * 0.34),
            ];
            p.add(Shape::convex_polygon(up, c, Stroke::NONE));
        }
        Icon::Dislike => {
            let down = vec![
                pos2(m.x - s * 0.34, m.y - s * 0.34),
                pos2(m.x - s * 0.34, m.y + s * 0.02),
                pos2(m.x - s * 0.10, m.y + s * 0.02),
                pos2(m.x + s * 0.02, m.y + s * 0.36),
                pos2(m.x + s * 0.16, m.y + s * 0.30),
                pos2(m.x + s * 0.10, m.y + s * 0.02),
                pos2(m.x + s * 0.34, m.y + s * 0.02),
                pos2(m.x + s * 0.34, m.y - s * 0.34),
            ];
            p.add(Shape::convex_polygon(down, c, Stroke::NONE));
        }
        Icon::More => {
            for dx in [-0.24, 0.0, 0.24] {
                p.circle_filled(pos2(m.x + s * dx, m.y), s * 0.07, c);
            }
        }
        Icon::Shuffle => {
            let mut a = pos2(m.x - s * 0.34, m.y - s * 0.22);
            let mut b = pos2(m.x + s * 0.30, m.y + s * 0.22);
            p.line_segment([a, b], Stroke::new(s * 0.09, c));
            let tri = vec![
                pos2(b.x - s * 0.14, b.y - s * 0.02),
                pos2(b.x + s * 0.02, b.y + s * 0.02),
                pos2(b.x - s * 0.02, b.y - s * 0.16),
            ];
            p.add(Shape::convex_polygon(tri, c, Stroke::NONE));
            a = pos2(m.x - s * 0.34, m.y + s * 0.22);
            b = pos2(m.x + s * 0.30, m.y - s * 0.22);
            p.line_segment([a, b], Stroke::new(s * 0.09, c));
            let tri2 = vec![
                pos2(b.x - s * 0.14, b.y + s * 0.02),
                pos2(b.x + s * 0.02, b.y - s * 0.02),
                pos2(b.x - s * 0.02, b.y + s * 0.16),
            ];
            p.add(Shape::convex_polygon(tri2, c, Stroke::NONE));
        }
        Icon::Repeat => {
            let r = s * 0.24;
            for (a0, a1) in [(0.55f32, 2.15f32), (3.70, 5.30)] {
                let n = 14;
                let pts: Vec<egui::Pos2> = (0..=n)
                    .map(|i| {
                        let a = a0 + (a1 - a0) * (i as f32 / n as f32);
                        pos2(m.x + a.cos() * r, m.y + a.sin() * r)
                    })
                    .collect();
                p.add(Shape::line(pts, Stroke::new(s * 0.09, c)));
            }
            let tip = pos2(m.x + 0.55f32.cos() * r, m.y + 0.55f32.sin() * r);
            let tri = vec![
                pos2(tip.x - s * 0.04, tip.y - s * 0.10),
                pos2(tip.x + s * 0.10, tip.y + s * 0.02),
                pos2(tip.x - s * 0.10, tip.y + s * 0.02),
            ];
            p.add(Shape::convex_polygon(tri, c, Stroke::NONE));
        }
        Icon::Bell => {
            let pts: Vec<egui::Pos2> = vec![
                pos2(m.x - s * 0.28, m.y + s * 0.20),
                pos2(m.x - s * 0.24, m.y - s * 0.02),
                pos2(m.x - s * 0.18, m.y - s * 0.20),
                pos2(m.x, m.y - s * 0.28),
                pos2(m.x + s * 0.18, m.y - s * 0.20),
                pos2(m.x + s * 0.24, m.y - s * 0.02),
                pos2(m.x + s * 0.28, m.y + s * 0.20),
            ];
            p.add(Shape::line(pts, Stroke::new(s * 0.09, c)));
            p.line_segment(
                [
                    pos2(m.x - s * 0.28, m.y + s * 0.20),
                    pos2(m.x + s * 0.28, m.y + s * 0.20),
                ],
                Stroke::new(s * 0.09, c),
            );
            p.circle_filled(pos2(m.x, m.y + s * 0.30), s * 0.06, c);
        }
        Icon::Cast => {
            let r = Rect::from_center_size(
                pos2(m.x + s * 0.04, m.y - s * 0.02),
                vec2(s * 0.52, s * 0.40),
            );
            p.rect_stroke(r, Rounding::same(s * 0.06), Stroke::new(s * 0.08, c));
            p.circle_filled(pos2(m.x - s * 0.28, m.y + s * 0.22), s * 0.045, c);
            p.add(Shape::line(
                vec![
                    pos2(m.x - s * 0.28, m.y + s * 0.36),
                    pos2(m.x - s * 0.06, m.y + s * 0.36),
                ],
                Stroke::new(s * 0.07, c),
            ));
        }
        Icon::Sample => {
            // Dua segitiga ke kanan ("fast forward"), ikon tab Sampel di
            // YouTube Music -- bukan grafik batang seperti versi pertama.
            for dx in [-0.36f32, 0.01] {
                let tri = vec![
                    pos2(m.x + s * dx, m.y - s * 0.26),
                    pos2(m.x + s * dx, m.y + s * 0.26),
                    pos2(m.x + s * (dx + 0.31), m.y),
                ];
                p.add(Shape::convex_polygon(tri, c, Stroke::NONE));
            }
        }
        Icon::Upgrade => {
            // Cincin dengan segitiga play di tengah, seperti logo YouTube
            // Music yang dipakai tab Upgrade.
            p.circle_stroke(m, s * 0.42, Stroke::new(s * 0.09, c));
            let tri = vec![
                pos2(m.x - s * 0.10, m.y - s * 0.17),
                pos2(m.x - s * 0.10, m.y + s * 0.17),
                pos2(m.x + s * 0.19, m.y),
            ];
            p.add(Shape::convex_polygon(tri, c, Stroke::NONE));
        }
        Icon::Logo => {
            p.circle_filled(m, s * 0.48, Color32::from_rgb(255, 0, 0));
            draw_icon(p, Icon::Play, Rect::from_center_size(m, vec2(s * 0.46, s * 0.46)), Color32::WHITE);
        }
        Icon::Theme => {
            p.circle_filled(m, s * 0.26, c);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::PI / 4.0;
                p.line_segment(
                    [
                        pos2(m.x + a.cos() * s * 0.33, m.y + a.sin() * s * 0.33),
                        pos2(m.x + a.cos() * s * 0.46, m.y + a.sin() * s * 0.46),
                    ],
                    Stroke::new(s * 0.08, c),
                );
            }
        }
    }
}

/// Ikon yang punya dua keadaan. Suka/tidak suka digambar garis saat mati dan
/// penuh saat hidup; acak/ulangi memakai warna aksen plus titik kecil di bawah,
/// supaya keadaan tombolnya kelihatan tanpa harus ditekan dulu.
fn draw_ikon(p: &egui::Painter, icon: Icon, r: Rect, c: Color32, aktif: bool) {
    match icon {
        Icon::Like | Icon::Dislike => {
            let m = r.center();
            let s = r.width().min(r.height());
            let d = if icon == Icon::Like { -1.0 } else { 1.0 };
            let pts = vec![
                pos2(m.x - s * 0.34, m.y - s * 0.34 * d),
                pos2(m.x - s * 0.34, m.y + s * 0.02 * d),
                pos2(m.x - s * 0.10, m.y + s * 0.02 * d),
                pos2(m.x + s * 0.02, m.y + s * 0.36 * d),
                pos2(m.x + s * 0.16, m.y + s * 0.30 * d),
                pos2(m.x + s * 0.10, m.y + s * 0.02 * d),
                pos2(m.x + s * 0.34, m.y + s * 0.02 * d),
                pos2(m.x + s * 0.34, m.y - s * 0.34 * d),
            ];
            if aktif {
                p.add(Shape::convex_polygon(pts.clone(), c, Stroke::NONE));
            } else {
                p.add(Shape::closed_line(pts, Stroke::new(s * 0.08, c)));
            }
        }
        Icon::Shuffle | Icon::Repeat => {
            draw_icon(p, icon, r, c);
            if aktif {
                p.circle_filled(pos2(r.center().x, r.bottom() + 2.0), 2.0, c);
            }
        }
        _ => draw_icon(p, icon, r, c),
    }
}

// ------------------------------------------------------------------ app

pub struct App {
    api: Arc<crate::api::Api>,
    player: Player,
    worker: Worker,

    tab: Tab,
    view: View,
    home: Vec<Row>,
    home_err: String,
    koleksi: Option<Collection>,
    lib: Vec<Card>,
    logged: Option<String>,

    query: String,
    results: Vec<Track>,
    suggest: Vec<String>,
    show_suggest: bool,
    suggest_at: Option<Instant>,
    /// Pencarian otomatis setelah berhenti mengetik. Di Android tombol Enter
    /// dari papan ketik tidak sampai ke egui, jadi tanpa ini kolom cari
    /// tidak pernah mengirim pencarian.
    cari_at: Option<Instant>,
    searched: String,
    focus_cari: bool,

    lyrics: Option<Lyrics>,
    lyrics_id: String,
    /// Kapan permintaan lirik terakhir dikirim; dipakai jaring pengaman di
    /// `lyrics_view` supaya panel tidak berhenti di "memuat lirik…".
    lyrics_diminta: Option<std::time::Instant>,
    show_lyrics: bool,
    /// Keadaan yang harus terbaca dari ikonnya: acak, ulangi, dan nilai
    /// suka/tidak suka per lagu (1 suka, -1 tidak, 0 belum).
    shuffle_on: bool,
    ulangi: u8,
    rating: std::collections::HashMap<String, i8>,
    /// Keadaan suka lagu yang sedang main, dibagi dengan pengendali luar
    /// (soket `swara --ctl`), supaya ikon di jendela dan di pill islet sama.
    suka: crate::ctl::Bersama,
    show_queue: bool,
    /// Tinggi bilah status Android dalam titik; dipakai layar ponsel supaya
    /// judul tidak tertutup jam dan notifikasi.
    inset_atas: f32,
    /// Tinggi area gestur Android di bawah, supaya tab paling bawah tidak
    /// tertimpa bilah navigasi.
    inset_bawah: f32,
    /// Berapa kali inset dibaca; beberapa frame pertama belum stabil, jadi
    /// nilai terkecil yang masuk akal yang dipakai.
    inset_coba: u8,
    /// Kolom cari sedang dipegang kursor ketik; pintasan huruf dimatikan.
    search_focus: bool,

    queue: Vec<Track>,
    current: Option<Track>,
    current_info: Option<Stream>,
    loading: bool,
    progress: Progress,
    drag_pos: Option<f64>,

    thumbs: HashMap<String, Option<TextureHandle>>,
    thumb_pending: HashSet<String>,
    thumb_rx: Receiver<ThumbOut>,
    thumb_tx: Sender<ThumbOut>,

    scheme: Scheme,
    seed: Option<[u8; 3]>,
    toast: Option<(String, Instant)>,
    last_error: Option<String>,
    autoplay: Option<String>,
    /// Kendali MPRIS (tulis saja). Perintah dari panel sistem ditangani
    /// `bridge.rs`, bukan di sini — saat jendela di workspace lain `update`
    /// tidak dipanggil sama sekali.
    mpris: Option<MprisCtl>,
    expanded: bool,
    debug_seek: Option<f64>,
    debug_next: Option<(f64, std::time::Instant)>,
    cond: Arc<Mutex<crate::conductor::Cond>>,
    cond_seq: u64,
    /// Berkas di kotak unggah (nama, ukuran) di layar ponsel.
    unggahan: Vec<(String, f64)>,
    unggah_info: Option<String>,
    tab_warna: Vec<Color32>,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        api: Arc<crate::api::Api>,
        player: Player,
        worker: Worker,
        autoplay: Option<String>,
        play_id: Option<String>,
        play_list: Option<String>,
        mpris: Option<MprisCtl>,
        cond: Arc<Mutex<crate::conductor::Cond>>,
        suka: crate::ctl::Bersama,
    ) -> Self {
        install_fonts(&cc.egui_ctx);
        worker.attach(cc.egui_ctx.clone());
        let (thumb_tx, thumb_rx) = channel();
        let mut app = App {
            api,
            player,
            cond,
            worker,
            tab: Tab::Home,
            view: View::Feed,
            home: Vec::new(),
            home_err: String::new(),
            koleksi: None,
            lib: Vec::new(),
            logged: None,
            query: String::new(),
            results: Vec::new(),
            suggest: Vec::new(),
            show_suggest: false,
            suggest_at: None,
            cari_at: None,
            searched: String::new(),
            focus_cari: false,
            lyrics: None,
            lyrics_id: String::new(),
            lyrics_diminta: None,
            // Uji manual: SWARA_LIRIK_TAB=1 membuka panel lirik saat mulai.
            show_lyrics: std::env::var("SWARA_LIRIK_TAB").is_ok(),
            shuffle_on: false,
            ulangi: 0,
            rating: std::collections::HashMap::new(),
            suka,
            show_queue: false,
            inset_atas: 0.0,
            inset_bawah: 0.0,
            inset_coba: 0,
            search_focus: false,
            queue: Vec::new(),
            current: None,
            current_info: None,
            loading: false,
            progress: Progress {
                volume: 0.8,
                ..Default::default()
            },
            drag_pos: None,
            thumbs: HashMap::new(),
            thumb_pending: HashSet::new(),
            thumb_rx,
            thumb_tx,
            scheme: theme::scheme(true, None),
            seed: None,
            toast: None,
            last_error: None,
            autoplay,
            mpris,
            expanded: std::env::var("SWARA_EXPAND")
                .map(|v| !v.is_empty() && v != "0")
                .unwrap_or(false),
            cond_seq: 0,
            unggahan: Vec::new(),
            unggah_info: None,
            tab_warna: vec![
                Color32::from_rgb(140, 120, 220),
                Color32::from_rgb(220, 140, 120),
                Color32::from_rgb(120, 180, 140),
                Color32::from_rgb(200, 170, 100),
            ],
    debug_next: std::env::var("SWARA_NEXT")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .map(|d| (d, std::time::Instant::now())),
    debug_seek: std::env::var("SWARA_SEEK")
        .ok()
        .and_then(|v| v.parse::<f64>().ok()),
        };
        app.logged = if app.api.logged_in() {
            app.api.session_label()
        } else {
            None
        };
        app.worker.send(Job::Home);
        if let Some(id) = play_list {
            // Playlist akun jadi antrean penuh; lagu pertama langsung jalan.
            app.worker.send(Job::PlayList(id));
        } else if let Some(id) = play_id {
            // Lagu tertentu: judulnya menyusul dari `Out::Loaded` (Stream),
            // jadi `track` di sini cukup id-nya.
            app.play_track(
                Track {
                    id,
                    ..Default::default()
                },
                None,
            );
        }
        if let Some(q) = app.autoplay.clone() {
            app.tab = Tab::Cari;
            app.query = q.clone();
            app.searched = q.clone();
            app.worker.send(Job::Search(q));
        }
        app
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    fn apply_out(&mut self, out: Out) {
        match out {
            Out::Home(rows) => {
                if rows.is_empty() {
                    self.home_err = "beranda kosong, periksa koneksi".into();
                }
                self.home = rows;
            }
            Out::Search(q, t) => {
                if q == self.query {
                    self.results = t;
                    self.show_suggest = false;
                    if self.autoplay.is_some() {
                        self.autoplay = None;
                        if let Some(first) = self.results.first().cloned() {
                            let queue = self.results.clone();
                            self.play_track(first, Some(queue));
                        }
                    }
                }
            }
            Out::Suggest(q, s) => {
                if q == self.query {
                    self.suggest = s;
                    self.show_suggest = true;
                }
            }
            Out::Browse(c) => self.koleksi = Some(c),
            Out::PlayList(t) => self.play_queue(t),
            Out::Logged(l) => {
                self.logged = l;
                if let Some(name) = self.logged.clone() {
                    self.toast(format!("tersambung: {name}"));
                } else {
                    self.toast("sesi akun dilepas");
                    self.lib.clear();
                }
            }
            Out::Library(cards) => {
                if cards.is_empty() {
                    self.toast("belum ada playlist di akun ini");
                }
                self.lib = cards;
            }
            Out::Rated(id, like) => {
                let _ = id;
                self.toast(if like {
                    "ditambahkan ke lagu disukai"
                } else {
                    "dihapus dari lagu disukai"
                });
            }
            Out::Radio(id, t) => {
                if self.current.as_ref().map(|c| c.id == id).unwrap_or(false) {
                    let have: HashSet<String> = self.queue.iter().map(|x| x.id.clone()).collect();
                    for item in t {
                        if !have.contains(&item.id) {
                            self.queue.push(item);
                        }
                    }
                    let cur_id = self.current.as_ref().map(|c| c.id.clone()).unwrap_or_default();
                    self.cond.lock().unwrap().set_queue(&self.queue, &cur_id);
                }
            }
            Out::Lyrics(id, l) => {
                eprintln!(
                    "[swara] lirik {} (diminta untuk {}): {} baris{}",
                    id,
                    self.lyrics_id,
                    l.synced.len(),
                    if id == self.lyrics_id { "" } else { " -- BASI, dibuang" }
                );
                if id == self.lyrics_id {
                    self.lyrics = Some(l);
                }
            }
            Out::Loaded(track, info, audio, art) => {
                self.loading = false;
                eprintln!("[swara] loaded {}", track.id);
                if self.current.as_ref().map(|c| c.id != track.id).unwrap_or(true) {
                    return;
                }
                let dur = if info.dur > 0.0 { info.dur } else { track.secs };
                self.current_info = Some(info);
                // Judul/artis dari `Stream` mengisi kekosongan pada track yang
                // datang tanpa metadata (mis. diputar lewat `SWARA_PLAY`).
                if let Some(c) = self.current.as_mut() {
                    if c.title.is_empty() {
                        c.title = self.current_info.as_ref().map(|i| i.title.clone()).unwrap_or_default();
                    }
                    if c.artist.is_empty() {
                        c.artist = self.current_info.as_ref().map(|i| i.artist.clone()).unwrap_or_default();
                    }
                    if c.thumb.is_empty() {
                        c.thumb = self.current_info.as_ref().map(|i| i.thumb.clone()).unwrap_or_default();
                    }
                    if c.secs <= 0.0 {
                        c.secs = dur;
                    }
                }
                self.player.play(audio, 0.0, dur);
                self.progress.dur = dur;
                self.progress.ended = false;
                if self.seed.is_none() {
                    if let Some(bytes) = art {
                        if let Ok(img) = image::load_from_memory(&bytes) {
                            self.set_seed(&img);
                        }
                    }
                }
                if let Some(m) = &self.mpris {
                    if let Some(cur) = &self.current {
                        m.track(cur, dur, &cur.thumb.clone());
                        m.status(true);
                        m.position(0.0);
                    }
                }
                self.minta_lirik(track);
            }
            Out::Failed(e) => {
                eprintln!("[swara] {e}");
                self.loading = false;
                self.last_error = Some(e.clone());
                self.toast(format!("gagal: {e}"));
            }
        }
    }

    /// Ambil alih lagu yang dipilih konduktor. Ini terjadi kalau lagu berganti
    /// saat jendela tersembunyi — `update` baru jalan lagi ketika jendela
    /// tampil, lalu menyusul keadaan yang sudah berjalan.
    fn adopt(&mut self, t: Track, s: Option<crate::api::Stream>) {
        self.current = Some(t.clone());
        self.loading = false;
        self.seed = None;
        self.drag_pos = None;
        self.lyrics = None;
        self.lyrics_id = t.id.clone();
        if let Some(info) = s {
            let dur = if info.dur > 0.0 { info.dur } else { t.secs };
            self.progress.dur = dur;
            self.current_info = Some(info.clone());
            if let Some(m) = &self.mpris {
                m.track(&t, dur, &info.thumb);
                m.status(true);
                m.position(0.0);
            }
        }
        if !self.queue.iter().any(|x| x.id == t.id) {
            self.queue.push(t.clone());
        }
        self.minta_lirik(t);
    }

    /// Pasang antrean penuh lalu mulai dari lagu pertama. Dipakai playlist
    /// akun (`SWARA_LIST`) supaya yang diputar bukan cuma satu lagu + radio.
    fn play_queue(&mut self, tracks: Vec<Track>) {
        if tracks.is_empty() {
            return;
        }
        eprintln!("[swara] antrean {} lagu", tracks.len());
        self.queue = tracks;
        let first = self.queue[0].clone();
        self.play_track(first, None);
    }

    fn play_track(&mut self, track: Track, queue: Option<Vec<Track>>) {
        eprintln!("[swara] play {}", track.id);
        if let Some(q) = queue {
            self.queue = q;
        }
        self.current = Some(track.clone());
        self.current_info = None;
        self.lyrics = None;
        self.lyrics_id = track.id.clone();
        self.loading = true;
        self.seed = None;
        self.drag_pos = None;
        // Audio lama harus berhenti sekarang, kalau tidak bar durasi masih
        // memutar posisi lagu sebelumnya selama lagu baru dimuat.
        self.player.stop();
        self.progress = Progress {
            volume: self.progress.volume,
            ..Default::default()
        };
        self.worker.send(Job::Load(track.clone()));
        if !self.queue.iter().any(|t| t.id == track.id) {
            self.queue.insert(0, track.clone());
        }
        self.cond.lock().unwrap().set_queue(&self.queue, &track.id);
        // Siapkan lagu berikutnya di latar supaya pindah lagu tidak menunggu
        // jaringan.
        if let Some(n) = self
            .queue
            .iter()
            .position(|t| t.id == track.id)
            .and_then(|i| self.queue.get(i + 1))
            .cloned()
        {
            self.api.prefetch(&n);
        }
        self.worker.send(Job::Radio(track.id.clone()));
    }

    fn next_track(&mut self, step: isize) {
        let Some(cur) = self.current.clone() else {
            return;
        };
        let len = self.queue.len() as isize;
        if len == 0 {
            return;
        }
        if step > 0 && self.ulangi == 2 {
            // ulangi satu lagu: tombol next tetap pindah, tapi pemutaran ulang
            // otomatis di ujung lagu juga memakai jalur ini.
            if self.progress.pos > 0.5 && self.progress.dur > 0.0 {
                self.player.seek(0.0);
                self.player.resume();
                return;
            }
        }
        let idx = self.queue.iter().position(|t| t.id == cur.id);
        let next = match idx {
            Some(i) => {
                let j = i as isize + step;
                if step > 0 && self.ulangi == 0 && j >= len {
                    self.player.pause();
                    self.toast("antrean habis");
                    return;
                }
                j.rem_euclid(len) as usize
            }
            None => 0,
        };
        if let Some(t) = self.queue.get(next).cloned() {
            if t.id != cur.id {
                self.play_track(t, None);
            }
        }
    }

    /// Minta lirik sekali untuk satu lagu.
    ///
    /// Tanpa penjaga ini, satu pergantian lagu bisa mengirim beberapa
    /// permintaan sekaligus (dari `Loaded`, dari konduktor, dan dari panel
    /// lirik). Worker memproses permintaan satu per satu lewat jaringan, jadi
    /// antrean ganda membuat panel berhenti di "memuat lirik…" puluhan detik
    /// sampai semua permintaan lama selesai.
    fn minta_lirik(&mut self, t: Track) {
        let masih_jalan = self.lyrics_id == t.id
            && self
                .lyrics_diminta
                .map(|x| x.elapsed() < std::time::Duration::from_secs(4))
                .unwrap_or(false);
        if masih_jalan {
            return;
        }
        self.lyrics_id = t.id.clone();
        self.lyrics_diminta = Some(std::time::Instant::now());
        self.worker.send(Job::Lyrics(t));
    }

    /// Nyatakan suka (+1) atau tidak suka (-1) untuk lagu yang sedang main.
    /// Ditekan lagi pada pilihan yang sama berarti membatalkannya.
    fn nilai_lagu(&mut self, nilai: i8) {
        let Some(id) = self.current.as_ref().map(|t| t.id.clone()) else {
            self.toast("belum ada lagu");
            return;
        };
        if self.logged.is_none() {
            self.toast("masuk dulu lewat sesi Chromium");
            return;
        }
        let lama = self.rating.get(&id).copied().unwrap_or(0);
        let baru = if lama == nilai { 0 } else { nilai };
        if baru == 0 {
            self.rating.remove(&id);
        } else {
            self.rating.insert(id.clone(), baru);
        }
        {
            let mut g = self.suka.lock().unwrap();
            g.id = id.clone();
            g.suka = baru == 1;
            drop(g);
            crate::ctl::tulis(&self.suka);
        }
        self.worker.send(Job::Rate(id, baru == 1));
        self.toast(match baru {
            1 => "disukai",
            -1 => "tidak disukai",
            _ => "penilaian dibatalkan",
        });
    }

    fn toggle_play(&mut self) {
        if self.progress.playing {
            self.player.pause();
        } else if self.progress.dur > 0.0 {
            self.player.resume();
        } else if let Some(t) = self.current.clone().or_else(|| self.queue.first().cloned()) {
            self.play_track(t, None);
        }
    }

    fn set_seed(&mut self, img: &image::DynamicImage) {
        if let Some(seed) = cover_seed(img) {
            self.seed = Some(seed);
            self.scheme = theme::scheme(self.scheme.dark, self.seed);
        }
    }

    fn ensure_thumb(&mut self, ctx: &egui::Context, url: &str) {
        if url.is_empty() || self.thumbs.contains_key(url) || self.thumb_pending.contains(url) {
            return;
        }
        self.thumb_pending.insert(url.to_string());
        let api = self.api.clone();
        let tx = self.thumb_tx.clone();
        let c = ctx.clone();
        let u = url.to_string();
        std::thread::spawn(move || {
            let got = api.fetch_image(&u).ok().and_then(|b| {
                image::load_from_memory(&b).ok().map(|img| {
                    let tex = load_texture(&c, &format!("t{}", hash_name(&u)), &img);
                    (b, tex)
                })
            });
            let _ = tx.send((u, got));
            c.request_repaint();
        });
    }

    fn thumb_of(&self, url: &str) -> Option<TextureHandle> {
        self.thumbs.get(url).and_then(|x| x.clone())
    }

    fn cover_of(&self) -> Option<TextureHandle> {
        let t = self.current.as_ref()?;
        self.thumb_of(&t.thumb)
    }
}

// ------------------------------------------------------------------ komponen

impl App {
    fn layer(&self, ui: &Ui, rect: Rect, radius: f32, resp: &Response, alpha: f32) -> f32 {
        let t = ui
            .ctx()
            .animate_bool_with_time(resp.id.with("layer"), resp.hovered(), 0.12);
        if t > 0.001 {
            ui.painter().rect_filled(
                rect,
                Rounding::same(radius),
                self.scheme.on_surface.gamma_multiply(alpha * t),
            );
        }
        t
    }

    fn chip(&self, ui: &mut Ui, label: &str, selected: bool) -> Response {
        let s = self.scheme;
        let (bg, fg) = if selected {
            (s.on_surface, s.surface)
        } else {
            (s.surface_high, s.on_surface)
        };
        ui.add(
            egui::Button::new(ts::title_m(label, fg).size(13.0))
                .fill(bg)
                .stroke(Stroke::NONE)
                .rounding(Rounding::same(16.0))
                .min_size(vec2(0.0, 32.0)),
        )
    }

    /// Pil aksi berikon: dipakai baris suka/lirik/antrean di pemutar ponsel.
    fn chip_ikon(&self, ui: &mut Ui, icon: Icon, label: &str, aktif: bool) -> bool {
        let s = self.scheme;
        let teks_w = label.chars().count() as f32 * 7.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(20.0 + 8.0 + teks_w + 14.0, 34.0), Sense::click());
        ui.painter().rect_filled(
            rect,
            Rounding::same(17.0),
            if aktif { s.primary_container } else { s.surface_high },
        );
        let fg = if aktif { s.on_primary_container } else { s.on_surface };
        draw_ikon(
            ui.painter(),
            icon,
            Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(17.0, 17.0)),
            fg,
            aktif,
        );
        ui.painter().text(
            pos2(rect.left() + 34.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.5),
            fg,
        );
        resp.clicked()
    }

    fn icon_button(&self, ui: &mut Ui, icon: Icon, size: f32, style: Btn) -> Response {
        let s = self.scheme;
        let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
        let (bg, fg) = match style {
            Btn::Filled => (s.primary, s.on_primary),
            Btn::Tonal => (s.secondary_container, s.on_secondary_container),
            Btn::Ghost => (Color32::TRANSPARENT, s.on_surface),
        };
        if style == Btn::Ghost {
            self.layer(ui, rect, size / 2.0, &resp, 0.09);
        } else {
            let t = ui
                .ctx()
                .animate_bool_with_time(resp.id.with("hov"), resp.hovered(), 0.10);
            let fill = if t > 0.0 {
                bg.lerp_to_gamma(fg, 0.10 * t)
            } else {
                bg
            };
            ui.painter()
                .circle_filled(rect.center(), size / 2.0, fill);
        }
        let icon_rect = Rect::from_center_size(rect.center(), vec2(size * 0.44, size * 0.44));
        draw_icon(ui.painter(), icon, icon_rect, fg);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp
    }

    /// Seek bar Material: garis 4 px, kenop muncul saat disentuh.
    fn seek_bar(&mut self, ui: &mut Ui, dur: f64, width: f32) -> Option<f64> {
        let s = self.scheme;
        let pos = self.drag_pos.unwrap_or(self.progress.pos);
        let (rect, resp) =
            ui.allocate_exact_size(vec2(width.max(80.0), 22.0), Sense::click_and_drag());
        let cy = rect.center().y;
        let track = Rect::from_min_max(
            pos2(rect.left(), cy - 2.0),
            pos2(rect.right(), cy + 2.0),
        );
        ui.painter()
            .rect_filled(track, Rounding::same(2.0), s.surface_highest);
        let frac = if dur > 0.0 {
            (pos / dur).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let filled = Rect::from_min_max(
            track.min,
            pos2(track.left() + track.width() * frac, track.max.y),
        );
        ui.painter()
            .rect_filled(filled, Rounding::same(2.0), s.primary);
        let t = ui
            .ctx()
            .animate_bool_with_time(resp.id.with("thumb"), resp.hovered() || resp.dragged(), 0.1);
        if t > 0.01 {
            ui.painter()
                .circle_filled(pos2(filled.right(), cy), 6.0 * t, s.primary);
        }
        let mut out = None;
        if (resp.dragged() || resp.clicked()) && dur > 0.0 {
            if let Some(ptr) = resp.interact_pointer_pos() {
                let f = ((ptr.x - track.left()) / track.width().max(1.0)).clamp(0.0, 1.0);
                let v = f as f64 * dur;
                if resp.dragged() {
                    self.drag_pos = Some(v);
                }
                if resp.drag_stopped() || resp.clicked() {
                    self.drag_pos = None;
                    out = Some(v);
                }
            }
        }
        out
    }

    fn volume(&mut self, ui: &mut Ui, width: f32) {
        let s = self.scheme;
        let mut vol = self.progress.volume;
        let (rect, resp) =
            ui.allocate_exact_size(vec2(width.max(48.0), 20.0), Sense::click_and_drag());
        let cy = rect.center().y;
        let track = Rect::from_min_max(pos2(rect.left(), cy - 2.0), pos2(rect.right(), cy + 2.0));
        ui.painter()
            .rect_filled(track, Rounding::same(2.0), s.surface_highest);
        let filled = Rect::from_min_max(
            track.min,
            pos2(track.left() + track.width() * vol, track.max.y),
        );
        ui.painter()
            .rect_filled(filled, Rounding::same(2.0), s.on_surface_variant);
        if resp.hovered() || resp.dragged() {
            ui.painter().circle_filled(
                pos2(filled.right(), cy),
                5.0,
                s.on_surface_variant,
            );
        }
        if resp.dragged() || resp.clicked() {
            if let Some(ptr) = resp.interact_pointer_pos() {
                vol = ((ptr.x - track.left()) / track.width().max(1.0)).clamp(0.0, 1.0);
                self.player.set_volume(vol);
            }
        }
    }

    fn art(&mut self, ui: &mut Ui, side: f32, round: f32, url: &str) {
        let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click());
        let s = self.scheme;
        ui.painter()
            .rect_filled(rect, Rounding::same(round), s.surface_high);
        if let Some(t) = self.thumb_of(url) {
            egui::Image::new((t.id(), t.size_vec2()))
                .fit_to_exact_size(vec2(side, side))
                .rounding(Rounding::same(round))
                .paint_at(ui, rect);
        } else if !url.is_empty() {
            let tt = ui.input(|i| i.time) as f32;
            let a = 0.16 + 0.16 * (tt * 1.7).sin().abs();
            ui.painter()
                .circle_filled(rect.center(), side * 0.05, s.on_surface.gamma_multiply(a));
        }
        if resp.hovered() {
            self.layer(ui, rect, round, &resp, 0.05);
        }
    }


    fn track_row(&mut self, ui: &mut Ui, t: &Track, idx: usize, dense: bool) -> bool {
        let s = self.scheme;
        let _ = idx;
        let playing_this = self.current.as_ref().map(|c| c.id == t.id).unwrap_or(false);
        let ctx = ui.ctx().clone();
        self.ensure_thumb(&ctx, &t.thumb);
        let h = if dense { 50.0 } else { 56.0 };
        let side = if dense { 38.0 } else { 40.0 };
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
        if playing_this {
            ui.painter()
                .rect_filled(rect, Rounding::same(shape::ROW), s.surface_high);
        } else if resp.hovered() {
            ui.painter().rect_filled(
                rect,
                Rounding::same(shape::ROW),
                s.on_surface.gamma_multiply(0.06),
            );
        }
        let mid = rect.center().y;
        let img = Rect::from_min_size(pos2(rect.left() + 4.0, mid - side / 2.0), vec2(side, side));
        ui.painter()
            .rect_filled(img, Rounding::same(shape::RAD_THUMB), s.surface_high);
        if let Some(tex) = self.thumb_of(&t.thumb) {
            egui::Image::new((tex.id(), tex.size_vec2()))
                .fit_to_exact_size(vec2(side, side))
                .rounding(Rounding::same(shape::RAD_THUMB))
                .paint_at(ui, img);
        }
        if playing_this {
            ui.painter().rect_filled(
                img,
                Rounding::same(shape::RAD_THUMB),
                Color32::from_black_alpha(150),
            );
            draw_icon(
                ui.painter(),
                Icon::Play,
                Rect::from_center_size(img.center(), vec2(13.0, 13.0)),
                Color32::WHITE,
            );
        }

        let x = img.right() + 12.0;
        let dur_w = 46.0;
        let text_w = (rect.right() - x - dur_w - 12.0).max(60.0);
        ui.painter().text(
            pos2(x, mid - 8.0),
            Align2::LEFT_CENTER,
            pas(ui.ctx(), &t.title, text_w, 14.0),
            FontId::proportional(14.0),
            s.on_surface,
        );
        ui.painter().text(
            pos2(x, mid + 10.0),
            Align2::LEFT_CENTER,
            pas(ui.ctx(), &t.artist, text_w, 12.0),
            FontId::proportional(12.0),
            s.on_surface_variant,
        );
        ui.painter().text(
            pos2(rect.right() - 10.0, mid),
            Align2::RIGHT_CENTER,
            &t.dur,
            FontId::proportional(11.0),
            s.on_surface_variant,
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp.clicked()
    }


    /// Kartu album: wadah `#181818` radius 8 dengan padding 8, sampul radius 6,
    /// judul 14 tebal + sub 12 redup.
    fn card(&mut self, ui: &mut Ui, card: &Card, w: f32) {
        let s = self.scheme;
        let ctx = ui.ctx().clone();
        self.ensure_thumb(&ctx, &card.thumb);
        let tex = self.thumb_of(&card.thumb);
        let tinggi = w + 76.0;
        ui.allocate_ui_with_layout(vec2(w, tinggi), Layout::top_down(Align::Min), |ui| {
            ui.set_width(w);
            let (wadah, resp) = ui.allocate_exact_size(vec2(w, tinggi), Sense::click());
            ui.painter()
                .rect_filled(wadah, Rounding::same(shape::CARD), s.surface_low);
            if resp.hovered() {
                ui.painter().rect_filled(
                    wadah,
                    Rounding::same(shape::CARD),
                    s.surface_highest,
                );
            }
            let pad = 8.0;
            let rect = Rect::from_min_size(
                pos2(wadah.left() + pad, wadah.top() + pad),
                vec2(w - pad * 2.0, w - pad * 2.0),
            );
            ui.painter()
                .rect_filled(rect, Rounding::same(shape::RAD_CARD), s.surface_high);
            if let Some(t) = tex {
                egui::Image::new((t.id(), t.size_vec2()))
                    .fit_to_exact_size(rect.size())
                    .rounding(Rounding::same(shape::RAD_CARD))
                    .paint_at(ui, rect);
            } else if !card.thumb.is_empty() {
                let tt = ui.input(|i| i.time) as f32;
                let a = 0.16 + 0.16 * (tt * 1.7).sin().abs();
                ui.painter().circle_filled(
                    rect.center(),
                    rect.width() * 0.045,
                    s.on_surface.gamma_multiply(a),
                );
            }
            if resp.hovered() {
                let c = pos2(rect.right() - 24.0, rect.bottom() - 24.0);
                ui.painter().circle_filled(c, 20.0, Color32::WHITE);
                draw_icon(
                    ui.painter(),
                    Icon::Play,
                    Rect::from_center_size(c, vec2(16.0, 16.0)),
                    s.on_primary,
                );
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                self.open_card(card);
            }
            // Teks di dalam padding, tidak menembus tepi wadah.
            let lebar_teks = (w - pad * 2.0).max(40.0);
            ui.painter().text(
                pos2(wadah.left() + pad + 2.0, rect.bottom() + 18.0),
                Align2::LEFT_CENTER,
                pas(ui.ctx(), &card.title, lebar_teks, 14.0),
                FontId::proportional(14.0),
                s.on_surface,
            );
            ui.painter().text(
                pos2(wadah.left() + pad + 2.0, rect.bottom() + 38.0),
                Align2::LEFT_CENTER,
                pas(ui.ctx(), &card.sub, lebar_teks, 12.0),
                FontId::proportional(12.0),
                s.on_surface_variant,
            );
            // ruang kosong tetap dialokasikan supaya tinggi kartu seragam
            ui.add_space(tinggi - pad - rect.height());
        });
    }

    fn card_grid(&mut self, ui: &mut Ui, items: &[Card], compact: bool) {
        self.card_grid_sized(ui, items, compact, false)
    }

    /// `hero` memperbesar kartu baris pertama: momen editorial khas Expressive.
    fn card_grid_sized(&mut self, ui: &mut Ui, items: &[Card], compact: bool, hero: bool) {
        let avail = ui.available_width();
        let gap = 16.0;
        let target = if compact { 128.0 } else { 172.0 };
        let max_cols = if compact { 3 } else { 6 };
        let cols = (((avail + gap) / (target + gap)).floor() as usize)
            .clamp(1, max_cols)
            .min(items.len().max(1));
        let _ = hero;
        let cw = ((avail - gap * (cols as f32 - 1.0)) / cols as f32).floor();
        for chunk in items.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for card in chunk {
                    self.card(ui, card, cw);
                }
            });
            ui.add_space(gap);
        }
    }

    fn open_card(&mut self, card: &Card) {
        if card.kind == "song" {
            let t = Track {
                id: card.id.clone(),
                title: card.title.clone(),
                artist: card
                    .sub
                    .split('•')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                album: String::new(),
                thumb: card.thumb.clone(),
                dur: String::new(),
                secs: 0.0,
            };
            self.play_track(t, None);
        } else {
            self.koleksi = None;
            self.view = View::Collection;
            self.tab = Tab::Koleksi;
            self.worker.send(Job::Browse(card.id.clone()));
        }
    }
}

// ------------------------------------------------------------------ tata letak

impl App {

    fn ui(&mut self, ctx: &egui::Context) {
        let s = self.scheme;
        let w = ctx.screen_rect().width();
        // Breakpoint tetap seperti sebelumnya; yang berubah hanya geometri.
        let narrow = w < 1312.0;
        let side_w = if narrow { 88.0 } else { 240.0 };

        // Bilah pemutar menempel di dasar: 90 px, garis 1 px di tepi atas.
        if self.current.is_some() {
            egui::TopBottomPanel::bottom("bar")
                .resizable(false)
                .exact_height(90.0)
                .frame(
                    egui::Frame::none()
                        .fill(s.surface_low)
                        .stroke(egui::Stroke::new(1.0, s.outline_variant))
                        .inner_margin(egui::Margin::symmetric(16.0, 0.0)),
                )
                .show(ctx, |ui| self.play_bar(ui, w));
        }

        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(side_w)
            .frame(
                egui::Frame::none()
                    .fill(s.surface_low)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0)),
            )
            .show(ctx, |ui| self.side_nav(ui, narrow));

        egui::TopBottomPanel::top("top")
            .resizable(false)
            .exact_height(64.0)
            .frame(
                egui::Frame::none()
                    .fill(s.surface)
                    .inner_margin(egui::Margin::symmetric(24.0, 10.0)),
            )
            .show(ctx, |ui| self.top_bar(ui));

        // Panel kanan 384 px, berhenti di atas bilah pemutar.
        if self.show_queue {
            let qw = if narrow { (w * 0.42).clamp(300.0, 384.0) } else { 384.0 };
            egui::SidePanel::right("queue")
                .resizable(false)
                .exact_width(qw)
                .frame(
                    egui::Frame::none()
                        .fill(s.surface_low)
                        .inner_margin(egui::Margin::symmetric(20.0, 16.0)),
                )
                .show(ctx, |ui| self.queue_panel(ui));
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none().fill(s.surface).inner_margin(egui::Margin {
                    left: 24.0,
                    right: 24.0,
                    top: 8.0,
                    bottom: 8.0,
                }),
            )
            .show(ctx, |ui| match self.tab {
                Tab::Home => self.tab_home(ui, narrow),
                Tab::Cari => self.tab_cari(ui),
                Tab::Koleksi => self.tab_koleksi(ui),
            });

        // ---- lembar pemutar penuh, naik dari bawah
        let anim = ctx.animate_bool_with_time(egui::Id::new("np-expand"), self.expanded, 0.26);
        if anim > 0.001 {
            self.expanded_sheet(ctx, anim);
        }

        if let Some((msg, at)) = self.toast.clone() {
            if at.elapsed() < Duration::from_secs(4) {
                egui::Area::new("toast".into())
                    .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -110.0))
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(self.scheme.surface_highest)
                            .rounding(Rounding::same(shape::ROW))
                            .inner_margin(egui::Margin::symmetric(16.0, 10.0))
                            .show(ui, |ui| {
                                ui.label(ts::body(&msg, self.scheme.on_surface));
                            });
                    });
                ctx.request_repaint_after(Duration::from_millis(350));
            } else {
                self.toast = None;
            }
        }
    }

    fn goto(&mut self, t: Tab) {
        self.tab = t;
        if t == Tab::Koleksi {
            self.view = View::Feed;
            if self.koleksi.is_none() {
                self.worker.send(Job::Browse("FEmusic_liked_playlists".into()));
            }
            // Langsung tarik playlist akun; jangan menunggu tombol ditekan.
            if self.lib.is_empty() && self.logged.is_some() {
                self.worker.send(Job::Library);
            }
        }
        if t == Tab::Cari {
            self.focus_cari = true;
        }
    }

    /// Satu baris navigasi: ikon 20 px, label 14 px, pil radius 10 px saat aktif.

    fn side_item(&mut self, ui: &mut Ui, t: Tab, active: bool, narrow: bool) -> Response {
        let s = self.scheme;
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
        let pill = if narrow {
            Rect::from_center_size(rect.center(), vec2(w.min(56.0), 40.0))
        } else {
            Rect::from_center_size(rect.center(), vec2(w, 40.0))
        };
        if active {
            ui.painter()
                .rect_filled(pill, Rounding::same(shape::ROW), s.surface_highest);
        } else if resp.hovered() {
            ui.painter().rect_filled(
                pill,
                Rounding::same(shape::ROW),
                s.on_surface.gamma_multiply(0.06),
            );
        }
        let fg = if active { s.on_surface } else { s.on_surface_variant };
        if narrow {
            draw_icon(
                ui.painter(),
                t.icon(),
                Rect::from_center_size(pill.center(), vec2(22.0, 22.0)),
                fg,
            );
        } else {
            draw_icon(
                ui.painter(),
                t.icon(),
                Rect::from_min_size(
                    pos2(pill.left() + 22.0, pill.center().y - 11.0),
                    vec2(22.0, 22.0),
                ),
                fg,
            );
            ui.painter().text(
                pos2(pill.left() + 46.0, pill.center().y),
                Align2::LEFT_CENTER,
                t.label(),
                FontId::proportional(14.0),
                fg,
            );
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp
    }


    fn side_nav(&mut self, ui: &mut Ui, narrow: bool) {
        let s = self.scheme;

        // Logo lingkaran putih 32 px + wordmark, blok 76 px sampai baris nav.
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 60.0), Sense::hover());
        let cx = if narrow { r.center().x } else { r.left() + 16.0 };
        ui.painter().circle_filled(pos2(cx, r.center().y), 16.0, Color32::WHITE);
        draw_icon(
            ui.painter(),
            Icon::Logo,
            Rect::from_center_size(pos2(cx, r.center().y), vec2(18.0, 18.0)),
            s.on_primary,
        );
        if !narrow {
            // Nama aplikasi di samping logo: tanpa ini sidebar terasa kosong.
            let mut teks = "swara".to_string();
            if let Some(c) = teks.get_mut(0..1) {
                let _ = c;
            }
            ui.painter().text(
                pos2(r.left() + 44.0, r.center().y),
                Align2::LEFT_CENTER,
                teks,
                FontId::proportional(18.0),
                s.on_surface,
            );
        }
        ui.add_space(8.0);

        for t in [Tab::Home, Tab::Cari, Tab::Koleksi] {
            let active = self.tab == t;
            if self.side_item(ui, t, active, narrow).clicked() {
                self.goto(t);
            }
            ui.add_space(2.0);
        }

        if !narrow {
            ui.add_space(24.0);
            ui.label(ts::label("Pustaka", s.on_surface_variant));
            ui.add_space(6.0);
            if self.lib_item(ui, "Lagu disukai").clicked() {
                self.koleksi = None;
                self.view = View::Collection;
                self.tab = Tab::Koleksi;
                self.worker.send(Job::Browse("FEmusic_liked_videos".into()));
            }
            if self.lib_item(ui, "Playlist saya").clicked() {
                if self.logged.is_some() {
                    self.tab = Tab::Koleksi;
                    self.view = View::Feed;
                    self.worker.send(Job::Library);
                } else {
                    self.toast("masuk dulu lewat sesi Chromium");
                }
            }
        }

        // kaki sidebar: tema + akun
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            let mut keluar = false;
            let mut masuk = false;
            if !narrow {
                let (rect, resp) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
                let ind = Rect::from_center_size(rect.center(), vec2(rect.width(), 36.0));
                if resp.hovered() {
                    ui.painter().rect_filled(
                        ind,
                        Rounding::same(shape::ROW),
                        s.on_surface.gamma_multiply(0.06),
                    );
                }
                let nama = self.logged.clone();
                let awal = nama
                    .as_ref()
                    .map(|n| n.chars().next().unwrap_or('?').to_uppercase().to_string())
                    .unwrap_or_else(|| "?".into());
                ui.painter().circle_filled(
                    pos2(ind.left() + 16.0, ind.center().y),
                    15.0,
                    if nama.is_some() {
                        s.surface_highest
                    } else {
                        s.surface_high
                    },
                );
                ui.painter().text(
                    pos2(ind.left() + 16.0, ind.center().y),
                    Align2::CENTER_CENTER,
                    awal,
                    FontId::proportional(12.5),
                    s.on_surface,
                );
                ui.painter().text(
                    pos2(ind.left() + 40.0, ind.center().y),
                    Align2::LEFT_CENTER,
                    match &nama {
                        Some(n) => pas(ui.ctx(), n, ind.width() - 56.0, 13.0),
                        None => "Masuk".to_string(),
                    },
                    FontId::proportional(13.0),
                    s.on_surface,
                );
                if resp.clicked() {
                    if nama.is_some() {
                        keluar = true;
                    } else {
                        masuk = true;
                    }
                }
                ui.add_space(4.0);
            }
            if self.icon_button(ui, Icon::Theme, 34.0, Btn::Ghost).clicked() {
                self.scheme = theme::scheme(!s.dark, self.seed);
            }
            if keluar {
                self.worker.send(Job::Logout);
                self.logged = None;
                self.toast("keluar dari akun");
            }
            if masuk {
                self.toast("membaca sesi YouTube dari Chromium, tanpa kata sandi");
                self.worker.send(Job::Login);
            }
        });
    }


    fn lib_item(&mut self, ui: &mut Ui, label: &str) -> Response {
        let s = self.scheme;
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(
                rect,
                Rounding::same(shape::ROW),
                s.on_surface.gamma_multiply(0.06),
            );
        }
        draw_icon(
            ui.painter(),
            Icon::Library,
            Rect::from_min_size(
                pos2(rect.left() + 22.0, rect.center().y - 8.5),
                vec2(17.0, 17.0),
            ),
            s.on_surface_variant,
        );
        ui.painter().text(
            pos2(rect.left() + 46.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(13.0),
            s.on_surface,
        );
        resp
    }

    /// Bilah atas: kolom cari membulat penuh di kiri, ikon di kanan.

    /// Bilah atas 64 px: kolom cari pil 44 px, tiga ikon, lalu avatar 34 px.
    fn top_bar(&mut self, ui: &mut Ui) {
        let s = self.scheme;
        let full = ui.max_rect();
        let mut submit = false;

        let sisa = (full.width() - 210.0).max(160.0);
        let field_w = 420.0_f32.min(sisa);
        let pill = Rect::from_min_size(
            pos2(full.left(), full.center().y - 22.0),
            vec2(field_w, 44.0),
        );
        let garis = if self.search_focus {
            s.outline
        } else {
            s.surface_high
        };
        ui.painter()
            .rect_filled(pill, Rounding::same(22.0), s.surface_high);
        if self.search_focus {
            ui.painter()
                .rect_stroke(pill, Rounding::same(22.0), egui::Stroke::new(1.0, garis));
        }
        draw_icon(
            ui.painter(),
            Icon::Search,
            Rect::from_min_size(
                pos2(pill.left() + 18.0, pill.center().y - 8.5),
                vec2(17.0, 17.0),
            ),
            s.on_surface_variant,
        );
        let te_rect = Rect::from_min_max(
            pos2(pill.left() + 50.0, pill.top() + 12.0),
            pos2(pill.right() - 14.0, pill.bottom() - 12.0),
        );
        let te = egui::TextEdit::singleline(&mut self.query)
            .frame(false)
            .margin(vec2(0.0, 0.0));
        let resp = ui.put(te_rect, te);
        self.search_focus = resp.has_focus();
        if self.query.is_empty() && !resp.has_focus() {
            ui.painter().text(
                pos2(te_rect.left(), te_rect.center().y),
                Align2::LEFT_CENTER,
                "Cari lagu, album, artis",
                FontId::proportional(13.0),
                s.on_surface_variant,
            );
        }
        if self.focus_cari {
            resp.request_focus();
            self.focus_cari = false;
        }
        if resp.changed() {
            self.suggest_at = Some(Instant::now());
            self.show_suggest = true;
        }
        let tekan_enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if tekan_enter && (resp.has_focus() || resp.lost_focus()) {
            submit = true;
        }

        // ---- kanan: avatar 34 di ujung, ikon 18 berjarak 46 px
        let cy = full.center().y;
        let avatar = Rect::from_center_size(pos2(full.right() - 17.0, cy), vec2(34.0, 34.0));
        let logged = self.logged.clone();
        let mut keluar = false;
        let mut masuk = false;
        let (av) = ui.interact(avatar, egui::Id::new("tb-avatar"), Sense::click());
        ui.painter()
            .circle_filled(avatar.center(), 17.0, s.surface_highest);
        ui.painter().text(
            avatar.center(),
            Align2::CENTER_CENTER,
            logged
                .as_ref()
                .map(|n| n.chars().next().unwrap_or('?').to_uppercase().to_string())
                .unwrap_or_else(|| "?".into()),
            FontId::proportional(13.0),
            if logged.is_some() {
                s.on_surface
            } else {
                s.on_surface_variant
            },
        );
        if av.clicked() {
            if logged.is_some() {
                keluar = true;
            } else {
                masuk = true;
            }
        }

        let mut ikon = |id: &str, offset: f32, icon: Icon, aktif: bool| -> bool {
            let r = Rect::from_center_size(pos2(full.right() - offset, cy), vec2(19.0, 19.0));
            let hit = ui.interact(r.expand(10.0), egui::Id::new(id), Sense::click());
            if hit.hovered() {
                ui.painter()
                    .circle_filled(r.center(), 17.0, s.on_surface.gamma_multiply(0.08));
            }
            draw_ikon(
                ui.painter(),
                icon,
                r,
                if aktif { s.on_surface } else { s.on_surface_variant },
                aktif,
            );
            if hit.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            hit.clicked()
        };
        let tema = ikon("tb-tema", 168.0, Icon::Theme, false);
        let antrean = ikon("tb-queue", 122.0, Icon::Queue, self.show_queue && !self.show_lyrics);
        let lirik = ikon("tb-lyrics", 76.0, Icon::Lyrics, self.show_queue && self.show_lyrics);
        if tema {
            self.scheme = theme::scheme(!s.dark, self.seed);
        }
        if antrean {
            self.show_queue = true;
            self.show_lyrics = false;
        }
        if lirik {
            self.show_queue = true;
            self.show_lyrics = true;
        }
        if let Some(at) = self.suggest_at {
            if at.elapsed() > Duration::from_millis(240) && !self.query.is_empty() {
                self.worker.send(Job::Suggest(self.query.clone()));
                self.suggest_at = None;
            }
        }
        if submit && !self.query.is_empty() {
            self.searched = self.query.clone();
            self.show_suggest = false;
            self.results.clear();
            self.last_error = None;
            self.tab = Tab::Cari;
            self.worker.send(Job::Search(self.query.clone()));
        }
        if keluar {
            self.worker.send(Job::Logout);
            self.logged = None;
            self.toast("keluar dari akun");
        }
        if masuk {
            self.toast("membaca sesi YouTube dari Chromium, tanpa kata sandi");
            self.worker.send(Job::Login);
        }
    }

    fn bar_icon(&mut self, ui: &mut Ui, icon: Icon, rect: Rect, id: &str) -> bool {
        let s = self.scheme;
        let resp = ui.interact(rect, egui::Id::new(id), Sense::click());
        if resp.hovered() {
            ui.painter()
                .circle_filled(rect.center(), rect.width() * 0.52, s.on_surface.gamma_multiply(0.08));
        }
        draw_icon(ui.painter(), icon, rect.shrink(5.0), s.on_surface_variant);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp.clicked()
    }

    /// Pemutar bawah: kontrol rapat di kiri, info lagu tepat di tengah,
    /// ikon sekunder berderet dari kanan — susunan yang sama dengan YT Music.

    /// Bilah pemutar 90 px: info lagu di kiri, transport + durasi di tengah,
    /// ikon sekunder di kanan (kanvas `swara-ui`).
    fn play_bar(&mut self, ui: &mut Ui, win_w: f32) {
        let s = self.scheme;
        let track = self.current.clone();
        let dur = self
            .current_info
            .as_ref()
            .map(|i| i.dur)
            .filter(|d| *d > 0.0)
            .or_else(|| track.as_ref().map(|t| t.secs))
            .unwrap_or(0.0);
        let cover = self.cover_of();
        let full = ui.max_rect();
        let mid = full.center().y;
        let pos_now = self.drag_pos.unwrap_or(self.progress.pos);

        // ---- kiri: sampul 56, judul 14, artis 11.5
        let art = Rect::from_min_size(pos2(full.left() + 16.0, mid - 28.0), vec2(56.0, 56.0));
        ui.painter()
            .rect_filled(art, Rounding::same(shape::RAD_THUMB), s.surface_high);
        if let Some(t) = cover {
            egui::Image::new((t.id(), t.size_vec2()))
                .fit_to_exact_size(vec2(56.0, 56.0))
                .rounding(Rounding::same(shape::RAD_THUMB))
                .paint_at(ui, art);
        }
        let tx = art.right() + 12.0;
        // Ruang teks dibatasi supaya judul tidak pernah menembus transport.
        let ruang = (full.width() * 0.26).clamp(110.0, 240.0);
        let (title, artist) = match &track {
            Some(t) => (t.title.clone(), t.artist.clone()),
            None => ("Belum ada lagu".into(), String::new()),
        };
        ui.painter().text(
            pos2(tx, mid - 9.0),
            Align2::LEFT_CENTER,
            pas(ui.ctx(), &title, ruang, 14.0),
            FontId::proportional(14.0),
            s.on_surface,
        );
        ui.painter().text(
            pos2(tx, mid + 10.0),
            Align2::LEFT_CENTER,
            pas(ui.ctx(), &artist, ruang, 11.5),
            FontId::proportional(11.5),
            s.on_surface_variant,
        );

        // ---- tengah: shuffle, prev, putar 44, next, repeat
        let cx = full.center().x;
        let y_atas = full.top() + 30.0;
        let shuffle = Rect::from_center_size(pos2(cx - 116.0, y_atas), vec2(18.0, 18.0));
        let prev = Rect::from_center_size(pos2(cx - 70.0, y_atas), vec2(22.0, 22.0));
        let play = Rect::from_center_size(pos2(cx, y_atas), vec2(44.0, 44.0));
        let nxt = Rect::from_center_size(pos2(cx + 70.0, y_atas), vec2(22.0, 22.0));
        let repeat = Rect::from_center_size(pos2(cx + 116.0, y_atas), vec2(18.0, 18.0));

        let mut tombol = |r: Rect, id: &str, icon: Icon, ukuran: f32, aktif: bool| -> bool {
            let hit = ui.interact(r.expand(8.0), egui::Id::new(id), Sense::click());
            if hit.hovered() {
                ui.painter()
                    .circle_filled(r.center(), r.width() * 0.85, s.on_surface.gamma_multiply(0.08));
            }
            draw_ikon(
                ui.painter(),
                icon,
                Rect::from_center_size(r.center(), vec2(ukuran, ukuran)),
                if aktif { s.on_surface } else { s.on_surface_variant },
                aktif,
            );
            if hit.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            hit.clicked()
        };

        let luas = win_w >= 900.0;
        if luas && tombol(shuffle, "pb-shuf", Icon::Shuffle, 18.0, self.shuffle_on) {
            self.shuffle_queue();
        }
        if tombol(prev, "pb-prev", Icon::Prev, 22.0, false) {
            self.next_track(-1);
        }
        // Tombol putar: lingkaran putih penuh, glyph hitam.
        let ph = ui.interact(play.expand(4.0), egui::Id::new("pb-play"), Sense::click());
        ui.painter().circle_filled(
            play.center(),
            22.0,
            if ph.hovered() { Color32::from_gray(235) } else { Color32::WHITE },
        );
        draw_icon(
            ui.painter(),
            if self.progress.playing {
                Icon::Pause
            } else {
                Icon::Play
            },
            play.shrink(12.0),
            s.on_primary,
        );
        if ph.clicked() {
            self.toggle_play();
        }
        if tombol(nxt, "pb-next", Icon::Next, 22.0, false) {
            self.next_track(1);
        }
        if luas && tombol(repeat, "pb-rep", Icon::Repeat, 18.0, self.ulangi > 0) {
            self.putar_ulang();
        }

        // ---- durasi: track 440 px di tengah bawah, waktu di kedua ujung
        let lebar_track = 440.0_f32.min(full.width() * 0.42);
        let ty = full.top() + 66.0;
        let track_rect = Rect::from_center_size(pos2(cx, ty), vec2(lebar_track, 3.0));
        ui.painter()
            .rect_filled(track_rect, Rounding::same(1.5), s.outline);
        if dur > 0.0 {
            let f = (pos_now / dur).clamp(0.0, 1.0) as f32;
            ui.painter().rect_filled(
                Rect::from_min_max(
                    track_rect.min,
                    pos2(track_rect.left() + track_rect.width() * f, track_rect.bottom()),
                ),
                Rounding::same(1.5),
                Color32::WHITE,
            );
        }
        let hit = ui.interact(
            track_rect.expand2(vec2(0.0, 9.0)),
            egui::Id::new("bar-seek"),
            Sense::click(),
        );
        if hit.clicked() && dur > 0.0 {
            if let Some(p) = hit.interact_pointer_pos() {
                let f = ((p.x - track_rect.left()) / track_rect.width().max(1.0)).clamp(0.0, 1.0);
                self.player.seek(f as f64 * dur);
            }
        }
        ui.painter().text(
            pos2(track_rect.left() - 10.0, ty),
            Align2::RIGHT_CENTER,
            fmt_time(pos_now),
            FontId::proportional(10.5),
            s.on_surface_variant,
        );
        ui.painter().text(
            pos2(track_rect.right() + 10.0, ty),
            Align2::LEFT_CENTER,
            fmt_time(dur),
            FontId::proportional(10.5),
            s.on_surface_variant,
        );

        // ---- kanan: ikon 18 berjarak 46 px dari tepi
        let ry = full.top() + 30.0;
        let mut kanan = |id: &str, offset: f32, icon: Icon, aktif: bool, ukuran: f32| -> bool {
            let r = Rect::from_center_size(
                pos2(full.right() - offset, ry),
                vec2(ukuran, ukuran),
            );
            let hit = ui.interact(r.expand(9.0), egui::Id::new(id), Sense::click());
            if hit.hovered() {
                ui.painter()
                    .circle_filled(r.center(), 17.0, s.on_surface.gamma_multiply(0.08));
            }
            draw_ikon(
                ui.painter(),
                icon,
                r,
                if aktif { s.on_surface } else { s.on_surface_variant },
                aktif,
            );
            if hit.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            hit.clicked()
        };
        let nilai = self
            .current
            .as_ref()
            .and_then(|t| self.rating.get(&t.id).copied())
            .unwrap_or(0);
        if win_w >= 760.0 && kanan("pb-up", 164.0, Icon::ChevronUp, false, 18.0) {
            self.expanded = true;
        }
        if kanan("pb-like", 120.0, Icon::Like, nilai == 1, 18.0) {
            self.nilai_lagu(1);
        }
        if kanan("pb-queue", 76.0, Icon::Queue, self.show_queue && !self.show_lyrics, 18.0) {
            self.show_queue = true;
            self.show_lyrics = false;
        }
        let bisu = self.progress.volume <= 0.01;
        if kanan("pb-vol", 32.0, Icon::Volume, bisu, 18.0) {
            let v = self.progress.volume;
            self.player.set_volume(if v > 0.01 { 0.0 } else { 0.8 });
        }
    }

    /// Ulangi tiga keadaan: mati, semua lagu, satu lagu.
    fn putar_ulang(&mut self) {
        self.ulangi = (self.ulangi + 1) % 3;
        self.toast(match self.ulangi {
            1 => "ulangi: semua lagu",
            2 => "ulangi: satu lagu",
            _ => "ulangi: mati",
        });
    }

    fn shuffle_queue(&mut self) {
        self.shuffle_on = !self.shuffle_on;
        if !self.shuffle_on {
            self.toast("acak mati");
            return;
        }
        let n = self.queue.len();
        if n < 2 {
            self.toast("antrean terlalu pendek untuk diacak");
            return;
        }
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E3779B9);
        for i in (1..n).rev() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let j = (seed >> 33) as usize % (i + 1);
            self.queue.swap(i, j);
        }
        self.toast("acak hidup: antrean diacak");
    }

    /// Panel kanan: antrean atau lirik, dengan tab bergaris bawah.
    fn queue_panel(&mut self, ui: &mut Ui) {
        let s = self.scheme;
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            for (lbl, lyr) in [("BERIKUTNYA", false), ("LIRIK", true)] {
                let sel = self.show_lyrics == lyr;
                let wid = 96.0;
                let (rect, resp) = ui.allocate_exact_size(vec2(wid, 30.0), Sense::click());
                ui.painter().text(
                    pos2(rect.center().x, rect.center().y - 2.0),
                    Align2::CENTER_CENTER,
                    lbl,
                    FontId::proportional(11.0),
                    if sel { s.on_surface } else { s.on_surface_variant },
                );
                if sel {
                    ui.painter().rect_filled(
                        Rect::from_min_size(pos2(rect.left() + 10.0, rect.bottom() - 3.0), vec2(wid - 20.0, 2.0)),
                        Rounding::same(1.0),
                        s.on_surface,
                    );
                }
                if resp.clicked() {
                    self.show_lyrics = lyr;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.icon_button(ui, Icon::More, 28.0, Btn::Ghost).clicked() {
                    self.show_queue = false;
                }
            });
        });
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter()
            .rect_filled(line, Rounding::same(0.5), s.outline_variant);
        ui.add_space(10.0);

        if self.show_lyrics {
            let h = ui.available_height();
            self.lyrics_view(ui, h);
            return;
        }

        ui.label(ts::body_s("Diputar dari", s.on_surface_variant));
        let from = self
            .current
            .as_ref()
            .map(|t| format!("Radio {}", short(&t.title, 22)))
            .unwrap_or_else(|| "Antrean".into());
        ui.label(ts::title_m(from, s.on_surface).size(14.0));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if self.chip(ui, "Semua", true).clicked() {}
            if self.chip(ui, "Berikutnya", false).clicked() {
                ui.ctx().request_repaint();
            }
        });
        ui.add_space(6.0);
        let queue = self.queue.clone();
        let cur = self.current.clone();
        ScrollArea::vertical()
            .id_salt("qrows")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if let Some(t) = cur.as_ref() {
                    self.track_row(ui, t, 0, false);
                }
                for (i, t) in queue.iter().enumerate() {
                    if self.track_row(ui, t, i + 1, false) {
                        let q = queue.clone();
                        self.play_track(t.clone(), Some(q));
                    }
                }
                ui.add_space(16.0);
            });
    }

    // ---- pemutar penuh (klik chevron di bar bawah)

    /// Lembar pemutar penuh yang naik dari bawah. `t` 0..1 dari animasi bawaan
    /// egui, jadi membuka dan menutup sama-sama beranimasi.
    fn expanded_sheet(&mut self, ctx: &egui::Context, t: f32) {
        let s = self.scheme;
        let screen = ctx.screen_rect();
        let dy = (1.0 - t) * screen.height();
        egui::Area::new("np-sheet".into())
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_clip_rect(screen);
                // latar gelap penuh; tepi atasnya ikut naik bersama animasi
                ui.painter().rect_filled(
                    Rect::from_min_max(pos2(screen.left(), screen.top() + dy), screen.max),
                    Rounding::same(0.0),
                    s.surface,
                );
                ui.add_space(dy);
                // Isi tetap seukuran layar penuh, hanya digeser turun; tata
                // letaknya tidak dihitung ulang, jadi animasi turun mulus.
                let body = Rect::from_min_size(
                    pos2(screen.left(), screen.top() + dy),
                    vec2(screen.width(), screen.height()),
                );
                let clip = Rect::from_min_max(
                    pos2(screen.left(), screen.top() + dy),
                    screen.max,
                );
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(body.shrink2(vec2(24.0, 12.0)))
                        .layout(Layout::top_down(Align::Min)),
                );
                child.set_clip_rect(clip);
                self.now_playing(&mut child, screen.width());
            });
        // terus minta gambar selama animasi berjalan, termasuk saat menutup
        if t > 0.0 && t < 1.0 {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    fn now_playing(&mut self, ui: &mut Ui, w: f32) {
        let track = self.current.clone();
        let dur = self
            .current_info
            .as_ref()
            .map(|i| i.dur)
            .filter(|d| *d > 0.0)
            .or_else(|| track.as_ref().map(|t| t.secs))
            .unwrap_or(0.0);
        let cover = self.cover_of();
        let split = w >= 900.0;

        // kepala: turun ke beranda + pemilih panel
        ui.horizontal(|ui| {
            if self.icon_button(ui, Icon::ChevronDown, 36.0, Btn::Ghost).clicked() {
                self.expanded = false;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.chip(ui, "Lirik", self.show_lyrics).clicked() {
                    self.show_lyrics = !self.show_lyrics;
                }
                if self.chip(ui, "Antrean", !self.show_lyrics).clicked() {
                    self.show_lyrics = false;
                }
            });
        });
        ui.add_space(6.0);

        if split {
            let total_w = ui.available_width();
            let total_h = ui.available_height();
            ui.horizontal_top(|ui| {
                let side = (total_h - 210.0).clamp(150.0, total_w * 0.46).min(400.0);
                ui.vertical(|ui| {
                    ui.set_width(side);
                    self.big_art(ui, side, cover.clone());
                    ui.add_space(16.0);
                    self.player_body(ui, &track, dur);
                });
                ui.add_space(28.0);
                ui.vertical(|ui| {
                    ui.set_width((total_w - side - 28.0).max(240.0));
                    self.side_panel_body(ui, total_h - 12.0);
                });
            });
        } else {
            let side = (ui.available_width() * 0.62)
                .min(ui.available_height() * 0.40)
                .clamp(120.0, 320.0);
            ui.vertical_centered(|ui| self.big_art(ui, side, cover.clone()));
            ui.add_space(12.0);
            self.player_body(ui, &track, dur);
            ui.add_space(12.0);
            let rest = ui.available_height();
            if rest > 80.0 {
                self.side_panel_body(ui, rest - 8.0);
            }
        }
    }

    /// Isi panel kanan pada pemutar penuh: antrean atau lirik.
    fn side_panel_body(&mut self, ui: &mut Ui, max_h: f32) {
        let s = self.scheme;
        if self.show_lyrics {
            self.lyrics_view(ui, max_h);
            return;
        }
        let queue = self.queue.clone();
        let cur = self.current.clone();
        ScrollArea::vertical()
            .id_salt("np-queue")
            .max_height(max_h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label(ts::label("BERIKUTNYA", s.on_surface_variant).size(10.5));
                ui.add_space(6.0);
                if let Some(t) = cur.as_ref() {
                    self.track_row(ui, t, 0, false);
                }
                for (i, t) in queue.iter().enumerate() {
                    if self.track_row(ui, t, i + 1, false) {
                        let q = queue.clone();
                        self.play_track(t.clone(), Some(q));
                    }
                }
                ui.add_space(16.0);
            });
    }

    fn big_art(&mut self, ui: &mut Ui, side: f32, cover: Option<TextureHandle>) {
        let s = self.scheme;
        let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        ui.painter().rect_filled(rect, Rounding::same(8.0), s.surface_high);
        if let Some(t) = cover {
            egui::Image::new((t.id(), t.size_vec2()))
                .fit_to_exact_size(vec2(side, side))
                .rounding(Rounding::same(8.0))
                .paint_at(ui, rect);
        }
    }

    fn player_body(&mut self, ui: &mut Ui, track: &Option<Track>, dur: f64) {
        let s = self.scheme;
        let w = ui.available_width();
        let (title, artist) = match track {
            Some(t) => (t.title.clone(), t.artist.clone()),
            None => ("Belum ada lagu".into(), "Pilih dari beranda".into()),
        };
        let chars = (w / 9.0).max(10.0) as usize;
        ui.label(
            RichText::new(short(&title, chars))
                .size(22.0)
                .strong()
                .color(s.on_surface),
        );
        ui.add_space(1.0);
        ui.label(
            RichText::new(short(&artist, chars))
                .size(14.0)
                .color(s.on_surface_variant),
        );
        // Baris aksi di bawah artis, seperti pemutar YouTube Music: suka dan
        // tidak suka berdiri sendiri, tidak lagi menumpang di baris kontrol.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let nilai = self
                .current
                .as_ref()
                .and_then(|t| self.rating.get(&t.id).copied())
                .unwrap_or(0);
            if self.chip_ikon(ui, Icon::Like, "Suka", nilai == 1) {
                self.nilai_lagu(1);
            }
            if self.chip_ikon(ui, Icon::Dislike, "Tidak", nilai == -1) {
                self.nilai_lagu(-1);
            }
        });
        ui.add_space(10.0);
        if let Some(v) = self.seek_bar(ui, dur, w) {
            self.player.seek(v);
        }
        ui.horizontal(|ui| {
            ui.label(ts::label(fmt_time(self.progress.pos), s.on_surface_variant).size(11.5));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(ts::label(fmt_time(dur), s.on_surface_variant).size(11.5));
            });
        });
        ui.add_space(10.0);

        let playing = self.progress.playing;
        let mut act = 0i8;
        // Satu baris dengan geometri mutlak supaya pusat ketiga tombol benar-benar
        // sejajar, tidak bergantung pada cross-align layout egui.
        let (row, _) = ui.allocate_exact_size(vec2(w, 64.0), Sense::hover());
        let rmid = row.center().y;
        let rcx = row.center().x;
        let play = Rect::from_center_size(pos2(rcx, rmid), vec2(56.0, 56.0));
        // Urutan tombol mengikuti YouTube Music: shuffle, prev, play, next, repeat.
        let shuffle = Rect::from_center_size(pos2(rcx - 140.0, rmid), vec2(22.0, 22.0));
        let prev = Rect::from_center_size(pos2(rcx - 84.0, rmid), vec2(26.0, 26.0));
        let next = Rect::from_center_size(pos2(rcx + 84.0, rmid), vec2(26.0, 26.0));
        let repeat = Rect::from_center_size(pos2(rcx + 140.0, rmid), vec2(22.0, 22.0));

        let ph = ui.interact(prev.expand(8.0), egui::Id::new("np-prev"), Sense::click());
        if ph.hovered() {
            ui.painter()
                .circle_filled(prev.center(), 19.0, s.on_surface.gamma_multiply(0.08));
        }
        draw_icon(ui.painter(), Icon::Prev, prev, s.on_surface);
        if ph.clicked() {
            act = -1;
        }

        let pr_hit = ui.interact(play, egui::Id::new("np-play"), Sense::click());
        let fill = if pr_hit.hovered() {
            s.on_surface.gamma_multiply(0.88)
        } else {
            s.on_surface
        };
        ui.painter().circle_filled(play.center(), 28.0, fill);
        draw_icon(
            ui.painter(),
            if playing { Icon::Pause } else { Icon::Play },
            play.shrink(16.0),
            s.surface,
        );
        if pr_hit.clicked() {
            act = 2;
        }

        let nh = ui.interact(next.expand(8.0), egui::Id::new("np-next"), Sense::click());
        if nh.hovered() {
            ui.painter()
                .circle_filled(next.center(), 19.0, s.on_surface.gamma_multiply(0.08));
        }
        draw_icon(ui.painter(), Icon::Next, next, s.on_surface);
        if nh.clicked() {
            act = 1;
        }

        for (rect, ikon, id, label, nyala) in [
            (shuffle, Icon::Shuffle, "np-shuffle", "acak", self.shuffle_on),
            (repeat, Icon::Repeat, "np-repeat", "ulangi", self.ulangi > 0),
        ] {
            let h = ui.interact(rect.expand(8.0), egui::Id::new(id), Sense::click());
            if h.hovered() {
                ui.painter()
                    .circle_filled(rect.center(), 17.0, s.on_surface.gamma_multiply(0.08));
            }
            draw_ikon(
                ui.painter(),
                ikon,
                rect,
                if nyala { s.primary } else { s.on_surface_variant },
                nyala,
            );
            if h.clicked() {
                if label == "acak" {
                    self.shuffle_queue();
                } else {
                    self.putar_ulang();
                }
            }
        }
        match act {
            -1 => self.next_track(-1),
            1 => self.next_track(1),
            2 => self.toggle_play(),
            _ => {}
        }
    }

    fn lyrics_view(&mut self, ui: &mut Ui, max_h: f32) {
        let s = self.scheme;
        let Some(l) = self.lyrics.clone() else {
            ui.label(ts::body_s("memuat lirik…", s.on_surface_variant));
            // Kalau respons lirik hilang (mis. datang untuk lagu sebelumnya
            // lalu dibuang, atau permintaannya tidak pernah terkirim), panel
            // akan berhenti di "memuat lirik…" selamanya. Minta ulang sendiri
            // tiap beberapa detik alih-alih menunggu pengguna menutup tab.
            let basi = self
                .lyrics_diminta
                .map(|t| t.elapsed() > std::time::Duration::from_secs(3))
                .unwrap_or(true);
            if basi {
                if let Some(cur) = self.current.clone() {
                    self.minta_lirik(cur);
                }
            }
            return;
        };
        if l.synced.is_empty() {
            let txt = if l.text.is_empty() {
                "Tidak ada lirik untuk lagu ini".to_string()
            } else {
                l.text.clone()
            };
            ScrollArea::vertical()
                .id_salt("lyr")
                .max_height(max_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(txt)
                            .size(16.0)
                            .color(s.on_surface_variant),
                    );
                });
            return;
        }
        let pos = self.progress.pos;
        let active = l
            .synced
            .iter()
            .position(|line| line.t > pos)
            .unwrap_or(l.synced.len())
            .saturating_sub(1);
        ScrollArea::vertical()
            .id_salt("lyrsync")
            .max_height(max_h)
            .auto_shrink([false, false])
            .vertical_scroll_offset(((active as f32) * 34.0 - max_h / 2.0).max(0.0))
            .show(ui, |ui| {
                for (i, line) in l.synced.iter().enumerate() {
                    let is_now = i == active;
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(&line.text)
                            .size(if is_now { 20.0 } else { 15.0 })
                            .strong()
                            .color(if is_now { s.on_surface } else { s.on_surface_variant }),
                    );
                    ui.add_space(2.0);
                }
            });
    }

    // ---- beranda

    fn tab_home(&mut self, ui: &mut Ui, compact: bool) {
        let s = self.scheme;
        if self.home.is_empty() && !self.home_err.is_empty() {
            ui.label(ts::body(&self.home_err, s.on_surface_variant));
        }
        if self.home.is_empty() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.add_space(6.0);
                ui.label(ts::body("Memuat beranda…", s.on_surface_variant));
            });
            return;
        }
        let rows = self.home.clone();
        ScrollArea::vertical()
            .id_salt("home")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(10.0);
                // chip suasana, seperti baris mood di YouTube Music
                ScrollArea::horizontal()
                    .id_salt("moods")
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            for m in [
                                "Semangat", "Santai", "Fokus", "Sedih", "Tidur", "Jalan",
                                "Kerja", "Nongkrong",
                            ] {
                                if self.chip(ui, m, self.searched == m).clicked() {
                                    self.query = m.to_string();
                                    self.searched = m.to_string();
                                    self.tab = Tab::Cari;
                                    self.results.clear();
                                    self.worker.send(Job::Search(m.to_string()));
                                }
                            }
                        });
                    });

                for (i, row) in rows.iter().enumerate() {
                    ui.add_space(if i == 0 { 18.0 } else { 26.0 });
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&row.title)
                                .size(if compact { 22.0 } else { 30.0 })
                                .strong()
                                .color(s.on_surface),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if self.chip(ui, "Putar semua", true).clicked() {
                                let q: Vec<Track> = row
                                    .items
                                    .iter()
                                    .filter(|c| c.kind == "song")
                                    .map(|c| Track {
                                        id: c.id.clone(),
                                        title: c.title.clone(),
                                        artist: c.sub.split('•').next().unwrap_or("").trim().to_string(),
                                        album: String::new(),
                                        thumb: c.thumb.clone(),
                                        dur: String::new(),
                                        secs: 0.0,
                                    })
                                    .collect();
                                match q.first().cloned() {
                                    Some(first) => {
                                        let all = q.clone();
                                        self.play_track(first, Some(all));
                                    }
                                    None => {
                                        if let Some(c) = row.items.first() {
                                            self.open_card(c);
                                        }
                                    }
                                }
                            }
                        });
                    });
                    ui.add_space(12.0);
                    self.card_grid_sized(ui, &row.items, compact, false);
                }
                ui.add_space(28.0);
            });
    }

    // ---- cari

    fn tab_cari(&mut self, ui: &mut Ui) {
        let s = self.scheme;

        // Saran ditumpuk ke bawah, satu baris per saran. Dulu digeser ke
        // samping, dan di layar ponsel hampir semua saran jadi tidak terbaca.
        if self.show_suggest && !self.suggest.is_empty() {
            let list: Vec<String> = self.suggest.iter().take(8).cloned().collect();
            let mut dipilih: Option<String> = None;
            ui.add_space(10.0);
            egui::ScrollArea::vertical()
                .id_salt("sug")
                .max_height(260.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for sug in list {
                        let (r, h) = ui.allocate_exact_size(
                            vec2(ui.available_width(), 38.0),
                            Sense::click(),
                        );
                        if h.hovered() {
                            ui.painter().rect_filled(
                                r,
                                Rounding::same(10.0),
                                s.on_surface.gamma_multiply(0.06),
                            );
                        }
                        draw_icon(
                            ui.painter(),
                            Icon::Search,
                            Rect::from_center_size(
                                pos2(r.left() + 22.0, r.center().y),
                                vec2(15.0, 15.0),
                            ),
                            s.on_surface_variant,
                        );
                        ui.painter().text(
                            pos2(r.left() + 42.0, r.center().y),
                            Align2::LEFT_CENTER,
                            pas(ui.ctx(), &sug, r.width() - 56.0, 13.5),
                            FontId::proportional(13.5),
                            s.on_surface,
                        );
                        if h.clicked() {
                            dipilih = Some(sug);
                        }
                    }
                });
            if let Some(sug) = dipilih {
                self.query = sug.clone();
                self.searched = sug.clone();
                self.show_suggest = false;
                self.worker.send(Job::Search(sug));
            }
        }

        ui.add_space(12.0);
        if self.results.is_empty() {
            if let Some(e) = self.last_error.clone() {
                ui.label(ts::body(format!("Error: {e}"), s.on_surface_variant));
            } else if !self.searched.is_empty() && !self.show_suggest {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.add_space(6.0);
                    ui.label(ts::body("Mencari…", s.on_surface_variant));
                });
            } else {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Cari di YouTube Music")
                        .size(22.0)
                        .strong()
                        .color(s.on_surface),
                );
                ui.label(ts::body_s(
                    "Ketik judul lagu atau nama artis di kolom atas.",
                    s.on_surface_variant,
                ));
            }
            return;
        }
        ui.label(
            RichText::new(format!("Hasil untuk “{}”", short(&self.searched, 34)))
                .size(22.0)
                .strong()
                .color(s.on_surface),
        );
        ui.add_space(8.0);
        let results = self.results.clone();
        ScrollArea::vertical()
            .id_salt("results")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, t) in results.iter().enumerate() {
                    if self.track_row(ui, t, i + 1, false) {
                        let q = results.clone();
                        self.play_track(t.clone(), Some(q));
                    }
                }
                ui.add_space(20.0);
            });
    }

    // ---- koleksi

    fn tab_koleksi(&mut self, ui: &mut Ui) {
        let s = self.scheme;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Koleksi")
                    .size(24.0)
                    .strong()
                    .color(s.on_surface),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                match self.logged.clone() {
                    Some(name) => {
                        if self.icon_button(ui, Icon::Theme, 30.0, Btn::Ghost).clicked() {
                            self.worker.send(Job::Logout);
                        }
                        ui.label(ts::body_s(short(&name, 24), s.on_surface_variant));
                    }
                    None => {
                        let label = if cfg!(target_os = "android") {
                            "Masuk lewat Google"
                        } else {
                            "Masuk lewat Chromium"
                        };
                        if self.chip(ui, label, true).clicked() {
                            self.worker.send(Job::Login);
                        }
                    }
                }
            });
        });
        ui.label(ts::body_s(
            if self.logged.is_some() {
                "Sesi dibaca dari profil Chromium di mesin ini, bukan dari kata sandi."
            } else if cfg!(target_os = "android") {
                "Login lewat halaman Google di dalam aplikasi. Swara tidak pernah meminta kata sandimu."
            } else {
                "Swara memakai sesi YouTube yang sudah ada di Chromium. Tanpa kata sandi, tanpa halaman login."
            },
            s.on_surface_variant,
        ));
        ui.add_space(10.0);

        if self.view == View::Feed {
            ui.horizontal_wrapped(|ui| {
                if self.chip(ui, "Playlist saya", false).clicked() {
                    if self.logged.is_some() {
                        self.worker.send(Job::Library);
                    } else {
                        self.toast("masuk dulu lewat sesi Chromium");
                    }
                }
                if self.chip(ui, "Lagu disukai", false).clicked() {
                    self.koleksi = None;
                    self.view = View::Collection;
                    self.worker.send(Job::Browse("FEmusic_liked_videos".into()));
                }
            });
            ui.add_space(8.0);
            if self.lib.is_empty() {
                ui.horizontal(|ui| {
                    if self.logged.is_some() {
                        ui.spinner();
                        ui.add_space(6.0);
                        ui.label(ts::body("Memuat playlist akun…", s.on_surface_variant));
                    } else {
                        ui.label(ts::body(
                            "Masuk lewat sesi Chromium untuk melihat playlist akun.",
                            s.on_surface_variant,
                        ));
                    }
                });
            } else {
                let lib = self.lib.clone();
                ScrollArea::vertical()
                    .id_salt("lib")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.card_grid(ui, &lib, false);
                    });
            }
            return;
        }

        let Some(c) = self.koleksi.clone() else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.add_space(6.0);
                ui.label(ts::body("Memuat koleksi…", s.on_surface_variant));
            });
            return;
        };
        // Sampul di kepala koleksi: tanpa ini halaman album terasa kosong, dan
        // itulah yang dulu terlihat seperti "fotonya tidak muncul".
        if !c.thumb.is_empty() {
            let ctx = ui.ctx().clone();
            self.ensure_thumb(&ctx, &c.thumb);
            let ukuran = 96.0;
            let (r, _) = ui.allocate_exact_size(vec2(ukuran, ukuran), Sense::hover());
            ui.painter()
                .rect_filled(r, Rounding::same(shape::RAD_CARD), s.surface_high);
            if let Some(tex) = self.thumb_of(&c.thumb) {
                egui::Image::new((tex.id(), tex.size_vec2()))
                    .fit_to_exact_size(vec2(ukuran, ukuran))
                    .rounding(Rounding::same(shape::RAD_CARD))
                    .paint_at(ui, r);
            }
            ui.add_space(8.0);
        }
        ui.horizontal(|ui| {
            if self.icon_button(ui, Icon::Prev, 32.0, Btn::Ghost).clicked() {
                self.view = View::Feed;
            }
            ui.add_space(4.0);
            ui.label(ts::title(short(&c.title, 30), s.on_surface));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !c.tracks.is_empty() && self.chip(ui, "Putar semua", true).clicked() {
                    let q = c.tracks.clone();
                    if let Some(first) = q.first().cloned() {
                        self.play_track(first, Some(q));
                    }
                }
            });
        });
        if !c.sub.is_empty() {
            ui.label(ts::body_s(short(&c.sub, 70), s.on_surface_variant));
        }
        ui.add_space(6.0);
        let tracks = c.tracks.clone();
        ScrollArea::vertical()
            .id_salt("coll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, t) in tracks.iter().enumerate() {
                    if self.track_row(ui, t, i + 1, true) {
                        let q = tracks.clone();
                        self.play_track(t.clone(), Some(q));
                    }
                }
                if tracks.is_empty() {
                    ui.label(ts::body("Belum ada lagu.", s.on_surface_variant));
                }
                ui.add_space(20.0);
            });
    }

    // ---- tata letak ponsel (mengikuti aplikasi YouTube Music)

    /// Header: logo Music di kiri, lonceng dan avatar di kanan.

    fn mobile_header(&mut self, ui: &mut Ui) {
        let s = self.scheme;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
            ui.painter()
                .circle_filled(r.center(), 13.0, Color32::WHITE);
            draw_icon(
                ui.painter(),
                Icon::Logo,
                Rect::from_center_size(r.center(), vec2(15.0, 15.0)),
                s.on_primary,
            );
            ui.add_space(6.0);
            ui.label(RichText::new("swara").size(17.0).strong().color(s.on_surface));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let logged = self.logged.clone();
                let (av, avr) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
                ui.painter()
                    .circle_filled(av.center(), 15.0, s.surface_highest);
                ui.painter().text(
                    av.center(),
                    Align2::CENTER_CENTER,
                    logged
                        .as_ref()
                        .map(|n| n.chars().next().unwrap_or('?').to_uppercase().to_string())
                        .unwrap_or_else(|| "?".into()),
                    FontId::proportional(12.5),
                    s.on_surface,
                );
                if avr.clicked() {
                    if logged.is_some() {
                        self.worker.send(Job::Logout);
                        self.logged = None;
                        self.toast("keluar dari akun");
                    } else {
                        self.worker.send(Job::Login);
                    }
                }
                ui.add_space(10.0);
                let (b, br) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
                draw_icon(ui.painter(), Icon::Bell, b.shrink(4.0), s.on_surface);
                ui.painter().circle_filled(
                    pos2(b.right() - 4.0, b.top() + 7.0),
                    5.0,
                    Color32::WHITE,
                );
                if br.clicked() {
                    self.toast("belum ada notifikasi baru");
                }
            });
        });
    }

    /// Bilah tab bawah: Beranda, Sampel, Telusuri, Koleksi. Tab "Upgrade" milik
    /// YouTube Music sengaja tidak dipakai -- Swara gratis, tidak ada langganan.

    /// Tiga tab: Beranda, Telusuri, Koleksi. Tab aktif putih penuh dengan
    /// titik 4 px di bawah label.
    fn mobile_nav(&mut self, ui: &mut Ui) {
        let s = self.scheme;
        let full = ui.max_rect();
        let w = full.width();
        let tab: [(Tab, Icon, &str); 3] = [
            (Tab::Home, Icon::Home, "Beranda"),
            (Tab::Cari, Icon::Search, "Telusuri"),
            (Tab::Koleksi, Icon::Library, "Koleksi"),
        ];
        for (i, (t, ikon, label)) in tab.into_iter().enumerate() {
            let cx = full.left() + w * (0.5 + i as f32) / 3.0;
            let akt = self.tab == t;
            let fg = if akt { s.on_surface } else { s.on_surface_variant };
            let r = Rect::from_min_size(pos2(cx - 22.0, full.top() + 8.0), vec2(44.0, 44.0));
            draw_icon(
                ui.painter(),
                ikon,
                Rect::from_center_size(pos2(cx, full.top() + 24.0), vec2(22.0, 22.0)),
                fg,
            );
            ui.painter().text(
                pos2(cx, full.top() + 46.0),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(10.0),
                fg,
            );
            if akt {
                ui.painter().circle_filled(
                    pos2(cx, full.top() + 60.0),
                    2.0,
                    Color32::WHITE,
                );
            }
            let hit = ui.interact(r, egui::Id::new(("mtab", i)), Sense::click());
            if hit.clicked() {
                self.goto(t);
            }
        }
    }

    /// Mini player di atas bilah tab: sampul, judul, cast, putar/jeda.

    /// Mini player 68 px: sampul 48, judul 13.5, artis 11.5, durasi 3 px di dasar.
    fn mobile_mini(&mut self, ui: &mut Ui) {
        let s = self.scheme;
        let ctx = ui.ctx().clone();
        let cur = self.current.clone();
        let Some(t) = cur else { return };
        let full = ui.max_rect();
        let resp = ui.interact(full, egui::Id::new("mmini-hit"), Sense::click());
        self.ensure_thumb(&ctx, &t.thumb);
        let img = Rect::from_min_size(pos2(full.left(), full.center().y - 26.0), vec2(48.0, 48.0));
        ui.painter()
            .rect_filled(img, Rounding::same(shape::RAD_THUMB), s.surface_high);
        if let Some(tex) = self.thumb_of(&t.thumb) {
            egui::Image::new((tex.id(), tex.size_vec2()))
                .fit_to_exact_size(vec2(48.0, 48.0))
                .rounding(Rounding::same(shape::RAD_THUMB))
                .paint_at(ui, img);
        }
        // lebar teks dibatasi tombol putar di kanan, jadi judul tidak menembus
        let pr = Rect::from_center_size(pos2(full.right() - 20.0, full.center().y - 4.0), vec2(22.0, 22.0));
        let teks_w = (pr.left() - img.right() - 24.0).max(60.0);
        ui.painter().text(
            pos2(img.right() + 12.0, full.center().y - 11.0),
            Align2::LEFT_CENTER,
            pas(ui.ctx(), &t.title, teks_w, 13.5),
            FontId::proportional(13.5),
            s.on_surface,
        );
        ui.painter().text(
            pos2(img.right() + 12.0, full.center().y + 9.0),
            Align2::LEFT_CENTER,
            pas(ui.ctx(), &t.artist, teks_w, 11.5),
            FontId::proportional(11.5),
            s.on_surface_variant,
        );
        draw_icon(
            ui.painter(),
            if self.progress.playing {
                Icon::Pause
            } else {
                Icon::Play
            },
            pr,
            s.on_surface,
        );
        let dur = self.progress.dur;
        let frac = if dur > 0.0 {
            (self.progress.pos / dur).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let bar = Rect::from_min_max(
            pos2(full.left(), full.bottom() - 3.0),
            pos2(full.right(), full.bottom()),
        );
        ui.painter().rect_filled(bar, Rounding::ZERO, s.outline);
        ui.painter().rect_filled(
            Rect::from_min_max(bar.min, pos2(bar.left() + bar.width() * frac, bar.bottom())),
            Rounding::ZERO,
            Color32::WHITE,
        );
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                if (p.x - pr.center().x).abs() < 26.0 {
                    self.toggle_play();
                } else {
                    self.expanded = true;
                }
            }
        }
    }

    /// Isi tab Beranda di ponsel: chip suasana, daftar "Pilihan cepat", grid pintasan.
    fn mobile_feed(&mut self, ui: &mut Ui) {
        let s = self.scheme;
        let rows = self.home.clone();
        ScrollArea::vertical()
            .id_salt("mfeed")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ScrollArea::horizontal()
                    .id_salt("mchips")
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            for m in [
                                "Mengisi energi",
                                "Senang",
                                "Bersantai",
                                "Sedih",
                                "Perjalanan",
                            ] {
                                if self.chip(ui, m, self.searched == m).clicked() {
                                    self.query = m.to_string();
                                    self.searched = m.to_string();
                                    self.tab = Tab::Cari;
                                    self.results.clear();
                                    self.worker.send(Job::Search(m.to_string()));
                                }
                            }
                        });
                    });
                ui.add_space(14.0);

                if rows.is_empty() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.add_space(6.0);
                        ui.label(ts::body("Memuat beranda…", s.on_surface_variant));
                    });
                    return;
                }

                // ---- seksi pertama: daftar baris, seperti "Pilihan cepat"
                let baris = rows[0].items.clone();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&rows[0].title)
                            .size(21.0)
                            .strong()
                            .color(s.on_surface),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.chip(ui, "Putar semua", false).clicked() {
                            let q: Vec<Track> = baris
                                .iter()
                                .filter(|c| c.kind == "song")
                                .map(|c| Track {
                                    id: c.id.clone(),
                                    title: c.title.clone(),
                                    artist: c
                                        .sub
                                        .split('•')
                                        .next()
                                        .unwrap_or("")
                                        .trim()
                                        .to_string(),
                                    album: String::new(),
                                    thumb: c.thumb.clone(),
                                    dur: String::new(),
                                    secs: 0.0,
                                })
                                .collect();
                            if let Some(first) = q.first().cloned() {
                                self.play_track(first, Some(q));
                            } else if let Some(c) = baris.first() {
                                self.open_card(c);
                            }
                        }
                    });
                });
                ui.add_space(6.0);
                for (i, c) in baris.iter().take(6).enumerate() {
                    if self.mobile_row(ui, c, i) {
                        self.open_card(c);
                    }
                }

                // ---- seksi berikutnya: grid 3 kolom dengan judul di atas gambar.
                // Dulu cuma tiga seksi, jadi beranda ponsel jauh lebih pendek
                // daripada desktop tanpa alasan.
                for row in rows.iter().skip(1) {
                    ui.add_space(20.0);
                    ui.horizontal(|ui| {
                        if row.title.to_lowercase().contains("pintasan") {
                            let (av, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                            ui.painter()
                                .circle_filled(av.center(), 11.0, s.surface_highest);
                        }
                        ui.label(
                            RichText::new(&row.title)
                                .size(21.0)
                                .strong()
                                .color(s.on_surface),
                        );
                    });
                    ui.add_space(8.0);
                    let gap = 8.0;
                    let avail = ui.available_width();
                    let cw = ((avail - gap * 2.0) / 3.0).floor();
                    for chunk in row.items.chunks(3) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for c in chunk {
                                self.mobile_tile(ui, c, cw);
                            }
                        });
                        ui.add_space(gap);
                    }
                }
                ui.add_space(48.0);
            });
    }

    /// Baris daftar untuk ponsel: sampul, judul, subteks, titik tiga.
    fn mobile_row(&mut self, ui: &mut Ui, c: &Card, idx: usize) -> bool {
        let s = self.scheme;
        let _ = idx;
        let ctx = ui.ctx().clone();
        self.ensure_thumb(&ctx, &c.thumb);
        let playing = self.current.as_ref().map(|t| t.id == c.id).unwrap_or(false);
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 62.0), Sense::click());
        let img = Rect::from_center_size(
            pos2(rect.left() + 26.0, rect.center().y),
            vec2(52.0, 52.0),
        );
        ui.painter()
            .rect_filled(img, Rounding::same(6.0), s.surface_high);
        if let Some(tex) = self.thumb_of(&c.thumb) {
            egui::Image::new((tex.id(), tex.size_vec2()))
                .fit_to_exact_size(vec2(52.0, 52.0))
                .rounding(Rounding::same(6.0))
                .paint_at(ui, img);
        }
        let x = img.right() + 12.0;
        ui.painter().text(
            pos2(x, rect.center().y - 10.0),
            Align2::LEFT_CENTER,
            pas(ui.ctx(), &c.title, ui.available_width() - 96.0, 18.0),
            FontId::proportional(14.5),
            if playing { s.primary } else { s.on_surface },
        );
        ui.painter().text(
            pos2(x, rect.center().y + 10.0),
            Align2::LEFT_CENTER,
            // YouTube Music cuma menampilkan artis + jumlah pemutaran; potongan
            // lirik di segmen ketiga membuat subteks sesak dan kepotong.
            short(
                &c.sub
                    .split('•')
                    .take(2)
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join(" · "),
                30,
            ),
            FontId::proportional(12.0),
            s.on_surface_variant,
        );
        draw_icon(
            ui.painter(),
            Icon::More,
            Rect::from_center_size(
                pos2(rect.right() - 16.0, rect.center().y),
                vec2(18.0, 18.0),
            ),
            s.on_surface_variant,
        );
        resp.clicked()
    }

    /// Kartu persegi 3 kolom, judul menempel di bawah gambar.
    fn mobile_tile(&mut self, ui: &mut Ui, c: &Card, w: f32) -> bool {
        let s = self.scheme;
        let ctx = ui.ctx().clone();
        self.ensure_thumb(&ctx, &c.thumb);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, w), Sense::click());
        ui.painter()
            .rect_filled(rect, Rounding::same(6.0), s.surface_high);
        if let Some(tex) = self.thumb_of(&c.thumb) {
            egui::Image::new((tex.id(), tex.size_vec2()))
                .fit_to_exact_size(vec2(w, w))
                .rounding(Rounding::same(6.0))
                .paint_at(ui, rect);
        }
        // gradasi gelap di kaki kartu supaya judul terbaca
        let kaki = Rect::from_min_max(pos2(rect.left(), rect.bottom() - w * 0.42), rect.max);
        let mut mesh = egui::Mesh::default();
        let atas = s.surface.gamma_multiply(0.0);
        let bawah = Color32::from_black_alpha(190);
        mesh.colored_vertex(kaki.left_top(), atas);
        mesh.colored_vertex(kaki.right_top(), atas);
        mesh.colored_vertex(kaki.left_bottom(), bawah);
        mesh.colored_vertex(kaki.right_bottom(), bawah);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 3, 2);
        ui.painter().add(Shape::mesh(mesh));
        ui.painter().text(
            pos2(rect.center().x, rect.bottom() - 8.0),
            Align2::CENTER_BOTTOM,
            short(&c.title, (w / 7.4).max(8.0) as usize),
            FontId::proportional(11.5),
            Color32::WHITE,
        );
        if resp.clicked() {
            self.open_card(c);
            return true;
        }
        false
    }

    /// Kerangka layar ponsel: header, isi tab, mini player, bilah tab.

    fn mobile(&mut self, ctx: &egui::Context, _w: f32) {
        let s = self.scheme;
        let bawah = self.inset_bawah;
        // Nav bawah 68 px: ikon 22, label 10, titik 4 px untuk tab aktif.
        egui::TopBottomPanel::bottom("mnav")
            .resizable(false)
            .exact_height(68.0 + bawah)
            .frame(
                egui::Frame::none()
                    .fill(s.surface_low)
                    .stroke(egui::Stroke::new(1.0, s.outline_variant))
                    .inner_margin(egui::Margin {
                        left: 0.0,
                        right: 0.0,
                        top: 0.0,
                        bottom: bawah,
                    }),
            )
            .show(ctx, |ui| self.mobile_nav(ui));

        // Mini player menempel di atas nav, tinggi 68 px.
        if self.current.is_some() {
            egui::TopBottomPanel::bottom("mmini")
                .resizable(false)
                .exact_height(68.0)
                .frame(
                    egui::Frame::none()
                        .fill(s.surface_low)
                        .inner_margin(egui::Margin::symmetric(12.0, 0.0)),
                )
                .show(ctx, |ui| self.mobile_mini(ui));
        }

        let atas = self.inset_atas;
        egui::TopBottomPanel::top("mhead")
            .resizable(false)
            .exact_height(76.0 + atas)
            .frame(
                egui::Frame::none()
                    .fill(s.surface)
                    .inner_margin(egui::Margin {
                        left: 16.0,
                        right: 16.0,
                        top: 34.0 + atas,
                        bottom: 0.0,
                    }),
            )
            .show(ctx, |ui| self.mobile_header(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(s.surface)
                    .inner_margin(egui::Margin::symmetric(16.0, 0.0)),
            )
            .show(ctx, |ui| match self.tab {
                Tab::Home => self.mobile_feed(ui),
                Tab::Cari => {
                    self.mobile_search(ui);
                }
                Tab::Koleksi => self.tab_koleksi(ui),
            });
    }

    /// Kolom cari di ponsel, di atas hasil pencarian.
    fn mobile_search(&mut self, ui: &mut Ui) {
        let s = self.scheme;
        let mut submit = false;
        ui.add_space(6.0);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, Rounding::same(21.0), s.surface_high);
        draw_icon(
            ui.painter(),
            Icon::Search,
            Rect::from_center_size(pos2(rect.left() + 22.0, rect.center().y), vec2(18.0, 18.0)),
            s.on_surface_variant,
        );
        let te_rect = Rect::from_min_max(
            pos2(rect.left() + 40.0, rect.top() + 11.0),
            pos2(rect.right() - 14.0, rect.bottom() - 11.0),
        );
        let te = egui::TextEdit::singleline(&mut self.query)
            .frame(false)
            .margin(vec2(0.0, 0.0));
        let resp = ui.put(te_rect, te);
        self.search_focus = resp.has_focus();
        if self.query.is_empty() && !resp.has_focus() {
            ui.painter().text(
                pos2(te_rect.left(), te_rect.center().y),
                Align2::LEFT_CENTER,
                "Cari lagu, album, artis",
                FontId::proportional(13.0),
                s.on_surface_variant,
            );
        }
        if self.focus_cari {
            resp.request_focus();
            self.focus_cari = false;
        }
        if resp.changed() {
            self.suggest_at = Some(Instant::now());
            self.show_suggest = true;
            self.cari_at = Some(Instant::now());
        }
        let tekan_enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if tekan_enter && (resp.has_focus() || resp.lost_focus()) {
            submit = true;
        }
        // Cari sendiri setelah berhenti mengetik: papan ketik Android tidak
        // mengirim Enter ke egui, dulu kolom ini diam saja setelah diketik.
        // Saran dibiarkan tampil lebih dulu — dulu pencarian langsung jalan
        // setelah 800 ms sehingga daftar saran cuma berkelebat.
        if let Some(at) = self.cari_at {
            let tunggu = if self.show_suggest && !self.suggest.is_empty() {
                Duration::from_millis(2600)
            } else {
                Duration::from_millis(900)
            };
            if at.elapsed() > tunggu {
                self.cari_at = None;
                let q = self.query.trim().to_string();
                if !q.is_empty() && q != self.searched {
                    submit = true;
                }
            }
        }
        if submit && !self.query.is_empty() {
            self.searched = self.query.clone();
            self.show_suggest = false;
            self.results.clear();
            self.last_error = None;
            self.worker.send(Job::Search(self.query.clone()));
        }
        ui.add_space(8.0);
        self.tab_cari(ui);
    }

    /// Pemutar penuh di ponsel: sampul besar, aksi, bilah durasi, kontrol.

    /// Pemutar penuh ponsel: sampul selebar layar dengan gradasi pudar di tepi,
    /// judul 24, chip kapital, durasi, lalu transport dengan tombol putar 60 px.
    fn mobile_player(&mut self, ctx: &egui::Context) {
        let s = self.scheme;
        let cur = self.current.clone();
        let dur = self.progress.dur;
        let mut tutup = false;
        let mut seek_ke: Option<f64> = None;
        let atas = self.inset_atas;
        let bawah = self.inset_bawah;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none().fill(s.surface).inner_margin(egui::Margin {
                    left: 0.0,
                    right: 0.0,
                    top: atas,
                    bottom: bawah,
                }),
            )
            .show(ctx, |ui| {
                let full = ui.max_rect();
                let w = full.width();

                // ---- baris atas: panah tutup
                let chev = Rect::from_min_size(pos2(full.left() + 16.0, full.top() + 10.0), vec2(32.0, 32.0));
                draw_icon(ui.painter(), Icon::ChevronDown, chev.shrink(6.0), s.on_surface);
                if ui
                    .interact(chev, egui::Id::new("mp-tutup"), Sense::click())
                    .clicked()
                {
                    tutup = true;
                }

                // ---- sampul + gradasi tepi (54 px), hanya saat panel tertutup
                let panel = self.show_lyrics || self.show_queue;
                let mut y = chev.bottom() + 6.0;
                if !panel {
                    let sisi = (w * 0.94).min((full.height() - 300.0).max(120.0));
                    let img = Rect::from_center_size(pos2(full.center().x, y + sisi / 2.0), vec2(sisi, sisi));
                    ui.painter()
                        .rect_filled(img, Rounding::same(shape::CARD), s.surface_high);
                    if let Some(t) = &cur {
                        let ctx2 = ui.ctx().clone();
                        self.ensure_thumb(&ctx2, &t.thumb);
                        if let Some(tex) = self.thumb_of(&t.thumb) {
                            egui::Image::new((tex.id(), tex.size_vec2()))
                                .fit_to_exact_size(img.size())
                                .rounding(Rounding::same(shape::CARD))
                                .paint_at(ui, img);
                        }
                    }
                    // gradasi: tumpukan baris warna latar dengan alfa menurun
                    for i in 0..18 {
                        let a = 0.08 + 0.05 * (18 - i) as f32 / 18.0;
                        let yy = img.top() + i as f32 * 2.0;
                        ui.painter().rect_filled(
                            Rect::from_min_max(pos2(img.left(), yy), pos2(img.right(), yy + 2.0)),
                            Rounding::ZERO,
                            s.surface.gamma_multiply(a),
                        );
                    }
                    y = img.bottom() + 18.0;
                }

                // ---- judul, artis
                let pad = 20.0;
                let judul = cur.as_ref().map(|t| t.title.clone()).unwrap_or_default();
                let artis = cur.as_ref().map(|t| t.artist.clone()).unwrap_or_default();
                let teks_w = w - pad * 2.0;
                ui.painter().text(
                    pos2(full.left() + pad, y + 8.0),
                    Align2::LEFT_CENTER,
                    pas(ui.ctx(), &judul, teks_w, 24.0),
                    FontId::proportional(24.0),
                    s.on_surface,
                );
                ui.painter().text(
                    pos2(full.left() + pad, y + 34.0),
                    Align2::LEFT_CENTER,
                    pas(ui.ctx(), &artis, teks_w, 12.0),
                    FontId::proportional(12.0),
                    s.on_surface_variant,
                );
                y += 54.0;

                // ---- chip aksi (kapital, pil 32 px)
                let aksi: [(Icon, &str); 4] = [
                    (Icon::Like, "Suka"),
                    (Icon::Dislike, "Tidak"),
                    (Icon::Lyrics, "Lirik"),
                    (Icon::Queue, "Antrean"),
                ];
                let nilai = self
                    .current
                    .as_ref()
                    .and_then(|t| self.rating.get(&t.id).copied())
                    .unwrap_or(0);
                let mut x = full.left() + pad;
                for (ikon, label) in aksi {
                    let nyala = match label {
                        "Suka" => nilai == 1,
                        "Tidak" => nilai == -1,
                        "Lirik" => self.show_lyrics && !self.show_queue,
                        "Antrean" => self.show_queue && !self.show_lyrics,
                        _ => false,
                    };
                    let lebar = 34.0 + label.chars().count() as f32 * 7.0;
                    if x + lebar > full.right() - pad {
                        break;
                    }
                    let r = Rect::from_min_size(pos2(x, y), vec2(lebar, 32.0));
                    let latar = if nyala { Color32::WHITE } else { s.surface_high };
                    let fg = if nyala { s.on_primary } else { s.on_surface };
                    ui.painter().rect_filled(r, Rounding::same(shape::CHIP), latar);
                    draw_ikon(
                        ui.painter(),
                        ikon,
                        Rect::from_center_size(pos2(r.left() + 17.0, r.center().y), vec2(15.0, 15.0)),
                        fg,
                        nyala,
                    );
                    ui.painter().text(
                        pos2(r.left() + 29.0, r.center().y),
                        Align2::LEFT_CENTER,
                        label.to_uppercase(),
                        FontId::proportional(10.5),
                        fg,
                    );
                    let hit = ui.interact(r, egui::Id::new(("mp-chip", label)), Sense::click());
                    if hit.clicked() {
                        match label {
                            "Suka" => self.nilai_lagu(1),
                            "Tidak" => self.nilai_lagu(-1),
                            "Lirik" => {
                                let buka = !(self.show_lyrics && !self.show_queue);
                                self.show_lyrics = buka;
                                self.show_queue = false;
                            }
                            _ => {
                                let buka = !(self.show_queue && !self.show_lyrics);
                                self.show_queue = buka;
                                self.show_lyrics = false;
                            }
                        }
                    }
                    x += lebar + 8.0;
                }
                y += 44.0;

                // ---- panel lirik / antrean
                if panel {
                    let h = (full.bottom() - y - 130.0).clamp(120.0, 520.0);
                    ui.allocate_ui_with_layout(
                        vec2(teks_w, h),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(teks_w);
                            self.side_panel_body(ui, h);
                        },
                    );
                }

                // ---- durasi dan transport menempel di bawah
                let bawah_y = full.bottom() - 10.0;
                let lebar_track = (w - pad * 2.0).min(560.0);
                let ty = bawah_y - 96.0;
                let track_rect =
                    Rect::from_center_size(pos2(full.center().x, ty), vec2(lebar_track, 3.0));
                ui.painter()
                    .rect_filled(track_rect, Rounding::same(1.5), s.outline);
                let pos_now = self.drag_pos.unwrap_or(self.progress.pos);
                if dur > 0.0 {
                    let f = (pos_now / dur).clamp(0.0, 1.0) as f32;
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            track_rect.min,
                            pos2(track_rect.left() + track_rect.width() * f, track_rect.bottom()),
                        ),
                        Rounding::same(1.5),
                        Color32::WHITE,
                    );
                }
                let hit = ui.interact(
                    track_rect.expand2(vec2(0.0, 14.0)),
                    egui::Id::new("mp-seek"),
                    Sense::click(),
                );
                if hit.clicked() && dur > 0.0 {
                    if let Some(p) = hit.interact_pointer_pos() {
                        let f = ((p.x - track_rect.left()) / track_rect.width().max(1.0))
                            .clamp(0.0, 1.0);
                        seek_ke = Some(f as f64 * dur);
                    }
                }
                ui.painter().text(
                    pos2(track_rect.left(), ty + 16.0),
                    Align2::LEFT_CENTER,
                    fmt_time(pos_now),
                    FontId::proportional(10.5),
                    s.on_surface_variant,
                );
                ui.painter().text(
                    pos2(track_rect.right(), ty + 16.0),
                    Align2::RIGHT_CENTER,
                    fmt_time(dur),
                    FontId::proportional(10.5),
                    s.on_surface_variant,
                );

                let cy = bawah_y - 44.0;
                let xs = [
                    full.left() + pad + 12.0,
                    full.center().x - 64.0,
                    full.center().x,
                    full.center().x + 64.0,
                    full.right() - pad - 12.0,
                ];
                for (i, ikon) in [Icon::Shuffle, Icon::Prev, Icon::Play, Icon::Next, Icon::Repeat]
                    .into_iter()
                    .enumerate()
                {
                    if i == 2 {
                        let r = Rect::from_center_size(pos2(xs[2], cy), vec2(60.0, 60.0));
                        ui.painter().circle_filled(r.center(), 30.0, Color32::WHITE);
                        draw_icon(
                            ui.painter(),
                            if self.progress.playing {
                                Icon::Pause
                            } else {
                                Icon::Play
                            },
                            Rect::from_center_size(r.center(), vec2(22.0, 22.0)),
                            s.on_primary,
                        );
                        let hitr = ui.interact(r, egui::Id::new("mplay"), Sense::click());
                        if hitr.clicked() {
                            self.toggle_play();
                        }
                        continue;
                    }
                    let r = Rect::from_center_size(pos2(xs[i], cy), vec2(40.0, 40.0));
                    let nyala = (i == 0 && self.shuffle_on) || (i == 4 && self.ulangi > 0);
                    let warna = if nyala { s.on_surface } else { s.on_surface_variant };
                    draw_ikon(
                        ui.painter(),
                        ikon,
                        Rect::from_center_size(r.center(), vec2(if i == 1 || i == 3 { 24.0 } else { 20.0 }, if i == 1 || i == 3 { 24.0 } else { 20.0 })),
                        warna,
                        nyala,
                    );
                    let hitr = ui.interact(r, egui::Id::new(("mctl", i)), Sense::click());
                    if hitr.clicked() {
                        match i {
                            0 => self.shuffle_queue(),
                            1 => self.next_track(-1),
                            3 => self.next_track(1),
                            _ => self.putar_ulang(),
                        }
                    }
                }
            });
        if let Some(v) = seek_ke {
            self.player.seek(v);
        }
        if tutup {
            self.expanded = false;
        }
    }
}

// ------------------------------------------------------------------ util

/// Ikon di panel bawah ponsel: digambar dengan painter, label di bawahnya.
fn nav_item(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    active: bool,
    fg: Color32,
    redup: Color32,
    w: f32,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 50.0), Sense::click());
    let c = if active { fg } else { redup };
    draw_icon(
        ui.painter(),
        icon,
        Rect::from_center_size(pos2(rect.center().x, rect.center().y - 8.0), vec2(23.0, 23.0)),
        c,
    );
    ui.painter().text(
        pos2(rect.center().x, rect.bottom() - 9.0),
        Align2::CENTER_BOTTOM,
        label,
        FontId::proportional(10.0),
        c,
    );
    // Tab aktif ditandai pil putih kecil di bawah label, sama seperti di
    // aplikasi YouTube Music -- warnanya saja tidak cukup di layar gelap.
    if active {
        ui.painter().rect_filled(
            Rect::from_center_size(pos2(rect.center().x, rect.bottom() - 1.5), vec2(18.0, 3.0)),
            Rounding::same(1.5),
            fg,
        );
    }
    resp.clicked()
}

/// Baris berkas untuk kotak unggah (gambar/video) di layar ponsel.
fn berkas_baris(ui: &mut Ui, nama: &str, ukuran: f64, warna: Color32, teks: Color32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 54.0), Sense::click());
    let bulat = Rect::from_min_size(
        pos2(rect.left(), rect.center().y - 20.0),
        vec2(40.0, 40.0),
    );
    ui.painter().rect_filled(bulat, Rounding::same(8.0), warna);
    let (ikon, ket) = if nama.to_lowercase().ends_with(".mp4")
        || nama.to_lowercase().ends_with(".mkv")
        || nama.to_lowercase().ends_with(".webm")
    {
        (Icon::Play, "video")
    } else {
        (Icon::Upgrade, "gambar")
    };
    draw_icon(ui.painter(), ikon, Rect::from_center_size(bulat.center(), vec2(18.0, 18.0)), teks);
    ui.painter().text(
        pos2(bulat.right() + 12.0, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        short(nama, 34),
        FontId::proportional(13.5),
        teks,
    );
    ui.painter().text(
        pos2(bulat.right() + 12.0, rect.center().y + 10.0),
        Align2::LEFT_CENTER,
        format!("{ket} • {:.0} KB", ukuran / 1024.0),
        FontId::proportional(11.5),
        teks.gamma_multiply(0.75),
    );
    resp.clicked()
}

/// Potong teks supaya benar-benar muat di `lebar` px pada ukuran font itu.
/// Dipakai di semua tempat yang menggambar teks dengan `painter().text`,
/// karena teks seperti itu tidak pernah dibungkus dan bisa menembus induknya.
///
/// Diukur dengan lebar font asli, bukan jumlah karakter: huruf lebar seperti
/// "W" dan "M" membuat hitungan karakter meleset. Hasilnya di-cache karena
/// teks yang sama digambar tiap frame.
fn pas(ctx: &egui::Context, teks: &str, lebar: f32, size: f32) -> String {
    use std::cell::RefCell;
    use std::collections::HashMap;
    thread_local! {
        static CACHE: RefCell<HashMap<(String, u32, u32), String>> = RefCell::new(HashMap::new());
    }
    if lebar <= 8.0 {
        return String::new();
    }
    let kunci = (teks.to_owned(), lebar.round() as u32, (size * 10.0) as u32);
    if let Some(hit) = CACHE.with(|c| c.borrow().get(&kunci).cloned()) {
        return hit;
    }
    let font = FontId::proportional(size);
    let ukur = |s: &str| -> f32 {
        ctx.fonts(|f| {
            f.layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE)
                .size()
                .x
        })
    };
    let hasil = if ukur(teks) <= lebar {
        teks.to_owned()
    } else {
        // Bagi dua jumlah karakter: beberapa pengukuran saja, bukan satu per huruf.
        let huruf: Vec<char> = teks.chars().collect();
        let (mut lo, mut hi) = (0usize, huruf.len());
        while lo < hi {
            let mid = (lo + hi + 1) / 2;
            let coba: String = huruf[..mid].iter().collect::<String>() + "…";
            if ukur(&coba) <= lebar {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let potong: String = huruf[..lo].iter().collect();
        format!("{}…", potong.trim_end())
    };
    CACHE.with(|c| {
        let mut m = c.borrow_mut();
        if m.len() > 512 {
            m.clear();
        }
        m.insert(kunci, hasil.clone());
    });
    hasil
}

fn short(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn fmt_time(s: f64) -> String {
    let s = s.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn hash_name(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn cover_seed(img: &image::DynamicImage) -> Option<[u8; 3]> {
    let small = img
        .resize(8, 8, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let mut acc = [0u32; 3];
    let mut n = 0u32;
    for p in small.pixels() {
        acc[0] += p[0] as u32;
        acc[1] += p[1] as u32;
        acc[2] += p[2] as u32;
        n += 1;
    }
    if n == 0 {
        None
    } else {
        Some([(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8])
    }
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let candidates = [
        // Linux
        "/usr/share/fonts/roboto/Roboto[ital,wdth,wght].ttf",
        "/usr/share/fonts/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/LiberationSans-Regular.ttf",
        // Windows
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        // macOS
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/PingFang.ttc",
        // Android
        "/system/fonts/Roboto-Regular.ttf",
        "/system/fonts/NotoSansCJK-Regular.ttc",
        "/system/fonts/NotoSansSymbols-Regular.ttf",
    ];
    let mut loaded = Vec::new();
    for (i, path) in candidates.iter().enumerate() {
        if let Ok(bytes) = std::fs::read(path) {
            let name = format!("sys{i}");
            fonts
                .font_data
                .insert(name.clone(), FontData::from_owned(bytes));
            loaded.push(name);
        }
    }
    for fam in [FontFamily::Proportional, FontFamily::Monospace] {
        let list = fonts.families.entry(fam).or_default();
        for name in loaded.iter().rev() {
            list.push(name.clone());
        }
    }
    ctx.set_fonts(fonts);
}

fn load_texture(ctx: &egui::Context, name: &str, img: &image::DynamicImage) -> TextureHandle {
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    ctx.load_texture(name.to_string(), color, Default::default())
}

// ------------------------------------------------------------------ eframe

impl eframe::App for App {
    fn clear_color(&self, _v: &eframe::egui::Visuals) -> [f32; 4] {
        let c = self.scheme.surface;
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            1.0,
        ]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Beberapa frame pertama ukuran inset belum benar (pernah terbaca dua
        // kali lipat tinggi bilah status). Ambil nilai terkecil selama 40
        // frame pertama, lalu berhenti membaca.
        #[cfg(target_os = "android")]
        if self.inset_coba < 40 {
            self.inset_coba += 1;
            let ppp = ctx.pixels_per_point().max(0.5);
            let px = crate::jembatan_android::inset_atas_px();
            if px > 0.0 {
                let titik = px / ppp;
                if self.inset_atas == 0.0 || titik < self.inset_atas {
                    self.inset_atas = titik;
                    eprintln!("[swara] inset atas {:.1} titik", self.inset_atas);
                }
            }
            let pb = crate::jembatan_android::inset_bawah_px();
            if pb > 0.0 {
                let titik = pb / ppp;
                if self.inset_bawah == 0.0 || titik < self.inset_bawah {
                    self.inset_bawah = titik;
                    eprintln!("[swara] inset bawah {:.1} titik", self.inset_bawah);
                }
            }
        }
        self.sinkron_suka();
        // Tombol dari notifikasi, layar kunci, dan headset Android.
        #[cfg(target_os = "android")]
        {
            use crate::jembatan_android as ja;
            ja::detak();
            for aksi in ja::ambil_aksi() {
                match aksi {
                    ja::AKSI_TOGGLE => self.toggle_play(),
                    ja::AKSI_PAUSE => {
                        if self.progress.playing {
                            self.player.pause();
                        }
                    }
                    ja::AKSI_PLAY => {
                        if !self.progress.playing {
                            self.toggle_play();
                        }
                    }
                    ja::AKSI_SUKA => self.nilai_lagu(1),
                    ja::AKSI_NEXT => self.next_track(1),
                    ja::AKSI_PREV => self.next_track(-1),
                    _ => {}
                }
            }
            // Keadaan untuk kartu pemutar bawaan Android.
            let (judul, artis, sampul) = self
                .current
                .as_ref()
                .map(|c| (c.title.clone(), c.artist.clone(), c.thumb.clone()))
                .unwrap_or_default();
            let suka = self
                .current
                .as_ref()
                .map(|c| self.rating.get(&c.id).copied().unwrap_or(0) == 1)
                .unwrap_or(false);
            ja::perbarui_media(
                &judul,
                &artis,
                &sampul,
                suka,
                self.progress.playing,
                self.progress.pos,
                self.progress.dur,
            );
        }
        while let Ok(out) = self.worker.rx.try_recv() {
            self.apply_out(out);
        }
        while let Ok((u, got)) = self.thumb_rx.try_recv() {
            self.thumb_pending.remove(&u);
            if let Some((bytes, tex)) = got {
                let is_cover = self.current.as_ref().map(|c| c.thumb == u).unwrap_or(false);
                if is_cover && self.seed.is_none() {
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        self.set_seed(&img);
                    }
                }
                self.thumbs.insert(u, Some(tex));
            }
        }

        // Sampul lagu aktif harus tetap diminta walau mode daftar tidak tampak.
        if let Some(t) = self.current.clone() {
            if !t.thumb.is_empty() {
                self.ensure_thumb(ctx, &t.thumb);
            }
        }

        let prog = self.player.poll();
        if let Some(e) = prog.error.clone() {
            if self.last_error.as_deref() != Some(e.as_str()) {
                self.last_error = Some(e.clone());
                self.toast(format!("gagal memutar: {e}"));
            }
        }
        // Selama lagu baru dimuat, jangan biarkan posisi/durasi lagu lama
        // menempel di bar durasi.
        self.progress = if self.loading && !prog.playing {
            Progress {
                volume: prog.volume,
                ..Default::default()
            }
        } else {
            prog
        };
        // Lagu berikutnya dipilih konduktor, bukan di sini: kalau jendela
        // tersembunyi, `update` tidak dipanggil sama sekali.

        // Konduktor memutar lagu baru tanpa lewat UI; ambil alih tampilannya.
        let adopted = {
            let g = self.cond.lock().unwrap();
            if g.seq != self.cond_seq && g.seq > 0 {
                let id = g.seq;
                let t = g.track.clone();
                let s = g.stream.clone();
                Some((id, t, s))
            } else {
                None
            }
        };
        if let Some((seq, Some(t), s)) = adopted {
            self.cond_seq = seq;
            self.adopt(t, s);
        }

        // Uji manual: SWARA_NEXT=12 menekan "lagu berikutnya" sekali setelah
        // 12 detik, untuk menguji jalur lirik pada pergantian lagu.
        if let Some((detik, mulai)) = self.debug_next {
            if mulai.elapsed().as_secs_f64() >= detik && self.progress.playing {
                self.debug_next = None;
                eprintln!("[swara] uji: menekan next");
                self.next_track(1);
            }
        }

        // Uji manual: SWARA_SEEK=120 melompat sekali setelah pemutaran jalan.
        if let Some(v) = self.debug_seek.take() {
            if !self.loading && self.progress.dur > 0.0 && self.progress.pos > 2.0 {
                self.player.seek(v);
                self.progress.pos = v;
            } else {
                self.debug_seek = Some(v);
            }
        }

        // Kendali dari panel sistem (islet, playerctl, waybar) ditangani
        // `bridge.rs`; di sini tinggal memantulkan keadaan ke D-Bus saat UI
        // sendiri yang mengganti lagu. Posisi/status dikabarkan jembatan, yang
        // tetap hidup walau jendela tidak menggambar.
        if self.loading || self.progress.playing || self.toast.is_some() {
            ctx.request_repaint_after(Duration::from_millis(80));
        }

        // Tetap bangun walau jendela disembunyikan di workspace lain. Tanpa
        // detak ini egui hanya menggambar saat ada input, jadi lagu yang habis
        // tidak pernah lanjut ke antrean berikutnya sampai jendelanya dibuka.
        if std::env::var("SWARA_TICK_DEBUG").is_ok() {
            eprintln!("[tick] pos={:.1} fps_ok", self.progress.pos);
        }
        let tick = if self.current.is_some() { 200 } else { 1000 };
        ctx.request_repaint_after(std::time::Duration::from_millis(tick));

        // Semua pintasan huruf diabaikan saat ada kolom teks yang fokus, jadi
        // mengetik "f" di pencarian tidak lagi membuka pemutar penuh.
        let typing =
            self.search_focus || ctx.wants_keyboard_input() || ctx.memory(|m| m.focused().is_some());
        ctx.input(|i| {
            if !typing && i.key_pressed(egui::Key::F) {
                self.expanded = !self.expanded;
            }
            if i.key_pressed(egui::Key::Escape) {
                self.expanded = false;
            }
        });
        if !typing {
            let mut seek_delta = 0.0;
            let mut step = 0;
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Space) {
                    step = 99;
                }
                if i.key_pressed(egui::Key::ArrowRight) {
                    seek_delta = 5.0;
                }
                if i.key_pressed(egui::Key::ArrowLeft) {
                    seek_delta = -5.0;
                }
                if i.key_pressed(egui::Key::N) {
                    step = 1;
                }
                if i.key_pressed(egui::Key::P) {
                    step = -1;
                }
            });
            if step == 99 {
                self.toggle_play();
            } else if step != 0 {
                self.next_track(step);
            }
            if seek_delta != 0.0 {
                let target = (self.progress.pos + seek_delta).max(0.0);
                self.player.seek(target);
            }
        }

        let s = self.scheme;
        let mut style = (*ctx.style()).clone();
        style.visuals = if s.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        style.visuals.panel_fill = s.surface;
        style.visuals.window_fill = s.surface_low;
        style.visuals.extreme_bg_color = s.surface_highest;
        style.visuals.override_text_color = Some(s.on_surface);
        style.visuals.hyperlink_color = s.primary;
        style.visuals.selection.bg_fill = s.primary.gamma_multiply(0.35);
        style.spacing.item_spacing = vec2(8.0, 8.0);
        style.spacing.button_padding = vec2(14.0, 7.0);
        for w in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.noninteractive,
        ] {
            w.bg_fill = s.surface_highest;
            w.weak_bg_fill = s.surface_highest;
            w.bg_stroke = Stroke::NONE;
            w.fg_stroke = Stroke::new(1.0_f32, s.on_surface);
            w.rounding = Rounding::same(shape::CHIP);
        }
        style.visuals.widgets.hovered.bg_fill = s.surface_highest;
        style.visuals.widgets.active.bg_fill = s.surface_highest;
        ctx.set_style(style);

        let lebar = ctx.screen_rect().width();
        if lebar < 800.0 || cfg!(target_os = "android") {
            // Di layar ponsel tata letaknya mengikuti aplikasi YouTube Music:
            // header, chip suasana, daftar, grid pintasan, mini player, tab bawah.
            if self.expanded {
                self.mobile_player(ctx);
            } else {
                self.mobile(ctx, lebar);
            }
        } else {
            self.ui(ctx);
        }
    }
}

impl App {
    /// Samakan peta `rating` dengan keadaan yang dibagi ke pengendali luar.
    /// Dua arah: lagu baru kita laporkan keluar, perubahan dari soket kita
    /// ikuti masuk — kalau tidak, ikon hati di jendela bisa berbeda dengan
    /// yang dilihat islet.
    fn sinkron_suka(&mut self) {
        let Some(id) = self.current.as_ref().map(|t| t.id.clone()) else {
            return;
        };
        let masuk = self.logged.is_some();
        let mut g = self.suka.lock().unwrap();
        g.masuk = masuk;
        if g.id == id {
            if g.suka {
                self.rating.insert(id, 1);
            } else {
                self.rating.remove(&id);
            }
            return;
        }
        let suka = self.rating.get(&id).copied().unwrap_or(0) == 1;
        g.id = id;
        g.suka = suka;
        drop(g);
        crate::ctl::tulis(&self.suka);
    }
}
