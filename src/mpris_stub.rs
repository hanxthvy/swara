//! Pengganti `mpris.rs` di sistem selain Linux.
//!
//! MPRIS itu protokol D-Bus, jadi hanya ada di Linux. Di Windows/macOS Swara
//! tetap memutar musik dan melaporkan posisi ke UI, hanya tidak mengumumkannya
//! ke panel sistem. Bentuk API-nya sengaja sama supaya sisa kode tidak perlu
//! tahu sistem apa yang sedang dipakai.

use crate::api::Track;
use std::sync::mpsc::{channel, Receiver};

#[derive(Clone, Debug)]
pub enum Cmd {
    Track {
        track: Track,
        dur: f64,
        art: String,
    },
    Status(bool),
    Position(f64),
    Seeked(f64),
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

/// Tidak ada yang dikirim ke mana pun; semua metode jadi diam.
#[derive(Clone)]
pub struct Ctl;

impl Ctl {
    pub fn send(&self, _cmd: Cmd) {}
    pub fn track(&self, _t: &Track, _dur: f64, _art: &str) {}
    pub fn status(&self, _playing: bool) {}
    pub fn position(&self, _pos: f64) {}
    pub fn seeked(&self, _pos: f64) {}
    pub fn clear(&self) {}
}

pub struct Mpris {
    pub rx: Receiver<Event>,
}

impl Mpris {
    pub fn ctl(&self) -> Ctl {
        Ctl
    }

    pub fn spawn() -> Option<Self> {
        let (_tx, rx) = channel::<Event>();
        Some(Mpris { rx })
    }
}
