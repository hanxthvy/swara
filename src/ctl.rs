//! Kendali dari luar jendela: satu soket Unix kecil + berkas keadaan "suka".
//!
//! MPRIS tidak punya konsep suka/tidak suka, dan `mpris-server` memegang
//! antarmuka D-Bus-nya sendiri. Jalan termurah: soket baris-perintah di
//! `$XDG_RUNTIME_DIR/swara-ctl.sock` untuk perintah, dan berkas
//! `$XDG_RUNTIME_DIR/swara-suka.json` untuk keadaan yang dibaca islet lewat
//! `FileView` (jadi tidak perlu polling).
//!
//! Perintah: `state` | `suka` | `tidak` | `toggle`. Balasannya satu baris JSON.
//! Semua ini hidup di thread sendiri, bukan di `App::update`.

use crate::worker::Pintu;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Lagu yang sedang main dan status sukanya. Ditulis UI maupun soket, dibaca
/// keduanya, supaya ikon di jendela dan di pill tidak pernah berbeda.
#[derive(Default, Clone)]
pub struct Keadaan {
    pub id: String,
    pub suka: bool,
    /// Ada sesi akun atau tidak. Tanpa ini tombol di islet bisa mengaku
    /// sudah menyukai lagu padahal permintaannya tidak pernah terkirim.
    pub masuk: bool,
}

pub type Bersama = Arc<Mutex<Keadaan>>;

pub fn bersama() -> Bersama {
    Arc::new(Mutex::new(Keadaan::default()))
}

fn dir_runtime() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

pub fn soket() -> PathBuf {
    dir_runtime().join("swara-ctl.sock")
}

pub fn berkas() -> PathBuf {
    dir_runtime().join("swara-suka.json")
}

/// Tulis keadaan untuk pembaca luar. Lewat berkas sementara lalu `rename`,
/// supaya pembaca tidak pernah menangkap JSON separuh.
pub fn tulis(k: &Bersama) {
    let isi = {
        let g = k.lock().unwrap();
        format!(
            "{{\"id\":\"{}\",\"suka\":{},\"masuk\":{}}}\n",
            g.id.replace('"', ""),
            g.suka,
            g.masuk
        )
    };
    let tmp = berkas().with_extension("tmp");
    if std::fs::write(&tmp, isi).is_ok() {
        let _ = std::fs::rename(&tmp, berkas());
    }
}

pub fn spawn(pintu: Pintu, k: Bersama) {
    let _ = std::thread::Builder::new()
        .name("kendali-luar".into())
        .spawn(move || {
            let p = soket();
            let _ = std::fs::remove_file(&p);
            let l = match UnixListener::bind(&p) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[swara] soket kendali gagal: {e}");
                    return;
                }
            };
            tulis(&k);
            for s in l.incoming() {
                match s {
                    Ok(s) => {
                        let _ = layani(s, &pintu, &k);
                    }
                    Err(_) => break,
                }
            }
        });
}

fn layani(mut s: UnixStream, pintu: &Pintu, k: &Bersama) -> std::io::Result<()> {
    s.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut baris = String::new();
    BufReader::new(s.try_clone()?).read_line(&mut baris)?;
    let jawab = jalankan(baris.trim(), pintu, k);
    s.write_all(format!("{jawab}\n").as_bytes())?;
    s.flush()
}

fn jalankan(cmd: &str, pintu: &Pintu, k: &Bersama) -> String {
    let cmd = if cmd.is_empty() { "state" } else { cmd };
    let (id, lama, masuk) = {
        let g = k.lock().unwrap();
        (g.id.clone(), g.suka, g.masuk)
    };
    let baru = match cmd {
        "state" | "keadaan" => None,
        "suka" | "like" => Some(true),
        "tidak" | "unlike" | "batal" => Some(false),
        "toggle" | "balik" => Some(!lama),
        lain => return format!("{{\"error\":\"perintah tidak dikenal: {lain}\"}}"),
    };
    if let Some(baru) = baru {
        if id.is_empty() {
            return "{\"error\":\"belum ada lagu\"}".into();
        }
        if !masuk {
            return "{\"error\":\"belum masuk akun\"}".into();
        }
        pintu.send(crate::worker::Job::Rate(id.clone(), baru));
        {
            let mut g = k.lock().unwrap();
            g.id = id.clone();
            g.suka = baru;
        }
        tulis(k);
    }
    let g = k.lock().unwrap();
    format!(
        "{{\"id\":\"{}\",\"suka\":{},\"masuk\":{}}}",
        g.id, g.suka, g.masuk
    )
}

/// Sisi klien untuk `swara --ctl <perintah>`.
pub fn kirim(cmd: &str) -> anyhow::Result<String> {
    let mut s = UnixStream::connect(soket())
        .map_err(|e| anyhow::anyhow!("swara tidak berjalan ({e})"))?;
    s.set_read_timeout(Some(Duration::from_secs(2)))?;
    s.write_all(format!("{cmd}\n").as_bytes())?;
    s.flush()?;
    let mut jawab = String::new();
    BufReader::new(s).read_line(&mut jawab)?;
    Ok(jawab.trim().to_string())
}
