//! Sesi akun dari Chromium.
//!
//! Swara tidak pernah meminta kata sandi Google. Yang dipakai adalah cookie
//! YouTube yang sudah ada di profil Chromium di mesin ini: nilai `v10`
//! didekripsi dengan AES-128-CBC (kunci PBKDF2-SHA1 dari "peanuts", 1 iterasi,
//! IV 16 spasi), lalu 32 byte hash host dibuang dari depan. Setelah itu
//! `Authorization: SAPISIDHASH` dihitung dari SAPISID seperti yang dilakukan
//! web YouTube sendiri.

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Session {
    pub profile: String,
    /// user: nama yang ditampilkan di UI
    pub label: String,
    pub cookies: BTreeMap<String, String>,
    pub saved_at: u64,
}

fn cache_path() -> PathBuf {
    crate::paths::cache_dir().join("auth.json")
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn candidates() -> Vec<PathBuf> {
    // Profil bawaan Chromium, Chrome, Brave, Edge, dan Vivaldi di tiap sistem.
    let mut roots: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        if let Ok(la) = std::env::var("LOCALAPPDATA") {
            let la = PathBuf::from(la);
            for rel in [
                "Chromium\\User Data",
                "Google\\Chrome\\User Data",
                "BraveSoftware\\Brave-Browser\\User Data",
                "Microsoft\\Edge\\User Data",
                "Vivaldi\\User Data",
            ] {
                roots.push(la.join(rel));
            }
        }
    } else if cfg!(target_os = "macos") {
        if let Ok(h) = std::env::var("HOME") {
            let base = PathBuf::from(h).join("Library").join("Application Support");
            for rel in [
                "Chromium",
                "Google/Chrome",
                "BraveSoftware/Brave-Browser",
                "Microsoft Edge",
                "Vivaldi",
            ] {
                roots.push(base.join(rel));
            }
        }
    } else if let Ok(h) = std::env::var("HOME") {
        let base = PathBuf::from(h);
        for rel in [
            ".config/chromium",
            ".config/google-chrome",
            ".config/chromium-browser",
            ".config/BraveSoftware/Brave-Browser",
            ".config/microsoft-edge",
            ".config/vivaldi",
        ] {
            roots.push(base.join(rel));
        }
    }

    let mut out = Vec::new();
    for dir in roots {
        if dir.is_dir() {
            let def = dir.join("Default");
            out.push(if def.is_dir() { def } else { dir });
        }
    }
    out
}

pub fn load() -> Option<Session> {
    let b = std::fs::read(cache_path()).ok()?;
    let s: Session = serde_json::from_slice(&b).ok()?;
    if s.cookies.is_empty() {
        return None;
    }
    Some(s)
}

