//! Tugas HTTP di latar: memuat metadata dan audio tanpa membekukan UI.

use crate::api::{self, Api, Card, Collection, Lyrics, Row, Stream, Track};
use anyhow::Result;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

pub enum Job {
    Home,
    Search(String),
    Suggest(String),
    Browse(String),
    Radio(String),
    Lyrics(Track),
    /// Ambil seluruh isi playlist lalu jadikan antrean (dipakai `SWARA_LIST`).
    PlayList(String),
    /// Siapkan lagu: ambil URL stream, unduh audio, ambil sampul & lirik.
    Load(Track),
    Login,
    Logout,
    Library,
    Rate(String, bool),
    /// Tanya status suka lagu ini ke akun (LIKE/INDIFFERENT/DISLIKE).
    Rating(String),
    /// Simpan lagu ke penyimpanan privat aplikasi untuk diputar tanpa jaringan.
    Unduh(Track),
}

pub enum Out {
    Home(Vec<Row>),
    Search(String, Vec<Track>),
    Suggest(String, Vec<String>),
    Browse(Collection),
    Radio(String, Vec<Track>),
    Lyrics(String, Lyrics),
    /// isi playlist siap diputar berurutan
    PlayList(Vec<Track>),
    /// track, stream, audio (mengalir), sampul (bytes opsional)
    Loaded(Track, Stream, Arc<crate::stream::Buf>, Option<Vec<u8>>),
    Logged(Option<String>),
    Library(Vec<Card>),
    Rated(String, bool),
    /// Jawaban status suka dari akun untuk satu lagu.
    Rating(String, bool),
    /// Hasil simpan lagu offline: (id, berhasil).
    Terunduh(String, bool),
    Failed(String),
}

pub struct Worker {
    tx: Sender<Job>,
    pub rx: Receiver<Out>,
    ctx: Arc<std::sync::Mutex<Option<egui::Context>>>,
}

impl Worker {
    pub fn new(api: Arc<Api>) -> Self {
        let (job_tx, job_rx) = channel::<Job>();
        let (out_tx, out_rx) = channel::<Out>();
        let ctx: Arc<std::sync::Mutex<Option<egui::Context>>> =
            Arc::new(std::sync::Mutex::new(None));
        let ctx2 = ctx.clone();
        std::thread::Builder::new()
            .name("worker".into())
            .spawn(move || run(api, job_rx, out_tx, ctx2))
            .expect("spawn worker");
        Worker {
            tx: job_tx,
            rx: out_rx,
            ctx,
        }
    }

    pub fn send(&self, job: Job) {
        let _ = self.tx.send(job);
    }

    /// Salinan kanal masuk saja, untuk pengendali luar (soket kendali):
    /// `Receiver` tidak bisa di-clone, sedangkan thread lain cukup bisa
    /// mengirim pekerjaan dan membangunkan jendela.
    pub fn pintu(&self) -> Pintu {
        Pintu {
            tx: self.tx.clone(),
            ctx: self.ctx.clone(),
        }
    }

    /// Tanpa ini hasil dari thread lain baru terlihat setelah jendela
    /// disentuh: egui hanya menggambar kalau ada yang meminta.
    pub fn attach(&self, ctx: egui::Context) {
        *self.ctx.lock().unwrap() = Some(ctx);
    }
}

/// Pengirim pekerjaan dari luar UI (soket kendali).
#[derive(Clone)]
pub struct Pintu {
    tx: Sender<Job>,
    ctx: Arc<std::sync::Mutex<Option<egui::Context>>>,
}

impl Pintu {
    pub fn send(&self, job: Job) {
        let _ = self.tx.send(job);
        if let Some(c) = self.ctx.lock().ok().and_then(|g| g.clone()) {
            c.request_repaint();
        }
    }
}

