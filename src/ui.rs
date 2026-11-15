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
    searched: String,
    focus_cari: bool,

    lyrics: Option<Lyrics>,
    lyrics_id: String,
    show_lyrics: bool,
    show_queue: bool,
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
    cond: Arc<Mutex<crate::conductor::Cond>>,
    cond_seq: u64,
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
            searched: String::new(),
            focus_cari: false,
            lyrics: None,
            lyrics_id: String::new(),
            show_lyrics: false,
            show_queue: false,
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
                self.worker.send(Job::Lyrics(track));
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
        self.worker.send(Job::Lyrics(t));
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
        let idx = self.queue.iter().position(|t| t.id == cur.id);
        let next = match idx {
            Some(i) => (i as isize + step).rem_euclid(len) as usize,
            None => 0,
        };
        if let Some(t) = self.queue.get(next).cloned() {
            if t.id != cur.id {
                self.play_track(t, None);
            }
        }
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
        let side = if dense { 36.0 } else { 40.0 };
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
        if resp.hovered() {
            ui.painter()
                .rect_filled(rect, Rounding::same(8.0), s.on_surface.gamma_multiply(0.06));
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
        let dur_w = 44.0;
        let text_w = (rect.right() - x - dur_w - 8.0).max(60.0);
        let chars = (text_w / 7.2).max(8.0) as usize;
        ui.painter().text(
            pos2(x, mid - 8.0),
            Align2::LEFT_CENTER,
            short(&t.title, chars),
            FontId::proportional(13.5),
            if playing_this { s.primary } else { s.on_surface },
        );
        ui.painter().text(
            pos2(x, mid + 9.0),
            Align2::LEFT_CENTER,
            short(&t.artist, (chars as f32 * 0.9) as usize),
            FontId::proportional(11.5),
            s.on_surface_variant,
        );
        ui.painter().text(
            pos2(rect.right() - 8.0, mid),
            Align2::RIGHT_CENTER,
            &t.dur,
            FontId::proportional(11.5),
            s.on_surface_variant,
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp.clicked()
    }

    fn card(&mut self, ui: &mut Ui, card: &Card, w: f32) {
        let s = self.scheme;
        let ctx = ui.ctx().clone();
        self.ensure_thumb(&ctx, &card.thumb);
        let tex = self.thumb_of(&card.thumb);
        ui.allocate_ui_with_layout(vec2(w, w + 46.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(w);
            let (rect, resp) = ui.allocate_exact_size(vec2(w, w), Sense::click());
            ui.painter()
                .rect_filled(rect, Rounding::same(shape::RAD_CARD), s.surface_high);
            if let Some(t) = tex {
                egui::Image::new((t.id(), t.size_vec2()))
                    .fit_to_exact_size(vec2(w, w))
                    .rounding(Rounding::same(shape::RAD_CARD))
                    .paint_at(ui, rect);
            } else if !card.thumb.is_empty() {
                let tt = ui.input(|i| i.time) as f32;
                let a = 0.16 + 0.16 * (tt * 1.7).sin().abs();
                ui.painter()
                    .circle_filled(rect.center(), w * 0.045, s.on_surface.gamma_multiply(a));
            }
            if resp.hovered() {
                let c = pos2(rect.right() - 26.0, rect.bottom() - 26.0);
                ui.painter().circle_filled(c, 18.0, s.on_surface);
                draw_icon(
                    ui.painter(),
                    Icon::Play,
                    Rect::from_center_size(c, vec2(16.0, 16.0)),
                    s.surface,
                );
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                self.open_card(card);
            }
            ui.add_space(7.0);
            let chars = (w / 6.6).max(8.0) as usize;
            ui.label(
                RichText::new(short(&card.title, chars))
                    .size(14.0)
                    .color(s.on_surface),
            );
            ui.add_space(1.0);
            ui.label(
                RichText::new(short(&card.sub, chars))
                    .size(12.0)
                    .color(s.on_surface_variant),
            );
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
        // Breakpoint mengikuti YouTube Music: rail lebar >=1312, rail ikon di
        // 800..1312, dan bilah tab di bawah untuk jendela sempit.
        let mobile = w < 800.0;
        let narrow = mobile || w < 1312.0;
        let side_w = if mobile { 0.0 } else if w >= 1312.0 { 240.0 } else { 72.0 };

        // ---- pemutar bawah, selebar jendela seperti YouTube Music
        if self.current.is_some() {
            egui::TopBottomPanel::bottom("bar")
                .resizable(false)
                .exact_height(76.0)
                .frame(
                    egui::Frame::none()
                        .fill(s.surface_low)
                        .inner_margin(egui::Margin::symmetric(18.0, 10.0)),
                )
                .show(ctx, |ui| self.play_bar(ui, w));
        }

        // ---- bilah tab bawah (jendela sempit)
        // ---- bilah tab bawah (mode sempit, seperti aplikasi YT Music)
        if mobile {
            egui::TopBottomPanel::bottom("tabbar")
                .resizable(false)
                .exact_height(54.0)
                .frame(
                    egui::Frame::none()
                        .fill(s.surface_low)
                        .inner_margin(egui::Margin::symmetric(4.0, 4.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let cw = ui.available_width() / 3.0;
                        for t in [Tab::Home, Tab::Cari, Tab::Koleksi] {
                            let active = self.tab == t;
                            let (rect, resp) = ui.allocate_exact_size(
                                vec2(cw - 4.0, 44.0),
                                Sense::click(),
                            );
                            let fg = if active { s.on_surface } else { s.on_surface_variant };
                            draw_icon(
                                ui.painter(),
                                t.icon(),
                                Rect::from_center_size(
                                    pos2(rect.center().x, rect.center().y - 7.0),
                                    vec2(20.0, 20.0),
                                ),
                                fg,
                            );
                            ui.painter().text(
                                pos2(rect.center().x, rect.bottom() - 4.0),
                                Align2::CENTER_BOTTOM,
                                t.label(),
                                FontId::proportional(10.0),
                                fg,
                            );
                            if resp.clicked() {
                                self.goto(t);
                            }
                        }
                    });
                });
        }

        // ---- sidebar kiri
        if !mobile {
            egui::SidePanel::left("nav")
                .resizable(false)
                .exact_width(side_w)
                .frame(
                    egui::Frame::none()
                        .fill(s.surface)
                        .inner_margin(egui::Margin::symmetric(
                            if w >= 1312.0 { 12.0 } else { 6.0 },
                            10.0,
                        )),
                )
                .show(ctx, |ui| self.side_nav(ui, w < 1312.0));

        // ---- bilah atas: kolom cari + akun
        egui::TopBottomPanel::top("top")
            .resizable(false)
            .exact_height(if mobile { 56.0 } else { 64.0 })
            .frame(
                egui::Frame::none().fill(s.surface).inner_margin(egui::Margin::symmetric(
                    if mobile { 12.0 } else { 20.0 },
                    12.0,
                )),
            )
            .show(ctx, |ui| self.top_bar(ui));
        }

        // ---- panel antrean / lirik
        if self.show_queue {
            let qw = if w >= 1320.0 {
                384.0
            } else if w >= 900.0 {
                320.0
            } else {
                (w - 44.0).max(220.0)
            };
            egui::SidePanel::right("queue")
                .resizable(false)
                .exact_width(qw)
                .frame(
                    egui::Frame::none()
                        .fill(s.surface_low)
                        .inner_margin(egui::Margin::symmetric(16.0, 12.0)),
                )
                .show(ctx, |ui| self.queue_panel(ui));
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none().fill(s.surface).inner_margin(egui::Margin::symmetric(
                    if narrow { 14.0 } else { 24.0 },
                    0.0,
                )),
            )
            .show(ctx, |ui| match self.tab {
                Tab::Home => self.tab_home(ui, narrow),
                Tab::Cari => self.tab_cari(ui),
                Tab::Koleksi => self.tab_koleksi(ui),
            });

        // ---- lembar pemutar penuh, naik dari bawah seperti YT Music
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
                            .rounding(Rounding::same(20.0))
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
        let (rect, resp) = ui.allocate_exact_size(vec2(w, if narrow { 54.0 } else { 44.0 }), Sense::click());
        let ind = if narrow {
            Rect::from_center_size(rect.center(), vec2(52.0, 46.0))
        } else {
            rect
        };
        if active {
            ui.painter()
                .rect_filled(ind, Rounding::same(10.0), s.surface_high);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(ind, Rounding::same(10.0), s.on_surface.gamma_multiply(0.07));
        }
        let fg = if active { s.on_surface } else { s.on_surface_variant };
        if narrow {
            draw_icon(
                ui.painter(),
                t.icon(),
                Rect::from_center_size(ind.center(), vec2(21.0, 21.0)),
                fg,
            );
        } else {
            draw_icon(
                ui.painter(),
                t.icon(),
                Rect::from_center_size(pos2(ind.left() + 16.0, ind.center().y), vec2(20.0, 20.0)),
                fg,
            );
            ui.painter().text(
                pos2(ind.left() + 44.0, ind.center().y),
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

        // logo: tombol putar merah + wordmark
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
        let cy = r.center().y;
        ui.painter().circle_filled(
            pos2(r.left() + 14.0, cy),
            11.0,
            Color32::from_rgb(238, 0, 51),
        );
        draw_icon(
            ui.painter(),
            Icon::Play,
            Rect::from_center_size(pos2(r.left() + 14.0, cy), vec2(10.0, 10.0)),
            Color32::WHITE,
        );
        if !narrow {
            ui.painter().text(
                pos2(r.left() + 33.0, cy),
                Align2::LEFT_CENTER,
                "swara",
                FontId::proportional(19.0),
                s.on_surface,
            );
        }
        ui.add_space(4.0);

        for t in [Tab::Home, Tab::Cari, Tab::Koleksi] {
            let active = self.tab == t;
            if self.side_item(ui, t, active, narrow).clicked() {
                self.goto(t);
            }
            ui.add_space(2.0);
        }

        if !narrow {
            ui.add_space(10.0);
            let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter()
                .rect_filled(line, Rounding::same(0.5), s.outline_variant);
            ui.add_space(12.0);
            ui.label(ts::label("PUSTAKA", s.on_surface_variant).size(10.5));
            ui.add_space(4.0);
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

        // kaki sidebar: akun + tema
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            if self.icon_button(ui, Icon::Theme, 34.0, Btn::Ghost).clicked() {
                self.scheme = theme::scheme(!s.dark, self.seed);
            }
            if !narrow {
                ui.add_space(6.0);
                let (rect, resp) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
                let ind = Rect::from_center_size(rect.center(), vec2(rect.width(), 36.0));
                if resp.hovered() {
                    ui.painter()
                        .rect_filled(ind, Rounding::same(10.0), s.on_surface.gamma_multiply(0.07));
                }
                match self.logged.clone() {
                    Some(name) => {
                        ui.painter().circle_filled(
                            pos2(ind.left() + 16.0, ind.center().y),
                            13.0,
                            s.primary_container,
                        );
                        let initial = name
                            .chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .to_string();
                        ui.painter().text(
                            pos2(ind.left() + 16.0, ind.center().y),
                            Align2::CENTER_CENTER,
                            initial,
                            FontId::proportional(12.5),
                            s.on_primary_container,
                        );
                        ui.painter().text(
                            pos2(ind.left() + 36.0, ind.center().y),
                            Align2::LEFT_CENTER,
                            short(&name, 18),
                            FontId::proportional(13.0),
                            s.on_surface,
                        );
                        if resp.clicked() {
                            self.worker.send(Job::Logout);
                            self.logged = None;
                            self.toast("keluar dari akun");
                        }
                    }
                    None => {
                        ui.painter().circle_filled(
                            pos2(ind.left() + 16.0, ind.center().y),
                            13.0,
                            s.surface_highest,
                        );
                        ui.painter().text(
                            pos2(ind.left() + 16.0, ind.center().y),
                            Align2::CENTER_CENTER,
                            "?",
                            FontId::proportional(12.5),
                            s.on_surface_variant,
                        );
                        ui.painter().text(
                            pos2(ind.left() + 36.0, ind.center().y),
                            Align2::LEFT_CENTER,
                            "Masuk lewat Chromium",
                            FontId::proportional(13.0),
                            s.on_surface,
                        );
                        if resp.clicked() {
                            self.toast("membaca sesi YouTube dari Chromium, tanpa kata sandi");
                            self.worker.send(Job::Login);
                        }
                    }
                }
            }
        });
    }

    fn lib_item(&mut self, ui: &mut Ui, label: &str) -> Response {
        let s = self.scheme;
        let (rect, resp) =
            ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
        if resp.hovered() {
            ui.painter()
                .rect_filled(rect, Rounding::same(10.0), s.on_surface.gamma_multiply(0.06));
        }
        draw_icon(
            ui.painter(),
            Icon::Library,
            Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(17.0, 17.0)),
            s.on_surface_variant,
        );
        ui.painter().text(
            pos2(rect.left() + 36.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(13.0),
            s.on_surface,
        );
        resp
    }

    /// Bilah atas: kolom cari membulat penuh di kiri, ikon di kanan.
    fn top_bar(&mut self, ui: &mut Ui) {
        let s = self.scheme;
        let compact = ui.available_width() < 700.0;
        let mut submit = false;
        ui.horizontal(|ui| {
            let field_w = (ui.available_width() - if compact { 110.0 } else { 210.0 })
                .clamp(150.0, 480.0);
            let (rect, _) = ui.allocate_exact_size(vec2(field_w, 40.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, Rounding::same(20.0), s.surface_high);
            draw_icon(
                ui.painter(),
                Icon::Search,
                Rect::from_center_size(pos2(rect.left() + 22.0, rect.center().y), vec2(18.0, 18.0)),
                s.on_surface_variant,
            );
            let te_rect = Rect::from_min_max(
                pos2(rect.left() + 38.0, rect.top() + 10.0),
                pos2(rect.right() - 14.0, rect.bottom() - 10.0),
            );
            let te = egui::TextEdit::singleline(&mut self.query)
                .frame(false)
                .margin(vec2(0.0, 0.0));
            let resp = ui.put(te_rect, te);
            self.search_focus = resp.has_focus();
            // placeholder digambar sendiri: hint bawaan TextEdit bisa terpotong
            // di kiri saat kolomnya sempit.
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
            if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit = true;
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let logged = self.logged.clone();
                let (av, avr) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
                ui.painter().circle_filled(av.center(), 15.0, s.surface_highest);
                let initial = logged
                    .as_ref()
                    .map(|n| n.chars().next().unwrap_or('?').to_uppercase().to_string())
                    .unwrap_or_else(|| "?".into());
                ui.painter().text(
                    av.center(),
                    Align2::CENTER_CENTER,
                    initial,
                    FontId::proportional(13.0),
                    if logged.is_some() {
                        s.on_surface
                    } else {
                        s.on_surface_variant
                    },
                );
                if avr.clicked() {
                    match logged {
                        Some(_) => {
                            self.worker.send(Job::Logout);
                            self.logged = None;
                            self.toast("keluar dari akun");
                        }
                        None => {
                            self.toast("membaca sesi YouTube dari Chromium, tanpa kata sandi");
                            self.worker.send(Job::Login);
                        }
                    }
                }
                ui.add_space(4.0);
                if self.icon_button(ui, Icon::Theme, 34.0, Btn::Ghost).clicked() {
                    self.scheme = theme::scheme(!s.dark, self.seed);
                }
                let qbtn = self.icon_button(ui, Icon::Queue, 34.0, Btn::Ghost);
                if qbtn.clicked() {
                    self.show_queue = !self.show_queue;
                }
            });
        });

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
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, 56.0), Sense::hover());
        let mid = rect.center().y;
        let left = rect.left();

        // garis durasi tipis di tepi atas bar
        let tr = Rect::from_min_max(
            pos2(left, rect.top() - 9.0),
            pos2(rect.right(), rect.top() - 6.0),
        );
        ui.painter()
            .rect_filled(tr, Rounding::same(1.5), s.outline_variant);
        let pos_now = self.drag_pos.unwrap_or(self.progress.pos);
        if dur > 0.0 {
            let f = (pos_now / dur).clamp(0.0, 1.0) as f32;
            ui.painter().rect_filled(
                Rect::from_min_max(tr.min, pos2(tr.left() + tr.width() * f, tr.max.y)),
                Rounding::same(1.5),
                s.on_surface,
            );
        }
        let hit = ui.interact(
            tr.expand2(vec2(0.0, 7.0)),
            egui::Id::new("bar-seek"),
            Sense::click(),
        );
        if hit.clicked() && dur > 0.0 {
            if let Some(p) = hit.interact_pointer_pos() {
                let f = ((p.x - tr.left()) / tr.width().max(1.0)).clamp(0.0, 1.0);
                self.player.seek(f as f64 * dur);
            }
        }

        // ---- kiri: prev / putar / next / waktu, jarak tetap 16 px
        let cx = left + 20.0;
        let prev = Rect::from_center_size(pos2(cx, mid), vec2(26.0, 26.0));
        let play = Rect::from_center_size(pos2(cx + 52.0, mid), vec2(34.0, 34.0));
        let nxt = Rect::from_center_size(pos2(cx + 104.0, mid), vec2(26.0, 26.0));

        let prev_hit = ui.interact(prev.expand(6.0), egui::Id::new("pb-prev"), Sense::click());
        if prev_hit.hovered() {
            ui.painter()
                .circle_filled(prev.center(), 20.0, s.on_surface.gamma_multiply(0.08));
        }
        if win_w >= 520.0 {
            draw_icon(ui.painter(), Icon::Prev, prev, s.on_surface);
        }
        if prev_hit.clicked() {
            self.next_track(-1);
        }

        let play_hit = ui.interact(play.expand(6.0), egui::Id::new("pb-play"), Sense::click());
        if play_hit.hovered() {
            ui.painter()
                .circle_filled(play.center(), 22.0, s.on_surface.gamma_multiply(0.08));
        }
        draw_icon(
            ui.painter(),
            if self.progress.playing {
                Icon::Pause
            } else {
                Icon::Play
            },
            play.shrink(3.0),
            s.on_surface,
        );
        if play_hit.clicked() {
            self.toggle_play();
        }

        let next_hit = ui.interact(nxt.expand(6.0), egui::Id::new("pb-next"), Sense::click());
        if next_hit.hovered() {
            ui.painter()
                .circle_filled(nxt.center(), 20.0, s.on_surface.gamma_multiply(0.08));
        }
        if win_w >= 520.0 {
            draw_icon(ui.painter(), Icon::Next, nxt, s.on_surface);
        }
        if next_hit.clicked() {
            self.next_track(1);
        }

        if win_w >= 720.0 {
            ui.painter().text(
                pos2(nxt.right() + 20.0, mid),
                Align2::LEFT_CENTER,
                format!("{} / {}", fmt_time(pos_now), fmt_time(dur)),
                FontId::proportional(11.5),
                s.on_surface_variant,
            );
        }

        // ---- tengah: sampul + judul + artis, rapat di kanan jam.
        // Sengaja TIDAK dipusatkan ke tengah jendela: di layar lebar itu
        // meninggalkan celah ratusan piksel antara jam dan sampul, dan itu
        // yang berulang kali dikeluhkan. Jangkar selalu di kanan jam.
        let roomy = win_w >= 980.0;
        let text_w: f32 = if roomy { 190.0 } else { 130.0 };
        let group_w = 40.0 + 12.0 + text_w;
        let time_x = nxt.right() + 20.0;
        let time_w = if win_w >= 720.0 { 66.0 } else { 0.0 };
        let art_left: f32 = (time_x + time_w + 30.0).max(rect.left() + 8.0);
        let art = Rect::from_center_size(pos2(art_left, mid), vec2(40.0, 40.0));
        ui.painter().rect_filled(art, Rounding::same(4.0), s.surface_high);
        if let Some(t) = cover {
            egui::Image::new((t.id(), t.size_vec2()))
                .fit_to_exact_size(vec2(40.0, 40.0))
                .rounding(Rounding::same(4.0))
                .paint_at(ui, art);
        }
        let tx = art.right() + 12.0;
        let chars = (text_w / 7.0).max(8.0) as usize;
        let (title, artist) = match &track {
            Some(t) => (t.title.clone(), t.artist.clone()),
            None => ("Belum ada lagu".into(), String::new()),
        };
        ui.painter().text(
            pos2(tx, mid - 9.0),
            Align2::LEFT_CENTER,
            short(&title, chars),
            FontId::proportional(13.0),
            s.on_surface,
        );
        ui.painter().text(
            pos2(tx, mid + 9.0),
            Align2::LEFT_CENTER,
            short(&artist, chars),
            FontId::proportional(11.5),
            s.on_surface_variant,
        );

        if roomy {
            // trio ini ikut grup info, bukan ke tengah jendela
            let gx = art.left() + group_w + 20.0;
            let menu = Rect::from_center_size(pos2(gx + 62.0, mid), vec2(24.0, 24.0));
            let mh = ui.interact(menu.expand(6.0), egui::Id::new("pb-more"), Sense::click());
            if mh.hovered() {
                ui.painter()
                    .circle_filled(menu.center(), 18.0, s.on_surface.gamma_multiply(0.08));
            }
            draw_icon(ui.painter(), Icon::More, menu, s.on_surface_variant);
            if mh.clicked() {
                self.show_queue = !self.show_queue;
            }
            let dislike = Rect::from_center_size(pos2(gx + 28.0, mid), vec2(24.0, 24.0));
            let dh = ui.interact(dislike.expand(6.0), egui::Id::new("pb-dislike"), Sense::click());
            if dh.hovered() {
                ui.painter()
                    .circle_filled(dislike.center(), 18.0, s.on_surface.gamma_multiply(0.08));
            }
            draw_icon(ui.painter(), Icon::Dislike, dislike, s.on_surface_variant);
            let like = Rect::from_center_size(pos2(gx, mid), vec2(24.0, 24.0));
            let lh = ui.interact(like.expand(6.0), egui::Id::new("pb-like"), Sense::click());
            if lh.hovered() {
                ui.painter()
                    .circle_filled(like.center(), 18.0, s.on_surface.gamma_multiply(0.08));
            }
            draw_icon(ui.painter(), Icon::Like, like, s.on_surface_variant);
            if lh.clicked() {
                match self.current.as_ref().map(|t| t.id.clone()) {
                    Some(id) => {
                        if self.logged.is_some() {
                            self.worker.send(Job::Rate(id, true));
                            self.toast("disukai");
                        } else {
                            self.toast("masuk dulu lewat sesi Chromium");
                        }
                    }
                    None => self.toast("belum ada lagu"),
                }
            }
        }

        // ---- kanan: satu deret, jarak 16 px, margin 20 px
        let mut rx = rect.right() - 20.0;
        let mut slot = |name: &str| -> Rect {
            let r = Rect::from_center_size(pos2(rx - 12.0, mid), vec2(24.0, 24.0));
            rx -= 40.0;
            let _ = name;
            r
        };
        let up = slot("up");
        let shuf = slot("shuf");
        let rep = slot("rep");
        let que = slot("que");
        let lyr = slot("lyr");
        let vol = slot("vol");

        let mut bar_btn = |ui: &mut Ui, icon: Icon, r: Rect, id: &str, on: bool, me: &mut App| -> bool {
            let resp = ui.interact(r.expand(8.0), egui::Id::new(id), Sense::click());
            if resp.hovered() {
                ui.painter()
                    .circle_filled(r.center(), 18.0, me.scheme.on_surface.gamma_multiply(0.08));
            }
            draw_icon(
                ui.painter(),
                icon,
                r,
                if on {
                    me.scheme.primary
                } else {
                    me.scheme.on_surface_variant
                },
            );
            resp.clicked()
        };

        if bar_btn(ui, Icon::ChevronUp, up, "pb-up", false, self) {
            self.expanded = true;
        }
        if win_w >= 860.0 && bar_btn(ui, Icon::Shuffle, shuf, "pb-shuf", false, self) {
            self.shuffle_queue();
        }
        if win_w >= 860.0 && bar_btn(ui, Icon::Repeat, rep, "pb-rep", false, self) {
            self.toast("antrean berputar otomatis di ujung");
        }
        if bar_btn(ui, Icon::Queue, que, "pb-queue", self.show_queue && !self.show_lyrics, self) {
            self.show_queue = !self.show_queue;
            self.show_lyrics = false;
        }
        if bar_btn(ui, Icon::Lyrics, lyr, "pb-lyr", self.show_queue && self.show_lyrics, self) {
            self.show_queue = true;
            self.show_lyrics = true;
        }
        if win_w >= 860.0 && bar_btn(ui, Icon::Volume, vol, "pb-vol", false, self) {
            let v = self.progress.volume;
            self.player.set_volume(if v > 0.01 { 0.0 } else { 0.8 });
        }
    }

    fn shuffle_queue(&mut self) {
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
        self.toast("antrean diacak");
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
        ui.add_space(14.0);
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
        let row_w = w.min(360.0);
        let (row, _) = ui.allocate_exact_size(vec2(w, 64.0), Sense::hover());
        let rmid = row.center().y;
        let rcx = row.center().x;
        let play = Rect::from_center_size(pos2(rcx, rmid), vec2(56.0, 56.0));
        let prev = Rect::from_center_size(pos2(play.center().x - 62.0, rmid), vec2(26.0, 26.0));
        let next = Rect::from_center_size(pos2(play.center().x + 62.0, rmid), vec2(26.0, 26.0));
        let like = Rect::from_center_size(pos2(play.center().x + 128.0, rmid), vec2(24.0, 24.0));
        let _ = row_w;

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

        let lh = ui.interact(like.expand(8.0), egui::Id::new("np-like"), Sense::click());
        if lh.hovered() {
            ui.painter()
                .circle_filled(like.center(), 18.0, s.on_surface.gamma_multiply(0.08));
        }
        draw_icon(ui.painter(), Icon::Like, like, s.on_surface_variant);
        if lh.clicked() {
            match self.current.as_ref().map(|t| t.id.clone()) {
                Some(id) => {
                    if self.logged.is_some() {
                        self.worker.send(Job::Rate(id, true));
                        self.toast("disukai");
                    } else {
                        self.toast("masuk dulu lewat sesi Chromium");
                    }
                }
                None => self.toast("belum ada lagu"),
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

        if self.show_suggest && !self.suggest.is_empty() {
            let list = self.suggest.clone();
            ui.add_space(10.0);
            ScrollArea::horizontal()
                .id_salt("sug")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        for sug in list {
                            if self.chip(ui, &sug, false).clicked() {
                                self.query = sug.clone();
                                self.searched = sug.clone();
                                self.show_suggest = false;
                                self.worker.send(Job::Search(sug));
                            }
                        }
                    });
                });
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
}

// ------------------------------------------------------------------ util

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

        self.ui(ctx);
    }
}
