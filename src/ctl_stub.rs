//! Pengganti `ctl` di luar Linux: tidak ada soket Unix, jadi yang dipakai
//! hanya keadaan di memori (dipakai Android/Windows).

use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
pub struct Keadaan {
    pub id: String,
    pub suka: bool,
    pub masuk: bool,
}

pub type Bersama = Arc<Mutex<Keadaan>>;

pub fn bersama() -> Bersama {
    Arc::new(Mutex::new(Keadaan::default()))
}

pub fn tulis(_k: &Bersama) {}

pub fn spawn(_pintu: crate::worker::Pintu, _k: Bersama) {}

pub fn kirim(_cmd: &str) -> anyhow::Result<String> {
    Err(anyhow::anyhow!("kendali luar hanya tersedia di Linux"))
}
