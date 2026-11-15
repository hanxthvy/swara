//! Audio yang mengalir sambil diunduh.
//!
//! Dulu satu lagu diunduh utuh dulu (`Vec<u8>`) baru didekode, jadi setiap
//! pindah lagu menunggu seluruh file turun — 3-5 detik di jalur cepat, puluhan
//! detik kalau CDN-nya lambat. Di sini pengunduh menulis potongan ke `Buf` dan
//! dekoder membacanya seolah file biasa: `Read` akan menunggu potongan
//! berikutnya. Bunyi mulai setelah beberapa ratus KB, bukan setelah file utuh.
//!
//! `Buf` juga menyimpan seluruh file, jadi seek tetap bisa membangun ulang
//! dekoder dari awal lalu melewati `pos` detik seperti sebelumnya.

use std::io::{self, Read};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

struct St {
    data: Vec<u8>,
    done: bool,
    err: Option<String>,
}

pub struct Buf {
    st: Mutex<St>,
    cv: Condvar,
}

impl Buf {
    pub fn new() -> Arc<Self> {
        Arc::new(Buf {
            st: Mutex::new(St {
                data: Vec::new(),
                done: false,
                err: None,
            }),
            cv: Condvar::new(),
        })
    }

    pub fn push(&self, b: &[u8]) {
        let mut g = self.st.lock().unwrap();
        g.data.extend_from_slice(b);
        drop(g);
        self.cv.notify_all();
    }

    pub fn finish(&self) {
        let mut g = self.st.lock().unwrap();
        g.done = true;
        drop(g);
        self.cv.notify_all();
    }

    pub fn fail(&self, msg: String) {
        let mut g = self.st.lock().unwrap();
        if g.err.is_none() && !g.data.is_empty() {
            // Sudah ada data: biarkan pemutar menghabiskan yang ada, error
            // dilaporkan hanya kalau memang tidak ada yang bisa didekode.
            g.err = Some(msg);
            g.done = true;
        } else if g.err.is_none() {
            g.err = Some(msg);
            g.done = true;
        }
        drop(g);
        self.cv.notify_all();
    }

    pub fn error(&self) -> Option<String> {
        self.st.lock().unwrap().err.clone()
    }

    pub fn len(&self) -> usize {
        self.st.lock().unwrap().data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Tunggu sampai ada `n` byte (atau unduhan selesai/gagal).
    pub fn wait_for(&self, n: usize) {
        let mut g = self.st.lock().unwrap();
        while g.data.len() < n && !g.done {
            g = self.cv.wait_timeout(g, Duration::from_millis(500)).unwrap().0;
        }
    }

    /// Baca dari `pos`. Memblokir sampai ada data baru; `0` berarti EOF.
    fn read_at(&self, pos: &mut usize, out: &mut [u8]) -> io::Result<usize> {
        let mut g = self.st.lock().unwrap();
        loop {
            if *pos < g.data.len() {
                let n = (g.data.len() - *pos).min(out.len());
                out[..n].copy_from_slice(&g.data[*pos..*pos + n]);
                *pos += n;
                return Ok(n);
            }
            if g.done {
                if *pos == 0 {
                    if let Some(e) = &g.err {
                        return Err(io::Error::new(io::ErrorKind::Other, e.clone()));
                    }
                }
                return Ok(0);
            }
            g = self.cv.wait_timeout(g, Duration::from_millis(500)).unwrap().0;
        }
    }
}

/// Sumber data untuk symphonia: file yang tumbuh di belakang layar.
pub struct Grow {
    buf: Arc<Buf>,
    pos: usize,
}

impl Grow {
    pub fn new(buf: Arc<Buf>) -> Self {
        Grow { buf, pos: 0 }
    }
}

impl Read for Grow {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.buf.read_at(&mut self.pos, out)
    }
}

impl symphonia::core::io::MediaSource for Grow {
    fn is_seekable(&self) -> bool {
        false
    }
    fn byte_len(&self) -> Option<u64> {
        None
    }
}

impl std::io::Seek for Grow {
    fn seek(&mut self, _pos: std::io::SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "sumber mengalir tidak bisa di-seek",
        ))
    }
}

/// Unduh `url` di thread sendiri ke `Buf`. `tag` hanya untuk nama file dump.
pub fn download(
    client: reqwest::blocking::Client,
    url: String,
    ua: &'static str,
    tag: String,
) -> Arc<Buf> {
    let buf = Buf::new();
    let b2 = buf.clone();
    let _ = std::thread::Builder::new()
        .name("unduh-audio".into())
        .spawn(move || {
            let run = || -> anyhow::Result<()> {
                let mut resp = client
                    .get(&url)
                    .header("User-Agent", ua)
                    .header("Range", "bytes=0-")
                    .header("Accept-Encoding", "identity")
                    .timeout(Duration::from_secs(120))
                    .send()?;
                if !resp.status().is_success() && resp.status().as_u16() != 206 {
                    anyhow::bail!("stream http {}", resp.status());
                }
                let mut chunk = vec![0u8; 64 * 1024];
                loop {
                    match resp.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => b2.push(&chunk[..n]),
                        Err(e) => {
                            b2.fail(format!("unduh terputus: {e}"));
                            return Ok(());
                        }
                    }
                }
                b2.finish();
                // SWARA_DUMP=1: simpan audio yang selesai diunduh, dipakai
                // untuk mengukur sinkronisasi lirik (bandingkan rekaman output
                // dengan isi file).
                if std::env::var("SWARA_DUMP").is_ok() {
                    let g = b2.st.lock().unwrap();
                    let _ = std::fs::write(format!("/tmp/swara-dump-{tag}.m4a"), &g.data);
                }
                Ok(())
            };
            if let Err(e) = run() {
                b2.fail(format!("{e:#}"));
            }
        });
    buf
}
