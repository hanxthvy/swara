//! Pengganti `bridge.rs` di sistem selain Linux: tidak ada D-Bus, tidak ada
//! jembatan. Fungsinya cuma mempertahankan bentuk pemanggilan di `main.rs`.

use crate::conductor::Ctl as CondCtl;
use crate::mpris::{Ctl, Mpris};
use crate::player::{Progress, Remote};
use std::sync::{Arc, Mutex};

pub fn spawn(
    mpris: Option<Mpris>,
    _remote: Remote,
    _cond: CondCtl,
    _shared: Arc<Mutex<crate::conductor::Cond>>,
    _snap: Arc<Mutex<Progress>>,
) -> Option<Ctl> {
    mpris.map(|m| m.ctl())
}
