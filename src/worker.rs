//! Tugas HTTP di latar: memuat metadata dan audio tanpa membekukan UI.

use crate::api::{self, Api, Collection, Lyrics, Row, Stream, Track};
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
    /// Siapkan lagu: ambil URL stream, unduh audio, ambil sampul & lirik.
    Load(Track),
}

pub enum Out {
    Home(Vec<Row>),
    Search(String, Vec<Track>),
    Suggest(String, Vec<String>),
    Browse(Collection),
    Radio(String, Vec<Track>),
    Lyrics(String, Lyrics),
    /// track, stream, audio, sampul (bytes opsional)
    Loaded(Track, Stream, Vec<u8>, Option<Vec<u8>>),
    Failed(String),
}

pub struct Worker {
    tx: Sender<Job>,
    pub rx: Receiver<Out>,
}

impl Worker {
    pub fn new(api: Arc<Api>) -> Self {
        let (job_tx, job_rx) = channel::<Job>();
        let (out_tx, out_rx) = channel::<Out>();
        std::thread::Builder::new()
            .name("worker".into())
            .spawn(move || run(api, job_rx, out_tx))
            .expect("spawn worker");
        Worker {
            tx: job_tx,
            rx: out_rx,
        }
    }

    pub fn send(&self, job: Job) {
        let _ = self.tx.send(job);
    }
}

fn run(api: Arc<Api>, jobs: Receiver<Job>, out: Sender<Out>) {
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
        Job::Radio(id) => {
            let r = api.radio(&id, 25).unwrap_or_default();
            Out::Radio(id, r)
        }
        Job::Lyrics(t) => Out::Lyrics(
            t.id.clone(),
            api.lyrics(&t.title, &t.artist, t.secs),
        ),
        Job::Load(t) => {
            let s = api.stream(&t.id)?;
            let audio = api.fetch_audio(&s.url)?;
            let thumb = s.thumb.clone();
            let art = api::thumb_or(&thumb).and_then(|u| api.fetch_image(&u).ok());
            Out::Loaded(t, s, audio, art)
        }
    })
}
