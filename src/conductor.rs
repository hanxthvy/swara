//! Penentu lagu berikutnya, sengaja di luar UI.
//!
//! Kalau jendela Swara disembunyikan (pindah workspace), compositor Wayland
//! berhenti mengirim frame callback sehingga `egui::App::update` tidak pernah
//! dipanggil lagi. Logika "lagu habis, putar berikutnya" yang tinggal di dalam
//! `update` jadi mati bersama jendelanya. Konduktor ini memegang sendiri
//! antrean dan memerintah pemutar lewat `Remote`, jadi lagu tetap berlanjut
//! walau tidak ada satu piksel pun digambar.
//!
//! Perintah `Next`/`Prev` dari panel sistem masuk lewat kanal yang sama, bukan
//! lewat UI — sama alasannya.

use crate::api::{Api, Stream, Track};
use crate::player::{Progress, Remote};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
        if self.queue.is_empty() {
            return None;
        }
        // Habis di ujung: putar lagi dari awal, jangan berhenti.
        self.queue
            .get(self.idx + 1)
            .cloned()
            .or_else(|| self.queue.first().cloned())
    }
}

pub enum Cmd {
    /// Lompat `n` lagu dari posisi sekarang (1 = berikutnya, -1 = sebelumnya).
    Step(isize),
}

/// Pegangan untuk mengirim perintah ke konduktor dari thread lain.
#[derive(Clone)]
pub struct Ctl {
    tx: Sender<Cmd>,
}

impl Ctl {
    pub fn step(&self, n: isize) {
        let _ = self.tx.send(Cmd::Step(n));
    }
}

pub fn spawn(
    api: Arc<Api>,
    remote: Remote,
    snap: Arc<Mutex<Progress>>,
    ends: Arc<AtomicU64>,
) -> (Arc<Mutex<Cond>>, Ctl) {
    let cond = Arc::new(Mutex::new(Cond::default()));
    let shared = cond.clone();
    let (tx, rx) = channel::<Cmd>();
    let ctl = Ctl { tx };

    let _ = std::thread::Builder::new()
        .name("konduktor".into())
        .spawn(move || {
            let mut seen = ends.load(Ordering::SeqCst);
            loop {
                match rx.recv_timeout(Duration::from_millis(120)) {
                    Ok(Cmd::Step(n)) => {
                        let next = {
                            let g = shared.lock().unwrap();
                            let len = g.queue.len() as isize;
                            if len == 0 {
                                None
                            } else {
                                let i = (g.idx as isize + n).rem_euclid(len) as usize;
                                g.queue.get(i).cloned()
                            }
                        };
                        if let Some(t) = next {
                            eprintln!("[konduktor] step {n} ke {}", t.id);
                            start(&api, &remote, &shared, t);
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }

                let n = ends.load(Ordering::SeqCst);
                if n == seen {
                    continue;
                }
                seen = n;
                let p = snap.lock().unwrap().clone();
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
                            let a = api.fetch_audio_stream(&s.url, &t.id);
                            remote.play(a, 0.0, s.dur);
                            continue;
                        }
                    }
                    remote.pause();
                    continue;
                }

                let next = shared.lock().unwrap().after_current();
                let Some(t) = next else { continue };

                eprintln!("[konduktor] lanjut ke {}", t.id);
                // Jaringan ponsel bisa putus sebentar saat layar mati. Dulu satu
                // kegagalan membuat pemutaran berhenti diam-diam; sekarang dicoba
                // beberapa kali, lalu lompat ke lagu berikutnya.
                let mut jalan = false;
                for coba in 0..3u64 {
                    if start(&api, &remote, &shared, t.clone()) {
                        jalan = true;
                        break;
                    }
                    eprintln!("[konduktor] gagal memulai {}, coba {}", t.id, coba + 1);
                    std::thread::sleep(Duration::from_millis(800 * (coba + 1)));
                }
                if !jalan {
                    let lain = {
                        let mut g = shared.lock().unwrap();
                        g.idx = (g.idx + 1) % g.queue.len().max(1);
                        g.queue.get(g.idx).cloned()
                    };
                    if let Some(n) = lain {
                        if n.id != t.id {
                            eprintln!("[konduktor] lewati ke {}", n.id);
                            let _ = start(&api, &remote, &shared, n);
                        }
                    }
                }
            }
        });

    (cond, ctl)
}

/// Unduh lalu putar `t`, dan naikkan `seq` supaya UI (dan jembatan MPRIS) tahu
/// lagu berganti. `false` kalau unduhan gagal.
fn start(api: &Arc<Api>, remote: &Remote, shared: &Arc<Mutex<Cond>>, t: Track) -> bool {
    // Kalau lagu ini sudah disiapkan (`prefetch`), tidak ada tunda jaringan.
    let (s, a) = match api.take_ready(&t.id) {
        Some(pair) => pair,
        None => {
            let Ok(s) = api.stream(&t.id) else { return false };
            let a = api.fetch_audio_stream(&s.url, &t.id);
            (s, a)
        }
    };
    remote.play(a, 0.0, s.dur);
    let next = {
        let mut g = shared.lock().unwrap();
        g.early = 0;
        g.seq += 1;
        if let Some(i) = g.queue.iter().position(|x| x.id == t.id) {
            g.idx = i;
        }
        g.track = Some(t);
        g.stream = Some(s);
        g.queue.get(g.idx + 1).cloned()
    };
    if let Some(n) = next {
        api.prefetch(&n);
    }
    true
}
