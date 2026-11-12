//! Antarmuka Swara: tab di kiri, konten di tengah, pemutar penuh di kanan.
//!
//! Warna mengikuti sampul lagu yang sedang diputar: hue dari rata-rata piksel
//! sampul, lalu permukaan dan aksen diturunkan dari hue yang sama.

use crate::api::{Card, Collection, Lyrics, Row, Stream, Track};
use crate::player::{Player, Progress};
use crate::worker::{Job, Out, Worker};
use egui::{
    vec2, Align, Color32, FontData, FontDefinitions, FontFamily, FontId, Layout,
    ProgressBar, Response, RichText, Rounding, ScrollArea, Sense, Stroke, TextureHandle, Ui,
};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

type ThumbOut = (String, Option<TextureHandle>);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Cari,
    Koleksi,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Feed,
    Collection,
}

// ------------------------------------------------------------------ palet

#[derive(Clone, Copy)]
struct Palette {
    dark: bool,
    bg: Color32,
    surface: Color32,
    surface_hi: Color32,
    text: Color32,
    dim: Color32,
    accent: Color32,
    on_accent: Color32,
}

impl Palette {
    fn base(dark: bool) -> Self {
        Self::from_seed(dark, None)
    }

    fn from_seed(dark: bool, seed: Option<[u8; 3]>) -> Self {
        let (h, s) = dominant(seed);
        if dark {
            Palette {
                dark,
                bg: hsl(h, s * 0.20, 0.065),
                surface: hsl(h, s * 0.18, 0.115),
                surface_hi: hsl(h, s * 0.24, 0.185),
                text: hsl(h, 0.10, 0.95),
                dim: hsl(h, 0.10, 0.62),
                accent: hsl(h, (s * 0.95).max(0.45), 0.68),
                on_accent: hsl(h, 0.40, 0.10),
            }
        } else {
            Palette {
                dark,
                bg: hsl(h, s * 0.22, 0.972),
                surface: hsl(h, s * 0.18, 0.94),
                surface_hi: hsl(h, s * 0.28, 0.885),
                text: hsl(h, 0.30, 0.13),
                dim: hsl(h, 0.16, 0.44),
                accent: hsl(h, (s * 0.9).max(0.40), 0.40),
                on_accent: hsl(h, 0.25, 0.99),
            }
        }
    }
}

