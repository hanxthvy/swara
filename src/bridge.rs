//! Jembatan MPRIS, sengaja di luar `App::update`.
//!
//! Di Wayland, jendela yang tidak terlihat (pindah workspace) tidak menerima
//! frame callback, jadi `update` berhenti dipanggil. Kalau kendali panel
//! sistem dan laporan `Position` tinggal di sana, gejalanya: lagu habis tapi
//! islet tetap menampilkan judul lama, bar durasi membeku, dan geser bar dari
//! islet mendarat di kanal yang tidak pernah dibaca.
//!
//! Thread ini membaca perintah MPRIS dan mengabarkan keadaan pemutar ke D-Bus
//! tanpa bantuan UI. `Position` dikirim tanpa `PropertiesChanged` (sesuai
//! spesifikasi), jadi cukup di-setiapkan tiap 200 ms supaya pembaca yang
//! mem-poll selalu melihat angka baru.

use crate::conductor::Ctl as CondCtl;
use crate::mpris::{Ctl, Event, Mpris};
use crate::player::{Progress, Remote};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn spawn(
    mpris: Option<Mpris>,
    remote: Remote,
    cond: CondCtl,
    shared: Arc<Mutex<crate::conductor::Cond>>,
    snap: Arc<Mutex<Progress>>,
) -> Option<Ctl> {
    let m = mpris?;
    let ctl = m.ctl();
    let rx = m.rx;
    let ui = ctl.clone();
    let _ = std::thread::Builder::new()
        .name("mpris-jembatan".into())
        .spawn(move || {
            let mut last_playing: Option<bool> = None;
            let mut last_seq = 0u64;
            // Jam pembanding: kalau posisi berhenti maju (unduhan tersendat)
            // atau melompat, pembaca yang mengekstrapolasi sendiri akan
            // melenceng. Dorong `Seeked` supaya mereka menyegarkan diri.
            let mut prev_pos = 0.0f64;
            let mut prev_at = std::time::Instant::now();
            let mut last_sync = std::time::Instant::now();
            loop {
                while let Ok(e) = rx.try_recv() {
                    let p = snap.lock().unwrap().clone();
                    match e {
                        Event::Play => {
                            if !p.playing {
                                remote.resume();
                            }
                        }
                        Event::Pause => {
                            if p.playing {
                                remote.pause();
                            }
                        }
                        Event::PlayPause => {
                            if p.playing {
                                remote.pause();
                            } else {
                                remote.resume();
                            }
                        }
                        Event::Next => cond.step(1),
                        Event::Prev => cond.step(-1),
                        Event::SeekBy(off) => remote.seek(p.pos + off),
                        Event::SeekTo(pos) => remote.seek(pos),
                        Event::Stop => remote.stop(),
                    }
                }

                let p = snap.lock().unwrap().clone();
                if last_playing != Some(p.playing) {
                    last_playing = Some(p.playing);
                    ctl.status(p.playing);
                }
                ctl.position(p.pos);

                let now = std::time::Instant::now();
                let dt = now.duration_since(prev_at).as_secs_f64();
                let drift = (p.pos - prev_pos - dt).abs();
                if p.playing && (drift > 0.30 || now.duration_since(last_sync).as_secs() >= 5) {
                    ctl.seeked(p.pos);
                    last_sync = now;
                }
                prev_pos = p.pos;
                prev_at = now;

                // Lagu yang diputar konduktor (jendela tersembunyi) tidak
                // lewat UI, jadi metadata-nya juga harus dikirim dari sini.
                let (seq, track, stream) = {
                    let g = shared.lock().unwrap();
                    (g.seq, g.track.clone(), g.stream.clone())
                };
                if seq != last_seq {
                    last_seq = seq;
                    if let Some(t) = track {
                        let dur = stream
                            .as_ref()
                            .map(|s| if s.dur > 0.0 { s.dur } else { t.secs })
                            .unwrap_or(t.secs);
                        let art = stream
                            .as_ref()
                            .map(|s| s.thumb.clone())
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| t.thumb.clone());
                        ctl.track(&t, dur, &art);
                        ctl.status(true);
                    }
                }

                std::thread::sleep(Duration::from_millis(200));
            }
        });
    Some(ui)
}
