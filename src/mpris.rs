//! MPRIS: supaya panel sistem (islet, waybar, playerctl) bisa membaca lagu
//! yang sedang diputar — judul, artis, album, panjang, dan posisi.
//!
//! Lirik tidak dikirim lewat MPRIS. Islet mencari liriknya sendiri dari judul,
//! artis, dan durasi, jadi yang penting metadata dan `Position` selalu akurat,
//! termasuk sesudah lagu digeser.

use crate::api::Track;
use mpris_server::{Metadata, PlaybackStatus, Player, Time, TrackId};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

pub enum Cmd {
    Track {
        track: Track,
        dur: f64,
        art: String,
    },
    Status(bool),
    Position(f64),
    Unsupported,
}

#[derive(Clone, Debug)]
pub enum Event {
    Play,
    Pause,
    PlayPause,
    Next,
    Prev,
    SeekBy(f64),
    SeekTo(f64),
    Stop,
}

pub struct Mpris {
    tx: Sender<Cmd>,
    pub rx: Receiver<Event>,
}

impl Mpris {
    pub fn spawn() -> Option<Self> {
        let (cmd_tx, cmd_rx) = channel::<Cmd>();
        let (ev_tx, ev_rx) = channel::<Event>();
        let ok = std::thread::Builder::new()
            .name("mpris".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        eprintln!("[mpris] runtime: {e}");
                        return;
                    }
                };
                rt.block_on(async move {
                    if let Err(e) = serve(cmd_rx, ev_tx).await {
                        eprintln!("[mpris] {e}");
                    }
                });
            })
            .is_ok();
        if !ok {
            return None;
        }
        Some(Mpris {
            tx: cmd_tx,
            rx: ev_rx,
        })
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }

    pub fn track(&self, t: &Track, dur: f64, art: &str) {
        self.send(Cmd::Track {
            track: t.clone(),
            dur,
            art: art.to_string(),
        });
    }

    pub fn status(&self, playing: bool) {
        self.send(Cmd::Status(playing));
    }

    pub fn position(&self, pos: f64) {
        self.send(Cmd::Position(pos));
    }

    pub fn clear(&self) {
        self.send(Cmd::Unsupported);
    }
}

async fn serve(rx: Receiver<Cmd>, events: Sender<Event>) -> anyhow::Result<()> {
    let player = Player::builder("io.github.hanxthvy.Swara")
        .identity("Swara")
        .desktop_entry("swara")
        .can_play(true)
        .can_pause(true)
        .can_go_next(true)
        .can_go_previous(true)
        .can_seek(true)
        .can_control(true)
        .build()
        .await?;

    // Callback hanya meneruskan niat; perubahan status dikirim balik oleh UI
    // lewat `Cmd::Status`, jadi tidak ada pekerjaan async di dalam callback.
    macro_rules! fwd {
        ($connect:ident, $ev:expr) => {{
            let tx = events.clone();
            player.$connect(move |_p: &Player| {
                let _ = tx.send($ev);
            });
        }};
    }
    fwd!(connect_play, Event::Play);
    fwd!(connect_pause, Event::Pause);
    fwd!(connect_play_pause, Event::PlayPause);
    fwd!(connect_next, Event::Next);
    fwd!(connect_previous, Event::Prev);
    {
        let tx = events.clone();
        player.connect_seek(move |_p: &Player, offset: Time| {
            let _ = tx.send(Event::SeekBy(offset.as_micros() as f64 / 1_000_000.0));
        });
    }
    {
        let tx = events.clone();
        player.connect_set_position(move |_p: &Player, _id: &TrackId, pos: Time| {
            let _ = tx.send(Event::SeekTo(pos.as_micros() as f64 / 1_000_000.0));
        });
    }

    // Keduanya harus hidup bersamaan: `run()` melayani D-Bus, `commands`
    // menerjemahkan perintah UI menjadi perubahan properti.
    tokio::join!(player.run(), commands(&player, rx));
    eprintln!("[mpris] layanan berhenti");
    Ok(())
}

async fn commands(player: &Player, rx: Receiver<Cmd>) {
    // `recv_timeout` akan memblokir thread runtime sehingga D-Bus berhenti
    // dilayani. Jadi: coba ambil tanpa menunggu, lalu tidur async sebentar.
    loop {
        match rx.try_recv() {
            Ok(Cmd::Track { track, dur, art }) => {
                let mut b = Metadata::builder()
                    .title(track.title.clone())
                    .artist(vec![track.artist.clone()])
                    .length(Time::from_micros((dur * 1_000_000.0) as i64));
                if !track.album.is_empty() {
                    b = b.album(track.album.clone());
                }
                if !art.is_empty() {
                    b = b.art_url(art);
                }
                let _ = player.set_metadata(b.build()).await;
                let _ = player.set_can_seek(true).await;
                player.set_position(Time::from_micros(0));
            }
            Ok(Cmd::Status(playing)) => {
                let st = if playing {
                    PlaybackStatus::Playing
                } else {
                    PlaybackStatus::Paused
                };
                let _ = player.set_playback_status(st).await;
            }
            Ok(Cmd::Position(pos)) => {
                player.set_position(Time::from_micros((pos * 1_000_000.0) as i64));
            }
            Ok(Cmd::Unsupported) => {
                let _ = player.set_playback_status(PlaybackStatus::Stopped).await;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
        }
    }
}