/// Hue + saturasi kasar dari warna rata-rata sampul.
fn dominant(seed: Option<[u8; 3]>) -> (f32, f32) {
    let (r, g, b) = match seed {
        Some(c) => (
            c[0] as f32 / 255.0,
            c[1] as f32 / 255.0,
            c[2] as f32 / 255.0,
        ),
        None => (0.60, 0.53, 0.72),
    };
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    let mut h = if d < 1e-4 {
        200.0
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
    // Sampul yang hampir hitam memberi hue acak; tarik ke netral.
    let s = if l < 0.14 || d < 0.06 { 0.42 } else { d };
    (h / 360.0, s.clamp(0.18, 0.82))
}

fn hsl(h: f32, s: f32, l: f32) -> Color32 {
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

fn hash_name(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn fmt_time(s: f64) -> String {
    let s = s.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn parse_dur(s: &str) -> f64 {
    let mut total = 0.0;
    for p in s.split(':') {
        total = total * 60.0 + p.trim().parse::<f64>().unwrap_or(0.0);
    }
    total
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
    open_home_row: Option<usize>,
    koleksi: Option<Collection>,

    query: String,
    results: Vec<Track>,
    suggest: Vec<String>,
    show_suggest: bool,
    suggest_at: Option<Instant>,
    searched: String,

    lyrics: Option<Lyrics>,
    lyrics_id: String,

    queue: Vec<Track>,
    current: Option<Track>,
    current_info: Option<Stream>,
    loading: bool,
    progress: Progress,
    pending_pos: Option<f64>,
    drag_pos: Option<f64>,

    art: Option<TextureHandle>,
    art_pending: bool,
    thumbs: HashMap<String, Option<TextureHandle>>,
    thumb_pending: HashSet<String>,
    thumb_rx: Receiver<ThumbOut>,
    thumb_tx: Sender<ThumbOut>,
    art_rx: Receiver<(String, Option<Vec<u8>>)>,
    art_tx: Sender<(String, Option<Vec<u8>>)>,

    palette: Palette,
    seed: Option<[u8; 3]>,
    toast: Option<(String, Instant)>,
    show_full: bool,
    boot_at: Instant,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        api: Arc<crate::api::Api>,
        player: Player,
        worker: Worker,
    ) -> Self {
        install_fonts(&cc.egui_ctx);
        let (thumb_tx, thumb_rx) = channel();
        let (art_tx, art_rx) = channel();
        let mut app = App {
            api,
            player,
            worker,
            tab: Tab::Home,
            view: View::Feed,
            home: Vec::new(),
            home_err: String::new(),
            open_home_row: None,
            koleksi: None,
            query: String::new(),
            results: Vec::new(),
            suggest: Vec::new(),
            show_suggest: false,
            suggest_at: None,
            searched: String::new(),
            lyrics: None,
            lyrics_id: String::new(),
            queue: Vec::new(),
            current: None,
            current_info: None,
            loading: false,
            progress: Progress {
                volume: 0.8,
                ..Default::default()
            },
            pending_pos: None,
            drag_pos: None,
            art: None,
            art_pending: false,
            thumbs: HashMap::new(),
            thumb_pending: HashSet::new(),
            thumb_rx,
            thumb_tx,
            art_rx,
            art_tx,
            palette: Palette::base(true),
            seed: None,
            toast: None,
            show_full: false,
            boot_at: Instant::now(),
        };
        app.worker.send(Job::Home);
        app
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    fn apply_out(&mut self, out: Out, ctx: &egui::Context) {
        match out {
            Out::Home(rows) => {
                if rows.is_empty() {
                    self.home_err = "beranda kosong, mungkin koneksi".into();
                }
                self.home = rows;
            }
            Out::Search(q, t) => {
                if q == self.query {
                    self.results = t;
                    self.show_suggest = false;
                }
            }
            Out::Suggest(q, s) => {
                if q == self.query {
                    self.suggest = s;
                    self.show_suggest = true;
                }
            }
            Out::Browse(c) => {
                self.koleksi = Some(c);
            }
            Out::Radio(id, t) => {
                if self.current.as_ref().map(|c| c.id == id).unwrap_or(false) {
                    let have: HashSet<String> = self.queue.iter().map(|x| x.id.clone()).collect();
                    for item in t {
                        if !have.contains(&item.id) {
                            self.queue.push(item);
                        }
                    }
                }
            }
            Out::Lyrics(id, l) => {
                if id == self.lyrics_id {
                    self.lyrics = Some(l);
                }
            }
            Out::Loaded(track, info, audio, art) => {
                self.loading = false;
                self.art_pending = false;
                if self.current.as_ref().map(|c| c.id != track.id).unwrap_or(true) {
                    return;
                }
                let dur = if info.dur > 0.0 { info.dur } else { track.secs };
                self.current_info = Some(info);
                self.player.play(audio, 0.0, dur);
                self.progress.dur = dur;
                self.progress.ended = false;
                if let Some(pos) = self.pending_pos.take() {
                    if pos > 1.0 {
                        self.player.seek(pos);
                        self.progress.pos = pos;
                    }
                }
                if let Some(bytes) = art {
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        self.set_seed(&img);
                        let key = format!("cover{}", hash_name(&track.thumb));
                        self.art = Some(load_texture(ctx, &key, &img));
                    }
                }
                self.worker.send(Job::Lyrics(track));
            }
            Out::Failed(e) => {
                self.loading = false;
                self.toast(format!("gagal: {e}"));
            }
        }
    }

    fn play_track(&mut self, track: Track, queue: Option<Vec<Track>>) {
        if let Some(q) = queue {
            self.queue = q;
        }
        self.current = Some(track.clone());
        self.current_info = None;
        self.lyrics = None;
        self.lyrics_id = track.id.clone();
        self.loading = true;
        self.art = None;
        self.art_pending = false;
        self.progress = Progress {
            volume: self.progress.volume,
            ..Default::default()
        };
        self.worker.send(Job::Load(track.clone()));
        if !self.queue.iter().any(|t| t.id == track.id) {
            self.queue.insert(0, track.clone());
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
            if self.seed.map(|s| {
                (s[0] as i32 - seed[0] as i32).abs()
                    + (s[1] as i32 - seed[1] as i32).abs()
                    + (s[2] as i32 - seed[2] as i32).abs()
                    > 12
            }) != Some(false)
            {
                self.seed = Some(seed);
                self.palette = Palette::from_seed(self.palette.dark, self.seed);
            }
        }
    }

    fn ensure_thumb(&mut self, ctx: &egui::Context, url: &str) -> bool {
        if url.is_empty() {
            return false;
        }
        if let Some(v) = self.thumbs.get(url) {
            return v.is_some();
        }
        if !self.thumb_pending.insert(url.to_string()) {
            return false;
        }
        self.thumbs.insert(url.to_string(), None);
        let api = self.api.clone();
        let tx = self.thumb_tx.clone();
        let c = ctx.clone();
        let u = url.to_string();
        std::thread::spawn(move || {
            let tex = api.fetch_image(&u).ok().and_then(|b| {
                image::load_from_memory(&b)
                    .ok()
                    .map(|img| load_texture(&c, &format!("t{}", hash_name(&u)), &img))
            });
            let _ = tx.send((u, tex));
        });
        false
    }

    fn pill(&self, ui: &mut Ui, label: &str, active: bool) -> Response {
        let p = self.palette;
        let (bg, fg) = if active {
            (p.accent, p.on_accent)
        } else {
            (p.surface, p.text)
        };
        ui.add(
            egui::Button::new(RichText::new(label).color(fg).size(14.0))
                .fill(bg)
                .rounding(Rounding::same(18.0))
                .min_size(vec2(0.0, 34.0)),
        )
    }

    fn icon_button(&self, ui: &mut Ui, glyph: &str, size: f32, filled: bool) -> Response {
        let p = self.palette;
        let (bg, fg) = if filled {
            (p.accent, p.on_accent)
        } else {
            (p.surface, p.text)
        };
        ui.add(
            egui::Button::new(RichText::new(glyph).color(fg).size(size * 0.42))
                .fill(bg)
                .rounding(Rounding::same(size / 2.0))
                .min_size(vec2(size, size)),
        )
    }

    fn thumb_widget(&mut self, ui: &mut Ui, url: &str, size: f32, round: u8) -> Response {
        let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
        let p = self.palette;
        ui.painter()
            .rect_filled(rect, Rounding::same(round as f32), p.surface_hi);
        if let Some(Some(t)) = self.thumbs.get(url) {
            egui::Image::new((t.id(), t.size_vec2()))
                .fit_to_exact_size(vec2(size, size))
                .rounding(Rounding::same(round as f32))
                .paint_at(ui, rect);
        } else if !url.is_empty() {
            let c = rect.center();
            let t = ui.input(|i| i.time) as f32;
            let a = 0.30 + 0.22 * (t * 2.2).sin().abs();
            ui.painter()
                .circle_filled(c, size * 0.05, p.dim.gamma_multiply(a));
        } else {
            let g = ui
                .painter()
                .layout_no_wrap("♪".into(), FontId::proportional(size * 0.32), p.dim);
            let at = rect.center() - g.size() / 2.0;
            ui.painter().galley(at, g, p.dim);
        }
        if resp.hovered() {
            ui.painter().rect_stroke(
                rect,
                Rounding::same(round as f32),
                Stroke::new(2.0, p.accent),
            );
        }
        resp
    }

    fn cover_widget(&mut self, ui: &mut Ui, size: f32, round: u8) {
        let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
        let p = self.palette;
        ui.painter()
            .rect_filled(rect, Rounding::same(round as f32), p.surface_hi);
        if let Some(t) = &self.art {
            egui::Image::new((t.id(), t.size_vec2()))
                .fit_to_exact_size(vec2(size, size))
                .rounding(Rounding::same(round as f32))
                .paint_at(ui, rect);
        } else if let Some(track) = self.current.clone() {
            if self.ensure_thumb(ui.ctx(), &track.thumb) {
                if let Some(Some(t)) = self.thumbs.get(&track.thumb) {
                    egui::Image::new((t.id(), t.size_vec2()))
                        .fit_to_exact_size(vec2(size, size))
                        .rounding(Rounding::same(round as f32))
                        .paint_at(ui, rect);
                    return;
                }
            }
            let g = ui
                .painter()
                .layout_no_wrap("♪".into(), FontId::proportional(size * 0.26), p.dim);
            let at = rect.center() - g.size() / 2.0;
            ui.painter().galley(at, g, p.dim);
        }
    }
}

// ------------------------------------------------------------------ fonts

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

// helper layout
fn heading(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text).size(size).color(color).strong()
}

fn para(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text).size(size).color(color)
}

impl App {
fn ui(&mut self, ctx: &egui::Context) {
        let p = self.palette;
        egui::TopBottomPanel::top("top")
            .frame(egui::Frame::none().fill(p.bg).inner_margin(egui::Margin::symmetric(22.0, 14.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(heading("swara", 22.0, p.accent));
                    ui.add_space(18.0);
                    let tabs = [Tab::Home, Tab::Cari, Tab::Koleksi];
                    for t in tabs {
                        let label = match t {
                            Tab::Home => "Beranda",
                            Tab::Cari => "Cari",
                            Tab::Koleksi => "Koleksi",
                        };
                        let active = self.tab == t;
                        if self.pill(ui, label, active).clicked() {
                            self.tab = t;
                            self.view = if t == Tab::Home { View::Feed } else { View::Collection };
                            if t == Tab::Koleksi && self.koleksi.is_none() {
                                self.worker.send(Job::Browse("FEmusic_liked_playlists".into()));
                            }
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let glyph = if p.dark { "☀" } else { "☾" };
                        if self
                            .icon_button(ui, glyph, 34.0, false)
                            .on_hover_text("tema")
                            .clicked()
                        {
                            self.palette = Palette::from_seed(!p.dark, self.seed);
                        }
                        if self.progress.playing {
                            ui.label(para("● memutar", 13.0, p.accent));
                        }
                    });
                });
            });

        if let Some(track) = self.current.clone() {
            self.player_bar(ctx, &track);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(p.bg).inner_margin(egui::Margin::same(22.0)))
            .show(ctx, |ui| match self.tab {
                Tab::Home => self.tab_home(ui),
                Tab::Cari => self.tab_cari(ui),
                Tab::Koleksi => self.tab_koleksi(ui),
            });

        if let Some((msg, at)) = self.toast.clone() {
            if at.elapsed() < Duration::from_secs(4) {
                egui::Area::new("toast".into())
                    .anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, -110.0))
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(self.palette.surface_hi)
                            .rounding(Rounding::same(14.0))
                            .inner_margin(egui::Margin::symmetric(16.0, 10.0))
                            .show(ui, |ui| {
                                ui.label(para(&msg, 14.0, self.palette.text));
                            });
                    });
                ctx.request_repaint_after(Duration::from_millis(400));
            } else {
                self.toast = None;
            }
        }
    }

    fn player_bar(&mut self, ctx: &egui::Context, track: &Track) {
        let p = self.palette;
        let dur = if self.progress.dur > 0.0 {
            self.progress.dur
        } else {
            track.secs
        };
        egui::TopBottomPanel::bottom("player")
            .frame(
                egui::Frame::none()
                    .fill(p.surface)
                    .rounding(Rounding::same(20.0))
                    .inner_margin(egui::Margin::symmetric(18.0, 12.0))
                    .outer_margin(egui::Margin::symmetric(22.0, 10.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            if self.thumb_widget(ui, &track.thumb, 54.0, 12).clicked() {
                                self.show_full = true;
                            }
                            ui.vertical(|ui| {
                                ui.label(heading(&track.title, 15.0, p.text));
                                ui.label(para(&track.artist, 13.0, p.dim));
                            });
                        });
                    });
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            if self.icon_button(ui, "⏮", 34.0, false).clicked() {
                                self.next_track(-1);
                            }
                            let glyph = if self.loading {
                                "…"
                            } else if self.progress.playing {
                                "⏸"
                            } else {
                                "▶"
                            };
                            if self.icon_button(ui, glyph, 46.0, true).clicked() {
                                self.toggle_play();
                            }
                            if self.icon_button(ui, "⏭", 34.0, false).clicked() {
                                self.next_track(1);
                            }
                            ui.add_space(14.0);
                            ui.label(para(&fmt_time(self.progress.pos), 12.0, p.dim));
                            let w = (ui.available_width() - 110.0).clamp(120.0, 560.0);
                            let mut v = self.drag_pos.unwrap_or(self.progress.pos);
                            let bar = ProgressBar::new(if dur > 0.0 {
                                (v / dur) as f32
                            } else {
                                0.0
                            })
                            .desired_width(w)
                            .desired_height(6.0)
                            .fill(p.accent)
                            .rounding(Rounding::same(3.0));
                            let resp = ui.add(bar);
                            if resp.hovered() || self.drag_pos.is_some() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.dragged() || resp.clicked() {
                                if let Some(pos) = resp.interact_pointer_pos() {
                                    let frac = ((pos.x - resp.rect.left())
                                        / resp.rect.width().max(1.0))
                                    .clamp(0.0, 1.0) as f64;
                                    v = frac * dur;
                                    self.drag_pos = if resp.dragged() { Some(v) } else { None };
                                    if resp.drag_stopped() || resp.clicked() {
                                        self.player.seek(v);
                                        self.drag_pos = None;
                                    }
                                }
                            }
                            ui.label(para(&fmt_time(dur), 12.0, p.dim));
                            ui.add_space(8.0);
                            let mut vol = self.progress.volume;
                            if ui
                                .add(
                                    egui::Slider::new(&mut vol, 0.0..=1.0)
                                        .show_value(false)
                                        .trailing_fill(true),
                                )
                                .changed()
                            {
                                self.player.set_volume(vol);
                            }
                        });
                    });
                });
            });
    }

    fn tab_home(&mut self, ui: &mut Ui) {
        let p = self.palette;
        if !self.home_err.is_empty() && self.home.is_empty() {
            ui.label(para(&self.home_err, 14.0, p.dim));
        }
        if self.home.is_empty() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(para("memuat beranda", 14.0, p.dim));
            });
            return;
        }
        let rows = self.home.clone();
        ScrollArea::vertical()
            .id_salt("home")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for row in rows.iter() {
                    ui.add_space(10.0);
                    ui.label(heading(&row.title, 17.0, p.text));
                    ui.add_space(4.0);
                    ScrollArea::horizontal()
                        .id_salt(format!("row{}", row.title))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for card in row.items.iter() {
                                    self.card_tile(ui, card, 150.0);
                                }
                            });
                        });
                }
                ui.add_space(30.0);
            });
    }

    fn card_tile(&mut self, ui: &mut Ui, card: &Card, size: f32) {
        let p = self.palette;
        let ctx = ui.ctx().clone();
        self.ensure_thumb(&ctx, &card.thumb);
        ui.allocate_ui_with_layout(
            vec2(size, size + 52.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(size);
                if self.thumb_widget(ui, &card.thumb, size, 16).clicked() {
                    self.open_card(card);
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new(&card.title)
                        .size(13.0)
                        .color(p.text)
                        .strong(),
                );
                ui.label(para(&short(&card.sub, 26), 12.0, p.dim));
            },
        );
    }

    fn open_card(&mut self, card: &Card) {
        if card.kind == "song" {
            let t = Track {
                id: card.id.clone(),
                title: card.title.clone(),
                artist: card.sub.split('•').next().unwrap_or("").trim().to_string(),
                album: String::new(),
                thumb: card.thumb.clone(),
                dur: String::new(),
                secs: 0.0,
            };
            self.play_track(t, None);
        } else {
            self.tab = Tab::Koleksi;
            self.view = View::Collection;
            self.koleksi = None;
            self.worker.send(Job::Browse(card.id.clone()));
        }
    }

    fn tab_cari(&mut self, ui: &mut Ui) {
        let p = self.palette;
        let mut submit = false;
        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("cari lagu, artis, album")
                    .desired_width(420.0)
                    .margin(vec2(12.0, 9.0)),
            );
            if resp.changed() {
                self.suggest_at = Some(Instant::now());
                self.show_suggest = true;
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit = true;
            }
            if ui.button("Cari").clicked() {
                submit = true;
            }
            if resp.has_focus() && !self.suggest.is_empty() && self.show_suggest {
                ui.ctx().request_repaint_after(Duration::from_millis(200));
            }
        });

        if let Some(at) = self.suggest_at {
            if at.elapsed() > Duration::from_millis(260) && !self.query.is_empty() {
                if self.suggest.is_empty() || self.suggest.first() != Some(&self.query) {
                    self.worker.send(Job::Suggest(self.query.clone()));
                }
                self.suggest_at = None;
            }
        }
        if submit && !self.query.is_empty() {
            self.searched = self.query.clone();
            self.show_suggest = false;
            self.results.clear();
            self.worker.send(Job::Search(self.query.clone()));
        }

        if self.show_suggest && !self.suggest.is_empty() {
            let list = self.suggest.clone();
            for s in list {
                if ui
                    .add(
                        egui::Button::new(para(&s, 14.0, p.dim))
                            .fill(Color32::TRANSPARENT)
                            .rounding(Rounding::same(8.0)),
                    )
                    .clicked()
                {
                    self.query = s;
                    self.show_suggest = false;
                    self.searched = self.query.clone();
                    self.worker.send(Job::Search(self.query.clone()));
                }
            }
        }

        ui.add_space(8.0);
        if self.results.is_empty() {
            if !self.searched.is_empty() {
                ui.label(para("tidak ada hasil", 14.0, p.dim));
            }
            return;
        }
        let results = self.results.clone();
        ScrollArea::vertical()
            .id_salt("results")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, t) in results.iter().enumerate() {
                    if self.track_row(ui, t, i + 1) {
                        let q = results.clone();
                        self.play_track(t.clone(), Some(q));
                    }
                }
                ui.add_space(30.0);
            });
    }

    fn tab_koleksi(&mut self, ui: &mut Ui) {
        let p = self.palette;
        if self.view == View::Feed {
            ui.label(heading("Koleksi", 20.0, p.text));
            let mut open = None;
            ui.horizontal(|ui| {
                for (label, id) in [
                    ("Playlist saya", "FEmusic_liked_playlists"),
                    ("Lagu disukai", "FEmusic_liked_videos"),
                ] {
                    if self.pill(ui, label, false).clicked() {
                        open = Some(id.to_string());
                    }
                }
            });
            if let Some(id) = open {
                self.koleksi = None;
                self.view = View::Collection;
                self.worker.send(Job::Browse(id));
            }
            return;
        }
        let Some(c) = self.koleksi.clone() else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(para("memuat koleksi", 14.0, p.dim));
            });
            return;
        };
        ui.horizontal(|ui| {
            if self.icon_button(ui, "←", 32.0, false).clicked() {
                self.view = View::Feed;
            }
            ui.label(heading(&c.title, 20.0, p.text));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !c.tracks.is_empty() && ui.button("Putar semua").clicked() {
                    let q = c.tracks.clone();
                    if let Some(first) = q.first().cloned() {
                        self.play_track(first, Some(q));
                    }
                }
            });
        });
        ui.label(para(&c.sub, 13.0, p.dim));
        ui.add_space(6.0);
        let tracks = c.tracks.clone();
        ScrollArea::vertical()
            .id_salt("coll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, t) in tracks.iter().enumerate() {
                    if self.track_row(ui, t, i + 1) {
                        let q = tracks.clone();
                        self.play_track(t.clone(), Some(q));
                    }
                }
                if tracks.is_empty() {
                    ui.label(para("belum ada lagu di sini", 14.0, p.dim));
                }
                ui.add_space(30.0);
            });
    }

    /// Satu baris lagu; true kalau diklik (putar).
    fn track_row(&mut self, ui: &mut Ui, t: &Track, idx: usize) -> bool {
        let p = self.palette;
        let playing_this = self.current.as_ref().map(|c| c.id == t.id).unwrap_or(false);
        let ctx = ui.ctx().clone();
        self.ensure_thumb(&ctx, &t.thumb);
        let mut clicked = false;
        egui::Frame::none()
            .fill(Color32::TRANSPARENT)
            .rounding(Rounding::same(12.0))
            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_width(ui.available_width());
                    let resp = ui.allocate_response(vec2(ui.available_width(), 46.0), Sense::click());
                    let rect = resp.rect;
                    if resp.hovered() {
                        ui.painter()
                            .rect_filled(rect, Rounding::same(12.0), p.surface);
                    }
                    let mut x = rect.left() + 4.0;
                    let mid = rect.center().y;
                    ui.painter().text(
                        egui::pos2(x + 12.0, mid),
                        egui::Align2::CENTER_CENTER,
                        if playing_this {
                            "▶".to_string()
                        } else {
                            idx.to_string()
                        },
                        FontId::proportional(13.0),
                        if playing_this { p.accent } else { p.dim },
                    );
                    x += 34.0;
                    let img_rect =
                        egui::Rect::from_min_size(egui::pos2(x, mid - 18.0), vec2(36.0, 36.0));
                    ui.painter()
                        .rect_filled(img_rect, Rounding::same(9.0), p.surface_hi);
                    if let Some(Some(tex)) = self.thumbs.get(&t.thumb) {
                        egui::Image::new((tex.id(), tex.size_vec2()))
                            .fit_to_exact_size(vec2(36.0, 36.0))
                            .rounding(Rounding::same(9.0))
                            .paint_at(ui, img_rect);
                    }
                    x += 48.0;
                    let max_w = (rect.width() - 240.0).max(120.0);
                    let title = short(&t.title, ((max_w / 7.0) as usize).max(18));
                    ui.painter().text(
                        egui::pos2(x, mid - 9.0),
                        egui::Align2::LEFT_CENTER,
                        title,
                        FontId::proportional(14.0),
                        if playing_this { p.accent } else { p.text },
                    );
                    ui.painter().text(
                        egui::pos2(x, mid + 10.0),
                        egui::Align2::LEFT_CENTER,
                        short(&t.artist, 34),
                        FontId::proportional(12.0),
                        p.dim,
                    );
                    ui.painter().text(
                        egui::pos2(rect.right() - 12.0, mid),
                        egui::Align2::RIGHT_CENTER,
                        &t.dur,
                        FontId::proportional(12.0),
                        p.dim,
                    );
                    clicked = resp.clicked();
                });
            });
        clicked
    }
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

