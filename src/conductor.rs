//! Penentu lagu berikutnya, sengaja di luar UI.
//!
//! Kalau jendela Swara disembunyikan (pindah workspace), compositor Wayland
//! berhenti mengirim frame callback sehingga `egui::App::update` tidak pernah
//! dipanggil lagi. Logika "lagu habis, putar berikutnya" yang tinggal di dalam
//! `update` jadi mati bersama jendelanya. Konduktor ini memegang sendiri
//! antrean dan memerintah pemutar lewat `Remote`, jadi lagu tetap berlanjut
//! walau tidak ada satu piksel pun digambar.

use crate::api::{Api, Stream, Track};
use crate::player::{Progress, Remote};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct Cond {
    pub queue: Vec<Track>,
    pub idx: usize,
    /// Naik setiap konduktor mulai memutar lagu; UI memakainya untuk tahu
    /// bahwa lagu berganti tanpa ia sendiri yang meminta.
    pub seq: u64,
    pub track: Option<Track>,
    pub stream: Option<Stream>,
    pub early: u8,
}

impl Cond {
    /// Samakan antrean dengan yang dipakai UI, sekaligus menunjuk lagu aktif.
    pub fn set_queue(&mut self, queue: &[Track], id: &str) {
        self.queue = queue.to_vec();
        self.idx = queue.iter().position(|t| t.id == id).unwrap_or(0);
    }

    fn after_current(&self) -> Option<Track> {
        self.queue.get(self.idx + 1).cloned()
    }
}

pub fn spawn(api: Arc<Api>, remote: Remote, mirror: Receiver<Progress>) -> Arc<Mutex<Cond>> {
    let cond = Arc::new(Mutex::new(Cond::default()));
    let shared = cond.clone();

    let _ = std::thread::Builder::new()
        .name("konduktor".into())
        .spawn(move || {
            while let Ok(p) = mirror.recv() {
                if !p.ended {
                    continue;
                }
                // Lagu yang "habis" padahal baru sekejap berarti payload-nya
                // rusak: ulangi, dan setelah tiga kali diamkan saja.
                if p.dur > 0.0 && p.pos < 1.5 {
                    let (n, cur) = {
                        let mut g = shared.lock().unwrap();
                        g.early += 1;
                        (g.early, g.track.clone())
                    };
                    if n >= 3 {
                        remote.pause();
                        continue;
                    }
                    if let Some(t) = cur {
                        if let Some(s) = api.stream(&t.id).ok() {
                            if let Ok(a) = api.fetch_audio(&s.url) {
                                remote.play(a, 0.0, s.dur);
                                continue;
                            }
                        }
                    }
                    remote.pause();
                    continue;
                }

                let next = shared.lock().unwrap().after_current();
                let Some(t) = next else { continue };

                eprintln!("[konduktor] lanjut ke {}", t.id);
                let Ok(s) = api.stream(&t.id) else { continue };
                let Ok(a) = api.fetch_audio(&s.url) else {
                    continue;
                };
                remote.play(a, 0.0, s.dur);
                let mut g = shared.lock().unwrap();
                g.early = 0;
                g.seq += 1;
                if let Some(i) = g.queue.iter().position(|x| x.id == t.id) {
                    g.idx = i;
                }
                g.track = Some(t);
                g.stream = Some(s);
            }
        });

    cond
}
