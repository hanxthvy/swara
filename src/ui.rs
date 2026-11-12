//! Antarmuka Swara — Material 3 Expressive di atas egui.
//!
//! Semua ukuran mengikuti lebar jendela: rail navigasi menyempit, grid kartu
//! menghitung sendiri jumlah kolomnya, dan pemutar berpindah dari panel kanan
//! ke mini player mengapung saat ruang tidak cukup.

use crate::api::{Card, Collection, Lyrics, Row, Stream, Track};
use crate::player::{Player, Progress};
use crate::mpris::{Event as MprisEvent, Mpris};
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
            let flip = if icon == Icon::Next { 1.0 } else { -1.0 };
            let tri = vec![
                pos2(m.x - s * 0.26 * flip, m.y - s * 0.28),
                pos2(m.x - s * 0.26 * flip, m.y + s * 0.28),
                pos2(m.x + s * 0.16 * flip, m.y),
            ];
            p.add(Shape::convex_polygon(tri, c, Stroke::NONE));
            let bar = Rect::from_center_size(
                pos2(m.x + s * 0.30 * flip, m.y),
                vec2(s * 0.10, s * 0.56),
            );
            p.rect_filled(bar, Rounding::same(s * 0.05), c);
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
    mpris: Option<Mpris>,
    last_playing: bool,
    last_pos_push: Instant,
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
        mpris: Option<Mpris>,
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
            last_playing: false,
            last_pos_push: Instant::now(),
            expanded: std::env::var("SWARA_EXPAND").is_ok(),
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
                        m.track(cur, dur, &track.thumb);
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
        let (bg, fg, stroke) = if selected {
            (
                s.secondary_container,
                s.on_secondary_container,
                Stroke::NONE,
            )
        } else {
            (
                Color32::TRANSPARENT,
                s.on_surface_variant,
                Stroke::new(1.0_f32, s.outline_variant),
            )
        };
        ui.add(
            egui::Button::new(ts::title_m(label, fg).size(12.5))
                .fill(bg)
                .stroke(stroke)
                .rounding(Rounding::same(shape::CHIP))
                .min_size(vec2(0.0, 34.0)),
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
        let playing_this = self.current.as_ref().map(|c| c.id == t.id).unwrap_or(false);
        let ctx = ui.ctx().clone();
        self.ensure_thumb(&ctx, &t.thumb);
        let h = if dense { 46.0 } else { 56.0 };
        let side = if dense { 34.0 } else { 42.0 };
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
        if resp.hovered() {
            self.layer(ui, rect, 14.0, &resp, 0.07);
        }
        let mid = rect.center().y;
        let mut x = rect.left() + 8.0;
        if playing_this {
            let ir = Rect::from_center_size(pos2(x + 10.0, mid), vec2(12.0, 12.0));
            draw_icon(ui.painter(), Icon::Play, ir, s.primary);
        } else {
            ui.painter().text(
                pos2(x + 10.0, mid),
                Align2::CENTER_CENTER,
                idx.to_string(),
                FontId::proportional(11.5),
                s.on_surface_variant,
            );
        }
        x += 28.0;
        let img = Rect::from_min_size(pos2(x, mid - side / 2.0), vec2(side, side));
        ui.painter()
            .rect_filled(img, Rounding::same(10.0), s.surface_high);
        if let Some(tex) = self.thumb_of(&t.thumb) {
            egui::Image::new((tex.id(), tex.size_vec2()))
                .fit_to_exact_size(vec2(side, side))
                .rounding(Rounding::same(10.0))
                .paint_at(ui, img);
        }
        x += side + 12.0;

        let dur_w = 44.0;
        let text_w = (rect.right() - x - dur_w - 8.0).max(60.0);
        let chars = (text_w / 7.4).max(8.0) as usize;
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
            short(&t.artist, (chars as f32 * 0.85) as usize),
            FontId::proportional(11.5),
            s.on_surface_variant,
        );
        ui.painter().text(
            pos2(rect.right() - 8.0, mid),
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

    fn card(&mut self, ui: &mut Ui, card: &Card, w: f32) {
        let s = self.scheme;
        let ctx = ui.ctx().clone();
        self.ensure_thumb(&ctx, &card.thumb);
        let tex = self.thumb_of(&card.thumb);
        ui.allocate_ui_with_layout(vec2(w, w + 50.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(w);
            let (rect, resp) = ui.allocate_exact_size(vec2(w, w), Sense::click());
            ui.painter()
                .rect_filled(rect, Rounding::same(shape::CARD), s.surface_high);
            if let Some(t) = tex {
                egui::Image::new((t.id(), t.size_vec2()))
                    .fit_to_exact_size(vec2(w, w))
                    .rounding(Rounding::same(shape::CARD))
                    .paint_at(ui, rect);
            } else if !card.thumb.is_empty() {
                let tt = ui.input(|i| i.time) as f32;
                let a = 0.16 + 0.16 * (tt * 1.7).sin().abs();
                ui.painter()
                    .circle_filled(rect.center(), w * 0.045, s.on_surface.gamma_multiply(a));
            }
            if resp.hovered() {
                ui.painter().rect_stroke(
                    rect,
                    Rounding::same(shape::CARD),
                    Stroke::new(1.5_f32, s.outline),
                );
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                self.open_card(card);
            }
            ui.add_space(8.0);
            let chars = (w / 6.8).max(8.0) as usize;
            ui.label(ts::title_m(short(&card.title, chars), s.on_surface).size(13.0));
            ui.label(ts::body_s(short(&card.sub, chars), s.on_surface_variant).size(11.5));
        });
    }

    fn card_grid(&mut self, ui: &mut Ui, items: &[Card], compact: bool) {
        self.card_grid_sized(ui, items, compact, false)
    }

    /// `hero` memperbesar kartu baris pertama: momen editorial khas Expressive.
    fn card_grid_sized(&mut self, ui: &mut Ui, items: &[Card], compact: bool, hero: bool) {
        let avail = ui.available_width();
        let gap = 14.0;
        let target = if hero {
            if compact {
                200.0
            } else {
                260.0
            }
        } else if compact {
            132.0
        } else {
            164.0
        };
        let cols = (((avail + gap) / (target + gap)).floor() as usize)
            .clamp(1, 7)
            .min(items.len().max(1));
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
        if self.expanded {
            self.now_playing(ctx);
            return;
        }
        let s = self.scheme;
        let screen = ctx.screen_rect();
        let w = screen.width();
        let h = screen.height();
        let compact = w < 820.0;
        let very_compact = w < 540.0;
        let rail_wide = w >= 1120.0;
        let dock = w >= 1000.0 && h >= 640.0;

        // ---- navigasi
        if compact {
            egui::TopBottomPanel::top("nav")
                .frame(
                    egui::Frame::none()
                        .fill(s.surface_low)
                        .inner_margin(egui::Margin::symmetric(10.0, 8.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if !very_compact {
                            ui.label(ts::title("swara", s.primary));
                            ui.add_space(8.0);
                        }
                        for t in [Tab::Home, Tab::Cari, Tab::Koleksi] {
                            let active = self.tab == t;
                            let r = self.nav_chip(ui, t, active, very_compact);
                            if r.clicked() {
                                self.goto(t);
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if self.icon_button(ui, Icon::Theme, 34.0, Btn::Ghost).clicked() {
                                self.scheme = theme::scheme(!s.dark, self.seed);
                            }
                        });
                    });
                });
        } else {
            let rail_w = if rail_wide { 200.0 } else { 84.0 };
            egui::SidePanel::left("rail")
                .resizable(false)
                .exact_width(rail_w)
                .frame(
                    egui::Frame::none()
                        .fill(s.surface_low)
                        .inner_margin(egui::Margin::symmetric(10.0, 16.0)),
                )
                .show(ctx, |ui| {
                    if rail_wide {
                        ui.horizontal(|ui| {
                            ui.add_space(4.0);
                            ui.label(ts::headline("swara", s.primary));
                        });
                        ui.add_space(4.0);
                        ui.label(ts::label("PEMUTAR MUSIK", s.on_surface_variant));
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.label(ts::headline("s", s.primary));
                        });
                    }
                    ui.add_space(18.0);
                    for t in [Tab::Home, Tab::Cari, Tab::Koleksi] {
                        let active = self.tab == t;
                        if self.rail_item(ui, t, active, rail_wide).clicked() {
                            self.goto(t);
                        }
                        ui.add_space(6.0);
                    }
                    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                        if self.icon_button(ui, Icon::Theme, 40.0, Btn::Ghost).clicked() {
                            self.scheme = theme::scheme(!s.dark, self.seed);
                        }
                    });
                });
        }

        // ---- pemutar
        if dock {
            egui::SidePanel::right("dock")
                .resizable(false)
                .exact_width(if w >= 1240.0 { 344.0 } else { 312.0 })
                .frame(
                    egui::Frame::none()
                        .fill(s.surface)
                        .inner_margin(egui::Margin::symmetric(18.0, 18.0)),
                )
                .show(ctx, |ui| self.dock_player(ui));
        } else {
            egui::TopBottomPanel::bottom("bar")
                .frame(
                    egui::Frame::none()
                        .fill(s.surface_high)
                        .rounding(Rounding::same(shape::SHEET))
                        .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                        .outer_margin(egui::Margin::symmetric(10.0, 10.0)),
                )
                .show(ctx, |ui| {
                    let full = ui.max_rect();
                    let hit = ui.interact(full, egui::Id::new("mini-expand"), Sense::click());
                    self.mini_player(ui);
                    if hit.clicked() && self.current.is_some() {
                        self.expanded = true;
                    }
                });
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(s.surface)
                    .inner_margin(egui::Margin::symmetric(if compact { 16.0 } else { 26.0 }, 18.0)),
            )
            .show(ctx, |ui| match self.tab {
                Tab::Home => self.tab_home(ui, compact),
                Tab::Cari => self.tab_cari(ui),
                Tab::Koleksi => self.tab_koleksi(ui),
            });

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
        if t == Tab::Koleksi && self.koleksi.is_none() {
            self.view = View::Feed;
            self.worker.send(Job::Browse("FEmusic_liked_playlists".into()));
        }
        if t == Tab::Cari {
            self.focus_cari = true;
        }
    }

    fn nav_chip(&mut self, ui: &mut Ui, t: Tab, active: bool, icon_only: bool) -> Response {
        let s = self.scheme;
        let label = if icon_only { "" } else { t.label() };
        let w = if icon_only { 44.0 } else { 0.0 };
        let (rect, resp) = ui.allocate_exact_size(
            vec2(
                if icon_only {
                    w
                } else {
                    24.0 + label.chars().count() as f32 * 7.2
                },
                36.0,
            ),
            Sense::click(),
        );
        if active {
            ui.painter().rect_filled(
                rect,
                Rounding::same(18.0),
                s.secondary_container,
            );
        } else {
            self.layer(ui, rect, 18.0, &resp, 0.07);
        }
        let fg = if active {
            s.on_secondary_container
        } else {
            s.on_surface_variant
        };
        let icon_rect = Rect::from_center_size(
            if icon_only {
                rect.center()
            } else {
                pos2(rect.left() + 20.0, rect.center().y)
            },
            vec2(15.0, 15.0),
        );
        draw_icon(ui.painter(), t.icon(), icon_rect, fg);
        if !icon_only {
            ui.painter().text(
                pos2(rect.left() + 32.0, rect.center().y),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(13.0),
                fg,
            );
        }
        resp
    }

    fn rail_item(&mut self, ui: &mut Ui, t: Tab, active: bool, wide: bool) -> Response {
        let s = self.scheme;
        let width = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(width, 54.0), Sense::click());
        let ind = Rect::from_center_size(
            pos2(rect.center().x, rect.center().y),
            vec2(if wide { width - 8.0 } else { 56.0 }, 40.0),
        );
        if active {
            ui.painter()
                .rect_filled(ind, Rounding::same(20.0), s.secondary_container);
        } else {
            self.layer(ui, ind, 20.0, &resp, 0.07);
        }
        let fg = if active {
            s.on_secondary_container
        } else {
            s.on_surface_variant
        };
        let icon_rect = Rect::from_center_size(
            if wide {
                pos2(ind.left() + 24.0, ind.center().y)
            } else {
                pos2(ind.center().x, ind.center().y - 8.0)
            },
            vec2(16.0, 16.0),
        );
        draw_icon(ui.painter(), t.icon(), icon_rect, fg);
        if wide {
            ui.painter().text(
                pos2(ind.left() + 44.0, ind.center().y),
                Align2::LEFT_CENTER,
                t.label(),
                FontId::proportional(13.5),
                fg,
            );
        } else {
            ui.painter().text(
                pos2(ind.center().x, ind.bottom() - 4.0),
                Align2::CENTER_BOTTOM,
                t.label(),
                FontId::proportional(9.5),
                fg,
            );
        }
        resp
    }

    // ---- pemutar samping

    fn dock_player(&mut self, ui: &mut Ui) {
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

        let side = ui.available_width().min(ui.available_height() * 0.46).min(300.0);
        let url = track.as_ref().map(|t| t.thumb.clone()).unwrap_or_default();
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), side), Sense::hover());
        let art = Rect::from_center_size(rect.center(), vec2(side, side));
        ui.painter()
            .rect_filled(art, Rounding::same(shape::SHEET), s.surface_high);
        if let Some(t) = cover {
            egui::Image::new((t.id(), t.size_vec2()))
                .fit_to_exact_size(vec2(side, side))
                .rounding(Rounding::same(shape::SHEET))
                .paint_at(ui, art);
        } else if !url.is_empty() {
            let tt = ui.input(|i| i.time) as f32;
            let a = 0.16 + 0.16 * (tt * 1.7).sin().abs();
            ui.painter()
                .circle_filled(art.center(), side * 0.04, s.on_surface.gamma_multiply(a));
        }

        ui.add_space(16.0);
        let (title, artist) = match &track {
            Some(t) => (t.title.clone(), t.artist.clone()),
            None => ("Belum ada lagu".into(), "Pilih dari beranda".into()),
        };
        ui.label(ts::headline(short(&title, 34), s.on_surface));
        ui.label(ts::body(short(&artist, 40), s.on_surface_variant));

        ui.add_space(12.0);
        let w = ui.available_width();
        if let Some(v) = self.seek_bar(ui, dur, w) {
            self.player.seek(v);
        }
        ui.horizontal(|ui| {
            ui.label(ts::label(fmt_time(self.progress.pos), s.on_surface_variant));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(ts::label(fmt_time(dur), s.on_surface_variant));
            });
        });

        ui.add_space(14.0);
        let icon = if self.progress.playing {
            Icon::Pause
        } else {
            Icon::Play
        };
        let mut act = 0i8;
        ui.columns(3, |cols| {
            cols[0].vertical_centered(|ui| {
                if self.icon_button(ui, Icon::Prev, 46.0, Btn::Tonal).clicked() {
                    act = -1;
                }
            });
            cols[1].vertical_centered(|ui| {
                if self.icon_button(ui, icon, 64.0, Btn::Filled).clicked() {
                    act = 2;
                }
            });
            cols[2].vertical_centered(|ui| {
                if self.icon_button(ui, Icon::Next, 46.0, Btn::Tonal).clicked() {
                    act = 1;
                }
            });
        });
        match act {
            -1 => self.next_track(-1),
            1 => self.next_track(1),
            2 => self.toggle_play(),
            _ => {}
        }


        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if self.chip(ui, "Lirik", self.show_lyrics).clicked() {
                self.show_lyrics = !self.show_lyrics;
            }
            if self.chip(ui, "Suka", false).clicked() {
                match self.current.as_ref().map(|t| t.id.clone()) {
                    Some(id) => {
                        if self.logged.is_some() {
                            self.worker.send(Job::Rate(id, true));
                        } else {
                            self.toast("masuk dulu lewat sesi Chromium");
                        }
                    }
                    None => self.toast("belum ada lagu"),
                }
            }
        });
        if self.show_lyrics {
            ui.add_space(8.0);
            self.lyrics_view(ui, 200.0);
        }
    }

    // ---- pemutar bawah

    fn mini_player(&mut self, ui: &mut Ui) {
        let s = self.scheme;
        let track = self.current.clone();
        let dur = self
            .current_info
            .as_ref()
            .map(|i| i.dur)
            .filter(|d| *d > 0.0)
            .or_else(|| track.as_ref().map(|t| t.secs))
            .unwrap_or(0.0);
        let narrow = ui.available_width() < 400.0;
        let cover = self.cover_of();

        ui.horizontal(|ui| {
            let side = if narrow { 38.0 } else { 44.0 };
            let url = track.as_ref().map(|t| t.thumb.clone()).unwrap_or_default();
            let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
            ui.painter()
                .rect_filled(rect, Rounding::same(12.0), s.surface_highest);
            if let Some(t) = cover {
                egui::Image::new((t.id(), t.size_vec2()))
                    .fit_to_exact_size(vec2(side, side))
                    .rounding(Rounding::same(12.0))
                    .paint_at(ui, rect);
            }
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                let title = track
                    .as_ref()
                    .map(|t| t.title.clone())
                    .unwrap_or_else(|| "Belum ada lagu".into());
                let artist = track
                    .as_ref()
                    .map(|t| t.artist.clone())
                    .unwrap_or_default();
                ui.label(ts::title_m(short(&title, if narrow { 22 } else { 40 }), s.on_surface).size(13.5));
                if !artist.is_empty() {
                    ui.label(ts::body_s(short(&artist, if narrow { 24 } else { 46 }), s.on_surface_variant));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let icon = if self.progress.playing {
                    Icon::Pause
                } else {
                    Icon::Play
                };
                if self.icon_button(ui, icon, 40.0, Btn::Filled).clicked() {
                    self.toggle_play();
                }
                ui.add_space(2.0);
                if self.icon_button(ui, Icon::Next, 34.0, Btn::Ghost).clicked() {
                    self.next_track(1);
                }
                if self.icon_button(ui, Icon::ChevronUp, 30.0, Btn::Ghost).clicked() {
                    self.expanded = true;
                }
            });
        });
        if !narrow && dur > 0.0 {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(ts::label(fmt_time(self.progress.pos), s.on_surface_variant).size(10.5));
                let w = ui.available_width() - 46.0;
                if let Some(v) = self.seek_bar(ui, dur, w) {
                    self.player.seek(v);
                }
                ui.label(ts::label(fmt_time(dur), s.on_surface_variant).size(10.5));
            });
        }
    }

    // ---- pemutar penuh (klik bar bawah untuk membuka)

    fn now_playing(&mut self, ctx: &egui::Context) {
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

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(s.surface)
                    .inner_margin(egui::Margin::symmetric(24.0, 14.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.icon_button(ui, Icon::ChevronDown, 38.0, Btn::Ghost).clicked() {
                        self.expanded = false;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(ts::label("SEDANG DIPUTAR", s.on_surface_variant));
                    });
                });

                let w = ui.available_width();
                let h = ui.available_height();
                let side = (w * 0.34).min(h * 0.52).clamp(110.0, 340.0);
                ui.add_space(6.0);
                let (rect, _) = ui.allocate_exact_size(vec2(w, side), Sense::hover());
                let art = Rect::from_center_size(rect.center(), vec2(side, side));
                ui.painter()
                    .rect_filled(art, Rounding::same(30.0), s.surface_high);
                if let Some(t) = cover {
                    egui::Image::new((t.id(), t.size_vec2()))
                        .fit_to_exact_size(vec2(side, side))
                        .rounding(Rounding::same(30.0))
                        .paint_at(ui, art);
                }

                ui.add_space(12.0);
                let (title, artist) = match &track {
                    Some(t) => (t.title.clone(), t.artist.clone()),
                    None => ("Belum ada lagu".into(), "Pilih dari beranda".into()),
                };
                ui.vertical_centered(|ui| {
                    ui.label(ts::headline(short(&title, 46), s.on_surface));
                    ui.label(ts::body(short(&artist, 52), s.on_surface_variant));
                });

                ui.add_space(10.0);
                if let Some(v) = self.seek_bar(ui, dur, w) {
                    self.player.seek(v);
                }
                ui.horizontal(|ui| {
                    ui.label(ts::label(fmt_time(self.progress.pos), s.on_surface_variant));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(ts::label(fmt_time(dur), s.on_surface_variant));
                    });
                });

                ui.add_space(8.0);
                let icon = if self.progress.playing {
                    Icon::Pause
                } else {
                    Icon::Play
                };
                let mut act = 0i8;
                ui.columns(3, |cols| {
                    cols[0].vertical_centered(|ui| {
                        if self.icon_button(ui, Icon::Prev, 52.0, Btn::Tonal).clicked() {
                            act = -1;
                        }
                    });
                    cols[1].vertical_centered(|ui| {
                        if self.icon_button(ui, icon, 72.0, Btn::Filled).clicked() {
                            act = 2;
                        }
                    });
                    cols[2].vertical_centered(|ui| {
                        if self.icon_button(ui, Icon::Next, 52.0, Btn::Tonal).clicked() {
                            act = 1;
                        }
                    });
                });
                match act {
                    -1 => self.next_track(-1),
                    1 => self.next_track(1),
                    2 => self.toggle_play(),
                    _ => {}
                }


                ui.add_space(12.0);
                let rest = ui.available_height();
                if rest > 54.0 {
                    self.lyrics_view(ui, rest - 6.0);
                }
            });
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
                .show(ui, |ui| {
                    ui.label(ts::body_s(txt, s.on_surface_variant));
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
            .vertical_scroll_offset(((active as f32) * 24.0 - max_h / 2.0).max(0.0))
            .show(ui, |ui| {
                for (i, line) in l.synced.iter().enumerate() {
                    let is_now = i == active;
                    ui.label(
                        RichText::new(&line.text)
                            .size(if is_now { 14.0 } else { 12.0 })
                            .color(if is_now { s.on_surface } else { s.on_surface_variant }),
                    );
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
                for (i, row) in rows.iter().enumerate() {
                    ui.add_space(if i == 0 { 2.0 } else { 10.0 });
                    ui.label(if i == 0 {
                        ts::headline(row.title.clone(), s.on_surface)
                    } else {
                        ts::title(row.title.clone(), s.on_surface)
                    });
                    ui.add_space(10.0);
                    self.card_grid_sized(ui, &row.items, compact, i == 0);
                    ui.add_space(6.0);
                }
                ui.add_space(20.0);
            });
    }

    // ---- cari

    fn tab_cari(&mut self, ui: &mut Ui) {
        let s = self.scheme;
        let mut submit = false;
        let field_w = ui.available_width().min(560.0);
        egui::Frame::none()
            .fill(s.surface_highest)
            .rounding(Rounding::same(shape::FIELD))
            .inner_margin(egui::Margin::symmetric(16.0, 10.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (ir, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    draw_icon(ui.painter(), Icon::Search, ir, s.on_surface_variant);
                    ui.add_space(8.0);
                    let te = egui::TextEdit::singleline(&mut self.query)
                        .frame(false)
                        .hint_text("Cari lagu, artis, album")
                        .desired_width(field_w - 80.0)
                        .margin(vec2(0.0, 2.0));
                    let resp = ui.add(te);
                    if self.focus_cari {
                        resp.request_focus();
                        self.focus_cari = false;
                    }
                    if resp.changed() {
                        self.suggest_at = Some(Instant::now());
                        self.show_suggest = true;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submit = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !self.query.is_empty() && ui.button("Cari").clicked() {
                            submit = true;
                        }
                    });
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
            self.worker.send(Job::Search(self.query.clone()));
        }

        if self.show_suggest && !self.suggest.is_empty() {
            let list = self.suggest.clone();
            ui.add_space(6.0);
            for sug in list {
                let (rect, resp) = ui.allocate_exact_size(
                    vec2(ui.available_width().min(field_w), 34.0),
                    Sense::click(),
                );
                self.layer(ui, rect, 12.0, &resp, 0.07);
                ui.painter().text(
                    pos2(rect.left() + 12.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    &sug,
                    FontId::proportional(13.0),
                    s.on_surface_variant,
                );
                if resp.clicked() {
                    self.query = sug.clone();
                    self.searched = sug.clone();
                    self.show_suggest = false;
                    self.worker.send(Job::Search(sug));
                }
            }
        }

        ui.add_space(12.0);
        if self.results.is_empty() {
            if let Some(e) = self.last_error.clone() {
                ui.label(ts::body(format!("Error: {e}"), s.on_surface_variant));
            } else if !self.searched.is_empty() && !self.show_suggest {
                ui.label(ts::body("Tidak ada hasil", s.on_surface_variant));
            } else {
                ui.label(ts::body("Ketik judul lagu atau nama artis.", s.on_surface_variant));
            }
            return;
        }
        ui.label(ts::title(format!("Hasil untuk “{}”", short(&self.searched, 32)), s.on_surface));
        ui.add_space(6.0);
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
            ui.label(ts::headline("Koleksi", s.on_surface));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                match self.logged.clone() {
                    Some(name) => {
                        if self.icon_button(ui, Icon::Theme, 30.0, Btn::Ghost).clicked() {
                            self.worker.send(Job::Logout);
                        }
                        ui.label(ts::body_s(short(&name, 24), s.on_surface_variant));
                    }
                    None => {
                        if self.chip(ui, "Masuk lewat Chromium", true).clicked() {
                            self.worker.send(Job::Login);
                        }
                    }
                }
            });
        });
        ui.label(ts::body_s(
            if self.logged.is_some() {
                "Sesi dibaca dari profil Chromium di mesin ini, bukan dari kata sandi."
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
                ui.label(ts::body(
                    "Playlist milik akun akan muncul di sini setelah tersambung.",
                    s.on_surface_variant,
                ));
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
        "/usr/share/fonts/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/LiberationSans-Regular.ttf",
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
        let ended = prog.ended;
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
        if ended {
            eprintln!(
                "[swara] selesai pos={:.1}s dur={:.1}s",
                self.progress.pos, self.progress.dur
            );
            // Lagu berikutnya dipilih konduktor, bukan di sini: kalau jendela
            // tersembunyi, `update` tidak dipanggil sama sekali.
        }

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

        // Kendali dari panel sistem lewat MPRIS (islet, playerctl, waybar).
        let mut evs: Vec<MprisEvent> = Vec::new();
        if let Some(m) = &self.mpris {
            while let Ok(e) = m.rx.try_recv() {
                evs.push(e);
            }
        }
        for ev in evs {
            match ev {
                MprisEvent::Play => {
                    if !self.progress.playing {
                        self.player.resume();
                    }
                }
                MprisEvent::Pause => {
                    if self.progress.playing {
                        self.player.pause();
                    }
                }
                MprisEvent::PlayPause => self.toggle_play(),
                MprisEvent::Next => self.next_track(1),
                MprisEvent::Prev => self.next_track(-1),
                MprisEvent::SeekBy(off) => {
                    let t = (self.progress.pos + off).max(0.0);
                    self.player.seek(t);
                }
                MprisEvent::SeekTo(pos) => self.player.seek(pos.max(0.0)),
                MprisEvent::Stop => self.player.stop(),
            }
        }
        if self.progress.playing != self.last_playing {
            self.last_playing = self.progress.playing;
            if let Some(m) = &self.mpris {
                m.status(self.progress.playing);
            }
        }
        if self.progress.playing && self.last_pos_push.elapsed() > Duration::from_millis(500) {
            self.last_pos_push = Instant::now();
            if let Some(m) = &self.mpris {
                m.position(self.progress.pos);
            }
        }
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

        // F / Esc berlaku walau kotak cari sedang fokus.
        ctx.input(|i| {
            if i.key_pressed(egui::Key::F) {
                self.expanded = !self.expanded;
            }
            if i.key_pressed(egui::Key::Escape) {
                self.expanded = false;
            }
        });

        // Pintasan papan ketik, diabaikan saat kotak cari sedang fokus.
        let typing = ctx.memory(|m| m.focused().is_some());
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