impl eframe::App for App {
    fn clear_color(&self, _v: &eframe::egui::Visuals) -> [f32; 4] {
        let c = self.palette.bg;
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            1.0,
        ]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(out) = self.worker.rx.try_recv() {
            self.apply_out(out, ctx);
        }
        while let Ok((u, tex)) = self.thumb_rx.try_recv() {
            self.thumb_pending.remove(&u);
            self.thumbs.insert(u, tex);
        }
        while let Ok((u, bytes)) = self.art_rx.try_recv() {
            self.art_pending = false;
            if let Some(b) = bytes {
                if let Ok(img) = image::load_from_memory(&b) {
                    self.set_seed(&img);
                    self.art = Some(load_texture(ctx, &format!("cover{}", hash_name(&u)), &img));
                }
            }
        }

        let prog = self.player.poll();
        let ended = prog.ended;
        self.progress = prog;
        if ended {
            self.next_track(1);
        }
        if self.loading || self.progress.playing || self.toast.is_some() {
            ctx.request_repaint_after(Duration::from_millis(80));
        }

        let p = self.palette;
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = p.bg;
        style.visuals.window_fill = p.surface;
        style.visuals.extreme_bg_color = p.surface_hi;
        style.visuals.override_text_color = Some(p.text);
        style.visuals.hyperlink_color = p.accent;
        style.visuals.selection.bg_fill = p.accent.gamma_multiply(0.45);
        style.spacing.item_spacing = vec2(10.0, 8.0);
        style.spacing.button_padding = vec2(12.0, 7.0);
        for w in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.noninteractive,
        ] {
            w.bg_fill = p.surface;
            w.weak_bg_fill = p.surface;
            w.bg_stroke = Stroke::NONE;
            w.fg_stroke = Stroke::new(1.0, p.text);
            w.rounding = Rounding::same(12.0);
        }
        style.visuals.widgets.hovered.bg_fill = p.surface_hi;
        style.visuals.widgets.hovered.weak_bg_fill = p.surface_hi;
        ctx.set_style(style);

        self.ui(ctx);
    }
}