pub fn save(s: &Session) -> Result<()> {
    let p = cache_path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&p, serde_json::to_vec(s)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn clear() {
    let _ = std::fs::remove_file(cache_path());
}

fn key_peanuts() -> [u8; 16] {
    let mut out = [0u8; 16];
    pbkdf2::pbkdf2::<hmac::Hmac<Sha1>>(b"peanuts", b"saltysalt", 1, &mut out)
        .expect("pbkdf2");
    out
}

fn decrypt_v10(blob: &[u8], key: &[u8; 16]) -> Option<String> {
    if blob.len() <= 3 {
        return None;
    }
    let mut buf = blob[3..].to_vec();
    let iv = [b' '; 16];
    let dec = Aes128CbcDec::new(key.into(), &iv.into());
    let plain = dec.decrypt_padded_mut::<Pkcs7>(&mut buf).ok()?;
    // Chromium >= v24 menempelkan sha256(host_key) 32 byte di depan.
    let body = if plain.len() > 32 { &plain[32..] } else { plain };
    let s = String::from_utf8_lossy(body).trim_end_matches('\0').to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Ambil cookie youtube.com/google.com dari profil Chromium di `dir`.
pub fn read_profile(dir: &Path) -> Result<BTreeMap<String, String>> {
    let db = [dir.join("Cookies"), dir.join("Network").join("Cookies")]
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| anyhow!("Cookies tidak ada di {}", dir.display()))?;
    // Chromium memegang lock; salin ke berkas sementara.
    // TMPDIR bisa menunjuk ke direktori milik pengguna lain (mis. sesi root),
    // jadi salinan dibuat di cache milik pengguna sendiri.
    let Some(tmpdir) = cache_path().parent().map(|p| p.to_path_buf()) else {
        bail!("cache swara tidak diketahui");
    };
    std::fs::create_dir_all(&tmpdir)?;
    let tmp = tmpdir.join(format!("cookies-{}.db", std::process::id()));
    std::fs::copy(&db, &tmp).map_err(|e| anyhow!("salin {}: {e}", db.display()))?;
    let con = rusqlite::Connection::open_with_flags(
        &tmp,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let mut stmt = con.prepare(
        "select host_key, name, encrypted_value, value from cookies \
         where host_key like '%youtube.com%' or host_key like '%google.com%'",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Vec<u8>>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let key = key_peanuts();
    let mut jar: BTreeMap<String, (String, String)> = BTreeMap::new();
    for row in rows.flatten() {
        let (host, name, enc, plain) = row;
        let value = if !plain.is_empty() {
            plain
        } else if enc.starts_with(b"v10") {
            match decrypt_v10(&enc, &key) {
                Some(v) => v,
                None => continue,
            }
        } else {
            String::new()
        };
        if value.is_empty() {
            continue;
        }
        // Domain yang lebih spesifik menang; cukup satu nilai per nama.
        let mut keep = true;
        if let Some((old_host, _)) = jar.get(&name) {
            keep = host.len() > old_host.len();
        }
        if keep {
            jar.insert(name, (host, value));
        }
    }
    drop(stmt);
    drop(con);
    let _ = std::fs::remove_file(&tmp);
    Ok(jar.into_iter().map(|(k, (_, v))| (k, v)).collect())
}

/// Impor sesi. Di Android diambil dari WebView, di Linux dari profil Chromium.
pub fn import_any() -> Result<Session> {
    #[cfg(target_os = "android")]
    {
        return import_webview();
    }
    #[cfg(not(target_os = "android"))]
    {
        import_chromium()
    }
}

/// Android: cookie sudah ditulis `Masuk.java` setelah pengguna login di WebView.
/// Kalau belum ada, halaman login dibuka sendiri dan fungsi ini gagal dengan
/// pesan yang jelas, bukan diam-diam tanpa akun.
#[cfg(target_os = "android")]
fn import_webview() -> Result<Session> {
    let header = crate::jembatan_android::baca_cookie().unwrap_or_default();
    let mut jar = BTreeMap::new();
    for kv in header.split(';') {
        if let Some((k, v)) = kv.trim().split_once('=') {
            if !k.is_empty() {
                jar.insert(k.to_string(), v.to_string());
            }
        }
    }
    if !(jar.contains_key("SAPISID") || jar.contains_key("__Secure-3PAPISID")) {
        crate::jembatan_android::buka_login();
        if jar.is_empty() {
            bail!("halaman login dibuka: masuk lewat Google, lalu kembali ke Swara");
        }
        bail!("login belum selesai: selesaikan di halaman Google lalu kembali");
    }
    let s = Session {
        profile: "webview".into(),
        label: String::new(),
        cookies: jar,
        saved_at: now(),
    };
    save(&s)?;
    Ok(s)
}

/// Muat sesi dari profil Chromium pertama yang punya cookie login.
fn import_chromium() -> Result<Session> {
    // Chromium di Linux mengenkripsi cookie dengan kunci dari "peanuts".
    // Di Windows kuncinya lewat DPAPI dan di macOS lewat Keychain, jadi impor
    // sesi belum bisa dilakukan di sana; Swara tetap jalan tanpa akun.
    if !cfg!(target_os = "linux") {
        bail!(
            "impor sesi Chromium baru didukung di Linux. Di sistem ini Swara tetap \
             bisa mencari dan memutar, hanya fitur akun yang belum aktif"
        );
    }
    let mut last = None;
    for dir in candidates() {
        match read_profile(&dir) {
            Ok(jar) => {
                if jar.contains_key("SAPISID") || jar.contains_key("__Secure-3PAPISID") {
                    let s = Session {
                        profile: dir.display().to_string(),
                        label: String::new(),
                        cookies: jar,
                        saved_at: now(),
                    };
                    save(&s)?;
                    return Ok(s);
                }
                last = Some(format!("{}: tanpa sesi YouTube", dir.display()));
            }
            Err(e) => last = Some(format!("{e}")),
        }
    }
    Err(anyhow!(
        "{}",
        last.unwrap_or_else(|| "tidak ada profil Chromium ditemukan".into())
    ))
}

pub fn cookie_header(s: &Session) -> String {
    s.cookies
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Header `Authorization` gaya web YouTube.
pub fn auth_headers(s: &Session) -> Vec<(String, String)> {
    let sapisid = s
        .cookies
        .get("SAPISID")
        .or_else(|| s.cookies.get("__Secure-3PAPISID"))
        .cloned()
        .unwrap_or_default();
    if sapisid.is_empty() {
        return Vec::new();
    }
    let ts = now();
    let mut h = Sha1::new();
    h.update(format!("{ts} {sapisid} https://music.youtube.com").as_bytes());
    let digest: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    vec![
        ("Cookie".into(), cookie_header(s)),
        ("Authorization".into(), format!("SAPISIDHASH {ts}_{digest}")),
        ("X-Goog-AuthUser".into(), "0".into()),
        ("X-Origin".into(), "https://music.youtube.com".into()),
    ]
}