fn run(
    api: Arc<Api>,
    jobs: Receiver<Job>,
    out: Sender<Out>,
    ctx: Arc<std::sync::Mutex<Option<egui::Context>>>,
) {
    while let Ok(job) = jobs.recv() {
        let result = handle(&api, job);
        match result {
            Ok(o) => {
                if out.send(o).is_err() {
                    return;
                }
            }
            Err(e) => {
                if out.send(Out::Failed(format!("{e:#}"))).is_err() {
                    return;
                }
            }
        }
        if let Some(c) = ctx.lock().ok().and_then(|g| g.clone()) {
            c.request_repaint();
        }
    }
}

fn handle(api: &Api, job: Job) -> Result<Out> {
    Ok(match job {
        Job::Home => Out::Home(api.home()?),
        Job::Search(q) => {
            let r = api.search(&q, 30)?;
            Out::Search(q, r)
        }
        Job::Suggest(q) => {
            let r = api.suggest(&q).unwrap_or_default();
            Out::Suggest(q, r)
        }
        Job::Browse(id) => Out::Browse(api.browse(&id)?),
        Job::PlayList(id) => {
            let c = api.browse(&id)?;
            Out::PlayList(c.tracks)
        }
        Job::Radio(id) => {
            let r = api.radio(&id, 25).unwrap_or_default();
            Out::Radio(id, r)
        }
        Job::Lyrics(t) => Out::Lyrics(
            t.id.clone(),
            api.lyrics(&t.title, &t.artist, t.secs),
        ),
        Job::Login => Out::Logged(Some(api.login()?)),
        Job::Logout => {
            api.logout();
            Out::Logged(None)
        }
        Job::Library => Out::Library(api.library()?),
        Job::Rate(id, like) => {
            api.rate(&id, like)?;
            Out::Rated(id, like)
        }
        Job::Rating(id) => {
            let suka = api.like_status(&id).unwrap_or(false);
            eprintln!("[swara] status suka {id}: {}", if suka { "LIKE" } else { "bukan" });
            Out::Rating(id, suka)
        }
        Job::Unduh(t) => {
            let s = api.stream(&t.id)?;
            let bytes = api.fetch_audio(&s.url)?;
            crate::offline::simpan_lagu(&t, s.dur, &bytes)?;
            eprintln!(
                "[swara] tersimpan offline {} ({} KB)",
                t.id,
                bytes.len() / 1024
            );
            Out::Terunduh(t.id, true)
        }
        Job::Load(t) => {
            // Sudah tersimpan offline? Putar dari berkas, tanpa jaringan sama
            // sekali — inilah gunanya fitur simpan.
            if let Some(bytes) = crate::offline::baca(&t.id) {
                let info = Stream {
                    url: String::new(),
                    itag: 0,
                    mime: "audio/mp4".into(),
                    kbps: 0,
                    title: t.title.clone(),
                    artist: t.artist.clone(),
                    dur: t.secs,
                    thumb: t.thumb.clone(),
                };
                let audio = crate::stream::Buf::new();
                audio.push(&bytes);
                audio.finish();
                let art = api::thumb_or(&t.thumb).and_then(|u| api.fetch_image(&u).ok());
                return Ok(Out::Loaded(t, info, audio, art));
            }
            // Sudah disiapkan di latar? langsung main.
            if let Some((s, audio)) = api.take_ready(&t.id) {
                let thumb = s.thumb.clone();
                let art = api::thumb_or(&thumb).and_then(|u| api.fetch_image(&u).ok());
                return Ok(Out::Loaded(t, s, audio, art));
            }
            let s = api.stream(&t.id)?;
            // Unduhan jalan di latar; `Loaded` dikirim begitu potongan pertama
            // ada, bukan setelah file utuh.
            let audio = api.fetch_audio_stream(&s.url, &t.id);
            let thumb = s.thumb.clone();
            let art = api::thumb_or(&thumb).and_then(|u| api.fetch_image(&u).ok());
            Out::Loaded(t, s, audio, art)
        }
    })
}
